//! Spreadsheet and slide extraction from OOXML package parts.
//! The initial output is deliberately bounded and carries sheet/cell or
//! slide anchors. No spreadsheet evaluation, macros, media or external links.
use crate::{native_reader::{ReaderFailure,ReaderFormat,ReaderText,MAX_READER_OUTPUT_CHARS},
    native_reader_openxml::OpenXmlPackage};
use quick_xml::{events::{BytesStart,Event},Reader};
use std::collections::BTreeMap;

fn bad()->ReaderFailure{ReaderFailure::InvalidDocument}
fn attr(tag:&BytesStart<'_>,name:&[u8])->Option<String>{
    tag.attributes().with_checks(true).filter_map(Result::ok)
        .find(|a|a.key.as_ref()==name)
        .and_then(|a|std::str::from_utf8(a.value.as_ref()).ok())
        .and_then(|s|quick_xml::escape::unescape(s).ok())
        .map(|s|s.into_owned())
}
fn xml_value(e:&Event<'_>)->Result<Option<String>,ReaderFailure>{
    match e{
        Event::Text(t)=>Ok(Some(t.decode().map_err(|_|bad())?.into_owned())),
        Event::CData(t)=>Ok(Some(t.decode().map_err(|_|bad())?.into_owned())),
        Event::GeneralRef(r)=>{
            if let Some(ch)=r.resolve_char_ref().map_err(|_|bad())?{
                return Ok(Some(ch.to_string()));
            }
            let name=r.decode().map_err(|_|bad())?;
            Ok(Some(quick_xml::escape::resolve_xml_entity(&name)
                .ok_or_else(bad)?.to_owned()))
        },
        _=>Ok(None)
    }
}
fn relationships(xml:&[u8])->Result<BTreeMap<String,String>,ReaderFailure>{
    let mut reader=Reader::from_reader(xml);
    let mut rels=BTreeMap::new();
    let mut count=0;
    loop{
        count+=1;if count>5000{return Err(ReaderFailure::TooLarge)}
        let event=reader.read_event().map_err(|_|bad())?;
        match event{
            Event::Empty(ref e)|Event::Start(ref e)
                if e.local_name().as_ref()==b"Relationship"=>{
                if attr(e,b"TargetMode").as_deref()==Some("External"){
                    return Err(bad())
                }
                let id=attr(e,b"Id").ok_or_else(bad)?;
                let target=attr(e,b"Target").ok_or_else(bad)?;
                if id.len()>100||target.len()>256||
                   target.starts_with('/')||target.contains('\\')||
                   target.contains('?')||target.contains('#'){
                    return Err(bad())
                }
                if rels.insert(id,target).is_some(){return Err(bad())}
            },
            Event::DocType(_)=>return Err(bad()),
            Event::Eof=>break,
            _=>{}
        }
    }
    Ok(rels)
}
fn resolve(base:&str,target:&str)->Result<String,ReaderFailure>{
    // Canonicalize OOXML relationship targets without escaping archive root.
    let mut parts=base.split('/').map(str::to_owned).collect::<Vec<_>>();
    parts.pop();
    for segment in target.trim_start_matches('/').split('/'){
        match segment{
            ""|"."=>return Err(bad()),
            ".."=>{if parts.pop().is_none(){return Err(bad())}},
            s if s.contains(':')||s.contains('\0')=>return Err(bad()),
            s=>parts.push(s.to_owned())
        }
    }
    let path=parts.join("/");
    if path.len()>512{return Err(bad())}
    Ok(path)
}
fn bounded(text:String,format:ReaderFormat,max_chars:usize)
    ->Result<ReaderText,ReaderFailure>{
    if max_chars==0||max_chars>MAX_READER_OUTPUT_CHARS{return Err(ReaderFailure::TooLarge)}
    if text.trim().is_empty(){return Err(ReaderFailure::Empty)}
    let truncated=text.chars().count()>max_chars;
    Ok(ReaderText{format,text:if truncated{text.chars().take(max_chars).collect()}
        else{text},truncated,extracted_from_image:false})
}
fn shared_strings(xml:&[u8])->Result<Vec<String>,ReaderFailure>{
    let mut strings=Vec::new();
    let mut current=String::new();
    let mut inside=false;let mut in_t=false;let mut count=0usize;
    let mut reader=Reader::from_reader(xml);
    loop{
        count+=1;if count>200_000{return Err(ReaderFailure::TooLarge)}
        let event=reader.read_event().map_err(|_|bad())?;
        match &event{
            Event::Start(e) if e.local_name().as_ref()==b"si"=>{inside=true;current.clear()},
            Event::Start(e) if e.local_name().as_ref()==b"t"&&inside=>in_t=true,
            Event::End(e) if e.local_name().as_ref()==b"t"=>in_t=false,
            Event::End(e) if e.local_name().as_ref()==b"si"=>{
                strings.push(std::mem::take(&mut current));inside=false;
                if strings.len()>25_000{return Err(ReaderFailure::TooLarge)}
            },
            Event::Eof=>break,
            _=>{}
        }
        if in_t{
            if let Some(value)=xml_value(&event)?{
                current.push_str(&value);
                if current.len()>32_000{return Err(ReaderFailure::TooLarge)}
            }
        }
    }
    Ok(strings)
}
fn worksheet(xml:&[u8],strings:&[String],name:&str,out:&mut String)
    ->Result<(),ReaderFailure>{
    let mut reader=Reader::from_reader(xml);
    let mut cell=String::new();let mut cell_type=String::new();
    let mut value=String::new();let mut formula=String::new();
    let mut active=false;let mut field=0u8;let mut events=0usize;
    out.push_str(&format!("\n[Sheet: {}]\n",name));
    loop{
        events+=1;if events>250_000{return Err(ReaderFailure::TooLarge)}
        let event=reader.read_event().map_err(|_|bad())?;
        match &event{
            Event::Start(e) if e.local_name().as_ref()==b"c"=>{
                cell=attr(e,b"r").ok_or_else(bad)?;
                if cell.len()>24||!cell.bytes().all(|c|c.is_ascii_alphanumeric()){
                    return Err(bad())
                }
                cell_type=attr(e,b"t").unwrap_or_default();
                value.clear();formula.clear();active=true;
            },
            Event::Start(e) if active && e.local_name().as_ref()==b"f"=>field=2,
            Event::Start(e) if active &&
                (e.local_name().as_ref()==b"v"||e.local_name().as_ref()==b"t")=>field=1,
            Event::End(e) if e.local_name().as_ref()==b"f"||
                e.local_name().as_ref()==b"v"||e.local_name().as_ref()==b"t"=>field=0,
            Event::End(e) if e.local_name().as_ref()==b"c"&&active=>{
                if cell_type=="s"{
                    let index=value.parse::<usize>().map_err(|_|bad())?;
                    value=strings.get(index).ok_or_else(bad)?.clone();
                }
                if !value.is_empty()||!formula.is_empty(){
                    out.push_str(&cell);
                    out.push('=');
                    out.push_str(&value.replace('\n'," "));
                    if !formula.is_empty(){
                        out.push_str(" [formula: =");
                        out.push_str(&formula);
                        out.push_str("; cached value may be stale]");
                    }
                    out.push('\n');
                }
                active=false;field=0;
            },
            Event::Eof=>break,
            _=>{}
        }
        if active&&field!=0{
            if let Some(value_text)=xml_value(&event)?{
                if field==1{value.push_str(&value_text)}else{formula.push_str(&value_text)}
                if value.len()>32_000||formula.len()>32_000{
                    return Err(ReaderFailure::TooLarge)
                }
            }
        }
        if out.len()>MAX_READER_OUTPUT_CHARS*4{return Err(ReaderFailure::TooLarge)}
    }
    Ok(())
}
pub fn read_xlsx(bytes:&[u8],max_chars:usize)->Result<ReaderText,ReaderFailure>{
    let pkg=OpenXmlPackage::new(bytes)?;
    if !pkg.has("xl/workbook.xml"){return Err(bad())}
    let relationships=relationships(pkg.part("xl/_rels/workbook.xml.rels")?)?;
    let strings=match pkg.optional_part("xl/sharedStrings.xml"){
        Some(part)=>shared_strings(part)?,
        None=>Vec::new(),
    };
    let mut reader=Reader::from_reader(pkg.part("xl/workbook.xml")?);
    let mut sheets=Vec::new();
    let mut count=0;
    loop{
        count+=1;if count>10_000{return Err(ReaderFailure::TooLarge)}
        let event=reader.read_event().map_err(|_|bad())?;
        match event{
            Event::Empty(ref e)|Event::Start(ref e)
                if e.local_name().as_ref()==b"sheet"=>{
                let name=attr(e,b"name").ok_or_else(bad)?;
                let id=attr(e,b"r:id").ok_or_else(bad)?;
                if name.len()>128{return Err(bad())}
                let path=resolve("xl/workbook.xml",
                    relationships.get(&id).ok_or_else(bad)?)?;
                if !path.starts_with("xl/worksheets/")||
                    !path.ends_with(".xml"){return Err(bad())}
                sheets.push((name,path));
                if sheets.len()>128{return Err(ReaderFailure::TooLarge)}
            },
            Event::Eof=>break,
            Event::DocType(_)=>return Err(bad()),
            _=>{}
        }
    }
    if sheets.is_empty(){return Err(bad())}
    let mut out=String::new();
    for (name,path) in sheets{
        worksheet(pkg.part(&path)?,&strings,&name,&mut out)?;
    }
    bounded(out,ReaderFormat::Xlsx,max_chars)
}
fn slide_text(xml:&[u8],out:&mut String)->Result<(),ReaderFailure>{
    let mut reader=Reader::from_reader(xml);
    let mut in_t=false;let mut events=0usize;
    loop{
        events+=1;if events>250_000{return Err(ReaderFailure::TooLarge)}
        let event=reader.read_event().map_err(|_|bad())?;
        match &event{
            Event::Start(e) if e.local_name().as_ref()==b"t"=>in_t=true,
            Event::End(e) if e.local_name().as_ref()==b"t"=>{
                in_t=false;out.push('\n');
            },
            Event::Eof=>break,
            Event::DocType(_)=>return Err(bad()),
            _=>{}
        }
        if in_t{
            if let Some(s)=xml_value(&event)?{out.push_str(&s)}
        }
        if out.len()>MAX_READER_OUTPUT_CHARS*4{
            return Err(ReaderFailure::TooLarge)
        }
    }
    Ok(())
}
pub fn read_pptx(bytes:&[u8],max_chars:usize)->Result<ReaderText,ReaderFailure>{
    let pkg=OpenXmlPackage::new(bytes)?;
    if !pkg.has("ppt/presentation.xml"){return Err(bad())}
    let rels=relationships(pkg.part("ppt/_rels/presentation.xml.rels")?)?;
    let mut reader=Reader::from_reader(pkg.part("ppt/presentation.xml")?);
    let mut slides=Vec::new();let mut events=0usize;
    loop{
        events+=1;if events>10_000{return Err(ReaderFailure::TooLarge)}
        let event=reader.read_event().map_err(|_|bad())?;
        match event{
            Event::Empty(ref e)|Event::Start(ref e)
                if e.local_name().as_ref()==b"sldId"=>{
                let id=attr(e,b"r:id").ok_or_else(bad)?;
                let path=resolve("ppt/presentation.xml",
                    rels.get(&id).ok_or_else(bad)?)?;
                if !path.starts_with("ppt/slides/")||!path.ends_with(".xml"){
                    return Err(bad())
                }
                slides.push(path);
                if slides.len()>512{return Err(ReaderFailure::TooLarge)}
            },
            Event::Eof=>break,
            Event::DocType(_)=>return Err(bad()),
            _=>{}
        }
    }
    if slides.is_empty(){return Err(bad())}
    let mut out=String::new();
    for (i,slide) in slides.iter().enumerate(){
        out.push_str(&format!("\n[Slide {}]\n",i+1));
        slide_text(pkg.part(slide)?,&mut out)?;
    }
    bounded(out,ReaderFormat::Pptx,max_chars)
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn invalid_packages_do_not_claim_success(){
        assert!(read_xlsx(b"PK\x03\x04fake",200).is_err());
        assert!(read_pptx(b"PK\x03\x04fake",200).is_err());
    }
    #[test]
    fn relationship_target_cannot_escape_package(){
        assert!(resolve("ppt/presentation.xml","../../../../x.xml").is_err());
        assert_eq!(resolve("xl/workbook.xml","worksheets/sheet1.xml").unwrap(),
            "xl/worksheets/sheet1.xml");
    }
}
