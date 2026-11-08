//! Actual reviewed E1 artifact producer: one fixed operation and two
//! host-selected document kinds, no arbitrary filenames or user scripts.
//! The only output path is the OS-owned STDOUT pipe consumed by Lu Core.
//! Worker does not open or choose an output file or any network connection.
use std::{io::{self,Write,Cursor},ffi::OsStr};
use sha2::{Digest,Sha256};
use zip::{ZipWriter,write::SimpleFileOptions,CompressionMethod};
use lu_core::native_spreadsheet_create::{
    render_xlsx,SpreadsheetCell,CellValue,
};
use lu_core::native_app_execution::{
    E1_VERSION_MARKER_BYTES
};
const MODE:&str="--lu-e1-artifact-one-shot";

fn make_docx()->Result<Vec<u8>,Box<dyn std::error::Error>>{
    let document=r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Lu E1 artifact custody verified</w:t></w:r></w:p></w:body></w:document>"#;
    let content_types=r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#;
    let root_rels=r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
    let mut archive=ZipWriter::new(Cursor::new(Vec::new()));
    for (name,part) in [
        ("[Content_Types].xml",content_types),
        ("_rels/.rels",root_rels),
        ("word/document.xml",document),
    ]{
        archive.start_file(name,SimpleFileOptions::default()
            .compression_method(CompressionMethod::Stored))?;
        archive.write_all(part.as_bytes())?;
    }
    Ok(archive.finish()?.into_inner())
}
fn make_xlsx()->Result<Vec<u8>,Box<dyn std::error::Error>>{
    let cells=vec![
        SpreadsheetCell{row:1,column:1,value:CellValue::Text("Status".into())},
        SpreadsheetCell{row:1,column:2,value:CellValue::Text("Reviewed".into())},
        SpreadsheetCell{row:2,column:1,value:CellValue::Text("Frames".into())},
        SpreadsheetCell{row:2,column:2,value:CellValue::Number(48.0)},
    ];
    Ok(render_xlsx("E1",&cells)
        .map_err(|_|io::Error::other("reviewed workbook rendering failed"))?)
}
fn produce()->Result<(),Box<dyn std::error::Error>>{
    let mut args=std::env::args_os().skip(1);
    if args.next().as_deref()!=Some(OsStr::new(MODE)){
        return Err("wrong owned worker mode".into())
    }
    let format=args.next().ok_or("missing host-reviewed format")?;
    if args.next().is_some(){return Err("unexpected arguments".into())}
    let (format,bytes)=match format.to_str(){
        Some("docx")=>("docx",make_docx()?),
        Some("xlsx")=>("xlsx",make_xlsx()?),
        _=>return Err("unsupported reviewed document format".into()),
    };
    let mut out=io::stdout().lock();
    let count=E1_VERSION_MARKER_BYTES.len();
    writeln!(out,"P\t0\t{count}")?;
    writeln!(out,"P\t{count}\t{count}")?;
    writeln!(out,"D\t{:x}",Sha256::digest(E1_VERSION_MARKER_BYTES))?;
    writeln!(out,"A\t{format}\t{}\t{:x}",bytes.len(),Sha256::digest(&bytes))?;
    out.write_all(&bytes)?;
    out.flush()?;
    Ok(())
}
fn main(){
    if produce().is_err(){std::process::exit(2)}
}
