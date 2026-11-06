//! WAVE2: bounded new-directory archive tree extraction. No existing folder
//! is overwritten. All entry payloads are validated before filesystem writes.
//! Non-atomic multi-file commit is journal-owned and leaves an INCOMPLETE
//! directory after interruption; it must be observed, never auto-replayed.
//! Reject all symlinks, devices, hardlinks, traversal, exotic ZIP methods,
//! archive-file names colliding with the Lu completion marker.
use std::{
    collections::{BTreeSet,BTreeMap},
    io::{Read,Write},
    sync::{Arc,atomic::{AtomicBool,Ordering}},
};
use cap_std::fs::{Dir,OpenOptions};
use sha2::{Digest,Sha256};
use zip::{ZipArchive,CompressionMethod};
use crate::native_linux_archive::{
    linux_archive_tree_members,LinuxArchiveError,
};
use crate::native_cpio::{cpio_tree_members,is_cpio_name,CpioError};
use crate::native_ar_deb::{ar_tree_members,is_ar_name,ArError};
use crate::native_seven_rar::{seven_rar_tree_members_cancellable,detect_7z_rar,SevenRarError};
use crate::native_rpm::{rpm_tree_members,RpmError};
const MAX_ARCHIVE_BYTES:u64=128*1024*1024;
const MAX_FILE:usize=64*1024*1024;
const MAX_TOTAL:usize=256*1024*1024;
const MAX_ENTRIES:usize=4096;
const MAX_DEPTH:usize=16;
const MARKER:&str=".lu-extract-complete";
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum TreeExtractError{
    InvalidName,InvalidArchive,UnsafeMember,DuplicateMember,UnsupportedMethod,
    TooLarge,TargetExists,ManifestChanged,MissingSource,
    Cancelled,PublishedNeedsReconciliation,Io(String),
}
#[derive(Debug,Clone,PartialEq,Eq,serde::Serialize,serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TreeManifestEntry{
    pub path:String,pub is_directory:bool,pub uncompressed_bytes:u64,
    pub sha256:String,
}
#[derive(Debug,Clone,PartialEq,Eq,serde::Serialize,serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TreeManifest {
    pub entries:Vec<TreeManifestEntry>,
    pub file_count:usize,
    pub total_bytes:u64,
    pub sha256:String,
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct TreeReceipt{
    pub destination_folder:String,pub files_extracted:usize,
    pub bytes_extracted:u64,pub manifest_sha256:String,
}
/// Host-owned cooperative cancellation signal. It conveys no write authority.
#[derive(Debug,Clone,Default)]
pub struct ArchiveCancellation{cancelled:Arc<AtomicBool>}
impl ArchiveCancellation{
    pub fn cancel(&self){self.cancelled.store(true,Ordering::Release)}
    pub fn is_cancelled(&self)->bool{self.cancelled.load(Ordering::Acquire)}
    fn checkpoint(&self)->Result<(),TreeExtractError>{
        if self.is_cancelled(){Err(TreeExtractError::Cancelled)}else{Ok(())}
    }
}
struct Node{path:String,is_dir:bool,bytes:Vec<u8>}
fn io(e:std::io::Error)->TreeExtractError{TreeExtractError::Io(e.to_string())}
fn safe_base(s:&str)->bool{
    !s.is_empty()&&s.len()<=200&&s!="."&&s!=".."
        &&!s.starts_with(".lu-stage-")&&!s.ends_with('.')&&!s.ends_with(' ')
        &&!s.chars().any(|c|c.is_control()||matches!(c,'\\'|'/'|':'))
}
fn safe_part(s:&str)->bool{
    !s.is_empty()&&s.len()<=1024&&!s.starts_with('/')
        &&!s.chars().any(|c|c.is_control()||matches!(c,'\\'|':'))
        &&s.trim_end_matches('/').split('/').count()<=MAX_DEPTH
        &&s.trim_end_matches('/').split('/').all(|p|
            !p.is_empty()&&p!="."&&p!=".."&&p.len()<=200
            &&!p.starts_with(".lu-")&&!p.starts_with('-')
            &&!p.ends_with('.')&&!p.ends_with(' ')
            &&!p.eq_ignore_ascii_case(".git"))
}
fn valid_hash(s:&str)->bool{
    s.len()==64&&s.bytes().all(|b|b.is_ascii_digit()||(b'a'..=b'f').contains(&b))
}
fn manifest_digest(entries:&[TreeManifestEntry])->String{
    let mut hasher=Sha256::new();
    hasher.update(b"LU-ARCHIVE-TREE-MANIFEST-v1");
    for entry in entries{
        for field in [
            entry.path.as_bytes(),
            if entry.is_directory{b"D".as_slice()}else{b"F".as_slice()},
            entry.sha256.as_bytes(),
        ]{
            hasher.update((field.len() as u64).to_le_bytes());
            hasher.update(field);
        }
        hasher.update(entry.uncompressed_bytes.to_le_bytes());
    }
    format!("{:x}",hasher.finalize())
}
/// Validates a persisted approved snapshot without opening any filesystem
/// object. A digest or completion-marker string alone cannot certify a tree.
pub fn verify_tree_manifest(m:&TreeManifest)->bool{
    if m.entries.is_empty()||m.entries.len()>MAX_ENTRIES||!valid_hash(&m.sha256)
        ||m.file_count>MAX_ENTRIES||m.total_bytes>MAX_TOTAL as u64 {
        return false
    }
    let mut names=BTreeSet::new();
    let mut types=BTreeMap::<String,bool>::new();
    let mut count=0usize;
    let mut total=0u64;
    let mut prior:Option<&str>=None;
    for e in &m.entries{
        if !safe_part(&e.path)||prior.is_some_and(|p|p>=e.path.as_str())
            ||!names.insert(e.path.to_lowercase()) {
            return false
        }
        if e.is_directory{
            if e.uncompressed_bytes!=0||!e.sha256.is_empty(){return false}
        }else{
            if !valid_hash(&e.sha256)||e.uncompressed_bytes>MAX_FILE as u64{
                return false
            }
            count+=1;
            let Some(sum)=total.checked_add(e.uncompressed_bytes)else{return false};
            total=sum;
        }
        types.insert(e.path.to_lowercase(),e.is_directory);
        prior=Some(&e.path);
    }
    for e in &m.entries{
        let mut prefix=String::new();
        let components:Vec<_>=e.path.split('/').collect();
        for part in &components[..components.len().saturating_sub(1)]{
            if !prefix.is_empty(){prefix.push('/')}
            prefix.push_str(part);
            if types.get(&prefix.to_lowercase())==Some(&false){return false}
        }
    }
    count==m.file_count&&total==m.total_bytes&&
        manifest_digest(&m.entries)==m.sha256
}
fn package_zip(root:&Dir,name:&str,cancel:&ArchiveCancellation)->Result<Vec<Node>,TreeExtractError>{
    cancel.checkpoint()?;
    if !safe_base(name)||!name.to_ascii_lowercase().ends_with(".zip"){
        return Err(TreeExtractError::InvalidName)
    }
    let stat=root.symlink_metadata(name)
        .map_err(|_|TreeExtractError::MissingSource)?;
    if !stat.is_file()||stat.file_type().is_symlink()||stat.len()>MAX_ARCHIVE_BYTES{
        return Err(TreeExtractError::InvalidArchive)
    }
    let file=root.open(name).map_err(io)?;
    #[cfg(windows)]
    crate::windows_file_guard::reject_opened_file_reparse(&file)
        .map_err(|_|TreeExtractError::InvalidArchive)?;
    let mut zip=ZipArchive::new(file).map_err(|_|TreeExtractError::InvalidArchive)?;
    if zip.len()>MAX_ENTRIES||zip.has_overlapping_entries()
        .map_err(|_|TreeExtractError::InvalidArchive)?
        ||zip.validate_header_consistency().is_err(){
        return Err(TreeExtractError::InvalidArchive)
    }
    let mut result=Vec::with_capacity(zip.len());
    let mut total=0usize;
    for index in 0..zip.len(){
        cancel.checkpoint()?;
        let mut entry=zip.by_index(index).map_err(|_|TreeExtractError::InvalidArchive)?;
        let raw=entry.name().map_err(|_|TreeExtractError::InvalidArchive)?
            .trim_end_matches('/').to_owned();
        if !safe_part(&raw)||entry.is_symlink()||entry.encrypted(){
            return Err(TreeExtractError::UnsafeMember)
        }
        let mode=entry.unix_mode().unwrap_or(0)&0o170000;
        let directory=entry.is_dir();
        if mode!=0&&mode!=0o100000&&mode!=0o040000 ||
            directory&&mode==0o100000 ||!directory&&mode==0o040000 {
            return Err(TreeExtractError::UnsafeMember)
        }
        if !matches!(entry.compression(),CompressionMethod::Stored|CompressionMethod::Deflated){
            return Err(TreeExtractError::UnsupportedMethod)
        }
        if entry.size() as usize>MAX_FILE
            ||(entry.compressed_size()==0&&entry.size()>0)
            ||(entry.compressed_size()>0&&entry.size()/entry.compressed_size()>1000){
            return Err(TreeExtractError::TooLarge)
        }
        let mut bytes=Vec::new();
        if !directory{
            let mut stream=(&mut entry).take(MAX_FILE as u64+1);
            let mut chunk=[0u8;64*1024];
            loop{
                cancel.checkpoint()?;
                let n=stream.read(&mut chunk)
                    .map_err(|_|TreeExtractError::InvalidArchive)?;
                if n==0{break}
                bytes.extend_from_slice(&chunk[..n]);
                if bytes.len()>MAX_FILE{return Err(TreeExtractError::TooLarge)}
            }
            if bytes.len() as u64!=entry.size(){
                return Err(TreeExtractError::InvalidArchive)
            }
            let mut crc=crc32fast::Hasher::new();
            crc.update(&bytes);
            if crc.finalize()!=entry.crc32(){
                return Err(TreeExtractError::InvalidArchive)
            }
        }else if entry.size()>0{return Err(TreeExtractError::UnsafeMember)}
        total=total.checked_add(bytes.len()).ok_or(TreeExtractError::TooLarge)?;
        if total>MAX_TOTAL{return Err(TreeExtractError::TooLarge)}
        result.push(Node{path:raw,is_dir:directory,bytes});
    }
    Ok(result)
}
fn package_linux(root:&Dir,name:&str,cancel:&ArchiveCancellation)->Result<Vec<Node>,TreeExtractError>{
    cancel.checkpoint()?;
    if name.to_ascii_lowercase().ends_with(".rpm"){
        return rpm_tree_members(root,name).map_err(|e|match e{
            RpmError::TooLarge=>TreeExtractError::TooLarge,
            RpmError::UnsafePayload|RpmError::UnsafeSource=>TreeExtractError::UnsafeMember,
            _=>TreeExtractError::InvalidArchive,
        }).map(|items|items.into_iter().map(|(path,is_dir,bytes)|
            Node{path,is_dir,bytes}).collect());
    }
    if detect_7z_rar(name).is_some(){
        return seven_rar_tree_members_cancellable(root,name,&||cancel.is_cancelled()).map_err(|e|match e{
            SevenRarError::Cancelled=>TreeExtractError::Cancelled,
            SevenRarError::TooLarge=>TreeExtractError::TooLarge,
            SevenRarError::UnsafeArchive|SevenRarError::EncryptedOrUnsupported=>
                TreeExtractError::UnsafeMember,
            _=>TreeExtractError::InvalidArchive,
        }).map(|items|items.into_iter().map(|(path,is_dir,bytes)|
            Node{path,is_dir,bytes}).collect());
    }
    if is_cpio_name(name){
        return cpio_tree_members(root,name).map_err(|e|match e{
            CpioError::TooLarge=>TreeExtractError::TooLarge,
            CpioError::UnsupportedEntry|CpioError::UnsafeEntry|
            CpioError::DuplicateEntry=>TreeExtractError::UnsafeMember,
            _=>TreeExtractError::InvalidArchive,
        }).map(|items|items.into_iter().map(|(path,is_dir,bytes)|
            Node{path,is_dir,bytes}).collect());
    }
    if is_ar_name(name){
        return ar_tree_members(root,name).map_err(|e|match e{
            ArError::TooLarge=>TreeExtractError::TooLarge,
            ArError::UnsafeName|ArError::DuplicateName=>TreeExtractError::UnsafeMember,
            _=>TreeExtractError::InvalidArchive,
        }).map(|items|items.into_iter().map(|(path,is_dir,bytes)|
            Node{path,is_dir,bytes}).collect());
    }
    linux_archive_tree_members(root,name).map_err(|e|match e{
        LinuxArchiveError::TooLarge=>TreeExtractError::TooLarge,
        LinuxArchiveError::UnsafeEntry|LinuxArchiveError::UnsupportedEntry|
        LinuxArchiveError::DuplicateEntry=>TreeExtractError::UnsafeMember,
        _=>TreeExtractError::InvalidArchive,
    }).map(|items|items.into_iter().map(|(path,is_dir,bytes)|
        Node{path,is_dir,bytes}).collect())
}
fn manifest_and_nodes(root:&Dir,name:&str,cancel:&ArchiveCancellation)->Result<(TreeManifest,Vec<Node>),TreeExtractError>{
    cancel.checkpoint()?;
    let items=if name.to_ascii_lowercase().ends_with(".zip"){
        package_zip(root,name,cancel)?
    }else{
        package_linux(root,name,cancel)?
    };
    cancel.checkpoint()?;
    if items.is_empty()||items.len()>MAX_ENTRIES{return Err(TreeExtractError::InvalidArchive)}
    let mut names=BTreeSet::new();
    let mut types=BTreeMap::new();
    let mut count=0usize;
    let mut total=0u64;
    let mut entries=Vec::new();
    for node in &items{
        cancel.checkpoint()?;
        if !safe_part(&node.path)||!names.insert(node.path.to_lowercase()){
            return Err(TreeExtractError::DuplicateMember)
        }
        if !node.is_dir{
            count+=1;
            total=total.checked_add(node.bytes.len() as u64)
                .ok_or(TreeExtractError::TooLarge)?;
        }
        types.insert(node.path.to_lowercase(),node.is_dir);
        entries.push(TreeManifestEntry{
            path:node.path.clone(),is_directory:node.is_dir,
            uncompressed_bytes:node.bytes.len() as u64,
            sha256:if node.is_dir{String::new()}else{format!("{:x}",Sha256::digest(&node.bytes))},
        });
    }
    if total>MAX_TOTAL as u64{return Err(TreeExtractError::TooLarge)}
    for node in &items{
        let mut prefix=String::new();
        let parts:Vec<_>=node.path.split('/').collect();
        for component in &parts[..parts.len().saturating_sub(1)]{
            if !prefix.is_empty(){prefix.push('/')}
            prefix.push_str(component);
            if types.get(&prefix.to_lowercase())==Some(&false){
                return Err(TreeExtractError::UnsafeMember)
            }
        }
    }
    // Domain-separated canonical manifest digest of the entire exact tree.
    entries.sort_by(|a,b|a.path.cmp(&b.path));
    let digest=manifest_digest(&entries);
    Ok((TreeManifest{
        entries,file_count:count,total_bytes:total,sha256:digest,
    },items))
}
/// Read-only full-data inspection with actual CRC32/expanded data SHA hashes.
pub fn preview_archive_tree(
    root:&Dir,source_name:&str
)->Result<TreeManifest,TreeExtractError>{
    manifest_and_nodes(root,source_name,&ArchiveCancellation::default())
        .map(|(manifest,_)|manifest)
}
fn open_or_create_child(current:&Dir,name:&str)->Result<Dir,TreeExtractError>{
    if !safe_base(name){return Err(TreeExtractError::UnsafeMember)}
    match current.create_dir(name){
        Ok(())=>{},
        Err(e) if e.kind()==std::io::ErrorKind::AlreadyExists=>{},
        Err(e)=>return Err(io(e)),
    }
    let meta=current.symlink_metadata(name).map_err(io)?;
    if !meta.is_dir()||meta.file_type().is_symlink(){
        return Err(TreeExtractError::UnsafeMember)
    }
    let child=current.open_dir(name).map_err(io)?;
    #[cfg(windows)]
    crate::windows_file_guard::reject_opened_directory_reparse(&child)
        .map_err(|_|TreeExtractError::UnsafeMember)?;
    Ok(child)
}
fn publish_node(root:&Dir,entry:&Node,cancel:&ArchiveCancellation)->Result<(),TreeExtractError>{
    cancel.checkpoint()?;
    let parts:Vec<&str>=entry.path.split('/').collect();
    let mut current=root.try_clone().map_err(io)?;
    for component in &parts[..parts.len()-1]{
        current=open_or_create_child(&current,component)?;
    }
    let name=parts[parts.len()-1];
    if entry.is_dir{
        let _=open_or_create_child(&current,name)?;
    }else{
        let mut opts=OpenOptions::new();
        opts.create_new(true).write(true);
        let mut f=current.open_with(name,&opts).map_err(io)?;
        for chunk in entry.bytes.chunks(64*1024){
            cancel.checkpoint()?;
            f.write_all(chunk).map_err(io)?;
        }
        cancel.checkpoint()?;
        f.sync_all().map_err(io)?;
    }
    Ok(())
}
/// Strict no-clobber new-directory publication. If any write fails AFTER
/// destination folder creation, return UNCERTAIN and retain partial contents.
pub fn extract_archive_tree_new(
    source:&Dir,source_name:&str,destination_parent:&Dir,new_folder:&str,
    approved_sha256:&str,
)->Result<TreeReceipt,TreeExtractError>{
    extract_archive_tree_new_cancellable(
        source,source_name,destination_parent,new_folder,approved_sha256,
        &ArchiveCancellation::default()
    )
}
/// A cancellation observed before destination creation is clean. Once the
/// folder exists, ANY cancellation is WRITE_UNCERTAIN: never erase unknown
/// partial contents or auto-retry the same journaled effect.
pub fn extract_archive_tree_new_cancellable(
    source:&Dir,source_name:&str,destination_parent:&Dir,new_folder:&str,
    approved_sha256:&str,cancel:&ArchiveCancellation,
)->Result<TreeReceipt,TreeExtractError>{
    cancel.checkpoint()?;
    if !safe_base(new_folder)||!valid_hash(approved_sha256){
        return Err(TreeExtractError::InvalidName)
    }
    let (manifest,mut nodes)=manifest_and_nodes(source,source_name,cancel)?;
    if manifest.sha256!=approved_sha256{return Err(TreeExtractError::ManifestChanged)}
    cancel.checkpoint()?;
    let root=destination_parent.create_dir(new_folder).map_err(|e|
        if e.kind()==std::io::ErrorKind::AlreadyExists{
            TreeExtractError::TargetExists
        }else{io(e)}
    );
    root?;
    let result=(||->Result<(),TreeExtractError>{
        let folder=destination_parent.open_dir(new_folder).map_err(io)?;
        #[cfg(windows)]
        crate::windows_file_guard::reject_opened_directory_reparse(&folder)
            .map_err(|_|TreeExtractError::UnsafeMember)?;
        // Parent-first directories; files after full-tree preflight.
        nodes.sort_by_key(|n|(if n.is_dir{0}else{1},n.path.matches('/').count()));
        for node in &nodes{publish_node(&folder,node,cancel)?}
        // The completion marker is the final write. After it is synced,
        // cancellation is too late to revoke an already-published result.
        cancel.checkpoint()?;
        let mut opts=OpenOptions::new();
        opts.create_new(true).write(true);
        let mut marker=folder.open_with(MARKER,&opts).map_err(io)?;
        marker.write_all(manifest.sha256.as_bytes()).map_err(io)?;
        marker.sync_all().map_err(io)?;
        Ok(())
    })();
    if result.is_err(){return Err(TreeExtractError::PublishedNeedsReconciliation)}
    Ok(TreeReceipt{
        destination_folder:new_folder.into(),
        files_extracted:manifest.file_count,
        bytes_extracted:manifest.total_bytes,
        manifest_sha256:manifest.sha256,
    })
}
/// Read-only post-crash reconciliation. The trusted host must keep the
/// approved manifest in its durable journal. A marker alone is NEVER proof:
/// verify every file's digest, every directory, and reject extra entries.
pub fn observe_tree_completion(
    parent:&Dir,folder_name:&str,approved:&TreeManifest,
)->Result<bool,TreeExtractError>{
    if !safe_base(folder_name)||!verify_tree_manifest(approved){
        return Err(TreeExtractError::InvalidName)
    }
    // Interruption before directory creation, or before the last durable marker,
    // is incomplete—not a successful or automatically replayable extraction.
    // Preserve all non-NotFound failures for manual reconciliation.
    let source_meta=match parent.symlink_metadata(folder_name){
        Ok(m)=>m,
        Err(e) if e.kind()==std::io::ErrorKind::NotFound=>return Ok(false),
        Err(e)=>return Err(io(e)),
    };
    if !source_meta.is_dir()||source_meta.file_type().is_symlink(){
        return Ok(false)
    }
    let folder=match parent.open_dir(folder_name){
        Ok(d)=>d,
        Err(e) if e.kind()==std::io::ErrorKind::NotFound=>return Ok(false),
        Err(e)=>return Err(io(e)),
    };
    #[cfg(windows)]
    if crate::windows_file_guard::reject_opened_directory_reparse(&folder).is_err(){
        return Ok(false)
    }
    let marker_meta=match folder.symlink_metadata(MARKER){
        Ok(m)=>m,
        Err(e) if e.kind()==std::io::ErrorKind::NotFound=>return Ok(false),
        Err(e)=>return Err(io(e)),
    };
    if !marker_meta.is_file()||marker_meta.file_type().is_symlink()
        ||marker_meta.len()!=64 {return Ok(false)}
    let mut marker=String::new();
    let f=match folder.open(MARKER){
        Ok(file)=>file,
        Err(e) if e.kind()==std::io::ErrorKind::NotFound=>return Ok(false),
        Err(e)=>return Err(io(e)),
    };
    #[cfg(windows)]
    if crate::windows_file_guard::reject_opened_file_reparse(&f).is_err(){
        return Ok(false)
    }
    f.take(65).read_to_string(&mut marker).map_err(io)?;
    if marker!=approved.sha256{return Ok(false)}
    let mut expected=BTreeMap::<String,Option<&TreeManifestEntry>>::new();
    for entry in &approved.entries{
        if !safe_part(&entry.path){return Err(TreeExtractError::UnsafeMember)}
        let components:Vec<_>=entry.path.split('/').collect();
        let mut prefix=String::new();
        for (i,component) in components.iter().enumerate(){
            if i>0{prefix.push('/')}
            prefix.push_str(component);
            let is_last=i+1==components.len();
            let value=if is_last{Some(entry)}else{None};
            expected.entry(prefix.to_lowercase()).or_insert(value);
        }
    }
    let mut observed=BTreeSet::new();
    let mut total=0u64;
    let result=verify_existing_directory(
        &folder,"",0,&expected,&mut observed,&mut total
    )?;
    Ok(result && observed.len()==expected.len() &&
        total==approved.total_bytes)
}
fn verify_existing_directory(
    root:&Dir,prefix:&str,depth:usize,
    expected:&BTreeMap<String,Option<&TreeManifestEntry>>,
    observed:&mut BTreeSet<String>,total:&mut u64,
)->Result<bool,TreeExtractError>{
    if depth>MAX_DEPTH{return Err(TreeExtractError::TooLarge)}
    for entry in root.read_dir(".").map_err(io)?{
        let entry=entry.map_err(io)?;
        let name=entry.file_name();
        let Some(name)=name.to_str() else {return Ok(false)};
        if prefix.is_empty()&&name==MARKER{continue}
        if !safe_base(name){return Ok(false)}
        let path=if prefix.is_empty(){name.to_owned()}else{
            format!("{prefix}/{name}")
        };
        let key=path.to_lowercase();
        let Some(spec)=expected.get(&key) else {return Ok(false)};
        if !observed.insert(key){return Ok(false)}
        let meta=root.symlink_metadata(name).map_err(io)?;
        if meta.file_type().is_symlink(){return Ok(false)}
        if meta.is_dir(){
            if spec.is_some_and(|s|!s.is_directory){return Ok(false)}
            let child=root.open_dir(name).map_err(io)?;
            #[cfg(windows)]
            if crate::windows_file_guard::reject_opened_directory_reparse(&child).is_err(){
                return Ok(false)
            }
            if !verify_existing_directory(
                &child,&path,depth+1,expected,observed,total
            )?{return Ok(false)}
        }else if meta.is_file(){
            let Some(spec)=spec else{return Ok(false)};
            if spec.is_directory||meta.len()!=spec.uncompressed_bytes ||
                meta.len()>MAX_FILE as u64 {return Ok(false)}
            let mut file=root.open(name).map_err(io)?;
            #[cfg(windows)]
            if crate::windows_file_guard::reject_opened_file_reparse(&file).is_err(){
                return Ok(false)
            }
            let mut hasher=Sha256::new();
            let mut count=0u64;
            let mut buffer=[0u8;64*1024];
            loop{
                let n=file.read(&mut buffer).map_err(io)?;
                if n==0{break}
                count=count.checked_add(n as u64).ok_or(TreeExtractError::TooLarge)?;
                if count>MAX_FILE as u64{return Ok(false)}
                hasher.update(&buffer[..n]);
            }
            if count!=spec.uncompressed_bytes ||
                format!("{:x}",hasher.finalize())!=spec.sha256 {
                return Ok(false)
            }
            *total=total.checked_add(count).ok_or(TreeExtractError::TooLarge)?;
            if *total>MAX_TOTAL as u64{return Ok(false)}
        }else{return Ok(false)}
    }
    Ok(true)
}
