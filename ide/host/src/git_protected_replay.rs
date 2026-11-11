//! IDE 6C-04C/D: one-use native-OS-authorized cherry-pick/rebase publication.
//!
//! The user review returns a digest, NEVER an authorization token. Only the
//! Tauri host's separate blocking native OS dialog may call execute_approved.
//! Objects are computed in an isolated bare repo; after approval only, they
//! are copied to source ODB, followed by safe checkout and an exact HEAD CAS.
//! A protected original-tip ref is created before ANY rebase history rewrite.
//! All interrupted states remain manual-reconciliation-only, not auto-retry.
use crate::{
    git_branch_operations::{exact_repo, local_commit},
    git_rebase_cherry_plan::ReplayReviewKind,
    git_replay_quarantine::{compute_in_sandbox, ProspectiveReplayResult, ReplaySandbox},
    IdeWorkspaceRegistry,
};
use git2::{build::CheckoutBuilder, ObjectType, Oid, Repository, RepositoryState, StatusOptions};
use serde::{Deserialize,Serialize};
use sha2::{Digest,Sha256};
use std::{
    collections::BTreeSet,
    fs::{self,File,OpenOptions}, io::Write,
    path::{Path,PathBuf},
    sync::atomic::{AtomicU8,Ordering},
};
const MAX_JOURNAL: u64=1024*1024;
const MAX_ENTRY: usize=4096;
const MAX_INDEX: u64=8*1024*1024;
const MAX_NEW_OBJECT: usize=16*1024*1024;
const MAX_OBJECTS: usize=256;
const MAX_OUTPUT_PATHS:usize=80;

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct ReplayJournal {
    schema_version:u32,
    operation_id:String,
    project_id:String,
    operation:ReplayReviewKind,
    phase:String,
    branch_ref:String,
    original_head:String,
    original_tree:String,
    selected_local_branch:String,
    selected_tip:String,
    candidate_head:String,
    candidate_tree:String,
    original_index_sha256:String,
    candidate_index_sha256:String,
    result_sha256:String,
    preserved_original_ref:Option<String>,
}
#[derive(Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub struct ApprovedReplayReceipt {
    pub schema_version:u32,
    pub project_id:String,
    pub operation:ReplayReviewKind,
    pub operation_id:String,
    pub original_head:String,
    pub published_head:String,
    pub resulting_tree:String,
    pub prior_branch_ref:String,
    pub original_history_ref:Option<String>,
    pub final_index_sha256:String,
    pub result_sha256:String,
    pub original_objects_preserved:bool,
    pub independently_verified:bool,
    pub exact_ref_cas:true_like_bool,
    pub one_use_native_os_consent:true_like_bool,
    pub journal_synced:bool,
    pub git_hooks_executed:bool,
    pub remote_updated:bool,
}
type true_like_bool=bool;

#[derive(Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub struct ReplayRecovery {
    pub schema_version:u32,
    pub project_id:String,
    pub operation:Option<ReplayReviewKind>,
    pub state:&'static str,
    pub last_phase:String,
    pub original_head:String,
    pub candidate_head:String,
    pub branch_matches_original:bool,
    pub branch_matches_candidate:bool,
    pub backup_ref_verified:bool,
    pub original_index_matches:bool,
    pub candidate_index_matches:bool,
    pub worktree_matches_index:bool,
    pub index_lock_present:bool,
    pub operator_reconciliation_required:bool,
    pub automatic_ref_retry:bool,
    pub automatic_history_rewrite:bool,
    pub automatic_checkout:bool,
    pub automatic_rollback:bool,
    pub automatic_foreign_lock_deletion:bool,
}

fn sha(bytes:&[u8])->String {format!("{:x}",Sha256::digest(bytes))}
fn index_sha(dir:&Path)->Result<String,&'static str>{
    let path=dir.join("index");
    let meta=fs::symlink_metadata(&path).map_err(|_|"G06C04_INDEX_UNAVAILABLE")?;
    if !meta.is_file()||meta.file_type().is_symlink()||meta.len()>MAX_INDEX {
        return Err("G06C04_INDEX_UNTRUSTED");
    }
    let bytes=fs::read(&path).map_err(|_|"G06C04_INDEX_READ_FAILED")?;
    if bytes.len() as u64!=meta.len(){return Err("G06C04_INDEX_CHANGED_DURING_READ");}
    Ok(sha(&bytes))
}
fn clean_tree(repo:&Repository,tree_oid:&str)->Result<bool,&'static str>{
    let mut idx=repo.index().map_err(|_|"G06C04_INDEX_UNAVAILABLE")?;
    idx.read(true).map_err(|_|"G06C04_INDEX_RELOAD_FAILED")?;
    if idx.has_conflicts(){return Ok(false);}
    let written=idx.write_tree_to(repo).map_err(|_|"G06C04_INDEX_TREE_FAILED")?;
    if written.to_string()!=tree_oid {return Ok(false);}
    let mut opts=StatusOptions::new();
    opts.include_untracked(true).recurse_untracked_dirs(true).include_ignored(false);
    Ok(repo.statuses(Some(&mut opts))
        .map_err(|_|"G06C04_STATUS_FAILED")?.is_empty())
}
fn worktree_matches_index(repo:&Repository)->Result<bool,&'static str>{
    let mut opts=StatusOptions::new();
    opts.include_untracked(true).recurse_untracked_dirs(true).include_ignored(false);
    let s=repo.statuses(Some(&mut opts)).map_err(|_|"G06C04_WT_STATUS_FAILED")?;
    Ok(s.iter().all(|x|!x.status().intersects(
        git2::Status::WT_MODIFIED|git2::Status::WT_DELETED|
        git2::Status::WT_NEW|git2::Status::WT_RENAMED|
        git2::Status::WT_TYPECHANGE|git2::Status::CONFLICTED
    )))
}
fn exact_branch(repo:&Repository)->Result<(String,Oid),&'static str>{
    if repo.is_shallow()||repo.state()!=RepositoryState::Clean||
        repo.head_detached().unwrap_or(true) {
        return Err("G06C04_BRANCH_REQUIRES_CLEAN_STATE");
    }
    let head=repo.head().map_err(|_|"G06C04_HEAD_UNAVAILABLE")?;
    let name=head.name().ok_or("G06C04_HEAD_REF_UNAVAILABLE")?;
    let branch=name.strip_prefix("refs/heads/").ok_or("G06C04_HEAD_NOT_LOCAL")?;
    if !crate::git_branch_operations::safe_local_name(branch) {
        return Err("G06C04_CURRENT_BRANCH_UNSAFE");
    }
    Ok((name.to_owned(),head.target().ok_or("G06C04_HEAD_NOT_DIRECT")?))
}
fn journal_path(reg:&IdeWorkspaceRegistry,id:&str)->Result<PathBuf,&'static str>{
    reg.registered_git_root(id).map_err(|_|"G06C04_UNKNOWN_PROJECT")?;
    if id.is_empty()||id.len()>128||!id.bytes().all(|b|b.is_ascii_alphanumeric()||b=='-' as u8||b=='_' as u8) {
        return Err("G06C04_JOURNAL_PROJECT_INVALID");
    }
    let root=reg.catalog.parent().ok_or("G06C04_NO_APPDATA")?
        .join("ide-g06c04-protected-replay");
    fs::create_dir_all(&root).map_err(|_|"G06C04_JOURNAL_ROOT_FAILED")?;
    let m=fs::symlink_metadata(&root).map_err(|_|"G06C04_JOURNAL_ROOT_UNREADABLE")?;
    if !m.is_dir()||m.file_type().is_symlink() {
        return Err("G06C04_JOURNAL_ROOT_UNTRUSTED");
    }
    Ok(root.join(format!("{id}.jsonl")))
}
fn read_last(path:&Path)->Result<Option<ReplayJournal>,&'static str>{
    let meta=match fs::symlink_metadata(path){
        Ok(x)=>x,
        Err(e) if e.kind()==std::io::ErrorKind::NotFound=>return Ok(None),
        Err(_)=>return Err("G06C04_JOURNAL_READ_FAILED")
    };
    if meta.file_type().is_symlink()||!meta.is_file()||meta.len()>MAX_JOURNAL {
        return Err("G06C04_JOURNAL_UNTRUSTED");
    }
    let bytes=fs::read(path).map_err(|_|"G06C04_JOURNAL_READ_FAILED")?;
    if bytes.last()!=Some(&b'\n'){return Err("G06C04_JOURNAL_INCOMPLETE");}
    let entry=bytes.split(|x|*x==b'\n').filter(|x|!x.is_empty()).last()
        .ok_or("G06C04_JOURNAL_EMPTY")?;
    if entry.len()>MAX_ENTRY{return Err("G06C04_JOURNAL_ENTRY_OVERSIZED");}
    let j:ReplayJournal=serde_json::from_slice(entry)
        .map_err(|_|"G06C04_JOURNAL_CORRUPTED")?;
    if j.schema_version!=1||j.operation_id.len()!=32||
        !j.operation_id.bytes().all(|b|b.is_ascii_hexdigit())||
        !matches!(j.phase.as_str(),"prepared"|"objects_intent"|"objects_ready"|
            "checkout_intent"|"checkout_verified"|"publishing"|
            "published"|"verified"|"cancelled"){
        return Err("G06C04_JOURNAL_RECORD_INVALID");
    }
    Ok(Some(j))
}
fn append(path:&Path,j:&ReplayJournal)->Result<(),&'static str>{
    let previous=match fs::symlink_metadata(path) {
        Ok(x) if x.is_file()&&!x.file_type().is_symlink()=>x.len(),
        Err(e) if e.kind()==std::io::ErrorKind::NotFound=>0,
        _=>return Err("G06C04_JOURNAL_UNTRUSTED")
    };
    let bytes=serde_json::to_vec(j).map_err(|_|"G06C04_JOURNAL_SERIALIZATION")?;
    if bytes.len()>MAX_ENTRY||previous+bytes.len() as u64+1>MAX_JOURNAL {
        return Err("G06C04_JOURNAL_CAP_REACHED");
    }
    let mut file=OpenOptions::new().append(true).create(true).open(path)
        .map_err(|_|"G06C04_JOURNAL_APPEND_DENIED")?;
    if file.metadata().map_err(|_|"G06C04_JOURNAL_STAT_FAILED")?.len()!=previous {
        return Err("G06C04_JOURNAL_CONCURRENT_CHANGE");
    }
    file.write_all(&bytes).and_then(|_|file.write_all(b"\n"))
        .and_then(|_|file.sync_all())
        .map_err(|_|"G06C04_JOURNAL_FSYNC_FAILED")?;
    #[cfg(unix)]
    File::open(path.parent().ok_or("G06C04_JOURNAL_PARENT_MISSING")?)
        .and_then(|f|f.sync_all())
        .map_err(|_|"G06C04_JOURNAL_DIR_SYNC_FAILED")?;
    Ok(())
}
fn ref_oid(repo:&Repository,name:&str)->Result<String,&'static str>{
    Ok(repo.find_reference(name).map_err(|_|"G06C04_REF_NOT_FOUND")?
        .target().ok_or("G06C04_REF_NOT_DIRECT")?.to_string())
}
fn candidate_is_authorizable(
    result:&ProspectiveReplayResult, digest:&str,
)->Result<(),&'static str>{
    if result.evidence_sha256!=digest {
        return Err("G06C04_REVIEW_STALE_OR_FORGED");
    }
    if result.prospective_head.is_none()||result.prospective_tree.is_none()||
        result.result_steps.is_empty()||result.result_steps.len()>16||
        !result.blockers.is_empty()||result.conflict_count!=0||
        result.candidate_change_paths==0||result.candidate_change_paths>MAX_OUTPUT_PATHS||
        !result.quarantine_only_result_objects ||
        !result.original_project_unchanged{
        return Err("G06C04_REPLAY_CANNOT_BE_AUTHORIZED");
    }
    Ok(())
}
pub(crate) fn copy_quarantine_objects(
    source:&Repository,scratch:&Repository,result:&ProspectiveReplayResult
)->Result<(),&'static str>{
    let src=source.odb().map_err(|_|"G06C04_SOURCE_ODB_FAILED")?;
    let alt=scratch.odb().map_err(|_|"G06C04_SCRATCH_ODB_FAILED")?;
    let mut seen=BTreeSet::new();
    fn copy_one(
        id:Oid,kind:ObjectType,src:&git2::Odb<'_>,
        alt:&git2::Odb<'_>,seen:&mut BTreeSet<Oid>,level:u32
    )->Result<(),&'static str>{
        if level>32||seen.len()>=MAX_OBJECTS{return Err("G06C04_OBJECT_LIMIT");}
        if !seen.insert(id){return Ok(());}
        if src.exists(id){return Ok(());}
        let obj=alt.read(id).map_err(|_|"G06C04_QUARANTINE_OBJECT_MISSING")?;
        if obj.kind()!=kind || obj.len()>MAX_NEW_OBJECT {
            return Err("G06C04_OBJECT_KIND_SIZE_MISMATCH");
        }
        if kind==ObjectType::Tree {
            // Walk every descendant *before* writing root tree object.
            let bytes=obj.data();
            let mut off=0usize;
            while off<bytes.len(){
                let space=bytes[off..].iter().position(|x|*x==b' ')
                    .ok_or("G06C04_TREE_MODE_INVALID")?+off;
                let null=bytes[space+1..].iter().position(|x|*x==0)
                    .ok_or("G06C04_TREE_PATH_INVALID")?+space+1;
                let mode=std::str::from_utf8(&bytes[off..space])
                    .map_err(|_|"G06C04_TREE_MODE_NONUTF8")?;
                let start=null+1;
                if start+20>bytes.len(){return Err("G06C04_TREE_ENTRY_TRUNCATED");}
                let child=Oid::from_bytes(&bytes[start..start+20])
                    .map_err(|_|"G06C04_TREE_CHILD_OID_INVALID")?;
                let child_kind=match mode {
                    "40000"|"040000"=>ObjectType::Tree,
                    "100644"|"100755"=>ObjectType::Blob,
                    _=>return Err("G06C04_UNSAFE_TREE_ENTRY_MODE")
                };
                copy_one(child,child_kind,src,alt,seen,level+1)?;
                off=start+20;
            }
        }
        let new=src.write(kind,obj.data()).map_err(|_|"G06C04_SOURCE_OBJECT_WRITE_FAILED")?;
        if new!=id || !src.exists(id){return Err("G06C04_SOURCE_OBJECT_READBACK_FAILED");}
        Ok(())
    }
    // Original commits' blob/tree object dependencies are unchanged and are
    // already reachable in source. Only scratch-generated tree and commit IDs
    // need copying; parent chain copied in original->new order.
    for step in &result.result_steps {
        let tree=Oid::from_str(&step.resulting_tree).map_err(|_|"G06C04_TREE_OID_INVALID")?;
        copy_one(tree,ObjectType::Tree,&src,&alt,&mut seen,0)?;
        let commit=Oid::from_str(&step.resulting_commit)
            .map_err(|_|"G06C04_COMMIT_OID_INVALID")?;
        copy_one(commit,ObjectType::Commit,&src,&alt,&mut seen,0)?;
    }
    Ok(())
}
fn ensure_changed_paths_safe(
    root:&Path,repo:&Repository,
    old_tree:&str,new_tree:&str
)->Result<(),&'static str>{
    let old=repo.find_tree(Oid::from_str(old_tree).map_err(|_|"G06C04_OLD_TREE_INVALID")?)
        .map_err(|_|"G06C04_OLD_TREE_MISSING")?;
    let new=repo.find_tree(Oid::from_str(new_tree).map_err(|_|"G06C04_NEW_TREE_INVALID")?)
        .map_err(|_|"G06C04_NEW_TREE_MISSING")?;
    let diff=repo.diff_tree_to_tree(Some(&old),Some(&new),None)
        .map_err(|_|"G06C04_PATH_DIFF_UNAVAILABLE")?;
    if diff.deltas().len()==0||diff.deltas().len()>MAX_OUTPUT_PATHS {
        return Err("G06C04_OUTPUT_PATHS_UNSUPPORTED");
    }
    for d in diff.deltas() {
        if !matches!(d.status(),git2::Delta::Added|git2::Delta::Deleted|git2::Delta::Modified){
            return Err("G06C04_UNSAFE_TREE_DELTA");
        }
        for path in [d.old_file().path(),d.new_file().path()].into_iter().flatten(){
            let s=path.to_str().ok_or("G06C04_NONUTF8_SOURCE_PATH")?;
            crate::git_protected_merge::changed_path_is_plain_safe(root,s)?;
        }
    }
    Ok(())
}

impl IdeWorkspaceRegistry {
    /// Rebuild source-bound results in disposable scratch before OS dialog.
    pub fn revalidate_protected_replay(
        &self,id:&str,kind:ReplayReviewKind,branch:&str,digest:&str
    )->Result<ProspectiveReplayResult,&'static str>{
        let candidate=self.preview_prospective_replay(id,kind,branch)?;
        candidate_is_authorizable(&candidate,digest)?;
        Ok(candidate)
    }

    /// SECURITY: This method is *host-internal after a distinct native OS
    /// prompt*; no WebView can grant itself a call to it. Rebase also requires
    /// explicit history rewrite wording in the native OS prompt.
    pub fn execute_os_approved_replay(
        &self,id:&str,kind:ReplayReviewKind,branch:&str,
        digest:&str,cancel:&AtomicU8
    )->Result<ApprovedReplayReceipt,&'static str>{
        if cancel.load(Ordering::SeqCst)!=0{return Err("G06C04_CANCELLED_BEFORE_REVIEW");}
        let original=self.review_local_rebase_cherry_pick(id,kind,branch)?;
        let repo=exact_repo(self,id)?;
        let build=compute_in_sandbox(&repo,&original,id)?;
        let candidate=&build.result;
        candidate_is_authorizable(candidate,digest)?;
        let (branch_ref,old_head)=exact_branch(&repo)?;
        if branch_ref!=format!("refs/heads/{}",candidate.current_branch)||
            old_head.to_string()!=candidate.current_head||
            local_commit(&repo,branch)?.to_string()!=candidate.selected_tip{
            return Err("G06C04_SOURCE_REF_STALE");
        }
        let root=self.registered_git_root(id).map_err(|_|"G06C04_SOURCE_PROJECT_UNAVAILABLE")?;
        if fs::symlink_metadata(repo.path().join("index.lock")).is_ok(){
            return Err("G06C04_FOREIGN_INDEX_LOCK");
        }
        if index_sha(repo.path())?!=candidate.original_index_sha256||
            !clean_tree(&repo,&candidate.current_tree)?{
            return Err("G06C04_DIRTY_OR_STALE_INDEX_WORKTREE");
        }
        let journal_file=journal_path(self,id)?;
        if let Some(last)=read_last(&journal_file)?{
            if !matches!(last.phase.as_str(),"verified"|"cancelled") {
                return Err("G06C04_INTERRUPTED_OP_REQUIRES_MANUAL_REVIEW");
            }
        }
        let operation_id=format!("{:032x}",rand::random::<u128>());
        let original_ref=if kind==ReplayReviewKind::RebaseCurrentOntoLocal {
            Some(format!("refs/lu-ide/g06c04/original/{operation_id}"))
        }else{None};
        let next_head=candidate.prospective_head.as_deref().ok_or("G06C04_NO_HEAD")?.to_owned();
        let next_tree=candidate.prospective_tree.as_deref().ok_or("G06C04_NO_TREE")?.to_owned();
        let mut j=ReplayJournal{
            schema_version:1,operation_id:operation_id.clone(),project_id:id.to_owned(),
            operation:kind,phase:"prepared".into(),
            branch_ref:branch_ref.clone(),original_head:old_head.to_string(),
            original_tree:candidate.current_tree.clone(),
            selected_local_branch:branch.to_owned(),selected_tip:candidate.selected_tip.clone(),
            candidate_head:next_head.clone(),candidate_tree:next_tree.clone(),
            original_index_sha256:candidate.original_index_sha256.clone(),
            candidate_index_sha256:String::new(),result_sha256:digest.to_owned(),
            preserved_original_ref:original_ref.clone(),
        };
        if cancel.load(Ordering::SeqCst)!=0 {
            return Err("G06C04_CANCELLED_BEFORE_JOURNAL");
        }
        append(&journal_file,&j)?;
        if cancel.compare_exchange(0,1,Ordering::SeqCst,Ordering::SeqCst).is_err(){
            j.phase="cancelled".into();append(&journal_file,&j)?;
            return Err("G06C04_CANCELLED_BEFORE_OBJECT_PUBLICATION");
        }
        j.phase="objects_intent".into();append(&journal_file,&j)?;
        if ref_oid(&repo,&branch_ref)?!=j.original_head ||
            local_commit(&repo,branch)?.to_string()!=j.selected_tip ||
            index_sha(repo.path())?!=j.original_index_sha256 ||
            !clean_tree(&repo,&j.original_tree)? ||
            fs::symlink_metadata(repo.path().join("index.lock")).is_ok(){
            return Err("G06C04_SOURCE_CHANGED_BEFORE_OBJECT_WRITE");
        }
        let quarantine=build.sandbox.repository()?;
        copy_quarantine_objects(&repo,quarantine,candidate)?;
        if repo.find_commit(Oid::from_str(&next_head).map_err(|_|"G06C04_HEAD_OID_INVALID")?)
            .map_err(|_|"G06C04_COMMIT_WRITE_READBACK_FAILED")?
            .tree_id().to_string()!=next_tree{
            return Err("G06C04_COMMIT_TREE_READBACK_MISMATCH");
        }
        j.phase="objects_ready".into();append(&journal_file,&j)?;
        if let Some(ref name)=original_ref{
            if repo.find_reference(name).is_ok() {
                return Err("G06C04_BACKUP_REF_ALREADY_EXISTS");
            }
            repo.reference(name,old_head,false,"Lu IDE original head before one-use rebase")
                .map_err(|_|"G06C04_ORIGINAL_HISTORY_BACKUP_FAILED")?;
            if ref_oid(&repo,name)?!=j.original_head {
                return Err("G06C04_ORIGINAL_HISTORY_BACKUP_UNVERIFIED");
            }
        }
        if ref_oid(&repo,&branch_ref)?!=j.original_head ||
            index_sha(repo.path())?!=j.original_index_sha256 ||
            !clean_tree(&repo,&j.original_tree)?{
            return Err("G06C04_SOURCE_CHANGED_BEFORE_CHECKOUT");
        }
        ensure_changed_paths_safe(&root,&repo,&j.original_tree,&j.candidate_tree)?;
        j.phase="checkout_intent".into();append(&journal_file,&j)?;
        let new_object=repo.find_object(
            Oid::from_str(&next_head).map_err(|_|"G06C04_NEW_HEAD_INVALID")?,
            Some(ObjectType::Commit)
        ).map_err(|_|"G06C04_OUTPUT_OBJECT_NOT_AVAILABLE")?;
        let mut checkout=CheckoutBuilder::new();
        checkout.safe().recreate_missing(true).remove_untracked(false)
            .remove_ignored(false).overwrite_ignored(false).update_index(true);
        repo.checkout_tree(&new_object,Some(&mut checkout))
            .map_err(|_|"G06C04_SAFE_CHECKOUT_FAILED_MANUAL_RECOVERY")?;
        j.candidate_index_sha256=index_sha(repo.path())?;
        let mut idx=repo.index().map_err(|_|"G06C04_POST_INDEX_UNAVAILABLE")?;
        idx.read(true).map_err(|_|"G06C04_POST_INDEX_READ_FAILED")?;
        if idx.has_conflicts()||
            idx.write_tree_to(&repo).map_err(|_|"G06C04_POST_TREE_INVALID")?.to_string()!=j.candidate_tree||
            !worktree_matches_index(&repo)?{
            return Err("G06C04_POST_CHECKOUT_TREE_UNVERIFIED");
        }
        j.phase="checkout_verified".into();append(&journal_file,&j)?;
        #[cfg(test)]
        test_abort_disposable(&root,repo.path(),"IDE_G06C04_ABORT_AFTER_CHECKOUT");
        if ref_oid(&repo,&branch_ref)?!=j.original_head||
            local_commit(&repo,branch)?.to_string()!=j.selected_tip||
            index_sha(repo.path())?!=j.candidate_index_sha256{
            return Err("G06C04_PRE_CAS_SOURCE_CHANGED");
        }
        j.phase="publishing".into();append(&journal_file,&j)?;
        repo.reference_matching(&branch_ref,
            Oid::from_str(&next_head).map_err(|_|"G06C04_NEW_REF_INVALID")?,
            true,old_head,"Lu IDE: OS-approved original replay exact ref CAS"
        ).map_err(|_|"G06C04_EXACT_REF_CAS_FAILED_MANUAL_REPAIR")?;
        #[cfg(test)]
        test_abort_disposable(&root,repo.path(),"IDE_G06C04_ABORT_AFTER_CAS");
        j.phase="published".into();append(&journal_file,&j)?;
        if ref_oid(&repo,&branch_ref)?!=next_head||
            index_sha(repo.path())?!=j.candidate_index_sha256||
            !clean_tree(&repo,&j.candidate_tree)? ||
            original_ref.as_deref().map(|name|ref_oid(&repo,name))
                .transpose()?.is_some_and(|oid|oid!=j.original_head){
            return Err("G06C04_RESULT_READBACK_FAILED");
        }
        j.phase="verified".into();append(&journal_file,&j)?;
        Ok(ApprovedReplayReceipt{
            schema_version:1,project_id:id.to_owned(),operation:kind,
            operation_id,original_head:j.original_head,
            published_head:j.candidate_head,resulting_tree:j.candidate_tree,
            prior_branch_ref:branch_ref,original_history_ref:original_ref,
            final_index_sha256:j.candidate_index_sha256,result_sha256:j.result_sha256,
            original_objects_preserved:true,independently_verified:true,
            exact_ref_cas:true,one_use_native_os_consent:true,
            journal_synced:true,git_hooks_executed:false,remote_updated:false,
        })
    }
    pub fn inspect_protected_replay_recovery(
        &self,id:&str
    )->Result<ReplayRecovery,&'static str>{
        let repo=exact_repo(self,id)?;
        let journal=read_last(&journal_path(self,id)?)?;
        let index_lock=fs::symlink_metadata(repo.path().join("index.lock")).is_ok();
        let current=repo.head().ok();
        let ref_name=current.as_ref().and_then(|r|r.name()).map(str::to_owned);
        let ref_target=current.as_ref().and_then(|r|r.target()).map(|oid|oid.to_string());
        let index=index_sha(repo.path()).ok();
        let wt=worktree_matches_index(&repo).unwrap_or(false);
        let (state,phase,kind,original,candidate,old_match,new_match,backup,oldidx,newidx)=
            if let Some(ref j)=journal{
                if j.project_id!=id{return Err("G06C04_JOURNAL_PROJECT_MISMATCH");}
                let same_branch=ref_name.as_deref()==Some(j.branch_ref.as_str());
                let same_selected=local_commit(&repo,&j.selected_local_branch).ok()
                    .is_some_and(|oid|oid.to_string()==j.selected_tip);
                let old=same_branch&&same_selected&&
                    ref_target.as_deref()==Some(j.original_head.as_str());
                let new=same_branch&&same_selected&&
                    ref_target.as_deref()==Some(j.candidate_head.as_str());
                let backed=j.preserved_original_ref.as_deref()
                    .map(|n|ref_oid(&repo,n).ok().as_deref()==Some(j.original_head.as_str()))
                    .unwrap_or(true);
                let old_index=index.as_deref()==Some(j.original_index_sha256.as_str());
                let new_index=!j.candidate_index_sha256.is_empty()&&
                    index.as_deref()==Some(j.candidate_index_sha256.as_str());
                let status=if j.phase=="verified"&&new&&new_index&&backed&&wt{
                    "verified_original_replay"
                }else if j.phase=="cancelled"&&old&&old_index&&backed&&wt{
                    "cancelled_before_object_write"
                }else if new&&new_index&&backed&&wt{
                    "published_ref_requires_separate_receipt"
                }else if old&&new_index&&wt{
                    "checkout_complete_ref_not_published"
                }else if old&&old_index&&wt&&matches!(j.phase.as_str(),
                    "prepared"|"objects_intent"|"objects_ready"|"checkout_intent"){
                    "prepublication_intent_requires_review"
                }else{
                    "ambiguous_replay_or_branch_drift"
                };
                (status,j.phase.clone(),Some(j.operation),
                    j.original_head.clone(),j.candidate_head.clone(),
                    old,new,backed,old_index,new_index)
            }else{
                ("no_journal",String::new(),None,String::new(),String::new(),
                    false,false,false,false,false)
            };
        let manual=index_lock||!wt||
            !matches!(state,"no_journal"|"verified_original_replay"|"cancelled_before_object_write");
        Ok(ReplayRecovery{
            schema_version:1,project_id:id.to_owned(),operation:kind,
            state,last_phase:phase,original_head:original,candidate_head:candidate,
            branch_matches_original:old_match,branch_matches_candidate:new_match,
            backup_ref_verified:backup,original_index_matches:oldidx,
            candidate_index_matches:newidx,worktree_matches_index:wt,
            index_lock_present:index_lock,operator_reconciliation_required:manual,
            automatic_ref_retry:false,automatic_history_rewrite:false,
            automatic_checkout:false,automatic_rollback:false,
            automatic_foreign_lock_deletion:false,
        })
    }

    /// Separately OS-approved receipt-only reconciliation: never perform a
    /// new replay/ref write/checkout/recovery of source files.
    pub fn reconcile_protected_replay_receipt(
        &self,id:&str
    )->Result<ReplayRecovery,&'static str>{
        let repo=exact_repo(self,id)?;
        if fs::symlink_metadata(repo.path().join("index.lock")).is_ok(){
            return Err("G06C04_RECOVERY_FOREIGN_LOCK");
        }
        let path=journal_path(self,id)?;
        let mut j=read_last(&path)?.ok_or("G06C04_NO_RECOVERY_RECORD")?;
        let r=self.inspect_protected_replay_recovery(id)?;
        if r.state!="published_ref_requires_separate_receipt" ||
            !matches!(j.phase.as_str(),"publishing"|"published") ||
            !r.worktree_matches_index||!r.backup_ref_verified{
            return Err("G06C04_RECOVERY_NOT_SAFELY_VERIFIABLE");
        }
        if ref_oid(&repo,&j.branch_ref)?!=j.candidate_head ||
            local_commit(&repo,&j.selected_local_branch)?.to_string()!=j.selected_tip||
            !clean_tree(&repo,&j.candidate_tree)? ||
            index_sha(repo.path())?!=j.candidate_index_sha256{
            return Err("G06C04_RECOVERY_INDEPENDENT_READBACK_FAILED");
        }
        j.phase="verified".into();append(&path,&j)?;
        self.inspect_protected_replay_recovery(id)
    }
}
/// Test-only synthetic crash is guarded by disposable test fixture under
/// system temporary directory and a marker inside its private Git metadata.
#[cfg(test)]
fn test_abort_disposable(root:&Path,gitdir:&Path,var:&str){
    let Some(needle)=std::env::var_os(var)else{return;};
    let canonical=fs::canonicalize(root).ok();
    let claimed=fs::canonicalize(PathBuf::from(needle)).ok();
    let temp=fs::canonicalize(std::env::temp_dir()).ok();
    if canonical.is_some()&&canonical==claimed&&
        canonical.as_ref().zip(temp.as_ref()).is_some_and(|(a,b)|a.starts_with(b))&&
        root.file_name().is_some_and(|n|n=="repo")&&
        root.parent().and_then(|p|p.file_name())
            .is_some_and(|n|n.to_string_lossy().starts_with("ide-g06c04-protected-"))&&
        fs::read(gitdir.join("g06c04-abort-fixture")).ok().as_deref()==Some(b"DISPOSABLE\n"){
        std::process::abort();
    }
}
