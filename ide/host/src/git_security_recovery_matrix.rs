//! IDE Group 6F-01: consolidated, **read-only** Git security/recovery veto.
//!
//! Existing 6B/6C operation journals keep their original independent
//! validity and native OS approval semantics. This is an additional
//! fail-closed CROSS-DOMAIN gate: a pending commit, merge, replay, foreign
//! lock, dirty worktree or drifting source blocks admission of any new
//! protected Git operation. A "quiet" report never authorizes execution.
//!
//! There is no token, Git CLI, network request, ref/index/file mutation,
//! credential, automatic repair, lock removal or WebView execution route.
use crate::{git_branch_operations::exact_repo,IdeWorkspaceRegistry};
use git2::{Repository,RepositoryState,StatusOptions};
use serde::Serialize;
use sha2::{Digest,Sha256};
use std::{fs,io::Read};

const MAX_INDEX_BYTES:u64=8*1024*1024;
const MAX_STATUS:usize=2048;

#[derive(Clone,Debug,PartialEq,Eq)]
struct SourceSnapshot {
    branch:String,
    head:String,
    index_sha256:String,
    locked:bool,
    state:RepositoryState,
    dirty:usize,
}
fn snapshot(repo:&Repository)->Result<SourceSnapshot,&'static str>{
    let reference=repo.head().map_err(|_|"G06F_HEAD_UNAVAILABLE")?;
    if !reference.is_branch(){return Err("G06F_NON_BRANCH_HEAD");}
    let branch=reference.name()
        .ok_or("G06F_BRANCH_INVALID")?.to_owned();
    let head=reference.target()
        .ok_or("G06F_HEAD_NOT_DIRECT")?.to_string();
    let path=repo.path().join("index");
    let metadata=fs::symlink_metadata(&path)
        .map_err(|_|"G06F_INDEX_UNREADABLE")?;
    if !metadata.is_file()||metadata.file_type().is_symlink()
        ||metadata.len()>MAX_INDEX_BYTES {
        return Err("G06F_INDEX_UNTRUSTED");
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes()&0x400!=0{
            return Err("G06F_INDEX_REPARSE_DENIED");
        }
    }
    let mut bytes=Vec::new();
    fs::File::open(&path).map_err(|_|"G06F_INDEX_UNREADABLE")?
        .take(MAX_INDEX_BYTES+1).read_to_end(&mut bytes)
        .map_err(|_|"G06F_INDEX_UNREADABLE")?;
    if bytes.len() as u64!=metadata.len(){
        return Err("G06F_INDEX_CHANGED_DURING_READ");
    }
    let mut opts=StatusOptions::new();
    opts.include_untracked(true).recurse_untracked_dirs(false)
        .include_ignored(false);
    let statuses=repo.statuses(Some(&mut opts))
        .map_err(|_|"G06F_STATUS_UNAVAILABLE")?;
    if statuses.len()>MAX_STATUS{return Err("G06F_STATUS_OVER_LIMIT");}
    let locked=repo.path().join("index.lock").exists()
        ||repo.path().join("HEAD.lock").exists()
        ||repo.path().join("packed-refs.lock").exists();
    Ok(SourceSnapshot{
        branch,head,index_sha256:format!("{:x}",Sha256::digest(&bytes)),
        locked,state:repo.state(),dirty:statuses.len(),
    })
}
#[derive(Clone,Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub struct RecoveryDomainEvidence {
    pub domain:&'static str,
    pub observed_state:&'static str,
    pub manual_reconciliation_required:bool,
    pub authoritative_original_journal_inspected:bool,
    pub automatic_retry_authorized:bool,
    pub automatic_git_mutation_authorized:bool,
}
fn unresolved_original_journal(state:&str,original_manual:bool)->bool{
    // The old 6C inspectors may mark an otherwise empty or already
    // terminal journal as "manual" solely because untracked or modified
    // files exist. That is a legitimate starting state for stage/stash
    // and must not create a GLOBAL recovery veto. All genuine
    // interrupted/unknown phases retain their original manual bit.
    if matches!(state,
        "no_journal"|"verified_historical_fast_forward"
        |"cancelled_before_checkout"
        |"historical_verified_divergent_merge"
        |"verified_original_replay"
        |"cancelled_before_object_write"
        |"verified_historical_commit"
        |"cancelled_before_ref_update"
    ){
        return false;
    }
    original_manual
}
fn domain(name:&'static str,state:&'static str,manual:bool)
    ->RecoveryDomainEvidence{
    RecoveryDomainEvidence{
        domain:name,observed_state:state,manual_reconciliation_required:manual,
        authoritative_original_journal_inspected:true,
        automatic_retry_authorized:false,
        automatic_git_mutation_authorized:false,
    }
}
#[derive(Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub struct ConsolidatedGitRecoveryEvidence {
    pub schema_version:u32,
    pub project_id:String,
    pub exact_branch_ref:String,
    pub exact_source_head:String,
    pub exact_index_sha256:String,
    pub source_reobserved:bool,
    pub foreign_git_lock_present:bool,
    pub worktree_dirty:bool,
    pub git_operation_in_progress:bool,
    pub domains:Vec<RecoveryDomainEvidence>,
    pub cross_domain_manual_reconciliation_required:bool,
    pub local_new_operation_preflight_clear:bool,
    pub original_operation_approval_preserved:bool,
    pub remote_publication_authorized:bool,
    pub git_mutation_performed:bool,
    pub os_human_approval_granted:bool,
    pub automatic_retry_authorized:bool,
    pub automatic_lock_removal_authorized:bool,
    pub evidence_sha256:String,
}
fn consolidate(
    id:&str,source:&SourceSnapshot,domains:Vec<RecoveryDomainEvidence>,
)->Result<ConsolidatedGitRecoveryEvidence,&'static str>{
    if domains.len()!=9
        ||domains.iter().any(|x|x.automatic_git_mutation_authorized
            ||x.automatic_retry_authorized
            ||!x.authoritative_original_journal_inspected)
    {return Err("G06F_ORIGINAL_RECOVERY_PROOF_INVALID");}
    let mut found=std::collections::BTreeSet::new();
    for d in &domains {
        if !found.insert(d.domain){return Err("G06F_DUPLICATE_DOMAIN");}
    }
    if found!=std::collections::BTreeSet::from([
        "reviewed_commit","protected_fast_forward",
        "protected_divergent_merge","protected_replay",
        "partial_stage","conflict_resolution","reviewed_unstage",
        "single_file_restore","single_file_stash",
    ]){return Err("G06F_MISSING_ORIGINAL_DOMAIN");}
    let troubled=domains.iter().any(|d|d.manual_reconciliation_required);
    let git_busy=source.state!=RepositoryState::Clean;
    // Local staging/commit/stash can legitimately begin from dirty files.
    // Their ORIGINAL per-operation source policy remains authoritative.
    // 6F's global veto covers interrupted journal/recovery, foreign
    // locks or an active Git operation, not generic modified files.
    let blocked=troubled||source.locked||git_busy;
    let mut h=Sha256::new();
    for s in [id,&source.branch,&source.head,&source.index_sha256]{
        h.update((s.len() as u64).to_le_bytes());h.update(s.as_bytes());
    }
    for d in &domains {
        h.update(d.domain.as_bytes());
        h.update(d.observed_state.as_bytes());
        h.update([u8::from(d.manual_reconciliation_required)]);
    }
    h.update([u8::from(blocked)]);
    Ok(ConsolidatedGitRecoveryEvidence{
        schema_version:1,project_id:id.to_owned(),
        exact_branch_ref:source.branch.clone(),exact_source_head:source.head.clone(),
        exact_index_sha256:source.index_sha256.clone(),source_reobserved:true,
        foreign_git_lock_present:source.locked,
        worktree_dirty:source.dirty!=0,
        git_operation_in_progress:git_busy,
        domains,cross_domain_manual_reconciliation_required:blocked,
        // "clear" means there is no observed LOCAL unresolved operation;
        // existing native one-use OS approval + project capability STILL
        // required for every actual Git mutation or remote publication.
        local_new_operation_preflight_clear:!blocked,
        original_operation_approval_preserved:true,
        remote_publication_authorized:false,git_mutation_performed:false,
        os_human_approval_granted:false,
        automatic_retry_authorized:false,automatic_lock_removal_authorized:false,
        evidence_sha256:format!("{:x}",h.finalize()),
    })
}
impl IdeWorkspaceRegistry{
    /// Re-read each ORIGINAL journal inspector; unlike a synthetic CI
    /// receipt, a single journal error denies the entire group. Reobserve
    /// registered repo identity/HEAD/index/status after all four inspections.
    pub fn inspect_group6f_git_recovery(
        &self,project_id:&str,
    )->Result<ConsolidatedGitRecoveryEvidence,&'static str>{
        let repo=exact_repo(self,project_id)?;
        let before=snapshot(&repo)?;
        let commit=self.inspect_reviewed_commit_recovery(project_id)?;
        let fast=self.inspect_protected_merge_recovery(project_id)?;
        let divergent=self.inspect_protected_divergent_recovery(project_id)?;
        let replay=self.inspect_protected_replay_recovery(project_id)?;
        let stage=self.inspect_partial_stage_recovery(project_id)?;
        let conflict=self.inspect_protected_conflict_recovery(project_id)?;
        let unstage=self.inspect_unstage_interrupted(project_id)?;
        let restore=self.single_file_restore_recovery_required(project_id)?;
        let stash=self.one_file_stash_recovery_required(project_id)?;
        if commit.project_id!=project_id||fast.project_id!=project_id
            ||divergent.project_id!=project_id||replay.project_id!=project_id
            ||commit.schema_version!=1||fast.schema_version!=1
            ||divergent.schema_version!=1||replay.schema_version!=1
            ||stage.project_id!=project_id||conflict.project_id!=project_id
            ||stage.schema_version!=1||conflict.schema_version!=1
            ||stage.auto_delete_git_lock||stage.auto_rollback_index
            ||stage.write_authority_granted
            ||conflict.automatic_retry||conflict.automatic_restore
            ||conflict.automatic_index_write||conflict.automatic_ref_update
            ||commit.automatic_retry||commit.automatic_ref_rewrite
            ||fast.automatic_retry||fast.automatic_ref_rewrite
            ||fast.automatic_checkout||fast.automatic_lock_deletion
            ||divergent.automatic_retry||divergent.automatic_ref_rewrite
            ||divergent.automatic_checkout||divergent.automatic_rollback
            ||divergent.automatic_lock_deletion
            ||replay.automatic_ref_retry||replay.automatic_history_rewrite
            ||replay.automatic_checkout||replay.automatic_rollback
            ||replay.automatic_foreign_lock_deletion
        {return Err("G06F_ORIGINAL_RECOVERY_NOT_AUTHORITATIVE");}
        let domains=vec![
            domain("reviewed_commit",commit.state,
                unresolved_original_journal(commit.state,
                    commit.operator_reconciliation_required)),
            domain("protected_fast_forward",fast.state,
                unresolved_original_journal(fast.state,
                    fast.operator_reconciliation_required)),
            domain("protected_divergent_merge",divergent.state,
                unresolved_original_journal(divergent.state,
                    divergent.operator_reconciliation_required)),
            domain("protected_replay",replay.state,
                unresolved_original_journal(replay.state,
                    replay.operator_reconciliation_required)),
            domain("partial_stage",stage.state,
                stage.operator_reconciliation_required),
            domain("conflict_resolution",conflict.state,
                conflict.operator_review_required),
            domain("reviewed_unstage",unstage.unwrap_or("no_journal"),
                unstage.is_some_and(|s| !matches!(s,
                    "verified"|"cancelled_before_publication"))),
            domain("single_file_restore",
                if restore{"incomplete_restore_requires_review"}else{"no_pending_restore"},
                restore),
            domain("single_file_stash",
                if stash{"incomplete_stash_requires_review"}else{"no_pending_stash"},
                stash),
        ];
        let after_repo=exact_repo(self,project_id)?;
        if before!=snapshot(&after_repo)?{
            return Err("G06F_SOURCE_DRIFT_DURING_RECONCILIATION");
        }
        consolidate(project_id,&before,domains)
    }
}

/// Group 6F-02 — one cross-domain restart verdict for local Git +
/// original remote 6E-05C prepared journal. All outcomes prohibit a push.
/// This method cannot call credentials, GitHub, Tauri or OS approval code;
/// a caller must bring a NEW, genuinely host-authenticated 6E-05B read.
#[derive(Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub struct CrossDomainRemoteRecovery {
    pub schema_version:u32,
    pub project_id:String,
    pub exact_local_head:String,
    pub original_intent_sha256:String,
    pub remote_classification:&'static str,
    pub local_git_reconciliation_required:bool,
    pub remote_operator_reconciliation_required:bool,
    pub source_rechecked_after_remote_classification:bool,
    pub remote_publication_authorized:bool,
    pub remote_publication_claimed_completed:bool,
    pub native_approval_carried_over:bool,
    pub automatic_retry_authorized:bool,
    pub remote_mutation_performed:bool,
}
impl IdeWorkspaceRegistry{
    pub(crate) fn inspect_group6f_remote_restart(
        &self,project_id:&str,
        host_private_journal:&std::path::Path,
        original:&crate::github_remote_publication_intent::RemotePublicationIntent,
        fresh:&crate::github_remote_live_policy::LiveBranchPolicyEvidence,
    )->Result<CrossDomainRemoteRecovery,&'static str>{
        let before=self.inspect_group6f_git_recovery(project_id)?;
        if original.project_id!=project_id
            ||original.proposed_local_commit!=before.exact_source_head
            ||original.schema_version!=1
            ||original.intent_sha256.len()!=64
        {return Err("G06F_REMOTE_SOURCE_OR_INTENT_DRIFT");}
        let classified=
            crate::github_remote_publication_intent::
                classify_uncertain_publication_on_restart(
                    host_private_journal,original,fresh,
                )?;
        let after=self.inspect_group6f_git_recovery(project_id)?;
        if before.evidence_sha256!=after.evidence_sha256
            ||before.exact_branch_ref!=after.exact_branch_ref
            ||before.exact_index_sha256!=after.exact_index_sha256
        {return Err("G06F_REMOTE_LOCAL_STATE_CHANGED");}
        // All three classes remain UNCERTAIN, not "published". Even a
        // matching proposed tip could have been written independently.
        let state=match classified{
            crate::github_remote_publication_intent::
                UncertainRemoteOutcome::RefUnchangedNoRetry=>
                    "old_tip_still_visible_no_retry",
            crate::github_remote_publication_intent::
                UncertainRemoteOutcome::RefMatchesProposedButUnproven=>
                    "proposed_tip_visible_not_our_publication",
            crate::github_remote_publication_intent::
                UncertainRemoteOutcome::CompetingRemoteWriterQuarantine=>
                    "competing_writer_quarantine",
        };
        Ok(CrossDomainRemoteRecovery{
            schema_version:1,project_id:project_id.to_owned(),
            exact_local_head:after.exact_source_head,
            original_intent_sha256:original.intent_sha256.clone(),
            remote_classification:state,
            local_git_reconciliation_required:
                before.cross_domain_manual_reconciliation_required,
            remote_operator_reconciliation_required:true,
            source_rechecked_after_remote_classification:true,
            remote_publication_authorized:false,
            remote_publication_claimed_completed:false,
            native_approval_carried_over:false,
            automatic_retry_authorized:false,
            remote_mutation_performed:false,
        })
    }
}
impl IdeWorkspaceRegistry{
    /// Source-only, non-authorizing 6F check to run immediately BEFORE
    /// every original one-use mutation. The operation's own original
    /// native consent, revocation, source and journal checks still decide
    /// execution. The sole exception allows the conflict-resolution
    /// workflow to inspect a legitimate Git Merge state; it never exempts
    /// a pending journal or foreign lock.
    pub(crate) fn require_group6f_no_pending_recovery(
        &self,project_id:&str,allow_git_conflict_state:bool,
    )->Result<(),&'static str>{
        let e=self.inspect_group6f_git_recovery(project_id)?;
        if e.foreign_git_lock_present
            ||e.domains.iter().any(|d|d.manual_reconciliation_required)
            ||(e.git_operation_in_progress&&!allow_git_conflict_state){
            return Err("G06F_CROSS_DOMAIN_RECOVERY_VETO");
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use crate::{IdeError,NativeDirectoryPicker};
    use git2::{Repository,Signature};
    use std::{fs,path::PathBuf};
    struct Picker(PathBuf);
    impl NativeDirectoryPicker for Picker{
        fn choose_project_directory(&mut self)->Result<Option<PathBuf>,IdeError>{
            Ok(Some(self.0.clone()))
        }
    }
    struct Fixture{
        dir:PathBuf,root:PathBuf,registry:IdeWorkspaceRegistry,id:String,
    }
    impl Drop for Fixture{
        fn drop(&mut self){let _=fs::remove_dir_all(&self.dir);}
    }
    fn fixture()->Fixture{
        let dir=std::env::temp_dir().join(format!("g06f-{:032x}",rand::random::<u128>()));
        let root=dir.join("repo");
        fs::create_dir_all(&root).unwrap();
        let repo=Repository::init(&root).unwrap();
        fs::write(root.join("file.txt"),b"test\n").unwrap();
        let mut index=repo.index().unwrap();
        index.add_path(std::path::Path::new("file.txt")).unwrap();
        index.write().unwrap();
        let tree=repo.find_tree(index.write_tree().unwrap()).unwrap();
        let sig=Signature::now("CI","ci@example.invalid").unwrap();
        repo.commit(Some("HEAD"),&sig,&sig,"init",&tree,&[]).unwrap();
        drop(tree);drop(index);drop(repo);
        let registry=IdeWorkspaceRegistry::open(dir.join("catalog.json")).unwrap();
        let id=registry.register_from_native_picker(&mut Picker(root.clone()))
            .unwrap().unwrap().project_id;
        Fixture{dir,root,registry,id}
    }
    #[test]
    fn actual_registered_clean_repo_consults_four_original_recovery_domains(){
        let f=fixture();
        let result=f.registry.inspect_group6f_git_recovery(&f.id).unwrap();
        assert_eq!(result.domains.len(),9);
        assert!(result.source_reobserved);
        assert!(result.local_new_operation_preflight_clear);
        assert!(!result.remote_publication_authorized);
        assert!(!result.git_mutation_performed);
        assert!(!result.os_human_approval_granted);
        assert!(!result.automatic_retry_authorized);
    }
    #[test]
    fn real_foreign_git_lock_and_dirty_worktree_block_cross_domain_new_work(){
        let f=fixture();
        fs::write(f.root.join(".git/index.lock"),b"foreign").unwrap();
        fs::write(f.root.join("file.txt"),b"dirty\n").unwrap();
        let result=f.registry.inspect_group6f_git_recovery(&f.id).unwrap();
        assert!(result.foreign_git_lock_present);
        assert!(result.worktree_dirty);
        assert!(result.cross_domain_manual_reconciliation_required);
        assert!(!result.local_new_operation_preflight_clear);
        assert!(f.root.join(".git/index.lock").exists()); // never delete it
    }
    #[test]
    fn terminal_historical_journal_with_dirty_files_is_not_a_new_approval(){
        assert!(!unresolved_original_journal("verified_historical_fast_forward",true));
        assert!(!unresolved_original_journal("no_journal",true));
        assert!(unresolved_original_journal("published_ref_requires_operator_receipt",true));
        assert!(unresolved_original_journal("ambiguous_replay_or_branch_drift",true));
    }
    #[test]
    fn ordinary_dirty_worktree_does_not_invent_a_cross_domain_recovery_block(){
        let f=fixture();
        fs::write(f.root.join("file.txt"),b"needs staging\n").unwrap();
        let result=f.registry.inspect_group6f_git_recovery(&f.id).unwrap();
        assert!(result.worktree_dirty);
        assert!(!result.cross_domain_manual_reconciliation_required);
        assert!(result.local_new_operation_preflight_clear);
        assert!(!result.git_mutation_performed);
        assert!(!result.os_human_approval_granted);
    }
    #[test]
    fn other_domains_cannot_hide_one_uncertain_transaction(){
        let f=fixture();
        let repo=exact_repo(&f.registry,&f.id).unwrap();
        let src=snapshot(&repo).unwrap();
        let domains=vec![
            domain("reviewed_commit","no_journal",false),
            domain("protected_fast_forward","published_requires_receipt",true),
            domain("protected_divergent_merge","no_journal",false),
            domain("protected_replay","no_journal",false),
            domain("partial_stage","no_journal",false),
            domain("conflict_resolution","no_journal",false),
            domain("reviewed_unstage","no_journal",false),
            domain("single_file_restore","no_pending_restore",false),
            domain("single_file_stash","no_pending_stash",false),
        ];
        let result=consolidate(&f.id,&src,domains).unwrap();
        assert!(result.cross_domain_manual_reconciliation_required);
        assert!(!result.local_new_operation_preflight_clear);
        assert!(!result.automatic_retry_authorized);
    }
    #[test]
    fn truncated_original_fast_forward_journal_blocks_all_other_git_domains(){
        let f=fixture();
        let original=f.dir.join("ide-g06c03-protected-merges");
        fs::create_dir_all(&original).unwrap();
        fs::write(original.join(format!("{}.jsonl",f.id)),b"{invalid_no_fsync_newline").unwrap();
        assert!(f.registry.inspect_group6f_git_recovery(&f.id).is_err());
        assert_eq!(fs::read_dir(&original).unwrap().count(),1);
    }
    #[test]
    fn corrupt_existing_single_file_stash_marker_vetoes_every_git_operation(){
        let f=fixture();
        fs::write(f.root.join(".git/lu-ide-fin03-stash-active"),b"not-a-valid-64-hex-marker").unwrap();
        assert!(f.registry.inspect_group6f_git_recovery(&f.id).is_err());
    }
    #[test]
    fn missing_or_forged_original_domain_is_not_a_valid_consolidation(){
        let f=fixture();
        let repo=exact_repo(&f.registry,&f.id).unwrap();
        let src=snapshot(&repo).unwrap();
        let mut domains=vec![
            domain("reviewed_commit","no_journal",false),
            domain("protected_fast_forward","no_journal",false),
            domain("protected_divergent_merge","no_journal",false),
            domain("protected_replay","no_journal",false),
            domain("partial_stage","no_journal",false),
            domain("conflict_resolution","no_journal",false),
            domain("reviewed_unstage","no_journal",false),
            domain("single_file_restore","no_pending_restore",false),
            domain("single_file_stash","no_pending_stash",false),
        ];
        domains[2].automatic_retry_authorized=true;
        assert!(consolidate(&f.id,&src,domains).is_err());
    }
}
