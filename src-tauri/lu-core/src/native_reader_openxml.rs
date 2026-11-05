//! Minimal read-only Open XML package admission for Reader format adapters.
//! Only central-directory-confirmed parts are returned; no extraction to disk,
//! external links, macro content, ZIP64, encrypted entries or data descriptors.
use crate::native_reader::ReaderFailure;
use flate2::read::DeflateDecoder;
use quick_xml::{events::Event,Reader};
use std::{collections::BTreeMap,io::Read};

const MAX_ARCHIVE_BYTES:usize=16*1024*1024;
const MAX_ENTRY_BYTES:usize=4*1024*1024;
const MAX_INFLATED_TOTAL:usize=12*1024*1024;
const MAX_ENTRIES:usize=512;
fn malformed()->ReaderFailure{ReaderFailure::InvalidDocument}
fn u16_at(b:&[u8],pos:usize)->Result<usize,ReaderFailure>{
    let bytes=b.get(pos..pos+2).ok_or_else(malformed)?;
    Ok(u16::from_le_bytes([bytes[0],bytes[1]]) as usize)
}
fn u32_at(b:&[u8],pos:usize)->Result<usize,ReaderFailure>{
    let bytes=b.get(pos..pos+4).ok_or_else(malformed)?;
    Ok(u32::from_le_bytes([bytes[0],bytes[1],bytes[2],bytes[3]]) as usize)
}
fn slice_at<'a>(bytes:&'a [u8],start:usize,size:usize)->Result<&'a [u8],ReaderFailure>{
    bytes.get(start..start.checked_add(size).ok_or_else(malformed)?)
        .ok_or_else(malformed)
}
#[derive(Debug)]
pub struct OpenXmlPackage{
    parts:BTreeMap<String,Vec<u8>>,
}
fn reject_external_relationships(xml:&[u8])->Result<(),ReaderFailure>{
    let mut reader=Reader::from_reader(xml);
    let mut events=0usize;
    loop{
        events+=1;
        if events>10_000{return Err(ReaderFailure::TooLarge)}
        let event=reader.read_event().map_err(|_|malformed())?;
        match event{
            Event::Start(ref e)|Event::Empty(ref e)
                if e.local_name().as_ref()==b"Relationship"=>{
                let mut target_mode=None::<String>;
                let mut target=None::<String>;
                for item in e.attributes().with_checks(true){
                    let attr=item.map_err(|_|malformed())?;
                    let raw=std::str::from_utf8(attr.value.as_ref())
                        .map_err(|_|malformed())?;
                    let value=quick_xml::escape::unescape(raw)
                        .map_err(|_|malformed())?.into_owned();
                    if attr.key.as_ref().eq_ignore_ascii_case(b"TargetMode"){
                        target_mode=Some(value);
                    }else if attr.key.as_ref().eq_ignore_ascii_case(b"Target"){
                        target=Some(value);
                    }
                }
                if target_mode.as_deref().is_some_and(|m|
                    !m.eq_ignore_ascii_case("Internal")) {
                    return Err(malformed())
                }
                if let Some(target)=target{
                    let lower=target.to_ascii_lowercase();
                    if lower.contains("://")||lower.starts_with("file:")||
                        lower.starts_with("mailto:")||lower.starts_with("\\\\")||
                        lower.starts_with('/')||lower.contains('?')||
                        lower.contains('#'){
                        return Err(malformed())
                    }
                }
            },
            Event::DocType(_)=>return Err(malformed()),
            Event::Eof=>break,
            _=>{},
        }
    }
    Ok(())
}
impl OpenXmlPackage{
    pub fn new(bytes:&[u8])->Result<Self,ReaderFailure>{
        Self::new_inner(bytes,true)
    }
    pub fn new_odf(bytes:&[u8])->Result<Self,ReaderFailure>{
        Self::new_inner(bytes,false)
    }
    fn new_inner(bytes:&[u8],ooxml:bool)->Result<Self,ReaderFailure>{
        if bytes.len()<22||bytes.len()>MAX_ARCHIVE_BYTES||
           !bytes.starts_with(b"PK\x03\x04"){return Err(malformed())}
        let start=bytes.len().saturating_sub(65_535+22);
        let end=bytes.len()-22;
        let eocd=(start..=end).rev().find(|&i|
            bytes.get(i..i+4)==Some(b"PK\x05\x06") &&
            u16_at(bytes,i+20).ok().is_some_and(|n|i+22+n==bytes.len()))
            .ok_or_else(malformed)?;
        if u16_at(bytes,eocd+4)?!=0||u16_at(bytes,eocd+6)?!=0{
            return Err(malformed())
        }
        let entries=u16_at(bytes,eocd+10)?;
        if entries==0||entries>MAX_ENTRIES||entries!=u16_at(bytes,eocd+8)?{
            return Err(ReaderFailure::TooLarge)
        }
        let directory_size=u32_at(bytes,eocd+12)?;
        let directory_offset=u32_at(bytes,eocd+16)?;
        if directory_offset.checked_add(directory_size)!=Some(eocd){
            return Err(malformed()) // reject ZIP64, prefixed archive, overlap
        }
        let mut at=directory_offset;
        let mut total=0usize;
        let mut parts=BTreeMap::new();
        for _ in 0..entries{
            if slice_at(bytes,at,4)?!=b"PK\x01\x02" {return Err(malformed())}
            let flags=u16_at(bytes,at+8)?;
            let method=u16_at(bytes,at+10)?;
            let crc=u32_at(bytes,at+16)? as u32;
            let packed=u32_at(bytes,at+20)?;
            let unpacked=u32_at(bytes,at+24)?;
            let name_len=u16_at(bytes,at+28)?;
            let extra_len=u16_at(bytes,at+30)?;
            let comment_len=u16_at(bytes,at+32)?;
            let local=u32_at(bytes,at+42)?;
            if flags & 0x09!=0||!matches!(method,0|8)||
               [packed,unpacked,local].contains(&(u32::MAX as usize)){
                return Err(malformed())
            }
            if packed>MAX_ENTRY_BYTES||unpacked>MAX_ENTRY_BYTES||
               unpacked>packed.saturating_mul(250).saturating_add(4096){
                return Err(ReaderFailure::TooLarge)
            }
            total=total.checked_add(unpacked).ok_or(ReaderFailure::TooLarge)?;
            if total>MAX_INFLATED_TOTAL{return Err(ReaderFailure::TooLarge)}
            let entry_size=46usize.checked_add(name_len)
                .and_then(|n|n.checked_add(extra_len))
                .and_then(|n|n.checked_add(comment_len)).ok_or_else(malformed)?;
            if at.checked_add(entry_size).is_none_or(|end|end>eocd){
                return Err(malformed())
            }
            let raw_name=slice_at(bytes,at+46,name_len)?;
            let name=std::str::from_utf8(raw_name).map_err(|_|malformed())?;
            let directory=name.ends_with('/');
            let clean_name=if directory{&name[..name.len()-1]}else{name};
            if clean_name.is_empty()||name.len()>512||name.contains('\\')||
               name.contains('\0')||name.starts_with('/')||
               clean_name.split('/').any(|s|s=="."||s==".."||s.is_empty())||
               (directory && (packed!=0 || unpacked!=0)){
                return Err(malformed())
            }
            let lower=name.to_ascii_lowercase();
            if lower.ends_with("vbaproject.bin")||
                lower.starts_with("basic/")||lower.starts_with("scripts/")||
                lower.ends_with(".exe")||lower.ends_with(".dll"){
                return Err(malformed()) // no macros or binaries in Reader v1
            }
            if slice_at(bytes,local,4)?!=b"PK\x03\x04"||
               u16_at(bytes,local+8)?!=method||u16_at(bytes,local+6)?!=flags{
                return Err(malformed())
            }
            let lname=u16_at(bytes,local+26)?;
            let lextra=u16_at(bytes,local+28)?;
            if slice_at(bytes,local+30,lname)?!=raw_name{return Err(malformed())}
            let data_at=local.checked_add(30).and_then(|p|p.checked_add(lname))
                .and_then(|p|p.checked_add(lextra)).ok_or_else(malformed)?;
            if data_at.checked_add(packed).is_none_or(|p|p>directory_offset){
                return Err(malformed())
            }
            let source=slice_at(bytes,data_at,packed)?;
            let part=if method==0 {
                if packed!=unpacked{return Err(malformed())}
                source.to_vec()
            }else{
                let mut decoded=Vec::with_capacity(unpacked.min(4096));
                DeflateDecoder::new(source).take(MAX_ENTRY_BYTES as u64+1)
                    .read_to_end(&mut decoded).map_err(|_|malformed())?;
                decoded
            };
            if part.len()!=unpacked||crc32fast::hash(&part)!=crc{
                return Err(malformed())
            }
            if !directory && parts.insert(name.to_owned(),part).is_some(){
                return Err(malformed())
            }
            at+=entry_size;
        }
        if at!=eocd{return Err(malformed())}
        if ooxml&&!parts.contains_key("[Content_Types].xml"){
            return Err(malformed())
        }
        if !ooxml&&(!parts.contains_key("mimetype")||!parts.contains_key("content.xml")){
            return Err(malformed())
        }
        // Reject DTDs in all XML parts, and external relationship targets.
        for (name,content) in &parts{
            if name.ends_with(".xml")||name.ends_with(".rels"){
                if content.windows(9).any(|w|w.eq_ignore_ascii_case(b"<!DOCTYPE"))
                   ||content.windows(8).any(|w|w.eq_ignore_ascii_case(b"<!ENTITY")){
                    return Err(malformed())
                }
                if name.to_ascii_lowercase().ends_with(".rels"){
                    reject_external_relationships(content)?;
                }
            }
        }
        Ok(Self{parts})
    }
    pub fn part(&self,path:&str)->Result<&[u8],ReaderFailure>{
        self.parts.get(path).map(Vec::as_slice).ok_or_else(malformed)
    }
    pub fn optional_part(&self,path:&str)->Option<&[u8]>{
        self.parts.get(path).map(Vec::as_slice)
    }
    pub fn has(&self,path:&str)->bool{self.parts.contains_key(path)}
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn rejects_external_relationships_regardless_of_xml_quoting_or_spacing(){
        for xml in [
            r#"<Relationships><Relationship Id='rId1' TargetMode='External' Target='https://remote.example/secret'/></Relationships>"#,
            r#"<Relationships><Relationship TargetMode = "external" Target="file:///etc/passwd"/></Relationships>"#,
            r#"<Relationships><Relationship Id="rId3" Target="https://remote.example/file" /></Relationships>"#,
            r#"<Relationships><Relationship Id="rId5" TargetMode="FTP" Target="slides/s.xml"/></Relationships>"#,
        ]{
            assert!(reject_external_relationships(xml.as_bytes()).is_err());
        }
        assert!(reject_external_relationships(
            br#"<Relationships><Relationship Id="rId4" Target="slides/slide1.xml"/></Relationships>"#
        ).is_ok());
    }
    #[test]
    fn rejects_fake_zip_and_non_ooxml(){
        assert!(OpenXmlPackage::new(b"PK\x03\x04bad").is_err());
    }
}
