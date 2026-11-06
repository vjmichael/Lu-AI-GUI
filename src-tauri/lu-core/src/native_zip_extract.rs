//! WAVE2: selected ZIP member extraction; no bulk/untrusted paths, no overwrite.
//! The host selects already-open source and destination directory handles.
//! Callers MUST dispatch via a persisted human turn, an exact content policy
//! and the durable effect journal before invoking this write primitive.
use cap_std::fs::{Dir,OpenOptions};
use std::{collections::BTreeSet,io::{Read,Write},path::Path};
use sha2::{Digest,Sha256};
use zip::{ZipArchive,CompressionMethod};
const MAX_ARCHIVE:u64=128*1024*1024;
const MAX_ENTRIES:usize=4096;
const MAX_MEMBER:u64=64*1024*1024;
const MAX_TOTAL:u64=256*1024*1024;
const MAX_RATIO:u64=1000;
const MAX_NAME:usize=1024;
const MAX_STAGE:usize=128;
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum ZipExtractError{
    InvalidName,InvalidStage,MissingSource,NotRegularFile,InvalidZip,
    UnsupportedMethod,UnsafeEntry,DuplicateEntry,TooLarge,NotFound,
    SourceChanged,TargetExists,StageCollision,PublishedNeedsReconciliation,
    Io(String),
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct ZipExtractReceipt{
    pub target_name:String,pub source_member:String,pub bytes_written:u64,
    pub sha256:String,
}
fn io(e:std::io::Error)->ZipExtractError{ZipExtractError::Io(e.to_string())}
fn base_name(n:&str)->bool{
    !n.is_empty()&&n.len()<=240&&n!="."&&n!=".."
        && !n.starts_with(".lu-stage-")&&!n.ends_with('.')&&!n.ends_with(' ')
        && !n.chars().any(|c|c.is_control()||matches!(c,'/'|'\\'|':'))
        && !Path::new(n).is_absolute()
}
fn member_name(n:&str)->bool{
    !n.is_empty()&&n.len()<=MAX_NAME&&!n.starts_with('/')
        && !n.chars().any(|c|c.is_control()||matches!(c,'\\'|':'))
        && n.trim_end_matches('/').split('/').all(|p|
            !p.is_empty()&&p!="."&&p!=".."&&!p.eq_ignore_ascii_case(".git")
            && !p.starts_with('-')&&!p.ends_with('.')&&!p.ends_with(' '))
}
fn stage_ok(n:&str)->bool{
    (16..=MAX_STAGE).contains(&n.len())
        && n.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'-'||b==b'_')
}
fn sha_ok(n:&str)->bool{
    n.len()==64&&n.bytes().all(|b|b.is_ascii_hexdigit()&&!b.is_ascii_uppercase())
}
fn source_zip(root:&Dir,name:&str)->Result<ZipArchive<cap_std::fs::File>,ZipExtractError>{
    if !base_name(name)||!name.to_ascii_lowercase().ends_with(".zip"){
        return Err(ZipExtractError::InvalidName)
    }
    let meta=root.symlink_metadata(name).map_err(|_|ZipExtractError::MissingSource)?;
    if !meta.is_file()||meta.file_type().is_symlink(){
        return Err(ZipExtractError::NotRegularFile)
    }
    #[cfg(windows)]{
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes()&0x400!=0{return Err(ZipExtractError::NotRegularFile)}
    }
    if meta.len()>MAX_ARCHIVE{return Err(ZipExtractError::TooLarge)}
    let handle=root.open(name).map_err(io)?;
    let hmeta=handle.metadata().map_err(io)?;
    if !hmeta.is_file()||hmeta.len()>MAX_ARCHIVE{
        return Err(ZipExtractError::NotRegularFile)
    }
    let mut archive=ZipArchive::new(handle).map_err(|_|ZipExtractError::InvalidZip)?;
    if archive.len()>MAX_ENTRIES{return Err(ZipExtractError::TooLarge)}
    if archive.has_overlapping_entries().map_err(|_|ZipExtractError::InvalidZip)?{
        return Err(ZipExtractError::InvalidZip)
    }
    archive.validate_header_consistency().map_err(|_|ZipExtractError::InvalidZip)?;
    Ok(archive)
}
/// Validate the *entire* ZIP before reading one member. Fail closed on links,
/// duplicates, encrypted or unsupported compression, and bomb-like metadata.
/// The actual extraction separately limits real byte count and checks CRC32.
fn chosen_member(
    archive:&mut ZipArchive<cap_std::fs::File>,member:&str
)->Result<usize,ZipExtractError>{
    if !member_name(member)||member.ends_with('/'){
        return Err(ZipExtractError::InvalidName)
    }
    let mut seen=BTreeSet::new();
    let mut selected=None;
    let mut total=0u64;
    for i in 0..archive.len(){
        let entry=archive.by_index_raw(i).map_err(|_|ZipExtractError::InvalidZip)?;
        let name=entry.name().map_err(|_|ZipExtractError::InvalidZip)?.into_owned();
        if !member_name(&name){return Err(ZipExtractError::UnsafeEntry)}
        let key=name.trim_end_matches('/').to_lowercase();
        if !seen.insert(key){return Err(ZipExtractError::DuplicateEntry)}
        let mode=entry.unix_mode().unwrap_or(0);
        let kind=mode&0o170000;
        if entry.is_symlink() || kind==0o120000 ||
            (kind!=0&&kind!=0o100000&&kind!=0o040000){
            return Err(ZipExtractError::UnsafeEntry)
        }
        let directory=entry.is_dir();
        if (directory&&kind==0o100000)||(!directory&&kind==0o040000){
            return Err(ZipExtractError::UnsafeEntry)
        }
        let expanded=entry.size();
        let compressed=entry.compressed_size();
        if expanded>MAX_MEMBER||(expanded>0&&compressed==0)
            ||(compressed>0&&expanded/compressed>MAX_RATIO){
            return Err(ZipExtractError::TooLarge)
        }
        total=total.checked_add(expanded).ok_or(ZipExtractError::TooLarge)?;
        if total>MAX_TOTAL{return Err(ZipExtractError::TooLarge)}
        if name==member&&!directory{selected=Some(i)}
    }
    selected.ok_or(ZipExtractError::NotFound)
}
fn cleanup(root:&Dir,name:&str,original:ZipExtractError)->ZipExtractError{
    if root.remove_file(name).is_err(){
        ZipExtractError::PublishedNeedsReconciliation
    }else{original}
}
/// Compute an exact content preview without creating any file.
pub fn zip_member_sha256(
    source:&Dir,archive_name:&str,member:&str
)->Result<String,ZipExtractError>{
    let mut zip=source_zip(source,archive_name)?;
    let index=chosen_member(&mut zip,member)?;
    let mut f=zip.by_index(index).map_err(|_|ZipExtractError::InvalidZip)?;
    if !matches!(f.compression(),CompressionMethod::Stored|CompressionMethod::Deflated){
        return Err(ZipExtractError::UnsupportedMethod)
    }
    let mut hasher=Sha256::new();
    let mut buf=[0u8;64*1024];
    let mut size=0u64;
    let mut crc=crc32fast::Hasher::new();
    loop{
        let n=f.read(&mut buf).map_err(|_|ZipExtractError::InvalidZip)?;
        if n==0{break}
        size=size.checked_add(n as u64).ok_or(ZipExtractError::TooLarge)?;
        if size>MAX_MEMBER{return Err(ZipExtractError::TooLarge)}
        hasher.update(&buf[..n]);crc.update(&buf[..n]);
    }
    if size!=f.size()||crc.finalize()!=f.crc32(){
        return Err(ZipExtractError::InvalidZip)
    }
    Ok(format!("{:x}",hasher.finalize()))
}
/// Publish exactly one explicitly selected ZIP member to a new flat filename.
/// A mismatched approved digest never publishes; existing targets are untouched.
/// Supports Stored and Deflated ZIP entries via zip-rs, rejecting other
/// compression methods, encrypted members, and oversized expansion.
pub fn extract_zip_member_new_checked(
    source:&Dir,archive_name:&str,member:&str,
    destination:&Dir,target_name:&str,stage_id:&str,approved_sha256:&str,
)->Result<ZipExtractReceipt,ZipExtractError>{
    if !base_name(target_name){return Err(ZipExtractError::InvalidName)}
    if !stage_ok(stage_id){return Err(ZipExtractError::InvalidStage)}
    if !sha_ok(approved_sha256){return Err(ZipExtractError::SourceChanged)}
    let mut zip=source_zip(source,archive_name)?;
    let index=chosen_member(&mut zip,member)?;
    let mut f=zip.by_index(index).map_err(|_|ZipExtractError::InvalidZip)?;
    if !matches!(f.compression(),CompressionMethod::Stored|CompressionMethod::Deflated){
        return Err(ZipExtractError::UnsupportedMethod)
    }
    let stage=format!(".lu-stage-{stage_id}.zip-extract");
    let mut opts=OpenOptions::new();
    opts.create_new(true).write(true);
    let mut out=destination.open_with(&stage,&opts).map_err(|e|
        if e.kind()==std::io::ErrorKind::AlreadyExists{
            ZipExtractError::StageCollision
        }else{io(e)})?;
    let result=(||->Result<(u64,String),ZipExtractError>{
        let mut hasher=Sha256::new();
        let mut crc=crc32fast::Hasher::new();
        let mut size=0u64;
        let mut buf=[0u8;64*1024];
        loop{
            let n=f.read(&mut buf).map_err(|_|ZipExtractError::InvalidZip)?;
            if n==0{break}
            size=size.checked_add(n as u64).ok_or(ZipExtractError::TooLarge)?;
            if size>MAX_MEMBER{return Err(ZipExtractError::TooLarge)}
            out.write_all(&buf[..n]).map_err(io)?;
            crc.update(&buf[..n]);hasher.update(&buf[..n]);
        }
        if size!=f.size()||crc.finalize()!=f.crc32(){
            return Err(ZipExtractError::InvalidZip)
        }
        out.sync_all().map_err(io)?;
        Ok((size,format!("{:x}",hasher.finalize())))
    })();
    drop(out);
    let (size,digest)=match result{
        Ok(v)=>v,Err(e)=>return Err(cleanup(destination,&stage,e)),
    };
    if digest!=approved_sha256{
        return Err(cleanup(destination,&stage,ZipExtractError::SourceChanged))
    }
    if let Err(e)=destination.hard_link(&stage,destination,target_name){
        return Err(cleanup(destination,&stage,if e.kind()==std::io::ErrorKind::AlreadyExists{
            ZipExtractError::TargetExists
        }else{io(e)}))
    }
    destination.remove_file(&stage)
        .map_err(|_|ZipExtractError::PublishedNeedsReconciliation)?;
    Ok(ZipExtractReceipt{
        target_name:target_name.into(),source_member:member.into(),
        bytes_written:size,sha256:digest,
    })
}
