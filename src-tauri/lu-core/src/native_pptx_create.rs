//! WAVE2-PPTX: bounded text-first PowerPoint creation.
//! Host must independently bind an exact content digest, new target, saved
//! human turn and durable effect journal. The engine has NO authority itself.
//! No scripts, external resources, macros, OLE, audio or embedded objects.
use std::io::{Cursor,Write};
use cap_std::fs::Dir;
use zip::{write::SimpleFileOptions,CompressionMethod,ZipWriter};
use crate::native_atomic_write::{create_new_atomic_in_dir,AtomicWriteError,WriteReceipt};
const MAX_SLIDES:usize=64;
const MAX_BULLETS:usize=20;
const MAX_TEXT:usize=8*1024;
const MAX_PACKAGE:usize=1024*1024;
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct PresentationSlide{pub title:String,pub bullet_points:Vec<String>}
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum PptxCreateError{
    InvalidName,InvalidContent,TooLarge,Empty,Zip(String),Publish(AtomicWriteError),
}
fn text_ok(s:&str)->bool{
    s.len()<=MAX_TEXT&&s.chars().all(|c|c=='\t'||c=='\n'||c=='\r'
        ||(!c.is_control()&&c!='\u{FFFE}'&&c!='\u{FFFF}'))
}
fn name_ok(name:&str)->bool{
    !name.is_empty()&&name.len()<=200&&name!="."&&name!=".."
        &&!name.starts_with(".lu-stage-")&&!name.ends_with('.')&&!name.ends_with(' ')
        &&!name.chars().any(|c|c.is_control()||matches!(c,'/'|'\\'|':'))
        &&name.to_ascii_lowercase().ends_with(".pptx")
}
fn escape_xml(t:&str)->String{
    let mut output=String::new();
    for c in t.chars(){
        match c{
            '&'=>output.push_str("&amp;"),'<'=>output.push_str("&lt;"),
            '>'=>output.push_str("&gt;"),'"'=>output.push_str("&quot;"),
            '\''=>output.push_str("&apos;"),_=>output.push(c),
        }
    }
    output
}
fn shape(id:u32,name:&str,x:u32,y:u32,w:u32,h:u32,content:&str,size:u32,bold:bool)->String{
    let mut body=String::from(r#"<a:p><a:pPr algn="l"/><a:r><a:rPr lang="en-US" sz=""#);
    body.push_str(&size.to_string());
    body.push('"');
    if bold{body.push_str(r#" b="1""#);}
    body.push_str("/><a:t>");
    body.push_str(&escape_xml(content));
    body.push_str("</a:t></a:r></a:p>");
    format!(r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="{name}"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{w}" cy="{h}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:noFill/></p:spPr><p:txBody><a:bodyPr wrap="square"><a:spAutoFit/></a:bodyPr><a:lstStyle/>{body}</p:txBody></p:sp>"#)
}
fn slide_xml(slide:&PresentationSlide)->String{
    let mut content=String::from(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>"#);
    content.push_str(&shape(2,"Slide title",650000,520000,11000000,1600000,&slide.title,3200,true));
    if !slide.bullet_points.is_empty(){
        let mut body=String::new();
        for bullet in &slide.bullet_points{
            body.push_str("<a:p><a:pPr lvl=\"0\" marL=\"300000\" indent=\"-200000\"><a:buChar char=\"•\"/></a:pPr><a:r><a:rPr lang=\"en-US\" sz=\"2200\"/><a:t>");
            body.push_str(&escape_xml(bullet));
            body.push_str("</a:t></a:r></a:p>");
        }
        content.push_str(r#"<p:sp><p:nvSpPr><p:cNvPr id="3" name="Bullet points"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="800000" y="2250000"/><a:ext cx="10400000" cy="4000000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:noFill/></p:spPr><p:txBody><a:bodyPr wrap="square"><a:spAutoFit/></a:bodyPr><a:lstStyle/>"#);
        content.push_str(&body);
        content.push_str("</p:txBody></p:sp>");
    }
    content.push_str("</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>");
    content
}
fn write_part<W:Write+std::io::Seek>(
    zip:&mut ZipWriter<W>,name:&str,content:&str
)->Result<(),PptxCreateError>{
    if content.len()>MAX_PACKAGE{return Err(PptxCreateError::TooLarge)}
    zip.start_file(name,SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored))
        .map_err(|e|PptxCreateError::Zip(e.to_string()))?;
    zip.write_all(content.as_bytes()).map_err(|e|PptxCreateError::Zip(e.to_string()))
}
/// A minimal no-master PPTX accepted by common OpenXML readers. Slide content
/// is owned by Lu; no inherited external theme or reference dependencies.
pub fn render_pptx(slides:&[PresentationSlide])->Result<Vec<u8>,PptxCreateError>{
    if slides.is_empty(){return Err(PptxCreateError::Empty)}
    if slides.len()>MAX_SLIDES ||slides.iter().any(|s|
        s.bullet_points.len()>MAX_BULLETS || !text_ok(&s.title) ||
        s.title.is_empty() ||s.bullet_points.iter().any(|b|!text_ok(b))
    ){return Err(PptxCreateError::InvalidContent)}
    let mut types=String::from(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>"#);
    let mut ids=String::new();
    let mut relationships=String::from(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#);
    for (index,_) in slides.iter().enumerate(){
        let num=index+1;
        types.push_str(&format!(r#"<Override PartName="/ppt/slides/slide{num}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>"#));
        ids.push_str(&format!(r#"<p:sldId id="{}" r:id="rId{num}"/>"#,256+index));
        relationships.push_str(&format!(r#"<Relationship Id="rId{num}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide{num}.xml"/>"#));
    }
    types.push_str("</Types>");
    relationships.push_str("</Relationships>");
    let presentation=format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><p:sldIdLst>{ids}</p:sldIdLst><p:sldSz cx="12192000" cy="6858000"/><p:notesSz cx="6858000" cy="9144000"/></p:presentation>"#);
    let root_rels=r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/></Relationships>"#;
    let mut zip=ZipWriter::new(Cursor::new(Vec::new()));
    for (name,content) in [
        ("[Content_Types].xml",types.as_str()),
        ("_rels/.rels",root_rels),
        ("ppt/presentation.xml",presentation.as_str()),
        ("ppt/_rels/presentation.xml.rels",relationships.as_str()),
    ]{write_part(&mut zip,name,content)?}
    for (i,slide) in slides.iter().enumerate(){
        write_part(&mut zip,&format!("ppt/slides/slide{}.xml",i+1),
            &slide_xml(slide))?;
    }
    let output=zip.finish().map_err(|e|PptxCreateError::Zip(e.to_string()))?
        .into_inner();
    if output.len()>MAX_PACKAGE{return Err(PptxCreateError::TooLarge)}
    Ok(output)
}
pub fn create_pptx_new(
    dir:&Dir,target_name:&str,stage_id:&str,slides:&[PresentationSlide],
)->Result<WriteReceipt,PptxCreateError>{
    if !name_ok(target_name){return Err(PptxCreateError::InvalidName)}
    let bytes=render_pptx(slides)?;
    create_new_atomic_in_dir(dir,target_name,stage_id,&bytes)
        .map_err(PptxCreateError::Publish)
}
