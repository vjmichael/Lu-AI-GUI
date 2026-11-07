//! P08 ARCHIVE-SPLIT-01: format-agnostic archive-byte parts, NOT native
//! spanned ZIP/RAR/7z format. All effects require a host-selected Dir grant,
//! explicit native approval, and a host-generated staging token.
use cap_std::fs::{Dir,OpenOptions};
use serde::{Serialize,Deserialize};
use sha2::{Digest,Sha256};
use std::io::{Read,Write};

const MAX_BYTES:u64=512*1024*1024;
const MIN_PART:u64=64*1024;
const MAX_PART:u64=32*1024*1024;
const MAX_PARTS:usize=128;
pub const MANIFEST:&str="lu-parts.json";
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum SplitError{
    Invalid,Unsupported,TooLarge,Changed,PartCorrupt,Exists,StageConflict,
    IncompletePublication,Io(String)
}
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq,Eq)]
#[serde(deny_unknown_fields)]
pub struct Chunk{pub name:String,pub offset:u64,pub bytes:u64,pub sha256:String}
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq,Eq)]
#[serde(deny_unknown_fields)]
pub struct Plan{
    pub version:u8,pub format:String,pub source:String,pub bytes:u64,
    pub sha256:String,pub chunk_size:u64,pub chunks:Vec<Chunk>,
}
#[derive(Debug,Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub struct Receipt{
    pub source:String,pub destination:String,pub bytes:u64,pub sha256:String,
    pub parts:usize,pub manifest_sha256:String,pub stage:String,
    pub complete:bool,pub note:&'static str,
}
fn io(e:std::io::Error)->SplitError{SplitError::Io(e.to_string())}
fn sha(b:&[u8])->String{format!("{:x}",Sha256::digest(b))}
fn name_ok(n:&str)->bool{
    !n.is_empty()&&n.len()<=180&&n!="."&&n!=".."
    &&!n.ends_with('.')&&!n.ends_with(' ')&&!n.starts_with(".lu-")
    &&!n.chars().any(|c|c.is_control()||matches!(c,'/'|'\\'|':'|'?'|'*'|'<'|'>'|'|'|'"'))
}
fn archive_ok(n:&str)->bool{
    let v=n.to_ascii_lowercase();
    [".zip",".7z",".tar",".tar.gz",".tar.xz",".tar.zst",".tgz",
     ".rar",".iso",".deb",".rpm",".cpio",".gz",".xz",".bz2",".zst"]
        .iter().any(|e|v.ends_with(e))
}
fn token_ok(s:&str)->bool{
    (16..=80).contains(&s.len())&&s.bytes()
        .all(|b|b.is_ascii_alphanumeric()||b==b'-'||b==b'_')
}
fn hex(s:&str)->bool{s.len()==64&&s.bytes().all(|b|b.is_ascii_hexdigit())}
fn regular(dir:&Dir,name:&str)->Result<u64,SplitError>{
    let m=dir.symlink_metadata(name).map_err(|_|SplitError::Invalid)?;
    if !m.is_file()||m.file_type().is_symlink(){return Err(SplitError::Invalid);}
    let f=dir.open(name).map_err(io)?;
    if !f.metadata().map_err(io)?.is_file(){return Err(SplitError::Invalid);}
    Ok(m.len())
}
fn hash_file(dir:&Dir,name:&str,cap:u64)->Result<(u64,String),SplitError>{
    let mut file=dir.open(name).map_err(io)?;
    let mut hash=Sha256::new();let mut bytes=0u64;
    let mut buf=[0u8;64*1024];
    loop{
        let n=file.read(&mut buf).map_err(io)?;
        if n==0{break;}
        bytes=bytes.checked_add(n as u64).ok_or(SplitError::TooLarge)?;
        if bytes>cap{return Err(SplitError::TooLarge);}
        hash.update(&buf[..n]);
    }
    Ok((bytes,format!("{:x}",hash.finalize())))
}
fn exact(dir:&Dir,c:&Chunk)->Result<(),SplitError>{
    if regular(dir,&c.name).map_err(|_|SplitError::PartCorrupt)?!=c.bytes{
        return Err(SplitError::PartCorrupt);
    }
    if hash_file(dir,&c.name,c.bytes).map_err(|_|SplitError::PartCorrupt)?
        !=(c.bytes,c.sha256.clone()){
        return Err(SplitError::PartCorrupt);
    }
    Ok(())
}
fn json(m:&Plan)->Result<Vec<u8>,SplitError>{
    let mut bytes=serde_json::to_vec_pretty(m).map_err(|_|SplitError::Invalid)?;
    bytes.push(b'\n');
    if bytes.len()>64*1024{return Err(SplitError::Invalid);}
    Ok(bytes)
}
fn checked(m:&Plan)->Result<(),SplitError>{
    if m.version!=1||m.format!="lu-archive-byte-parts"
       ||!name_ok(&m.source)||!archive_ok(&m.source)
       ||m.bytes==0||m.bytes>MAX_BYTES||!hex(&m.sha256)
       ||!(MIN_PART..=MAX_PART).contains(&m.chunk_size)
       ||m.chunks.is_empty()||m.chunks.len()>MAX_PARTS
       ||m.chunks.len() as u64!=(m.bytes+m.chunk_size-1)/m.chunk_size{
        return Err(SplitError::Invalid);
    }
    let mut at=0u64;
    for (i,c) in m.chunks.iter().enumerate(){
        let expected=(m.bytes-at).min(m.chunk_size);
        if c.name!=format!("part-{:04}.lup",i+1)||c.offset!=at
            ||c.bytes!=expected||!hex(&c.sha256){
            return Err(SplitError::Invalid);
        }
        at+=expected;
    }
    if at!=m.bytes{return Err(SplitError::Invalid);}
    Ok(())
}
pub fn read_plan(dir:&Dir)->Result<Plan,SplitError>{
    if regular(dir,MANIFEST).map_err(|_|SplitError::Invalid)? >64*1024{
        return Err(SplitError::Invalid);
    }
    let mut bytes=Vec::new();
    dir.open(MANIFEST).map_err(io)?.take(64*1024+1)
        .read_to_end(&mut bytes).map_err(io)?;
    let m:Plan=serde_json::from_slice(&bytes).map_err(|_|SplitError::Invalid)?;
    checked(&m)?;
    if bytes!=json(&m)?{return Err(SplitError::Invalid);}
    Ok(m)
}
pub fn folder(name:&str)->Result<String,SplitError>{
    if !name_ok(name)||!archive_ok(name){return Err(SplitError::Invalid);}
    Ok(format!("{name}.lu-parts"))
}
/// Inspect original bytes without allocating the archive in RAM.
pub fn plan_split(root:&Dir,name:&str,chunk_size:u64)->Result<Plan,SplitError>{
    if !name_ok(name){return Err(SplitError::Invalid);}
    if !archive_ok(name){return Err(SplitError::Unsupported);}
    if !(MIN_PART..=MAX_PART).contains(&chunk_size){return Err(SplitError::Invalid);}
    let size=regular(root,name)?;
    if size==0||size>MAX_BYTES{return Err(SplitError::TooLarge);}
    let count=(size+chunk_size-1)/chunk_size;
    if count as usize>MAX_PARTS{return Err(SplitError::TooLarge);}
    let mut source=root.open(name).map_err(io)?;
    let mut total=Sha256::new();
    let mut chunks=Vec::new();
    let mut at=0u64;
    let mut buf=[0u8;64*1024];
    for i in 0..count{
        let bytes=(size-at).min(chunk_size);
        let mut left=bytes;let mut h=Sha256::new();
        while left>0{
            let need=left.min(buf.len() as u64) as usize;
            let n=source.read(&mut buf[..need]).map_err(io)?;
            if n==0{return Err(SplitError::Changed);}
            left-=n as u64;h.update(&buf[..n]);total.update(&buf[..n]);
        }
        chunks.push(Chunk{
            name:format!("part-{:04}.lup",i+1),offset:at,bytes,
            sha256:format!("{:x}",h.finalize()),
        });
        at+=bytes;
    }
    if source.read(&mut buf[..1]).map_err(io)?!=0{return Err(SplitError::Changed);}
    let m=Plan{
        version:1,format:"lu-archive-byte-parts".into(),source:name.into(),
        bytes:size,sha256:format!("{:x}",total.finalize()),chunk_size,chunks,
    };
    checked(&m)?;Ok(m)
}
fn stage(token:&str,prefix:&str)->Result<String,SplitError>{
    if !token_ok(token){return Err(SplitError::Invalid);}
    Ok(format!(".lu-p08-{prefix}-{token}"))
}
/// Explicit reapproval with SAME token may continue an interrupted stage:
/// the original is rehashed and every staged chunk is independently rehashed.
pub fn split(root:&Dir,m:&Plan,token:&str)->Result<Receipt,SplitError>{
    checked(m)?;
    if plan_split(root,&m.source,m.chunk_size)?!=*m{
        return Err(SplitError::Changed);
    }
    let out=folder(&m.source)?;
    if root.symlink_metadata(&out).is_ok(){return Err(SplitError::Exists);}
    let stage_name=stage(token,"split")?;
    match root.create_dir(&stage_name){
        Ok(())=>{},
        Err(e) if e.kind()==std::io::ErrorKind::AlreadyExists=>{},
        Err(e)=>return Err(io(e)),
    }
    let stage_meta=root.symlink_metadata(&stage_name)
        .map_err(|_|SplitError::StageConflict)?;
    if !stage_meta.is_dir()||stage_meta.file_type().is_symlink(){
        return Err(SplitError::StageConflict);
    }
    let staging=root.open_dir(&stage_name).map_err(|_|SplitError::StageConflict)?;
    let bytes=json(m)?;
    match staging.symlink_metadata(MANIFEST){
        Ok(_)=>if read_plan(&staging).map_err(|_|SplitError::StageConflict)?!=*m{
            return Err(SplitError::StageConflict);
        },
        Err(e) if e.kind()==std::io::ErrorKind::NotFound=>{
            let mut o=OpenOptions::new();o.create_new(true).write(true);
            let mut f=staging.open_with(MANIFEST,&o).map_err(io)?;
            f.write_all(&bytes).map_err(io)?;f.sync_all().map_err(io)?;
        },
        Err(e)=>return Err(io(e)),
    }
    let mut source=root.open(&m.source).map_err(io)?;
    let mut buf=[0u8;64*1024];
    for c in &m.chunks{
        let exists=match staging.symlink_metadata(&c.name){
            Ok(_)=>{exact(&staging,c)?;true},
            Err(e) if e.kind()==std::io::ErrorKind::NotFound=>false,
            Err(e)=>return Err(io(e)),
        };
        let mut target=if exists{None}else{
            let mut o=OpenOptions::new();o.create_new(true).write(true);
            Some(staging.open_with(&c.name,&o).map_err(io)?)
        };
        let mut left=c.bytes;let mut h=Sha256::new();
        while left>0{
            let need=left.min(buf.len() as u64) as usize;
            let n=source.read(&mut buf[..need]).map_err(io)?;
            if n==0{return Err(SplitError::Changed);}
            left-=n as u64;h.update(&buf[..n]);
            if let Some(t)=target.as_mut(){t.write_all(&buf[..n]).map_err(io)?;}
        }
        if format!("{:x}",h.finalize())!=c.sha256{return Err(SplitError::Changed);}
        if let Some(t)=target{t.sync_all().map_err(io)?;}
        exact(&staging,c)?;
    }
    if source.read(&mut buf[..1]).map_err(io)?!=0{return Err(SplitError::Changed);}
    if plan_split(root,&m.source,m.chunk_size)?!=*m{return Err(SplitError::Changed);}
    // New-directory admission forbids replacing an existing group. Publication
    // may become partially visible during interruption: manifest is linked LAST.
    // Such an incomplete folder is UNCERTAIN; it is not silently resumed.
    root.create_dir(&out).map_err(|e|if e.kind()==std::io::ErrorKind::AlreadyExists{
        SplitError::Exists
    }else{io(e)})?;
    let published=root.open_dir(&out).map_err(|_|SplitError::IncompletePublication)?;
    for c in &m.chunks{
        staging.hard_link(&c.name,&published,&c.name)
            .map_err(|_|SplitError::IncompletePublication)?;
        exact(&published,c).map_err(|_|SplitError::IncompletePublication)?;
    }
    staging.hard_link(MANIFEST,&published,MANIFEST)
        .map_err(|_|SplitError::IncompletePublication)?;
    if read_plan(&published).map_err(|_|SplitError::IncompletePublication)?!=*m{
        return Err(SplitError::IncompletePublication);
    }
    Ok(Receipt{
        source:m.source.clone(),destination:out,bytes:m.bytes,sha256:m.sha256.clone(),
        parts:m.chunks.len(),manifest_sha256:sha(&bytes),stage:stage_name,complete:true,
        note:"Lu byte-part container only; not a native spanned ZIP/RAR/7z archive.",
    })
}
pub fn inspect(root:&Dir,folder_name:&str)->Result<Plan,SplitError>{
    if !name_ok(folder_name)||!folder_name.ends_with(".lu-parts"){
        return Err(SplitError::Invalid);
    }
    let md=root.symlink_metadata(folder_name).map_err(|_|SplitError::Invalid)?;
    if !md.is_dir()||md.file_type().is_symlink(){return Err(SplitError::Invalid);}
    let dir=root.open_dir(folder_name).map_err(io)?;
    let m=read_plan(&dir)?;
    if folder(&m.source)?!=folder_name{return Err(SplitError::Invalid);}
    let mut total=Sha256::new();let mut buf=[0u8;64*1024];
    for c in &m.chunks{
        exact(&dir,c)?;
        let mut part=dir.open(&c.name).map_err(io)?;
        let mut left=c.bytes;
        while left>0{
            let need=left.min(buf.len() as u64) as usize;
            let n=part.read(&mut buf[..need]).map_err(io)?;
            if n==0{return Err(SplitError::PartCorrupt);}
            left-=n as u64;total.update(&buf[..n]);
        }
    }
    if format!("{:x}",total.finalize())!=m.sha256{
        return Err(SplitError::PartCorrupt);
    }
    Ok(m)
}
/// No-clobber reassembly: create-new staging file, verify every part and
/// whole archive digest, sync, hard-link destination, then remove staging.
/// An interrupted stage is retained for manual reconciliation.
pub fn reassemble(source_root:&Dir,destination_root:&Dir,folder_name:&str,token:&str)->Result<Receipt,SplitError>{
    let m=inspect(source_root,folder_name)?;
    let stage_name=stage(token,"join")?;
    if destination_root.symlink_metadata(&m.source).is_ok(){return Err(SplitError::Exists);}
    let dir=source_root.open_dir(folder_name).map_err(io)?;
    let mut opts=OpenOptions::new();opts.create_new(true).write(true);
    let mut out=destination_root.open_with(&stage_name,&opts)
        .map_err(|e|if e.kind()==std::io::ErrorKind::AlreadyExists{
            SplitError::StageConflict
        }else{io(e)})?;
    let mut h=Sha256::new();let mut buf=[0u8;64*1024];
    for c in &m.chunks{
        let mut input=dir.open(&c.name).map_err(io)?;
        let mut left=c.bytes;let mut piece=Sha256::new();
        while left>0{
            let need=left.min(buf.len() as u64) as usize;
            let n=input.read(&mut buf[..need]).map_err(io)?;
            if n==0{return Err(SplitError::PartCorrupt);}
            left-=n as u64;piece.update(&buf[..n]);h.update(&buf[..n]);
            out.write_all(&buf[..n]).map_err(io)?;
        }
        if format!("{:x}",piece.finalize())!=c.sha256{
            return Err(SplitError::PartCorrupt);
        }
    }
    if format!("{:x}",h.finalize())!=m.sha256{return Err(SplitError::PartCorrupt);}
    out.sync_all().map_err(io)?;drop(out);
    destination_root.hard_link(&stage_name,destination_root,&m.source)
        .map_err(|e|if e.kind()==std::io::ErrorKind::AlreadyExists{
            SplitError::Exists
        }else{SplitError::IncompletePublication})?;
    destination_root.remove_file(&stage_name).map_err(|_|SplitError::IncompletePublication)?;
    Ok(Receipt{
        source:folder_name.into(),destination:m.source.clone(),bytes:m.bytes,
        sha256:m.sha256.clone(),parts:m.chunks.len(),
        manifest_sha256:sha(&json(&m)?),stage:stage_name,complete:true,
        note:"Byte-exact archive reassembled; source multipart group left intact.",
    })
}

/// Read-only crash recovery: verify an output group against the exact approved
/// source plan. Partial publication is never renamed, resumed, or deleted here.
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum PublicationReview {
    Absent,
    Incomplete { verified_parts: usize, expected_parts: usize },
    Complete,
}
pub fn reconcile_publication(root:&Dir,m:&Plan)->Result<PublicationReview,SplitError>{
    checked(m)?;
    let out=folder(&m.source)?;
    let meta=match root.symlink_metadata(&out){
        Ok(v)=>v,
        Err(e) if e.kind()==std::io::ErrorKind::NotFound=>
            return Ok(PublicationReview::Absent),
        Err(e)=>return Err(io(e)),
    };
    if !meta.is_dir()||meta.file_type().is_symlink(){
        return Err(SplitError::StageConflict);
    }
    let published=root.open_dir(&out).map_err(io)?;
    let mut seen=std::collections::BTreeSet::new();
    for entry in published.read_dir(".").map_err(io)?{
        let entry=entry.map_err(io)?;
        let name=entry.file_name().into_string()
            .map_err(|_|SplitError::StageConflict)?;
        if name!=MANIFEST&&!m.chunks.iter().any(|c|c.name==name){
            return Err(SplitError::StageConflict);
        }
        if !seen.insert(name){return Err(SplitError::StageConflict);}
    }
    let mut count=0usize;
    for c in &m.chunks{
        if seen.contains(&c.name){exact(&published,c)?;count+=1;}
    }
    if seen.contains(MANIFEST){
        if read_plan(&published)?!=*m{return Err(SplitError::StageConflict);}
        if count!=m.chunks.len(){return Err(SplitError::IncompletePublication);}
        // A valid manifest and individually valid parts are not sufficient:
        // the reconstructed byte stream must match the original SHA-256.
        if inspect(root,&out)?!=*m{return Err(SplitError::PartCorrupt);}
        return Ok(PublicationReview::Complete);
    }
    Ok(PublicationReview::Incomplete{
        verified_parts:count,expected_parts:m.chunks.len()
    })
}
