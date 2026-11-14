//! FIN-03: single-path, native one-use Git unstage with durable recovery.
//! No command invocation, no checkout or worktree write, no remote access.
//! Only one staged file, no other dirty entry, and an exact OS-picked repo.
use crate::{git_branch_operations::exact_repo,git_protected_merge::changed_path_is_plain_safe,IdeWorkspaceRegistry};
use git2::{Repository,Status,StatusOptions};
use serde::{Serialize,Deserialize};
use sha2::{Digest,Sha256};
use std::{fs::{self,OpenOptions,File},io::{Read,Write},path::{Path,PathBuf},sync::atomic::{AtomicU8,Ordering}};
const LIMIT:u64=64*1024*1024;
const JOURNAL_MAX:u64=1024*1024;

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ReviewedUnstage{
 pub project_id:String,pub head_sha:String,pub relative_path:String,
 pub index_sha256:String,pub index_blob:String,
 pub head_blob:Option<String>,pub review_sha256:String,
 pub source_rechecked:bool,pub operation_authorized:bool,
 pub hooks_executed:bool,pub refs_modified:bool,
 pub source_files_modified:bool,
}
#[derive(Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub struct UnstageReceipt{
 pub project_id:String,pub relative_path:String,pub review_sha256:String,
 pub head_sha:String,pub index_before:String,pub index_after:String,
 pub restored_head_blob:Option<String>,
 pub index_independently_verified:bool,pub source_files_modified:bool,
 pub refs_modified:bool,pub git_process_started:bool,
 pub operation_state:&'static str,
}
#[derive(Debug,Clone,Serialize,Deserialize)]
struct Journal{
 schema:u32,project_id:String,head_sha:String,
 relative_path:String,review_sha256:String,
 before:String,after:Option<String>,phase:String,
}
fn hex(bytes:&[u8])->String{format!("{:x}",Sha256::digest(bytes))}
fn index_digest(repo:&Repository)->Result<String,&'static str>{
 let path=repo.path().join("index");
 let meta=fs::symlink_metadata(&path).map_err(|_|"FIN03_INDEX_MISSING")?;
 if !meta.is_file()||meta.file_type().is_symlink()||meta.len()>LIMIT{
    return Err("FIN03_INDEX_UNTRUSTED");
 }
 let mut file=File::open(path).map_err(|_|"FIN03_INDEX_UNREADABLE")?;
 let mut bytes=Vec::new();
 file.take(LIMIT+1).read_to_end(&mut bytes).map_err(|_|"FIN03_INDEX_READ_FAILED")?;
 if bytes.len() as u64!=meta.len(){return Err("FIN03_INDEX_CHANGED_DURING_READ");}
 Ok(hex(&bytes))
}
fn inspected_repo(reg:&IdeWorkspaceRegistry,project:&str)
 ->Result<(Repository,String,String,Option<String>),&'static str>{
 let repo=exact_repo(reg,project)?;
 if repo.state()!=git2::RepositoryState::Clean||
    repo.head_detached().unwrap_or(true)||
    repo.path().join("index.lock").exists(){
    return Err("FIN03_UNSTAGE_REPO_BUSY");
 }
 let head=repo.head().map_err(|_|"FIN03_UNSTAGE_HEAD_MISSING")?;
 let oid=head.target().ok_or("FIN03_UNSTAGE_HEAD_INDIRECT")?;
 let commit=repo.find_commit(oid).map_err(|_|"FIN03_UNSTAGE_HEAD_NOT_COMMIT")?;
 let mut opts=StatusOptions::new();
 opts.include_untracked(true).recurse_untracked_dirs(true)
     .include_ignored(false).renames_head_to_index(false)
     .renames_index_to_workdir(false);
 let changes=repo.statuses(Some(&mut opts))
     .map_err(|_|"FIN03_UNSTAGE_STATUS_UNAVAILABLE")?;
 if changes.len()!=1{return Err("FIN03_UNSTAGE_REQUIRES_ONE_CHANGED_FILE");}
 let item=changes.get(0).ok_or("FIN03_UNSTAGE_STATUS_MISSING")?;
 let st=item.status();
 if !st.intersects(Status::INDEX_MODIFIED|Status::INDEX_NEW|Status::INDEX_DELETED)
    || st.intersects(Status::CONFLICTED|Status::WT_MODIFIED|Status::WT_NEW
        |Status::WT_DELETED|Status::WT_TYPECHANGE|Status::WT_RENAMED
        |Status::INDEX_RENAMED|Status::INDEX_TYPECHANGE){
    return Err("FIN03_UNSTAGE_REQUIRES_ONE_STAGED_FILE");
 }
 let path=item.path().ok_or("FIN03_UNSTAGE_PATH_MISSING")?.to_owned();
 let root=reg.registered_git_root(project)
     .map_err(|_|"FIN03_UNSTAGE_PROJECT_CLOSED")?;
 changed_path_is_plain_safe(&root,&path)?;
 let mut index=repo.index().map_err(|_|"FIN03_UNSTAGE_INDEX_UNAVAILABLE")?;
 index.read(true).map_err(|_|"FIN03_UNSTAGE_INDEX_RELOAD")?;
 if index.has_conflicts(){return Err("FIN03_UNSTAGE_CONFLICTED");}
 let entry=index.get_path(Path::new(&path),0);
 let index_blob=entry.map(|e|e.id.to_string()).unwrap_or_default();
 if index_blob.is_empty() && !st.contains(Status::INDEX_DELETED){
    return Err("FIN03_UNSTAGE_NO_INDEX_BLOB");
 }
 let tree=commit.tree().map_err(|_|"FIN03_UNSTAGE_TREE_UNAVAILABLE")?;
 let old=tree.get_path(Path::new(&path)).ok().map(|e|e.id().to_string());
 if old.is_none() && !st.contains(Status::INDEX_NEW){
    return Err("FIN03_UNSTAGE_HEAD_FILE_MISSING");
 }
 let old_sha=oid.to_string();
 // git2 returned borrowed views must be released before moving Repository.
 drop(tree);drop(index);drop(changes);drop(commit);drop(head);
 Ok((repo,old_sha,path,old))
}
fn journal_path(repo:&Repository)->PathBuf{
 repo.path().join("lu-ide-fin03-unstage-v1.jsonl")
}
fn latest(repo:&Repository)->Result<Option<Journal>,&'static str>{
 let path=journal_path(repo);
 let meta=match fs::symlink_metadata(&path){
    Ok(m)=>m,
    Err(e) if e.kind()==std::io::ErrorKind::NotFound=>return Ok(None),
    Err(_)=>return Err("FIN03_UNSTAGE_JOURNAL_STAT"),
 };
 if !meta.is_file()||meta.file_type().is_symlink()||meta.len()>JOURNAL_MAX{
    return Err("FIN03_UNSTAGE_JOURNAL_UNTRUSTED");
 }
 let bytes=fs::read(&path).map_err(|_|"FIN03_UNSTAGE_JOURNAL_READ")?;
 if bytes.last()!=Some(&b'\n'){return Err("FIN03_UNSTAGE_JOURNAL_INCOMPLETE");}
 let row=bytes.split(|b|*b==b'\n').filter(|b|!b.is_empty()).last()
    .ok_or("FIN03_UNSTAGE_JOURNAL_EMPTY")?;
 let j:Journal=serde_json::from_slice(row)
    .map_err(|_|"FIN03_UNSTAGE_JOURNAL_CORRUPT")?;
 if j.schema!=1 || j.project_id.len()!=36 || !j.project_id.starts_with("ide-")
    || !matches!(j.phase.as_str(),"prepared"|"publishing"|"verified"|"cancelled") {
    return Err("FIN03_UNSTAGE_JOURNAL_INVALID");
 }
 Ok(Some(j))
}
fn append(repo:&Repository,j:&Journal)->Result<(),&'static str>{
 let p=journal_path(repo);
 let old=match fs::symlink_metadata(&p){
    Ok(m) if m.is_file()&&!m.file_type().is_symlink()=>m.len(),
    Err(e) if e.kind()==std::io::ErrorKind::NotFound=>0,
    _=>return Err("FIN03_UNSTAGE_JOURNAL_UNTRUSTED"),
 };
 let line=serde_json::to_vec(j).map_err(|_|"FIN03_UNSTAGE_JOURNAL_SERIALIZE")?;
 if old+line.len() as u64+1>JOURNAL_MAX{return Err("FIN03_UNSTAGE_JOURNAL_FULL")}
 let mut out=OpenOptions::new().create(true).append(true).open(p)
    .map_err(|_|"FIN03_UNSTAGE_JOURNAL_OPEN")?;
 if out.metadata().map_err(|_|"FIN03_UNSTAGE_JOURNAL_STAT")?.len()!=old{
    return Err("FIN03_UNSTAGE_JOURNAL_RACE");
 }
 out.write_all(&line).and_then(|_|out.write_all(b"\n"))
    .and_then(|_|out.sync_all())
    .map_err(|_|"FIN03_UNSTAGE_JOURNAL_FSYNC")?;
 Ok(())
}
fn review_hash(project:&str,head:&str,path:&str,index:&str,
 old:Option<&str>,new:&str)->String{
 let data=format!("fin03-native-unstage-v1\0{project}\0{head}\0{path}\0{index}\0{}\0{new}",
    old.unwrap_or("absent"));
 hex(data.as_bytes())
}
impl IdeWorkspaceRegistry{
 pub fn review_single_staged_unstage(&self,id:&str)
    ->Result<ReviewedUnstage,&'static str>{
  let (repo,head,path,old)=inspected_repo(self,id)?;
  if let Some(j)=latest(&repo)?{
    if !matches!(j.phase.as_str(),"verified"|"cancelled"){return Err("FIN03_UNSTAGE_PREVIOUS_INTERRUPTED");}
  }
  let index=index_digest(&repo)?;
  let ix=repo.index().map_err(|_|"FIN03_UNSTAGE_INDEX")?;
  let blob=ix.get_path(Path::new(&path),0).map(|x|x.id.to_string())
     .unwrap_or_default();
  Ok(ReviewedUnstage{
    project_id:id.into(),head_sha:head.clone(),relative_path:path.clone(),
    index_sha256:index.clone(),index_blob:blob.clone(),
    head_blob:old.clone(),review_sha256:review_hash(id,&head,&path,&index,
       old.as_deref(),&blob),
    source_rechecked:true,operation_authorized:false,hooks_executed:false,
    refs_modified:false,source_files_modified:false,
  })
 }
 /// Only call after separate native modal; one-use cancellation is checked
 /// immediately before the libgit2 index commit point.
 pub fn execute_approved_single_unstage(&self,expected:&ReviewedUnstage,
    cancel:&AtomicU8)->Result<UnstageReceipt,&'static str>{
  let fresh=self.review_single_staged_unstage(&expected.project_id)?;
  if fresh.review_sha256!=expected.review_sha256||
    fresh.head_sha!=expected.head_sha||
    fresh.index_sha256!=expected.index_sha256||
    fresh.relative_path!=expected.relative_path||
    fresh.index_blob!=expected.index_blob||
    fresh.head_blob!=expected.head_blob||
    expected.operation_authorized||
    cancel.load(Ordering::SeqCst)!=0{
    return Err("FIN03_UNSTAGE_STALE_OR_CANCELLED");
  }
  let (repo,head,path,old)=inspected_repo(self,&expected.project_id)?;
  let mut j=Journal{
    schema:1,project_id:expected.project_id.clone(),head_sha:head.clone(),
    relative_path:path.clone(),review_sha256:fresh.review_sha256.clone(),
    before:fresh.index_sha256.clone(),after:None,phase:"prepared".into(),
  };
  self.require_group6f_no_pending_recovery(&expected.project_id,false)?;
  append(&repo,&j)?;
  if cancel.compare_exchange(0,1,Ordering::SeqCst,Ordering::SeqCst).is_err(){
    // Even a pre-commit interrupt is left for explicit reconciliation.
    return Err("FIN03_UNSTAGE_CANCELLED_WITH_PREPARED_JOURNAL");
  }
  j.phase="publishing".into();append(&repo,&j)?;
  let head_obj=repo.find_object(
    git2::Oid::from_str(&head).map_err(|_|"FIN03_UNSTAGE_BAD_HEAD")?,
    None).map_err(|_|"FIN03_UNSTAGE_HEAD_UNAVAILABLE")?;
  repo.reset_default(Some(&head_obj),[Path::new(&path)])
    .map_err(|_|"FIN03_UNSTAGE_INDEX_WRITE_AMBIGUOUS")?;
  let after=index_digest(&repo)?;
  let ix=repo.index().map_err(|_|"FIN03_UNSTAGE_INDEX_RELOAD")?;
  let observed=ix.get_path(Path::new(&path),0).map(|e|e.id.to_string());
  if observed!=old || head!=repo.head().map_err(|_|"FIN03_UNSTAGE_HEAD_CHANGED")?
        .target().ok_or("FIN03_UNSTAGE_HEAD_INDIRECT")?.to_string(){
    return Err("FIN03_UNSTAGE_POST_WRITE_UNVERIFIED");
  }
  // A matching index readback proves only this action's index effect.
  j.after=Some(after.clone());j.phase="verified".into();append(&repo,&j)?;
  Ok(UnstageReceipt{
    project_id:expected.project_id.clone(),relative_path:path,
    review_sha256:fresh.review_sha256,head_sha:head,
    index_before:fresh.index_sha256,index_after:after,
    restored_head_blob:old,index_independently_verified:true,
    source_files_modified:false,refs_modified:false,
    git_process_started:false,operation_state:"verified_index_only",
  })
 }
 pub fn inspect_unstage_interrupted(&self,id:&str)
    ->Result<Option<&'static str>,&'static str>{
   let repo=exact_repo(self,id)?;
   let Some(j)=latest(&repo)? else{return Ok(None)};
   if j.project_id!=id{return Err("FIN03_UNSTAGE_JOURNAL_PROJECT_MISMATCH");}
   Ok(Some(match j.phase.as_str(){
      "verified"=>"verified",
      "cancelled"=>"cancelled_before_publication",
      _=>"manual_reconciliation_required",
   }))
 }
 /// This does NOT replay an index operation. It only classifies an
 /// interrupted exact-HEAD journal from independently reread native Git.
 pub fn reconcile_unstage_interrupted(&self,id:&str)
    ->Result<&'static str,&'static str>{
   let repo=exact_repo(self,id)?;
   if repo.head_detached().unwrap_or(true) ||
      repo.state()!=git2::RepositoryState::Clean ||
      repo.path().join("index.lock").exists(){
      return Err("FIN03_UNSTAGE_RECOVERY_GIT_BUSY");
   }
   let mut j=latest(&repo)?.ok_or("FIN03_UNSTAGE_NO_INTERRUPTED_RECORD")?;
   if j.project_id!=id{return Err("FIN03_UNSTAGE_RECOVERY_WRONG_PROJECT");}
   if matches!(j.phase.as_str(),"cancelled"|"verified"){
      return Ok("already_terminal");
   }
   let now_head=repo.head().map_err(|_|"FIN03_UNSTAGE_HEAD_MISSING")?
      .target().ok_or("FIN03_UNSTAGE_HEAD_INDIRECT")?.to_string();
   if now_head!=j.head_sha{
      return Err("FIN03_UNSTAGE_RECOVERY_HEAD_CHANGED");
   }
   let root=self.registered_git_root(id)
      .map_err(|_|"FIN03_UNSTAGE_RECOVERY_PROJECT_CLOSED")?;
   changed_path_is_plain_safe(&root,&j.relative_path)?;
   let now=index_digest(&repo)?;
   if now==j.before{
      j.after=Some(now);j.phase="cancelled".into();
      append(&repo,&j)?;
      return Ok("cancelled_without_index_publication");
   }
   let head=repo.find_commit(git2::Oid::from_str(&j.head_sha)
      .map_err(|_|"FIN03_UNSTAGE_BAD_HEAD")?)
      .map_err(|_|"FIN03_UNSTAGE_HEAD_NOT_COMMIT")?;
   let tree=head.tree().map_err(|_|"FIN03_UNSTAGE_TREE_MISSING")?;
   let expect=tree.get_path(Path::new(&j.relative_path)).ok()
      .map(|x|x.id().to_string());
   let idx=repo.index().map_err(|_|"FIN03_UNSTAGE_RECOVERY_INDEX")?;
   if idx.has_conflicts() ||
      idx.get_path(Path::new(&j.relative_path),0)
         .map(|e|e.id.to_string())!=expect{
      return Err("FIN03_UNSTAGE_RECOVERY_INDEX_AMBIGUOUS");
   }
   let mut opts=StatusOptions::new();
   opts.include_untracked(true).recurse_untracked_dirs(true)
       .renames_head_to_index(false).renames_index_to_workdir(false);
   let status=repo.statuses(Some(&mut opts))
       .map_err(|_|"FIN03_UNSTAGE_RECOVERY_STATUS")?;
   if status.iter().any(|e|e.status().intersects(
       Status::INDEX_MODIFIED|Status::INDEX_NEW|Status::INDEX_DELETED|
       Status::INDEX_RENAMED|Status::INDEX_TYPECHANGE|Status::CONFLICTED)){
       return Err("FIN03_UNSTAGE_RECOVERY_OTHER_STAGED_ENTRIES");
   }
   j.after=Some(now);j.phase="verified".into();append(&repo,&j)?;
   Ok("verified_index_already_restored")
 }
}
#[cfg(test)]
mod tests{
 use super::*;
 #[test] fn source_bound_review_hash_changes_with_index(){
  let a=review_hash("ide-x","head","a.rs","abc",None,"new");
  let b=review_hash("ide-x","head","a.rs","abcd",None,"new");
  assert_ne!(a,b);
 }
}
