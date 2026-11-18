//! Bounded EPUB text spine reader. Strict existing ZIP admission, no scripts,
//! external links, media fetches, DRM decoding or XHTML rendering.
use crate::{
 native_reader::{ReaderFailure,ReaderFormat,ReaderText,MAX_READER_OUTPUT_CHARS},
 native_reader_openxml::OpenXmlPackage,
 native_reader_office::{attr,xml_value},
};
use quick_xml::{Reader,events::Event};
use std::collections::BTreeMap;
fn bad()->ReaderFailure{ReaderFailure::InvalidDocument}
fn safe_path(base:&str,relative:&str)->Result<String,ReaderFailure>{
 if relative.is_empty()||relative.len()>256||relative.starts_with('/')||
   relative.contains(['\\',':','?','#','%','\0']){return Err(bad())}
 let mut parts=base.split('/').map(str::to_owned).collect::<Vec<_>>();
 parts.pop();
 for segment in relative.split('/'){
  match segment{
   ""|"."=>return Err(bad()),
   ".."=>{if parts.pop().is_none(){return Err(bad())}},
   s=>parts.push(s.to_owned()),
  }
 }
 let path=parts.join("/");
 if path.len()>512{return Err(bad())}
 Ok(path)
}
fn root_path(xml:&[u8])->Result<String,ReaderFailure>{
 let mut r=Reader::from_reader(xml);let mut count=0usize;
 let mut found=None::<String>;
 loop{
  count+=1;if count>10_000{return Err(ReaderFailure::TooLarge)}
  match r.read_event().map_err(|_|bad())?{
   Event::Start(ref e)|Event::Empty(ref e)
     if e.local_name().as_ref()==b"rootfile"=>{
    let p=attr(e,b"full-path").ok_or_else(bad)?;
    if p.starts_with('/')||p.len()>256||!p.ends_with(".opf")||
       p.contains(['\\',':','?','#','%','\0'])||
       p.split('/').any(|x|x==""||x=="."||x==".."){return Err(bad())}
    if found.replace(p).is_some(){return Err(bad())}
   },
   Event::DocType(_)=>return Err(bad()),
   Event::Eof=>break,
   _=>{}
  }
 }
 found.ok_or_else(bad)
}
fn spine(xml:&[u8],root:&str)->Result<Vec<String>,ReaderFailure>{
 let mut r=Reader::from_reader(xml);
 let mut items=BTreeMap::<String,String>::new();
 let mut order=Vec::<String>::new();
 let mut manifest=false;let mut body=false;let mut count=0;
 loop{
  count+=1;if count>250_000{return Err(ReaderFailure::TooLarge)}
  match r.read_event().map_err(|_|bad())?{
   Event::Start(ref e) if e.local_name().as_ref()==b"manifest"=>manifest=true,
   Event::End(ref e) if e.local_name().as_ref()==b"manifest"=>manifest=false,
   Event::Start(ref e) if e.local_name().as_ref()==b"spine"=>body=true,
   Event::End(ref e) if e.local_name().as_ref()==b"spine"=>body=false,
   Event::Start(ref e)|Event::Empty(ref e)
     if manifest&&e.local_name().as_ref()==b"item"=>{
    let id=attr(e,b"id").ok_or_else(bad)?;
    let href=attr(e,b"href").ok_or_else(bad)?;
    let media=attr(e,b"media-type").ok_or_else(bad)?;
    if id.len()>128||id.is_empty()||items.len()>=512{return Err(bad())}
    if media=="application/xhtml+xml"{
     let path=safe_path(root,&href)?;
     if !path.ends_with(".xhtml")&&!path.ends_with(".html"){return Err(bad())}
     if items.insert(id,path).is_some(){return Err(bad())}
    }
   },
   Event::Start(ref e)|Event::Empty(ref e)
     if body&&e.local_name().as_ref()==b"itemref"=>{
    if order.len()>=128{return Err(ReaderFailure::TooLarge)}
    order.push(attr(e,b"idref").ok_or_else(bad)?);
   },
   Event::DocType(_)=>return Err(bad()),
   Event::Eof=>break,
   _=>{}
  }
 }
 if order.is_empty(){return Err(bad())}
 order.iter().map(|id|items.get(id).cloned().ok_or_else(bad)).collect()
}
fn chapter_text(xml:&[u8],out:&mut String)->Result<(),ReaderFailure>{
 let mut r=Reader::from_reader(xml);
 let mut in_body=false;let mut ignored=0usize;let mut events=0usize;
 loop{
  events+=1;if events>250_000{return Err(ReaderFailure::TooLarge)}
  let event=r.read_event().map_err(|_|bad())?;
  match &event{
   Event::Start(e)=>{
    let tag=e.local_name();
    if tag.as_ref()==b"body"{in_body=true}
    else if in_body&&ignored>0{ignored+=1}
    else if in_body&&[b"script".as_slice(),b"style",b"svg",b"iframe",b"math"]
      .iter().any(|x|*x==tag.as_ref()){ignored=1}
   },
   Event::End(e)=>{
    let tag=e.local_name();
    if in_body&&ignored>0{ignored-=1}
    else if tag.as_ref()==b"body"{in_body=false}
    else if in_body&&[
      b"h1".as_slice(),b"h2",b"h3",b"h4",b"h5",b"h6",
      b"p",b"li",b"div",b"blockquote",b"pre"
    ].iter().any(|x|*x==tag.as_ref()){out.push('\n')}
   },
   Event::Empty(e) if in_body&&ignored==0&&e.local_name().as_ref()==b"br"=>
    out.push('\n'),
   Event::DocType(_)=>return Err(bad()),
   Event::Eof=>break,
   _=>{}
  }
  if in_body&&ignored==0{
   if let Some(text)=xml_value(&event)?{out.push_str(&text)}
  }
  if out.len()>MAX_READER_OUTPUT_CHARS*4{return Err(ReaderFailure::TooLarge)}
 }
 Ok(())
}
pub fn read_epub(bytes:&[u8],max_chars:usize,chapter:Option<usize>)
 ->Result<ReaderText,ReaderFailure>{
 if max_chars==0||max_chars>MAX_READER_OUTPUT_CHARS{return Err(ReaderFailure::TooLarge)}
 let pkg=OpenXmlPackage::new_epub(bytes)?;
 if pkg.part("mimetype")?!=b"application/epub+zip"||
    pkg.has("META-INF/encryption.xml"){return Err(bad())}
 let root=root_path(pkg.part("META-INF/container.xml")?)?;
 let parts=spine(pkg.part(&root)?,&root)?;
 if chapter.is_some_and(|n|n==0||n>parts.len()){
  return Err(ReaderFailure::SelectorNotFound)
 }
 let mut out=String::new();
 for (index,path) in parts.iter().enumerate(){
  if chapter.is_some_and(|n|n!=index+1){continue}
  out.push_str(&format!("\n[Chapter {}: {}]\n",index+1,path));
  chapter_text(pkg.part(path)?,&mut out)?;
 }
 if out.trim().is_empty(){return Err(ReaderFailure::Empty)}
 let truncated=out.chars().count()>max_chars;
 let text=if truncated{out.chars().take(max_chars).collect()}else{out};
 Ok(ReaderText{format:ReaderFormat::Epub,text,truncated,extracted_from_image:false})
}
#[cfg(test)]
mod tests{
 use super::*;
 fn zip(parts:&[(&str,&str)])->Vec<u8>{
  fn w16(out:&mut Vec<u8>,n:u16){out.extend(n.to_le_bytes())}
  fn w32(out:&mut Vec<u8>,n:u32){out.extend(n.to_le_bytes())}
  let mut data=Vec::new();let mut cd=Vec::new();
  for (name,contents) in parts{
   let offset=data.len() as u32;
   let bytes=contents.as_bytes();let crc=crc32fast::hash(bytes);
   w32(&mut data,0x04034b50);w16(&mut data,20);
   for _ in 0..4{w16(&mut data,0)}
   w32(&mut data,crc);w32(&mut data,bytes.len() as u32);
   w32(&mut data,bytes.len() as u32);
   w16(&mut data,name.len() as u16);w16(&mut data,0);
   data.extend(name.as_bytes());data.extend(bytes);
   w32(&mut cd,0x02014b50);w16(&mut cd,20);w16(&mut cd,20);
   for _ in 0..4{w16(&mut cd,0)}
   w32(&mut cd,crc);w32(&mut cd,bytes.len() as u32);
   w32(&mut cd,bytes.len() as u32);w16(&mut cd,name.len() as u16);
   for _ in 0..4{w16(&mut cd,0)}
   w32(&mut cd,0);w32(&mut cd,offset);cd.extend(name.as_bytes());
  }
  let start=data.len() as u32;data.extend(&cd);
  w32(&mut data,0x06054b50);w16(&mut data,0);w16(&mut data,0);
  w16(&mut data,parts.len() as u16);w16(&mut data,parts.len() as u16);
  w32(&mut data,cd.len() as u32);w32(&mut data,start);w16(&mut data,0);
  data
 }
 #[test]
 fn epub_reads_only_ordered_chapters_and_skips_scripts(){
  let file=zip(&[
   ("mimetype","application/epub+zip"),
   ("META-INF/container.xml",r#"<container><rootfile full-path="OPS/book.opf"/></container>"#),
   ("OPS/book.opf",r#"<package><manifest>
     <item id="b" href="b.xhtml" media-type="application/xhtml+xml"/>
     <item id="a" href="a.xhtml" media-type="application/xhtml+xml"/>
     </manifest><spine><itemref idref="a"/><itemref idref="b"/></spine></package>"#),
   ("OPS/a.xhtml",r#"<html><head><title>Not body</title></head><body>
      <h1>First chapter</h1><script>Malicious</script><p>Safe &amp; sound</p>
    </body></html>"#),
   ("OPS/b.xhtml",r#"<html><body><p>Second chapter</p></body></html>"#),
  ]);
  let all=read_epub(&file,2000,None).unwrap();
  assert!(all.text.find("First chapter").unwrap()<all.text.find("Second chapter").unwrap());
  assert!(!all.text.contains("Malicious"));
  let chapter=read_epub(&file,2000,Some(2)).unwrap();
  assert!(chapter.text.contains("Second chapter"));
  assert!(!chapter.text.contains("First chapter"));
  assert_eq!(read_epub(&file,2000,Some(3)),Err(ReaderFailure::SelectorNotFound));
 }
 #[test]
 fn epub_path_scope_refuses_traversal_or_network_locations(){
  assert_eq!(safe_path("OPS/book.opf","text/ch1.xhtml").unwrap(),"OPS/text/ch1.xhtml");
  assert!(safe_path("OPS/book.opf","../../outside.xhtml").is_err());
  assert!(safe_path("OPS/book.opf","https://bad.example/x").is_err());
 }
}
