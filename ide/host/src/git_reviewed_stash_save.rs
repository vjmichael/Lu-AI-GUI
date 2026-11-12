//! FIN-03 reviewed one-file stash save. No remote operation or shell.
//! This is a source-level implementation candidate, default disabled.
use crate::{git_branch_operations::exact_repo,git_reviewed_restore::SingleFileRestoreReview,
 IdeWorkspaceRegistry};
use git2::Signature;
use serde::Serialize;
use sha2::{Digest,Sha256};
use std::{fs::{self,OpenOptions},io::Write,sync::atomic::{AtomicU8,Ordering}};
fn sha(b:&[u8])->String{format!("{:x}",Sha256::digest(b))}
#[derive(Clone,Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub struct ReviewedStashSave {
 pub project_id:String,pub relative_path:String,pub head_sha:String,
 pub original_sha256:String,pub source_review_sha256:String,
 pub initial_stash_oid:Option<String>,pub review_sha256:String,
 pub operation_authorized:bool,pub refs_modified:bool,pub worktree_modified:bool,
}
#[derive(Clone,Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub struct ReviewedStashReceipt {
 pub project_id:String,pub relative_path:String,pub stash_oid:String,
 pub review_sha256:String,pub original_backup_name:String,
 pub worktree_restored_to_head:bool,pub stash_ref_changed:bool,
 pub original_preserved:bool,pub installed_accepted:bool,
}
fn stash_tip(repo:&git2::Repository)->Result<Option<String>,&'static str>{
 match repo.find_reference("refs/stash"){
  Ok(r)=>r.target().map(|x|x.to_string()).ok_or("FIN03_STASH_REF_INDIRECT").map(Some),
  Err(e) if e.code()==git2::ErrorCode::NotFound=>Ok(None),
  Err(_)=>Err("FIN03_STASH_REF_UNVERIFIABLE"),
 }
}
impl IdeWorkspaceRegistry {
 pub fn review_one_file_stash_save(&self,id:&str)
  ->Result<ReviewedStashSave,&'static str>{
  let source:SingleFileRestoreReview=self.review_single_file_restore(id)?;
  let repo=exact_repo(self,id)?;
  if repo.path().join("lu-ide-fin03-stash-active").exists(){
   return Err("FIN03_STASH_PREVIOUS_INTERRUPTION");
  }
  let tip=stash_tip(&repo)?;
  let reviewed=sha(format!("fin03-stash-save-v1:{}:{}:{}:{}",
    id,source.review_sha256,source.relative_path,
    tip.as_deref().unwrap_or("no-stash")).as_bytes());
  Ok(ReviewedStashSave{
   project_id:id.into(),relative_path:source.relative_path,
   head_sha:source.head_sha,original_sha256:source.original_sha256,
   source_review_sha256:source.review_sha256,initial_stash_oid:tip,
   review_sha256:reviewed,operation_authorized:false,refs_modified:false,
   worktree_modified:false})
 }
 pub fn execute_reviewed_one_file_stash_save(&self,expected:&ReviewedStashSave,
  cancel:&AtomicU8)->Result<ReviewedStashReceipt,&'static str>{
  if expected.operation_authorized || expected.refs_modified ||
     expected.worktree_modified || cancel.load(Ordering::SeqCst)!=0 {
    return Err("FIN03_STASH_AUTHORITY_DENIED");
  }
  let fresh=self.review_one_file_stash_save(&expected.project_id)?;
  if fresh.review_sha256!=expected.review_sha256 ||
     fresh.relative_path!=expected.relative_path ||
     fresh.source_review_sha256!=expected.source_review_sha256 ||
     fresh.head_sha!=expected.head_sha ||
     fresh.original_sha256!=expected.original_sha256 ||
     fresh.initial_stash_oid!=expected.initial_stash_oid {
    return Err("FIN03_STASH_REVIEW_CHANGED");
  }
  let mut repo=exact_repo(self,&expected.project_id)?;
  let root=self.registered_git_root(&expected.project_id)
   .map_err(|_|"FIN03_STASH_PROJECT_CLOSED")?;
  let original=fs::read(root.join(&fresh.relative_path))
   .map_err(|_|"FIN03_STASH_ORIGINAL_UNREADABLE")?;
  if original.len()>8*1024*1024{return Err("FIN03_STASH_SOURCE_TOO_LARGE");}
  let backup_name=format!("lu-ide-fin03-stash-original-{}.bin",fresh.review_sha256);
  let backup=repo.path().join(&backup_name);
  let mut backup_opts=OpenOptions::new();
  backup_opts.create_new(true).write(true);
  #[cfg(unix)] {
   use std::os::unix::fs::OpenOptionsExt;
   backup_opts.mode(0o600);
  }
  let mut file=backup_opts.open(&backup)
   .map_err(|_|"FIN03_STASH_BACKUP_NOT_CREATED")?;
  file.write_all(&original).and_then(|_|file.sync_all())
   .map_err(|_|"FIN03_STASH_BACKUP_SYNC_FAILED")?;
  let marker=repo.path().join("lu-ide-fin03-stash-active");
  let mut journal_opts=OpenOptions::new();
  journal_opts.create_new(true).write(true);
  #[cfg(unix)] {
   use std::os::unix::fs::OpenOptionsExt;
   journal_opts.mode(0o600);
  }
  let mut journal=journal_opts.open(&marker)
   .map_err(|_|"FIN03_STASH_INTERRUPTED_OPERATION")?;
  journal.write_all(fresh.review_sha256.as_bytes())
   .and_then(|_|journal.sync_all())
   .map_err(|_|"FIN03_STASH_JOURNAL_SYNC_FAILED")?;
  #[cfg(unix)]
  std::fs::File::open(repo.path()).and_then(|dir|dir.sync_all())
   .map_err(|_|"FIN03_STASH_GIT_DIRECTORY_SYNC_FAILED")?;
  if cancel.compare_exchange(0,1,Ordering::SeqCst,Ordering::SeqCst).is_err(){
   return Err("FIN03_STASH_CANCELLED_RECONCILIATION_REQUIRED");
  }
  // A fresh tracked-status/index/tree review is separate from the stash
  // review, which cannot run while the private interruption marker exists.
  let source_again=self.review_single_file_restore(&expected.project_id)?;
  if source_again.review_sha256!=fresh.source_review_sha256 ||
     source_again.head_sha!=fresh.head_sha ||
     source_again.original_sha256!=fresh.original_sha256 {
   return Err("FIN03_STASH_SOURCE_CHANGED_AFTER_BACKUP");
  }
  if fs::read(root.join(&fresh.relative_path))
      .map_err(|_|"FIN03_STASH_TARGET_UNREADABLE")?!=original ||
     stash_tip(&repo)?!=fresh.initial_stash_oid {
   return Err("FIN03_STASH_SOURCE_CHANGED_AFTER_BACKUP");
  }
  let signature=Signature::now("Lu IDE protected stash","lu-ide@local.invalid")
   .map_err(|_|"FIN03_STASH_SIGNATURE_INVALID")?;
  let oid=repo.stash_save(&signature,"Lu IDE reviewed one-file stash",None)
   .map_err(|_|"FIN03_STASH_EXECUTION_UNCERTAIN")?;
  let expected_tip=oid.to_string();
  if stash_tip(&repo)?.as_deref()!=Some(expected_tip.as_str()){
   return Err("FIN03_STASH_REF_READBACK_UNCERTAIN");
  }
  let original_head=repo.head().map_err(|_|"FIN03_STASH_HEAD_UNAVAILABLE")?
   .peel_to_commit().map_err(|_|"FIN03_STASH_HEAD_UNAVAILABLE")?;
  if original_head.id().to_string()!=fresh.head_sha {
   return Err("FIN03_STASH_HEAD_CHANGED");
  }
  let tree=original_head.tree().map_err(|_|"FIN03_STASH_HEAD_TREE_UNAVAILABLE")?;
  let entry=tree.get_path(std::path::Path::new(&fresh.relative_path))
   .map_err(|_|"FIN03_STASH_HEAD_PATH_UNAVAILABLE")?;
  let head_bytes=repo.find_blob(entry.id())
   .map_err(|_|"FIN03_STASH_HEAD_BLOB_UNAVAILABLE")?.content().to_vec();
  let restored=fs::read(root.join(&fresh.relative_path))
    .map_err(|_|"FIN03_STASH_WORKTREE_READBACK_UNAVAILABLE")?;
  if restored!=head_bytes {
    return Err("FIN03_STASH_WORKTREE_READBACK_UNCERTAIN");
  }
  // Verify the original persisted backup independently before clearing.
  if fs::read(&backup).map_err(|_|"FIN03_STASH_BACKUP_READBACK_UNAVAILABLE")?!=original {
    return Err("FIN03_STASH_ORIGINAL_BACKUP_CHANGED");
  }
  fs::remove_file(marker).map_err(|_|"FIN03_STASH_JOURNAL_CLEAR_FAILED")?;
  #[cfg(unix)]
  std::fs::File::open(repo.path()).and_then(|dir|dir.sync_all())
   .map_err(|_|"FIN03_STASH_JOURNAL_RELEASE_SYNC_FAILED")?;
  Ok(ReviewedStashReceipt{
   project_id:expected.project_id.clone(),relative_path:fresh.relative_path,
   stash_oid:oid.to_string(),review_sha256:fresh.review_sha256,
   original_backup_name:backup_name,worktree_restored_to_head:true,
   stash_ref_changed:true,original_preserved:true,installed_accepted:false,
  })
 }
 pub fn one_file_stash_recovery_required(&self,id:&str)->Result<bool,&'static str>{
  let repo=exact_repo(self,id)?;
  let marker=repo.path().join("lu-ide-fin03-stash-active");
  match fs::symlink_metadata(&marker){
   Ok(meta)=>{
    if !meta.is_file()||meta.file_type().is_symlink()||meta.len()!=64{
     return Err("FIN03_STASH_JOURNAL_CORRUPT");
    }
    let bytes=fs::read(&marker).map_err(|_|"FIN03_STASH_JOURNAL_UNREADABLE")?;
    if bytes.len()!=64 || !bytes.iter().all(|b|b.is_ascii_hexdigit()&&!b.is_ascii_uppercase()){
     return Err("FIN03_STASH_JOURNAL_INVALID");
    }
    let digest=std::str::from_utf8(&bytes).map_err(|_|"FIN03_STASH_JOURNAL_INVALID")?;
    let name=format!("lu-ide-fin03-stash-original-{digest}.bin");
    let backup=fs::symlink_metadata(repo.path().join(name))
     .map_err(|_|"FIN03_STASH_ORIGINAL_BACKUP_MISSING")?;
    if !backup.is_file() || backup.file_type().is_symlink() ||
       backup.len()>8*1024*1024 {
     return Err("FIN03_STASH_ORIGINAL_BACKUP_UNTRUSTED");
    }
    Ok(true)
   },
   Err(e) if e.kind()==std::io::ErrorKind::NotFound=>Ok(false),
   Err(_)=>Err("FIN03_STASH_RECOVERY_UNAVAILABLE"),
  }
 }
}
