//! HF P05 transfer **local integrity/publication** fence.
//! This is not the HTTPS downloader. A transport MUST be independently
//! identity-pinned, bounded, HTTPS-only, cancellable and supervised before
//! passing its byte stream here. No model/renderer path, URL or credentials
//! may enter this module. A staged file is NEVER a registered GGUF until
//! the desktop host separately admits it via native GGUF header checks.
use std::{
    fs::{self,File,OpenOptions},
    io::{self,Read,Write},
    path::{Path,PathBuf},
    sync::atomic::{AtomicBool,Ordering},
};
use sha2::{Digest,Sha256};

pub const MAX_MODEL_TRANSFER_BYTES:u64=128*1024*1024*1024;
const READ_BLOCK:usize=64*1024;
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum TransferError{
    InvalidPlan,UnsafeRoot,AlreadyExists,Interrupted,Transport,
    Overlong,Truncated,DigestMismatch,ReadbackMismatch,Publication,
}
#[derive(Debug,Clone,PartialEq,Eq,serde::Serialize,serde::Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ExactFilePlan{
    pub repo_id:String,pub revision:String,pub filename:String,
    pub expected_bytes:u64,pub sha256:String,
}
impl ExactFilePlan{
    pub fn validated(&self)->Result<(),TransferError>{
        if !crate::model_provider_catalog::valid_repo(&self.repo_id)||
           !crate::model_provider_catalog::valid_sha40(&self.revision)||
           self.expected_bytes==0||self.expected_bytes>MAX_MODEL_TRANSFER_BYTES||
           self.sha256.len()!=64||
           !self.sha256.bytes().all(|ch|ch.is_ascii_hexdigit())||
           self.sha256!=self.sha256.to_ascii_lowercase()||
           self.filename.len()>180||
           !self.filename.to_ascii_lowercase().ends_with(".gguf")||
           self.filename.starts_with('.')||self.filename.contains("..")||
           !self.filename.bytes().all(|ch|
               ch.is_ascii_alphanumeric()||matches!(ch,b'-'|b'_'|b'.')){
            return Err(TransferError::InvalidPlan)
        }
        Ok(())
    }
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct VerifiedTransfer{
    /// Exact SHA and bytes checked TWICE: while reading the approved stream
    /// and by separately reopening the materialized file on disk.
    pub sha256:String,pub bytes:u64,pub file:PathBuf,
    /// Does NOT mean the GGUF is admitted, registered or loaded.
    pub status:&'static str,
}
fn regular_directory(path:&Path)->bool{
    fs::symlink_metadata(path).is_ok_and(|m|m.file_type().is_dir())
}
fn valid_nonce(nonce:&str)->bool{
    (16..=96).contains(&nonce.len()) &&
    nonce.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'-'||b==b'_')
}
fn independent_readback(path:&Path,maximum:u64)->Result<(u64,String),TransferError>{
    if !fs::symlink_metadata(path).is_ok_and(|m|
        m.file_type().is_file()&&m.len()<=maximum){
        return Err(TransferError::ReadbackMismatch)
    }
    let mut data=File::open(path).map_err(|_|TransferError::ReadbackMismatch)?;
    let mut counted=0u64;let mut sha=Sha256::new();let mut chunk=[0u8;READ_BLOCK];
    loop{
        let read=data.read(&mut chunk).map_err(|_|TransferError::ReadbackMismatch)?;
        if read==0{break}
        counted=counted.checked_add(read as u64)
            .ok_or(TransferError::ReadbackMismatch)?;
        if counted>maximum{return Err(TransferError::ReadbackMismatch)}
        sha.update(&chunk[..read]);
    }
    Ok((counted,format!("{:x}",sha.finalize())))
}
/// The reviewed OS host creates a previously nonexistent private nonce
/// directory under a fixed profile-root. An interrupted .part is left for
/// explicit reconciliation; this module never auto-retries or silently
/// renames a partial file. Never give the reader a path from the model.
pub fn receive_verified_stream<R:Read>(
    trusted_root:&Path,nonce:&str,plan:&ExactFilePlan,
    mut stream:R,cancel:&AtomicBool,
)->Result<VerifiedTransfer,TransferError>{
    plan.validated()?;
    if !valid_nonce(nonce)||!regular_directory(trusted_root){
        return Err(TransferError::UnsafeRoot)
    }
    let staging_dir=trusted_root.join(nonce);
    if staging_dir.exists(){return Err(TransferError::AlreadyExists)}
    fs::create_dir(&staging_dir).map_err(|_|TransferError::UnsafeRoot)?;
    let temp=staging_dir.join("received.part");
    let mut output=OpenOptions::new().write(true).create_new(true)
        .open(&temp).map_err(|_|TransferError::Publication)?;
    let mut hash=Sha256::new();let mut received=0u64;
    let mut buf=[0u8;READ_BLOCK];
    loop{
        if cancel.load(Ordering::Acquire){return Err(TransferError::Interrupted)}
        let size=stream.read(&mut buf).map_err(|_|TransferError::Transport)?;
        if size==0{break}
        received=received.checked_add(size as u64).ok_or(TransferError::Overlong)?;
        if received>plan.expected_bytes{return Err(TransferError::Overlong)}
        hash.update(&buf[..size]);
        output.write_all(&buf[..size]).map_err(|_|TransferError::Transport)?;
    }
    output.flush().map_err(|_|TransferError::Publication)?;
    output.sync_all().map_err(|_|TransferError::Publication)?;
    drop(output);
    if received!=plan.expected_bytes{return Err(TransferError::Truncated)}
    let digest=format!("{:x}",hash.finalize());
    if digest!=plan.sha256{return Err(TransferError::DigestMismatch)}
    let (bytes,second)=independent_readback(&temp,plan.expected_bytes)?;
    if bytes!=received||second!=digest{return Err(TransferError::ReadbackMismatch)}
    if cancel.load(Ordering::Acquire){return Err(TransferError::Interrupted)}
    let final_path=staging_dir.join(&plan.filename);
    // The nonce directory was created NEW and the destination cannot be
    // supplied by the model. A second destination is never overwritten.
    if final_path.exists(){return Err(TransferError::AlreadyExists)}
    // hard_link is an atomic no-clobber publication. Unlike rename,
    // it never replaces a target created after the existence check.
    fs::hard_link(&temp,&final_path).map_err(|_|TransferError::Publication)?;
    // If the process dies here, the private .part and published file share
    // exact bytes; restart treats the journal as interrupted until reviewed.
    fs::remove_file(&temp).map_err(|_|TransferError::Publication)?;
    // Reopen after atomic publication: directory metadata and exact file
    // hash are not inferred from an earlier in-memory digest.
    let (published_bytes,published_sha)=independent_readback(&final_path,plan.expected_bytes)?;
    if published_bytes!=received||published_sha!=digest{
        return Err(TransferError::ReadbackMismatch)
    }
    Ok(VerifiedTransfer{
        sha256:digest,bytes:received,file:final_path,
        status:"verified_local_bytes_not_gguf_admitted",
    })
}
/// This function is the ONLY publication path for a real externally streamed
/// transfer. The transport must first prove exact HTTP 200 or 206 Range
/// semantics and successful curl termination. No 302/HTML/partial download
/// or cancellation may be promoted to an installed model.
pub fn publish_verified_part(
    trusted_root:&Path,nonce:&str,plan:&ExactFilePlan,cancel:&AtomicBool
)->Result<VerifiedTransfer,TransferError>{
    plan.validated()?;
    if !valid_nonce(nonce)||!regular_directory(trusted_root)||
        !regular_directory(&trusted_root.join(nonce)){
        return Err(TransferError::UnsafeRoot)
    }
    let folder=trusted_root.join(nonce);
    let part=folder.join("received.part");
    if cancel.load(Ordering::Acquire){return Err(TransferError::Interrupted)}
    let (bytes,digest)=independent_readback(&part,plan.expected_bytes)?;
    if bytes!=plan.expected_bytes{return Err(TransferError::Truncated)}
    if digest!=plan.sha256{return Err(TransferError::DigestMismatch)}
    if cancel.load(Ordering::Acquire){return Err(TransferError::Interrupted)}
    let destination=folder.join(&plan.filename);
    if destination.exists(){return Err(TransferError::AlreadyExists)}
    // Same-directory rename is atomic and never overwrites a previous target.
    fs::hard_link(&part,&destination).map_err(|_|TransferError::Publication)?;
    fs::remove_file(&part).map_err(|_|TransferError::Publication)?;
    // Cancellation after rename cannot un-publish data. Return an exact
    // readback result rather than inventing a failed/rolled-back operation.
    let (published_bytes,published_digest)=
        independent_readback(&destination,plan.expected_bytes)?;
    if published_bytes!=bytes||published_digest!=digest{
        return Err(TransferError::ReadbackMismatch)
    }
    Ok(VerifiedTransfer{
        sha256:digest,bytes,file:destination,
        status:"verified_local_bytes_not_gguf_admitted"
    })
}

#[cfg(test)]
mod tests{
    use super::*;
    use std::{io::Cursor,sync::atomic::AtomicBool,time::{SystemTime,UNIX_EPOCH}};
    fn root()->PathBuf{
        let p=std::env::temp_dir().join(format!("lu-hf-p05-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir(&p).unwrap();p
    }
    fn plan(bytes:&[u8])->ExactFilePlan{
        ExactFilePlan{
            repo_id:"official/verified-model".into(),
            revision:"0123456789abcdef0123456789abcdef01234567".into(),
            filename:"model-q4.gguf".into(),
            expected_bytes:bytes.len() as u64,
            sha256:format!("{:x}",Sha256::digest(bytes)),
        }
    }
    #[test]
    fn independently_reopens_exact_published_bytes(){
        let root=root();let bytes=b"GGUF-test-fixture-not-model";
        let result=receive_verified_stream(
            &root,"freshnonce0000001",&plan(bytes),
            Cursor::new(bytes),&AtomicBool::new(false)).unwrap();
        assert_eq!(result.bytes,bytes.len() as u64);
        assert_eq!(result.sha256,plan(bytes).sha256);
        assert_eq!(result.status,"verified_local_bytes_not_gguf_admitted");
        assert_eq!(fs::read(result.file).unwrap(),bytes);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn destination_can_never_be_overwritten_even_if_created_during_review(){
        let root=root();let bytes=b"real verified bytes";
        let approved=plan(bytes);
        let dir=root.join("freshnonce0000008");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("received.part"),bytes).unwrap();
        fs::write(dir.join(&approved.filename),b"existing must survive").unwrap();
        assert_eq!(publish_verified_part(&root,"freshnonce0000008",
            &approved,&AtomicBool::new(false)),Err(TransferError::AlreadyExists));
        assert_eq!(fs::read(dir.join(&approved.filename)).unwrap(),b"existing must survive");
        assert_eq!(fs::read(dir.join("received.part")).unwrap(),bytes);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn externally_streamed_part_cannot_publish_until_sha_size_and_review_match(){
        let root=root();let bytes=b"GGUF-trusted-network-fixture";
        let approved=plan(bytes);
        let folder=root.join("freshnonce0000006");
        fs::create_dir(&folder).unwrap();
        fs::write(folder.join("received.part"),&bytes[..8]).unwrap();
        assert_eq!(publish_verified_part(&root,"freshnonce0000006",
            &approved,&AtomicBool::new(false)),Err(TransferError::Truncated));
        fs::write(folder.join("received.part"),bytes).unwrap();
        assert_eq!(publish_verified_part(&root,"freshnonce0000006",
            &approved,&AtomicBool::new(true)),Err(TransferError::Interrupted));
        let done=publish_verified_part(&root,"freshnonce0000006",
            &approved,&AtomicBool::new(false)).unwrap();
        assert_eq!(done.sha256,approved.sha256);
        assert!(done.file.is_file());
        assert!(!folder.join("received.part").exists());
        assert_eq!(publish_verified_part(&root,"freshnonce0000006",
            &approved,&AtomicBool::new(false)),Err(TransferError::ReadbackMismatch));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn rejects_mismatch_truncation_replay_and_path_injection(){
        let root=root();let bytes=b"original";
        let mut wrong=plan(bytes);wrong.sha256="0".repeat(64);
        assert_eq!(receive_verified_stream(&root,"freshnonce0000002",
            &wrong,Cursor::new(bytes),&AtomicBool::new(false)),
            Err(TransferError::DigestMismatch));
        assert!(root.join("freshnonce0000002/received.part").exists());
        assert_eq!(receive_verified_stream(&root,"freshnonce0000002",
            &wrong,Cursor::new(bytes),&AtomicBool::new(false)),
            Err(TransferError::AlreadyExists));
        let correct=plan(bytes);
        assert_eq!(receive_verified_stream(&root,"freshnonce0000003",
            &correct,Cursor::new(&bytes[..3]),&AtomicBool::new(false)),
            Err(TransferError::Truncated));
        let mut bad=correct.clone();bad.filename="../evil.gguf".into();
        assert_eq!(receive_verified_stream(&root,"freshnonce0000004",
            &bad,Cursor::new(bytes),&AtomicBool::new(false)),
            Err(TransferError::InvalidPlan));
        assert_eq!(receive_verified_stream(&root,"../escape",
            &correct,Cursor::new(bytes),&AtomicBool::new(false)),
            Err(TransferError::UnsafeRoot));
        assert_eq!(receive_verified_stream(&root,"freshnonce0000005",
            &correct,Cursor::new(bytes),&AtomicBool::new(true)),
            Err(TransferError::Interrupted));
        fs::remove_dir_all(root).unwrap();
    }
}