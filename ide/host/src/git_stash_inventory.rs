//! FIN03: local stash *metadata* only, no content or Git mutation.
use crate::{git_branch_operations::exact_repo,IdeWorkspaceRegistry};
use serde::Serialize;
use git2::{Repository,RepositoryState};
#[derive(Serialize)]
#[serde(rename_all="camelCase")]
pub struct StashRow { pub index:usize,pub commit_sha:String }
#[derive(Serialize)]
#[serde(rename_all="camelCase")]
pub struct StashInventory {
 pub project_id:String,pub head_sha:String,pub entries:Vec<StashRow>,
 pub partial:bool,pub source_rechecked:bool,
 pub git_process_started:bool,pub refs_modified:bool,
 pub execution_authorized:bool,pub contents_read:bool,
}
fn stable_head(r:&Repository)->Result<String,&'static str>{
 if r.head_detached().unwrap_or(true) || r.state()!=RepositoryState::Clean{
     return Err("FIN03_STASH_REPO_BUSY");
 }
 let ref_=r.head().map_err(|_|"FIN03_HEAD_ABSENT")?;
 let oid=ref_.target().ok_or("FIN03_HEAD_NOT_DIRECT")?;
 r.find_commit(oid).map_err(|_|"FIN03_HEAD_NOT_COMMIT")?;
 Ok(oid.to_string())
}
impl IdeWorkspaceRegistry {
 pub fn read_local_stash_metadata(&self,project:&str)
    ->Result<StashInventory,&'static str>{
  let repo=exact_repo(self,project)?;
  let head=stable_head(&repo)?;
  let mut rows=Vec::new();let mut partial=false;
  if repo.find_reference("refs/stash").is_ok(){
   let log=repo.reflog("refs/stash").map_err(|_|"FIN03_STASH_REFLOG_UNREADABLE")?;
   if log.len()>128{return Err("FIN03_STASH_EVENT_LIMIT")}
   for i in 0..log.len(){
    if rows.len()==32{partial=true;break;}
    let event=log.get(i).ok_or("FIN03_STASH_EVENT_MISSING")?;
    let oid=event.id_new();
    repo.find_commit(oid).map_err(|_|"FIN03_STASH_RECORD_NOT_COMMIT")?;
    rows.push(StashRow{index:i,commit_sha:oid.to_string()});
   }
  }
  if head!=stable_head(&repo)?{
      return Err("FIN03_HEAD_CHANGED_DURING_STASH_READ");
  }
  Ok(StashInventory {
   project_id:project.to_owned(),head_sha:head,entries:rows,
   partial,source_rechecked:true,git_process_started:false,
   refs_modified:false,execution_authorized:false,contents_read:false,
  })
 }
}
