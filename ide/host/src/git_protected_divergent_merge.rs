//! 6C-03B: one-use, native-OS-approved divergent two-parent local merge.
//!
//! Only a content-verified, conflict-free result from a disposable bare ODB
//! may proceed. No WebView digest is an authorization credential. Every source
//! change is preceded by a synced private journal, and a crash NEVER triggers
//! automatic checkout, retry, rollback, ref write, or Git-lock removal.
use crate::{
    git_branch_operations::{exact_repo, local_commit},
    git_divergent_verify::VerifiedDivergentMerge,
    git_protected_merge::changed_path_is_plain_safe,
    git_replay_quarantine::ReplaySandbox,
    IdeWorkspaceRegistry,
};
use git2::{
    build::CheckoutBuilder, Delta, MergeOptions, ObjectType, Oid, Repository, Signature,
    Status, StatusOptions,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU8, Ordering},
};
use sha2::{Digest, Sha256};

const MAX_JOURNAL: u64 = 1024 * 1024;
const MAX_RECORD: usize = 4096;
const MAX_INDEX: u64 = 8 * 1024 * 1024;
const MAX_OBJECTS: usize = 256;
const MAX_OBJECT_BYTES: usize = 16 * 1024 * 1024;
const MAX_PATHS: usize = 80;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DivergentJournal {
    schema_version: u32,
    project_id: String,
    operation_id: String,
    phase: String,
    branch_ref: String,
    selected_local_branch: String,
    first_parent: String,
    second_parent: String,
    first_tree: String,
    resulting_tree: String,
    candidate_merge_commit: String,
    original_index_sha256: String,
    result_index_sha256: String,
    reviewed_result_sha256: String,
    reviewed_plan_sha256: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovedDivergentMergeReceipt {
    pub schema_version: u32,
    pub project_id: String,
    pub operation_id: String,
    pub first_parent: String,
    pub second_parent: String,
    pub published_merge_commit: String,
    pub resulting_tree: String,
    pub original_index_sha256: String,
    pub final_index_sha256: String,
    pub reviewed_result_sha256: String,
    pub parent_count_verified: bool,
    pub original_branches_preserved: bool,
    pub exact_old_head_cas: bool,
    pub separately_required_one_use_os_consent: bool,
    pub independently_verified: bool,
    pub journal_synced: bool,
    pub hooks_run: bool,
    pub remote_mutated: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DivergentMergeRecovery {
    pub schema_version: u32,
    pub project_id: String,
    pub state: &'static str,
    pub phase: String,
    pub original_head_matches: bool,
    pub candidate_head_matches: bool,
    pub original_index_matches: bool,
    pub candidate_index_matches: bool,
    pub selected_branch_preserved: bool,
    pub worktree_matches_index: bool,
    pub foreign_index_lock_present: bool,
    pub operator_reconciliation_required: bool,
    pub automatic_retry: bool,
    pub automatic_checkout: bool,
    pub automatic_ref_rewrite: bool,
    pub automatic_rollback: bool,
    pub automatic_lock_deletion: bool,
}

fn sha(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }

fn index_sha(path: &Path) -> Result<String, &'static str> {
    let p = path.join("index");
    let m = fs::symlink_metadata(&p).map_err(|_| "G06C03B_INDEX_MISSING")?;
    if !m.is_file() || m.file_type().is_symlink() || m.len() > MAX_INDEX {
        return Err("G06C03B_INDEX_UNTRUSTED");
    }
    let bytes = fs::read(p).map_err(|_| "G06C03B_INDEX_READ_FAILED")?;
    if bytes.len() as u64 != m.len() { return Err("G06C03B_INDEX_CHANGED"); }
    Ok(sha(&bytes))
}

fn path_for(registry: &IdeWorkspaceRegistry, id: &str) -> Result<PathBuf, &'static str> {
    registry.registered_git_root(id).map_err(|_| "G06C03B_PROJECT_NOT_OPEN")?;
    if id.is_empty() || id.len() > 128 ||
        !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') {
        return Err("G06C03B_BAD_PROJECT_ID");
    }
    let root = registry.catalog.parent().ok_or("G06C03B_NO_APP_PRIVATE_ROOT")?
        .join("ide-g06c03b-protected-divergent");
    fs::create_dir_all(&root).map_err(|_| "G06C03B_JOURNAL_DIR_FAILED")?;
    let m = fs::symlink_metadata(&root).map_err(|_| "G06C03B_JOURNAL_DIR_MISSING")?;
    if !m.is_dir() || m.file_type().is_symlink() {
        return Err("G06C03B_JOURNAL_DIR_UNTRUSTED");
    }
    Ok(root.join(format!("{id}.jsonl")))
}
fn last(path: &Path) -> Result<Option<DivergentJournal>, &'static str> {
    let m = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("G06C03B_JOURNAL_STAT_FAILED"),
    };
    if !m.is_file() || m.file_type().is_symlink() || m.len() > MAX_JOURNAL {
        return Err("G06C03B_JOURNAL_UNTRUSTED");
    }
    let bytes = fs::read(path).map_err(|_| "G06C03B_JOURNAL_READ_FAILED")?;
    if bytes.last() != Some(&b'\n') { return Err("G06C03B_JOURNAL_INCOMPLETE"); }
    let line = bytes.split(|b| *b == b'\n').filter(|b| !b.is_empty()).last()
        .ok_or("G06C03B_JOURNAL_EMPTY")?;
    if line.len() > MAX_RECORD { return Err("G06C03B_JOURNAL_RECORD_OVERSIZED"); }
    let j: DivergentJournal = serde_json::from_slice(line)
        .map_err(|_| "G06C03B_JOURNAL_BAD_JSON")?;
    if j.schema_version != 1 || j.operation_id.len() != 32
        || !j.operation_id.bytes().all(|b| b.is_ascii_hexdigit())
        || !matches!(j.phase.as_str(), "prepared" | "objects_intent" | "objects_ready"
            | "checkout_intent" | "checkout_verified" | "publishing"
            | "published" | "verified" | "cancelled") {
        return Err("G06C03B_JOURNAL_BAD_RECORD");
    }
    Ok(Some(j))
}
fn append(path: &Path, j: &DivergentJournal) -> Result<(), &'static str> {
    let size = match fs::symlink_metadata(path) {
        Ok(m) if m.is_file() && !m.file_type().is_symlink() => m.len(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => 0,
        _ => return Err("G06C03B_JOURNAL_UNTRUSTED"),
    };
    let bytes = serde_json::to_vec(j).map_err(|_| "G06C03B_JOURNAL_SERIALIZE_FAILED")?;
    if bytes.len() > MAX_RECORD || size + bytes.len() as u64 + 1 > MAX_JOURNAL {
        return Err("G06C03B_JOURNAL_LIMIT");
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)
        .map_err(|_| "G06C03B_JOURNAL_OPEN_FAILED")?;
    if file.metadata().map_err(|_| "G06C03B_JOURNAL_STAT_FAILED")?.len() != size {
        return Err("G06C03B_JOURNAL_CONCURRENT_WRITE");
    }
    file.write_all(&bytes).and_then(|_| file.write_all(b"\n"))
        .and_then(|_| file.sync_all())
        .map_err(|_| "G06C03B_JOURNAL_FSYNC_FAILED")?;
    #[cfg(unix)]
    File::open(path.parent().ok_or("G06C03B_JOURNAL_PARENT_MISSING")?)
        .and_then(|dir| dir.sync_all()).map_err(|_| "G06C03B_DIR_FSYNC_FAILED")?;
    Ok(())
}
fn ref_oid(repo: &Repository, name: &str) -> Result<String, &'static str> {
    Ok(repo.find_reference(name).map_err(|_| "G06C03B_REF_UNAVAILABLE")?
        .target().ok_or("G06C03B_REF_INDIRECT")?.to_string())
}
fn wt_matches_index(repo: &Repository) -> Result<bool, &'static str> {
    let mut opts = StatusOptions::new();
    opts.include_untracked(true).recurse_untracked_dirs(true);
    let s = repo.statuses(Some(&mut opts)).map_err(|_| "G06C03B_STATUS_FAILED")?;
    Ok(s.iter().all(|x| !x.status().intersects(
        Status::WT_MODIFIED | Status::WT_DELETED | Status::WT_NEW |
        Status::WT_RENAMED | Status::WT_TYPECHANGE | Status::CONFLICTED)))
}
fn verify_index_tree(repo: &Repository, expected: &str) -> Result<bool, &'static str> {
    let mut idx = repo.index().map_err(|_| "G06C03B_INDEX_LOAD_FAILED")?;
    idx.read(true).map_err(|_| "G06C03B_INDEX_REFRESH_FAILED")?;
    if idx.has_conflicts() { return Ok(false); }
    Ok(idx.write_tree_to(repo).map_err(|_| "G06C03B_INDEX_TREE_FAILED")?
        .to_string() == expected)
}
fn merge_parents(repo: &Repository, oid: &str, first: &str, second: &str,
    tree: &str) -> Result<bool, &'static str> {
    let c = repo.find_commit(Oid::from_str(oid).map_err(|_| "G06C03B_COMMIT_OID_INVALID")?)
        .map_err(|_| "G06C03B_COMMIT_MISSING")?;
    Ok(c.parent_count() == 2
        && c.parent_id(0).map_err(|_| "G06C03B_PARENT0_MISSING")?.to_string() == first
        && c.parent_id(1).map_err(|_| "G06C03B_PARENT1_MISSING")?.to_string() == second
        && c.tree_id().to_string() == tree)
}

fn copy_object(source: &git2::Odb<'_>, scratch: &git2::Odb<'_>,
    id: Oid, kind: ObjectType, seen: &mut BTreeSet<Oid>, depth: usize)
    -> Result<(), &'static str> {
    if depth > 32 || seen.len() >= MAX_OBJECTS {
        return Err("G06C03B_OBJECT_LIMIT");
    }
    if !seen.insert(id) || source.exists(id) { return Ok(()); }
    let object = scratch.read(id).map_err(|_| "G06C03B_SCRATCH_OBJECT_MISSING")?;
    if object.kind() != kind || object.len() > MAX_OBJECT_BYTES {
        return Err("G06C03B_OBJECT_KIND_OR_SIZE");
    }
    if kind == ObjectType::Tree {
        let bytes = object.data();
        let mut off = 0;
        while off < bytes.len() {
            let sep = bytes[off..].iter().position(|x| *x == b' ')
                .map(|p| p + off).ok_or("G06C03B_TREE_MODE_INVALID")?;
            let zero = bytes[sep+1..].iter().position(|x| *x == 0)
                .map(|p| p + sep + 1).ok_or("G06C03B_TREE_NAME_INVALID")?;
            if zero + 21 > bytes.len() { return Err("G06C03B_TREE_ENTRY_TRUNCATED"); }
            let mode = std::str::from_utf8(&bytes[off..sep])
                .map_err(|_| "G06C03B_TREE_MODE_ENCODING")?;
            let child_kind = match mode {
                "40000" | "040000" => ObjectType::Tree,
                "100644" | "100755" => ObjectType::Blob,
                _ => return Err("G06C03B_TREE_ENTRY_UNSAFE"),
            };
            let child = Oid::from_bytes(&bytes[zero+1..zero+21])
                .map_err(|_| "G06C03B_CHILD_OID_INVALID")?;
            copy_object(source, scratch, child, child_kind, seen, depth+1)?;
            off = zero + 21;
        }
    }
    let written = source.write(kind, object.data()).map_err(|_| "G06C03B_OBJECT_WRITE_FAILED")?;
    if written != id || !source.exists(id) {
        return Err("G06C03B_OBJECT_READBACK_MISMATCH");
    }
    Ok(())
}
fn changed_paths_safe(root: &Path, repo: &Repository, before: &str,
    after: &str) -> Result<(), &'static str> {
    let old = repo.find_tree(Oid::from_str(before).map_err(|_| "G06C03B_OLD_TREE_BAD_OID")?)
        .map_err(|_| "G06C03B_OLD_TREE_MISSING")?;
    let new = repo.find_tree(Oid::from_str(after).map_err(|_| "G06C03B_NEW_TREE_BAD_OID")?)
        .map_err(|_| "G06C03B_NEW_TREE_MISSING")?;
    let diff = repo.diff_tree_to_tree(Some(&old), Some(&new), None)
        .map_err(|_| "G06C03B_DIFF_FAILED")?;
    if diff.deltas().len() == 0 || diff.deltas().len() > MAX_PATHS {
        return Err("G06C03B_CHANGE_PATHS_UNSUPPORTED");
    }
    for d in diff.deltas() {
        if !matches!(d.status(), Delta::Added | Delta::Deleted | Delta::Modified) {
            return Err("G06C03B_UNSUPPORTED_DELTA");
        }
        for path in [d.old_file().path(), d.new_file().path()].into_iter().flatten() {
            changed_path_is_plain_safe(root,
                path.to_str().ok_or("G06C03B_NON_UTF8_PATH")?)?;
        }
    }
    Ok(())
}

/// Recomputes the independently reviewed result and retains its scratch ODB
/// until all objects are published. Only the native OS gate may call execution.
impl IdeWorkspaceRegistry {
    pub fn revalidate_protected_divergent_merge(&self, id: &str, selected: &str,
        plan_digest: &str, result_digest: &str)
        -> Result<VerifiedDivergentMerge, &'static str> {
        let result = self.verify_divergent_merge_result(id, selected, plan_digest)?;
        if !result.conflict_free_preview || result.conflict_count != 0
            || result.prospective_tree.is_none()
            || result.result_sha256 != result_digest {
            return Err("G06C03B_RESULT_NOT_AUTHORIZABLE");
        }
        Ok(result)
    }

    pub fn execute_os_approved_divergent_merge(&self, id: &str, selected: &str,
        plan_digest: &str, result_digest: &str, cancel: &AtomicU8)
        -> Result<ApprovedDivergentMergeReceipt, &'static str> {
        if cancel.load(Ordering::SeqCst) != 0 { return Err("G06C03B_CANCELLED"); }
        let evidence = self.revalidate_protected_divergent_merge(
            id, selected, plan_digest, result_digest)?;
        let repo = exact_repo(self, id)?;
        let root = self.registered_git_root(id).map_err(|_| "G06C03B_PROJECT_NOT_OPEN")?;
        let scratch = ReplaySandbox::open(&repo)?;
        let quarantine = scratch.repository()?;
        let first = Oid::from_str(&evidence.first_parent).map_err(|_| "G06C03B_FIRST_PARENT_INVALID")?;
        let second = Oid::from_str(&evidence.second_parent).map_err(|_| "G06C03B_SECOND_PARENT_INVALID")?;
        let base = Oid::from_str(&evidence.merge_base).map_err(|_| "G06C03B_BASE_INVALID")?;
        let base_tree = quarantine.find_commit(base).and_then(|c| c.tree())
            .map_err(|_| "G06C03B_BASE_TREE_MISSING")?;
        let ours = quarantine.find_commit(first).map_err(|_| "G06C03B_OUR_COMMIT_MISSING")?;
        let theirs = quarantine.find_commit(second).map_err(|_| "G06C03B_THEIR_COMMIT_MISSING")?;
        let our_tree = ours.tree().map_err(|_| "G06C03B_OUR_TREE_MISSING")?;
        let their_tree = theirs.tree().map_err(|_| "G06C03B_THEIR_TREE_MISSING")?;
        let mut opts = MergeOptions::new();
        let mut index = quarantine.merge_trees(&base_tree, &our_tree, &their_tree,
            Some(&mut opts)).map_err(|_| "G06C03B_RESULT_REBUILD_FAILED")?;
        if index.has_conflicts() { return Err("G06C03B_NEW_CONFLICTS"); }
        let tree = index.write_tree_to(quarantine)
            .map_err(|_| "G06C03B_TREE_REBUILD_FAILED")?;
        if tree.to_string() != evidence.prospective_tree.as_deref().unwrap_or("")
            || ref_oid(&repo, &format!("refs/heads/{}", self.preview_reviewed_local_merge(id, selected)?.current_branch))?
                != evidence.first_parent
            || local_commit(&repo, selected)?.to_string() != evidence.second_parent
            || index_sha(repo.path())? != self.preview_reviewed_local_merge(id, selected)?.index_sha256
            || !wt_matches_index(&repo)? {
            return Err("G06C03B_REBUILT_RESULT_STALE");
        }
        let current_branch = repo.head().map_err(|_| "G06C03B_HEAD_UNAVAILABLE")?;
        let branch_ref = current_branch.name().ok_or("G06C03B_BRANCH_REF_MISSING")?.to_owned();
        if !branch_ref.starts_with("refs/heads/") {
            return Err("G06C03B_NOT_LOCAL_BRANCH");
        }
        let first_tree = ours.tree_id().to_string();
        changed_paths_safe(&root, quarantine, &first_tree, &tree.to_string())?;
        let signature = repo.signature().map_err(|_| "G06C03B_GIT_IDENTITY_REQUIRED")?;
        let message = format!("Merge local branch '{}' into '{}' (Lu IDE protected)", selected,
            branch_ref.trim_start_matches("refs/heads/"));
        let merge_oid = quarantine.commit(None, &signature, &signature, &message,
            &quarantine.find_tree(tree).map_err(|_| "G06C03B_TREE_NOT_FOUND")?,
            &[&ours, &theirs]).map_err(|_| "G06C03B_COMMIT_CONSTRUCTION_FAILED")?;
        if !merge_parents(quarantine, &merge_oid.to_string(),
            &evidence.first_parent, &evidence.second_parent, &tree.to_string())? {
            return Err("G06C03B_INVALID_TWO_PARENT_COMMIT");
        }
        let journal_file = path_for(self, id)?;
        if let Some(j) = last(&journal_file)? {
            if !matches!(j.phase.as_str(), "verified" | "cancelled") {
                return Err("G06C03B_UNFINISHED_OPERATION_REQUIRES_REVIEW");
            }
        }
        if fs::symlink_metadata(repo.path().join("index.lock")).is_ok() {
            return Err("G06C03B_FOREIGN_INDEX_LOCK");
        }
        if cancel.load(Ordering::SeqCst) != 0 { return Err("G06C03B_CANCELLED_BEFORE_PREPARE"); }
        let mut j = DivergentJournal {
            schema_version: 1, project_id: id.to_owned(),
            operation_id: format!("{:032x}", rand::random::<u128>()),
            phase: "prepared".into(),
            branch_ref,
            selected_local_branch: selected.to_owned(),
            first_parent: evidence.first_parent,
            second_parent: evidence.second_parent,
            first_tree,
            resulting_tree: tree.to_string(),
            candidate_merge_commit: merge_oid.to_string(),
            original_index_sha256: index_sha(repo.path())?,
            result_index_sha256: String::new(),
            reviewed_result_sha256: result_digest.to_owned(),
            reviewed_plan_sha256: plan_digest.to_owned(),
        };
        append(&journal_file, &j)?;
        if cancel.compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst).is_err() {
            j.phase = "cancelled".into();
            append(&journal_file, &j)?;
            return Err("G06C03B_CANCELLED_BEFORE_PUBLICATION");
        }
        j.phase = "objects_intent".into();
        append(&journal_file, &j)?;
        if ref_oid(&repo, &j.branch_ref)? != j.first_parent
            || local_commit(&repo, selected)?.to_string() != j.second_parent
            || index_sha(repo.path())? != j.original_index_sha256
            || !verify_index_tree(&repo, &j.first_tree)?
            || !wt_matches_index(&repo)?
            || fs::symlink_metadata(repo.path().join("index.lock")).is_ok() {
            return Err("G06C03B_SOURCE_CHANGED_BEFORE_OBJECTS");
        }
        let src_odb = repo.odb().map_err(|_| "G06C03B_SOURCE_ODB_UNAVAILABLE")?;
        let scratch_odb = quarantine.odb().map_err(|_| "G06C03B_SCRATCH_ODB_UNAVAILABLE")?;
        let mut seen = BTreeSet::new();
        copy_object(&src_odb, &scratch_odb, tree, ObjectType::Tree, &mut seen, 0)?;
        copy_object(&src_odb, &scratch_odb, merge_oid, ObjectType::Commit, &mut seen, 0)?;
        if !merge_parents(&repo, &j.candidate_merge_commit,
            &j.first_parent, &j.second_parent, &j.resulting_tree)? {
            return Err("G06C03B_COMMIT_READBACK_FAILED");
        }
        j.phase = "objects_ready".into();
        append(&journal_file, &j)?;
        changed_paths_safe(&root, &repo, &j.first_tree, &j.resulting_tree)?;
        if ref_oid(&repo, &j.branch_ref)? != j.first_parent
            || local_commit(&repo, selected)?.to_string() != j.second_parent
            || index_sha(repo.path())? != j.original_index_sha256
            || !verify_index_tree(&repo, &j.first_tree)?
            || !wt_matches_index(&repo)? {
            return Err("G06C03B_CHANGED_BEFORE_CHECKOUT");
        }
        j.phase = "checkout_intent".into();
        append(&journal_file, &j)?;
        let candidate = repo.find_object(merge_oid, Some(ObjectType::Commit))
            .map_err(|_| "G06C03B_CANDIDATE_OBJECT_UNAVAILABLE")?;
        let mut checkout = CheckoutBuilder::new();
        checkout.safe().recreate_missing(true).remove_untracked(false)
            .remove_ignored(false).overwrite_ignored(false).update_index(true);
        repo.checkout_tree(&candidate, Some(&mut checkout))
            .map_err(|_| "G06C03B_CHECKOUT_FAILED_MANUAL_REPAIR")?;
        j.result_index_sha256 = index_sha(repo.path())?;
        if !verify_index_tree(&repo, &j.resulting_tree)? || !wt_matches_index(&repo)? {
            return Err("G06C03B_CHECKOUT_RESULT_UNVERIFIED");
        }
        j.phase = "checkout_verified".into();
        append(&journal_file, &j)?;
        #[cfg(test)]
        abort_disposable(&root, repo.path(), "IDE_G06C03B_ABORT_AFTER_CHECKOUT");
        if ref_oid(&repo, &j.branch_ref)? != j.first_parent
            || local_commit(&repo, selected)?.to_string() != j.second_parent
            || index_sha(repo.path())? != j.result_index_sha256 {
            return Err("G06C03B_CHANGED_BEFORE_REF_CAS");
        }
        j.phase = "publishing".into();
        append(&journal_file, &j)?;
        repo.reference_matching(&j.branch_ref, merge_oid, true, first,
            "Lu IDE OS-approved exact original-head divergent merge CAS")
            .map_err(|_| "G06C03B_REF_CAS_FAILED_MANUAL_REPAIR")?;
        #[cfg(test)]
        abort_disposable(&root, repo.path(), "IDE_G06C03B_ABORT_AFTER_CAS");
        j.phase = "published".into();
        append(&journal_file, &j)?;
        if ref_oid(&repo, &j.branch_ref)? != j.candidate_merge_commit
            || local_commit(&repo, selected)?.to_string() != j.second_parent
            || index_sha(repo.path())? != j.result_index_sha256
            || !verify_index_tree(&repo, &j.resulting_tree)?
            || !wt_matches_index(&repo)?
            || !merge_parents(&repo, &j.candidate_merge_commit,
                &j.first_parent, &j.second_parent, &j.resulting_tree)? {
            return Err("G06C03B_POST_CAS_READBACK_FAILED");
        }
        j.phase = "verified".into();
        append(&journal_file, &j)?;
        Ok(ApprovedDivergentMergeReceipt {
            schema_version: 1, project_id: id.to_owned(), operation_id: j.operation_id,
            first_parent: j.first_parent, second_parent: j.second_parent,
            published_merge_commit: j.candidate_merge_commit,
            resulting_tree: j.resulting_tree,
            original_index_sha256: j.original_index_sha256,
            final_index_sha256: j.result_index_sha256,
            reviewed_result_sha256: j.reviewed_result_sha256,
            parent_count_verified: true, original_branches_preserved: true,
            exact_old_head_cas: true, separately_required_one_use_os_consent: true,
            independently_verified: true, journal_synced: true,
            hooks_run: false, remote_mutated: false,
        })
    }

    pub fn inspect_protected_divergent_recovery(&self, id: &str)
        -> Result<DivergentMergeRecovery, &'static str> {
        let repo = exact_repo(self, id)?;
        let j = last(&path_for(self, id)?)?;
        let locked = fs::symlink_metadata(repo.path().join("index.lock")).is_ok();
        let head_ref = repo.head().ok();
        let current_ref = head_ref.as_ref().and_then(|r| r.name()).map(str::to_owned);
        let head = head_ref.as_ref().and_then(|r| r.target()).map(|x| x.to_string());
        let current_index = index_sha(repo.path()).ok();
        let (state, phase, old, new, oldidx, newidx, selected) = match j {
            None => ("no_journal", String::new(), false, false, false, false, false),
            Some(ref j) if j.project_id != id => return Err("G06C03B_WRONG_JOURNAL_PROJECT"),
            Some(j) => {
                let valid_ref = current_ref.as_deref() == Some(&j.branch_ref);
                let feature = local_commit(&repo, &j.selected_local_branch).ok()
                    .is_some_and(|oid| oid.to_string() == j.second_parent);
                let old = valid_ref && head.as_deref() == Some(&j.first_parent);
                let new = valid_ref && head.as_deref() == Some(&j.candidate_merge_commit);
                let old_idx = current_index.as_deref() == Some(&j.original_index_sha256);
                let new_idx = !j.result_index_sha256.is_empty() &&
                    current_index.as_deref() == Some(&j.result_index_sha256);
                let valid_merge = merge_parents(&repo, &j.candidate_merge_commit,
                    &j.first_parent, &j.second_parent, &j.resulting_tree).unwrap_or(false);
                let state = if j.phase == "verified" && new && new_idx && feature && valid_merge {
                    "historical_verified_divergent_merge"
                } else if new && new_idx && feature && valid_merge {
                    "published_requires_independent_receipt"
                } else if old && new_idx {
                    "checkout_without_ref_publication_manual_repair"
                } else if old && old_idx && feature && matches!(j.phase.as_str(),
                    "prepared" | "objects_intent" | "objects_ready") {
                    "unpublished_no_index_change_needs_review"
                } else {
                    "ambiguous_divergent_merge_manual_repair"
                };
                (state, j.phase, old, new, old_idx, new_idx, feature)
            }
        };
        let wt = wt_matches_index(&repo).unwrap_or(false);
        let review = locked || !wt || !matches!(state,
            "no_journal" | "historical_verified_divergent_merge");
        Ok(DivergentMergeRecovery {
            schema_version: 1, project_id: id.to_owned(), state, phase,
            original_head_matches: old, candidate_head_matches: new,
            original_index_matches: oldidx, candidate_index_matches: newidx,
            selected_branch_preserved: selected, worktree_matches_index: wt,
            foreign_index_lock_present: locked, operator_reconciliation_required: review,
            automatic_retry: false, automatic_checkout: false, automatic_ref_rewrite: false,
            automatic_rollback: false, automatic_lock_deletion: false,
        })
    }

    /// Only separately OS-approved receipt-only reconciliation; never reruns
    /// checkout, commit creation, ref CAS or source object publication.
    pub fn reconcile_protected_divergent_receipt(&self, id: &str)
        -> Result<DivergentMergeRecovery, &'static str> {
        let repo = exact_repo(self, id)?;
        if fs::symlink_metadata(repo.path().join("index.lock")).is_ok() {
            return Err("G06C03B_RECOVERY_FOREIGN_LOCK");
        }
        let path = path_for(self, id)?;
        let mut j = last(&path)?.ok_or("G06C03B_NO_JOURNAL")?;
        let state = self.inspect_protected_divergent_recovery(id)?;
        if !state.worktree_matches_index || !state.selected_branch_preserved {
            return Err("G06C03B_RECOVERY_SOURCE_DRIFT");
        }
        match state.state {
            "published_requires_independent_receipt" if matches!(j.phase.as_str(),
                "publishing" | "published") => {
                if !merge_parents(&repo, &j.candidate_merge_commit,
                    &j.first_parent, &j.second_parent, &j.resulting_tree)?
                    || !verify_index_tree(&repo, &j.resulting_tree)?
                    || ref_oid(&repo, &j.branch_ref)? != j.candidate_merge_commit {
                    return Err("G06C03B_RECOVERY_POST_CAS_NOT_PROVEN");
                }
                j.phase = "verified".into();
            }
            "unpublished_no_index_change_needs_review" if matches!(j.phase.as_str(),
                "prepared" | "objects_intent" | "objects_ready") => {
                if ref_oid(&repo, &j.branch_ref)? != j.first_parent
                    || !verify_index_tree(&repo, &j.first_tree)? {
                    return Err("G06C03B_RECOVERY_ORIGINAL_NOT_PROVEN");
                }
                j.phase = "cancelled".into();
            }
            _ => return Err("G06C03B_MANUAL_REPAIR_REQUIRED"),
        }
        append(&path, &j)?;
        self.inspect_protected_divergent_recovery(id)
    }
}

#[cfg(test)]
fn abort_disposable(root: &Path, gitdir: &Path, variable: &str) {
    let Some(value) = std::env::var_os(variable) else { return; };
    let expected = fs::canonicalize(PathBuf::from(value)).ok();
    let actual = fs::canonicalize(root).ok();
    let temp = fs::canonicalize(std::env::temp_dir()).ok();
    if expected == actual && actual.as_ref().zip(temp.as_ref())
        .is_some_and(|(a, b)| a.starts_with(b))
        && root.file_name().is_some_and(|x| x == "repo")
        && root.parent().and_then(|x| x.file_name())
            .is_some_and(|x| x.to_string_lossy().starts_with("ide-g06c03b-"))
        && fs::read(gitdir.join("g06c03b-disposable-marker")).ok().as_deref()
            == Some(b"G06C03B_TEST_ONLY\n") {
        std::process::abort();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{IdeError, NativeDirectoryPicker};
    use git2::Repository;

    struct Picker(PathBuf);
    impl NativeDirectoryPicker for Picker {
        fn choose_project_directory(&mut self) -> Result<Option<PathBuf>, IdeError> {
            Ok(Some(self.0.clone()))
        }
    }
    struct Fixture {
        base: PathBuf, root: PathBuf, id: String, registry: IdeWorkspaceRegistry,
    }
    impl Drop for Fixture {
        fn drop(&mut self) { let _ = fs::remove_dir_all(&self.base); }
    }
    fn fixture(conflict: bool) -> Fixture {
        let base = std::env::temp_dir().join(
            format!("ide-g06c03b-{:032x}", rand::random::<u128>()));
        let root = base.join("repo");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("one.txt"), b"base one\n").unwrap();
        fs::write(root.join("two.txt"), b"base two\n").unwrap();
        fs::write(root.join("unrelated.keep"), b"NEVER ALTER\n").unwrap();
        let repo = Repository::init(&root).unwrap();
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Local fixture operator").unwrap();
        config.set_str("user.email", "local@localhost.invalid").unwrap();
        drop(config);
        let sig = repo.signature().unwrap();
        let mut index = repo.index().unwrap();
        for name in ["one.txt", "two.txt", "unrelated.keep"] {
            index.add_path(Path::new(name)).unwrap();
        }
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let first = repo.commit(Some("HEAD"), &sig, &sig, "base", &tree, &[]).unwrap();
        let original = repo.find_commit(first).unwrap();
        repo.branch("feature", &original, false).unwrap();
        let initial_tree = original.tree().unwrap();
        let mut builder = repo.treebuilder(Some(&initial_tree)).unwrap();
        builder.insert("one.txt", repo.blob(b"feature one\n").unwrap(), 0o100644).unwrap();
        let feature_tree = repo.find_tree(builder.write().unwrap()).unwrap();
        repo.commit(Some("refs/heads/feature"), &sig, &sig, "feature",
            &feature_tree, &[&original]).unwrap();
        drop(feature_tree);
        drop(builder);
        let mut builder = repo.treebuilder(Some(&initial_tree)).unwrap();
        let main_file = if conflict { "one.txt" } else { "two.txt" };
        let main_bytes: &[u8] = if conflict { b"main one\n" } else { b"main two\n" };
        builder.insert(main_file, repo.blob(main_bytes).unwrap(), 0o100644).unwrap();
        let main_tree = repo.find_tree(builder.write().unwrap()).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "main", &main_tree, &[&original]).unwrap();
        drop(main_tree);
        drop(builder);
        drop(initial_tree);
        drop(original);
        drop(tree);
        drop(index);
        drop(repo);
        fs::write(root.join(if conflict { "one.txt" } else { "two.txt" }),
            if conflict { b"main one\n" } else { b"main two\n" }).unwrap();
        let repo = Repository::open(&root).unwrap();
        let mut idx = repo.index().unwrap();
        idx.read_tree(&repo.head().unwrap().peel_to_tree().unwrap()).unwrap();
        idx.write().unwrap();
        drop(idx);
        drop(repo);
        let registry = IdeWorkspaceRegistry::open(base.join("catalog.json")).unwrap();
        let id = registry.register_from_native_picker(&mut Picker(root.clone()))
            .unwrap().unwrap().project_id;
        Fixture { base, root, id, registry }
    }
    fn evidence(x: &Fixture) -> (String, String) {
        let p = x.registry.preview_reviewed_local_merge(&x.id, "feature").unwrap();
        let result = x.registry.verify_divergent_merge_result(
            &x.id, "feature", &p.evidence_sha256).unwrap();
        (p.evidence_sha256, result.result_sha256)
    }
    #[test]
    fn two_parents_and_worktree_are_independently_verified() {
        let x = fixture(false);
        let repo = Repository::open(&x.root).unwrap();
        let old = repo.head().unwrap().target().unwrap().to_string();
        let selected = local_commit(&repo, "feature").unwrap().to_string();
        let unrelated = fs::read(x.root.join("unrelated.keep")).unwrap();
        drop(repo);
        let (p, r) = evidence(&x);
        let receipt = x.registry.execute_os_approved_divergent_merge(
            &x.id, "feature", &p, &r, &AtomicU8::new(0)).unwrap();
        assert!(receipt.parent_count_verified && receipt.exact_old_head_cas);
        assert_eq!(receipt.first_parent, old);
        assert_eq!(receipt.second_parent, selected);
        assert_eq!(fs::read(x.root.join("one.txt")).unwrap(), b"feature one\n");
        assert_eq!(fs::read(x.root.join("two.txt")).unwrap(), b"main two\n");
        assert_eq!(fs::read(x.root.join("unrelated.keep")).unwrap(), unrelated);
        let repo = Repository::open(&x.root).unwrap();
        assert_eq!(repo.head().unwrap().target().unwrap().to_string(),
            receipt.published_merge_commit);
        assert!(merge_parents(&repo, &receipt.published_merge_commit,
            &old, &selected, &receipt.resulting_tree).unwrap());
        assert!(repo.statuses(None).unwrap().is_empty());
        assert_eq!(x.registry.inspect_protected_divergent_recovery(&x.id)
            .unwrap().state, "historical_verified_divergent_merge");
    }
    #[test]
    fn conflicting_native_result_and_stale_digest_cannot_write() {
        let x = fixture(true);
        let repo = Repository::open(&x.root).unwrap();
        let old = repo.head().unwrap().target().unwrap().to_string();
        let original_index = fs::read(x.root.join(".git/index")).unwrap();
        drop(repo);
        let (p, r) = evidence(&x);
        assert!(x.registry.execute_os_approved_divergent_merge(
            &x.id, "feature", &p, &r, &AtomicU8::new(0)).is_err());
        assert_eq!(old, Repository::open(&x.root).unwrap()
            .head().unwrap().target().unwrap().to_string());
        assert_eq!(original_index, fs::read(x.root.join(".git/index")).unwrap());
        assert_eq!(x.registry.inspect_protected_divergent_recovery(&x.id)
            .unwrap().state, "no_journal");
    }
    #[test]
    fn preapproved_cancellation_and_foreign_index_lock_deny_mutation() {
        let x = fixture(false);
        let (p, r) = evidence(&x);
        let denied = x.registry.execute_os_approved_divergent_merge(
            &x.id, "feature", &p, &r, &AtomicU8::new(2)).unwrap_err();
        assert_eq!(denied, "G06C03B_CANCELLED");
        let lock = x.root.join(".git/index.lock");
        fs::write(&lock, b"ANOTHER_PROCESS").unwrap();
        assert!(x.registry.execute_os_approved_divergent_merge(
            &x.id, "feature", &p, &r, &AtomicU8::new(0)).is_err());
        assert_eq!(fs::read(lock).unwrap(), b"ANOTHER_PROCESS");
        assert_eq!(x.registry.inspect_protected_divergent_recovery(&x.id)
            .unwrap().state, "no_journal");
    }
    #[test]
    fn child_abort_entry() {
        let Some(root) = std::env::var_os("IDE_G06C03B_CHILD_ROOT") else { return; };
        let root = PathBuf::from(root);
        let id = std::env::var("IDE_G06C03B_CHILD_ID").unwrap();
        let registry = IdeWorkspaceRegistry::open(root.parent().unwrap().join("catalog.json")).unwrap();
        registry.reopen(&id).unwrap();
        let plan = registry.preview_reviewed_local_merge(&id, "feature").unwrap();
        let candidate = registry.verify_divergent_merge_result(
            &id, "feature", &plan.evidence_sha256).unwrap();
        let _ = registry.execute_os_approved_divergent_merge(
            &id, "feature", &plan.evidence_sha256, &candidate.result_sha256, &AtomicU8::new(0));
        panic!("G06C03B child returned without genuine abort");
    }
    fn spawn_abort(x: &Fixture, phase: &str) {
        fs::write(x.root.join(".git/g06c03b-disposable-marker"),
            b"G06C03B_TEST_ONLY\n").unwrap();
        let result = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg("git_protected_divergent_merge::tests::child_abort_entry")
            .arg("--nocapture")
            .env("IDE_G06C03B_CHILD_ROOT", &x.root)
            .env("IDE_G06C03B_CHILD_ID", &x.id)
            .env(phase, &x.root)
            .output().unwrap();
        assert!(!result.status.success(), "child must actually abort");
        assert!(!String::from_utf8_lossy(&result.stderr)
            .contains("returned without genuine abort"),
            "child must reach native abort point");
    }
    #[test]
    fn real_abort_after_checkout_preserves_old_ref_and_requires_manual_repair() {
        if std::env::var_os("IDE_G06C03B_CHILD_ROOT").is_some() { return; }
        let x = fixture(false);
        let old = Repository::open(&x.root).unwrap().head().unwrap()
            .target().unwrap().to_string();
        spawn_abort(&x, "IDE_G06C03B_ABORT_AFTER_CHECKOUT");
        let repo = Repository::open(&x.root).unwrap();
        assert_eq!(repo.head().unwrap().target().unwrap().to_string(), old);
        assert_eq!(fs::read(x.root.join("one.txt")).unwrap(), b"feature one\n");
        assert_eq!(fs::read(x.root.join("unrelated.keep")).unwrap(), b"NEVER ALTER\n");
        let j = last(&path_for(&x.registry, &x.id).unwrap()).unwrap().unwrap();
        assert_eq!(j.phase, "checkout_verified");
        let restarted = IdeWorkspaceRegistry::open(x.base.join("catalog.json")).unwrap();
        restarted.reopen(&x.id).unwrap();
        let recovery = restarted.inspect_protected_divergent_recovery(&x.id).unwrap();
        assert_eq!(recovery.state, "checkout_without_ref_publication_manual_repair");
        assert!(recovery.operator_reconciliation_required);
        assert!(restarted.reconcile_protected_divergent_receipt(&x.id).is_err());
        assert!(!recovery.automatic_retry && !recovery.automatic_ref_rewrite
            && !recovery.automatic_checkout && !recovery.automatic_rollback);
    }
    #[test]
    fn real_abort_after_cas_allows_receipt_only_with_original_parents() {
        if std::env::var_os("IDE_G06C03B_CHILD_ROOT").is_some() { return; }
        let x = fixture(false);
        let old = Repository::open(&x.root).unwrap().head().unwrap()
            .target().unwrap().to_string();
        spawn_abort(&x, "IDE_G06C03B_ABORT_AFTER_CAS");
        let repo = Repository::open(&x.root).unwrap();
        let head = repo.head().unwrap().target().unwrap().to_string();
        let selected = local_commit(&repo, "feature").unwrap().to_string();
        assert_ne!(head, old);
        let j = last(&path_for(&x.registry, &x.id).unwrap()).unwrap().unwrap();
        assert_eq!(j.phase, "publishing");
        assert!(merge_parents(&repo, &head, &old, &selected, &j.resulting_tree).unwrap());
        assert_eq!(fs::read(x.root.join("unrelated.keep")).unwrap(), b"NEVER ALTER\n");
        drop(repo);
        let restarted = IdeWorkspaceRegistry::open(x.base.join("catalog.json")).unwrap();
        restarted.reopen(&x.id).unwrap();
        let state = restarted.inspect_protected_divergent_recovery(&x.id).unwrap();
        assert_eq!(state.state, "published_requires_independent_receipt");
        assert!(state.operator_reconciliation_required);
        // Unit-test simulation of separately approved receipt, not physical OS consent.
        let completed = restarted.reconcile_protected_divergent_receipt(&x.id).unwrap();
        assert_eq!(completed.state, "historical_verified_divergent_merge");
        assert!(!completed.operator_reconciliation_required);
        assert_eq!(Repository::open(&x.root).unwrap().head().unwrap()
            .target().unwrap().to_string(), head);
    }
}
