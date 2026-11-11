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

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

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
pub enum ReviewedCommitMode {
    Create,
    Amend,
}

impl ReviewedCommitMode {
    fn label(&self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Amend => "amend",
        }
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

fn safe_repo(
    registry: &IdeWorkspaceRegistry,
    project_id: &str,
) -> Result<(Repository, PathBuf), &'static str> {
    let root = registry
        .registered_git_root(project_id)
        .map_err(|_| "G06B_COMMIT_PROJECT_NOT_REGISTERED")?;
    let repo = Repository::open(&root).map_err(|_| "G06B_COMMIT_REPO_UNAVAILABLE")?;
    if repo.is_bare()
        || repo.is_worktree()
        || repo.head_detached().unwrap_or(true)
        || repo.state() != RepositoryState::Clean
    {
        return Err("G06B_COMMIT_UNSAFE_REPO_STATE");
    }
    if fs::canonicalize(repo.workdir().ok_or("G06B_COMMIT_NO_WORKDIR")?)
        .map_err(|_| "G06B_COMMIT_ROOT_UNVERIFIABLE")?
        != root
    {
        return Err("G06B_COMMIT_WORKDIR_CHANGED");
    }
    let gitdir = root.join(".git");
    let meta = fs::symlink_metadata(&gitdir).map_err(|_| "G06B_COMMIT_GITDIR_UNAVAILABLE")?;
    if !meta.is_dir()
        || meta.file_type().is_symlink()
        || fs::canonicalize(&gitdir).map_err(|_| "G06B_COMMIT_GITDIR_UNVERIFIABLE")?
            != fs::canonicalize(repo.path()).map_err(|_| "G06B_COMMIT_GITDIR_UNVERIFIABLE")?
    {
        return Err("G06B_COMMIT_GITDIR_EXTERNAL_OR_LINKED");
    }
    Ok((repo, gitdir))
}

fn index_sha(gitdir: &Path) -> Result<String, &'static str> {
    let path = gitdir.join("index");
    let meta = fs::symlink_metadata(&path).map_err(|_| "G06B_COMMIT_INDEX_UNAVAILABLE")?;
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > MAX_INDEX_BYTES {
        return Err("G06B_COMMIT_INDEX_UNTRUSTED");
    }
    Ok(sha(
        &fs::read(path).map_err(|_| "G06B_COMMIT_INDEX_UNREADABLE")?
    ))
}

fn branch(repo: &Repository) -> Result<(String, String), &'static str> {
    let head = repo.head().map_err(|_| "G06B_COMMIT_HEAD_ABSENT")?;
    let name = head.name().ok_or("G06B_COMMIT_HEAD_NAME_UNAVAILABLE")?;
    if !name.starts_with("refs/heads/") {
        return Err("G06B_COMMIT_NONLOCAL_REF_DENIED");
    }
    let oid = head
        .target()
        .ok_or("G06B_COMMIT_HEAD_UNRESOLVED")?
        .to_string();
    Ok((name.to_owned(), oid))
}

fn journal_path(registry: &IdeWorkspaceRegistry, project: &str) -> Result<PathBuf, &'static str> {
    registry
        .registered_git_root(project)
        .map_err(|_| "G06B_COMMIT_UNKNOWN_PROJECT")?;
    if project.is_empty()
        || project.len() > 128
        || !project
            .bytes()
            .all(|x| x.is_ascii_alphanumeric() || x == b'-' || x == b'_')
    {
        return Err("G06B_COMMIT_INVALID_PROJECT_ID");
    }
    let root = registry
        .catalog
        .parent()
        .ok_or("G06B_COMMIT_NO_APPDATA")?
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
    if bytes.last() != Some(&b'\n') {
        return Err("G06B_COMMIT_JOURNAL_INCOMPLETE");
    }
    let line = bytes
        .split(|b| *b == b'\n')
        .filter(|v| !v.is_empty())
        .last()
        .ok_or("G06B_COMMIT_JOURNAL_INCOMPLETE")?;
    let journal: CommitJournal =
        serde_json::from_slice(line).map_err(|_| "G06B_COMMIT_JOURNAL_INVALID")?;
    if journal.schema_version != 1 {
        return Err("G06B_COMMIT_JOURNAL_VERSION");
    }
    Ok(Some(journal))
}

fn append_journal(path: &Path, entry: &CommitJournal) -> Result<(), &'static str> {
    let previous = match fs::symlink_metadata(path) {
        Ok(meta)
            if meta.is_file()
                && !meta.file_type().is_symlink()
                && meta.len() <= MAX_JOURNAL - 4096 =>
        {
            true
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => false,
        _ => return Err("G06B_COMMIT_JOURNAL_UNTRUSTED"),
    };
    let bytes = serde_json::to_vec(entry).map_err(|_| "G06B_COMMIT_JOURNAL_SERIALIZE")?;
    if bytes.len() > 2048 {
        return Err("G06B_COMMIT_JOURNAL_RECORD_OVERSIZED");
    }
    let mut output = OpenOptions::new()
        .append(true)
        .create(true)
        .open(path)
        .map_err(|_| "G06B_COMMIT_JOURNAL_WRITE_DENIED")?;
    if previous
        && output
            .metadata()
            .map_err(|_| "G06B_COMMIT_JOURNAL_UNAVAILABLE")?
            .len()
            > MAX_JOURNAL - 4096
    {
        return Err("G06B_COMMIT_JOURNAL_FULL");
    }
    output
        .write_all(&bytes)
        .and_then(|_| output.write_all(b"\n"))
        .and_then(|_| output.sync_all())
        .map_err(|_| "G06B_COMMIT_JOURNAL_SYNC_FAILED")?;
    #[cfg(unix)]
    File::open(path.parent().ok_or("G06B_COMMIT_JOURNAL_PARENT_MISSING")?)
        .and_then(|dir| dir.sync_all())
        .map_err(|_| "G06B_COMMIT_JOURNAL_DIR_SYNC_FAILED")?;
    Ok(())
}

fn validate_message(message: &str, author: &str, email: &str) -> Result<(), &'static str> {
    if message.trim().is_empty()
        || message.len() > MAX_COMMIT_MESSAGE
        || author.trim().is_empty()
        || author.len() > MAX_PERSON
        || email.trim().is_empty()
        || email.len() > MAX_PERSON
        || !email.contains('@')
        || message.chars().any(|c| c.is_control() && c != '\n')
        || [author, email]
            .iter()
            .any(|s| s.chars().any(char::is_control))
    {
        return Err("G06B_COMMIT_AUTHOR_OR_MESSAGE_INVALID");
    }
    Ok(())
}

struct RemoveOwnLock(PathBuf);
impl Drop for RemoveOwnLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

impl IdeWorkspaceRegistry {
    pub fn preview_reviewed_partial_commit(
        &self,
        stage: &PartialStageReceipt,
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
        if head != stage.previous_head {
            return Err("G06B_COMMIT_HEAD_STALE");
        }
        let index_digest = index_sha(&gitdir)?;
        if index_digest != stage.new_index_sha256 {
            return Err("G06B_COMMIT_INDEX_STALE");
        }
        let mut index = repo.index().map_err(|_| "G06B_COMMIT_INDEX_UNAVAILABLE")?;
        index
            .read(true)
            .map_err(|_| "G06B_COMMIT_INDEX_RELOAD_FAILED")?;
        if index.has_conflicts() {
            return Err("G06B_COMMIT_CONFLICTS");
        }
        // Planning is read-only: even writing an unreferenced Git tree
        // object is deferred until native OS approval and locked execution.
        let staged_tree = "deferred_until_native_os_approval";
        let parent = repo
            .find_commit(Oid::from_str(&head).map_err(|_| "G06B_COMMIT_BAD_HEAD")?)
            .map_err(|_| "G06B_COMMIT_PARENT_UNAVAILABLE")?;
        let resulting_parent = match mode {
            ReviewedCommitMode::Create => head.clone(),
            ReviewedCommitMode::Amend => {
                // No hidden merge, root, or multi-parent history rewrites.
                if parent.parent_count() != 1 {
                    return Err("G06B_AMEND_ROOT_OR_MERGE_DENIED");
                }
                // A locally configured upstream means publisher visibility
                // cannot be verified offline; refuse rather than silently
                // rewrite history that may already be shared.
                let local = branch_ref
                    .strip_prefix("refs/heads/")
                    .ok_or("G06B_AMEND_BRANCH_UNVERIFIABLE")?;
                let branch = repo
                    .find_branch(local, git2::BranchType::Local)
                    .map_err(|_| "G06B_AMEND_BRANCH_UNAVAILABLE")?;
                if branch.upstream().is_ok() {
                    return Err("G06B_AMEND_UPSTREAM_UNVERIFIABLE");
                }
                parent
                    .parent_id(0)
                    .map_err(|_| "G06B_AMEND_PARENT_UNAVAILABLE")?
                    .to_string()
            }
        };
        let plan_digest = fingerprint(&[
            mode.label(),
            &stage.project_id,
            &stage.relative_path,
            &stage.selection_sha256,
            &stage.new_index_sha256,
            &stage.staged_blob_oid,
            &stage.staged_blob_sha256,
            &branch_ref,
            &head,
            &resulting_parent,
            &index_digest,
            staged_tree,
            message,
            author_name,
            author_email,
        ]);
        Ok(ReviewedCommitPlan {
            schema_version: 1,
            mode,
            stage: stage.clone(),
            branch_ref,
            previous_head: head,
            resulting_parent,
            index_sha256: index_digest,
            staged_tree_oid: staged_tree.to_owned(),
            message: message.to_owned(),
            author_name: author_name.to_owned(),
            author_email: author_email.to_owned(),
            plan_sha256: plan_digest,
            stage_granted: false,
            commit_granted: false,
            amend_granted: false,
            published: false,
        })
    }

    pub fn execute_approved_reviewed_commit(
        &self,
        expected: &ReviewedCommitPlan,
        cancel: &AtomicU8,
    ) -> Result<ReviewedCommitReceipt, &'static str> {
        if cancel.load(Ordering::SeqCst) != 0
            || expected.schema_version != 1
            || expected.stage_granted
            || expected.commit_granted
            || expected.amend_granted
            || expected.published
        {
            return Err("G06B_COMMIT_INVALID_OR_CANCELLED_PLAN");
        }
        let fresh = self.preview_reviewed_partial_commit(
            &expected.stage,
            expected.mode.clone(),
            &expected.message,
            &expected.author_name,
            &expected.author_email,
        )?;
        if &fresh != expected {
            return Err("G06B_COMMIT_STALE_OR_FORGED_PLAN");
        }
        let (repo, gitdir) = safe_repo(self, &fresh.stage.project_id)?;
        let journal_file = journal_path(self, &fresh.stage.project_id)?;
        if let Some(last) = last_journal(&journal_file)? {
            if !matches!(last.phase.as_str(), "verified" | "cancelled") {
                return Err("G06B_COMMIT_RECOVERY_REQUIRED");
            }
        }
        let lock_path = gitdir.join("index.lock");
        let lock = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
            .map_err(|_| "G06B_COMMIT_CONCURRENT_INDEX_LOCK")?;
        let _lock_guard = RemoveOwnLock(lock_path);
        // An index writer that uses Git's index lock cannot now alter
        // the independently verified staged bytes or staged tree.
        if index_sha(&gitdir)? != fresh.index_sha256
            || branch(&repo)? != (fresh.branch_ref.clone(), fresh.previous_head.clone())
        {
            return Err("G06B_COMMIT_CONCURRENT_INDEX_OR_HEAD_CHANGE");
        }
        let mut index = repo.index().map_err(|_| "G06B_COMMIT_INDEX_UNAVAILABLE")?;
        index
            .read(true)
            .map_err(|_| "G06B_COMMIT_INDEX_RELOAD_FAILED")?;
        if index.has_conflicts() {
            return Err("G06B_COMMIT_INDEX_TREE_CHANGED");
        }
        // Only the separately OS-approved execution may materialize a
        // Git tree object. The complete on-disk index SHA and approved
        // source were pinned before the exclusive Git index lock.
        let tree_oid = index
            .write_tree_to(&repo)
            .map_err(|_| "G06B_COMMIT_TREE_UNAVAILABLE")?;
        let tree = repo
            .find_tree(tree_oid)
            .map_err(|_| "G06B_COMMIT_TREE_MISSING")?;
        let parent = repo
            .find_commit(
                Oid::from_str(&fresh.resulting_parent).map_err(|_| "G06B_COMMIT_BAD_PARENT")?,
            )
            .map_err(|_| "G06B_COMMIT_PARENT_MISSING")?;
        let sig = Signature::now(&fresh.author_name, &fresh.author_email)
            .map_err(|_| "G06B_COMMIT_AUTHOR_INVALID")?;
        let operation_id = format!("{:032x}", rand::random::<u128>());
        let mut journal = CommitJournal {
            schema_version: 1,
            operation_id,
            project_id: fresh.stage.project_id.clone(),
            phase: "prepared".to_owned(),
            mode: fresh.mode.clone(),
            branch_ref: fresh.branch_ref.clone(),
            old_head: fresh.previous_head.clone(),
            commit_oid: String::new(),
            parent_oid: fresh.resulting_parent.clone(),
            tree_oid: tree_oid.to_string(),
            index_sha256: fresh.index_sha256.clone(),
            plan_sha256: fresh.plan_sha256.clone(),
            source_sha256: fresh.stage.source_sha256.clone(),
            relative_path: fresh.stage.relative_path.clone(),
        };
        if cancel.load(Ordering::SeqCst) != 0 {
            return Err("G06B_COMMIT_CANCELLED_BEFORE_JOURNAL");
        }
        append_journal(&journal_file, &journal)?;
        let oid = repo
            .commit(None, &sig, &sig, &fresh.message, &tree, &[&parent])
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
        if cancel
            .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            journal.phase = "cancelled".to_owned();
            append_journal(&journal_file, &journal)?;
            return Err("G06B_COMMIT_CANCELLED_BEFORE_REF_UPDATE");
        }
        if index_sha(&gitdir)? != fresh.index_sha256 {
            return Err("G06B_COMMIT_INDEX_CHANGED_BEFORE_CAS");
        }
        if sha(&self.reread_reviewed_git_source_bytes(
            &fresh.stage.project_id,
            &fresh.stage.relative_path,
        )?) != fresh.stage.source_sha256
        {
            return Err("G06B_COMMIT_SOURCE_CHANGED_BEFORE_CAS");
        }
        repo.reference_matching(
            &fresh.branch_ref,
            oid,
            true,
            Oid::from_str(&fresh.previous_head).map_err(|_| "G06B_COMMIT_BAD_HEAD")?,
            if fresh.mode == ReviewedCommitMode::Amend {
                "Lu IDE explicitly authorized local amendment"
            } else {
                "Lu IDE explicitly authorized reviewed local commit"
            },
        )
        .map_err(|_| "G06B_COMMIT_CONCURRENT_REF_UPDATE_DENIED")?;
        journal.phase = "ref_published".into();
        append_journal(&journal_file, &journal)?;
        if branch(&repo)? != (fresh.branch_ref.clone(), oid.to_string()) {
            return Err("G06B_COMMIT_PUBLISHED_REF_UNVERIFIED");
        }
        if index_sha(&gitdir)? != fresh.index_sha256 {
            return Err("G06B_COMMIT_PUBLISHED_INDEX_CHANGED");
        }
        let published = repo
            .find_commit(oid)
            .map_err(|_| "G06B_COMMIT_PUBLISHED_OBJECT_MISSING")?;
        if published.tree_id() != tree_oid
            || published.parent_count() != 1
            || published
                .parent_id(0)
                .map_err(|_| "G06B_COMMIT_PUBLISHED_PARENT_MISSING")?
                .to_string()
                != fresh.resulting_parent
        {
            return Err("G06B_COMMIT_PUBLISHED_TREE_OR_PARENT_MISMATCH");
        }
        let source_after = self.reread_reviewed_git_source_bytes(
            &fresh.stage.project_id,
            &fresh.stage.relative_path,
        )?;
        if sha(&source_after) != fresh.stage.source_sha256 {
            return Err("G06B_COMMIT_SOURCE_CHANGED_AFTER_REF_UPDATE");
        }
        journal.phase = "verified".into();
        append_journal(&journal_file, &journal)?;
        Ok(ReviewedCommitReceipt {
            schema_version: 1,
            project_id: fresh.stage.project_id.clone(),
            relative_path: fresh.stage.relative_path.clone(),
            mode: fresh.mode.clone(),
            branch_ref: fresh.branch_ref,
            previous_head: fresh.previous_head,
            resulting_parent: fresh.resulting_parent,
            commit_oid: oid.to_string(),
            committed_tree_oid: tree_oid.to_string(),
            index_sha256: fresh.index_sha256,
            plan_sha256: fresh.plan_sha256,
            approval_one_use: true,
            branch_ref_independently_verified: true,
            staged_tree_independently_verified: true,
            original_source_preserved: true,
            amended_existing_history: fresh.mode == ReviewedCommitMode::Amend,
            commit_created: true,
            git_hooks_executed: false,
            remote_updated: false,
            release_approved: false,
            journal_verified: true,
        })
    }

    /// A second, separately OS-authorized action after restart. It only
    /// appends a terminal fact after independently checking the branch ref,
    /// index, commit object, parent, tree and original worktree. It never
    /// retries a commit or rewrites a ref.
    pub fn reconcile_reviewed_commit_journal(
        &self,
        project_id: &str,
    ) -> Result<ReviewedCommitRecovery, &'static str> {
        let (repo, gitdir) = safe_repo(self, project_id)?;
        let path = journal_path(self, project_id)?;
        let mut journal = last_journal(&path)?.ok_or("G06B_COMMIT_RECOVERY_NO_JOURNAL")?;
        if journal.project_id != project_id {
            return Err("G06B_COMMIT_RECOVERY_WRONG_PROJECT");
        }
        if matches!(journal.phase.as_str(), "verified" | "cancelled") {
            return self.inspect_reviewed_commit_recovery(project_id);
        }
        if fs::symlink_metadata(gitdir.join("index.lock")).is_ok() {
            return Err("G06B_COMMIT_RECOVERY_LOCK_REQUIRES_MANUAL_REVIEW");
        }
        if index_sha(&gitdir)? != journal.index_sha256 {
            return Err("G06B_COMMIT_RECOVERY_INDEX_CHANGED");
        }
        let (branch_name, actual_head) = branch(&repo)?;
        if branch_name != journal.branch_ref {
            return Err("G06B_COMMIT_RECOVERY_BRANCH_CHANGED");
        }
        if actual_head == journal.old_head {
            if !matches!(
                journal.phase.as_str(),
                "prepared" | "object_written" | "publishing"
            ) {
                return Err("G06B_COMMIT_RECOVERY_AMBIGUOUS_UNPUBLISHED");
            }
            journal.phase = "cancelled".into();
            append_journal(&path, &journal)?;
            return self.inspect_reviewed_commit_recovery(project_id);
        }
        if journal.commit_oid.is_empty()
            || actual_head != journal.commit_oid
            || !matches!(journal.phase.as_str(), "publishing" | "ref_published")
        {
            return Err("G06B_COMMIT_RECOVERY_REF_DRIFT");
        }
        let published = repo
            .find_commit(
                Oid::from_str(&journal.commit_oid)
                    .map_err(|_| "G06B_COMMIT_RECOVERY_BAD_OBJECT")?,
            )
            .map_err(|_| "G06B_COMMIT_RECOVERY_COMMIT_MISSING")?;
        if published.tree_id().to_string() != journal.tree_oid
            || published.parent_count() != 1
            || published
                .parent_id(0)
                .map_err(|_| "G06B_COMMIT_RECOVERY_PARENT_MISSING")?
                .to_string()
                != journal.parent_oid
        {
            return Err("G06B_COMMIT_RECOVERY_TREE_PARENT_MISMATCH");
        }
        if sha(&self.reread_reviewed_git_source_bytes(project_id, &journal.relative_path)?)
            != journal.source_sha256
        {
            return Err("G06B_COMMIT_RECOVERY_SOURCE_CHANGED");
        }
        journal.phase = "verified".into();
        append_journal(&path, &journal)?;
        self.inspect_reviewed_commit_recovery(project_id)
    }

    pub fn inspect_reviewed_commit_recovery(
        &self,
        project_id: &str,
    ) -> Result<ReviewedCommitRecovery, &'static str> {
        let (repo, gitdir) = safe_repo(self, project_id)?;
        let last = last_journal(&journal_path(self, project_id)?)?;
        let head = branch(&repo)?.1;
        let locked = fs::symlink_metadata(gitdir.join("index.lock")).is_ok();
        let (state, phase, old, intended) = match last {
            None => ("no_journal", String::new(), false, false),
            Some(j) if j.project_id != project_id => {
                return Err("G06B_COMMIT_JOURNAL_PROJECT_MISMATCH")
            }
            Some(j) => {
                let old = head == j.old_head;
                let intended = !j.commit_oid.is_empty() && head == j.commit_oid;
                let status = if j.phase == "verified" {
                    "verified_historical_commit"
                } else if j.phase == "cancelled" && old {
                    "cancelled_before_ref_update"
                } else if intended {
                    "ref_published_requires_verification"
                } else if old {
                    "ref_not_published_or_external_change"
                } else {
                    "ref_drift_or_unverifiable"
                };
                (status, j.phase, old, intended)
            }
        };
        Ok(ReviewedCommitRecovery {
            schema_version: 1,
            project_id: project_id.to_owned(),
            state,
            last_phase: phase,
            branch_matches_previous: old,
            branch_matches_intended: intended,
            git_index_lock_present: locked,
            operator_reconciliation_required: locked
                || !matches!(
                    state,
                    "no_journal" | "verified_historical_commit" | "cancelled_before_ref_update"
                ),
            automatic_ref_rewrite: false,
            automatic_retry: false,
            network_queried: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{IdeError, NativeDirectoryPicker};
    use git2::Repository;
    use std::{
        fs,
        path::{Path, PathBuf},
    };

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
        let base = std::env::temp_dir().join(format!("ide-g06b04-{:032x}", rand::random::<u128>()));
        let root = base.join("repo");
        fs::create_dir_all(&root).unwrap();
        let text = (1..=48)
            .map(|n| format!("line {n:02}\n"))
            .collect::<String>();
        fs::write(root.join("tracked.txt"), text).unwrap();
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

    fn review_two_edits(
        registry: &IdeWorkspaceRegistry,
        id: &str,
        root: &Path,
    ) -> PartialStageReceipt {
        let old = fs::read_to_string(root.join("tracked.txt")).unwrap();
        let edited = old
            .replace("line 03\n", "selected 03\n")
            .replace("line 43\n", "later unselected 43\n");
        fs::write(root.join("tracked.txt"), edited).unwrap();
        let plan = registry
            .preview_reviewed_git_hunks(id, "tracked.txt")
            .unwrap();
        assert_eq!(plan.hunks.len(), 2);
        let selected = registry
            .select_reviewed_git_hunks(&plan, &[plan.hunks[0].id.clone()])
            .unwrap();
        registry
            .apply_approved_partial_stage(&selected, &AtomicU8::new(0))
            .unwrap()
    }

    fn preview(
        registry: &IdeWorkspaceRegistry,
        receipt: &PartialStageReceipt,
        mode: ReviewedCommitMode,
    ) -> ReviewedCommitPlan {
        registry
            .preview_reviewed_partial_commit(
                receipt,
                mode,
                "reviewed source only",
                "Fixture Author",
                "fixture@localhost.invalid",
            )
            .unwrap()
    }

    #[test]
    fn local_commit_contains_only_selected_hunk_with_unchanged_worktree() {
        let (_temp, registry, id, root) = fixture();
        let original = Repository::open(&root)
            .unwrap()
            .head()
            .unwrap()
            .target()
            .unwrap();
        let receipt = review_two_edits(&registry, &id, &root);
        let worktree = fs::read(root.join("tracked.txt")).unwrap();
        let planned = preview(&registry, &receipt, ReviewedCommitMode::Create);
        assert!(!planned.commit_granted && !planned.amend_granted);
        assert_eq!(planned.previous_head, original.to_string());
        assert_eq!(
            Repository::open(&root).unwrap().head().unwrap().target(),
            Some(original)
        );
        let done = registry
            .execute_approved_reviewed_commit(&planned, &AtomicU8::new(0))
            .unwrap();
        let repo = Repository::open(&root).unwrap();
        let committed = repo
            .find_commit(Oid::from_str(&done.commit_oid).unwrap())
            .unwrap();
        assert_eq!(repo.head().unwrap().target(), Some(committed.id()));
        assert_eq!(committed.parent_id(0).unwrap(), original);
        assert_eq!(committed.tree_id().to_string(), done.committed_tree_oid);
        assert_eq!(planned.staged_tree_oid, "deferred_until_native_os_approval");
        let tree = committed.tree().unwrap();
        let blob = repo
            .find_blob(tree.get_path(Path::new("tracked.txt")).unwrap().id())
            .unwrap();
        let text = std::str::from_utf8(blob.content()).unwrap();
        assert!(text.contains("selected 03\n"));
        assert!(text.contains("line 43\n"));
        assert!(!text.contains("later unselected 43\n"));
        assert_eq!(fs::read(root.join("tracked.txt")).unwrap(), worktree);
        assert!(
            done.approval_one_use
                && done.branch_ref_independently_verified
                && done.staged_tree_independently_verified
                && done.journal_verified
        );
        assert!(
            !done.amended_existing_history
                && !done.git_hooks_executed
                && !done.remote_updated
                && !done.release_approved
        );
        let recovered = registry.inspect_reviewed_commit_recovery(&id).unwrap();
        assert_eq!(recovered.state, "verified_historical_commit");
        assert!(!recovered.operator_reconciliation_required);
        // An old stage cannot be promoted again after the original HEAD moves.
        assert!(registry
            .preview_reviewed_partial_commit(
                &receipt,
                ReviewedCommitMode::Create,
                "replay",
                "Fixture",
                "fixture@localhost.invalid"
            )
            .is_err());
    }

    #[test]
    fn amend_requires_distinct_review_and_rewrites_only_exact_current_head() {
        let (_temp, registry, id, root) = fixture();
        let original_head = Repository::open(&root)
            .unwrap()
            .head()
            .unwrap()
            .target()
            .unwrap();
        let first_stage = review_two_edits(&registry, &id, &root);
        let first_plan = preview(&registry, &first_stage, ReviewedCommitMode::Create);
        let first = registry
            .execute_approved_reviewed_commit(&first_plan, &AtomicU8::new(0))
            .unwrap();
        // The original source still has the unselected second hunk.
        let second_plan = registry
            .preview_reviewed_git_hunks(&id, "tracked.txt")
            .unwrap();
        assert_eq!(second_plan.hunks.len(), 1);
        let second_sel = registry
            .select_reviewed_git_hunks(&second_plan, &[second_plan.hunks[0].id.clone()])
            .unwrap();
        let second_stage = registry
            .apply_approved_partial_stage(&second_sel, &AtomicU8::new(0))
            .unwrap();
        let amended_plan = preview(&registry, &second_stage, ReviewedCommitMode::Amend);
        assert_eq!(amended_plan.previous_head, first.commit_oid);
        assert_eq!(amended_plan.resulting_parent, original_head.to_string());
        let original_source = fs::read(root.join("tracked.txt")).unwrap();
        let done = registry
            .execute_approved_reviewed_commit(&amended_plan, &AtomicU8::new(0))
            .unwrap();
        assert!(done.amended_existing_history);
        assert_ne!(done.commit_oid, first.commit_oid);
        let repo = Repository::open(&root).unwrap();
        let amended = repo
            .find_commit(Oid::from_str(&done.commit_oid).unwrap())
            .unwrap();
        assert_eq!(amended.parent_id(0).unwrap(), original_head);
        assert_eq!(repo.head().unwrap().target(), Some(amended.id()));
        let blob = repo
            .find_blob(
                amended
                    .tree()
                    .unwrap()
                    .get_path(Path::new("tracked.txt"))
                    .unwrap()
                    .id(),
            )
            .unwrap();
        let text = std::str::from_utf8(blob.content()).unwrap();
        assert!(text.contains("selected 03\n"));
        assert!(text.contains("later unselected 43\n"));
        assert_eq!(fs::read(root.join("tracked.txt")).unwrap(), original_source);
        assert!(!done.remote_updated && !done.git_hooks_executed);
    }

    #[test]
    fn forged_stage_stale_index_and_cancel_cannot_publish_a_ref() {
        let (_temp, registry, id, root) = fixture();
        let stage = review_two_edits(&registry, &id, &root);
        let original = Repository::open(&root)
            .unwrap()
            .head()
            .unwrap()
            .target()
            .unwrap();
        let mut forged = stage.clone();
        forged.source_sha256 = "0".repeat(64);
        assert!(registry
            .preview_reviewed_partial_commit(
                &forged,
                ReviewedCommitMode::Create,
                "forged",
                "Fixture",
                "fixture@localhost.invalid"
            )
            .is_err());
        let mut forged_oid = stage.clone();
        forged_oid.old_blob_oid = "0".repeat(40);
        assert!(registry
            .preview_reviewed_partial_commit(
                &forged_oid,
                ReviewedCommitMode::Create,
                "forged",
                "Fixture",
                "fixture@localhost.invalid"
            )
            .is_err());
        let plan = preview(&registry, &stage, ReviewedCommitMode::Create);
        let mut changed = plan.clone();
        changed.message = "unreviewed message".into();
        assert_eq!(
            registry
                .execute_approved_reviewed_commit(&changed, &AtomicU8::new(0))
                .unwrap_err(),
            "G06B_COMMIT_STALE_OR_FORGED_PLAN"
        );
        let cancelled = AtomicU8::new(2);
        assert_eq!(
            registry
                .execute_approved_reviewed_commit(&plan, &cancelled)
                .unwrap_err(),
            "G06B_COMMIT_INVALID_OR_CANCELLED_PLAN"
        );
        fs::write(root.join(".git/index.lock"), b"foreign index writer").unwrap();
        assert!(registry
            .execute_approved_reviewed_commit(&plan, &AtomicU8::new(0))
            .is_err());
        assert_eq!(
            fs::read(root.join(".git/index.lock")).unwrap(),
            b"foreign index writer"
        );
        fs::remove_file(root.join(".git/index.lock")).unwrap();
        assert_eq!(
            Repository::open(&root).unwrap().head().unwrap().target(),
            Some(original)
        );
    }

    #[test]
    fn interrupted_prepublication_journal_is_only_reconciled_after_new_review() {
        let (_tmp, registry, id, root) = fixture();
        let stage = review_two_edits(&registry, &id, &root);
        let plan = preview(&registry, &stage, ReviewedCommitMode::Create);
        let path = journal_path(&registry, &id).unwrap();
        append_journal(
            &path,
            &CommitJournal {
                schema_version: 1,
                operation_id: "restart-fixture".into(),
                project_id: id.clone(),
                phase: "object_written".into(),
                mode: ReviewedCommitMode::Create,
                branch_ref: plan.branch_ref.clone(),
                old_head: plan.previous_head.clone(),
                commit_oid: "a".repeat(40),
                parent_oid: plan.resulting_parent.clone(),
                tree_oid: plan.staged_tree_oid.clone(),
                index_sha256: plan.index_sha256.clone(),
                plan_sha256: plan.plan_sha256.clone(),
                source_sha256: stage.source_sha256.clone(),
                relative_path: stage.relative_path.clone(),
            },
        )
        .unwrap();
        drop(registry);
        let reopened = IdeWorkspaceRegistry::open(_tmp.0.join("catalog.json")).unwrap();
        reopened.reopen(&id).unwrap();
        let pending = reopened.inspect_reviewed_commit_recovery(&id).unwrap();
        assert!(pending.operator_reconciliation_required);
        assert_eq!(pending.state, "ref_not_published_or_external_change");
        let after = reopened.reconcile_reviewed_commit_journal(&id).unwrap();
        assert_eq!(after.state, "cancelled_before_ref_update");
        assert!(!after.operator_reconciliation_required);
        assert_eq!(
            Repository::open(&root)
                .unwrap()
                .head()
                .unwrap()
                .target()
                .unwrap()
                .to_string(),
            plan.previous_head
        );
        assert!(!root.join(".git/index.lock").exists());
    }
    #[test]
    fn another_writer_moving_branch_denies_stale_commit_and_amend_plan() {
        let (_temp, registry, id, root) = fixture();
        let stage = review_two_edits(&registry, &id, &root);
        let expected = preview(&registry, &stage, ReviewedCommitMode::Create);
        let repo = Repository::open(&root).unwrap();
        let head = repo.head().unwrap().peel_to_commit().unwrap();
        let tree = head.tree().unwrap();
        let signature = Signature::now("Other Git writer", "other@localhost.invalid").unwrap();
        let competing_oid = repo
            .commit(
                None,
                &signature,
                &signature,
                "independent concurrent local commit",
                &tree,
                &[&head],
            )
            .unwrap();
        repo.reference_matching(
            &expected.branch_ref,
            competing_oid,
            true,
            head.id(),
            "competing local Git ref update",
        )
        .unwrap();
        drop(tree);
        drop(head);
        drop(repo);
        assert!(registry
            .execute_approved_reviewed_commit(&expected, &AtomicU8::new(0))
            .is_err());
        let reopened = Repository::open(&root).unwrap();
        assert_eq!(reopened.head().unwrap().target(), Some(competing_oid));
    }

    #[test]
    fn restart_restores_exact_staged_receipt_but_never_reuses_os_commit_grant() {
        let (temp, registry, id, root) = fixture();
        let original_head = Repository::open(&root)
            .unwrap()
            .head()
            .unwrap()
            .target()
            .unwrap();
        let staged = review_two_edits(&registry, &id, &root);
        drop(registry);
        let reopened = IdeWorkspaceRegistry::open(temp.0.join("catalog.json")).unwrap();
        reopened.reopen(&id).unwrap();
        let restored = reopened
            .restore_verified_partial_stage_receipt(&id)
            .unwrap();
        assert_eq!(restored, staged);
        let plan = preview(&reopened, &restored, ReviewedCommitMode::Create);
        assert_eq!(plan.previous_head, original_head.to_string());
        assert!(!plan.commit_granted && !plan.amend_granted && !plan.published);
        assert_eq!(
            reopened
                .execute_approved_reviewed_commit(&plan, &AtomicU8::new(2))
                .unwrap_err(),
            "G06B_COMMIT_INVALID_OR_CANCELLED_PLAN"
        );
        assert_eq!(
            Repository::open(&root).unwrap().head().unwrap().target(),
            Some(original_head)
        );
    }
}
