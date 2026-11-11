//! G06B-04: independently reviewed local commit and explicit history rewrite.
//! Only an exact, journal-verified 6B-03 selected-hunk index may be committed.
//! The trusted desktop host must separately obtain native one-use OS consent.
use crate::{git_partial_stage::PartialStageReceipt, IdeWorkspaceRegistry};
use git2::{Oid, Repository, RepositoryState, Signature};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU8, Ordering},
};

const MAX_COMMIT_MESSAGE: usize = 4096;
const MAX_PERSON: usize = 160;
const MAX_JOURNAL: u64 = 256 * 1024;
const MAX_INDEX_BYTES: u64 = 8 * 1024 * 1024;

fn sha(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }

fn fingerprint(parts: &[&str]) -> String {
    let mut h = Sha256::new();
    h.update(b"lu-ide-g06b04-reviewed-commit-v1\n");
    for part in parts {
        h.update((part.len() as u64).to_le_bytes());
        h.update(part.as_bytes());
    }
    format!("{:x}", h.finalize())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewedCommitMode { Create, Amend }

impl ReviewedCommitMode {
    fn label(&self) -> &'static str {
        match self { Self::Create => "create", Self::Amend => "amend" }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewedCommitPlan {
    pub schema_version: u32,
    pub mode: ReviewedCommitMode,
    pub stage: PartialStageReceipt,
    pub branch_ref: String,
    pub previous_head: String,
    pub resulting_parent: String,
    pub index_sha256: String,
    pub staged_tree_oid: String,
    pub message: String,
    pub author_name: String,
    pub author_email: String,
    pub plan_sha256: String,
    pub stage_granted: bool,
    pub commit_granted: bool,
    pub amend_granted: bool,
    pub published: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewedCommitReceipt {
    pub schema_version: u32,
    pub project_id: String,
    pub relative_path: String,
    pub mode: ReviewedCommitMode,
    pub branch_ref: String,
    pub previous_head: String,
    pub resulting_parent: String,
    pub commit_oid: String,
    pub committed_tree_oid: String,
    pub index_sha256: String,
    pub plan_sha256: String,
    pub approval_one_use: bool,
    pub branch_ref_independently_verified: bool,
    pub staged_tree_independently_verified: bool,
    pub original_source_preserved: bool,
    pub amended_existing_history: bool,
    pub commit_created: bool,
    pub git_hooks_executed: bool,
    pub remote_updated: bool,
    pub release_approved: bool,
    pub journal_verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CommitJournal {
    schema_version: u32,
    operation_id: String,
    project_id: String,
    phase: String,
    mode: ReviewedCommitMode,
    branch_ref: String,
    old_head: String,
    commit_oid: String,
    parent_oid: String,
    tree_oid: String,
    index_sha256: String,
    plan_sha256: String,
    source_sha256: String,
    relative_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewedCommitRecovery {
    pub schema_version: u32,
    pub project_id: String,
    pub state: &'static str,
    pub last_phase: String,
    pub branch_matches_previous: bool,
    pub branch_matches_intended: bool,
    pub git_index_lock_present: bool,
    pub operator_reconciliation_required: bool,
    pub automatic_ref_rewrite: bool,
    pub automatic_retry: bool,
    pub network_queried: bool,
}

fn safe_repo(registry: &IdeWorkspaceRegistry, project_id: &str)
    -> Result<(Repository, PathBuf), &'static str>
{
    let root = registry.registered_git_root(project_id)
        .map_err(|_| "G06B_COMMIT_PROJECT_NOT_REGISTERED")?;
    let repo = Repository::open(&root).map_err(|_| "G06B_COMMIT_REPO_UNAVAILABLE")?;
    if repo.is_bare() || repo.is_worktree() || repo.head_detached().unwrap_or(true)
        || repo.state() != RepositoryState::Clean
    { return Err("G06B_COMMIT_UNSAFE_REPO_STATE"); }
    if fs::canonicalize(repo.workdir().ok_or("G06B_COMMIT_NO_WORKDIR")?)
        .map_err(|_| "G06B_COMMIT_ROOT_UNVERIFIABLE")? != root
    { return Err("G06B_COMMIT_WORKDIR_CHANGED"); }
    let gitdir = root.join(".git");
    let meta = fs::symlink_metadata(&gitdir).map_err(|_| "G06B_COMMIT_GITDIR_UNAVAILABLE")?;
    if !meta.is_dir() || meta.file_type().is_symlink()
        || fs::canonicalize(&gitdir).map_err(|_| "G06B_COMMIT_GITDIR_UNVERIFIABLE")?
            != fs::canonicalize(repo.path()).map_err(|_| "G06B_COMMIT_GITDIR_UNVERIFIABLE")?
    { return Err("G06B_COMMIT_GITDIR_EXTERNAL_OR_LINKED"); }
    Ok((repo, gitdir))
}

fn index_sha(gitdir: &Path) -> Result<String, &'static str> {
    let path = gitdir.join("index");
    let meta = fs::symlink_metadata(&path).map_err(|_| "G06B_COMMIT_INDEX_UNAVAILABLE")?;
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > MAX_INDEX_BYTES {
        return Err("G06B_COMMIT_INDEX_UNTRUSTED");
    }
    Ok(sha(&fs::read(path).map_err(|_| "G06B_COMMIT_INDEX_UNREADABLE")?))
}

fn branch(repo: &Repository) -> Result<(String, String), &'static str> {
    let head = repo.head().map_err(|_| "G06B_COMMIT_HEAD_ABSENT")?;
    let name = head.name().ok_or("G06B_COMMIT_HEAD_NAME_UNAVAILABLE")?;
    if !name.starts_with("refs/heads/") {
        return Err("G06B_COMMIT_NONLOCAL_REF_DENIED");
    }
    let oid = head.target().ok_or("G06B_COMMIT_HEAD_UNRESOLVED")?.to_string();
    Ok((name.to_owned(), oid))
}

fn journal_path(registry: &IdeWorkspaceRegistry, project: &str) -> Result<PathBuf, &'static str> {
    registry.registered_git_root(project)
        .map_err(|_| "G06B_COMMIT_UNKNOWN_PROJECT")?;
    if project.is_empty() || project.len() > 128
        || !project.bytes().all(|x| x.is_ascii_alphanumeric() || x == b'-' || x == b'_') {
        return Err("G06B_COMMIT_INVALID_PROJECT_ID");
    }
    let root = registry.catalog.parent().ok_or("G06B_COMMIT_NO_APPDATA")?
        .join("ide-g06b-reviewed-commits");
    fs::create_dir_all(&root).map_err(|_| "G06B_COMMIT_JOURNAL_ROOT_UNAVAILABLE")?;
    let meta = fs::symlink_metadata(&root).map_err(|_| "G06B_COMMIT_JOURNAL_ROOT_UNAVAILABLE")?;
    if !meta.is_dir() || meta.file_type().is_symlink() {
        return Err("G06B_COMMIT_JOURNAL_ROOT_UNTRUSTED");
    }
    Ok(root.join(format!("{project}.jsonl")))
}

fn last_journal(path: &Path) -> Result<Option<CommitJournal>, &'static str> {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("G06B_COMMIT_JOURNAL_UNREADABLE"),
    };
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > MAX_JOURNAL {
        return Err("G06B_COMMIT_JOURNAL_UNTRUSTED");
    }
    let bytes = fs::read(path).map_err(|_| "G06B_COMMIT_JOURNAL_UNREADABLE")?;
    if bytes.last() != Some(&b'\n') { return Err("G06B_COMMIT_JOURNAL_INCOMPLETE"); }
    let line = bytes.split(|b| *b == b'\n').filter(|v| !v.is_empty())
        .last().ok_or("G06B_COMMIT_JOURNAL_INCOMPLETE")?;
    let journal: CommitJournal = serde_json::from_slice(line)
        .map_err(|_| "G06B_COMMIT_JOURNAL_INVALID")?;
    if journal.schema_version != 1 { return Err("G06B_COMMIT_JOURNAL_VERSION"); }
    Ok(Some(journal))
}

fn append_journal(path: &Path, entry: &CommitJournal) -> Result<(), &'static str> {
    let previous = match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_file() && !meta.file_type().is_symlink()
            && meta.len() <= MAX_JOURNAL - 4096 => true,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => false,
        _ => return Err("G06B_COMMIT_JOURNAL_UNTRUSTED"),
    };
    let bytes = serde_json::to_vec(entry).map_err(|_| "G06B_COMMIT_JOURNAL_SERIALIZE")?;
    if bytes.len() > 2048 { return Err("G06B_COMMIT_JOURNAL_RECORD_OVERSIZED"); }
    let mut output = OpenOptions::new().append(true).create(true).open(path)
        .map_err(|_| "G06B_COMMIT_JOURNAL_WRITE_DENIED")?;
    if previous && output.metadata().map_err(|_| "G06B_COMMIT_JOURNAL_UNAVAILABLE")?.len() > MAX_JOURNAL - 4096 {
        return Err("G06B_COMMIT_JOURNAL_FULL");
    }
    output.write_all(&bytes).and_then(|_| output.write_all(b"\n"))
        .and_then(|_| output.sync_all())
        .map_err(|_| "G06B_COMMIT_JOURNAL_SYNC_FAILED")?;
    #[cfg(unix)]
    File::open(path.parent().ok_or("G06B_COMMIT_JOURNAL_PARENT_MISSING")?)
        .and_then(|dir| dir.sync_all())
        .map_err(|_| "G06B_COMMIT_JOURNAL_DIR_SYNC_FAILED")?;
    Ok(())
}

fn validate_message(message: &str, author: &str, email: &str) -> Result<(), &'static str> {
    if message.trim().is_empty() || message.len() > MAX_COMMIT_MESSAGE
        || author.trim().is_empty() || author.len() > MAX_PERSON
        || email.trim().is_empty() || email.len() > MAX_PERSON
        || !email.contains('@')
        || [message, author, email].iter().any(|s| s.chars().any(char::is_control))
    {
        return Err("G06B_COMMIT_AUTHOR_OR_MESSAGE_INVALID");
    }
    Ok(())
}

struct RemoveOwnLock(PathBuf);
impl Drop for RemoveOwnLock {
    fn drop(&mut self) { let _ = fs::remove_file(&self.0); }
}

impl IdeWorkspaceRegistry {
    pub fn preview_reviewed_partial_commit(
        &self, stage: &PartialStageReceipt,
        mode: ReviewedCommitMode,
        message: &str,
        author_name: &str,
        author_email: &str,
    ) -> Result<ReviewedCommitPlan, &'static str> {
        validate_message(message, author_name, author_email)?;
        self.verify_partial_stage_for_commit(stage)?;
        let (repo, gitdir) = safe_repo(self, &stage.project_id)?;
        if fs::symlink_metadata(gitdir.join("index.lock")).is_ok() {
            return Err("G06B_COMMIT_INDEX_LOCKED");
        }
        let (branch_ref, head) = branch(&repo)?;
        if head != stage.previous_head { return Err("G06B_COMMIT_HEAD_STALE"); }
        let index_digest = index_sha(&gitdir)?;
        if index_digest != stage.new_index_sha256 {
            return Err("G06B_COMMIT_INDEX_STALE");
        }
        let mut index = repo.index().map_err(|_| "G06B_COMMIT_INDEX_UNAVAILABLE")?;
        index.read(true).map_err(|_| "G06B_COMMIT_INDEX_RELOAD_FAILED")?;
        if index.has_conflicts() { return Err("G06B_COMMIT_CONFLICTS"); }
        let tree_oid = index.write_tree_to(&repo)
            .map_err(|_| "G06B_COMMIT_TREE_UNAVAILABLE")?;
        let parent = repo.find_commit(Oid::from_str(&head).map_err(|_| "G06B_COMMIT_BAD_HEAD")?)
            .map_err(|_| "G06B_COMMIT_PARENT_UNAVAILABLE")?;
        let resulting_parent = match mode {
            ReviewedCommitMode::Create => head.clone(),
            ReviewedCommitMode::Amend => {
                // No hidden merge, root, or multi-parent history rewrites.
                if parent.parent_count() != 1 { return Err("G06B_AMEND_ROOT_OR_MERGE_DENIED"); }
                // A locally configured upstream means publisher visibility
                // cannot be verified offline; refuse rather than silently
                // rewrite history that may already be shared.
                let local = branch_ref.strip_prefix("refs/heads/")
                    .ok_or("G06B_AMEND_BRANCH_UNVERIFIABLE")?;
                let branch = repo.find_branch(local, git2::BranchType::Local)
                    .map_err(|_| "G06B_AMEND_BRANCH_UNAVAILABLE")?;
                if branch.upstream().is_ok() {
                    return Err("G06B_AMEND_UPSTREAM_UNVERIFIABLE");
                }
                parent.parent_id(0).map_err(|_| "G06B_AMEND_PARENT_UNAVAILABLE")?.to_string()
            },
        };
        let plan_digest = fingerprint(&[
            mode.label(), &stage.project_id, &stage.relative_path,
            &stage.selection_sha256, &stage.new_index_sha256, &stage.staged_blob_oid,
            &stage.staged_blob_sha256, &branch_ref, &head, &resulting_parent,
            &index_digest, &tree_oid.to_string(), message, author_name, author_email,
        ]);
        Ok(ReviewedCommitPlan {
            schema_version: 1, mode, stage: stage.clone(),
            branch_ref, previous_head: head, resulting_parent,
            index_sha256: index_digest,
            staged_tree_oid: tree_oid.to_string(),
            message: message.to_owned(), author_name: author_name.to_owned(),
            author_email: author_email.to_owned(), plan_sha256: plan_digest,
            stage_granted: false, commit_granted: false, amend_granted: false, published: false,
        })
    }

    pub fn execute_approved_reviewed_commit(
        &self, expected: &ReviewedCommitPlan, cancel: &AtomicU8,
    ) -> Result<ReviewedCommitReceipt, &'static str> {
        if cancel.load(Ordering::SeqCst) != 0 || expected.schema_version != 1
            || expected.stage_granted || expected.commit_granted
            || expected.amend_granted || expected.published
        { return Err("G06B_COMMIT_INVALID_OR_CANCELLED_PLAN"); }
        let fresh = self.preview_reviewed_partial_commit(
            &expected.stage, expected.mode.clone(),
            &expected.message, &expected.author_name, &expected.author_email)?;
        if &fresh != expected { return Err("G06B_COMMIT_STALE_OR_FORGED_PLAN"); }
        let (repo, gitdir) = safe_repo(self, &fresh.stage.project_id)?;
        let journal_file = journal_path(self, &fresh.stage.project_id)?;
        if let Some(last) = last_journal(&journal_file)? {
            if !matches!(last.phase.as_str(), "verified" | "cancelled") {
                return Err("G06B_COMMIT_RECOVERY_REQUIRED");
            }
        }
        let lock_path = gitdir.join("index.lock");
        let lock = OpenOptions::new().write(true).create_new(true).open(&lock_path)
            .map_err(|_| "G06B_COMMIT_CONCURRENT_INDEX_LOCK")?;
        let _lock_guard = RemoveOwnLock(lock_path);
        // An index writer that uses Git's index lock cannot now alter
        // the independently verified staged bytes or staged tree.
        if index_sha(&gitdir)? != fresh.index_sha256 || branch(&repo)? !=
            (fresh.branch_ref.clone(), fresh.previous_head.clone())
        { return Err("G06B_COMMIT_CONCURRENT_INDEX_OR_HEAD_CHANGE"); }
        let mut index = repo.index().map_err(|_| "G06B_COMMIT_INDEX_UNAVAILABLE")?;
        index.read(true).map_err(|_| "G06B_COMMIT_INDEX_RELOAD_FAILED")?;
        if index.has_conflicts() || index.write_tree_to(&repo)
            .map_err(|_| "G06B_COMMIT_TREE_UNAVAILABLE")?.to_string() != fresh.staged_tree_oid
        { return Err("G06B_COMMIT_INDEX_TREE_CHANGED"); }
        let tree = repo.find_tree(
            Oid::from_str(&fresh.staged_tree_oid).map_err(|_| "G06B_COMMIT_BAD_TREE")?)
            .map_err(|_| "G06B_COMMIT_TREE_MISSING")?;
        let parent = repo.find_commit(
            Oid::from_str(&fresh.resulting_parent).map_err(|_| "G06B_COMMIT_BAD_PARENT")?)
            .map_err(|_| "G06B_COMMIT_PARENT_MISSING")?;
        let sig = Signature::now(&fresh.author_name, &fresh.author_email)
            .map_err(|_| "G06B_COMMIT_AUTHOR_INVALID")?;
        let operation_id = format!("{:032x}", rand::random::<u128>());
        let mut journal = CommitJournal {
            schema_version: 1, operation_id, project_id: fresh.stage.project_id.clone(),
            phase: "prepared".to_owned(), mode: fresh.mode.clone(),
            branch_ref: fresh.branch_ref.clone(), old_head: fresh.previous_head.clone(),
            commit_oid: String::new(), parent_oid: fresh.resulting_parent.clone(),
            tree_oid: fresh.staged_tree_oid.clone(), index_sha256: fresh.index_sha256.clone(),
            plan_sha256: fresh.plan_sha256.clone(),
            source_sha256: fresh.stage.source_sha256.clone(),
            relative_path: fresh.stage.relative_path.clone(),
        };
        if cancel.load(Ordering::SeqCst) != 0 {
            return Err("G06B_COMMIT_CANCELLED_BEFORE_JOURNAL");
        }
        append_journal(&journal_file, &journal)?;
        let oid = repo.commit(None, &sig, &sig, &fresh.message, &tree, &[&parent])
            .map_err(|_| "G06B_COMMIT_OBJECT_CREATION_FAILED")?;
        journal.commit_oid = oid.to_string();
        journal.phase = "object_written".to_owned();
        append_journal(&journal_file, &journal)?;
        // Cancellation is allowed through object creation: an unreferenced
        // object is not a published commit, and no branch ref is changed.
        if cancel.load(Ordering::SeqCst) != 0 {
            journal.phase = "cancelled".to_owned();
            append_journal(&journal_file, &journal)?;
            return Err("G06B_COMMIT_CANCELLED_BEFORE_PUBLICATION");
        }
        // Prepublish sync journal, then host-owned atomic cancellation fence
        // prevents an observer from mistaking an already published ref
        // for a reversible operation.
        journal.phase = "publishing".to_owned();
        append_journal(&journal_file, &journal)?;
        if cancel.compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst).is_err() {
            journal.phase = "cancelled".to_owned();
            append_journal(&journal_file, &journal)?;
            return Err("G06B_COMMIT_CANCELLED_BEFORE_REF_UPDATE");
        }
        if index_sha(&gitdir)? != fresh.index_sha256 {
            return Err("G06B_COMMIT_INDEX_CHANGED_BEFORE_CAS");
        }
        if sha(&self.reread_reviewed_git_source_bytes(
            &fresh.stage.project_id, &fresh.stage.relative_path)?)
            != fresh.stage.source_sha256
        {
            return Err("G06B_COMMIT_SOURCE_CHANGED_BEFORE_CAS");
        }
        repo.reference_matching(
            &fresh.branch_ref, oid, true,
            Oid::from_str(&fresh.previous_head).map_err(|_| "G06B_COMMIT_BAD_HEAD")?,
            if fresh.mode == ReviewedCommitMode::Amend {
                "Lu IDE explicitly authorized local amendment"
            } else { "Lu IDE explicitly authorized reviewed local commit" },
        ).map_err(|_| "G06B_COMMIT_CONCURRENT_REF_UPDATE_DENIED")?;
        journal.phase = "ref_published".into();
        append_journal(&journal_file, &journal)?;
        if branch(&repo)? != (fresh.branch_ref.clone(), oid.to_string()) {
            return Err("G06B_COMMIT_PUBLISHED_REF_UNVERIFIED");
        }
        if index_sha(&gitdir)? != fresh.index_sha256 {
            return Err("G06B_COMMIT_PUBLISHED_INDEX_CHANGED");
        }
        let published = repo.find_commit(oid)
            .map_err(|_| "G06B_COMMIT_PUBLISHED_OBJECT_MISSING")?;
        if published.tree_id().to_string() != fresh.staged_tree_oid
            || published.parent_count() != 1
            || published.parent_id(0).map_err(|_| "G06B_COMMIT_PUBLISHED_PARENT_MISSING")?
                .to_string() != fresh.resulting_parent
        { return Err("G06B_COMMIT_PUBLISHED_TREE_OR_PARENT_MISMATCH"); }
        let source_after = self.reread_reviewed_git_source_bytes(
            &fresh.stage.project_id, &fresh.stage.relative_path)?;
        if sha(&source_after) != fresh.stage.source_sha256 {
            return Err("G06B_COMMIT_SOURCE_CHANGED_AFTER_REF_UPDATE");
        }
        journal.phase = "verified".into();
        append_journal(&journal_file, &journal)?;
        Ok(ReviewedCommitReceipt {
            schema_version: 1, project_id: fresh.stage.project_id.clone(),
            relative_path: fresh.stage.relative_path.clone(), mode: fresh.mode.clone(),
            branch_ref: fresh.branch_ref, previous_head: fresh.previous_head,
            resulting_parent: fresh.resulting_parent, commit_oid: oid.to_string(),
            committed_tree_oid: fresh.staged_tree_oid,
            index_sha256: fresh.index_sha256, plan_sha256: fresh.plan_sha256,
            approval_one_use: true, branch_ref_independently_verified: true,
            staged_tree_independently_verified: true, original_source_preserved: true,
            amended_existing_history: fresh.mode == ReviewedCommitMode::Amend,
            commit_created: true, git_hooks_executed: false, remote_updated: false,
            release_approved: false, journal_verified: true,
        })
    }

    pub fn inspect_reviewed_commit_recovery(
        &self, project_id: &str,
    ) -> Result<ReviewedCommitRecovery, &'static str> {
        let (repo, gitdir) = safe_repo(self, project_id)?;
        let last = last_journal(&journal_path(self, project_id)?)?;
        let head = branch(&repo)?.1;
        let locked = fs::symlink_metadata(gitdir.join("index.lock")).is_ok();
        let (state, phase, old, intended) = match last {
            None => ("no_journal", String::new(), false, false),
            Some(j) if j.project_id != project_id => return Err("G06B_COMMIT_JOURNAL_PROJECT_MISMATCH"),
            Some(j) => {
                let old = head == j.old_head;
                let intended = !j.commit_oid.is_empty() && head == j.commit_oid;
                let status = if j.phase == "verified" { "verified_historical_commit" }
                    else if j.phase == "cancelled" && old { "cancelled_before_ref_update" }
                    else if intended { "ref_published_requires_verification" }
                    else if old { "ref_not_published_or_external_change" }
                    else { "ref_drift_or_unverifiable" };
                (status, j.phase, old, intended)
            }
        };
        Ok(ReviewedCommitRecovery {
            schema_version: 1, project_id: project_id.to_owned(), state,
            last_phase: phase, branch_matches_previous: old,
            branch_matches_intended: intended, git_index_lock_present: locked,
            operator_reconciliation_required: locked || !matches!(state,
                "no_journal" | "verified_historical_commit" | "cancelled_before_ref_update"),
            automatic_ref_rewrite: false, automatic_retry: false, network_queried: false,
        })
    }
}
