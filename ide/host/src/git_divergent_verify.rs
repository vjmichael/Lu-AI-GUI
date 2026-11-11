//! G06C-03B: native, read-only three-way merge result verification.
//!
//! This is NOT an executor. The 6C-02 path-overlap heuristic never grants
//! publication. Conflicts and unsupported index modes fail closed. A future
//! execution gate must recompute this exact result after one-use OS consent.
use crate::{
    git_branch_operations::{exact_repo, local_commit, safe_local_name},
    git_merge_plan::ReviewedMergePlan,
    git_replay_quarantine::ReplaySandbox,
    IdeWorkspaceRegistry,
};
use git2::{MergeOptions, Oid, Repository, RepositoryState, StatusOptions};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

const MAX_ENTRIES: usize = 20_000;
const MAX_CONFLICTS: usize = 80;
const MAX_PATH: usize = 1024;
const MAX_INDEX_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifiedDivergentMerge {
    pub schema_version: u32,
    pub project_id: String,
    pub current_head: String,
    pub selected_tip: String,
    pub merge_base: String,
    pub first_parent: String,
    pub second_parent: String,
    pub proposed_entry_count: usize,
    pub resulting_index_sha256: String,
    pub source_plan_sha256: String,
    pub conflicts: Vec<String>,
    pub conflict_count: usize,
    pub publishable_candidate: bool,
    pub source_objects_written: bool,
    pub source_index_or_worktree_written: bool,
    pub authority_granted: bool,
}

fn hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn index_sha(path: &Path) -> Result<String, &'static str> {
    let path = path.join("index");
    let meta = fs::symlink_metadata(&path).map_err(|_| "G06C03B_INDEX_MISSING")?;
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > MAX_INDEX_BYTES {
        return Err("G06C03B_INDEX_UNTRUSTED");
    }
    let bytes = fs::read(path).map_err(|_| "G06C03B_INDEX_READ_FAILED")?;
    if bytes.len() as u64 != meta.len() {
        return Err("G06C03B_INDEX_CHANGED");
    }
    Ok(hex(&bytes))
}
fn safe_path(path: &[u8]) -> bool {
    !path.is_empty()
        && path.len() <= MAX_PATH
        && path.iter().all(|b| *b != 0 && *b != b'\\')
        && std::str::from_utf8(path).is_ok()
        && path.split(|b| *b == b'/')
            .all(|part| !part.is_empty() && part != b"." && part != b"..")
}
fn repo_fence(repo: &Repository) -> Result<(String, String), &'static str> {
    if repo.is_shallow() || repo.is_bare() || repo.state() != RepositoryState::Clean
        || repo.head_detached().unwrap_or(true)
        || fs::symlink_metadata(repo.path().join("index.lock")).is_ok()
    {
        return Err("G06C03B_UNSAFE_GIT_STATE");
    }
    let branch = repo.head().map_err(|_| "G06C03B_HEAD_MISSING")?;
    let oid = branch.target().ok_or("G06C03B_HEAD_NOT_DIRECT")?;
    let name = branch.shorthand().ok_or("G06C03B_BRANCH_INVALID")?;
    if !safe_local_name(name) {
        return Err("G06C03B_BRANCH_UNSAFE");
    }
    if !repo.statuses(Some(StatusOptions::new().include_untracked(true).recurse_untracked_dirs(true)))
        .map_err(|_| "G06C03B_STATUS_FAILED")?.is_empty() {
        return Err("G06C03B_DIRTY_WORKTREE");
    }
    let idx = repo.index().map_err(|_| "G06C03B_INDEX_UNAVAILABLE")?;
    if idx.has_conflicts() { return Err("G06C03B_EXISTING_CONFLICTS"); }
    let current = repo.find_commit(oid).map_err(|_| "G06C03B_COMMIT_MISSING")?;
    let tree = current.tree().map_err(|_| "G06C03B_TREE_MISSING")?;
    for entry in idx.iter() {
        if !safe_path(&entry.path) || !matches!(entry.mode, 0o100644 | 0o100755) {
            return Err("G06C03B_UNSUPPORTED_SOURCE_ENTRY");
        }
        let entry_path = std::str::from_utf8(&entry.path).map_err(|_| "G06C03B_PATH_ENCODING")?;
        if tree.get_path(Path::new(entry_path)).ok()
            .is_none_or(|item| item.id() != entry.id || item.filemode() as u32 != entry.mode) {
            return Err("G06C03B_INDEX_HEAD_MISMATCH");
        }
    }
    Ok((name.to_owned(), oid.to_string()))
}

impl IdeWorkspaceRegistry {
    /// Three-way Git content merge in memory. Never grants execution.
    pub fn verify_divergent_merge_result(
        &self, project_id: &str, selected_branch: &str, evidence_sha256: &str,
    ) -> Result<VerifiedDivergentMerge, &'static str> {
        if !safe_local_name(selected_branch) || evidence_sha256.len() != 64
            || !evidence_sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("G06C03B_INVALID_REQUEST");
        }
        let plan: ReviewedMergePlan = self.preview_reviewed_local_merge(project_id, selected_branch)?;
        if plan.evidence_sha256 != evidence_sha256 {
            return Err("G06C03B_STALE_PLAN");
        }
        if plan.classification != "divergent_merge_candidate" || plan.merge_base_count != 1
            || plan.merge_base_commit.is_none() || plan.current_head == plan.selected_tip {
            return Err("G06C03B_NOT_UNIQUE_DIVERGENCE");
        }
        // The only 6C-02 blockers this verifier can supersede are heuristic
        // uncertainty and possible content overlap. All other blockers deny.
        if plan.blockers.iter().any(|b| !matches!(*b,
            "divergent_result_tree_not_verified" | "potential_conflicts_require_manual_review")) {
            return Err("G06C03B_PLAN_BLOCKED");
        }
        let repo = exact_repo(self, project_id)?;
        let before_index = index_sha(repo.path())?;
        let before_fence = repo_fence(&repo)?;
        if before_fence.0 != plan.current_branch || before_fence.1 != plan.current_head
            || before_index != plan.index_sha256
            || local_commit(&repo, selected_branch)?.to_string() != plan.selected_tip {
            return Err("G06C03B_SOURCE_CHANGED");
        }
        let base = Oid::from_str(plan.merge_base_commit.as_deref().unwrap())
            .map_err(|_| "G06C03B_BAD_BASE")?;
        let ours = Oid::from_str(&plan.current_head).map_err(|_| "G06C03B_BAD_HEAD")?;
        let theirs = Oid::from_str(&plan.selected_tip).map_err(|_| "G06C03B_BAD_TIP")?;
        let bases = repo.merge_bases(ours, theirs).map_err(|_| "G06C03B_BASE_CHECK_FAILED")?;
        if bases.len() != 1 || bases.get(0) != Some(base) {
            return Err("G06C03B_BASE_CHANGED");
        }
        let base_tree = repo.find_commit(base).and_then(|c| c.tree())
            .map_err(|_| "G06C03B_BASE_TREE_MISSING")?;
        let ours_tree = repo.find_commit(ours).and_then(|c| c.tree())
            .map_err(|_| "G06C03B_OURS_TREE_MISSING")?;
        let theirs_tree = repo.find_commit(theirs).and_then(|c| c.tree())
            .map_err(|_| "G06C03B_THEIRS_TREE_MISSING")?;
        // The result may contain newly auto-merged blob objects. Compute it
        // only in a separate bare object store with source as a read-only alternate.
        let scratch = ReplaySandbox::open(&repo)?;
        let quarantine = scratch.repository()?;
        let base_tree = quarantine.find_tree(base_tree.id()).map_err(|_| "G06C03B_BASE_IN_QUARANTINE")?;
        let ours_tree = quarantine.find_tree(ours_tree.id()).map_err(|_| "G06C03B_OURS_IN_QUARANTINE")?;
        let theirs_tree = quarantine.find_tree(theirs_tree.id()).map_err(|_| "G06C03B_THEIRS_IN_QUARANTINE")?;
        let mut opts = MergeOptions::new();
        let merged = quarantine.merge_trees(&base_tree, &ours_tree, &theirs_tree, Some(&mut opts))
            .map_err(|_| "G06C03B_CONTENT_MERGE_FAILED")?;
        let mut conflicts = Vec::new();
        let mut count = 0usize;
        if merged.has_conflicts() {
            for c in merged.conflicts().map_err(|_| "G06C03B_CONFLICT_SCAN_FAILED")? {
                let c = c.map_err(|_| "G06C03B_CONFLICT_SCAN_FAILED")?;
                count += 1;
                if count > MAX_ENTRIES { return Err("G06C03B_TOO_MANY_CONFLICTS"); }
                let bytes = c.our.as_ref().or(c.their.as_ref()).or(c.ancestor.as_ref())
                    .ok_or("G06C03B_CONFLICT_WITHOUT_PATH")?.path.as_slice();
                if !safe_path(bytes) { return Err("G06C03B_UNSAFE_CONFLICT_PATH"); }
                if conflicts.len() < MAX_CONFLICTS {
                    conflicts.push(std::str::from_utf8(bytes)
                        .map_err(|_| "G06C03B_CONFLICT_ENCODING")?.to_owned());
                }
            }
        }
        let mut digest = Sha256::new();
        digest.update(b"lu-ide-g06c03b-merged-index-v1\n");
        let mut entries = 0usize;
        for e in merged.iter() {
            entries += 1;
            if entries > MAX_ENTRIES { return Err("G06C03B_RESULT_TOO_LARGE"); }
            if !safe_path(&e.path) || !matches!(e.mode, 0o100644 | 0o100755) {
                return Err("G06C03B_UNSUPPORTED_RESULT_ENTRY");
            }
            digest.update((e.path.len() as u64).to_le_bytes());
            digest.update(&e.path);
            digest.update(e.id.as_bytes());
            digest.update(e.mode.to_le_bytes());
            digest.update(e.flags.to_le_bytes());
            digest.update(e.flags_extended.to_le_bytes());
        }
        if repo_fence(&repo)? != before_fence || index_sha(repo.path())? != before_index
            || local_commit(&repo, selected_branch)?.to_string() != plan.selected_tip {
            return Err("G06C03B_SOURCE_CHANGED_DURING_MERGE");
        }
        Ok(VerifiedDivergentMerge {
            schema_version: 1, project_id: project_id.to_owned(),
            current_head: plan.current_head.clone(), selected_tip: plan.selected_tip.clone(),
            merge_base: plan.merge_base_commit.unwrap(),
            first_parent: plan.current_head, second_parent: plan.selected_tip,
            proposed_entry_count: entries,
            resulting_index_sha256: format!("{:x}", digest.finalize()),
            source_plan_sha256: plan.evidence_sha256,
            conflicts, conflict_count: count, publishable_candidate: count == 0,
            source_objects_written: false, source_index_or_worktree_written: false,
            authority_granted: false,
        })
    }
}
