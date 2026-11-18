use cap_std::{ambient_authority,fs::Dir};
use lu_core::native_docx_create::{
    create_docx_new,render_docx,DocxBlock,DocxBlockKind,DocxCreateError,
};
use std::{fs,io::Read,sync::atomic::{AtomicU64,Ordering}};
static SEQ:AtomicU64=AtomicU64::new(0);
fn paragraph(kind:DocxBlockKind,text:&str)->DocxBlock{
    DocxBlock{kind,text:text.into()}
}
#[test]
fn native_docx_package_opens_as_zip_and_roundtrips_reader(){
    let package=render_docx(&[
        paragraph(DocxBlockKind::Title,"A New Document"),
        paragraph(DocxBlockKind::Heading1,"Introduction & Findings"),
        paragraph(DocxBlockKind::Paragraph,"This is a <test> of Word.\nAnother line."),
        paragraph(DocxBlockKind::Heading2,"Numbers"),
        paragraph(DocxBlockKind::Paragraph,"Amount:\t42"),
    ]).unwrap();
    let mut zip=zip::ZipArchive::new(std::io::Cursor::new(&package)).unwrap();
    let mut xml=String::new();
    zip.by_name("word/document.xml").unwrap().read_to_string(&mut xml).unwrap();
    assert!(xml.contains("<w:pStyle w:val=\"Heading1\"/>"));
    assert!(xml.contains("Introduction &amp; Findings"));
    assert!(xml.contains("&lt;test&gt;"));
    assert!(xml.contains("<w:br/>"));
    assert!(xml.contains("<w:tab/>"));
    let text=lu_core::native_reader_docx::read_docx_bytes(&package,4096).unwrap();
    assert!(text.text.contains("A New Document"));
    assert!(text.text.contains("Introduction & Findings"));
    assert!(text.text.contains("This is a <test> of Word."));
}
#[test]
fn creation_cannot_overwrite_existing_word_document(){
    let dir=std::env::temp_dir().join(format!("lu-docx-{}-{}",
        std::process::id(),SEQ.fetch_add(1,Ordering::SeqCst)));
    fs::create_dir_all(&dir).unwrap();
    let handle=Dir::open_ambient_dir(&dir,ambient_authority()).unwrap();
    let content=[paragraph(DocxBlockKind::Paragraph,"hello")];
    create_docx_new(&handle,"letter.docx","transaction-123456789",&content).unwrap();
    let original=fs::read(dir.join("letter.docx")).unwrap();
    assert!(create_docx_new(&handle,"letter.docx","transaction-987654321",&content).is_err());
    assert_eq!(fs::read(dir.join("letter.docx")).unwrap(),original);
    drop(handle);fs::remove_dir_all(dir).unwrap();
}
#[test]
fn limits_reject_invalid_xml_and_unsafe_names(){
    assert_eq!(render_docx(&[]).unwrap_err(),DocxCreateError::Empty);
    assert_eq!(render_docx(&[paragraph(DocxBlockKind::Paragraph,"hello\0evil")]).unwrap_err(),
        DocxCreateError::InvalidText);
    let dir=std::env::temp_dir().join(format!("lu-docx-{}-{}",
        std::process::id(),SEQ.fetch_add(1,Ordering::SeqCst)));
    fs::create_dir_all(&dir).unwrap();
    let handle=Dir::open_ambient_dir(&dir,ambient_authority()).unwrap();
    let result=create_docx_new(&handle,"../escape.docx",
        "transaction-123456789",&[paragraph(DocxBlockKind::Paragraph,"safe")]);
    assert_eq!(result.unwrap_err(),DocxCreateError::InvalidName);
    drop(handle);fs::remove_dir_all(dir).unwrap();
}
