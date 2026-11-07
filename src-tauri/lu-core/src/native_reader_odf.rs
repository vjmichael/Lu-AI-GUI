//! First-stage OpenDocument ODS and ODP text extraction.
//! Uses Reader's bounded ZIP admission. Never follows links, macros, media,
//! embedded objects or external resources. Does not evaluate spreadsheet math.
use crate::{
    native_reader::{ReaderFailure,ReaderFormat,ReaderText,MAX_READER_OUTPUT_CHARS},
    native_reader_openxml::OpenXmlPackage,
};
use quick_xml::{events::{BytesStart,Event},Reader};

const MAX_EVENTS:usize=300_000;
fn bad()->ReaderFailure{ReaderFailure::InvalidDocument}
fn attr(e:&BytesStart<'_>,key:&[u8])->Option<String>{
    e.attributes().with_checks(true).filter_map(Result::ok)
        .find(|a|a.key.as_ref()==key)
        .and_then(|a|{
            let value=std::str::from_utf8(a.value.as_ref()).ok()?;
            quick_xml::escape::unescape(value).ok().map(|s|s.into_owned())
        })
}
fn text_event(e:&Event<'_>)->Result<Option<String>,ReaderFailure>{
    match e{
        Event::Text(value)=>Ok(Some(value.decode().map_err(|_|bad())?.into_owned())),
        Event::CData(value)=>Ok(Some(value.decode().map_err(|_|bad())?.into_owned())),
        Event::GeneralRef(value)=>{
            if let Some(ch)=value.resolve_char_ref().map_err(|_|bad())?{
                Ok(Some(ch.to_string()))
            }else{
                let name=value.decode().map_err(|_|bad())?;
                Ok(Some(quick_xml::escape::resolve_xml_entity(&name)
                    .ok_or_else(bad)?.into()))
            }
        },
        _=>Ok(None),
    }
}
fn stored_cell_value(e:&BytesStart<'_>)->String{
    for attribute in [
        b"office:value".as_slice(),b"office:string-value".as_slice(),
        b"office:date-value".as_slice(),b"office:time-value".as_slice(),
        b"office:boolean-value".as_slice()
    ]{
        if let Some(value)=attr(e,attribute){return value}
    }
    String::new()
}
fn repeat_count(e:&BytesStart<'_>,key:&[u8])->Result<usize,ReaderFailure>{
    match attr(e,key){
        None=>Ok(1),
        Some(s)=>s.parse::<usize>().ok().filter(|n|*n>0)
            .ok_or_else(bad),
    }
}
fn colname(mut col:usize)->Result<String,ReaderFailure>{
    if col==0||col>16_384{return Err(ReaderFailure::TooLarge)}
    let mut letters=Vec::new();
    while col>0{
        col-=1;
        letters.push((b'A'+(col%26) as u8) as char);
        col/=26;
    }
    Ok(letters.into_iter().rev().collect())
}
fn bounded(text:String,format:ReaderFormat,max_chars:usize)
    ->Result<ReaderText,ReaderFailure>{
    if max_chars==0||max_chars>MAX_READER_OUTPUT_CHARS{
        return Err(ReaderFailure::TooLarge)
    }
    if text.trim().is_empty(){return Err(ReaderFailure::Empty)}
    let truncated=text.chars().count()>max_chars;
    let text=if truncated{text.chars().take(max_chars).collect()}else{text};
    Ok(ReaderText{format,text,truncated,extracted_from_image:false})
}
fn extract_ods(xml:&[u8],max_chars:usize,selected:Option<&str>)
    ->Result<ReaderText,ReaderFailure>{
    let mut reader=Reader::from_reader(xml);
    let mut table=String::new();let mut inside=false;let mut found=false;
    let mut row=0usize;let mut col=0usize;
    let mut row_repeat=1usize;let mut cell_repeat=1usize;let mut row_had_content=false;
    let mut cell_active=false;let mut value=String::new();
    let mut cached=String::new();let mut formula=String::new();
    let mut text_depth=0usize;let mut out=String::new();let mut events=0usize;
    loop{
        events+=1;if events>MAX_EVENTS{return Err(ReaderFailure::TooLarge)}
        let event=reader.read_event().map_err(|_|bad())?;
        match &event{
            Event::Start(e) if e.name().as_ref()==b"table:table"=>{
                table=attr(e,b"table:name").unwrap_or_else(||"Unnamed".into());
                inside=selected.is_none_or(|s|s==table);
                found|=inside;
                row=0;col=0;
                if inside{out.push_str(&format!("\n[Sheet: {}]\n",table))}
            },
            Event::End(e) if e.name().as_ref()==b"table:table"=>inside=false,
            Event::Start(e) if e.name().as_ref()==b"table:table-row"&&inside=>{
                row+=1;col=0;row_had_content=false;
                row_repeat=repeat_count(e,b"table:number-rows-repeated")?;
                if row_repeat==0||row.checked_add(row_repeat-1)
                    .is_none_or(|n|n>1_048_576){return Err(ReaderFailure::TooLarge)}
            },
            Event::End(e) if e.name().as_ref()==b"table:table-row"&&inside=>{
                if row_had_content && row_repeat>1 {
                    return Err(ReaderFailure::UnsupportedAdapter(ReaderFormat::Ods));
                }
                row+=row_repeat-1;
                row_repeat=1;
            },
            Event::Empty(e) if e.name().as_ref()==b"table:table-row"&&inside=>{
                let repeated=repeat_count(e,b"table:number-rows-repeated")?;
                if repeated==0||row.checked_add(repeated)
                    .is_none_or(|n|n>1_048_576){return Err(ReaderFailure::TooLarge)}
                row+=repeated;col=0;
            },
            Event::Start(e) if e.name().as_ref()==b"table:table-cell"&&inside=>{
                col+=1;cell_active=true;value.clear();
                cached=stored_cell_value(e);
                formula=attr(e,b"table:formula").unwrap_or_default();
                cell_repeat=repeat_count(e,b"table:number-columns-repeated")?;
                if cell_repeat==0||col.checked_add(cell_repeat-1)
                    .is_none_or(|n|n>16_384){return Err(ReaderFailure::TooLarge)}
            },
            Event::Empty(e) if (e.name().as_ref()==b"table:table-cell"||
                e.name().as_ref()==b"table:covered-table-cell")&&inside=>{
                let repeated=repeat_count(e,b"table:number-columns-repeated")?;
                if repeated==0||col.checked_add(repeated)
                    .is_none_or(|n|n>16_384){return Err(ReaderFailure::TooLarge)}
                if e.name().as_ref()==b"table:table-cell"{
                    let value=stored_cell_value(e);
                    let formula=attr(e,b"table:formula").unwrap_or_default();
                    if !value.is_empty()||!formula.is_empty(){
                        row_had_content=true;
                        if repeated>128{return Err(ReaderFailure::TooLarge)}
                        for n in 0..repeated{
                            out.push_str(&format!("{}{}={}",colname(col+n+1)?,row,value));
                            if !formula.is_empty(){
                                out.push_str(&format!(
                                    " [formula: {}; cached value may be stale]",formula));
                            }
                            out.push('\n');
                        }
                    }
                }
                col+=repeated;
            },
            Event::Start(e) if e.name().as_ref()==b"text:p"&&cell_active=>{
                text_depth+=1;
            },
            Event::End(e) if e.name().as_ref()==b"text:p"&&cell_active=>{
                text_depth=text_depth.saturating_sub(1);
                if text_depth==0 && !value.ends_with('\n'){value.push('\n')}
            },
            Event::End(e) if e.name().as_ref()==b"table:table-cell"&&cell_active=>{
                let display=value.trim_end_matches('\n');
                let display=if display.is_empty(){&cached}else{display};
                if !display.is_empty()||!formula.is_empty(){
                    row_had_content=true;
                    if cell_repeat>128{return Err(ReaderFailure::TooLarge)}
                    for i in 0..cell_repeat {
                        out.push_str(&format!("{}{}={}",colname(col+i)?,row,
                            display.replace('\n'," ")));
                        if !formula.is_empty(){
                            out.push_str(&format!(" [formula: {}; cached value may be stale]",formula))
                        }
                        out.push('\n');
                    }
                }
                col+=cell_repeat-1;
                cell_repeat=1;
                cell_active=false;text_depth=0;
            },
            Event::DocType(_)=>return Err(bad()),
            Event::Eof=>break,
            _=>{}
        }
        if inside&&cell_active&&text_depth>0{
            if let Some(value_text)=text_event(&event)?{
                value.push_str(&value_text);
                if value.len()>32_000{return Err(ReaderFailure::TooLarge)}
            }
        }
        if out.len()>MAX_READER_OUTPUT_CHARS*4{return Err(ReaderFailure::TooLarge)}
    }
    if !found{return Err(ReaderFailure::SelectorNotFound)}
    bounded(out,ReaderFormat::Ods,max_chars)
}
fn extract_odp(xml:&[u8],max_chars:usize,selected:Option<usize>)
    ->Result<ReaderText,ReaderFailure>{
    let mut reader=Reader::from_reader(xml);
    let mut slide=0usize;let mut inside=false;let mut found=false;
    let mut paragraph_depth=0usize;let mut in_notes=0usize;
    let mut out=String::new();let mut events=0usize;
    loop{
        events+=1;if events>MAX_EVENTS{return Err(ReaderFailure::TooLarge)}
        let event=reader.read_event().map_err(|_|bad())?;
        match &event{
            Event::Start(e) if e.name().as_ref()==b"draw:page"=>{
                slide+=1;if slide>512{return Err(ReaderFailure::TooLarge)}
                inside=selected.is_none_or(|n|n==slide);
                found|=inside;in_notes=0;
                if inside{
                    let title=attr(e,b"draw:name").unwrap_or_default();
                    out.push_str(&format!("\n[Slide {}: {}]\n",slide,title));
                }
            },
            Event::End(e) if e.name().as_ref()==b"draw:page"=>inside=false,
            Event::Start(e) if e.name().as_ref()==b"presentation:notes"&&inside=>{
                in_notes+=1;
            },
            Event::End(e) if e.name().as_ref()==b"presentation:notes"=>in_notes=0,
            Event::Start(e) if e.name().as_ref()==b"text:p"&&inside&&in_notes==0=>{
                paragraph_depth+=1;
            },
            Event::End(e) if e.name().as_ref()==b"text:p"&&inside&&in_notes==0=>{
                paragraph_depth=paragraph_depth.saturating_sub(1);
                out.push('\n');
            },
            Event::DocType(_)=>return Err(bad()),
            Event::Eof=>break,
            _=>{},
        }
        if inside&&in_notes==0&&paragraph_depth>0{
            if let Some(value)=text_event(&event)?{out.push_str(&value)}
        }
        if out.len()>MAX_READER_OUTPUT_CHARS*4{return Err(ReaderFailure::TooLarge)}
    }
    if !found{return Err(ReaderFailure::SelectorNotFound)}
    bounded(out,ReaderFormat::Odp,max_chars)
}
fn package(bytes:&[u8],mime:&[u8])->Result<OpenXmlPackage,ReaderFailure>{
    let pkg=OpenXmlPackage::new_odf(bytes)?;
    if pkg.part("mimetype")?!=mime{return Err(bad())}
    Ok(pkg)
}
pub fn read_ods(bytes:&[u8],max_chars:usize,selected:Option<&str>)
    ->Result<ReaderText,ReaderFailure>{
    let pkg=package(bytes,b"application/vnd.oasis.opendocument.spreadsheet")?;
    extract_ods(pkg.part("content.xml")?,max_chars,selected)
}
pub fn read_odp(bytes:&[u8],max_chars:usize,selected:Option<usize>)
    ->Result<ReaderText,ReaderFailure>{
    let pkg=package(bytes,b"application/vnd.oasis.opendocument.presentation")?;
    extract_odp(pkg.part("content.xml")?,max_chars,selected)
}
#[cfg(test)]
mod tests{
    use super::*;
    fn zip(parts:&[(&str,&str)])->Vec<u8>{
        fn w16(b:&mut Vec<u8>,x:u16){b.extend(x.to_le_bytes())}
        fn w32(b:&mut Vec<u8>,x:u32){b.extend(x.to_le_bytes())}
        let mut data=Vec::new();let mut cd=Vec::new();
        for (name,contents) in parts{
            let offset=data.len() as u32;
            let payload=contents.as_bytes();let crc=crc32fast::hash(payload);
            w32(&mut data,0x04034b50);w16(&mut data,20);
            for _ in 0..4{w16(&mut data,0)}
            w32(&mut data,crc);
            w32(&mut data,payload.len() as u32);
            w32(&mut data,payload.len() as u32);
            w16(&mut data,name.len() as u16);w16(&mut data,0);
            data.extend(name.as_bytes());data.extend(payload);
            w32(&mut cd,0x02014b50);w16(&mut cd,20);w16(&mut cd,20);
            for _ in 0..4{w16(&mut cd,0)}
            w32(&mut cd,crc);w32(&mut cd,payload.len() as u32);
            w32(&mut cd,payload.len() as u32);
            w16(&mut cd,name.len() as u16);
            for _ in 0..4{w16(&mut cd,0)}
            w32(&mut cd,0);w32(&mut cd,offset);cd.extend(name.as_bytes());
        }
        let offset=data.len() as u32;
        data.extend(&cd);
        w32(&mut data,0x06054b50);
        w16(&mut data,0);w16(&mut data,0);
        w16(&mut data,parts.len() as u16);w16(&mut data,parts.len() as u16);
        w32(&mut data,cd.len() as u32);w32(&mut data,offset);w16(&mut data,0);
        data
    }
    #[test]
    fn reads_odf_spreadsheet_cells_formula_and_sheet_selector(){
        let file=zip(&[
            ("mimetype","application/vnd.oasis.opendocument.spreadsheet"),
            ("content.xml",r#"<office:document-content><table:table table:name="Annual"><table:table-row><table:table-cell office:value="4" table:formula="of:=SUM([.A2:.A4])"><text:p>Four &amp; more</text:p></table:table-cell></table:table-row></table:table></office:document-content>"#),
        ]);
        let extracted=read_ods(&file,500,Some("Annual")).unwrap();
        assert!(extracted.text.contains("A1=Four & more"));
        assert!(extracted.text.contains("of:=SUM"));
        assert_eq!(read_ods(&file,500,Some("Missing")),
            Err(ReaderFailure::SelectorNotFound));
    }
    #[test]
    fn ods_repeated_blank_cells_and_rows_keep_real_coordinates(){
        let file=zip(&[
            ("mimetype","application/vnd.oasis.opendocument.spreadsheet"),
            ("content.xml",r#"<office:document-content><table:table table:name="Coordinates">
            <table:table-row table:number-rows-repeated="3"/>
            <table:table-row><table:table-cell table:number-columns-repeated="5"/>
            <table:table-cell office:value="14"><text:p>Fourteen</text:p></table:table-cell>
            </table:table-row></table:table></office:document-content>"#),
        ]);
        let extracted=read_ods(&file,500,Some("Coordinates")).unwrap();
        assert!(extracted.text.contains("F4=Fourteen"));
    }
    #[test]
    fn ods_self_closing_numeric_cell_keeps_its_value(){
        let file=zip(&[
            ("mimetype","application/vnd.oasis.opendocument.spreadsheet"),
            ("content.xml",r#"<office:document-content><table:table table:name="Data"><table:table-row><table:table-cell office:value-type="float" office:value="2.5"/><table:table-cell office:date-value="2026-11-04"/></table:table-row></table:table></office:document-content>"#),
        ]);
        let result=read_ods(&file,500,None).unwrap();
        assert!(result.text.contains("A1=2.5"));
        assert!(result.text.contains("B1=2026-11-04"));
    }
    #[test]
    fn reads_odp_slide_text_and_slide_selector(){
        let file=zip(&[
            ("mimetype","application/vnd.oasis.opendocument.presentation"),
            ("content.xml",r#"<office:document-content><draw:page draw:name="Plan"><text:p>Slide &amp; one</text:p></draw:page><draw:page draw:name="Next"><text:p>Second slide</text:p></draw:page></office:document-content>"#),
        ]);
        let extracted=read_odp(&file,500,Some(2)).unwrap();
        assert!(extracted.text.contains("[Slide 2: Next]"));
        assert!(!extracted.text.contains("Slide & one"));
    }
    #[test]
    fn incompatible_mimetype_and_malformed_package_refused(){
        assert!(read_ods(b"PK\x03\x04bad",100,None).is_err());
        let file=zip(&[
            ("mimetype","application/vnd.oasis.opendocument.presentation"),
            ("content.xml",r#"<draw:page/>"#),
        ]);
        assert!(read_ods(&file,100,None).is_err());
    }
}
