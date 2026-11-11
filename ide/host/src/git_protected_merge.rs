//! IDE 6C-03A: protected fast-forward merge publication with durable recovery.
//!
//! Only the Tauri OS-confirmed route may invoke this host operation. The
//! WebView never receives an approval credential. Review digests are claims,
//! not authority. Divergent or conflicted merges remain unimplemented.
//!
//! A successful fast-forward requires a safe libgit2 index/worktree checkout
//! BEFORE the exact old-HEAD ref CAS. A failure or process death in that window
//! may leave partially advanced files while HEAD remains old. Durable recovery
//! is observational only and never silently rolls back or retries that window.
use crate::{git_branch_operations::{exact_repo, local_commit, safe_local_name}, IdeWorkspaceRegistry};
use git2::{build::CheckoutBuilder, ObjectType, Oid, Repository, RepositoryState, Status, StatusOptions};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU8, Ordering},
};

const MAX_JOURNAL_BYTES: u64 = 1024 * 1024;
const MAX_ENTRY_BYTES: usize = 4096;
const MAX_INDEX_BYTES: u64 = 8 * 1024 * 1024;
const MAX_CHECKOUT_PATHS: usize = 80;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MergeJournal {
    schema_version: u32,
    operation_id: String,
    project_id: String,
    phase: String,
    branch_ref: String,
    selected_local_branch: String,
    previous_head: String,
    selected_tip: String,
    previous_tree: String,
    selected_tree: String,
    original_index_sha256: String,
    selected_index_sha256: String,
    plan_sha256: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtectedFastForwardReceipt {
    pub schema_version: u32,
    pub project_id: String,
    pub operation_id: String,
    pub branch_ref: String,
    pub previous_head: String,
    pub selected_tip: String,
    pub resulting_head: String,
    pub resulting_tree: String,
    pub index_sha256: String,
    pub plan_sha256: String,
    pub original_history_retained: bool,
    pub worktree_and_index_verified: bool,
    pub exact_ref_cas_verified: bool,
    pub one_use_os_approval_required: bool,
    pub recovery_journal_verified: bool,
    pub hooks_executed: bool,
    pub remote_updated: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtectedMergeRecovery {
    pub schema_version: u32,
    pub project_id: String,
    pub state: &'static str,
    pub last_phase: String,
    pub branch_matches_original: bool,
    pub branch_matches_selected: bool,
    pub git_index_lock_present: bool,
    pub current_index_matches_original: bool,
    pub current_index_matches_selected: bool,
    pub worktree_matches_current_index: bool,
    pub operator_reconciliation_required: bool,
    pub automatic_retry: bool,
    pub automatic_ref_rewrite: bool,
    pub automatic_checkout: bool,
    pub automatic_lock_deletion: bool,
}

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn exact_index_sha(gitdir: &Path) -> Result<String, &'static str> {
    let path = gitdir.join("index");
    let meta = fs::symlink_metadata(&path).map_err(|_| "G06C03_INDEX_MISSING")?;
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > MAX_INDEX_BYTES {
        return Err("G06C03_INDEX_UNTRUSTED");
    }
    let bytes = fs::read(path).map_err(|_| "G06C03_INDEX_UNREADABLE")?;
    if bytes.len() as u64 != meta.len() {
        return Err("G06C03_INDEX_CHANGED_DURING_READ");
    }
    Ok(sha(&bytes))
}

fn current_branch(repo: &Repository) -> Result<(String, Oid), &'static str> {
    if repo.is_shallow() || repo.state() != RepositoryState::Clean ||
        repo.head_detached().unwrap_or(true) {
        return Err("G06C03_REPOSITORY_STATE_UNSAFE");
    }
    let head = repo.head().map_err(|_| "G06C03_HEAD_UNAVAILABLE")?;
    let branch = head.name().ok_or("G06C03_HEAD_NAME_UNAVAILABLE")?;
    let branch = branch.strip_prefix("refs/heads/").ok_or("G06C03_HEAD_NOT_LOCAL")?;
    if !safe_local_name(branch) {
        return Err("G06C03_UNSAFE_CURRENT_BRANCH");
    }
    Ok((branch.to_owned(), head.target().ok_or("G06C03_HEAD_NOT_DIRECT")?))
}

fn journal_file(
    registry: &IdeWorkspaceRegistry,
    project_id: &str,
) -> Result<PathBuf, &'static str> {
    registry.registered_git_root(project_id).map_err(|_| "G06C03_UNKNOWN_PROJECT")?;
    if project_id.is_empty() || project_id.len() > 128 ||
        !project_id.bytes().all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, b'_' | b'-')) {
        return Err("G06C03_BAD_PROJECT_ID");
    }
    let root = registry.catalog.parent().ok_or("G06C03_NO_APPDATA")?
        .join("ide-g06c03-protected-merges");
    fs::create_dir_all(&root).map_err(|_| "G06C03_JOURNAL_DIRECTORY_UNAVAILABLE")?;
    let meta = fs::symlink_metadata(&root).map_err(|_| "G06C03_JOURNAL_DIRECTORY_UNAVAILABLE")?;
    if !meta.is_dir() || meta.file_type().is_symlink() {
        return Err("G06C03_JOURNAL_DIRECTORY_UNTRUSTED");
    }
    Ok(root.join(format!("{project_id}.jsonl")))
}

fn read_latest(path: &Path) -> Result<Option<MergeJournal>, &'static str> {
    let meta = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("G06C03_JOURNAL_UNREADABLE"),
    };
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > MAX_JOURNAL_BYTES {
        return Err("G06C03_JOURNAL_UNTRUSTED");
    }
    let bytes = fs::read(path).map_err(|_| "G06C03_JOURNAL_UNREADABLE")?;
    if bytes.last() != Some(&b'\n') {
        return Err("G06C03_JOURNAL_INCOMPLETE");
    }
    let last = bytes.split(|b| *b == b'\n').filter(|p| !p.is_empty())
        .last().ok_or("G06C03_JOURNAL_INCOMPLETE")?;
    if last.len() > MAX_ENTRY_BYTES {
        return Err("G06C03_JOURNAL_RECORD_OVERSIZED");
    }
    let record: MergeJournal =
        serde_json::from_slice(last).map_err(|_| "G06C03_JOURNAL_CORRUPTED")?;
    if record.schema_version != 1
        || !matches!(record.phase.as_str(),
            "prepared" | "checkout_intent" | "checkout_verified" | "publishing"
            | "ref_published" | "verified" | "cancelled")
        || record.operation_id.len() != 32
        || !record.operation_id.bytes().all(|ch| ch.is_ascii_hexdigit())
    {
        return Err("G06C03_JOURNAL_RECORD_INVALID");
    }
    Ok(Some(record))
}

fn append(path: &Path, record: &MergeJournal) -> Result<(), &'static str> {
    let previous_size = match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_file() && !meta.file_type().is_symlink() => meta.len(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => 0,
        _ => return Err("G06C03_JOURNAL_UNTRUSTED"),
    };
    let bytes = serde_json::to_vec(record).map_err(|_| "G06C03_JOURNAL_SERIALIZE")?;
    if bytes.len() > MAX_ENTRY_BYTES || previous_size + bytes.len() as u64 + 1 > MAX_JOURNAL_BYTES {
        return Err("G06C03_JOURNAL_CAP_REACHED");
    }
    let mut file = OpenOptions::new().append(true).create(true).open(path)
        .map_err(|_| "G06C03_JOURNAL_WRITE_DENIED")?;
    if file.metadata().map_err(|_| "G06C03_JOURNAL_UNREADABLE")?.len() != previous_size {
        return Err("G06C03_JOURNAL_CONCURRENT_APPEND");
    }
    file.write_all(&bytes).and_then(|_| file.write_all(b"\n"))
        .and_then(|_| file.sync_all()).map_err(|_| "G06C03_JOURNAL_SYNC_FAILED")?;
    #[cfg(unix)]
    File::open(path.parent().ok_or("G06C03_JOURNAL_PARENT_MISSING")?)
        .and_then(|dir| dir.sync_all()).map_err(|_| "G06C03_JOURNAL_DIR_SYNC_FAILED")?;
    Ok(())
}

fn changed_path_is_plain_safe(root: &Path, path: &str) -> Result<(), &'static str> {
    // 6C03A is narrower than Git's full path vocabulary. Refuse Unicode,
    // colon, backslash, Windows devices, trailing dot/space and all aliases.
    if path.is_empty() || path.len() > 240 || !path.is_ascii() ||
        path.bytes().any(|b| !(b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b'/'))) {
        return Err("G06C03_CHANGED_PATH_NOT_SUPPORTED");
    }
    let mut cursor = root.to_path_buf();
    for item in path.split('/') {
        let raw = item.split('.').next().unwrap_or("").to_ascii_uppercase();
        if item.is_empty() || item == "." || item == ".." || item.ends_with('.') ||
            matches!(raw.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "COM1" | "COM2" |
              "COM3" | "COM4" | "COM5" | "COM6" | "COM7" | "COM8" | "COM9" |
              "LPT1" | "LPT2" | "LPT3" | "LPT4" | "LPT5" | "LPT6" | "LPT7" | "LPT8" | "LPT9") {
            return Err("G06C03_DANGEROUS_PATH_COMPONENT");
        }
        cursor.push(item);
        match fs::symlink_metadata(&cursor) {
            Ok(meta) if meta.file_type().is_symlink() => return Err("G06C03_SYMLINK_IN_WORKTREE"),
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err("G06C03_PATH_METADATA_UNVERIFIABLE"),
        }
    }
    Ok(())
}

fn no_wt_changes(repo: &Repository) -> Result<bool, &'static str> {
    let mut opts = StatusOptions::new();
    opts.include_untracked(true)
        .recurse_untracked_dirs(true)
        .include_ignored(false)
        .renames_head_to_index(false)
        .renames_index_to_workdir(false);
    let statuses = repo.statuses(Some(&mut opts)).map_err(|_| "G06C03_STATUS_UNAVAILABLE")?;
    Ok(statuses.iter().all(|entry| !entry.status().intersects(
        Status::WT_MODIFIED | Status::WT_DELETED | Status::WT_NEW |
        Status::WT_RENAMED | Status::WT_TYPECHANGE | Status::CONFLICTED
    )))
}

fn index_matches_tree(repo: &Repository, expected: &str) -> Result<bool, &'static str> {
    let mut index = repo.index().map_err(|_| "G06C03_INDEX_NOT_LOADED")?;
    index.read(true).map_err(|_| "G06C03_INDEX_RELOAD_FAILED")?;
    if index.has_conflicts() {
        return Ok(false);
    }
    let tree_oid = index.write_tree_to(repo).map_err(|_| "G06C03_INDEX_TREE_UNAVAILABLE")?;
    Ok(tree_oid.to_string() == expected)
}

fn matching_commit(repo: &Repository, ref_name: &str) -> Result<Option<String>, &'static str> {
    Ok(repo.find_reference(ref_name).map_err(|_| "G06C03_BRANCH_REF_NOT_FOUND")?
        .target().map(|oid| oid.to_string()))
}

fn approved_fast_forward_plan(
    registry: &IdeWorkspaceRegistry,
    project_id: &str,
    selected_branch: &str,
    expected_digest: &str,
) -> Result<crate::git_merge_plan::ReviewedMergePlan, &'static str> {
    if !safe_local_name(selected_branch) || expected_digest.len() != 64 ||
        !expected_digest.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("G06C03_BAD_REQUEST");
    }
    let plan = registry.preview_reviewed_local_merge(project_id, selected_branch)?;
    if plan.evidence_sha256 != expected_digest {
        return Err("G06C03_STALE_OR_FORGED_MERGE_PLAN");
    }
    if plan.classification != "fast_forward_candidate"
        || plan.merge_base_count != 1
        || plan.merge_base_commit.as_deref() != Some(plan.current_head.as_str())
        || !plan.blockers.is_empty() {
        return Err("G06C03_NON_FF_OR_BLOCKED_MERGE_DENIED");
    }
    if plan.changed_path_count == 0 || plan.changed_path_count > MAX_CHECKOUT_PATHS
        || plan.visible_paths.len() != plan.changed_path_count
        || plan.omitted_path_count != 0 || plan.possible_conflict_count != 0
        || plan.visible_paths.iter().any(|p| p.classification != "selected_only_change")
    {
        return Err("G06C03_CHECKOUT_CHANGESET_NOT_SUPPORTED");
    }
    Ok(plan)
}

impl IdeWorkspaceRegistry {
    /// Native source revalidation before one-use OS consent. Not an approval.
    pub fn revalidate_protected_fast_forward(
        &self, project_id: &str, selected_branch: &str, digest: &str,
    ) -> Result<crate::git_merge_plan::ReviewedMergePlan, &'static str> {
        approved_fast_forward_plan(self, project_id, selected_branch, digest)
    }

    /// Host-only authorized after the independent, visible native OS dialog.
    /// Cancellation CAS=0->1 occurs immediately before checkout intent.
    pub fn execute_os_approved_fast_forward(
        &self, project_id: &str, selected_branch: &str,
        digest: &str, cancel: &AtomicU8,
    ) -> Result<ProtectedFastForwardReceipt, &'static str> {
        if cancel.load(Ordering::SeqCst) != 0 {
            return Err("G06C03_CANCELLED_BEFORE_EXECUTION");
        }
        let plan = approved_fast_forward_plan(self, project_id, selected_branch, digest)?;
        let repo = exact_repo(self, project_id)?;
        let root = self.registered_git_root(project_id)
            .map_err(|_| "G06C03_PROJECT_NOT_OPEN")?;
        let gitdir = repo.path();
        let (branch, old_oid) = current_branch(&repo)?;
        if branch != plan.current_branch || old_oid.to_string() != plan.current_head
            || local_commit(&repo, selected_branch)?.to_string() != plan.selected_tip {
            return Err("G06C03_HEAD_OR_SELECTED_MOVED");
        }
        if fs::symlink_metadata(gitdir.join("index.lock")).is_ok() {
            return Err("G06C03_FOREIGN_INDEX_LOCK");
        }
        if exact_index_sha(gitdir)? != plan.index_sha256
            || !index_matches_tree(&repo, &plan.current_tree)?
            || !no_wt_changes(&repo)? {
            return Err("G06C03_SOURCE_INDEX_OR_WORKTREE_DRIFT");
        }
        for item in &plan.visible_paths {
            changed_path_is_plain_safe(&root, &item.relative_path)?;
        }
        let journal_path = journal_file(self, project_id)?;
        if let Some(last) = read_latest(&journal_path)? {
            if !matches!(last.phase.as_str(), "verified" | "cancelled") {
                return Err("G06C03_INTERRUPTED_MERGE_REQUIRES_REVIEW");
            }
        }
        let mut journal = MergeJournal {
            schema_version: 1,
            operation_id: format!("{:032x}", rand::random::<u128>()),
            project_id: project_id.to_owned(),
            phase: "prepared".into(),
            branch_ref: format!("refs/heads/{branch}"),
            selected_local_branch: selected_branch.to_owned(),
            previous_head: plan.current_head.clone(),
            selected_tip: plan.selected_tip.clone(),
            previous_tree: plan.current_tree.clone(),
            selected_tree: plan.selected_tree.clone(),
            original_index_sha256: plan.index_sha256.clone(),
            selected_index_sha256: String::new(),
            plan_sha256: plan.evidence_sha256.clone(),
        };
        if cancel.load(Ordering::SeqCst) != 0 {
            return Err("G06C03_CANCELLED_BEFORE_PREPARE");
        }
        append(&journal_path, &journal)?;
        if cancel.compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst).is_err() {
            journal.phase = "cancelled".into();
            append(&journal_path, &journal)?;
            return Err("G06C03_CANCELLED_BEFORE_CHECKOUT");
        }
        // The start of disk mutation is durably announced BEFORE checkout.
        journal.phase = "checkout_intent".into();
        append(&journal_path, &journal)?;
        if matching_commit(&repo, &journal.branch_ref)?.as_deref() != Some(&journal.previous_head)
            || local_commit(&repo, selected_branch)?.to_string() != journal.selected_tip
            || exact_index_sha(gitdir)? != journal.original_index_sha256
            || !no_wt_changes(&repo)? {
            return Err("G06C03_SOURCE_CHANGED_BEFORE_CHECKOUT");
        }
        let target = repo.find_object(
            Oid::from_str(&plan.selected_tip).map_err(|_| "G06C03_SELECTED_OID_INVALID")?,
            Some(ObjectType::Commit),
        ).map_err(|_| "G06C03_SELECTED_COMMIT_NOT_FOUND")?;
        // No force, untracked removal, ignored overwrite, or index bypass.
        let mut checkout = CheckoutBuilder::new();
        checkout.safe().recreate_missing(true).remove_untracked(false)
            .remove_ignored(false).overwrite_ignored(false).update_index(true);
        repo.checkout_tree(&target, Some(&mut checkout))
            .map_err(|_| "G06C03_SAFE_CHECKOUT_FAILED_RECOVERY_REQUIRED")?;
        if !index_matches_tree(&repo, &plan.selected_tree)? || !no_wt_changes(&repo)?
            || fs::symlink_metadata(gitdir.join("index.lock")).is_ok() {
            return Err("G06C03_CHECKOUT_POSTCONDITION_UNVERIFIED");
        }
        journal.selected_index_sha256 = exact_index_sha(gitdir)?;
        journal.phase = "checkout_verified".into();
        append(&journal_path, &journal)?;
        if matching_commit(&repo, &journal.branch_ref)?.as_deref() != Some(&journal.previous_head)
            || local_commit(&repo, selected_branch)?.to_string() != journal.selected_tip
            || exact_index_sha(gitdir)? != journal.selected_index_sha256 {
            return Err("G06C03_REF_OR_INDEX_CHANGED_BEFORE_PUBLICATION");
        }
        journal.phase = "publishing".into();
        append(&journal_path, &journal)?;
        repo.reference_matching(
            &journal.branch_ref,
            Oid::from_str(&journal.selected_tip).map_err(|_| "G06C03_NEW_HEAD_INVALID")?,
            true,
            old_oid,
            "Lu IDE: independently reviewed one-use OS-approved local fast-forward",
        ).map_err(|_| "G06C03_REF_CAS_FAILED_RECOVERY_REQUIRED")?;
        #[cfg(test)]
        {
            // Genuine abort only in a named child of the disposable G06C03
            // fixture. Never available to production/release binaries.
            if let Some(marker_path) = std::env::var_os("IDE_G06C03_ABORT_AFTER_REF_CAS") {
                let root_canonical = fs::canonicalize(&root).ok();
                let claimed = fs::canonicalize(PathBuf::from(marker_path)).ok();
                let temp = fs::canonicalize(std::env::temp_dir()).ok();
                let marker = root.join(".g06c03-abort-fixture");
                if root_canonical.is_some() && claimed == root_canonical
                    && root_canonical.as_ref().zip(temp.as_ref())
                        .is_some_and(|(a,b)| a.starts_with(b))
                    && root.file_name().is_some_and(|s| s == "repo")
                    && root.parent().and_then(|p| p.file_name())
                        .is_some_and(|s| s.to_string_lossy().starts_with("ide-g06c03-"))
                    && fs::read(marker).ok().as_deref() == Some(b"G06C03_DISPOSABLE_ONLY\n") {
                    std::process::abort();
                }
            }
        }
        journal.phase = "ref_published".into();
        append(&journal_path, &journal)?;
        if matching_commit(&repo, &journal.branch_ref)?.as_deref() != Some(&journal.selected_tip)
            || exact_index_sha(gitdir)? != journal.selected_index_sha256
            || !index_matches_tree(&repo, &journal.selected_tree)?
            || !no_wt_changes(&repo)?
            || !repo.statuses(Some(StatusOptions::new().include_untracked(true)
                .recurse_untracked_dirs(true))).map_err(|_| "G06C03_FINAL_STATUS_FAILED")?.is_empty()
        {
            return Err("G06C03_POST_CAS_INDEPENDENT_READBACK_FAILED");
        }
        journal.phase = "verified".into();
        append(&journal_path, &journal)?;
        Ok(ProtectedFastForwardReceipt {
            schema_version: 1,
            project_id: project_id.to_owned(),
            operation_id: journal.operation_id,
            branch_ref: journal.branch_ref,
            previous_head: journal.previous_head,
            selected_tip: journal.selected_tip.clone(),
            resulting_head: journal.selected_tip,
            resulting_tree: journal.selected_tree,
            index_sha256: journal.selected_index_sha256,
            plan_sha256: journal.plan_sha256,
            original_history_retained: true,
            worktree_and_index_verified: true,
            exact_ref_cas_verified: true,
            one_use_os_approval_required: true,
            recovery_journal_verified: true,
            hooks_executed: false,
            remote_updated: false,
        })
    }

    /// Read-only interrupted-operation evidence. No lock removal or retry.
    pub fn inspect_protected_merge_recovery(
        &self, project_id: &str,
    ) -> Result<ProtectedMergeRecovery, &'static str> {
        let repo = exact_repo(self, project_id)?;
        let last = read_latest(&journal_file(self, project_id)?)?;
        let index_lock = fs::symlink_metadata(repo.path().join("index.lock")).is_ok();
        let head = repo.head().ok().and_then(|reference| reference.target())
            .map(|oid| oid.to_string());
        let index_sha = exact_index_sha(repo.path()).ok();
        let (state, phase, match_old, match_new, index_old, index_new) = match last {
            None => ("no_journal", String::new(), false, false, false, false),
            Some(ref j) if j.project_id != project_id => {
                return Err("G06C03_RECOVERY_WRONG_PROJECT");
            }
            Some(j) => {
                let old = head.as_deref() == Some(&j.previous_head);
                let new = head.as_deref() == Some(&j.selected_tip);
                let old_index = index_sha.as_deref() == Some(&j.original_index_sha256);
                let new_index = !j.selected_index_sha256.is_empty()
                    && index_sha.as_deref() == Some(&j.selected_index_sha256);
                let state = if j.phase == "verified" && new && new_index {
                    "verified_historical_fast_forward"
                } else if j.phase == "cancelled" && old && old_index {
                    "cancelled_before_checkout"
                } else if new && new_index {
                    "published_ref_requires_operator_receipt"
                } else if old && new_index {
                    "checkout_completed_ref_not_published"
                } else if old && old_index && matches!(j.phase.as_str(), "prepared" | "checkout_intent") {
                    "prepublication_no_index_change_pending_review"
                } else {
                    "ambiguous_merge_files_or_ref_drift"
                };
                (state, j.phase, old, new, old_index, new_index)
            }
        };
        let wt_matches = no_wt_changes(&repo).unwrap_or(false);
        let requires_review = index_lock || !wt_matches || !matches!(state,
            "no_journal" | "verified_historical_fast_forward" | "cancelled_before_checkout");
        Ok(ProtectedMergeRecovery {
            schema_version: 1,
            project_id: project_id.to_owned(),
            state,
            last_phase: phase,
            branch_matches_original: match_old,
            branch_matches_selected: match_new,
            git_index_lock_present: index_lock,
            current_index_matches_original: index_old,
            current_index_matches_selected: index_new,
            worktree_matches_current_index: wt_matches,
            operator_reconciliation_required: requires_review,
            automatic_retry: false,
            automatic_ref_rewrite: false,
            automatic_checkout: false,
            automatic_lock_deletion: false,
        })
    }

    /// Independently verify the *already published* ref and worktree before
    /// appending an observation. Only the separate native OS dialog may call
    /// this. No ref update/checkout/rollback is ever done by reconciliation.
    pub fn reconcile_protected_merge_receipt(
        &self, project_id: &str,
    ) -> Result<ProtectedMergeRecovery, &'static str> {
        let repo = exact_repo(self, project_id)?;
        if fs::symlink_metadata(repo.path().join("index.lock")).is_ok() {
            return Err("G06C03_RECOVERY_FOREIGN_LOCK");
        }
        let path = journal_file(self, project_id)?;
        let mut last = read_latest(&path)?.ok_or("G06C03_RECOVERY_NO_JOURNAL")?;
        if last.project_id != project_id {
            return Err("G06C03_RECOVERY_PROJECT_MISMATCH");
        }
        let observed = self.inspect_protected_merge_recovery(project_id)?;
        if !observed.worktree_matches_current_index {
            return Err("G06C03_RECOVERY_WORKTREE_DRIFT");
        }
        if observed.state == "published_ref_requires_operator_receipt" &&
            matches!(last.phase.as_str(), "publishing" | "ref_published")
        {
            if !index_matches_tree(&repo, &last.selected_tree)?
                || matching_commit(&repo, &last.branch_ref)?.as_deref() != Some(&last.selected_tip)
                || local_commit(&repo, &last.selected_local_branch)?.to_string() != last.selected_tip
                || !repo.statuses(Some(StatusOptions::new().include_untracked(true)
                    .recurse_untracked_dirs(true)))
                    .map_err(|_| "G06C03_RECOVERY_STATUS_UNAVAILABLE")?.is_empty() {
                return Err("G06C03_RECOVERY_REF_OR_TREE_NOT_VERIFIED");
            }
            last.phase = "verified".into();
        } else if observed.state == "prepublication_no_index_change_pending_review" &&
            matches!(last.phase.as_str(), "prepared" | "checkout_intent") &&
            index_matches_tree(&repo, &last.previous_tree)?
            && matching_commit(&repo, &last.branch_ref)?.as_deref() == Some(&last.previous_head)
        {
            last.phase = "cancelled".into();
        } else {
            return Err("G06C03_RECOVERY_MANUAL_REPAIR_REQUIRED");
        }
        append(&path, &last)?;
        self.inspect_protected_merge_recovery(project_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{IdeError, NativeDirectoryPicker};
    use git2::Signature;
    use std::path::PathBuf;

    struct Picker(PathBuf);
    impl NativeDirectoryPicker for Picker {
        fn choose_project_directory(&mut self) -> Result<Option<PathBuf>, IdeError> {
            Ok(Some(self.0.clone()))
        }
    }
    struct Fixture {
        base: PathBuf,
        root: PathBuf,
        id: String,
        registry: IdeWorkspaceRegistry,
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.base);
        }
    }
    fn fixture() -> Fixture {
        let base = std::env::temp_dir().join(format!("ide-g06c03-{:032x}", rand::random::<u128>()));
        let root = base.join("repo");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("tracked.txt"), b"original\n").unwrap();
        fs::write(root.join("unrelated.keep"), b"not touched\n").unwrap();
        let repo = Repository::init(&root).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("tracked.txt")).unwrap();
        index.add_path(Path::new("unrelated.keep")).unwrap();
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let sig = Signature::now("Merge fixture", "merge@localhost.invalid").unwrap();
        let first = repo.commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[]).unwrap();
        drop(tree);
        let original = repo.find_commit(first).unwrap();
        repo.branch("feature", &original, false).unwrap();
        let base_tree = original.tree().unwrap();
        let mut builder = repo.treebuilder(Some(&base_tree)).unwrap();
        let next = repo.blob(b"feature revision\n").unwrap();
        builder.insert("tracked.txt", next, 0o100644).unwrap();
        let new_tree = repo.find_tree(builder.write().unwrap()).unwrap();
        repo.commit(Some("refs/heads/feature"), &sig, &sig,
            "feature", &new_tree, &[&original]).unwrap();
        drop(new_tree);
        drop(builder);
        drop(base_tree);
        drop(original);
        drop(index);
        drop(repo);
        let registry = IdeWorkspaceRegistry::open(base.join("catalog.json")).unwrap();
        let id = registry.register_from_native_picker(&mut Picker(root.clone()))
            .unwrap().unwrap().project_id;
        Fixture { base, root, id, registry }
    }
    fn reviewed(x: &Fixture) -> String {
        x.registry.preview_reviewed_local_merge(&x.id, "feature").unwrap().evidence_sha256
    }
    #[test]
    fn real_fast_forward_advances_head_index_and_worktree_without_changing_other_file() {
        let x = fixture();
        let unrelated = fs::read(x.root.join("unrelated.keep")).unwrap();
        let previous = Repository::open(&x.root).unwrap().head().unwrap().target().unwrap();
        let digest = reviewed(&x);
        let done = x.registry.execute_os_approved_fast_forward(
            &x.id, "feature", &digest, &AtomicU8::new(0)).unwrap();
        let repo = Repository::open(&x.root).unwrap();
        assert_eq!(done.previous_head, previous.to_string());
        assert_eq!(repo.head().unwrap().target().unwrap().to_string(), done.resulting_head);
        assert_eq!(local_commit(&repo, "feature").unwrap().to_string(), done.resulting_head);
        assert_eq!(fs::read(x.root.join("tracked.txt")).unwrap(), b"feature revision\n");
        assert_eq!(fs::read(x.root.join("unrelated.keep")).unwrap(), unrelated);
        assert!(index_matches_tree(&repo, &done.resulting_tree).unwrap());
        assert!(repo.statuses(None).unwrap().is_empty());
        assert!(done.exact_ref_cas_verified && done.recovery_journal_verified);
        assert!(done.original_history_retained && done.one_use_os_approval_required);
        assert!(!done.hooks_executed && !done.remote_updated);
        let reopened = IdeWorkspaceRegistry::open(x.base.join("catalog.json")).unwrap();
        let state = reopened.inspect_protected_merge_recovery(&x.id).unwrap();
        assert_eq!(state.state, "verified_historical_fast_forward");
        assert!(!state.operator_reconciliation_required);
        assert!(!state.automatic_ref_rewrite);
    }
    #[test]
    fn stale_plan_cancellation_foreign_lock_and_untrusted_branch_are_denied_without_writes() {
        let x = fixture();
        let first = reviewed(&x);
        assert_eq!(x.registry.execute_os_approved_fast_forward(
            &x.id, "feature", &first, &AtomicU8::new(2)).unwrap_err(),
            "G06C03_CANCELLED_BEFORE_EXECUTION");
        assert_eq!(fs::read(x.root.join("tracked.txt")).unwrap(), b"original\n");
        assert!(x.registry.execute_os_approved_fast_forward(
            &x.id, "../feature", &first, &AtomicU8::new(0)).is_err());
        let lock = x.root.join(".git/index.lock");
        fs::write(&lock, b"foreign process").unwrap();
        assert_eq!(x.registry.execute_os_approved_fast_forward(
            &x.id, "feature", &first, &AtomicU8::new(0)).unwrap_err(),
            "G06C03_NON_FF_OR_BLOCKED_MERGE_DENIED");
        assert_eq!(fs::read(&lock).unwrap(), b"foreign process");
        fs::remove_file(lock).unwrap();
        fs::write(x.root.join("tracked.txt"), b"other change\n").unwrap();
        assert!(x.registry.execute_os_approved_fast_forward(
            &x.id, "feature", &first, &AtomicU8::new(0)).is_err());
    }
    #[test]
    fn recovery_rejects_unpublished_checkout_and_foreign_index_lock() {
        let x = fixture();
        let digest = reviewed(&x);
        let repo = Repository::open(&x.root).unwrap();
        let original_ref = repo.head().unwrap().target().unwrap();
        let plan = x.registry.preview_reviewed_local_merge(&x.id, "feature").unwrap();
        let journal_path = journal_file(&x.registry, &x.id).unwrap();
        let mut record = MergeJournal {
            schema_version: 1,
            operation_id: "11111111111111111111111111111111".into(),
            project_id: x.id.clone(),
            phase: "checkout_intent".into(),
            branch_ref: format!("refs/heads/{}", plan.current_branch),
            selected_local_branch: "feature".into(),
            previous_head: plan.current_head.clone(),
            selected_tip: plan.selected_tip.clone(),
            previous_tree: plan.current_tree,
            selected_tree: plan.selected_tree,
            original_index_sha256: plan.index_sha256,
            selected_index_sha256: String::new(),
            plan_sha256: digest,
        };
        append(&journal_path, &record).unwrap();
        let mut checkout = CheckoutBuilder::new();
        checkout.safe().recreate_missing(true);
        let target = repo.find_object(
            Oid::from_str(&record.selected_tip).unwrap(), Some(ObjectType::Commit)).unwrap();
        repo.checkout_tree(&target, Some(&mut checkout)).unwrap();
        record.selected_index_sha256 = exact_index_sha(repo.path()).unwrap();
        record.phase = "checkout_verified".into();
        append(&journal_path, &record).unwrap();
        assert_eq!(repo.head().unwrap().target().unwrap(), original_ref);
        let state = x.registry.inspect_protected_merge_recovery(&x.id).unwrap();
        assert_eq!(state.state, "checkout_completed_ref_not_published");
        assert!(state.operator_reconciliation_required);
        assert_eq!(x.registry.reconcile_protected_merge_receipt(&x.id).unwrap_err(),
                   "G06C03_RECOVERY_MANUAL_REPAIR_REQUIRED");
        let lock = repo.path().join("index.lock");
        fs::write(&lock, b"foreign").unwrap();
        assert_eq!(x.registry.reconcile_protected_merge_receipt(&x.id).unwrap_err(),
                   "G06C03_RECOVERY_FOREIGN_LOCK");
        assert_eq!(fs::read(lock).unwrap(), b"foreign");
    }
    #[test]
    fn signed_off_stale_tree_and_divergent_history_are_not_executed() {
        let x = fixture();
        let digest = reviewed(&x);
        let repo = Repository::open(&x.root).unwrap();
        let original = repo.head().unwrap().target().unwrap();
        let tree = repo.find_commit(original).unwrap().tree().unwrap();
        let sig = Signature::now("Merge fixture", "merge@localhost.invalid").unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "main diverges", &tree,
                    &[&repo.find_commit(original).unwrap()]).unwrap();
        let now = x.registry.preview_reviewed_local_merge(&x.id, "feature").unwrap();
        assert_eq!(now.classification, "divergent_merge_candidate");
        assert!(x.registry.execute_os_approved_fast_forward(
            &x.id, "feature", &now.evidence_sha256, &AtomicU8::new(0)).is_err());
        assert!(x.registry.execute_os_approved_fast_forward(
            &x.id, "feature", &digest, &AtomicU8::new(0)).is_err());
        assert!(x.registry.inspect_protected_merge_recovery(&x.id).unwrap().state == "no_journal");
    }
}
