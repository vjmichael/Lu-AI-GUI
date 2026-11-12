//! FIN-03: review and restore exactly one modified tracked regular file.
//! Restoring is destructive to the visible worktree, so original bytes are
//! preserved under the local Git directory; interrupted jobs require review.
use crate::{git_branch_operations::exact_repo,IdeWorkspaceRegistry};
use git2::{Repository,Status,StatusOptions,RepositoryState,ObjectType};
use serde::{Serialize,Deserialize};
use sha2::{Sha256,Digest};
use std::{fs::{self,OpenOptions},io::Write,path::Path,sync::atomic::{AtomicU8,Ordering}};
const LIMIT:usize=8*1024*1024;
fn digest(b:&[u8])->String{format!("{:x}",Sha256::digest(b))}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct SingleFileRestoreReview {
 pub project_id:String,pub relative_path:String,pub head_sha:String,
 pub original_sha256:String,pub restored_sha256:String,pub review_sha256:String,
 pub operation_authorized:bool,pub source_files_modified:bool,
 pub refs_modified:bool,
}
#[derive(Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub struct SingleFileRestoreReceipt {
 pub project_id:String,pub relative_path:String,pub review_sha256:String,
 pub backup_relative_git_path:String,pub source_restored:bool,
 pub original_preserved:bool,pub refs_modified:bool,
 pub installed_acceptance:bool,
}
fn inspected(reg:&IdeWorkspaceRegistry,id:&str)
 ->Result<(Repository,String,String,Vec<u8>,Vec<u8>),&'static str>{
 let repo=exact_repo(reg,id)?;
 if repo.state()!=RepositoryState::Clean || repo.head_detached().unwrap_or(true) ||
    repo.path().join("index.lock").exists() ||
    repo.path().join("lu-ide-fin03-restore-active.json").exists() {
    return Err("FIN03_RESTORE_REPO_BUSY");
 }
 let head=repo.head().map_err(|_|"FIN03_RESTORE_HEAD_UNAVAILABLE")?
  .target().ok_or("FIN03_RESTORE_DETACHED_HEAD")?;
 let commit=repo.find_commit(head).map_err(|_|"FIN03_RESTORE_BAD_HEAD")?;
 let tree=commit.tree().map_err(|_|"FIN03_RESTORE_BAD_TREE")?;
 let mut opts=StatusOptions::new();
 opts.include_untracked(true).recurse_untracked_dirs(true).include_ignored(false)
  .renames_head_to_index(false).renames_index_to_workdir(false);
 let statuses=repo.statuses(Some(&mut opts)).map_err(|_|"FIN03_RESTORE_STATUS")?;
 if statuses.len()!=1{return Err("FIN03_RESTORE_ONE_FILE_ONLY");}
 let row=statuses.get(0).ok_or("FIN03_RESTORE_STATUS_EMPTY")?;
 if row.status()!=Status::WT_MODIFIED{return Err("FIN03_RESTORE_ONLY_UNSTAGED_MODIFICATION");}
 let path=row.path().ok_or("FIN03_RESTORE_PATH_MISSING")?.to_string();
 let root=reg.registered_git_root(id).map_err(|_|"FIN03_RESTORE_PROJECT_CLOSED")?;
 if path.starts_with('/') || path.contains('\\') || path.contains(':') ||
    path.split('/').any(|s|s.is_empty()||s=="."||s=="..") ||
    path.len()>1024 {return Err("FIN03_RESTORE_PATH_UNSAFE");}
 let target=root.join(&path);
 let meta=fs::symlink_metadata(&target).map_err(|_|"FIN03_RESTORE_FILE_MISSING")?;
 if !meta.is_file() || meta.file_type().is_symlink() || meta.len()>LIMIT as u64 {
    return Err("FIN03_RESTORE_NOT_REGULAR");
 }
 let original=fs::read(&target).map_err(|_|"FIN03_RESTORE_READ")?;
 if original.len()>LIMIT{return Err("FIN03_RESTORE_TOO_LARGE");}
 let entry=tree.get_path(Path::new(&path)).map_err(|_|"FIN03_RESTORE_TREE_ENTRY")?;
 if entry.kind()!=Some(ObjectType::Blob) || entry.filemode()!=0o100644 {
    return Err("FIN03_RESTORE_UNSUPPORTED_MODE");
 }
 let idx=repo.index().map_err(|_|"FIN03_RESTORE_INDEX")?;
 if idx.has_conflicts() || idx.get_path(Path::new(&path),0).map(|e|e.id)!=Some(entry.id()){
    return Err("FIN03_RESTORE_STAGED_CHANGES_PRESENT");
 }
 let bytes=repo.find_blob(entry.id()).map_err(|_|"FIN03_RESTORE_BLOB")?.content().to_vec();
 if bytes.len()>LIMIT{return Err("FIN03_RESTORE_HEAD_TOO_LARGE");}
 let h=head.to_string();
 drop(idx);drop(statuses);drop(tree);drop(commit);
 Ok((repo,h,path,original,bytes))
}
impl IdeWorkspaceRegistry {
 pub fn review_single_file_restore(&self,id:&str)
  ->Result<SingleFileRestoreReview,&'static str>{
  let (_,head,path,old,new)=inspected(self,id)?;
  let a=digest(&old);let b=digest(&new);
  let review=digest(format!("fin03-single-file-restore-v1\\0{id}\\0{head}\\0{path}\\0{a}\\0{b}").as_bytes());
  Ok(SingleFileRestoreReview{project_id:id.into(),relative_path:path,
   head_sha:head,original_sha256:a,restored_sha256:b,review_sha256:review,
   operation_authorized:false,source_files_modified:false,refs_modified:false})
 }
 pub fn execute_approved_single_file_restore(&self,expected:&SingleFileRestoreReview,
  cancel:&AtomicU8)->Result<SingleFileRestoreReceipt,&'static str>{
  if expected.operation_authorized || expected.source_files_modified || expected.refs_modified ||
     cancel.load(Ordering::SeqCst)!=0{return Err("FIN03_RESTORE_NOT_AUTHORIZED");}
  let fresh=self.review_single_file_restore(&expected.project_id)?;
  if fresh.review_sha256!=expected.review_sha256 || fresh.head_sha!=expected.head_sha ||
     fresh.relative_path!=expected.relative_path{return Err("FIN03_RESTORE_SOURCE_CHANGED");}
  let (repo,_,path,original,bytes)=inspected(self,&expected.project_id)?;
  let root=self.registered_git_root(&expected.project_id)
   .map_err(|_|"FIN03_RESTORE_PROJECT_CLOSED")?;
  let marker=repo.path().join("lu-ide-fin03-restore-active.json");
  let backup_name=format!("lu-ide-fin03-restore-backup-{}.bin",expected.review_sha256);
  let backup=repo.path().join(&backup_name);
  let backup_file=OpenOptions::new().write(true).create_new(true).open(&backup)
   .map_err(|_|"FIN03_RESTORE_BACKUP_EXISTS_OR_UNWRITABLE")?;
  let mut backup_file=backup_file;
  backup_file.write_all(&original).and_then(|_|backup_file.sync_all())
   .map_err(|_|"FIN03_RESTORE_BACKUP_SYNC_FAILED")?;
  let mut marker_file=OpenOptions::new().write(true).create_new(true).open(&marker)
   .map_err(|_|"FIN03_RESTORE_PREVIOUS_INTERRUPTION")?;
  marker_file.write_all(expected.review_sha256.as_bytes())
   .and_then(|_|marker_file.sync_all())
   .map_err(|_|"FIN03_RESTORE_JOURNAL_SYNC_FAILED")?;
  if cancel.compare_exchange(0,1,Ordering::SeqCst,Ordering::SeqCst).is_err(){
    return Err("FIN03_RESTORE_CANCELLED_RECONCILIATION_REQUIRED");
  }
  // Restrict the replacement to a previously validated regular tracked file.
  let target=root.join(&path);
  if fs::symlink_metadata(&target).map_err(|_|"FIN03_RESTORE_TARGET_GONE")?
      .file_type().is_symlink() ||
     digest(&fs::read(&target).map_err(|_|"FIN03_RESTORE_TARGET_READ")?)!=fresh.original_sha256 {
    return Err("FIN03_RESTORE_TARGET_CHANGED");
  }
  let tmp=root.join(format!(".lu-ide-fin03-restore-{}.tmp",fresh.review_sha256));
  let mut output=OpenOptions::new().write(true).create_new(true).open(&tmp)
   .map_err(|_|"FIN03_RESTORE_TEMP_EXISTS")?;
  output.write_all(&bytes).and_then(|_|output.sync_all())
   .map_err(|_|"FIN03_RESTORE_TEMP_SYNC_FAILED")?;
  // Windows replacement requires removing the old destination; preserve it
  // in the Git-private backup first. A failed rename requires operator repair.
  fs::remove_file(&target).map_err(|_|"FIN03_RESTORE_REPLACE_DENIED")?;
  fs::rename(&tmp,&target).map_err(|_|"FIN03_RESTORE_RENAME_FAILED")?;
  if digest(&fs::read(&target).map_err(|_|"FIN03_RESTORE_READBACK")?)!=fresh.restored_sha256 {
    return Err("FIN03_RESTORE_READBACK_MISMATCH");
  }
  fs::remove_file(&marker).map_err(|_|"FIN03_RESTORE_MARKER_CLEAR_FAILED")?;
  Ok(SingleFileRestoreReceipt{
   project_id:expected.project_id.clone(),relative_path:path,
   review_sha256:fresh.review_sha256,backup_relative_git_path:backup_name,
   source_restored:true,original_preserved:true,refs_modified:false,
   installed_acceptance:false})
 }
 pub fn single_file_restore_recovery_required(&self,id:&str)
  ->Result<bool,&'static str>{
  let repo=exact_repo(self,id)?;
  Ok(repo.path().join("lu-ide-fin03-restore-active.json").exists())
 }
}
