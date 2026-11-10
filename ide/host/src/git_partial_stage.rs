//! 6B-03: one-file selected-hunk index transaction after trusted OS approval.
//! No Git subprocess, hook, remote, checkout, source write, branch mutation,
//! or implicit approval. Only exact reviewed text hunks enter the Git index.
use crate::{
    git_hunk_review::{ReviewedHunk, ReviewedHunkSelection},
    IdeWorkspaceRegistry,
};
use git2::{Index, IndexEntry, Repository};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU8, Ordering},
};

const MAX_INDEX_FILE: u64 = 8 * 1024 * 1024;
const MAX_JOURNAL: u64 = 256 * 1024;
const MAX_SELECTED_BYTES: usize = 1024 * 1024;
const MAX_SELECTED_HUNKS: usize = 48;

/// Cancellation state: 0=cancelable, 1=commit-point reached, 2=canceled.
/// A compare-exchange is the exact boundary before atomic index publication.
/// Native Tauri may cancel only while still in state zero.
pub fn request_partial_stage_cancel(state: &AtomicU8) -> bool {
    state
        .compare_exchange(0, 2, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
}

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartialStageReceipt {
    pub schema_version: u32,
    pub project_id: String,
    pub relative_path: String,
    pub selection_sha256: String,
    pub previous_head: String,
    pub old_index_sha256: String,
    pub new_index_sha256: String,
    pub old_blob_oid: String,
    pub staged_blob_oid: String,
    pub staged_blob_sha256: String,
    pub selected_hunks: usize,
    pub git_index_independently_verified: bool,
    pub source_file_modified: bool,
    pub git_hooks_executed: bool,
    pub git_process_started: bool,
    pub remote_updated: bool,
    pub commit_created: bool,
    pub durable_journal_verified: bool,
    pub recovery_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JournalLine {
    schema_version: u32,
    operation_id: String,
    project_id: String,
    relative_path: String,
    phase: String,
    head_sha: String,
    selection_sha256: String,
    previous_index_sha256: String,
    intended_index_sha256: String,
    intended_blob_oid: String,
    intended_blob_sha256: String,
    source_sha256: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PartialStageRecovery {
    pub schema_version: u32,
    pub project_id: String,
    pub state: &'static str,
    pub last_phase: String,
    pub index_matches_original: bool,
    pub index_matches_intended: bool,
    pub pending_git_lock: bool,
    pub head_matches_original: bool,
    pub operator_reconciliation_required: bool,
    pub auto_delete_git_lock: bool,
    pub auto_rollback_index: bool,
    pub write_authority_granted: bool,
}

fn state_root(registry: &IdeWorkspaceRegistry) -> Result<PathBuf, &'static str> {
    let data = registry
        .catalog
        .parent()
        .ok_or("G06B_STAGE_DATA_ROOT_MISSING")?;
    let root = data.join("ide-g06b-partial-stage");
    fs::create_dir_all(&root).map_err(|_| "G06B_STAGE_JOURNAL_ROOT_UNAVAILABLE")?;
    if fs::symlink_metadata(&root)
        .map_err(|_| "G06B_STAGE_JOURNAL_ROOT_UNAVAILABLE")?
        .file_type()
        .is_symlink()
    {
        return Err("G06B_STAGE_JOURNAL_ROOT_LINK_DENIED");
    }
    Ok(root)
}

fn journal_path(
    registry: &IdeWorkspaceRegistry,
    project_id: &str,
) -> Result<PathBuf, &'static str> {
    // Validate project ID via the registered native capability before using
    // it in an app-data filename. Never accept a WebView supplied path.
    registry
        .registered_git_root(project_id)
        .map_err(|_| "G06B_STAGE_PROJECT_NOT_ACTIVE")?;
    if project_id.is_empty()
        || project_id.len() > 128
        || !project_id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
    {
        return Err("G06B_STAGE_PROJECT_ID_INVALID");
    }
    Ok(state_root(registry)?.join(format!("{project_id}.jsonl")))
}

fn last_journal(path: &Path) -> Result<Option<JournalLine>, &'static str> {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("G06B_STAGE_JOURNAL_UNREADABLE"),
    };
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > MAX_JOURNAL {
        return Err("G06B_STAGE_JOURNAL_UNTRUSTED");
    }
    let bytes = fs::read(path).map_err(|_| "G06B_STAGE_JOURNAL_UNREADABLE")?;
    if bytes.is_empty() {
        return Err("G06B_STAGE_JOURNAL_TRUNCATED");
    }
    if bytes.last() != Some(&b'\n') {
        return Err("G06B_STAGE_JOURNAL_INCOMPLETE");
    }
    let last = bytes
        .split(|b| *b == b'\n')
        .filter(|s| !s.is_empty())
        .last()
        .ok_or("G06B_STAGE_JOURNAL_INCOMPLETE")?;
    let line: JournalLine =
        serde_json::from_slice(last).map_err(|_| "G06B_STAGE_JOURNAL_INVALID")?;
    if line.schema_version != 1 {
        return Err("G06B_STAGE_JOURNAL_VERSION");
    }
    Ok(Some(line))
}

fn journal_append(path: &Path, entry: &JournalLine) -> Result<(), &'static str> {
    match fs::symlink_metadata(path) {
        Ok(meta)
            if !meta.is_file()
                || meta.file_type().is_symlink()
                || meta.len() > MAX_JOURNAL - 4096 =>
        {
            return Err("G06B_STAGE_JOURNAL_UNTRUSTED")
        }
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err("G06B_STAGE_JOURNAL_UNREADABLE"),
    }
    let bytes = serde_json::to_vec(entry).map_err(|_| "G06B_STAGE_JOURNAL_SERIALIZE")?;
    if bytes.len() > 2048 {
        return Err("G06B_STAGE_JOURNAL_RECORD_OVERSIZED");
    }
    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(path)
        .map_err(|_| "G06B_STAGE_JOURNAL_WRITE_DENIED")?;
    file.write_all(&bytes)
        .and_then(|_| file.write_all(b"\n"))
        .and_then(|_| file.sync_all())
        .map_err(|_| "G06B_STAGE_JOURNAL_WRITE_FAILED")?;
    #[cfg(unix)]
    File::open(path.parent().ok_or("G06B_STAGE_JOURNAL_ROOT_MISSING")?)
        .and_then(|dir| dir.sync_all())
        .map_err(|_| "G06B_STAGE_JOURNAL_DIR_SYNC_FAILED")?;
    Ok(())
}

fn selected_index_bytes(
    index_bytes: &[u8],
    selection: &ReviewedHunkSelection,
) -> Result<Vec<u8>, &'static str> {
    // Byte-exact line reconstruction. Disallow missing final newline and
    // "no newline" markers in this first conservative partial-stage slice.
    if !index_bytes.is_empty() && !index_bytes.ends_with(b"\n") {
        return Err("G06B_STAGE_NO_FINAL_NEWLINE_UNSUPPORTED");
    }
    let original = index_bytes
        .split_inclusive(|c| *c == b'\n')
        .collect::<Vec<_>>();
    let mut output = Vec::with_capacity(index_bytes.len());
    let mut cursor = 0_usize;
    let mut selected = 0_usize;
    for h in &selection.plan.hunks {
        if !selection.selected_ids.iter().any(|id| id == &h.id) {
            continue;
        }
        selected += 1;
        let start = if h.old_start == 0 {
            0
        } else {
            h.old_start as usize - 1
        };
        if start < cursor || start > original.len() {
            return Err("G06B_STAGE_OVERLAPPING_HUNKS");
        }
        for line in &original[cursor..start] {
            output.extend_from_slice(line);
        }
        cursor = start;
        let mut old_count = 0_u32;
        let mut new_count = 0_u32;
        for line in &h.lines {
            let bytes = line.text.as_bytes();
            if bytes.is_empty() || !bytes.ends_with(b"\n") {
                return Err("G06B_STAGE_NO_FINAL_NEWLINE_UNSUPPORTED");
            }
            match line.origin {
                ' ' | '-' => {
                    if cursor >= original.len() || original[cursor] != bytes {
                        return Err("G06B_STAGE_HUNK_CONTEXT_MISMATCH");
                    }
                    if line.origin == ' ' {
                        output.extend_from_slice(bytes);
                        new_count += 1;
                    }
                    cursor += 1;
                    old_count += 1;
                }
                '+' => {
                    output.extend_from_slice(bytes);
                    new_count += 1;
                }
                _ => return Err("G06B_STAGE_UNSUPPORTED_HUNK_LINE"),
            }
            if output.len() > MAX_SELECTED_BYTES {
                return Err("G06B_STAGE_SELECTED_OUTPUT_OVER_LIMIT");
            }
        }
        if old_count != h.old_lines || new_count != h.new_lines {
            return Err("G06B_STAGE_HUNK_LINE_COUNTS_INVALID");
        }
    }
    if selected == 0 || selected > MAX_SELECTED_HUNKS {
        return Err("G06B_STAGE_SELECTION_EMPTY_OR_OVER_LIMIT");
    }
    for line in &original[cursor..] {
        output.extend_from_slice(line);
    }
    if output.len() > MAX_SELECTED_BYTES {
        return Err("G06B_STAGE_SELECTED_OUTPUT_OVER_LIMIT");
    }
    Ok(output)
}

fn verified_repo(
    registry: &IdeWorkspaceRegistry,
    project_id: &str,
    for_write: bool,
) -> Result<(Repository, PathBuf, PathBuf), &'static str> {
    let root = registry
        .registered_git_root(project_id)
        .map_err(|_| "G06B_STAGE_PROJECT_NOT_ACTIVE")?;
    let repo = Repository::open(&root).map_err(|_| "G06B_STAGE_REPOSITORY_MISSING")?;
    if repo.is_worktree()
        || repo.is_bare()
        || (for_write
            && (repo.head_detached().unwrap_or(true)
                || repo.state() != git2::RepositoryState::Clean))
    {
        return Err("G06B_STAGE_GIT_OPERATION_OR_WORKTREE_DENIED");
    }
    let actual = fs::canonicalize(repo.workdir().ok_or("G06B_STAGE_BARE_DENIED")?)
        .map_err(|_| "G06B_STAGE_ROOT_INVALID")?;
    if actual != root {
        return Err("G06B_STAGE_GIT_ROOT_CHANGED");
    }
    let gitdir = root.join(".git");
    let meta = fs::symlink_metadata(&gitdir).map_err(|_| "G06B_STAGE_GITDIR_INVALID")?;
    if !meta.is_dir()
        || meta.file_type().is_symlink()
        || fs::canonicalize(repo.path()).map_err(|_| "G06B_STAGE_GITDIR_INVALID")?
            != fs::canonicalize(&gitdir).map_err(|_| "G06B_STAGE_GITDIR_INVALID")?
    {
        return Err("G06B_STAGE_LINKED_OR_EXTERNAL_GITDIR_DENIED");
    }
    Ok((repo, root, gitdir))
}

fn index_bytes(path: &Path) -> Result<Vec<u8>, &'static str> {
    let info = fs::symlink_metadata(path).map_err(|_| "G06B_STAGE_INDEX_UNREADABLE")?;
    if !info.is_file() || info.file_type().is_symlink() || info.len() > MAX_INDEX_FILE {
        return Err("G06B_STAGE_INDEX_UNTRUSTED");
    }
    fs::read(path).map_err(|_| "G06B_STAGE_INDEX_UNREADABLE")
}

struct RemoveOnDrop {
    path: PathBuf,
    armed: bool,
}
impl RemoveOnDrop {
    fn new(path: PathBuf) -> Self {
        Self { path, armed: true }
    }
    fn disarm(&mut self) {
        self.armed = false;
    }
}
impl Drop for RemoveOnDrop {
    fn drop(&mut self) {
        if self.armed {
            let _ = fs::remove_file(&self.path);
        }
    }
}

impl IdeWorkspaceRegistry {
    /// Read-only native recovery classification, also available after restart.
    /// Never delete an unknown index.lock or rewrite an ambiguous index.
    pub fn inspect_partial_stage_recovery(
        &self,
        project_id: &str,
    ) -> Result<PartialStageRecovery, &'static str> {
        let (repo, _root, gitdir) = verified_repo(self, project_id, false)?;
        let path = journal_path(self, project_id)?;
        let last = last_journal(&path)?;
        let lock_exists = fs::symlink_metadata(gitdir.join("index.lock")).is_ok();
        let index = index_bytes(&gitdir.join("index"))?;
        let digest = sha(&index);
        let head = repo
            .head()
            .ok()
            .and_then(|h| h.target())
            .map(|o| o.to_string());
        let (state, last_phase, old, intended, original_head) = match last {
            None => ("no_journal", String::new(), false, false, true),
            Some(j) if j.project_id != project_id => {
                return Err("G06B_STAGE_JOURNAL_PROJECT_MISMATCH")
            }
            Some(j) => {
                let old = digest == j.previous_index_sha256;
                let intended = digest == j.intended_index_sha256;
                let original_head = head.as_deref() == Some(j.head_sha.as_str());
                // Terminal journal receipts are historical facts. A later
                // authorized Git commit naturally changes the index bytes;
                // that does not retroactively turn a finished operation
                // into an interrupted one.
                let state = if j.phase == "verified" {
                    if intended {
                        "verified_receipt_observed"
                    } else {
                        "verified_receipt_followed_by_git_changes"
                    }
                } else if j.phase == "cancelled" {
                    if old {
                        "cancelled_index_unchanged"
                    } else {
                        "cancelled_receipt_followed_by_git_changes"
                    }
                } else if !original_head {
                    "head_changed_since_journal"
                } else if intended {
                    "index_published_requires_reconciliation"
                } else if old {
                    "index_not_published_or_external_change"
                } else {
                    "index_drift_or_unverifiable"
                };
                (state, j.phase, old, intended, original_head)
            }
        };
        let requires = lock_exists
            || !matches!(
                state,
                "no_journal"
                    | "verified_receipt_observed"
                    | "verified_receipt_followed_by_git_changes"
                    | "cancelled_index_unchanged"
                    | "cancelled_receipt_followed_by_git_changes"
            );
        Ok(PartialStageRecovery {
            schema_version: 1,
            project_id: project_id.to_owned(),
            state,
            last_phase,
            index_matches_original: old,
            index_matches_intended: intended,
            pending_git_lock: lock_exists,
            head_matches_original: original_head,
            operator_reconciliation_required: requires,
            auto_delete_git_lock: false,
            auto_rollback_index: false,
            write_authority_granted: false,
        })
    }

    /// Explicitly reconcile a journal after process restart. No Git index
    /// mutation is ever done here. Only an independently checked terminal
    /// receipt is appended, and foreign/stale index locks remain untouched.
    /// Tauri must require separate genuine OS operator consent.
    pub fn reconcile_partial_stage_journal(
        &self,
        project_id: &str,
    ) -> Result<PartialStageRecovery, &'static str> {
        let (repo, _root, gitdir) = verified_repo(self, project_id, false)?;
        if repo.state() != git2::RepositoryState::Clean {
            return Err("G06B_STAGE_RECOVERY_GIT_OPERATION_IN_PROGRESS");
        }
        let path = journal_path(self, project_id)?;
        let mut last = last_journal(&path)?.ok_or("G06B_STAGE_RECOVERY_NO_JOURNAL")?;
        if last.project_id != project_id {
            return Err("G06B_STAGE_RECOVERY_DIFFERENT_PROJECT");
        }
        if matches!(last.phase.as_str(), "verified" | "cancelled") {
            return self.inspect_partial_stage_recovery(project_id);
        }
        if fs::symlink_metadata(gitdir.join("index.lock")).is_ok() {
            return Err("G06B_STAGE_RECOVERY_GIT_LOCK_REQUIRES_OPERATOR");
        }
        let head = repo
            .head()
            .ok()
            .and_then(|h| h.target())
            .map(|o| o.to_string());
        if head.as_deref() != Some(last.head_sha.as_str()) {
            return Err("G06B_STAGE_RECOVERY_HEAD_CHANGED");
        }
        let current_sha = sha(&index_bytes(&gitdir.join("index"))?);
        if current_sha == last.previous_index_sha256
            && matches!(last.phase.as_str(), "prepared" | "publishing")
        {
            last.phase = "cancelled".into();
            journal_append(&path, &last)?;
            return self.inspect_partial_stage_recovery(project_id);
        }
        if current_sha != last.intended_index_sha256
            || !matches!(last.phase.as_str(), "publishing" | "published")
        {
            return Err("G06B_STAGE_RECOVERY_AMBIGUOUS_INDEX");
        }
        let mut index = repo
            .index()
            .map_err(|_| "G06B_STAGE_RECOVERY_INDEX_UNAVAILABLE")?;
        index
            .read(true)
            .map_err(|_| "G06B_STAGE_RECOVERY_INDEX_RELOAD_FAILED")?;
        if index.has_conflicts() {
            return Err("G06B_STAGE_RECOVERY_CONFLICTED_INDEX");
        }
        let entry = index
            .get_path(Path::new(&last.relative_path), 0)
            .ok_or("G06B_STAGE_RECOVERY_STAGED_FILE_MISSING")?;
        if entry.id.to_string() != last.intended_blob_oid {
            return Err("G06B_STAGE_RECOVERY_BLOB_OID_MISMATCH");
        }
        let blob = repo
            .find_blob(entry.id)
            .map_err(|_| "G06B_STAGE_RECOVERY_BLOB_MISSING")?;
        if sha(blob.content()) != last.intended_blob_sha256 {
            return Err("G06B_STAGE_RECOVERY_STAGED_BYTES_MISMATCH");
        }
        let original_source =
            self.reread_reviewed_git_source_bytes(project_id, &last.relative_path)?;
        if sha(&original_source) != last.source_sha256 {
            return Err("G06B_STAGE_RECOVERY_SOURCE_CHANGED");
        }
        // Index, object, source, HEAD, and lock absence independently
        // verified; a durable terminal receipt is now safe to append.
        last.phase = "verified".into();
        journal_append(&path, &last)?;
        self.inspect_partial_stage_recovery(project_id)
    }

    /// Only trusted native host calls this after a fresh physical OS dialog.
    /// The cancellation state is host-owned; WebView cannot create/forge it.
    pub fn apply_approved_partial_stage(
        &self,
        reviewed: &ReviewedHunkSelection,
        cancel: &AtomicU8,
    ) -> Result<PartialStageReceipt, &'static str> {
        if cancel.load(Ordering::SeqCst) != 0
            || reviewed.schema_version != 1
            || !reviewed.ready_for_operator_review
            || reviewed.stage_authorized
            || reviewed.commit_authorized
            || reviewed.operation_performed
        {
            return Err("G06B_STAGE_NOT_APPROVED_OR_CANCELLED");
        }
        let fresh = self.select_reviewed_git_hunks(&reviewed.plan, &reviewed.selected_ids)?;
        if fresh.selection_sha256 != reviewed.selection_sha256
            || fresh.selected_ids != reviewed.selected_ids
        {
            return Err("G06B_STAGE_SELECTION_STALE_OR_FORGED");
        }
        let (repo, _root, gitdir) = verified_repo(self, &fresh.plan.project_id, true)?;
        let old_index_path = gitdir.join("index");
        let path = journal_path(self, &fresh.plan.project_id)?;
        if let Some(last) = last_journal(&path)? {
            if !matches!(last.phase.as_str(), "verified" | "cancelled") {
                return Err("G06B_STAGE_RECOVERY_REQUIRED_BEFORE_NEW_WRITE");
            }
        }
        if fs::symlink_metadata(gitdir.join("index.lock")).is_ok() {
            return Err("G06B_STAGE_FOREIGN_OR_STALE_INDEX_LOCK");
        }
        let pinned =
            self.read_reviewed_git_hunk_buffers(&fresh.plan.project_id, &fresh.plan.relative_path)?;
        if pinned.review.local_head != fresh.plan.head_sha
            || pinned.review.index_fingerprint != fresh.plan.index_fingerprint
            || pinned.review.worktree_sha256 != fresh.plan.source_sha256
            || pinned.index_blob_oid != fresh.plan.index_blob_oid
        {
            return Err("G06B_STAGE_SOURCE_OR_INDEX_CHANGED");
        }
        let projected = selected_index_bytes(&pinned.index_bytes, &fresh)?;
        let staged_sha = sha(&projected);
        let oid = repo
            .blob(&projected)
            .map_err(|_| "G06B_STAGE_BLOB_WRITE_FAILED")?;
        let mut index = repo.index().map_err(|_| "G06B_STAGE_INDEX_UNAVAILABLE")?;
        index
            .read(true)
            .map_err(|_| "G06B_STAGE_INDEX_UNAVAILABLE")?;
        if index.has_conflicts() || index.len() > 8192 || index.version() > 3 {
            return Err("G06B_STAGE_COMPLEX_INDEX_DENIED");
        }
        let tree = repo
            .head()
            .and_then(|r| r.peel_to_tree())
            .map_err(|_| "G06B_STAGE_HEAD_UNAVAILABLE")?;
        if repo
            .diff_tree_to_index(Some(&tree), Some(&index), None)
            .map_err(|_| "G06B_STAGE_STAGED_DIFF_UNVERIFIABLE")?
            .deltas()
            .next()
            .is_some()
        {
            return Err("G06B_STAGE_PREEXISTING_STAGED_CHANGES");
        }
        let mut selected_entry = index
            .get_path(Path::new(&fresh.plan.relative_path), 0)
            .ok_or("G06B_STAGE_TRACKED_ENTRY_MISSING")?;
        if !matches!(selected_entry.mode, 0o100644 | 0o100755)
            || selected_entry.id.to_string() != fresh.plan.index_blob_oid
        {
            return Err("G06B_STAGE_NONREGULAR_OR_CHANGED_ENTRY");
        }
        let old_file_bytes = index_bytes(&old_index_path)?;
        let old_sha = sha(&old_file_bytes);
        selected_entry.id = oid;
        selected_entry.file_size = projected.len() as u32;
        selected_entry.ctime = git2::IndexTime::new(0, 0);
        selected_entry.mtime = git2::IndexTime::new(0, 0);
        // Serialize to an app-private temporary index. Only after it has
        // been fully verified do we publish using Git's exclusive lockfile.
        let op = format!("{:032x}", rand::random::<u128>());
        let temp_path = state_root(self)?.join(format!("{}.{}.index", fresh.plan.project_id, op));
        let _temp_guard = RemoveOnDrop::new(temp_path.clone());
        let mut temp = Index::open(&temp_path).map_err(|_| "G06B_STAGE_TEMP_INDEX_UNAVAILABLE")?;
        for entry in index.iter() {
            temp.add(&entry)
                .map_err(|_| "G06B_STAGE_TEMP_INDEX_COPY_FAILED")?;
        }
        temp.add(&selected_entry)
            .map_err(|_| "G06B_STAGE_TEMP_INDEX_UPDATE_FAILED")?;
        temp.write()
            .map_err(|_| "G06B_STAGE_TEMP_INDEX_WRITE_FAILED")?;
        let prepared_bytes = index_bytes(&temp_path)?;
        let intended_sha = sha(&prepared_bytes);
        let candidate = temp
            .get_path(Path::new(&fresh.plan.relative_path), 0)
            .ok_or("G06B_STAGE_TEMP_ENTRY_MISSING")?;
        if candidate.id != oid
            || repo
                .find_blob(candidate.id)
                .map_err(|_| "G06B_STAGE_TEMP_BLOB_MISSING")?
                .content()
                != projected.as_slice()
        {
            return Err("G06B_STAGE_TEMP_INDEX_VERIFICATION_FAILED");
        }
        for before in index.iter() {
            let now = temp
                .get_path(
                    Path::new(
                        std::str::from_utf8(&before.path)
                            .map_err(|_| "G06B_STAGE_NONUTF8_PATH_DENIED")?,
                    ),
                    0,
                )
                .ok_or("G06B_STAGE_TEMP_UNRELATED_ENTRY_MISSING")?;
            if before.path != selected_entry.path
                && (before.id != now.id || before.mode != now.mode || before.path != now.path)
            {
                return Err("G06B_STAGE_TEMP_UNRELATED_ENTRY_CHANGED");
            }
        }
        let mut journal = JournalLine {
            schema_version: 1,
            operation_id: op,
            project_id: fresh.plan.project_id.clone(),
            relative_path: fresh.plan.relative_path.clone(),
            phase: "prepared".into(),
            head_sha: fresh.plan.head_sha.clone(),
            selection_sha256: fresh.selection_sha256.clone(),
            previous_index_sha256: old_sha.clone(),
            intended_index_sha256: intended_sha.clone(),
            intended_blob_oid: oid.to_string(),
            intended_blob_sha256: staged_sha.clone(),
            source_sha256: fresh.plan.source_sha256.clone(),
        };
        if cancel.load(Ordering::SeqCst) != 0 {
            return Err("G06B_STAGE_CANCELLED_BEFORE_JOURNAL");
        }
        journal_append(&path, &journal)?;
        let lock_path = gitdir.join("index.lock");
        let mut lock = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
            .map_err(|_| "G06B_STAGE_INDEX_LOCK_UNAVAILABLE")?;
        let mut lock_guard = RemoveOnDrop::new(lock_path.clone());
        // Other well-behaved Git writers cannot acquire index.lock now.
        if sha(&index_bytes(&old_index_path)?) != old_sha
            || repo
                .head()
                .ok()
                .and_then(|r| r.target())
                .map(|o| o.to_string())
                .as_deref()
                != Some(fresh.plan.head_sha.as_str())
        {
            return Err("G06B_STAGE_RACE_BEFORE_PUBLICATION");
        }
        lock.write_all(&prepared_bytes)
            .and_then(|_| lock.sync_all())
            .map_err(|_| "G06B_STAGE_INDEX_LOCK_WRITE_FAILED")?;
        // One final OS-capability reread before the irreversible boundary.
        // A concurrent source change refuses without publishing the index.
        let current_source = self
            .reread_reviewed_git_source_bytes(&fresh.plan.project_id, &fresh.plan.relative_path)?;
        if sha(&current_source) != fresh.plan.source_sha256 {
            journal.phase = "cancelled".into();
            journal_append(&path, &journal)?;
            return Err("G06B_STAGE_SOURCE_CHANGED_BEFORE_PUBLICATION");
        }
        if cancel.load(Ordering::SeqCst) != 0 {
            journal.phase = "cancelled".into();
            journal_append(&path, &journal)?;
            return Err("G06B_STAGE_CANCELLED_BEFORE_PUBLICATION");
        }
        // Persist the point of no return before attempting atomic rename.
        journal.phase = "publishing".into();
        journal_append(&path, &journal)?;
        if cancel
            .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            journal.phase = "cancelled".into();
            journal_append(&path, &journal)?;
            return Err("G06B_STAGE_CANCELLED_BEFORE_PUBLICATION");
        }
        drop(lock);
        // Do not unlink a *new* index.lock created by another Git writer
        // after our successful rename. If rename fails, leave our lock for
        // explicit crash recovery rather than deleting uncertain bytes.
        lock_guard.disarm();
        fs::rename(&lock_path, &old_index_path)
            .map_err(|_| "G06B_STAGE_ATOMIC_INDEX_REPLACE_FAILED")?;
        #[cfg(unix)]
        File::open(&gitdir)
            .and_then(|dir| dir.sync_all())
            .map_err(|_| "G06B_STAGE_INDEX_DIR_SYNC_FAILED")?;
        journal.phase = "published".into();
        journal_append(&path, &journal)?;
        if sha(&index_bytes(&old_index_path)?) != intended_sha {
            return Err("G06B_STAGE_PUBLISHED_INDEX_HASH_MISMATCH");
        }
        // libgit2 may retain an index handle created before our exclusive
        // atomic rename. Force a disk reload before independent verification.
        let mut after = repo
            .index()
            .map_err(|_| "G06B_STAGE_INDEX_VERIFY_UNAVAILABLE")?;
        after
            .read(true)
            .map_err(|_| "G06B_STAGE_PUBLISHED_INDEX_RELOAD_FAILED")?;
        let staged_entry = after
            .get_path(Path::new(&fresh.plan.relative_path), 0)
            .ok_or("G06B_STAGE_ENTRY_NOT_PUBLISHED")?;
        if staged_entry.id != oid {
            return Err("G06B_STAGE_PUBLISHED_OID_MISMATCH");
        }
        let actual_blob = repo
            .find_blob(staged_entry.id)
            .map_err(|_| "G06B_STAGE_PUBLISHED_BLOB_MISSING")?;
        if sha(actual_blob.content()) != staged_sha {
            return Err("G06B_STAGE_PUBLISHED_BYTES_MISMATCH");
        }
        // Independently reread exact worktree bytes under the original
        // registered OS capability. A dirty index is EXPECTED here and
        // cannot be used as a proxy for original source verification.
        let fresh_source = self
            .reread_reviewed_git_source_bytes(&fresh.plan.project_id, &fresh.plan.relative_path)?;
        if sha(&fresh_source) != fresh.plan.source_sha256 {
            return Err("G06B_STAGE_SOURCE_CHANGED_AFTER_PUBLICATION");
        }
        journal.phase = "verified".into();
        journal_append(&path, &journal)?;
        Ok(PartialStageReceipt {
            schema_version: 1,
            project_id: fresh.plan.project_id.clone(),
            relative_path: fresh.plan.relative_path.clone(),
            selection_sha256: fresh.selection_sha256.clone(),
            previous_head: fresh.plan.head_sha.clone(),
            old_index_sha256: old_sha,
            new_index_sha256: intended_sha,
            old_blob_oid: fresh.plan.index_blob_oid.clone(),
            staged_blob_oid: oid.to_string(),
            staged_blob_sha256: staged_sha,
            selected_hunks: fresh.selected_ids.len(),
            git_index_independently_verified: true,
            source_file_modified: false,
            git_hooks_executed: false,
            git_process_started: false,
            remote_updated: false,
            commit_created: false,
            durable_journal_verified: true,
            recovery_required: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{IdeError, NativeDirectoryPicker};
    use git2::{Repository, Signature};
    use std::{fs, path::PathBuf};

    struct Picker(PathBuf);
    impl NativeDirectoryPicker for Picker {
        fn choose_project_directory(&mut self) -> Result<Option<PathBuf>, IdeError> {
            Ok(Some(self.0.clone()))
        }
    }
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn fixture() -> (Fixture, IdeWorkspaceRegistry, String, PathBuf) {
        let base = std::env::temp_dir().join(format!("ide-g06b03-{:032x}", rand::random::<u128>()));
        let root = base.join("project");
        fs::create_dir_all(&root).unwrap();
        let original = (1..=50)
            .map(|n| format!("row-{n:02}\n"))
            .collect::<String>();
        fs::write(root.join("tracked.txt"), &original).unwrap();
        let repo = Repository::init(&root).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("tracked.txt")).unwrap();
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let sig = Signature::now("Fixture", "fixture@localhost.invalid").unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[])
            .unwrap();
        drop(tree);
        drop(index);
        drop(repo);
        let registry = IdeWorkspaceRegistry::open(base.join("catalog.json")).unwrap();
        let id = registry
            .register_from_native_picker(&mut Picker(root.clone()))
            .unwrap()
            .unwrap()
            .project_id;
        (Fixture(base), registry, id, root)
    }

    fn reviewed(registry: &IdeWorkspaceRegistry, id: &str, root: &Path) -> ReviewedHunkSelection {
        let original = fs::read_to_string(root.join("tracked.txt")).unwrap();
        fs::write(
            root.join("tracked.txt"),
            original
                .replace("row-03\n", "selected-row-03\n")
                .replace("row-46\n", "unselected-row-46\n"),
        )
        .unwrap();
        let plan = registry
            .preview_reviewed_git_hunks(id, "tracked.txt")
            .unwrap();
        assert_eq!(plan.hunks.len(), 2);
        registry
            .select_reviewed_git_hunks(&plan, &[plan.hunks[0].id.clone()])
            .unwrap()
    }

    #[test]
    fn selected_hunk_only_is_published_original_worktree_and_other_lines_preserved() {
        let (_temp, registry, id, root) = fixture();
        let selected = reviewed(&registry, &id, &root);
        let cancel = AtomicU8::new(0);
        let before_source = fs::read(root.join("tracked.txt")).unwrap();
        let receipt = registry
            .apply_approved_partial_stage(&selected, &cancel)
            .unwrap();
        let repo = Repository::open(&root).unwrap();
        let index = repo.index().unwrap();
        let entry = index.get_path(Path::new("tracked.txt"), 0).unwrap();
        let staged = repo.find_blob(entry.id).unwrap();
        let actual = std::str::from_utf8(staged.content()).unwrap();
        assert!(actual.contains("selected-row-03"));
        assert!(actual.contains("row-46\n"));
        assert!(!actual.contains("unselected-row-46"));
        assert_eq!(fs::read(root.join("tracked.txt")).unwrap(), before_source);
        assert!(receipt.git_index_independently_verified && receipt.durable_journal_verified);
        assert!(!receipt.source_file_modified && !receipt.commit_created);
        let recovery = registry.inspect_partial_stage_recovery(&id).unwrap();
        assert_eq!(recovery.state, "verified_receipt_observed");
        assert!(!recovery.operator_reconciliation_required);
    }

    #[test]
    fn cancellation_before_publication_denies_without_index_changes() {
        let (_temp, registry, id, root) = fixture();
        let selection = reviewed(&registry, &id, &root);
        let before = fs::read(root.join(".git/index")).unwrap();
        let cancel = AtomicU8::new(0);
        assert!(request_partial_stage_cancel(&cancel));
        assert_eq!(
            registry
                .apply_approved_partial_stage(&selection, &cancel)
                .unwrap_err(),
            "G06B_STAGE_NOT_APPROVED_OR_CANCELLED"
        );
        assert_eq!(fs::read(root.join(".git/index")).unwrap(), before);
        assert!(!root.join(".git/index.lock").exists());
    }

    #[test]
    fn preexisting_lock_and_stale_selection_refuse_without_clobber() {
        let (_temp, registry, id, root) = fixture();
        let selection = reviewed(&registry, &id, &root);
        fs::write(root.join(".git/index.lock"), b"another Git writer").unwrap();
        let before = fs::read(root.join(".git/index")).unwrap();
        assert_eq!(
            registry
                .apply_approved_partial_stage(&selection, &AtomicU8::new(0))
                .unwrap_err(),
            "G06B_STAGE_FOREIGN_OR_STALE_INDEX_LOCK"
        );
        assert_eq!(
            fs::read(root.join(".git/index.lock")).unwrap(),
            b"another Git writer"
        );
        assert_eq!(fs::read(root.join(".git/index")).unwrap(), before);
        fs::remove_file(root.join(".git/index.lock")).unwrap();
        fs::write(root.join("tracked.txt"), b"changed after review\n").unwrap();
        assert!(registry
            .apply_approved_partial_stage(&selection, &AtomicU8::new(0))
            .is_err());
        assert_eq!(fs::read(root.join(".git/index")).unwrap(), before);
    }

    #[test]
    fn restart_reconciles_unpublished_prepared_journal_without_git_writes() {
        let (_temp, registry, id, root) = fixture();
        let journal = journal_path(&registry, &id).unwrap();
        let original = fs::read(root.join(".git/index")).unwrap();
        let head = Repository::open(&root)
            .unwrap()
            .head()
            .unwrap()
            .target()
            .unwrap();
        journal_append(
            &journal,
            &JournalLine {
                schema_version: 1,
                operation_id: "restart-fixture".into(),
                project_id: id.clone(),
                relative_path: "tracked.txt".into(),
                phase: "prepared".into(),
                head_sha: head.to_string(),
                selection_sha256: "a".repeat(64),
                previous_index_sha256: sha(&original),
                intended_index_sha256: "b".repeat(64),
                intended_blob_oid: "c".repeat(40),
                intended_blob_sha256: "d".repeat(64),
                source_sha256: sha(&fs::read(root.join("tracked.txt")).unwrap()),
            },
        )
        .unwrap();
        drop(registry);
        let reopened = IdeWorkspaceRegistry::open(_temp.0.join("catalog.json")).unwrap();
        reopened.reopen(&id).unwrap();
        let status = reopened.inspect_partial_stage_recovery(&id).unwrap();
        assert!(status.operator_reconciliation_required);
        assert_eq!(status.state, "index_not_published_or_external_change");
        let verified = reopened.reconcile_partial_stage_journal(&id).unwrap();
        assert_eq!(verified.state, "cancelled_index_unchanged");
        assert!(!verified.operator_reconciliation_required);
        assert_eq!(fs::read(root.join(".git/index")).unwrap(), original);
        assert!(!root.join(".git/index.lock").exists());
    }
    #[test]
    fn completed_receipt_stays_terminal_after_later_external_git_changes() {
        let (_tmp, registry, id, root) = fixture();
        let selection = reviewed(&registry, &id, &root);
        registry
            .apply_approved_partial_stage(&selection, &AtomicU8::new(0))
            .unwrap();
        let repo = Repository::open(&root).unwrap();
        let mut index = repo.index().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let head = repo.head().unwrap().peel_to_commit().unwrap();
        let sig = Signature::now("Fixture", "fixture@localhost.invalid").unwrap();
        repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            "later authorized commit",
            &tree,
            &[&head],
        )
        .unwrap();
        fs::write(
            root.join("tracked.txt"),
            b"later independently staged bytes\n",
        )
        .unwrap();
        index.add_path(Path::new("tracked.txt")).unwrap();
        index.write().unwrap();
        drop(head);
        drop(tree);
        drop(index);
        drop(repo);
        let status = registry.inspect_partial_stage_recovery(&id).unwrap();
        assert_eq!(status.state, "verified_receipt_followed_by_git_changes");
        assert!(!status.operator_reconciliation_required);
    }
    #[test]
    fn interrupted_journal_is_detected_and_never_auto_repaired() {
        let (_temp, registry, id, root) = fixture();
        let path = journal_path(&registry, &id).unwrap();
        let bytes = fs::read(root.join(".git/index")).unwrap();
        let head = Repository::open(&root)
            .unwrap()
            .head()
            .unwrap()
            .target()
            .unwrap();
        journal_append(
            &path,
            &JournalLine {
                schema_version: 1,
                operation_id: "fixture".into(),
                project_id: id.clone(),
                relative_path: "tracked.txt".into(),
                phase: "publishing".into(),
                head_sha: head.to_string(),
                selection_sha256: "a".repeat(64),
                previous_index_sha256: sha(&bytes),
                intended_index_sha256: "b".repeat(64),
                intended_blob_oid: "c".repeat(40),
                intended_blob_sha256: "d".repeat(64),
                source_sha256: "e".repeat(64),
            },
        )
        .unwrap();
        fs::write(root.join(".git/index.lock"), b"ambiguous").unwrap();
        let recovery = registry.inspect_partial_stage_recovery(&id).unwrap();
        assert!(recovery.operator_reconciliation_required);
        assert!(recovery.pending_git_lock);
        assert!(recovery.index_matches_original);
        assert!(!recovery.auto_delete_git_lock && !recovery.auto_rollback_index);
    }
}
