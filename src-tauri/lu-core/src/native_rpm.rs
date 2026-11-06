//! Native read-only RPM v3/v4 header and CPIO payload parser.
//! Never install, execute scripts, restore ownership, or trust signatures.
//! All extracted paths are prefixed by payload/; errors fail closed.
use std::{io::Read,collections::BTreeSet};
use cap_std::fs::Dir;
use crate::{native_cpio::parse_cpio_newc,
    native_linux_archive::{LinuxArchiveFormat,expand_bounded_linux_stream}};
const MAX_SOURCE:u64=128*1024*1024;
const MAX_UNPACKED:usize=256*1024*1024;
const MAX_HEADER:usize=16*1024*1024;
const MAX_RECORDS:usize=8192;
const LEAD_SIZE:usize=96;
#[derive(Clone,Debug,PartialEq,Eq)]
pub enum RpmError{UnsafeSource,InvalidLead,InvalidHeader,InvalidMetadata,
    UnsupportedPayload,UnsafePayload,MissingPayload,TooLarge,Io(String)}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct RpmInfo{pub name:Option<String>,pub version:Option<String>,
    pub release:Option<String>,pub arch:Option<String>,
    pub payload_format:String,pub compressor:String}
struct Header<'a>{records:Vec<Record>,store:&'a [u8],end:usize}
struct Record{tag:u32,kind:u32,offset:usize,count:usize}
fn be(data:&[u8])->Result<u32,RpmError>{
    Ok(u32::from_be_bytes(data.try_into().map_err(|_|RpmError::InvalidHeader)?))
}
fn parse_header(input:&[u8],start:usize)->Result<Header<'_>,RpmError>{
    let end=start.checked_add(16).ok_or(RpmError::TooLarge)?;
    if end>input.len()||input[start..start+4]!=[0x8e,0xad,0xe8,0x01]
        ||input[start+4..start+8]!=[0u8;4]{
        return Err(RpmError::InvalidHeader)
    }
    let count=be(&input[start+8..start+12])? as usize;
    let data_len=be(&input[start+12..start+16])? as usize;
    if count>MAX_RECORDS||data_len>MAX_HEADER{return Err(RpmError::TooLarge)}
    let start_data=end.checked_add(count.checked_mul(16)
        .ok_or(RpmError::TooLarge)?).ok_or(RpmError::TooLarge)?;
    let finish=start_data.checked_add(data_len).ok_or(RpmError::TooLarge)?;
    if finish>input.len(){return Err(RpmError::InvalidHeader)}
    let store=&input[start_data..finish];
    let mut records=Vec::with_capacity(count);
    let mut seen=BTreeSet::new();
    for i in 0..count{
        let o=end+i*16;
        let tag=be(&input[o..o+4])?;
        let kind=be(&input[o+4..o+8])?;
        let rel=be(&input[o+8..o+12])? as usize;
        let n=be(&input[o+12..o+16])? as usize;
        if !seen.insert(tag)||rel>store.len()||n>MAX_HEADER{
            return Err(RpmError::InvalidHeader)
        }
        let required=match kind {
            0=>0,1|2|7=>n,3=>n.checked_mul(2).ok_or(RpmError::TooLarge)?,
            4=>n.checked_mul(4).ok_or(RpmError::TooLarge)?,
            5=>n.checked_mul(8).ok_or(RpmError::TooLarge)?,
            6|8|9=>1,
            _=>return Err(RpmError::InvalidHeader),
        };
        if rel.checked_add(required).is_none_or(|x|x>store.len()){
            return Err(RpmError::InvalidHeader)
        }
        records.push(Record{tag,kind,offset:rel,count:n});
    }
    Ok(Header{records,store,end:finish})
}
impl Header<'_>{
    fn text(&self,tag:u32)->Result<Option<String>,RpmError>{
        let Some(p)=self.records.iter().find(|r|r.tag==tag)else{return Ok(None)};
        if p.kind!=6||p.count!=1{return Err(RpmError::InvalidMetadata)}
        let end=self.store[p.offset..].iter().position(|b|*b==0)
            .ok_or(RpmError::InvalidMetadata)?+p.offset;
        if end-p.offset>4096{return Err(RpmError::InvalidMetadata)}
        let s=std::str::from_utf8(&self.store[p.offset..end])
            .map_err(|_|RpmError::InvalidMetadata)?;
        if s.chars().any(|c|c.is_control()){return Err(RpmError::InvalidMetadata)}
        Ok(Some(s.into()))
    }
}
fn align8(n:usize)->Result<usize,RpmError>{
    n.checked_add(7).map(|v|v&!7).ok_or(RpmError::TooLarge)
}
pub fn parse_rpm(raw:&[u8])
    ->Result<(RpmInfo,Vec<(String,bool,Vec<u8>)>),RpmError>{
    if raw.len() as u64>MAX_SOURCE{return Err(RpmError::TooLarge)}
    if raw.len()<LEAD_SIZE+32||raw[..4]!=[0xed,0xab,0xee,0xdb]||
        !matches!(raw[4],3|4){return Err(RpmError::InvalidLead)}
    let sig=parse_header(raw,LEAD_SIZE)?;
    let main_start=align8(sig.end)?;
    if main_start>raw.len()||raw[sig.end..main_start].iter().any(|b|*b!=0){
        return Err(RpmError::InvalidHeader)
    }
    let main=parse_header(raw,main_start)?;
    let format=main.text(1124)?.unwrap_or_else(||"cpio".into());
    let compressor=main.text(1125)?.unwrap_or_else(||"gzip".into());
    if format!="cpio"{return Err(RpmError::UnsupportedPayload)}
    if main.end>=raw.len(){return Err(RpmError::MissingPayload)}
    let payload=&raw[main.end..];
    let codec=match compressor.as_str(){
        "gzip"|"gz"=>Some(LinuxArchiveFormat::Gzip),
        "xz"=>Some(LinuxArchiveFormat::Xz),
        "bzip2"|"bzip"=>Some(LinuxArchiveFormat::Bzip2),
        "zstd"|"zst"=>Some(LinuxArchiveFormat::Zstd),
        "none"=>None,
        _=>return Err(RpmError::UnsupportedPayload),
    };
    let clear=if let Some(codec)=codec{
        expand_bounded_linux_stream(codec,payload)
            .map_err(|_|RpmError::UnsupportedPayload)?
    }else{payload.to_vec()};
    if clear.len()>MAX_UNPACKED{return Err(RpmError::TooLarge)}
    let entries=parse_cpio_newc(&clear).map_err(|_|RpmError::UnsafePayload)?;
    if entries.is_empty(){return Err(RpmError::MissingPayload)}
    let info=RpmInfo{name:main.text(1000)?,version:main.text(1001)?,
        release:main.text(1002)?,arch:main.text(1022)?,
        payload_format:format,compressor};
    let files=entries.into_iter().map(|m|(
        format!("payload/{}",m.path),m.directory,m.bytes,
    )).collect();
    Ok((info,files))
}
pub fn rpm_tree_members(root:&Dir,name:&str)
    ->Result<Vec<(String,bool,Vec<u8>)>,RpmError>{
    if !name.to_ascii_lowercase().ends_with(".rpm")||name.len()>240
        ||name.starts_with(".lu-")||
        name.chars().any(|c|c.is_control()||matches!(c,'/'|'\\'|':')){
        return Err(RpmError::UnsafeSource)
    }
    let meta=root.symlink_metadata(name).map_err(|e|RpmError::Io(e.to_string()))?;
    if !meta.is_file()||meta.file_type().is_symlink()
        ||meta.len()>MAX_SOURCE{return Err(RpmError::UnsafeSource)}
    #[cfg(windows)]{
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes()&0x400!=0{return Err(RpmError::UnsafeSource)}
    }
    let file=root.open(name).map_err(|e|RpmError::Io(e.to_string()))?;
    if !file.metadata().map_err(|e|RpmError::Io(e.to_string()))?.is_file(){
        return Err(RpmError::UnsafeSource)
    }
    let mut raw=Vec::new();
    file.take(MAX_SOURCE+1).read_to_end(&mut raw)
        .map_err(|e|RpmError::Io(e.to_string()))?;
    if raw.len() as u64>MAX_SOURCE{return Err(RpmError::TooLarge)}
    Ok(parse_rpm(&raw)?.1)
}
