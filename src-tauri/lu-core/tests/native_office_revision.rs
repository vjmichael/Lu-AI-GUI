use std::{fs,io::{Cursor,Write,Read},sync::atomic::{AtomicU64,Ordering}};
use sha2::{Digest,Sha256};
use cap_std::{ambient_authority,fs::Dir};
use zip::{ZipWriter,ZipArchive,write::SimpleFileOptions,CompressionMethod};
use lu_core::{
    native_office_revision::{
        OfficeRevisionEdit,OfficeRevisionError,render_office_revision,publish_office_revision_new,
    },
    native_docx_create::{render_docx,DocxBlock,DocxBlockKind},
    native_spreadsheet_create::{render_xlsx,SpreadsheetCell,CellValue},
    native_reader_docx::read_docx_bytes,
    native_reader_office::read_xlsx,
};
static SEQ:AtomicU64=AtomicU64::new(0);
fn hash(b:&[u8])->String{format!("{:x}",Sha256::digest(b))}
fn fixture()->(std::path::PathBuf,Dir){
    let path=std::env::temp_dir().join(format!(
        "lu-ooxml-version-{}-{}",std::process::id(),SEQ.fetch_add(1,Ordering::SeqCst)
    ));
    fs::create_dir_all(&path).unwrap();
    let dir=Dir::open_ambient_dir(&path,ambient_authority()).unwrap();
    (path,dir)
}
fn part(zip_bytes:&[u8],name:&str)->Vec<u8>{
    let mut zip=ZipArchive::new(Cursor::new(zip_bytes)).unwrap();
    let mut data=Vec::new();
    zip.by_name(name).unwrap().read_to_end(&mut data).unwrap();
    data
}
#[test]
fn revised_word_text_retains_other_package_parts_and_original(){
    let source=render_docx(&[
        DocxBlock{kind:DocxBlockKind::Title,text:"Prior & Title".into()},
        DocxBlock{kind:DocxBlockKind::Paragraph,text:"Unchanged body".into()},
    ]).unwrap();
    let old_sha=hash(&source);
    let edit=OfficeRevisionEdit::WordRun{
        text_index:0,expected_old:"Prior & Title".into(),
        new_text:"Revised <Title> & new".into(),
    };
    let revised=render_office_revision(&source,&old_sha,&edit).unwrap();
    assert_eq!(part(&source,"word/styles.xml"),
        part(&revised.revised_bytes,"word/styles.xml"));
    assert_eq!(part(&source,"word/_rels/document.xml.rels"),
        part(&revised.revised_bytes,"word/_rels/document.xml.rels"));
    let parsed=read_docx_bytes(&revised.revised_bytes,4096).unwrap();
    assert!(parsed.text.contains("Revised <Title> & new"));
    assert!(parsed.text.contains("Unchanged body"));
    let original=read_docx_bytes(&source,4096).unwrap();
    assert!(original.text.contains("Prior & Title"));
    let (folder,dir)=fixture();
    fs::write(folder.join("draft.docx"),&source).unwrap();
    let published=publish_office_revision_new(
        &dir,"draft.docx",&dir,"draft-v2.docx",
        "transaction-revised-12345678",&old_sha,&revised.revised_sha256,&edit
    ).unwrap();
    assert_eq!(published.sha256,revised.revised_sha256);
    assert_eq!(fs::read(folder.join("draft.docx")).unwrap(),source);
    assert_eq!(fs::read(folder.join("draft-v2.docx")).unwrap(),
        revised.revised_bytes);
    // The new revision may not overwrite a user existing file.
    assert!(publish_office_revision_new(
        &dir,"draft.docx",&dir,"draft-v2.docx",
        "transaction-revised-87654321",&old_sha,&revised.revised_sha256,&edit
    ).is_err());
    drop(dir);fs::remove_dir_all(folder).unwrap();
}
#[test]
fn revised_existing_numeric_cell_retains_workbook_sheets_and_text(){
    let source=render_xlsx("Budget",&[
        SpreadsheetCell{row:1,column:1,value:CellValue::Text("Monthly costs".into())},
        SpreadsheetCell{row:2,column:2,value:CellValue::Number(25.0)},
        SpreadsheetCell{row:3,column:2,value:CellValue::Formula("SUM(B2:B2)".into())},
    ]).unwrap();
    let revision=render_office_revision(&source,&hash(&source),
        &OfficeRevisionEdit::CellNumber{
            sheet_number:1,address:"B2".into(),expected_old:25.0,new_number:27.5,
        }
    ).unwrap();
    let xml=String::from_utf8(part(&revision.revised_bytes,
        "xl/worksheets/sheet1.xml")).unwrap();
    assert!(xml.contains(r#"<c r="B2" t="n"><v>27.5</v></c>"#));
    assert!(xml.contains("SUM(B2:B2)"));
    assert_eq!(part(&revision.revised_bytes,"xl/sharedStrings.xml"),
        part(&source,"xl/sharedStrings.xml"));
    assert_eq!(part(&revision.revised_bytes,"xl/workbook.xml"),
        part(&source,"xl/workbook.xml"));
    let read=read_xlsx(&revision.revised_bytes,4096).unwrap();
    assert!(read.text.contains("B2=27.5"));
    assert!(read.text.contains("Monthly costs"));
}
#[test]
fn simple_powerpoint_text_edit_preserves_untouched_slide(){
    let raw=r#"<?xml version="1.0"?><p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:t>Before</a:t><a:t>Unchanged</a:t></p:sld>"#;
    let mut z=ZipWriter::new(Cursor::new(Vec::new()));
    z.start_file("ppt/slides/slide1.xml",SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)).unwrap();
    z.write_all(raw.as_bytes()).unwrap();
    z.start_file("ppt/slides/slide2.xml",SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)).unwrap();
    z.write_all(b"<a:t>Other slide</a:t>").unwrap();
    let original=z.finish().unwrap().into_inner();
    let revised=render_office_revision(&original,&hash(&original),
        &OfficeRevisionEdit::SlideRun{slide_number:1,text_index:0,
            expected_old:"Before".into(),new_text:"After".into()}
    ).unwrap();
    assert!(String::from_utf8(part(&revised.revised_bytes,"ppt/slides/slide1.xml"))
        .unwrap().contains("<a:t>After</a:t>"));
    assert_eq!(part(&revised.revised_bytes,"ppt/slides/slide2.xml"),
        part(&original,"ppt/slides/slide2.xml"));
}
#[test]
fn package_edits_fail_on_changed_baseline_unsupported_cell_and_signed_parts(){
    let original=render_xlsx("Budget",&[
        SpreadsheetCell{row:1,column:1,value:CellValue::Text("Do not strip strings".into())},
        SpreadsheetCell{row:2,column:1,value:CellValue::Formula("1+2".into())},
    ]).unwrap();
    let edit=OfficeRevisionEdit::CellNumber{
        sheet_number:1,address:"A1".into(),expected_old:0.0,new_number:5.0,
    };
    assert_eq!(render_office_revision(&original,&"0".repeat(64),&edit)
        .unwrap_err(),OfficeRevisionError::SourceChanged);
    assert_eq!(render_office_revision(&original,&hash(&original),&edit)
        .unwrap_err(),OfficeRevisionError::UnsupportedCell);
    let mut zip=ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file("word/document.xml",SimpleFileOptions::default()).unwrap();
    zip.write_all(b"<w:t>before</w:t>").unwrap();
    zip.start_file("word/vbaProject.bin",SimpleFileOptions::default()).unwrap();
    zip.write_all(b"macro").unwrap();
    let macro_package=zip.finish().unwrap().into_inner();
    assert_eq!(render_office_revision(&macro_package,&hash(&macro_package),
        &OfficeRevisionEdit::WordRun{
            text_index:0,expected_old:"before".into(),new_text:"after".into(),
        }).unwrap_err(),OfficeRevisionError::UnsupportedPackage);
}
