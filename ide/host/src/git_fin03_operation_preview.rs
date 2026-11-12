//! FIN-03G: conservative local Git revert and stash apply/drop impact preview.
//! This module is READ-ONLY. It never calls any Git index/worktree/ref update,
//! stash mutation, checkout, merge, hook, process or network API.
//! Path overlap is not a proven conflict forecast. A review SHA is not authority.
use crate::{git_branch_operations::{exact_repo,safe_local_name},IdeWorkspaceRegistry};
use git2::{Delta, ErrorCode, ObjectType, Oid, Repository, RepositoryState, StatusOptions, Tree};
use serde::{Deserialize,Serialize};
use sha2::{Digest,Sha256};
use std::{collections::BTreeSet,fs,path::Path};

const MAX_INDEX_BYTES:u64=8*1024*1024;
const MAX_STATUS:usize=512;
const MAX_PATHS:usize=256;
const MAX_VISIBLE:usize=48;
const MAX_PATH_BYTES:usize=1024;
const MAX_STASH_EVENTS:usize=128;

#[derive(Debug,Clone,Copy,Eq,PartialEq,Deserialize,Serialize)]
#[serde(rename_all="snake_case")]
pub enum Fin03ImpactKind { RevertCommit,StashApply,StashDrop }
impl Fin03ImpactKind {
 fn key(self)->&'static str {
  match self {
   Self::RevertCommit=>"revert_commit",
   Self::StashApply=>"stash_apply",
   Self::StashDrop=>"stash_drop",
  }
 }
}
#[derive(Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub struct Fin03ImpactReview {
 pub schema_version:u32,
 pub project_id:String,
 pub kind:Fin03ImpactKind,
 pub selected_commit:String,
 pub stash_index:Option<usize>,
 pub stash_ref_tip:Option<String>,
 pub current_head:String,
 pub current_branch:String,
 pub source_parent:String,
 pub source_tree:String,
 pub parent_tree:String,
 pub index_sha256:String,
 pub changed_paths_count:usize,
 pub visible_paths:Vec<String>,
 pub omitted_paths_count:usize,
 pub unsupported_delta_count:usize,
 pub possible_overlap_count:usize,
 pub visible_overlaps:Vec<String>,
 pub working_tree_dirty_count:usize,
 pub index_conflicts:bool,
 pub blockers:Vec<&'static str>,
 pub risk_assessment:&'static str,
 pub evidence_sha256:String,
 pub original_git_objects_preserved:bool,
 pub index_or_worktree_modified:bool,
 pub stash_or_branch_ref_modified:bool,
 pub git_process_started:bool,
 pub network_queried:bool,
 pub conflict_resolution_performed:bool,
 pub result_tree_verified:bool,
 pub execution_route_available:bool,
 pub execution_authorized:bool,
 pub source_rechecked:bool,
}
#[derive(Clone,Eq,PartialEq)]
struct Fence {
 head:Oid,
 branch:String,
 index_sha:String,
 stash_tip:Option<Oid>,
 dirty:usize,
 conflicts:bool,
 state:RepositoryState,
}
fn head_and_index(repo:&Repository)->Result<Fence,&'static str>{
 if repo.is_bare() || repo.is_worktree() ||
    repo.head_detached().unwrap_or(true) || repo.is_shallow(){
  return Err("FIN03G_NONSTANDARD_OR_SHALLOW_HEAD");
 }
 let ref_=repo.head().map_err(|_|"FIN03G_HEAD_UNAVAILABLE")?;
 let name=ref_.name().and_then(|x|x.strip_prefix("refs/heads/"))
  .ok_or("FIN03G_NONLOCAL_HEAD")?;
 if !safe_local_name(name){return Err("FIN03G_UNSAFE_BRANCH");}
 let head=ref_.target().ok_or("FIN03G_SYMBOLIC_HEAD")?;
 repo.find_commit(head).map_err(|_|"FIN03G_HEAD_NOT_COMMIT")?;
 let index_path=repo.path().join("index");
 let meta=fs::symlink_metadata(&index_path)
  .map_err(|_|"FIN03G_INDEX_UNAVAILABLE")?;
 if !meta.is_file() || meta.file_type().is_symlink() ||
     meta.len()==0 || meta.len()>MAX_INDEX_BYTES {
  return Err("FIN03G_INDEX_UNTRUSTED");
 }
 if repo.path().join("index.lock").exists(){
  return Err("FIN03G_FOREIGN_INDEX_LOCK");
 }
 let bytes=fs::read(&index_path).map_err(|_|"FIN03G_INDEX_UNREADABLE")?;
 if bytes.len() as u64!=meta.len(){return Err("FIN03G_INDEX_CHANGED");}
 let sha=format!("{:x}",Sha256::digest(bytes));
 let index=repo.index().map_err(|_|"FIN03G_INDEX_PARSE_FAILED")?;
 let conflicts=index.has_conflicts();
 let mut options=StatusOptions::new();
 options.include_untracked(true).recurse_untracked_dirs(true)
   .include_ignored(false).renames_head_to_index(false)
   .renames_index_to_workdir(false);
 let statuses=repo.statuses(Some(&mut options))
  .map_err(|_|"FIN03G_STATUS_UNVERIFIABLE")?;
 if statuses.len()>MAX_STATUS{return Err("FIN03G_DIRTY_STATUS_LIMIT");}
 let stash_tip=match repo.find_reference("refs/stash") {
  Ok(reference)=>Some(reference.target().ok_or("FIN03G_STASH_INDIRECT")?),
  Err(e) if e.code()==ErrorCode::NotFound=>None,
  Err(_)=>return Err("FIN03G_STASH_REF_UNVERIFIABLE"),
 };
 Ok(Fence{head,branch:name.to_owned(),index_sha:sha,
  stash_tip,dirty:statuses.len(),conflicts,state:repo.state()})
}
fn checked_path(path:&Path)->Result<String,&'static str>{
 let text=path.to_str().ok_or("FIN03G_NON_UTF8_PATH")?;
 if text.is_empty() || text.len()>MAX_PATH_BYTES ||
    text.starts_with('/') || text.contains('\\') || text.contains(':') ||
    text.chars().any(char::is_control) ||
    text.split('/').any(|x|x.is_empty()||x=="."||x=="..") {
  return Err("FIN03G_UNSAFE_GIT_PATH");
 }
 Ok(text.into())
}
fn ordinary_entry(tree:&Tree,path:&str)->bool{
 match tree.get_path(Path::new(path)) {
  Ok(entry)=>matches!(entry.kind(),Some(ObjectType::Blob)) &&
    matches!(entry.filemode(),0o100644|0o100755),
  Err(_)=>false,
 }
}
struct PathEvidence {paths:BTreeSet<String>,unsupported:usize,ledger_sha:String}
fn delta_paths(repo:&Repository,before:&Tree,after:&Tree)
 ->Result<PathEvidence,&'static str>{
 let diff=repo.diff_tree_to_tree(Some(before),Some(after),None)
  .map_err(|_|"FIN03G_LOCAL_TREE_DIFF_UNAVAILABLE")?;
 if diff.deltas().len()>MAX_PATHS{return Err("FIN03G_DIFF_PATH_BUDGET");}
 let mut paths=BTreeSet::new();
 let mut unsupported=0usize;
 let mut h=Sha256::new();
 h.update(b"lu-ide-fin03g-local-diff-v1\n");
 for delta in diff.deltas(){
  let kind=delta.status();
  if !matches!(kind,Delta::Added|Delta::Deleted|Delta::Modified){
   unsupported+=1;
  }
  let old=delta.old_file().path().map(checked_path).transpose()?;
  let new=delta.new_file().path().map(checked_path).transpose()?;
  if old.is_none() && new.is_none(){return Err("FIN03G_PATH_MISSING");}
  for value in [format!("{:?}",kind),
   old.clone().unwrap_or_default(),new.clone().unwrap_or_default(),
   delta.old_file().id().to_string(),delta.new_file().id().to_string()]{
   h.update((value.len() as u64).to_le_bytes());
   h.update(value.as_bytes());
  }
  for candidate in [old,new].into_iter().flatten(){
   if !ordinary_entry(before,&candidate) &&
      !ordinary_entry(after,&candidate) {unsupported+=1;}
   paths.insert(candidate);
  }
  if paths.len()>MAX_PATHS{return Err("FIN03G_DIFF_PATH_BUDGET");}
 }
 Ok(PathEvidence{paths,unsupported,
  ledger_sha:format!("{:x}",h.finalize())})
}
fn hash_parts(parts:&[&str])->String{
 let mut h=Sha256::new();
 h.update(b"lu-ide-fin03g-review-only-v1\n");
 for text in parts {
  h.update((text.len() as u64).to_le_bytes());
  h.update(text.as_bytes());
 }
 format!("{:x}",h.finalize())
}
impl IdeWorkspaceRegistry {
 /// Review *exactly* one local commit or an existing stash reflog entry.
 /// This cannot authorize a revert, stash apply/pop/drop, or any Git write.
 pub fn review_fin03_local_impact(
  &self,project_id:&str,kind:Fin03ImpactKind,
  commit_sha:Option<&str>,stash_index:Option<usize>
 )->Result<Fin03ImpactReview,&'static str>{
  let repo=exact_repo(self,project_id)?;
  let before=head_and_index(&repo)?;
  let (selected,stash_index)=match kind{
   Fin03ImpactKind::RevertCommit=>{
    if stash_index.is_some(){return Err("FIN03G_UNEXPECTED_STASH_SELECTOR");}
    let sha=commit_sha.ok_or("FIN03G_REVERT_COMMIT_REQUIRED")?;
    if sha.len()!=40 || !sha.bytes().all(|b|
       b.is_ascii_hexdigit() && !b.is_ascii_uppercase()){
     return Err("FIN03G_UNTRUSTED_COMMIT_SELECTOR");
    }
    let id=Oid::from_str(sha).map_err(|_|"FIN03G_COMMIT_INVALID")?;
    if id!=before.head &&
       !repo.graph_descendant_of(before.head,id)
         .map_err(|_|"FIN03G_ANCESTRY_UNVERIFIED")?{
     return Err("FIN03G_COMMIT_NOT_IN_CURRENT_HISTORY");
    }
    (id,None)
   },
   Fin03ImpactKind::StashApply|Fin03ImpactKind::StashDrop=>{
    if commit_sha.is_some(){return Err("FIN03G_UNEXPECTED_COMMIT_SELECTOR");}
    let slot=stash_index.ok_or("FIN03G_STASH_INDEX_REQUIRED")?;
    if slot>=32{return Err("FIN03G_STASH_SELECTION_LIMIT");}
    let tip=before.stash_tip.ok_or("FIN03G_STASH_MISSING")?;
    let reflog=repo.reflog("refs/stash")
     .map_err(|_|"FIN03G_STASH_REFLOG_UNAVAILABLE")?;
    if reflog.len()>MAX_STASH_EVENTS{return Err("FIN03G_STASH_REFLOG_LIMIT");}
    let top=reflog.get(0).ok_or("FIN03G_STASH_TOP_MISSING")?;
    if top.id_new()!=tip{return Err("FIN03G_STASH_TIP_DISAGREES_WITH_LOG");}
    let event=reflog.get(slot).ok_or("FIN03G_STASH_INDEX_OUT_OF_RANGE")?;
    (event.id_new(),Some(slot))
   }
  };
  let commit=repo.find_commit(selected).map_err(|_|"FIN03G_SOURCE_NOT_COMMIT")?;
  if match kind{
   Fin03ImpactKind::RevertCommit=>commit.parent_count()!=1,
   _=> !(2..=3).contains(&commit.parent_count())
  }{return Err("FIN03G_UNSUPPORTED_PARENT_SHAPE");}
  let parent=commit.parent(0).map_err(|_|"FIN03G_SOURCE_PARENT_MISSING")?;
  let parent_tree=parent.tree().map_err(|_|"FIN03G_PARENT_TREE_MISSING")?;
  let source_tree=commit.tree().map_err(|_|"FIN03G_SOURCE_TREE_MISSING")?;
  let impact=delta_paths(&repo,&parent_tree,&source_tree)?;
  let head=repo.find_commit(before.head).map_err(|_|"FIN03G_HEAD_DISAPPEARED")?;
  let head_tree=head.tree().map_err(|_|"FIN03G_HEAD_TREE_MISSING")?;
  // Revert conflicts concern edits made *after* the selected commit.
  // Comparing its parent to HEAD would falsely count the selected commit's
  // own changes as an overlap, even when reverting the current tip.
  // Stash apply instead compares the saved stash base with today's HEAD.
  let downstream_base=if kind==Fin03ImpactKind::RevertCommit {
   &source_tree
  } else {
   &parent_tree
  };
  let downstream=delta_paths(&repo,downstream_base,&head_tree)?;
  let overlaps:Vec<String>=impact.paths.intersection(&downstream.paths).cloned().collect();
  let visible_paths=impact.paths.iter().take(MAX_VISIBLE).cloned().collect();
  let visible_overlaps=overlaps.iter().take(MAX_VISIBLE).cloned().collect();
  let mut blockers=Vec::new();
  if before.state!=RepositoryState::Clean {
   blockers.push("git_operation_in_progress");
  }
  if before.conflicts{blockers.push("index_conflicts");}
  if before.dirty>0{blockers.push("worktree_or_index_dirty");}
  if repo.path().join("lu-ide-fin03-restore-active.json").exists() ||
     repo.path().join("lu-ide-fin03-stash-active").exists(){
    blockers.push("another_fin03_mutation_unreconciled");
  }
  if impact.unsupported>0{blockers.push("unsafe_or_nonregular_git_delta");}
  if impact.paths.is_empty(){blockers.push("empty_original_change");}
  if !overlaps.is_empty(){blockers.push("possible_path_overlap");}
  if kind==Fin03ImpactKind::StashDrop {
   blockers.push("stash_drop_would_discard_saved_reference");
  }
  if kind!=Fin03ImpactKind::RevertCommit && commit.parent_count()==3 {
   blockers.push("stash_untracked_parent_not_previewed");
  }
  blockers.push("resulting_tree_and_conflicts_not_verified");
  blockers.push("fin01_kernel_acceptance_required_before_mutation");
  let after=head_and_index(&repo)?;
  if before!=after{return Err("FIN03G_SOURCE_CHANGED_DURING_PREVIEW");}
  if kind!=Fin03ImpactKind::RevertCommit {
   let tip=after.stash_tip.ok_or("FIN03G_STASH_CHANGED")?;
   let log=repo.reflog("refs/stash")
    .map_err(|_|"FIN03G_STASH_REFLOG_CHANGED")?;
   if log.get(0).map(|x|x.id_new())!=Some(tip) ||
      log.get(stash_index.unwrap_or(0)).map(|x|x.id_new())!=Some(selected){
    return Err("FIN03G_SELECTED_STASH_CHANGED");
   }
  }
  let current_head=before.head.to_string();
  let selected_commit=selected.to_string();
  let parent_oid=parent.id().to_string();
  let source_oid=source_tree.id().to_string();
  let parent_tree_oid=parent_tree.id().to_string();
  let stash_tip=before.stash_tip.map(|x|x.to_string());
  let slot=stash_index.map(|x|x.to_string()).unwrap_or_default();
  let overlap_list=overlaps.join("\n");
  let evidence_sha256=hash_parts(&[
   project_id,kind.key(),&before.branch,&current_head,&before.index_sha,
   &selected_commit,&parent_oid,&source_oid,&parent_tree_oid,
   stash_tip.as_deref().unwrap_or(""),&slot,&impact.ledger_sha,&overlap_list,
   &before.dirty.to_string(),&before.conflicts.to_string()
  ]);
  Ok(Fin03ImpactReview{
   schema_version:1,project_id:project_id.to_owned(),kind,
   selected_commit,stash_index,stash_ref_tip:stash_tip,
   current_head,current_branch:before.branch,
   source_parent:parent_oid,source_tree:source_oid,parent_tree:parent_tree_oid,
   index_sha256:before.index_sha,
   changed_paths_count:impact.paths.len(),visible_paths,
   omitted_paths_count:impact.paths.len().saturating_sub(MAX_VISIBLE),
   unsupported_delta_count:impact.unsupported,
   possible_overlap_count:overlaps.len(),visible_overlaps,
   working_tree_dirty_count:before.dirty,index_conflicts:before.conflicts,
   blockers,risk_assessment:"path_overlap_only_not_conflict_or_result_tree_proof",
   evidence_sha256,original_git_objects_preserved:true,
   index_or_worktree_modified:false,stash_or_branch_ref_modified:false,
   git_process_started:false,network_queried:false,
   conflict_resolution_performed:false,result_tree_verified:false,
   execution_route_available:false,execution_authorized:false,
   source_rechecked:true,
  })
 }
}

#[cfg(test)]
mod tests {
 use super::*;
 use crate::{IdeError,NativeDirectoryPicker};
 use git2::{Repository,Signature};
 use std::{fs,path::{Path,PathBuf}};
 struct Picker(PathBuf);
 impl NativeDirectoryPicker for Picker {
  fn choose_project_directory(&mut self)->Result<Option<PathBuf>,IdeError>{
   Ok(Some(self.0.clone()))
  }
 }
 struct Fixture {
  home:PathBuf,root:PathBuf,registry:IdeWorkspaceRegistry,
  id:String,base:String,tip:String,
 }
 impl Drop for Fixture {
  fn drop(&mut self){let _=fs::remove_dir_all(&self.home);}
 }
 fn fixture()->Fixture{
  let home=std::env::temp_dir().join(
    format!("fin03g-impact-{:032x}",rand::random::<u128>()));
  let root=home.join("project");
  fs::create_dir_all(&root).unwrap();
  let repo=Repository::init(&root).unwrap();
  fs::write(root.join("tracked.txt"),b"original\n").unwrap();
  fs::write(root.join("untouched.txt"),b"leave untouched\n").unwrap();
  let mut index=repo.index().unwrap();
  for p in ["tracked.txt","untouched.txt"]{index.add_path(Path::new(p)).unwrap();}
  index.write().unwrap();
  let sig=Signature::now("fixture","fixture@example.invalid").unwrap();
  let base_tree=repo.find_tree(index.write_tree().unwrap()).unwrap();
  let base=repo.commit(Some("HEAD"),&sig,&sig,"base",&base_tree,&[]).unwrap();
  drop(base_tree);
  fs::write(root.join("tracked.txt"),b"modified\n").unwrap();
  index.add_path(Path::new("tracked.txt")).unwrap();
  index.write().unwrap();
  let next_tree=repo.find_tree(index.write_tree().unwrap()).unwrap();
  let prev=repo.find_commit(base).unwrap();
  let tip=repo.commit(Some("HEAD"),&sig,&sig,"modified",&next_tree,&[&prev]).unwrap();
  drop(prev);drop(next_tree);drop(index);drop(repo);
  let registry=IdeWorkspaceRegistry::open(home.join("catalog.json")).unwrap();
  let id=registry.register_from_native_picker(&mut Picker(root.clone()))
   .unwrap().unwrap().project_id;
  Fixture{home,root,registry,id,base:base.to_string(),tip:tip.to_string()}
 }
 fn original(f:&Fixture)->(String,Vec<u8>,Vec<u8>,Option<String>){
  let repo=Repository::open(&f.root).unwrap();
  let head=repo.head().unwrap().target().unwrap().to_string();
  let stash=repo.find_reference("refs/stash").ok()
    .and_then(|ref_|ref_.target().map(|id|id.to_string()));
  (head,fs::read(f.root.join("tracked.txt")).unwrap(),
   fs::read(f.root.join(".git/index")).unwrap(),stash)
 }
 #[test]
 fn revert_tip_review_never_mutates_original_repo(){
  let f=fixture();let before=original(&f);
  let review=f.registry.review_fin03_local_impact(
   &f.id,Fin03ImpactKind::RevertCommit,Some(&f.tip),None).unwrap();
  assert_eq!(review.selected_commit,f.tip);
  assert_eq!(review.changed_paths_count,1);
  assert_eq!(review.visible_paths,vec!["tracked.txt"]);
  assert_eq!(review.possible_overlap_count,0);
  assert_eq!(review.index_or_worktree_modified,false);
  assert_eq!(review.stash_or_branch_ref_modified,false);
  assert_eq!(review.execution_route_available,false);
  assert_eq!(review.execution_authorized,false);
  assert_eq!(review.evidence_sha256.len(),64);
  assert!(review.blockers.contains(&"resulting_tree_and_conflicts_not_verified"));
  assert_eq!(original(&f),before);
  let again=f.registry.review_fin03_local_impact(
   &f.id,Fin03ImpactKind::RevertCommit,Some(&f.tip),None).unwrap();
  assert_eq!(review.evidence_sha256,again.evidence_sha256);
  assert_eq!(original(&f),before);
 }
 #[test]
 fn stash_apply_and_drop_preview_no_mutation_or_content_exposure(){
  let f=fixture();
  fs::write(f.root.join("tracked.txt"),b"stashed change\n").unwrap();
  let mut repo=Repository::open(&f.root).unwrap();
  let sig=Signature::now("fixture","fixture@example.invalid").unwrap();
  let stash_oid=repo.stash_save(&sig,"fixture stash",None).unwrap();
  drop(repo);
  let before=original(&f);
  for operation in [Fin03ImpactKind::StashApply,Fin03ImpactKind::StashDrop]{
   let row=f.registry.review_fin03_local_impact(
    &f.id,operation,None,Some(0)).unwrap();
   assert_eq!(row.selected_commit,stash_oid.to_string());
   assert_eq!(row.stash_ref_tip.as_deref(),Some(stash_oid.to_string().as_str()));
   assert!(row.changed_paths_count>=1);
   assert!(!row.execution_route_available && !row.execution_authorized);
   assert!(!row.result_tree_verified);
   assert!(!row.stash_or_branch_ref_modified);
   assert_eq!(original(&f),before);
  }
 }
 #[test]
 fn invalid_selectors_and_uncommitted_state_are_observations_not_authority(){
  let f=fixture();let before=original(&f);
  assert!(f.registry.review_fin03_local_impact(
   &f.id,Fin03ImpactKind::RevertCommit,Some("HEAD"),None).is_err());
  assert!(f.registry.review_fin03_local_impact(
   &f.id,Fin03ImpactKind::RevertCommit,Some(&f.base),Some(0)).is_err());
  assert!(f.registry.review_fin03_local_impact(
   &f.id,Fin03ImpactKind::StashApply,Some(&f.tip),Some(0)).is_err());
  assert!(f.registry.review_fin03_local_impact(
   &f.id,Fin03ImpactKind::StashDrop,None,Some(32)).is_err());
  fs::write(f.root.join("untracked.tmp"),b"untracked private\n").unwrap();
  let review=f.registry.review_fin03_local_impact(
   &f.id,Fin03ImpactKind::RevertCommit,Some(&f.tip),None).unwrap();
  assert!(review.blockers.contains(&"worktree_or_index_dirty"));
  assert!(!review.execution_authorized);
  fs::remove_file(f.root.join("untracked.tmp")).unwrap();
  assert_eq!(original(&f),before);
 }
}
