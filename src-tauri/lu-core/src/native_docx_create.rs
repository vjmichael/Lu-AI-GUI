//! WAVE2-DOCX: deterministic structured Word document creation (new file).
//! Host must approve the exact rendered package and destination and use
//! durable create-new transaction. This is neither a model tool nor a grant.
//! OOXML is generated without macros, external links, embedded objects or
//! network lookups.
use std::io::{Cursor,Write};
use cap_std::fs::Dir;
use zip::{ZipWriter,write::SimpleFileOptions,CompressionMethod};
use crate::native_atomic_write::{create_new_atomic_in_dir,AtomicWriteError,WriteReceipt};

const MAX_BLOCKS:usize=512;
const MAX_CHARS:usize=200_000;
const MAX_BLOCK_BYTES:usize=32_768;
const MAX_PACKAGE_BYTES:usize=1024*1024;
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum DocxBlockKind {Title,Heading1,Heading2,Paragraph}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct DocxBlock{pub kind:DocxBlockKind,pub text:String}
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum DocxCreateError{
    InvalidName,InvalidText,TooLarge,Empty,Package(String),Publish(AtomicWriteError),
}
fn valid_name(name:&str)->bool{
    !name.is_empty() && name.len()<=200 && name!="." && name!=".."
        && !name.starts_with(".lu-stage-")&&!name.ends_with('.')&&!name.ends_with(' ')
        && !name.chars().any(|c|c.is_control()||matches!(c,'\\'|'/'|':'))
        && name.to_ascii_lowercase().ends_with(".docx")
}
fn xml(text:&str)->String{
    let mut escaped=String::with_capacity(text.len());
    for ch in text.chars(){
        match ch{
            '&'=>escaped.push_str("&amp;"),'<'=>escaped.push_str("&lt;"),
            '>'=>escaped.push_str("&gt;"),'"'=>escaped.push_str("&quot;"),
            '\''=>escaped.push_str("&apos;"),_=>escaped.push(ch),
        }
    }
    escaped
}
fn valid_text(text:&str)->bool{
    text.len()<=MAX_BLOCK_BYTES && text.chars().all(|ch|
        ch=='\t'||ch=='\n'||ch=='\r'||(!ch.is_control()&&ch!='\u{FFFE}'&&ch!='\u{FFFF}'))
}
fn paragraph(kind:DocxBlockKind,value:&str)->String{
    let style=match kind{
        DocxBlockKind::Title=>"<w:pPr><w:pStyle w:val=\"Title\"/></w:pPr>",
        DocxBlockKind::Heading1=>"<w:pPr><w:pStyle w:val=\"Heading1\"/></w:pPr>",
        DocxBlockKind::Heading2=>"<w:pPr><w:pStyle w:val=\"Heading2\"/></w:pPr>",
        DocxBlockKind::Paragraph=>"",
    };
    let mut runs=String::new();
    for (i,line) in value.split('\n').enumerate(){
        if i>0{runs.push_str("<w:r><w:br/></w:r>")}
        for (j,segment) in line.split('\t').enumerate(){
            if j>0{runs.push_str("<w:r><w:tab/></w:r>")}
            if !segment.is_empty(){
                runs.push_str("<w:r><w:t xml:space=\"preserve\">");
                runs.push_str(&xml(segment));
                runs.push_str("</w:t></w:r>");
            }
        }
    }
    format!("<w:p>{style}{runs}</w:p>")
}
fn part<W:Write+std::io::Seek>(
    zip:&mut ZipWriter<W>,name:&str,body:&str
)->Result<(),DocxCreateError>{
    if body.len()>MAX_PACKAGE_BYTES{return Err(DocxCreateError::TooLarge)}
    zip.start_file(name,SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored))
        .map_err(|e|DocxCreateError::Package(e.to_string()))?;
    zip.write_all(body.as_bytes()).map_err(|e|DocxCreateError::Package(e.to_string()))
}
/// Render a genuine OOXML DOCX with Word-native paragraph styles.
pub fn render_docx(blocks:&[DocxBlock])->Result<Vec<u8>,DocxCreateError>{
    if blocks.is_empty(){return Err(DocxCreateError::Empty)}
    if blocks.len()>MAX_BLOCKS{return Err(DocxCreateError::TooLarge)}
    if blocks.iter().any(|b|!valid_text(&b.text)){
        return Err(DocxCreateError::InvalidText)
    }
    if blocks.iter().map(|b|b.text.chars().count()).sum::<usize>()>MAX_CHARS{
        return Err(DocxCreateError::TooLarge)
    }
    let mut document=String::from(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>"#);
    for block in blocks{
        document.push_str(&paragraph(block.kind,&block.text));
        if document.len()>MAX_PACKAGE_BYTES{return Err(DocxCreateError::TooLarge)}
    }
    document.push_str(r#"<w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"/></w:sectPr></w:body></w:document>"#);
    let styles=r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style><w:style w:type="paragraph" w:styleId="Title"><w:name w:val="Title"/><w:basedOn w:val="Normal"/><w:rPr><w:b/><w:sz w:val="40"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:rPr><w:b/><w:sz w:val="32"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Heading2"><w:name w:val="heading 2"/><w:basedOn w:val="Normal"/><w:rPr><w:b/><w:sz w:val="26"/></w:rPr></w:style></w:styles>"#;
    let types=r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/></Types>"#;
    let root_rels=r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
    let doc_rels=r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#;
    let mut zip=ZipWriter::new(Cursor::new(Vec::new()));
    for (name,body) in [
        ("[Content_Types].xml",types),
        ("_rels/.rels",root_rels),
        ("word/document.xml",&document),
        ("word/_rels/document.xml.rels",doc_rels),
        ("word/styles.xml",styles),
    ]{part(&mut zip,name,body)?}
    let bytes=zip.finish().map_err(|e|DocxCreateError::Package(e.to_string()))?.into_inner();
    if bytes.len()>MAX_PACKAGE_BYTES{return Err(DocxCreateError::TooLarge)}
    Ok(bytes)
}
pub fn create_docx_new(
    dir:&Dir,target_name:&str,stage_id:&str,blocks:&[DocxBlock],
)->Result<WriteReceipt,DocxCreateError>{
    if !valid_name(target_name){return Err(DocxCreateError::InvalidName)}
    let bytes=render_docx(blocks)?;
    create_new_atomic_in_dir(dir,target_name,stage_id,&bytes)
        .map_err(DocxCreateError::Publish)
}
