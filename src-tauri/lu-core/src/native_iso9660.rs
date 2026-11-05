//! ISO-9660 primary-volume image reader. No mounting, shell, UDF,
 //! Rock Ridge metadata interpretation, symlinks or special files.
 //! Host supplies the opened source/destination directory capabilities.
 //! This is an unprivileged parser/primitive, not a model grant.
use std::{collections::BTreeSet, io::{Read,Seek,SeekFrom}, path::Path};
use cap_std::fs::Dir;
use crate::native_atomic_write::{create_new_atomic_in_dir,AtomicWriteError,WriteReceipt};

const SECTOR:u64=2048;
const MAX_IMAGE:u64=16*1024*1024*1024;
const MAX_ENTRIES:usize=4096;
const MAX_DIRECTORY_BYTES:u64=4*1024*1024;
const MAX_METADATA_READ:u64=32*1024*1024;
const MAX_DEPTH:usize=12;
const MAX_EXTRACT_BYTES:u64=1024*1024;

#[derive(Debug,Clone,PartialEq,Eq)]
pub enum IsoError {
    BadName, NotFile, NotFound, InvalidImage, UnsupportedFeature,
    TooLarge, DuplicateEntry, UnsafeEntry, SourceChanged, Io(String), Publish(AtomicWriteError),
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct IsoEntry{
    pub path:String,
    pub is_directory:bool,
    pub bytes:u64,
    pub extent:u64,
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct IsoManifest{
    pub entries:Vec<IsoEntry>,
    pub total_entries:usize,
    pub declared_bytes:u64,
    pub image_bytes:u64,
    pub truncated:bool,
}
fn fail(e:std::io::Error)->IsoError{IsoError::Io(e.to_string())}
fn safe_source(s:&str)->bool{
    !s.is_empty() && s.len()<=240 && s!="." && s!=".."
    && !s.starts_with(".lu-stage-") && !s.ends_with('.') && !s.ends_with(' ')
    && !s.chars().any(|c|c.is_control() || matches!(c,'/'|'\\'|':'))
    && s.to_ascii_lowercase().ends_with(".iso")
}
fn get32(x:&[u8])->u64{
    u32::from_le_bytes([x[0],x[1],x[2],x[3]]) as u64
}
fn check_both32(x:&[u8])->Result<u64,IsoError>{
    if x.len()<8{return Err(IsoError::InvalidImage)}
    let le=get32(x);
    let be=u32::from_be_bytes([x[4],x[5],x[6],x[7]]) as u64;
    if le!=be{return Err(IsoError::InvalidImage)}
    Ok(le)
}
fn check_both16(x:&[u8])->Result<u16,IsoError>{
    if x.len()<4{return Err(IsoError::InvalidImage)}
    let le=u16::from_le_bytes([x[0],x[1]]);
    let be=u16::from_be_bytes([x[2],x[3]]);
    if le!=be{return Err(IsoError::InvalidImage)}
    Ok(le)
}
#[derive(Clone)]
struct Record{extent:u64,size:u64,dir:bool,name:String}
fn parse_record(bytes:&[u8])->Result<Record,IsoError>{
    if bytes.len()<34 || bytes[0] as usize>bytes.len() || bytes[0]<34 {
        return Err(IsoError::InvalidImage)
    }
    let n=bytes[32] as usize;
    if n==0 || 33+n>bytes[0] as usize{return Err(IsoError::InvalidImage)}
    // No interleaved files, external attribute blocks, multi-extent or
    // associated files. Silently treating these as regular files is unsafe.
    if bytes[1]!=0 || bytes[26]!=0 || bytes[27]!=0
      || bytes[25]&0x84!=0 {return Err(IsoError::UnsupportedFeature)}
    let _seq=check_both16(&bytes[28..32])?;
    let extent=check_both32(&bytes[2..10])?;
    let size=check_both32(&bytes[10..18])?;
    let dir=bytes[25]&0x02!=0;
    let raw=&bytes[33..33+n];
    if n==1 && (raw[0]==0 || raw[0]==1) {
        return Ok(Record{extent,size,dir,name:String::new()})
    }
    // Basic ISO-9660 only; UTF-16 Joliet/UDF and Rock Ridge names are
    // separate future adapters. Reject any unrepresentable filename.
    if !raw.iter().all(|b|b.is_ascii_graphic() && *b!=b'/' && *b!=b'\\'){
        return Err(IsoError::UnsafeEntry)
    }
    let input=std::str::from_utf8(raw).map_err(|_|IsoError::UnsafeEntry)?;
    let bare=input.split_once(';').map(|(s,_)|s).unwrap_or(input);
    let name=bare.strip_suffix('.').unwrap_or(bare);
    if name.is_empty() || name.len()>180 || name=="." || name==".."
        || name.starts_with('-') || name.eq_ignore_ascii_case(".git")
        || name.chars().any(|c|c.is_control() || matches!(c,':'|'\\'))
    {return Err(IsoError::UnsafeEntry)}
    if input.contains(';') && !input.rsplit_once(';')
        .is_some_and(|(_,v)|!v.is_empty()&&v.bytes().all(|b|b.is_ascii_digit())){
        return Err(IsoError::UnsafeEntry)
    }
    Ok(Record{extent,size,dir,name:name.into()})
}
fn range_ok(extent:u64,size:u64,length:u64)->bool{
    extent.checked_mul(SECTOR).and_then(|start|start.checked_add(size))
        .is_some_and(|end|end<=length)
}
fn open_iso(root:&Dir,source:&str)->Result<(cap_std::fs::File,u64,Record),IsoError>{
    if !safe_source(source){return Err(IsoError::BadName)}
    let meta=root.symlink_metadata(source).map_err(fail)?;
    if !meta.is_file() || meta.file_type().is_symlink(){return Err(IsoError::NotFile)}
    #[cfg(windows)] {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {return Err(IsoError::NotFile)}
    }
    if meta.len()<17*SECTOR || meta.len()>MAX_IMAGE{return Err(IsoError::TooLarge)}
    let mut file=root.open(source).map_err(fail)?;
    let length=file.metadata().map_err(fail)?.len();
    if length!=meta.len() || !file.metadata().map_err(fail)?.is_file(){
        return Err(IsoError::NotFile)
    }
    let mut pvd=[0u8;2048];
    file.seek(SeekFrom::Start(16*SECTOR)).map_err(fail)?;
    file.read_exact(&mut pvd).map_err(fail)?;
    if pvd[0]!=1 || &pvd[1..6]!=b"CD001" || pvd[6]!=1{
        return Err(IsoError::InvalidImage)
    }
    if check_both16(&pvd[128..132])?!=2048{
        return Err(IsoError::UnsupportedFeature)
    }
    let sectors=check_both32(&pvd[80..88])?;
    if sectors==0 || sectors.checked_mul(SECTOR).is_none_or(|n|n>length){
        return Err(IsoError::InvalidImage)
    }
    let root_rec=parse_record(&pvd[156..])?;
    if !root_rec.dir || root_rec.size>MAX_DIRECTORY_BYTES ||
       !range_ok(root_rec.extent,root_rec.size,length){
        return Err(IsoError::InvalidImage)
    }
    Ok((file,length,root_rec))
}
fn read_dir(file:&mut cap_std::fs::File,length:u64,record:&Record,
  prefix:&str,depth:usize,seen:&mut BTreeSet<String>,
  all:&mut Vec<IsoEntry>,metadata_total:&mut u64)->Result<(),IsoError>{
    if depth>MAX_DEPTH{return Err(IsoError::TooLarge)}
    if record.size>MAX_DIRECTORY_BYTES || !range_ok(record.extent,record.size,length){
        return Err(IsoError::InvalidImage)
    }
    *metadata_total=metadata_total.checked_add(record.size).ok_or(IsoError::TooLarge)?;
    if *metadata_total>MAX_METADATA_READ{return Err(IsoError::TooLarge)}
    let size=usize::try_from(record.size).map_err(|_|IsoError::TooLarge)?;
    let mut data=vec![0;size];
    file.seek(SeekFrom::Start(record.extent*SECTOR)).map_err(fail)?;
    file.read_exact(&mut data).map_err(fail)?;
    let mut pos=0;
    // Collect children first: recursion seeks to other extents.
    let mut folders=Vec::new();
    while pos<data.len(){
        let len=data[pos] as usize;
        if len==0{
            pos=(((pos as u64)/SECTOR)+1) as usize*SECTOR;
            continue
        }
        if pos+len>data.len() || pos%SECTOR+len>SECTOR as usize {
            return Err(IsoError::InvalidImage)
        }
        let item=parse_record(&data[pos..pos+len])?;
        pos+=len;
        if item.name.is_empty(){continue}
        if !range_ok(item.extent,item.size,length){return Err(IsoError::InvalidImage)}
        let path=if prefix.is_empty(){item.name.clone()}else{
            format!("{prefix}/{}",item.name)
        };
        if path.len()>1024{return Err(IsoError::TooLarge)}
        if !seen.insert(path.to_lowercase()){return Err(IsoError::DuplicateEntry)}
        if all.len()>=MAX_ENTRIES{return Err(IsoError::TooLarge)}
        all.push(IsoEntry{
            path:path.clone(),is_directory:item.dir,
            bytes:item.size,extent:item.extent,
        });
        if item.dir{folders.push((path,item))}
    }
    for (path,item) in folders{
        read_dir(file,length,&item,&path,depth+1,seen,all,metadata_total)?
    }
    Ok(())
}
fn enumerate(root:&Dir,source:&str)->Result<(IsoManifest,cap_std::fs::File),IsoError>{
    let (mut file,length,record)=open_iso(root,source)?;
    let mut all=Vec::new();
    let mut seen=BTreeSet::new();
    let mut meta_bytes=0;
    read_dir(&mut file,length,&record,"",0,&mut seen,&mut all,&mut meta_bytes)?;
    let total=all.iter().filter(|e|!e.is_directory).try_fold(0u64,|sum,e|
        sum.checked_add(e.bytes).ok_or(IsoError::TooLarge))?;
    let result=IsoManifest{
        total_entries:all.len(),declared_bytes:total,image_bytes:length,
        entries:all,truncated:false,
    };
    Ok((result,file))
}
/// Metadata-only listing of ISO-9660 PVD files, with strict member and read caps.
pub fn inspect_iso(root:&Dir,source:&str,max_entries:usize)->Result<IsoManifest,IsoError>{
    if max_entries==0 || max_entries>256{return Err(IsoError::TooLarge)}
    let (mut manifest,_file)=enumerate(root,source)?;
    manifest.truncated=manifest.entries.len()>max_entries;
    manifest.entries.truncate(max_entries);
    Ok(manifest)
}
/// Extract exactly one regular ISO member into a new file. The host must have
/// authorized source image, member name, target and target directory separately.
/// File size is <=1 MiB until the streamed writer receives durable
/// interruption/reconciliation integration. No auto-replay of uncertain writes.
fn member_bytes(source_root:&Dir,source:&str,member:&str)->Result<Vec<u8>,IsoError>{
    let (manifest,mut file)=enumerate(source_root,source)?;
    let chosen=manifest.entries.iter().find(|e|e.path==member)
        .ok_or(IsoError::NotFound)?;
    if chosen.is_directory{return Err(IsoError::UnsupportedFeature)}
    if chosen.bytes>MAX_EXTRACT_BYTES{return Err(IsoError::TooLarge)}
    let mut bytes=vec![0u8;chosen.bytes as usize];
    file.seek(SeekFrom::Start(chosen.extent*SECTOR)).map_err(fail)?;
    file.read_exact(&mut bytes).map_err(fail)?;
    Ok(bytes)
}
pub fn iso_member_digest(
    source_root:&Dir,source:&str,member:&str
)->Result<String,IsoError>{
    use sha2::{Digest,Sha256};
    let bytes=member_bytes(source_root,source,member)?;
    Ok(format!("{:x}",Sha256::digest(&bytes)))
}
/// An exact hash is checked again against bytes actually published. It is
/// not a grant; the trusted host must independently authorize it.
pub fn extract_iso_file_new_checked(
    source_root:&Dir,source:&str,member:&str,
    destination:&Dir,new_name:&str,host_stage_id:&str,
    approved_sha256:&str,
)->Result<WriteReceipt,IsoError>{
    use sha2::{Digest,Sha256};
    let bytes=member_bytes(source_root,source,member)?;
    if format!("{:x}",Sha256::digest(&bytes))!=approved_sha256 {
        return Err(IsoError::SourceChanged)
    }
    create_new_atomic_in_dir(destination,new_name,host_stage_id,&bytes)
        .map_err(IsoError::Publish)
}
pub fn extract_iso_file_new(
    source_root:&Dir,source:&str,member:&str,
    destination:&Dir,new_name:&str,host_stage_id:&str,
)->Result<WriteReceipt,IsoError>{
    let digest=iso_member_digest(source_root,source,member)?;
    extract_iso_file_new_checked(
        source_root,source,member,destination,new_name,host_stage_id,&digest
    )
}
