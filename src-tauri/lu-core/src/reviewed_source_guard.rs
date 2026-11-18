//! E1-P13: host-only resource-revision custody for reviewed Git/FFprobe.
//!
//! Never infer a successful operation from a pathname, child exit code,
//! FinalAnswer or EffectState::Applied. Snapshot an independently observable
//! source BEFORE launching the supervised external worker and compare it
//! AFTER the worker completes and its own receipt has been parsed.
//!
//! For media, pin an open file handle for the whole worker lifetime, enforce
//! the originally approved SHA-256, and compare both pinned and freshly
//! resolved pathname content/metadata after the worker. Windows share mode
//! allows readers while excluding newly opened writers/renames; a preexisting
//! writable handle remains a theoretical race, so the digest and timestamps
//! must still be checked. For ordinary Unix files, dev+ino detects path
//! replacements while the pinned descriptor refers to the old inode.
//!
//! For Git, separately sample the approved .git directory identity and
//! independently parsed operation-specific content revision. Ordinary status
//! revisions include EVERY tracked file content digest (not just count).
//! This is a before/after observation, NOT a filesystem snapshot/transaction;
//! adversarial mutations and reversions between observations require stronger
//! OS-level openat2/handle or repository-lock semantics in a later gate.
use std::{
    fs::{self,File,Metadata,OpenOptions},
    io::{Read,Seek,SeekFrom},
    path::{Path,PathBuf},
    time::SystemTime,
};
use sha2::{Digest,Sha256};
use crate::{
    native_app_typed_git::GitReadOperation,
    reviewed_outcome_oracle::git_source_revision,
};
const MAX_MEDIA_BYTES:u64=256*1024*1024;
const CHUNK:usize=64*1024;

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum RevisionObservation{Stable,Changed,Unsupported}

/// Never serialize guards, handles, original names, digests or identities
/// into model results, WebView activity, or the immutable effect receipt.
pub enum SourceRevisionGuard{
    Git(GitRevisionGuard),
    Media(MediaRevisionGuard),
    Unsupported,
}
impl SourceRevisionGuard{
    pub fn git(root:&Path,operation:GitReadOperation)->Self{
        match GitRevisionGuard::capture(root,operation){
            Some(guard)=>Self::Git(guard),
            None=>Self::Unsupported,
        }
    }
    pub fn media(path:&Path,expected_sha:&str)->Self{
        match MediaRevisionGuard::capture(path,expected_sha){
            Some(guard)=>Self::Media(guard),
            None=>Self::Unsupported,
        }
    }
    pub fn recheck(&mut self)->RevisionObservation{
        match self{
            Self::Git(guard)=>guard.recheck(),
            Self::Media(guard)=>guard.recheck(),
            Self::Unsupported=>RevisionObservation::Unsupported,
        }
    }
}
#[derive(Debug,Clone)]
struct Identity{
    len:u64,
    modified:Option<SystemTime>,
    created:Option<SystemTime>,
    #[cfg(unix)]
    device:u64,
    #[cfg(unix)]
    inode:u64,
}
fn identity(meta:&Metadata)->Identity{
    #[cfg(unix)]
    use std::os::unix::fs::MetadataExt;
    Identity{
        len:meta.len(),
        modified:meta.modified().ok(),
        created:meta.created().ok(),
        #[cfg(unix)]
        device:meta.dev(),
        #[cfg(unix)]
        inode:meta.ino(),
    }
}
fn equal_identity(a:&Identity,b:&Identity,content_file:bool)->bool{
    if a.len!=b.len || a.modified!=b.modified{
        return false
    }
    #[cfg(unix)]
    if a.device!=b.device||a.inode!=b.inode{return false}
    #[cfg(windows)]
    if a.created!=b.created{return false}
    let _=content_file;
    true
}
fn no_links(path:&Path)->Option<Metadata>{
    for ancestor in path.ancestors(){
        let meta=fs::symlink_metadata(ancestor).ok()?;
        if meta.file_type().is_symlink(){return None}
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if meta.file_attributes()&0x400!=0{return None}
        }
    }
    fs::symlink_metadata(path).ok()
}
fn digest_file(file:&mut File)->Option<String>{
    file.seek(SeekFrom::Start(0)).ok()?;
    let start=file.metadata().ok()?;
    if !start.is_file()||start.len()>MAX_MEDIA_BYTES{return None}
    let mut digest=Sha256::new();
    let mut seen=0u64;
    let mut buffer=[0u8;CHUNK];
    loop{
        let length=file.read(&mut buffer).ok()?;
        if length==0{break}
        seen=seen.checked_add(length as u64)?;
        if seen>MAX_MEDIA_BYTES{return None}
        digest.update(&buffer[..length]);
    }
    let finish=file.metadata().ok()?;
    if seen!=start.len()||!equal_identity(
        &identity(&start),&identity(&finish),true
    ){return None}
    Some(format!("{:x}",digest.finalize()))
}
fn open_pinned_read(path:&Path)->Option<File>{
    #[allow(unused_mut)]
    let mut options=OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // FILE_SHARE_READ only: the supervised FFprobe must still open
        // the approved resource as a reader. Deny new write/delete opens
        // while Lu holds this handle, including rename/replacement.
        options.share_mode(0x0000_0001);
    }
    options.open(path).ok()
}
pub struct MediaRevisionGuard{
    path:PathBuf,
    pinned:File,
    before:Identity,
    approved_digest:String,
}
impl MediaRevisionGuard{
    fn capture(path:&Path,expected_sha:&str)->Option<Self>{
        if expected_sha.len()!=64||!expected_sha.bytes().all(|c|
            c.is_ascii_digit()||(b'a'..=b'f').contains(&c)){
            return None
        }
        let meta=no_links(path)?;
        if !meta.is_file()||meta.len()>MAX_MEDIA_BYTES{return None}
        let mut pinned=open_pinned_read(path)?;
        let before=identity(&pinned.metadata().ok()?);
        if !equal_identity(&before,&identity(&meta),true){return None}
        if digest_file(&mut pinned)?!=expected_sha{return None}
        Some(Self{
            path:path.to_path_buf(),pinned,before,
            approved_digest:expected_sha.to_string(),
        })
    }
    fn recheck(&mut self)->RevisionObservation{
        let mut check=||->Option<()>{
            let meta=no_links(&self.path)?;
            if !meta.is_file()||!equal_identity(
                &self.before,&identity(&meta),true){return None}
            if !equal_identity(&self.before,&identity(
                &self.pinned.metadata().ok()?
            ),true){return None}
            if digest_file(&mut self.pinned)?!=self.approved_digest{
                return None
            }
            // The pathname may have been rebound to a different file on
            // platforms that permit rename despite a pinned descriptor.
            let mut fresh=open_pinned_read(&self.path)?;
            if !equal_identity(&self.before,&identity(
                &fresh.metadata().ok()?
            ),true){return None}
            if digest_file(&mut fresh)?!=self.approved_digest{return None}
            Some(())
        };
        if check().is_some(){
            RevisionObservation::Stable
        }else{RevisionObservation::Changed}
    }
}
pub struct GitRevisionGuard{
    root:PathBuf,
    operation:GitReadOperation,
    root_identity:Identity,
    git_identity:Identity,
    _root_pin:Option<File>,
    _git_pin:Option<File>,
    revision:String,
}
impl GitRevisionGuard{
    fn capture(root:&Path,operation:GitReadOperation)->Option<Self>{
        let git=root.join(".git");
        let root_meta=no_links(root)?;
        let git_meta=no_links(&git)?;
        if !root_meta.is_dir()||!git_meta.is_dir(){return None}
        let revision=git_source_revision(root,operation)?;
        #[cfg(unix)]
        let (root_pin,git_pin)=(Some(File::open(root).ok()?),
            Some(File::open(&git).ok()?));
        #[cfg(not(unix))]
        let (root_pin,git_pin)=(None,None);
        Some(Self{
            root:root.to_path_buf(),operation,
            root_identity:identity(&root_meta),
            git_identity:identity(&git_meta),
            _root_pin:root_pin,
            _git_pin:git_pin,
            revision,
        })
    }
    fn recheck(&mut self)->RevisionObservation{
        let git=self.root.join(".git");
        let new_root=no_links(&self.root);
        let new_git=no_links(&git);
        let same=match (new_root,new_git){
            (Some(r),Some(g)) if r.is_dir()&&g.is_dir()=>
                equal_identity(&self.root_identity,&identity(&r),false)
                    &&equal_identity(&self.git_identity,&identity(&g),false),
            _=>false
        };
        if !same{return RevisionObservation::Changed}
        if git_source_revision(&self.root,self.operation).as_deref()
            ==Some(self.revision.as_str()){
            RevisionObservation::Stable
        }else{RevisionObservation::Changed}
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use std::time::{SystemTime,UNIX_EPOCH};
    fn root()->PathBuf{
        let dir=std::env::temp_dir().join(format!(
            "lu-e1-p13-custody-{}-{}",
            std::process::id(),SystemTime::now()
                .duration_since(UNIX_EPOCH).expect("clock").as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();dir
    }
    #[test]
    #[cfg(unix)]
    fn pinned_media_detects_in_place_mutation_without_changing_length(){
        let root=root();let path=root.join("approved.wav");
        fs::write(&path,b"first-content").unwrap();
        let digest=format!("{:x}",Sha256::digest(b"first-content"));
        let mut guard=SourceRevisionGuard::media(&path,&digest);
        assert_eq!(guard.recheck(),RevisionObservation::Stable);
        fs::write(&path,b"other-content").unwrap();
        assert_eq!(guard.recheck(),RevisionObservation::Changed);
        drop(guard);fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn pinned_approved_file_is_stable_when_unchanged(){
        let root=root();let path=root.join("approved.mp4");
        fs::write(&path,b"approved-sample").unwrap();
        let digest=format!("{:x}",Sha256::digest(b"approved-sample"));
        let mut pinned=SourceRevisionGuard::media(&path,&digest);
        assert!(!matches!(&pinned,SourceRevisionGuard::Unsupported));
        assert_eq!(pinned.recheck(),RevisionObservation::Stable);
        drop(pinned);fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn refusal_when_approved_media_digest_does_not_match(){
        let root=root();let path=root.join("approved.mp4");
        fs::write(&path,b"original").unwrap();
        assert!(matches!(SourceRevisionGuard::media(&path,&"0".repeat(64)),
            SourceRevisionGuard::Unsupported));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn approved_git_branch_revision_change_is_not_stable(){
        let root=root();fs::create_dir_all(root.join(".git")).unwrap();
        fs::write(root.join(".git/HEAD"),"ref: refs/heads/main\n").unwrap();
        let mut guard=SourceRevisionGuard::git(
            &root,GitReadOperation::CurrentBranch
        );
        assert_eq!(guard.recheck(),RevisionObservation::Stable);
        fs::write(root.join(".git/HEAD"),"ref: refs/heads/dev\n").unwrap();
        assert_eq!(guard.recheck(),RevisionObservation::Changed);
        drop(guard);fs::remove_dir_all(root).unwrap();
    }
    #[test]
    #[cfg(windows)]
    fn pinned_windows_media_handle_denies_new_writers_while_allowing_readers(){
        let root=root();let path=root.join("approved.mp4");
        fs::write(&path,b"unchanged-source").unwrap();
        let sha=format!("{:x}",Sha256::digest(b"unchanged-source"));
        let mut guard=SourceRevisionGuard::media(&path,&sha);
        assert!(!matches!(&guard,SourceRevisionGuard::Unsupported));
        assert!(OpenOptions::new().write(true).open(&path).is_err(),
            "Windows pinned reader must deny competing write opens");
        assert_eq!(guard.recheck(),RevisionObservation::Stable);
        drop(guard);fs::remove_dir_all(root).unwrap();
    }
    #[test]
    #[cfg(unix)]
    fn linked_media_path_is_not_captured_as_approved_file(){
        use std::os::unix::fs::symlink;
        let root=root();let original=root.join("original.mp4");
        let alias=root.join("alias.mp4");
        fs::write(&original,b"same").unwrap();
        symlink(&original,&alias).unwrap();
        let sha=format!("{:x}",Sha256::digest(b"same"));
        assert!(matches!(SourceRevisionGuard::media(&alias,&sha),
            SourceRevisionGuard::Unsupported));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    #[cfg(unix)]
    fn pinned_file_detects_same_length_path_replacement(){
        let root=root();let path=root.join("approved.mp4");
        let replacement=root.join("replacement.mp4");
        fs::write(&path,b"valid-payload").unwrap();
        fs::write(&replacement,b"valid-payload").unwrap();
        let sha=format!("{:x}",Sha256::digest(b"valid-payload"));
        let mut guard=SourceRevisionGuard::media(&path,&sha);
        assert_eq!(guard.recheck(),RevisionObservation::Stable);
        fs::rename(&replacement,&path).unwrap();
        assert_eq!(guard.recheck(),RevisionObservation::Changed);
        drop(guard);fs::remove_dir_all(root).unwrap();
    }
}
