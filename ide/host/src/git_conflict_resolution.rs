//! IDE 6D-03/04: evidence-bound text conflict review and native consent only.
//!
//! The review is never a capability. A separate Tauri native OS dialog must
//! precede each write. This module never creates a commit or updates a ref,
//! runs a hook, invokes Git CLI, or initiates network access.
use crate::{git_branch_operations::exact_repo, git_protected_merge::changed_path_is_plain_safe,
    IdeWorkspaceRegistry};
use git2::{IndexEntry, Oid, Repository};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fs::{self, File, OpenOptions}, io::Write, path::{Path, PathBuf},
    sync::atomic::{AtomicU8, Ordering}};

const MAX_CANDIDATE: usize = 128 * 1024;
const MAX_WORKTREE_BYTES: u64 = 1024 * 1024;
const MAX_INDEX_BYTES: u64 = 8 * 1024 * 1024;
const MAX_JOURNAL_BYTES: u64 = 128 * 1024;
const MAX_RECORD_BYTES: usize = 3072;

fn sha(data: &[u8]) -> String { format!("{:x}", Sha256::digest(data)) }
fn valid_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}
fn bind(hash: &mut Sha256, s: &str) {
    hash.update((s.len() as u64).to_le_bytes());
    hash.update(s.as_bytes());
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictResolutionReview {
    pub schema_version: u32,
    pub project_id: String,
    pub relative_path: String,
    pub current_head: String,
    pub current_branch: String,
    pub original_index_sha256: String,
    pub inventory_evidence_sha256: String,
    pub original_worktree_sha256: String,
    pub proposed_content_sha256: String,
    pub proposed_bytes: usize,
    pub file_mode: u32,
    pub ancestor_oid: Option<String>,
    pub ours_oid: Option<String>,
    pub theirs_oid: Option<String>,
    pub review_sha256: String,
    pub source_rechecked: bool,
    pub requires_separate_native_os_approval: bool,
    pub automatically_applied: bool,
    pub authority_granted: bool,
    pub index_or_worktree_modified: bool,
    pub branch_ref_modified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Journal {
    schema_version: u32,
    project_id: String,
    operation_id: String,
    phase: String,
    relative_path: String,
    original_head: String,
    original_index_sha256: String,
    original_worktree_sha256: String,
    proposed_content_sha256: String,
    review_sha256: String,
    candidate_blob_oid: String,
    resulting_index_sha256: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtectedResolutionReceipt {
    pub schema_version: u32,
    pub project_id: String,
    pub operation_id: String,
    pub relative_path: String,
    pub original_head: String,
    pub final_head: String,
    pub original_index_sha256: String,
    pub resulting_index_sha256: String,
    pub candidate_blob_oid: String,
    pub proposed_content_sha256: String,
    pub review_sha256: String,
    pub native_os_consent_required: bool,
    pub candidate_readback_verified: bool,
    pub target_conflict_cleared: bool,
    pub journal_synced: bool,
    pub branch_ref_modified: bool,
    pub commit_created: bool,
    pub remote_contacted: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtectedResolutionRecovery {
    pub schema_version: u32,
    pub project_id: String,
    pub state: &'static str,
    pub phase: String,
    pub operation_id: String,
    pub relative_path: String,
    pub original_head_matches: bool,
    pub original_index_matches: bool,
    pub candidate_index_entry_matches: bool,
    pub original_worktree_matches: bool,
    pub proposed_worktree_matches: bool,
    pub foreign_index_lock_present: bool,
    pub automatic_retry: bool,
    pub automatic_restore: bool,
    pub automatic_index_write: bool,
    pub automatic_ref_update: bool,
    pub operator_review_required: bool,
}

fn no_foreign_lock(repo: &Repository) -> Result<(), &'static str> {
    match fs::symlink_metadata(repo.path().join("index.lock")) {
        Ok(_) => Err("G06D04_FOREIGN_GIT_INDEX_LOCK"),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err("G06D04_GIT_LOCK_STATE_UNVERIFIABLE"),
    }
}
fn index_hash(repo: &Repository) -> Result<String, &'static str> {
    let path = repo.path().join("index");
    let m = fs::symlink_metadata(&path).map_err(|_| "G06D04_INDEX_MISSING")?;
    if !m.is_file() || m.file_type().is_symlink() || m.len() > MAX_INDEX_BYTES {
        return Err("G06D04_INDEX_UNTRUSTED");
    }
    let bytes = fs::read(path).map_err(|_| "G06D04_INDEX_READ_FAILED")?;
    if bytes.len() as u64 != m.len() { return Err("G06D04_INDEX_RACED"); }
    Ok(sha(&bytes))
}
fn worktree_bytes(root: &Path, relative: &str) -> Result<Vec<u8>, &'static str> {
    changed_path_is_plain_safe(root, relative)?;
    let p = root.join(relative);
    let meta = fs::symlink_metadata(&p).map_err(|_| "G06D04_TARGET_MISSING")?;
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > MAX_WORKTREE_BYTES {
        return Err("G06D04_TARGET_NOT_REGULAR_SMALL_FILE");
    }
    let bytes = fs::read(&p).map_err(|_| "G06D04_TARGET_READ_FAILED")?;
    if bytes.len() as u64 != meta.len() { return Err("G06D04_TARGET_RACED"); }
    Ok(bytes)
}
fn journal_path(registry: &IdeWorkspaceRegistry, id: &str)
    -> Result<PathBuf, &'static str> {
    registry.registered_git_root(id).map_err(|_| "G06D04_PROJECT_NOT_OPEN")?;
    if id.is_empty() || id.len() > 128 ||
        !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') {
        return Err("G06D04_BAD_PROJECT_ID");
    }
    let dir = registry.catalog.parent().ok_or("G06D04_NO_PRIVATE_ROOT")?
        .join("ide-g06d04-protected-conflict-resolution");
    fs::create_dir_all(&dir).map_err(|_| "G06D04_PRIVATE_DIR_CREATE_FAILED")?;
    let meta = fs::symlink_metadata(&dir).map_err(|_| "G06D04_PRIVATE_DIR_STAT_FAILED")?;
    if !meta.is_dir() || meta.file_type().is_symlink() {
        return Err("G06D04_PRIVATE_DIR_UNTRUSTED");
    }
    Ok(dir.join(format!("{id}.jsonl")))
}
fn journal_last(path: &Path) -> Result<Option<Journal>, &'static str> {
    let meta = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("G06D04_JOURNAL_STAT_FAILED"),
    };
    if !meta.is_file() || meta.file_type().is_symlink() ||
        meta.len() > MAX_JOURNAL_BYTES {
        return Err("G06D04_JOURNAL_UNTRUSTED");
    }
    let bytes = fs::read(path).map_err(|_| "G06D04_JOURNAL_READ_FAILED")?;
    if bytes.last() != Some(&b'\n') { return Err("G06D04_JOURNAL_INCOMPLETE"); }
    let row = bytes.split(|b| *b == b'\n').filter(|x| !x.is_empty()).last()
        .ok_or("G06D04_JOURNAL_EMPTY")?;
    if row.len() > MAX_RECORD_BYTES { return Err("G06D04_JOURNAL_ROW_TOO_LARGE"); }
    let j: Journal = serde_json::from_slice(row).map_err(|_| "G06D04_JOURNAL_BAD_JSON")?;
    if j.schema_version != 1 || j.operation_id.len() != 32 ||
        !j.operation_id.bytes().all(|c| c.is_ascii_hexdigit()) ||
        !matches!(j.phase.as_str(), "prepared" | "blob_ready" | "worktree_intent" |
            "worktree_verified" | "index_intent" | "index_written" |
            "verified" | "cancelled") ||
        !valid_hex(&j.original_index_sha256) ||
        !valid_hex(&j.original_worktree_sha256) ||
        !valid_hex(&j.proposed_content_sha256) ||
        !valid_hex(&j.review_sha256) {
        return Err("G06D04_JOURNAL_INVALID_RECORD");
    }
    Ok(Some(j))
}
fn journal_append(path: &Path, j: &Journal) -> Result<(), &'static str> {
    let before = match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_file() && !meta.file_type().is_symlink() => meta.len(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => 0,
        _ => return Err("G06D04_JOURNAL_UNTRUSTED"),
    };
    let encoded = serde_json::to_vec(j).map_err(|_| "G06D04_JOURNAL_SERIALIZE_FAILED")?;
    if encoded.len() > MAX_RECORD_BYTES ||
        before + encoded.len() as u64 + 1 > MAX_JOURNAL_BYTES {
        return Err("G06D04_JOURNAL_LIMIT");
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)
        .map_err(|_| "G06D04_JOURNAL_OPEN_FAILED")?;
    if file.metadata().map_err(|_| "G06D04_JOURNAL_STAT_FAILED")?.len() != before {
        return Err("G06D04_JOURNAL_CONCURRENT_MODIFICATION");
    }
    file.write_all(&encoded).and_then(|_| file.write_all(b"\n"))
        .and_then(|_| file.sync_all())
        .map_err(|_| "G06D04_JOURNAL_SYNC_FAILED")?;
    #[cfg(unix)]
    File::open(path.parent().ok_or("G06D04_PARENT_MISSING")?)
        .and_then(|d| d.sync_all()).map_err(|_| "G06D04_JOURNAL_DIR_SYNC_FAILED")?;
    Ok(())
}
fn head_matches(repo: &Repository, expected: &str) -> bool {
    repo.head().ok().and_then(|h| h.target()).is_some_and(|oid| oid.to_string() == expected)
}
fn stage0_matches(repo: &Repository, relative: &str, candidate: &str) -> bool {
    let Ok(oid) = Oid::from_str(candidate) else { return false; };
    let Ok(index) = repo.index() else { return false; };
    let path = Path::new(relative);
    index.get_path(path, 0).is_some_and(|e| e.id == oid) &&
        (1..=3).all(|s| index.get_path(path, s).is_none())
}
fn candidate_has_no_nul(candidate: &str) -> Result<(), &'static str> {
    if candidate.len() > MAX_CANDIDATE || candidate.as_bytes().contains(&0) {
        return Err("G06D03_CANDIDATE_NOT_PLAIN_BOUNDED_TEXT");
    }
    Ok(())
}

impl IdeWorkspaceRegistry {
    /// No object writes. A digest is a source-bound claim, never an approval.
    pub fn review_native_conflict_resolution(
        &self, id: &str, relative: &str, inventory_sha: &str,
        index_sha_claim: &str, candidate: &str)
        -> Result<ConflictResolutionReview, &'static str> {
        candidate_has_no_nul(candidate)?;
        if !valid_hex(inventory_sha) || !valid_hex(index_sha_claim) {
            return Err("G06D03_INVALID_PROVENANCE");
        }
        let repo = exact_repo(self, id)?;
        no_foreign_lock(&repo)?;
        let inventory = self.inspect_native_git_conflicts(id)?;
        if inventory.evidence_sha256 != inventory_sha ||
            inventory.index_sha256 != index_sha_claim {
            return Err("G06D03_SOURCE_EVIDENCE_STALE");
        }
        let entry = inventory.conflicts.iter()
            .find(|e| e.relative_path == relative)
            .ok_or("G06D03_NOT_CURRENTLY_CONFLICTED")?;
        // Deny all unsupported modes, renamed and unsafe paths; 6D01 already
        // refuses these before returning an inventory entry.
        let modes: Vec<u32> = [entry.ancestor.as_ref(), entry.ours.as_ref(),
            entry.theirs.as_ref()].iter().flatten().map(|v| v.mode).collect();
        let mode = *modes.first().ok_or("G06D03_NO_BASE_MODE")?;
        if modes.iter().any(|x| *x != mode) {
            return Err("G06D03_MODE_CHANGE_REQUIRES_SPECIALIST");
        }
        let root = self.registered_git_root(id).map_err(|_| "G06D03_PROJECT_CLOSED")?;
        let original_worktree = worktree_bytes(&root, relative)?;
        let worktree_digest = sha(&original_worktree);
        let proposed_sha = sha(candidate.as_bytes());
        let mut h = Sha256::new();
        h.update(b"lu-ide-g06d03-conflict-proposal-v1\n");
        for part in [id, relative, &inventory.current_branch, &inventory.current_head,
            &inventory.index_sha256, &inventory.evidence_sha256,
            &worktree_digest, &proposed_sha] { bind(&mut h, part); }
        h.update(mode.to_le_bytes());
        for oid in [&entry.ancestor, &entry.ours, &entry.theirs] {
            bind(&mut h, oid.as_ref().map(|v| v.oid.as_str()).unwrap_or("NONE"));
        }
        let review = format!("{:x}", h.finalize());
        if self.inspect_native_git_conflicts(id)?.evidence_sha256 != inventory_sha ||
            worktree_bytes(&root, relative)? != original_worktree ||
            !head_matches(&repo, &inventory.current_head) ||
            index_hash(&repo)? != inventory.index_sha256 {
            return Err("G06D03_SOURCE_CHANGED_DURING_REVIEW");
        }
        Ok(ConflictResolutionReview {
            schema_version: 1, project_id: id.to_owned(), relative_path: relative.to_owned(),
            current_head: inventory.current_head, current_branch: inventory.current_branch,
            original_index_sha256: inventory.index_sha256,
            inventory_evidence_sha256: inventory.evidence_sha256,
            original_worktree_sha256: worktree_digest,
            proposed_content_sha256: proposed_sha,
            proposed_bytes: candidate.len(), file_mode: mode,
            ancestor_oid: entry.ancestor.as_ref().map(|v| v.oid.clone()),
            ours_oid: entry.ours.as_ref().map(|v| v.oid.clone()),
            theirs_oid: entry.theirs.as_ref().map(|v| v.oid.clone()),
            review_sha256: review,
            source_rechecked: true, requires_separate_native_os_approval: true,
            automatically_applied: false, authority_granted: false,
            index_or_worktree_modified: false, branch_ref_modified: false,
        })
    }

    /// The caller MUST have obtained distinct one-use native OS approval.
    /// After the first mutation point cancellation is intentionally disabled:
    /// a partially applied file/index pair must be reconciled, never retried.
    pub fn apply_os_approved_conflict_resolution(
        &self, id: &str, relative: &str, inventory_sha: &str,
        index_sha_claim: &str, candidate: &str, review_sha: &str,
        cancel: &AtomicU8)
        -> Result<ProtectedResolutionReceipt, &'static str> {
        if cancel.load(Ordering::SeqCst) != 0 {
            return Err("G06D04_CANCELLED_BEFORE_REVIEW");
        }
        let reviewed = self.review_native_conflict_resolution(
            id, relative, inventory_sha, index_sha_claim, candidate)?;
        if reviewed.review_sha256 != review_sha {
            return Err("G06D04_REVIEW_DIGEST_STALE");
        }
        let repo = exact_repo(self, id)?;
        let root = self.registered_git_root(id).map_err(|_| "G06D04_PROJECT_CLOSED")?;
        let journal = journal_path(self, id)?;
        if let Some(prev) = journal_last(&journal)? {
            if !matches!(prev.phase.as_str(), "verified" | "cancelled") {
                return Err("G06D04_PREVIOUS_OPERATION_REQUIRES_REVIEW");
            }
        }
        no_foreign_lock(&repo)?;
        let before = worktree_bytes(&root, relative)?;
        if sha(&before) != reviewed.original_worktree_sha256 ||
            index_hash(&repo)? != reviewed.original_index_sha256 ||
            !head_matches(&repo, &reviewed.current_head) {
            return Err("G06D04_STALE_BEFORE_JOURNAL");
        }
        let mut j = Journal {
            schema_version: 1, project_id: id.to_owned(),
            operation_id: format!("{:032x}", rand::random::<u128>()),
            phase: "prepared".to_owned(), relative_path: relative.to_owned(),
            original_head: reviewed.current_head.clone(),
            original_index_sha256: reviewed.original_index_sha256.clone(),
            original_worktree_sha256: reviewed.original_worktree_sha256.clone(),
            proposed_content_sha256: reviewed.proposed_content_sha256.clone(),
            review_sha256: reviewed.review_sha256.clone(),
            candidate_blob_oid: String::new(),
            resulting_index_sha256: String::new(),
        };
        journal_append(&journal, &j)?;
        if cancel.compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst).is_err() {
            j.phase = "cancelled".to_owned();
            journal_append(&journal, &j)?;
            return Err("G06D04_CANCELLED_BEFORE_MUTATION");
        }
        // Use Git's own object writer; store and verify its content identity.
        let oid = repo.blob(candidate.as_bytes())
            .map_err(|_| "G06D04_BLOB_WRITE_FAILED")?;
        let checked = repo.find_blob(oid).map_err(|_| "G06D04_BLOB_READBACK_FAILED")?;
        if checked.content() != candidate.as_bytes() {
            return Err("G06D04_BLOB_CONTENT_MISMATCH");
        }
        j.candidate_blob_oid = oid.to_string();
        j.phase = "blob_ready".to_owned();
        journal_append(&journal, &j)?;
        if index_hash(&repo)? != j.original_index_sha256 ||
            sha(&worktree_bytes(&root, relative)?) != j.original_worktree_sha256 ||
            !head_matches(&repo, &j.original_head) {
            return Err("G06D04_SOURCE_CHANGED_BEFORE_WORKTREE");
        }
        no_foreign_lock(&repo)?;
        j.phase = "worktree_intent".to_owned();
        journal_append(&journal, &j)?;
        let target = root.join(relative);
        let parent = target.parent().ok_or("G06D04_TARGET_PARENT_MISSING")?;
        let filename = target.file_name().and_then(|f| f.to_str())
            .ok_or("G06D04_TARGET_NAME_INVALID")?;
        let scratch = parent.join(format!(".{filename}.lu-6d04-{}.tmp", j.operation_id));
        let permissions = fs::metadata(&target)
            .map_err(|_| "G06D04_ORIGINAL_PERMISSIONS_UNAVAILABLE")?.permissions();
        let mut file = OpenOptions::new().write(true).create_new(true).open(&scratch)
            .map_err(|_| "G06D04_TEMPFILE_UNAVAILABLE_MANUAL_REVIEW")?;
        file.write_all(candidate.as_bytes()).and_then(|_| file.sync_all())
            .map_err(|_| "G06D04_TEMPFILE_SYNC_FAILED_MANUAL_REVIEW")?;
        fs::set_permissions(&scratch, permissions)
            .map_err(|_| "G06D04_TEMPFILE_MODE_FAILED_MANUAL_REVIEW")?;
        if sha(&worktree_bytes(&root, relative)?) != j.original_worktree_sha256 ||
            index_hash(&repo)? != j.original_index_sha256 ||
            !head_matches(&repo, &j.original_head) {
            return Err("G06D04_DRIFT_BEFORE_WORKTREE_REPLACE_MANUAL_REVIEW");
        }
        no_foreign_lock(&repo)?;
        fs::rename(&scratch, &target)
            .map_err(|_| "G06D04_WORKTREE_REPLACE_FAILED_MANUAL_REVIEW")?;
        #[cfg(unix)]
        File::open(parent).and_then(|d| d.sync_all())
            .map_err(|_| "G06D04_WORKTREE_PARENT_FSYNC_FAILED_MANUAL_REVIEW")?;
        if sha(&worktree_bytes(&root, relative)?) != j.proposed_content_sha256 {
            return Err("G06D04_WORKTREE_READBACK_FAILED_MANUAL_REVIEW");
        }
        j.phase = "worktree_verified".to_owned();
        journal_append(&journal, &j)?;
        #[cfg(test)]
        test_abort(&root, repo.path(), "IDE_G06D04_ABORT_AFTER_WORKTREE");
        if index_hash(&repo)? != j.original_index_sha256 ||
            !head_matches(&repo, &j.original_head) {
            return Err("G06D04_SOURCE_CHANGED_BEFORE_INDEX_MANUAL_REVIEW");
        }
        no_foreign_lock(&repo)?;
        let mut idx = repo.index().map_err(|_| "G06D04_INDEX_OPEN_FAILED")?;
        idx.read(true).map_err(|_| "G06D04_INDEX_REFRESH_FAILED")?;
        let mut basis: IndexEntry = (1..=3).find_map(|s| idx.get_path(Path::new(relative), s))
            .ok_or("G06D04_CONFLICT_ENTRY_DISAPPEARED_MANUAL_REVIEW")?;
        j.phase = "index_intent".to_owned();
        journal_append(&journal, &j)?;
        basis.flags &= !0x3000; // The resolution must be stage zero, not another conflict entry.
        idx.add_frombuffer(&basis, candidate.as_bytes())
            .map_err(|_| "G06D04_INDEX_STAGE_FAILED_MANUAL_REVIEW")?;
        idx.write().map_err(|_| "G06D04_INDEX_WRITE_FAILED_MANUAL_REVIEW")?;
        j.resulting_index_sha256 = index_hash(&repo)?;
        j.phase = "index_written".to_owned();
        journal_append(&journal, &j)?;
        #[cfg(test)]
        test_abort(&root, repo.path(), "IDE_G06D04_ABORT_AFTER_INDEX");
        if !head_matches(&repo, &j.original_head) ||
            !stage0_matches(&repo, relative, &j.candidate_blob_oid) ||
            sha(&worktree_bytes(&root, relative)?) != j.proposed_content_sha256 ||
            index_hash(&repo)? != j.resulting_index_sha256 {
            return Err("G06D04_POST_WRITE_READBACK_FAILED");
        }
        j.phase = "verified".to_owned();
        journal_append(&journal, &j)?;
        Ok(ProtectedResolutionReceipt {
            schema_version: 1, project_id: id.to_owned(), operation_id: j.operation_id,
            relative_path: j.relative_path, original_head: j.original_head.clone(),
            final_head: j.original_head, original_index_sha256: j.original_index_sha256,
            resulting_index_sha256: j.resulting_index_sha256,
            candidate_blob_oid: j.candidate_blob_oid,
            proposed_content_sha256: j.proposed_content_sha256,
            review_sha256: j.review_sha256, native_os_consent_required: true,
            candidate_readback_verified: true, target_conflict_cleared: true,
            journal_synced: true, branch_ref_modified: false, commit_created: false,
            remote_contacted: false,
        })
    }

    /// Restart inspection never runs a write or automatically accepts a result.
    pub fn inspect_protected_conflict_recovery(&self, id: &str)
        -> Result<ProtectedResolutionRecovery, &'static str> {
        let repo = exact_repo(self, id)?;
        let Some(j) = journal_last(&journal_path(self, id)?)? else {
            return Ok(ProtectedResolutionRecovery {
                schema_version: 1, project_id: id.to_owned(), state: "no_journal",
                phase: String::new(), operation_id: String::new(),
                relative_path: String::new(), original_head_matches: false,
                original_index_matches: false, candidate_index_entry_matches: false,
                original_worktree_matches: false, proposed_worktree_matches: false,
                foreign_index_lock_present: false,
                automatic_retry: false, automatic_restore: false,
                automatic_index_write: false, automatic_ref_update: false,
                operator_review_required: false,
            });
        };
        if j.project_id != id { return Err("G06D04_WRONG_JOURNAL_PROJECT"); }
        let root = self.registered_git_root(id).map_err(|_| "G06D04_PROJECT_CLOSED")?;
        changed_path_is_plain_safe(&root, &j.relative_path)?;
        let lock = no_foreign_lock(&repo).is_err();
        let head_ok = head_matches(&repo, &j.original_head);
        let original_index = index_hash(&repo).ok().as_deref() == Some(&j.original_index_sha256);
        let staged = !j.candidate_blob_oid.is_empty() &&
            stage0_matches(&repo, &j.relative_path, &j.candidate_blob_oid);
        let wt = worktree_bytes(&root, &j.relative_path).ok();
        let old_wt = wt.as_ref().is_some_and(|bytes| sha(bytes) == j.original_worktree_sha256);
        let new_wt = wt.as_ref().is_some_and(|bytes| sha(bytes) == j.proposed_content_sha256);
        let phase = j.phase.clone();
        let state = if !lock && head_ok && staged && new_wt {
            if j.phase == "verified" { "verified_resolved_stage" }
            else { "resolved_requires_independent_receipt" }
        } else if !lock && head_ok && original_index && old_wt &&
            matches!(j.phase.as_str(), "prepared" | "blob_ready" | "cancelled") {
            "unpublished_original_source_preserved"
        } else { "partial_or_drifted_manual_repair" };
        Ok(ProtectedResolutionRecovery {
            schema_version: 1, project_id: id.to_owned(), state, phase,
            operation_id: j.operation_id, relative_path: j.relative_path,
            original_head_matches: head_ok, original_index_matches: original_index,
            candidate_index_entry_matches: staged,
            original_worktree_matches: old_wt, proposed_worktree_matches: new_wt,
            foreign_index_lock_present: lock,
            automatic_retry: false, automatic_restore: false,
            automatic_index_write: false, automatic_ref_update: false,
            operator_review_required: !matches!(state,
                "no_journal" | "verified_resolved_stage"),
        })
    }

    /// Native OS-approved receipt reconciliation only. Never touches Git.
    pub fn reconcile_protected_conflict_receipt(&self, id: &str)
        -> Result<ProtectedResolutionRecovery, &'static str> {
        let status = self.inspect_protected_conflict_recovery(id)?;
        if !matches!(status.state, "resolved_requires_independent_receipt" |
            "unpublished_original_source_preserved") {
            return Err("G06D04_RECEIPT_NOT_SAFE");
        }
        let path = journal_path(self, id)?;
        let mut j = journal_last(&path)?.ok_or("G06D04_NO_JOURNAL")?;
        if status.operation_id != j.operation_id { return Err("G06D04_JOURNAL_CHANGED"); }
        j.phase = if status.state == "resolved_requires_independent_receipt" {
            "verified".to_owned()
        } else { "cancelled".to_owned() };
        // A second observation ensures this receipt does not certify stale data.
        if self.inspect_protected_conflict_recovery(id)?.state != status.state {
            return Err("G06D04_SOURCE_CHANGED_BEFORE_RECEIPT");
        }
        journal_append(&path, &j)?;
        self.inspect_protected_conflict_recovery(id)
    }
}

#[cfg(test)]
fn test_abort(root: &Path, gitdir: &Path, env_key: &str) {
    let Some(configured) = std::env::var_os(env_key) else { return; };
    let given = fs::canonicalize(PathBuf::from(configured)).ok();
    let actual = fs::canonicalize(root).ok();
    let tmp = fs::canonicalize(std::env::temp_dir()).ok();
    if given == actual && actual.as_ref().zip(tmp.as_ref())
        .is_some_and(|(a, t)| a.starts_with(t))
        && root.file_name().is_some_and(|x| x == "repo")
        && root.parent().and_then(|x| x.file_name())
            .is_some_and(|x| x.to_string_lossy().starts_with("ide-g06d04-disposable-"))
        && fs::read(gitdir.join("g06d04-disposable-marker")).ok().as_deref()
            == Some(b"G06D04_TEST_ONLY\n") {
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

    fn fixture() -> Fixture {
        let base = std::env::temp_dir().join(
            format!("ide-g06d04-disposable-{:032x}", rand::random::<u128>()));
        let root = base.join("repo");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("conflict.txt"), b"base\n").unwrap();
        fs::write(root.join("unrelated.keep"), b"NEVER TOUCH\n").unwrap();
        let repo = Repository::init(&root).unwrap();
        let sig = git2::Signature::now("Fixture Operator", "fixture@localhost.invalid").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("conflict.txt")).unwrap();
        index.add_path(Path::new("unrelated.keep")).unwrap();
        index.write().unwrap();
        let mut original = index.get_path(Path::new("conflict.txt"), 0).unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "base", &tree, &[]).unwrap();
        drop(tree);
        index.clear().unwrap();
        index.add_path(Path::new("unrelated.keep")).unwrap();
        for (number, bytes) in [
            (1u16, b"base\n".as_slice()),
            (2u16, b"ours\n".as_slice()),
            (3u16, b"theirs\n".as_slice()),
        ] {
            original.id = repo.blob(bytes).unwrap();
            original.file_size = bytes.len() as u32;
            original.flags = (original.flags & !0x3000) | (number << 12);
            index.add(&original).unwrap();
        }
        index.write().unwrap();
        assert!(index.has_conflicts());
        drop(index);
        drop(repo);
        let registry = IdeWorkspaceRegistry::open(base.join("catalog.json")).unwrap();
        let id = registry.register_from_native_picker(&mut Picker(root.clone()))
            .unwrap().unwrap().project_id;
        Fixture { base, root, id, registry }
    }
    fn review(x: &Fixture, text: &str) -> ConflictResolutionReview {
        let current = x.registry.inspect_native_git_conflicts(&x.id).unwrap();
        x.registry.review_native_conflict_resolution(
            &x.id, "conflict.txt", &current.evidence_sha256,
            &current.index_sha256, text).unwrap()
    }
    fn apply(x: &Fixture, text: &str) -> ProtectedResolutionReceipt {
        let current = x.registry.inspect_native_git_conflicts(&x.id).unwrap();
        let proof = review(x, text);
        x.registry.apply_os_approved_conflict_resolution(
            &x.id, "conflict.txt", &current.evidence_sha256,
            &current.index_sha256, text, &proof.review_sha256, &AtomicU8::new(0)).unwrap()
    }
    #[test]
    fn review_is_read_only_and_stale_or_forged_claims_are_refused() {
        let x = fixture();
        let original_index = fs::read(x.root.join(".git/index")).unwrap();
        let original_worktree = fs::read(x.root.join("conflict.txt")).unwrap();
        let r = review(&x, "resolved\n");
        assert_eq!(r.proposed_bytes, 9);
        assert!(r.requires_separate_native_os_approval);
        assert!(!r.authority_granted && !r.index_or_worktree_modified);
        assert_eq!(fs::read(x.root.join(".git/index")).unwrap(), original_index);
        assert_eq!(fs::read(x.root.join("conflict.txt")).unwrap(), original_worktree);
        let inventory = x.registry.inspect_native_git_conflicts(&x.id).unwrap();
        assert!(x.registry.review_native_conflict_resolution(
            &x.id, "conflict.txt", &inventory.evidence_sha256,
            &inventory.index_sha256, "modified\n").is_ok());
        assert!(x.registry.apply_os_approved_conflict_resolution(
            &x.id, "conflict.txt", &inventory.evidence_sha256,
            &inventory.index_sha256, "forged\n", &r.review_sha256, &AtomicU8::new(0)).is_err());
        assert!(x.registry.inspect_protected_conflict_recovery(&x.id).unwrap().state == "no_journal");
        assert!(x.registry.review_native_conflict_resolution(
            &x.id, "../escape", &inventory.evidence_sha256,
            &inventory.index_sha256, "x").is_err());
    }
    #[test]
    fn approved_resolution_stages_exact_blob_preserves_head_and_other_paths() {
        let x = fixture();
        let old = Repository::open(&x.root).unwrap().head().unwrap()
            .target().unwrap().to_string();
        let unrelated = fs::read(x.root.join("unrelated.keep")).unwrap();
        let r = apply(&x, "accepted\n");
        let repo = Repository::open(&x.root).unwrap();
        assert_eq!(repo.head().unwrap().target().unwrap().to_string(), old);
        assert_eq!(r.final_head, old);
        assert!(!r.branch_ref_modified && !r.commit_created && !r.remote_contacted);
        assert_eq!(fs::read(x.root.join("conflict.txt")).unwrap(), b"accepted\n");
        assert_eq!(fs::read(x.root.join("unrelated.keep")).unwrap(), unrelated);
        let idx = repo.index().unwrap();
        assert_eq!(idx.get_path(Path::new("conflict.txt"), 0).unwrap().id.to_string(),
            r.candidate_blob_oid);
        assert!((1..=3).all(|stage| idx.get_path(Path::new("conflict.txt"), stage).is_none()));
        assert_eq!(repo.find_blob(Oid::from_str(&r.candidate_blob_oid).unwrap())
            .unwrap().content(), b"accepted\n");
        assert_eq!(x.registry.inspect_protected_conflict_recovery(&x.id)
            .unwrap().state, "verified_resolved_stage");
    }
    #[test]
    fn cancellation_and_external_git_lock_preserve_everything() {
        let x = fixture();
        let inventory = x.registry.inspect_native_git_conflicts(&x.id).unwrap();
        let r = review(&x, "accepted\n");
        let original_index = fs::read(x.root.join(".git/index")).unwrap();
        let original_wt = fs::read(x.root.join("conflict.txt")).unwrap();
        assert!(x.registry.apply_os_approved_conflict_resolution(
            &x.id, "conflict.txt", &inventory.evidence_sha256,
            &inventory.index_sha256, "accepted\n", &r.review_sha256,
            &AtomicU8::new(2)).is_err());
        fs::write(x.root.join(".git/index.lock"), b"OTHER_GIT_PROCESS").unwrap();
        assert!(x.registry.apply_os_approved_conflict_resolution(
            &x.id, "conflict.txt", &inventory.evidence_sha256,
            &inventory.index_sha256, "accepted\n", &r.review_sha256,
            &AtomicU8::new(0)).is_err());
        assert_eq!(fs::read(x.root.join(".git/index.lock")).unwrap(), b"OTHER_GIT_PROCESS");
        assert_eq!(fs::read(x.root.join(".git/index")).unwrap(), original_index);
        assert_eq!(fs::read(x.root.join("conflict.txt")).unwrap(), original_wt);
    }
    #[test]
    fn child_abort_entry() {
        let Some(raw) = std::env::var_os("IDE_G06D04_CHILD_ROOT") else { return; };
        let root = PathBuf::from(raw);
        let id = std::env::var("IDE_G06D04_CHILD_ID").unwrap();
        let registry = IdeWorkspaceRegistry::open(root.parent().unwrap().join("catalog.json"))
            .unwrap();
        registry.reopen(&id).unwrap();
        let inventory = registry.inspect_native_git_conflicts(&id).unwrap();
        let review = registry.review_native_conflict_resolution(
            &id, "conflict.txt", &inventory.evidence_sha256,
            &inventory.index_sha256, "accepted\n").unwrap();
        let _ = registry.apply_os_approved_conflict_resolution(
            &id, "conflict.txt", &inventory.evidence_sha256,
            &inventory.index_sha256, "accepted\n",
            &review.review_sha256, &AtomicU8::new(0));
        panic!("G06D04 child returned without genuine abort");
    }
    fn spawn_crash(x: &Fixture, phase: &str) {
        fs::write(x.root.join(".git/g06d04-disposable-marker"),
            b"G06D04_TEST_ONLY\n").unwrap();
        let result = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg("git_conflict_resolution::tests::child_abort_entry")
            .arg("--nocapture")
            .env("IDE_G06D04_CHILD_ROOT", &x.root)
            .env("IDE_G06D04_CHILD_ID", &x.id)
            .env(phase, &x.root)
            .output().unwrap();
        assert!(!result.status.success());
        assert!(!String::from_utf8_lossy(&result.stderr)
            .contains("returned without genuine abort"),
            "child must abort at the requested boundary");
    }
    #[test]
    fn genuine_abort_after_file_replace_is_manual_repair_only() {
        if std::env::var_os("IDE_G06D04_CHILD_ROOT").is_some() { return; }
        let x = fixture();
        let old = fs::read(x.root.join(".git/index")).unwrap();
        spawn_crash(&x, "IDE_G06D04_ABORT_AFTER_WORKTREE");
        assert_eq!(fs::read(x.root.join("conflict.txt")).unwrap(), b"accepted\n");
        assert_eq!(fs::read(x.root.join(".git/index")).unwrap(), old);
        let restarted = IdeWorkspaceRegistry::open(x.base.join("catalog.json")).unwrap();
        restarted.reopen(&x.id).unwrap();
        let state = restarted.inspect_protected_conflict_recovery(&x.id).unwrap();
        assert_eq!(state.state, "partial_or_drifted_manual_repair");
        assert!(state.operator_review_required && !state.automatic_retry);
        assert!(restarted.reconcile_protected_conflict_receipt(&x.id).is_err());
    }
    #[test]
    fn genuine_abort_after_index_write_requires_receipt_only() {
        if std::env::var_os("IDE_G06D04_CHILD_ROOT").is_some() { return; }
        let x = fixture();
        let old = Repository::open(&x.root).unwrap().head().unwrap()
            .target().unwrap().to_string();
        spawn_crash(&x, "IDE_G06D04_ABORT_AFTER_INDEX");
        let restarted = IdeWorkspaceRegistry::open(x.base.join("catalog.json")).unwrap();
        restarted.reopen(&x.id).unwrap();
        let state = restarted.inspect_protected_conflict_recovery(&x.id).unwrap();
        assert_eq!(state.state, "resolved_requires_independent_receipt");
        assert!(state.operator_review_required);
        assert!(state.candidate_index_entry_matches && state.proposed_worktree_matches);
        let after_receipt = restarted.reconcile_protected_conflict_receipt(&x.id).unwrap();
        assert_eq!(after_receipt.state, "verified_resolved_stage");
        assert_eq!(Repository::open(&x.root).unwrap().head().unwrap()
            .target().unwrap().to_string(), old);
    }
}
