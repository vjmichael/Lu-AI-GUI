//! Safe CPIO newc/CRC (070701,070702) Linux backup/initramfs reader.
//! Read-only byte parsing; no archive member becomes a filesystem path until
//! the separate archive-tree authorizer approves new-directory extraction.
//! Hardlinks, symlinks, devices, FIFOs, overwrite aliases and traversal refused.
use std::{collections::BTreeSet,io::Read};
use cap_std::fs::Dir;
use crate::native_linux_archive::{
    LinuxArchiveFormat,expand_bounded_linux_stream,
};
const MAX_SOURCE:u64=128*1024*1024;
const MAX_UNPACK:usize=256*1024*1024;
const MAX_FILE:usize=64*1024*1024;
const MAX_ENTRIES:usize=4096;
const MAX_NAME:usize=1024;
const MAX_DEPTH:usize=16;
const HEADER:usize=110;
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum CpioError{
    UnsupportedFormat,UnsafeSource,InvalidHeader,InvalidEncoding,
    UnsafeEntry,UnsupportedEntry,InvalidChecksum,DuplicateEntry,
    TooLarge,MissingTrailer,BackendUnavailable,Io(String),
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct CpioMember{pub path:String,pub directory:bool,pub bytes:Vec<u8>}
pub fn is_cpio_name(name:&str)->bool{
    let n=name.to_ascii_lowercase();
    [".cpio",".cpio.gz",".cpio.xz",".cpio.bz2",".cpio.zst"]
        .iter().any(|suffix|n.ends_with(suffix))
}
fn encoding(name:&str)->Result<Option<LinuxArchiveFormat>,CpioError>{
    let n=name.to_ascii_lowercase();
    if n.ends_with(".cpio.gz"){Ok(Some(LinuxArchiveFormat::Gzip))}
    else if n.ends_with(".cpio.xz"){Ok(Some(LinuxArchiveFormat::Xz))}
    else if n.ends_with(".cpio.bz2"){Ok(Some(LinuxArchiveFormat::Bzip2))}
    else if n.ends_with(".cpio.zst"){Ok(Some(LinuxArchiveFormat::Zstd))}
    else if n.ends_with(".cpio"){Ok(None)}
    else{Err(CpioError::UnsupportedFormat)}
}
fn plain(n:&str)->bool{
    !n.is_empty()&&n.len()<=240&&n!="."&&n!=".."
        &&!n.starts_with(".lu-")&&!n.ends_with('.')&&!n.ends_with(' ')
        &&!n.chars().any(|c|c.is_control()||matches!(c,'/'|'\\'|':'))
}
fn safe_path(s:&str,directory:bool)->Result<Option<String>,CpioError>{
    let name=if directory{s.trim_end_matches('/')}else{s};
    if name=="."&&directory{return Ok(None)}
    let name=name.strip_prefix("./").unwrap_or(name);
    if name.is_empty()||name.len()>MAX_NAME||name.starts_with('/')||
        name.chars().any(|c|c.is_control()||matches!(c,'\\'|':')){
        return Err(CpioError::UnsafeEntry)
    }
    let parts=name.split('/').collect::<Vec<_>>();
    if parts.len()>MAX_DEPTH||parts.iter().any(|x|
        x.is_empty()||*x=="."||*x==".."||x.len()>200||
        x.starts_with(".lu-")||x.starts_with('-')||
        x.ends_with('.')||x.ends_with(' ')||x.eq_ignore_ascii_case(".git")){
        return Err(CpioError::UnsafeEntry)
    }
    Ok(Some(name.to_owned()))
}
fn number(bytes:&[u8])->Result<u32,CpioError>{
    if bytes.len()!=8||!bytes.iter().all(u8::is_ascii_hexdigit){
        return Err(CpioError::InvalidHeader)
    }
    let s=std::str::from_utf8(bytes).map_err(|_|CpioError::InvalidHeader)?;
    u32::from_str_radix(s,16).map_err(|_|CpioError::InvalidHeader)
}
fn align4(n:usize)->Result<usize,CpioError>{
    n.checked_add(3).map(|v|v&!3).ok_or(CpioError::TooLarge)
}
pub fn parse_cpio_newc(raw:&[u8])->Result<Vec<CpioMember>,CpioError>{
    if raw.len()>MAX_UNPACK{return Err(CpioError::TooLarge)}
    let mut pos=0usize;
    let mut total=0usize;
    let mut names=BTreeSet::new();
    let mut members=Vec::new();
    loop{
        let end=pos.checked_add(HEADER).ok_or(CpioError::TooLarge)?;
        if end>raw.len(){return Err(CpioError::MissingTrailer)}
        let hdr=&raw[pos..end];
        let crc=match &hdr[..6]{
            b"070701"=>false,b"070702"=>true,
            _=>return Err(CpioError::UnsupportedFormat)
        };
        let mode=number(&hdr[14..22])?;
        let nlink=number(&hdr[38..46])?;
        let size=number(&hdr[54..62])? as usize;
        let n=number(&hdr[94..102])? as usize;
        let checksum=number(&hdr[102..110])?;
        if (!crc&&checksum!=0)||n==0||n>MAX_NAME+1||size>MAX_FILE{
            return Err(CpioError::InvalidHeader)
        }
        let nstart=end;
        let nend=nstart.checked_add(n).ok_or(CpioError::TooLarge)?;
        let dstart=align4(nend)?;
        let dend=dstart.checked_add(size).ok_or(CpioError::TooLarge)?;
        let next=align4(dend)?;
        if next>raw.len()||raw[nend-1]!=0||
            raw[nstart..nend-1].contains(&0)||
            raw[nend..dstart].iter().any(|b|*b!=0)||
            raw[dend..next].iter().any(|b|*b!=0){
            return Err(CpioError::InvalidHeader)
        }
        let name=std::str::from_utf8(&raw[nstart..nend-1])
            .map_err(|_|CpioError::InvalidEncoding)?;
        let bytes=&raw[dstart..dend];
        if crc{
            let sum=bytes.iter().fold(0u32,|a,b|a.wrapping_add(*b as u32));
            if sum!=checksum{return Err(CpioError::InvalidChecksum)}
        }
        if name=="TRAILER!!!"{
            if size!=0||mode!=0||raw[next..].iter().any(|b|*b!=0){
                return Err(CpioError::InvalidHeader)
            }
            return Ok(members)
        }
        let directory=match mode&0o170000{
            0o100000=>false,0o040000=>true,
            _=>return Err(CpioError::UnsupportedEntry)
        };
        if (directory&&size!=0)||nlink==0||nlink>(if directory{2}else{1}){
            return Err(CpioError::UnsupportedEntry)
        }
        if let Some(path)=safe_path(name,directory)?{
            if members.len()>=MAX_ENTRIES||!names.insert(path.to_lowercase()){
                return Err(if members.len()>=MAX_ENTRIES{
                    CpioError::TooLarge
                }else{CpioError::DuplicateEntry})
            }
            total=total.checked_add(size).ok_or(CpioError::TooLarge)?;
            if total>MAX_UNPACK{return Err(CpioError::TooLarge)}
            members.push(CpioMember{
                path,directory,bytes:bytes.to_vec(),
            });
        }
        pos=next;
    }
}
pub fn cpio_tree_members(root:&Dir,name:&str)
    ->Result<Vec<(String,bool,Vec<u8>)>,CpioError>{
    if !plain(name){return Err(CpioError::UnsafeSource)}
    let mode=encoding(name)?;
    let meta=root.symlink_metadata(name)
        .map_err(|e|CpioError::Io(e.to_string()))?;
    if !meta.is_file()||meta.file_type().is_symlink()||meta.len()>MAX_SOURCE{
        return Err(CpioError::UnsafeSource)
    }
    #[cfg(windows)] {
        // cap_std::fs::Metadata does not expose Windows file_attributes;
        // preserve the no-follow symlink check on capability-scoped metadata.
        if meta.file_type().is_symlink() { return Err(CpioError::UnsafeSource) }
    }
    let file=root.open(name).map_err(|e|CpioError::Io(e.to_string()))?;
    if !file.metadata().map_err(|e|CpioError::Io(e.to_string()))?.is_file(){
        return Err(CpioError::UnsafeSource)
    }
    let mut raw=Vec::new();
    file.take(MAX_SOURCE+1).read_to_end(&mut raw)
        .map_err(|e|CpioError::Io(e.to_string()))?;
    if raw.len() as u64>MAX_SOURCE{return Err(CpioError::TooLarge)}
    let clear=if let Some(mode)=mode{
        expand_bounded_linux_stream(mode,&raw).map_err(|e|match e{
            crate::native_linux_archive::LinuxArchiveError::TooLarge=>CpioError::TooLarge,
            _=>CpioError::BackendUnavailable,
        })?
    }else{raw};
    let members=parse_cpio_newc(&clear)?;
    if members.is_empty(){return Err(CpioError::InvalidHeader)}
    Ok(members.into_iter().map(|m|(m.path,m.directory,m.bytes)).collect())
}
