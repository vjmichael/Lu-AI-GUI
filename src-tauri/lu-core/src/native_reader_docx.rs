//! Bounded, read-only DOCX parser. The entire ZIP is never extracted to disk.
//! No relationships, external URLs, embedded packages, images, macros, or
//! document scripts are followed or executed. Only word/document.xml is read.
use crate::native_reader::{ReaderFailure,ReaderFormat,ReaderText,MAX_READER_OUTPUT_CHARS};
use flate2::read::DeflateDecoder;
use quick_xml::{events::Event,Reader};
use std::io::Read;

const MAX_ENTRIES:usize=256;
const MAX_XML:usize=8*1024*1024;
const MAX_ZIP_INPUT:usize=32*1024*1024;
const MAX_XML_EVENTS:usize=1_000_000;
const MAX_XML_DEPTH:usize=128;
fn bad()->ReaderFailure{ReaderFailure::InvalidDocument}
fn le16(buf:&[u8],i:usize)->Result<usize,ReaderFailure>{
    Ok(u16::from_le_bytes(buf.get(i..i+2).ok_or_else(bad)?
      .try_into().map_err(|_|bad())?) as usize)
}
fn le32(buf:&[u8],i:usize)->Result<usize,ReaderFailure>{
    Ok(u32::from_le_bytes(buf.get(i..i+4).ok_or_else(bad)?
      .try_into().map_err(|_|bad())?) as usize)
}
fn in_bounds(start:usize,len:usize,total:usize)->Result<(),ReaderFailure>{
    if start.checked_add(len).is_none_or(|end|end>total){Err(bad())}else{Ok(())}
}
/// Central-directory-backed file admission. Never trust local-header sizes.
fn document_xml(data:&[u8])->Result<Vec<u8>,ReaderFailure>{
    if data.len()<22||data.len()>MAX_ZIP_INPUT||!data.starts_with(b"PK\x03\x04"){
        return Err(bad());
    }
    let begin=data.len().saturating_sub(22+65_535);
    let end=data.len()-22;
    let eocd=(begin..=end).rev().find(|&i|
        data.get(i..i+4)==Some(b"PK\x05\x06")
        &&le16(data,i+20).ok().is_some_and(|comment|
            i.checked_add(22+comment)==Some(data.len())))
        .ok_or_else(bad)?;
    if le16(data,eocd+4)?!=0||le16(data,eocd+6)?!=0{
        return Err(bad()); // multivolume ZIP is unsupported
    }
    let count=le16(data,eocd+10)?;
    if count==0||count>MAX_ENTRIES||count!=le16(data,eocd+8)?{
        return Err(bad());
    }
    let cd_size=le32(data,eocd+12)?;
    let cd_offset=le32(data,eocd+16)?;
    if cd_offset.checked_add(cd_size)!=Some(eocd){
        return Err(bad()); // ZIP64 and ambiguous prefixed archives denied
    }
    let mut cursor=cd_offset;
    let mut xml=None;
    let mut seen=std::collections::HashSet::<Vec<u8>>::new();
    for _ in 0..count{
        in_bounds(cursor,46,data.len())?;
        if data.get(cursor..cursor+4)!=Some(b"PK\x01\x02"){return Err(bad());}
        let flags=le16(data,cursor+8)?;
        let method=le16(data,cursor+10)?;
        let crc=le32(data,cursor+16)? as u32;
        let compressed=le32(data,cursor+20)?;
        let unpacked=le32(data,cursor+24)?;
        let name_len=le16(data,cursor+28)?;
        let extra=le16(data,cursor+30)?;
        let comment=le16(data,cursor+32)?;
        let local=le32(data,cursor+42)?;
        if flags & 1!=0 || !matches!(method,0|8)||
            compressed==u32::MAX as usize||unpacked==u32::MAX as usize||
            local==u32::MAX as usize{
            return Err(bad());
        }
        let entry_len=46usize.checked_add(name_len).and_then(|x|x.checked_add(extra))
            .and_then(|x|x.checked_add(comment)).ok_or_else(bad)?;
        in_bounds(cursor,entry_len,eocd)?;
        let name=data[cursor+46..cursor+46+name_len].to_vec();
        if !seen.insert(name.clone())||name.contains(&0)||
           name.starts_with(b"/")||name.contains(&b'\\')||
           name.split(|&b|b==b'/').any(|part|part==b".."||part==b"."){
            return Err(bad());
        }
        if name.ends_with(b"vbaProject.bin")||name==b"word/vbaProject.bin"{
            return Err(bad());
        }
        if name==b"word/document.xml"{
            if compressed>MAX_XML||unpacked>MAX_XML||unpacked==0
                ||(compressed==0&&method==8)
                ||unpacked>compressed.saturating_mul(250).saturating_add(4096){
                return Err(ReaderFailure::TooLarge);
            }
            in_bounds(local,30,data.len())?;
            if data.get(local..local+4)!=Some(b"PK\x03\x04")
                ||le16(data,local+8)?!=method{
                return Err(bad());
            }
            let lname=le16(data,local+26)?;
            let lextra=le16(data,local+28)?;
            let start=local.checked_add(30).and_then(|n|n.checked_add(lname))
                .and_then(|n|n.checked_add(lextra)).ok_or_else(bad)?;
            in_bounds(local+30,lname,data.len())?;
            if data[local+30..local+30+lname]!=name{return Err(bad());}
            in_bounds(start,compressed,cd_offset)?;
            let compressed_data=&data[start..start+compressed];
            let mut result=Vec::new();
            match method{
                0=>{
                    if compressed!=unpacked{return Err(bad());}
                    result.extend_from_slice(compressed_data);
                },
                8=>{
                    let mut inflater=DeflateDecoder::new(compressed_data);
                    inflater.take(MAX_XML as u64+1).read_to_end(&mut result)
                        .map_err(|_|bad())?;
                },
                _=>return Err(bad()),
            }
            if result.len()!=unpacked||result.len()>MAX_XML
                ||crc32fast::hash(&result)!=crc{
                return Err(bad());
            }
            xml=Some(result);
        }
        cursor+=entry_len;
    }
    if cursor!=eocd{return Err(bad());}
    xml.ok_or_else(bad)
}
fn is(name:&[u8],s:&[u8])->bool{name==s}
/// Extract authored text runs and table separations from Office Open XML.
/// Deleted tracked changes are omitted; inserted runs remain visible.
fn parse_document(xml:&[u8],max_chars:usize)->Result<ReaderText,ReaderFailure>{
    if max_chars==0||max_chars>MAX_READER_OUTPUT_CHARS{
        return Err(ReaderFailure::TooLarge);
    }
    let mut reader=Reader::from_reader(xml);
    reader.config_mut().trim_text(false);
    let mut out=String::new();
    let mut depth=0usize;
    let mut del_depth=0usize;
    let mut in_text=false;
    let mut events=0usize;
    loop{
        events+=1;
        if events>MAX_XML_EVENTS{return Err(ReaderFailure::TooLarge);}
        match reader.read_event().map_err(|_|bad())?{
            Event::Start(ref e)=>{
                depth+=1;
                if depth>MAX_XML_DEPTH{return Err(ReaderFailure::TooLarge);}
                let name=e.name();
                let tag=name.as_ref();
                if is(tag,b"w:del"){del_depth+=1;}
                if is(tag,b"w:t")&&! (del_depth>0){in_text=true;}
            },
            Event::End(ref e)=>{
                if depth==0{return Err(bad());}
                let name=e.name();
                match name.as_ref(){
                    b"w:t"=>in_text=false,
                    b"w:del"=>del_depth=del_depth.saturating_sub(1),
                    b"w:p" if del_depth==0 =>out.push('\n'),
                    b"w:tc" if del_depth==0 =>out.push('\t'),
                    _=>{},
                }
                depth-=1;
            },
            Event::Empty(ref e)=>{
                if del_depth==0{
                    let name=e.name();
                    match name.as_ref(){
                        b"w:tab"=>out.push('\t'),
                        b"w:br"|b"w:cr"=>out.push('\n'),
                        _=>{},
                    }
                }
            },
            Event::Text(e)=>{
                if in_text&&del_depth==0{
                    out.push_str(&e.unescape().map_err(|_|bad())?);
                }
            },
            Event::CData(e)=>{
                if in_text&&del_depth==0{
                    out.push_str(std::str::from_utf8(e.as_ref()).map_err(|_|bad())?);
                }
            },
            Event::DocType(_)=>return Err(bad()),
            Event::Eof=>break,
            _=>{},
        }
        if out.len()>MAX_READER_OUTPUT_CHARS*4{return Err(ReaderFailure::TooLarge);}
    }
    if depth!=0||del_depth!=0{return Err(bad());}
    if out.trim().is_empty(){return Err(ReaderFailure::Empty);}
    let truncated=out.chars().count()>max_chars;
    let text=if truncated{out.chars().take(max_chars).collect()}else{out};
    Ok(ReaderText{format:ReaderFormat::Docx,text,truncated,
        extracted_from_image:false})
}
pub fn read_docx_bytes(bytes:&[u8],max_chars:usize)
    ->Result<ReaderText,ReaderFailure>{
    let xml=document_xml(bytes)?;
    parse_document(&xml,max_chars)
}
#[cfg(test)]
mod tests{
    use super::*;
    use flate2::{write::DeflateEncoder,Compression};
    use std::io::Write;
    fn fixture(xml:&[u8],deflate:bool)->Vec<u8>{
        let name=b"word/document.xml";
        let packed=if deflate{
            let mut writer=DeflateEncoder::new(Vec::new(),Compression::default());
            writer.write_all(xml).unwrap();
            writer.finish().unwrap()
        }else{xml.to_vec()};
        let mut buf=Vec::new();
        let n=if deflate{8u16}else{0u16};
        let crc=crc32fast::hash(xml);
        fn a16(dst:&mut Vec<u8>,n:u16){dst.extend_from_slice(&n.to_le_bytes());}
        fn a32(dst:&mut Vec<u8>,n:u32){dst.extend_from_slice(&n.to_le_bytes());}
        a32(&mut buf,0x04034b50);a16(&mut buf,20);a16(&mut buf,0);
        a16(&mut buf,n);a16(&mut buf,0);a16(&mut buf,0);a32(&mut buf,crc);
        a32(&mut buf,packed.len() as u32);a32(&mut buf,xml.len() as u32);
        a16(&mut buf,name.len() as u16);a16(&mut buf,0);
        buf.extend_from_slice(name);buf.extend_from_slice(&packed);
        let cd_offset=buf.len();
        a32(&mut buf,0x02014b50);a16(&mut buf,20);a16(&mut buf,20);
        a16(&mut buf,0);a16(&mut buf,n);a16(&mut buf,0);a16(&mut buf,0);
        a32(&mut buf,crc);a32(&mut buf,packed.len() as u32);
        a32(&mut buf,xml.len() as u32);a16(&mut buf,name.len() as u16);
        a16(&mut buf,0);a16(&mut buf,0);a16(&mut buf,0);a16(&mut buf,0);
        a32(&mut buf,0);a32(&mut buf,0);buf.extend_from_slice(name);
        let cd_len=buf.len()-cd_offset;
        a32(&mut buf,0x06054b50);a16(&mut buf,0);a16(&mut buf,0);
        a16(&mut buf,1);a16(&mut buf,1);a32(&mut buf,cd_len as u32);
        a32(&mut buf,cd_offset as u32);a16(&mut buf,0);
        buf
    }
    const XML:&[u8]=br#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Hello &amp; world</w:t></w:r></w:p><w:tbl><w:tr><w:tc><w:p><w:r><w:t>A</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>B</w:t></w:r></w:p></w:tc></w:tr></w:tbl><w:p><w:del><w:r><w:t>Deleted</w:t></w:r></w:del><w:ins><w:r><w:t>Kept</w:t></w:r></w:ins></w:p></w:body></w:document>"#;
    #[test]
    fn reads_stored_and_deflate_real_zip(){
        for deflate in [false,true]{
            let doc=fixture(XML,deflate);
            let text=read_docx_bytes(&doc,500).unwrap();
            assert!(text.text.contains("Hello & world"));
            assert!(text.text.contains("A\n\tB"));
            assert!(text.text.contains("Kept"));
            assert!(!text.text.contains("Deleted"));
            assert!(!text.truncated);
        }
    }
    #[test]
    fn truncated_output_and_crc_rejection(){
        let doc=fixture(XML,true);
        assert!(read_docx_bytes(&doc,5).unwrap().truncated);
        let mut broken=doc.clone();
        let cd=broken.windows(4).position(|w|w==b"PK\x01\x02").unwrap();
        broken[cd+16]^=0x42;
        assert_eq!(read_docx_bytes(&broken,500),Err(ReaderFailure::InvalidDocument));
    }
    #[test]
    fn refuses_dtd_and_fake_zip(){
        let xml=b"<!DOCTYPE x [<!ENTITY x SYSTEM 'file:///etc/passwd'>]><w:document><w:p><w:t>Hello</w:t></w:p></w:document>";
        assert_eq!(read_docx_bytes(&fixture(xml,true),500),
            Err(ReaderFailure::InvalidDocument));
        assert_eq!(read_docx_bytes(b"PK\x03\x04fake",500),
            Err(ReaderFailure::InvalidDocument));
    }
}
