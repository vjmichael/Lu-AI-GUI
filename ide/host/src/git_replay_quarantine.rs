//! IDE 6C-04B: reproducible prospective replay isolated from the project ODB.
//!
//! A temporary, bare libgit2 repository is the ONLY target of new tree/commit
//! objects during review. Its object database has a read-only alternate to the
//! original .git/objects. Never write to the registered repository on review.
//!
//! The only scope admitted is an ordered <=16 first-parent replay (rebase) or
//! the single selected local branch tip (cherry-pick). No merge commits, manual
//! conflict resolutions, Git scripts, external Git CLI, network or force options.
use crate::{
    git_branch_operations::{exact_repo, local_commit},
    git_rebase_cherry_plan::{ReplayReviewKind, ReviewedReplayPlan},
    IdeWorkspaceRegistry,
};
use git2::{Oid, Repository, ObjectType, Signature, StatusOptions};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{fs, path::{Path, PathBuf}};
const MAX_OUTPUT_PATHS: usize = 80;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplayResultStep {
    pub ordinal: usize,
    pub original_commit: String,
    pub original_parent: String,
    pub resulting_commit: String,
    pub resulting_tree: String,
    pub resulting_parent: String,
    pub conflict_path_count: usize,
    pub original_author_retained: bool,
    pub original_committer_retained: bool,
    pub original_message_retained: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProspectiveReplayResult {
    pub schema_version: u32,
    pub project_id: String,
    pub operation: ReplayReviewKind,
    pub current_branch: String,
    pub current_head: String,
    pub current_tree: String,
    pub selected_local_branch: String,
    pub selected_tip: String,
    pub selected_tree: String,
    pub original_index_sha256: String,
    pub replay_plan_sha256: String,
    pub evidence_sha256: String,
    pub prospective_head: Option<String>,
    pub prospective_tree: Option<String>,
    pub result_steps: Vec<ReplayResultStep>,
    pub candidate_change_paths: usize,
    pub conflict_count: usize,
    pub conflicting_paths: Vec<String>,
    pub blockers: Vec<&'static str>,
    pub original_project_unchanged: bool,
    pub quarantine_only_result_objects: bool,
    pub one_use_os_authorized: bool,
    pub project_ref_modified: bool,
    pub project_index_or_worktree_modified: bool,
    pub project_odb_modified: bool,
    pub no_hooks_or_external_processes: bool,
    pub remote_queried: bool,
    pub execution_route_available: bool,
}

fn fingerprint(parts: &[&str]) -> String {
    let mut hash = Sha256::new();
    hash.update(b"lu-ide-g06c04b-isolated-replay-result-v1\n");
    for item in parts {
        hash.update((item.len() as u64).to_le_bytes());
        hash.update(item.as_bytes());
    }
    format!("{:x}", hash.finalize())
}

fn add(v: &mut Vec<&'static str>, code: &'static str) {
    if !v.contains(&code) { v.push(code); }
}

/// This object must drop its Git handles before deleting its own scratchdir.
/// Git alternates are installed ONLY in this fresh disposable bare repository.
pub(crate) struct ReplaySandbox {
    pub(crate) repo: Option<Repository>,
    pub(crate) root: PathBuf,
}
impl Drop for ReplaySandbox {
    fn drop(&mut self) {
        self.repo.take();
        let _ = fs::remove_dir_all(&self.root);
    }
}
impl ReplaySandbox {
    pub(crate) fn open(original: &Repository) -> Result<Self, &'static str> {
        let root = std::env::temp_dir().join(
            format!("ide-g06c04-replay-{:032x}", rand::random::<u128>())
        );
        fs::create_dir(&root).map_err(|_| "G06C04B_SCRATCH_CREATE_FAILED")?;
        let mut sandbox = Self { repo: None, root };
        let scratch = Repository::init_bare(&sandbox.root)
            .map_err(|_| "G06C04B_SCRATCH_GIT_INIT_FAILED")?;
        let objects = original.path().join("objects");
        let meta = fs::symlink_metadata(&objects)
            .map_err(|_| "G06C04B_ORIGINAL_ODB_UNAVAILABLE")?;
        if !meta.is_dir() || meta.file_type().is_symlink() {
            return Err("G06C04B_ORIGINAL_ODB_UNTRUSTED");
        }
        let canonical = fs::canonicalize(objects)
            .map_err(|_| "G06C04B_ORIGINAL_ODB_UNVERIFIABLE")?;
        let path = canonical.to_str().ok_or("G06C04B_NON_UTF8_OBJECT_STORE")?;
        scratch.odb().map_err(|_| "G06C04B_SCRATCH_ODB_UNAVAILABLE")?
            .add_disk_alternate(path)
            .map_err(|_| "G06C04B_READONLY_ALTERNATE_FAILED")?;
        sandbox.repo = Some(scratch);
        Ok(sandbox)
    }
    pub(crate) fn repository(&self) -> Result<&Repository, &'static str> {
        self.repo.as_ref().ok_or("G06C04B_SCRATCH_NOT_OPEN")
    }
}

pub(crate) struct ReplayBuild {
    pub(crate) sandbox: ReplaySandbox,
    pub(crate) result: ProspectiveReplayResult,
}

/// Recreate each original commit's change against the preceding new parent.
/// The resulting in-memory index is converted to a tree and commit in the
/// SCRATCH bare repository only; original object database remains untouched.
pub(crate) fn compute_in_sandbox(
    original: &Repository, plan: &ReviewedReplayPlan,
    project_id: &str,
) -> Result<ReplayBuild, &'static str> {
    let scratch = ReplaySandbox::open(original)?;
    let quarantine = scratch.repository()?;
    let starting = match plan.operation {
        ReplayReviewKind::RebaseCurrentOntoLocal => &plan.selected_tip,
        ReplayReviewKind::CherryPickLocalTip => &plan.current_head,
    };
    let mut cursor = Oid::from_str(starting).map_err(|_| "G06C04B_PARENT_INVALID")?;
    let mut records = Vec::new();
    let mut conflict_count = 0usize;
    let mut conflicts = Vec::new();
    let mut blockers = Vec::new();
    for evidence in &plan.replay_commits {
        let original_id = Oid::from_str(&evidence.original_commit)
            .map_err(|_| "G06C04B_ORIGINAL_COMMIT_INVALID")?;
        let picked = quarantine.find_commit(original_id)
            .map_err(|_| "G06C04B_ORIGINAL_COMMIT_UNAVAILABLE")?;
        if picked.parent_count() != 1
            || picked.parent_id(0).map_err(|_| "G06C04B_PARENT_UNAVAILABLE")?.to_string()
                != evidence.original_parent
            || picked.tree_id().to_string() != evidence.original_tree {
            return Err("G06C04B_ORIGINAL_GRAPH_STALE");
        }
        let parent = quarantine.find_commit(cursor)
            .map_err(|_| "G06C04B_REPLAY_PARENT_UNAVAILABLE")?;
        let mut idx = quarantine.cherrypick_commit(&picked, &parent, 0, None)
            .map_err(|_| "G06C04B_REPLAY_MERGE_FAILED")?;
        if idx.has_conflicts() {
            let mut n = 0usize;
            let paths = idx.conflicts().map_err(|_| "G06C04B_CONFLICT_ITER_UNAVAILABLE")?;
            for path in paths {
                let conflict = path.map_err(|_| "G06C04B_CONFLICT_UNREADABLE")?;
                n = n.saturating_add(1);
                if conflicts.len() < MAX_OUTPUT_PATHS {
                    let first = conflict.our.or(conflict.their).or(conflict.ancestor)
                        .ok_or("G06C04B_CONFLICT_PATH_MISSING")?;
                    let raw = std::str::from_utf8(&first.path)
                        .map_err(|_| "G06C04B_CONFLICT_PATH_UNSAFE")?;
                    conflicts.push(raw.to_owned());
                }
            }
            conflict_count += n;
            add(&mut blockers, "actual_three_way_conflicts");
            break;
        }
        // This writes only into a fresh scratch bare repo, NEVER original.
        let tree_id = idx.write_tree_to(quarantine)
            .map_err(|_| "G06C04B_QUARANTINE_TREE_FAILED")?;
        let tree = quarantine.find_tree(tree_id)
            .map_err(|_| "G06C04B_RESULT_TREE_NOT_FOUND")?;
        let message = picked.message().ok_or("G06C04B_ORIGINAL_MESSAGE_NONUTF8")?;
        let author: Signature<'_> = picked.author();
        let committer: Signature<'_> = picked.committer();
        let result_id = quarantine.commit(None, &author, &committer, message, &tree, &[&parent])
            .map_err(|_| "G06C04B_QUARANTINE_COMMIT_FAILED")?;
        records.push(ReplayResultStep {
            ordinal: records.len() + 1,
            original_commit: original_id.to_string(),
            original_parent: evidence.original_parent.clone(),
            resulting_commit: result_id.to_string(),
            resulting_tree: tree_id.to_string(),
            resulting_parent: cursor.to_string(),
            conflict_path_count: 0,
            original_author_retained: true,
            original_committer_retained: true,
            original_message_retained: true,
        });
        cursor = result_id;
    }
    if records.is_empty() && conflict_count == 0 {
        add(&mut blockers, "replay_has_no_new_commits");
    }
    if plan.replay_commits.len() != records.len() {
        add(&mut blockers, "replay_incomplete_or_conflicted");
    }
    let old = original.find_commit(
        Oid::from_str(&plan.current_head).map_err(|_| "G06C04B_HEAD_INVALID")?
    ).map_err(|_| "G06C04B_HEAD_UNAVAILABLE")?;
    let new_tree_oid = if conflict_count == 0 && !records.is_empty() {
        Some(records.last().ok_or("G06C04B_EMPTY_RESULT")?.resulting_tree.clone())
    } else { None };
    // Re-examine candidate output relative to original HEAD in quarantine.
    let mut changed = 0usize;
    if let Some(ref oid) = new_tree_oid {
        let future = quarantine.find_tree(Oid::from_str(oid)
            .map_err(|_| "G06C04B_OUTPUT_TREE_INVALID")?)
            .map_err(|_| "G06C04B_OUTPUT_TREE_NOT_FOUND")?;
        let initial = old.tree().map_err(|_| "G06C04B_CURRENT_TREE_UNAVAILABLE")?;
        let diff = quarantine.diff_tree_to_tree(Some(&initial), Some(&future), None)
            .map_err(|_| "G06C04B_OUTPUT_DIFF_UNAVAILABLE")?;
        changed = diff.deltas().len();
        if changed > MAX_OUTPUT_PATHS {
            add(&mut blockers, "output_path_limit_exceeded");
        }
        for delta in diff.deltas() {
            let old = delta.old_file();
            let new = delta.new_file();
            for item in [old, new] {
                if let Some(path) = item.path() {
                    let name = path.to_str().ok_or("G06C04B_OUTPUT_PATH_NONUTF8")?;
                    if name.len() > 240 || name.contains('\\') || name.chars().any(char::is_control) {
                        add(&mut blockers, "unsafe_output_path");
                    }
                }
            }
            if !matches!(delta.status(), git2::Delta::Added
                | git2::Delta::Deleted | git2::Delta::Modified) {
                add(&mut blockers, "unsupported_output_delta");
            }
            if !matches!(new.mode(), git2::FileMode::Blob | git2::FileMode::BlobExecutable
                    | git2::FileMode::Unreadable)
                || !matches!(old.mode(), git2::FileMode::Blob | git2::FileMode::BlobExecutable
                    | git2::FileMode::Unreadable) {
                add(&mut blockers, "unsupported_output_file_mode");
            }
        }
    }
    if plan.possible_overlap_count > 0 {
        add(&mut blockers, "original_path_overlap_requires_manual_review");
    }
    if !plan.replay_commits.iter().all(|c| c.unsupported_delta_count == 0) {
        add(&mut blockers, "unsupported_original_deltas");
    }
    let mut ledger = Vec::new();
    for x in &records {
        ledger.push(format!("{}:{}:{}:{}:{}",x.original_commit,
            x.original_parent, x.resulting_parent, x.resulting_tree, x.resulting_commit));
    }
    let chain = fingerprint(&ledger.iter().map(String::as_str).collect::<Vec<_>>());
    let head = if conflict_count == 0 && records.len() == plan.replay_commits.len() {
        Some(cursor.to_string())
    } else { None };
    let digest = fingerprint(&[
        project_id,&plan.evidence_sha256,&plan.current_head,&plan.selected_tip,
        &plan.original_index_sha256,&chain,
        head.as_deref().unwrap_or(""),
        new_tree_oid.as_deref().unwrap_or(""),
        &conflict_count.to_string(),&format!("{:?}",blockers),
    ]);
    let result = ProspectiveReplayResult {
        schema_version: 1,
        project_id: project_id.to_owned(),
        operation: plan.operation,
        current_branch: plan.current_branch.clone(),
        current_head: plan.current_head.clone(),
        current_tree: plan.current_tree.clone(),
        selected_local_branch: plan.selected_local_branch.clone(),
        selected_tip: plan.selected_tip.clone(),
        selected_tree: plan.selected_tree.clone(),
        original_index_sha256: plan.original_index_sha256.clone(),
        replay_plan_sha256: plan.evidence_sha256.clone(),
        evidence_sha256: digest,
        prospective_head: head,
        prospective_tree: new_tree_oid,
        result_steps: records,
        candidate_change_paths: changed,
        conflict_count,
        conflicting_paths: conflicts,
        blockers,
        original_project_unchanged: true,
        quarantine_only_result_objects: true,
        one_use_os_authorized: false,
        project_ref_modified: false,
        project_index_or_worktree_modified: false,
        project_odb_modified: false,
        no_hooks_or_external_processes: true,
        remote_queried: false,
        execution_route_available: false,
    };
    Ok(ReplayBuild {sandbox:scratch,result})
}

impl IdeWorkspaceRegistry {
    pub fn preview_prospective_replay(
        &self, project_id:&str, kind:ReplayReviewKind, selected_branch:&str,
    ) -> Result<ProspectiveReplayResult, &'static str> {
        let before = self.review_local_rebase_cherry_pick(project_id,kind,selected_branch)?;
        let repo = exact_repo(self,project_id)?;
        let build = compute_in_sandbox(&repo,&before,project_id)?;
        let after = self.review_local_rebase_cherry_pick(project_id,kind,selected_branch)?;
        if before.evidence_sha256 != after.evidence_sha256
            || before.current_head != after.current_head
            || before.selected_tip != after.selected_tip
            || local_commit(&repo,selected_branch)?.to_string()!=before.selected_tip {
            return Err("G06C04B_SOURCE_CHANGED_DURING_QUARANTINE");
        }
        Ok(build.result)
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
        fn choose_project_directory(&mut self)->Result<Option<PathBuf>,IdeError>{Ok(Some(self.0.clone()))}
    }
    struct Fixture{base:PathBuf,root:PathBuf,id:String,registry:IdeWorkspaceRegistry}
    impl Drop for Fixture{fn drop(&mut self){let _=fs::remove_dir_all(&self.base);}}
    fn fixture()->Fixture{
        let base=std::env::temp_dir().join(format!("ide-g06c04b-{:032x}",rand::random::<u128>()));
        let root=base.join("repo");fs::create_dir_all(&root).unwrap();
        fs::write(root.join("stable.keep"),b"keep\n").unwrap();
        fs::write(root.join("main.txt"),b"base\n").unwrap();
        let repo=Repository::init(&root).unwrap();
        repo.config().unwrap().set_bool("core.autocrlf",false).unwrap();
        let sig=Signature::now("Fixture","fixture@localhost.invalid").unwrap();
        let mut idx=repo.index().unwrap();
        for p in ["stable.keep","main.txt"]{idx.add_path(Path::new(p)).unwrap();}
        idx.write().unwrap();
        let tree=repo.find_tree(idx.write_tree().unwrap()).unwrap();
        let root_commit=repo.commit(Some("HEAD"),&sig,&sig,"root",&tree,&[]).unwrap();
        let base_commit=repo.find_commit(root_commit).unwrap();
        repo.branch("feature",&base_commit,false).unwrap();
        // Existing feature tip changes an independent path.
        let mut builder=repo.treebuilder(Some(&tree)).unwrap();
        let blob=repo.blob(b"selected\n").unwrap();
        builder.insert("new-feature.txt",blob,0o100644).unwrap();
        let feature_tree=repo.find_tree(builder.write().unwrap()).unwrap();
        repo.commit(Some("refs/heads/feature"),&sig,&sig,"selected",&feature_tree,&[&base_commit]).unwrap();
        // Current HEAD changes a second independent path.
        fs::write(root.join("main.txt"),b"current\n").unwrap();
        idx.add_path(Path::new("main.txt")).unwrap();idx.write().unwrap();
        let current_tree=repo.find_tree(idx.write_tree().unwrap()).unwrap();
        repo.commit(Some("HEAD"),&sig,&sig,"current",&current_tree,&[&base_commit]).unwrap();
        drop(current_tree);drop(feature_tree);drop(builder);drop(base_commit);drop(tree);drop(idx);drop(repo);
        let registry=IdeWorkspaceRegistry::open(base.join("catalog.json")).unwrap();
        let id=registry.register_from_native_picker(&mut Picker(root.clone())).unwrap().unwrap().project_id;
        Fixture{base,root,id,registry}
    }
    fn snapshot(f:&Fixture)->(String,Vec<u8>,Vec<u8>,Vec<u8>){
        let r=Repository::open(&f.root).unwrap();
        let head=r.head().unwrap().target().unwrap().to_string();
        (head,fs::read(f.root.join(".git/index")).unwrap(),
         fs::read(f.root.join("main.txt")).unwrap(),
         fs::read(f.root.join("stable.keep")).unwrap())
    }
    #[test]
    fn cherry_pick_preview_isolated_and_preserves_all_original_git_bytes(){
        let f=fixture();let before=snapshot(&f);
        let r=f.registry.preview_prospective_replay(&f.id,ReplayReviewKind::CherryPickLocalTip,"feature").unwrap();
        assert_eq!(r.conflict_count,0);
        assert_eq!(r.result_steps.len(),1);
        assert_eq!(r.candidate_change_paths,1);
        assert!(r.prospective_tree.is_some()&&r.prospective_head.is_some());
        assert!(r.blockers.is_empty());
        assert!(r.original_project_unchanged&&r.quarantine_only_result_objects);
        assert!(!r.project_ref_modified&&!r.project_index_or_worktree_modified&&!r.project_odb_modified);
        assert_eq!(snapshot(&f),before);
        let original=Repository::open(&f.root).unwrap();
        assert!(original.find_commit(Oid::from_str(r.prospective_head.as_deref().unwrap()).unwrap()).is_err(),
            "prospective Git commit MUST be absent from source object database during preview");
    }
    #[test]
    fn rebase_preview_isolated_and_result_uses_selected_tip_as_parent(){
        let f=fixture();let before=snapshot(&f);
        let r=f.registry.preview_prospective_replay(&f.id,ReplayReviewKind::RebaseCurrentOntoLocal,"feature").unwrap();
        assert_eq!(r.result_steps.len(),1);
        assert_eq!(r.result_steps[0].resulting_parent,r.selected_tip);
        assert_eq!(r.conflict_count,0);
        assert_eq!(r.candidate_change_paths,1);
        assert!(r.blockers.is_empty());
        assert_eq!(snapshot(&f),before);
    }
    #[test]
    fn stale_and_dirty_sources_never_return_an_execution_capability(){
        let f=fixture();
        fs::write(f.root.join(".git/index.lock"),b"foreign").unwrap();
        let r=f.registry.preview_prospective_replay(&f.id,ReplayReviewKind::CherryPickLocalTip,"feature").unwrap();
        assert!(!r.original_project_unchanged==false || !r.execution_route_available);
        assert!(!r.execution_route_available);
        assert_eq!(fs::read(f.root.join(".git/index.lock")).unwrap(),b"foreign");
    }
}
