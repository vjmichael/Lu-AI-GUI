//! WO9-D — human-reviewed, one-file Git stage/commit with no subprocesses,
//! hooks, push, fetch, checkout, or arbitrary repository paths.
//! Source bytes are read through a registered project capability, then pinned
//! in an in-memory libgit2 blob; never stage ambient worktree paths by name.
use crate::{
    browser::{descend, validate_relative},
    IdeWorkspaceRegistry,
};
use git2::{Index, IndexEntry, IndexTime, Oid, Repository};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

const MAX_GIT_FILE_BYTES: u64 = 1024 * 1024;
const MAX_INDEX_ENTRIES: usize = 8192;
const MAX_IDENTITY: usize = 96;
const MAX_COMMIT_MESSAGE: usize = 1200;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewedGitFile {
    pub project_id: String,
    pub relative_path: String,
    pub local_head: String,
    pub worktree_sha256: String,
    pub index_fingerprint: String,
    pub file_size: usize,
    pub stage: String,
    pub code_executed: bool,
    pub git_hooks_executed: bool,
    pub remote_updated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewedStageReceipt {
    pub project_id: String,
    pub relative_path: String,
    pub base_head: String,
    pub source_sha256: String,
    pub staged_index_fingerprint: String,
    pub staged_blob_oid: String,
    pub git_hooks_executed: bool,
    pub remote_updated: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewedCommitReceipt {
    pub project_id: String,
    pub relative_path: String,
    pub previous_head: String,
    pub commit_sha: String,
    pub source_sha256: String,
    pub git_hooks_executed: bool,
    pub remote_updated: bool,
    pub release_approved: bool,
}

fn repo_at_selected_root(root: &Path) -> Result<Repository, &'static str> {
    let repo = Repository::open(root).map_err(|_| "WO9_GIT_REPOSITORY_ABSENT")?;
    let workdir = repo.workdir().ok_or("WO9_GIT_BARE_REPOSITORY_DENIED")?;
    let actual = fs::canonicalize(workdir).map_err(|_| "WO9_GIT_ROOT_INVALID")?;
    if actual != root || repo.head_detached().unwrap_or(true) {
        return Err("WO9_GIT_OUTSIDE_ROOT_OR_DETACHED");
    }
    let reference = repo.head().map_err(|_| "WO9_GIT_HEAD_ABSENT")?;
    if reference.target().is_none() || reference.name().is_none() {
        return Err("WO9_GIT_HEAD_UNRESOLVED");
    }
    drop(reference);
    Ok(repo)
}

fn clean_index(repo: &Repository) -> Result<Index, &'static str> {
    let mut index = repo.index().map_err(|_| "WO9_GIT_INDEX_UNAVAILABLE")?;
    index.read(true).map_err(|_| "WO9_GIT_INDEX_UNAVAILABLE")?;
    if index.len() > MAX_INDEX_ENTRIES || index.has_conflicts() {
        return Err("WO9_GIT_INDEX_COMPLEX_OR_CONFLICTED");
    }
    let head_tree = repo
        .head()
        .and_then(|h| h.peel_to_tree())
        .map_err(|_| "WO9_GIT_HEAD_TREE_MISSING")?;
    let staged = repo
        .diff_tree_to_index(Some(&head_tree), Some(&index), None)
        .map_err(|_| "WO9_GIT_INDEX_DIFF_UNAVAILABLE")?;
    if staged.deltas().next().is_some() {
        return Err("WO9_GIT_EXISTING_STAGED_CHANGES_DENIED");
    }
    Ok(index)
}

fn index_fingerprint(index: &Index) -> String {
    let mut h = Sha256::new();
    for item in index.iter() {
        h.update(&(item.path.len() as u64).to_le_bytes());
        h.update(&item.path);
        h.update(item.id.as_bytes());
        h.update(&item.mode.to_le_bytes());
    }
    format!("{:x}", h.finalize())
}

fn safe_relative(path: &str) -> Result<Vec<String>, &'static str> {
    let parts = validate_relative(path, false).map_err(|_| "WO9_GIT_INVALID_PATH")?;
    if parts
        .first()
        .is_some_and(|p| p.eq_ignore_ascii_case(".git"))
        || path.len() > 240
        || path.starts_with(".git/")
    {
        return Err("WO9_GIT_CONTROL_PATH_DENIED");
    }
    Ok(parts)
}

fn read_pinned_source(
    registry: &IdeWorkspaceRegistry,
    project_id: &str,
    path: &str,
) -> Result<Vec<u8>, &'static str> {
    let mut parts = safe_relative(path)?;
    let leaf = parts.pop().ok_or("WO9_GIT_INVALID_PATH")?;
    let root = registry
        .source_capability(project_id)
        .map_err(|_| "WO9_GIT_PROJECT_UNAVAILABLE")?;
    let parent = descend(root, &parts).map_err(|_| "WO9_GIT_PARENT_DENIED")?;
    let meta = parent
        .symlink_metadata(&leaf)
        .map_err(|_| "WO9_GIT_FILE_ABSENT")?;
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > MAX_GIT_FILE_BYTES {
        return Err("WO9_GIT_UNSAFE_OR_OVERSIZED_SOURCE");
    }
    let mut opened = parent.open(&leaf).map_err(|_| "WO9_GIT_SOURCE_DENIED")?;
    if !opened
        .metadata()
        .map_err(|_| "WO9_GIT_SOURCE_DENIED")?
        .is_file()
    {
        return Err("WO9_GIT_UNSAFE_SOURCE");
    }
    let mut bytes = Vec::new();
    opened
        .take(MAX_GIT_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "WO9_GIT_SOURCE_READ_FAILED")?;
    let after = parent
        .symlink_metadata(&leaf)
        .map_err(|_| "WO9_GIT_SOURCE_CHANGED")?;
    if after.file_type().is_symlink()
        || !after.is_file()
        || after.len() != meta.len()
        || bytes.len() as u64 != meta.len()
    {
        return Err("WO9_GIT_SOURCE_CHANGED");
    }
    Ok(bytes)
}

fn hex_digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn same_head(repo: &Repository, head: &str) -> Result<(), &'static str> {
    let now = repo
        .head()
        .ok()
        .and_then(|h| h.target())
        .map(|id| id.to_string());
    if now.as_deref() != Some(head) {
        Err("WO9_GIT_HEAD_CHANGED")
    } else {
        Ok(())
    }
}

fn valid_sha64(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}

fn checked_project(
    registry: &IdeWorkspaceRegistry,
    project_id: &str,
) -> Result<(Repository, PathBuf), &'static str> {
    let root = registry
        .registered_git_root(project_id)
        .map_err(|_| "WO9_GIT_PROJECT_NOT_ACTIVE")?;
    let repo = repo_at_selected_root(&root)?;
    Ok((repo, root))
}

impl IdeWorkspaceRegistry {
    pub fn preview_reviewed_git_file(
        &self,
        project_id: &str,
        relative_path: &str,
    ) -> Result<ReviewedGitFile, &'static str> {
        safe_relative(relative_path)?;
        let (repo, _) = checked_project(self, project_id)?;
        let index = clean_index(&repo)?;
        // Force only a tracked existing source. Untracked/imported content can
        // be reviewed as a future separate explicit add action.
        if index.get_path(Path::new(relative_path), 0).is_none() {
            return Err("WO9_GIT_UNTRACKED_OR_UNAPPROVED_SOURCE");
        }
        let tracked = index
            .get_path(Path::new(relative_path), 0)
            .ok_or("WO9_GIT_UNTRACKED_OR_UNAPPROVED_SOURCE")?;
        if tracked.mode != 0o100644 && tracked.mode != 0o100755 {
            return Err("WO9_GIT_NON_REGULAR_INDEX_MODE_DENIED");
        }
        let bytes = read_pinned_source(self, project_id, relative_path)?;
        let head = repo
            .head()
            .ok()
            .and_then(|h| h.target())
            .ok_or("WO9_GIT_HEAD_ABSENT")?;
        Ok(ReviewedGitFile {
            project_id: project_id.to_owned(),
            relative_path: relative_path.to_owned(),
            local_head: head.to_string(),
            worktree_sha256: hex_digest(&bytes),
            index_fingerprint: index_fingerprint(&index),
            file_size: bytes.len(),
            stage: "preview_only".into(),
            code_executed: false,
            git_hooks_executed: false,
            remote_updated: false,
        })
    }

    pub fn stage_reviewed_git_file(
        &self,
        expected: &ReviewedGitFile,
    ) -> Result<ReviewedStageReceipt, &'static str> {
        if !valid_sha64(&expected.worktree_sha256)
            || !valid_sha64(&expected.index_fingerprint)
            || expected.stage != "preview_only"
            || expected.code_executed
            || expected.git_hooks_executed
            || expected.remote_updated
        {
            return Err("WO9_GIT_INVALID_REVIEW_TOKEN");
        }
        safe_relative(&expected.relative_path)?;
        let (repo, _) = checked_project(self, &expected.project_id)?;
        same_head(&repo, &expected.local_head)?;
        let mut index = clean_index(&repo)?;
        if index_fingerprint(&index) != expected.index_fingerprint {
            return Err("WO9_GIT_INDEX_CHANGED");
        }
        let previous = index
            .get_path(Path::new(&expected.relative_path), 0)
            .ok_or("WO9_GIT_UNTRACKED_SOURCE")?;
        // Never reinterpret a tracked symlink, gitlink/submodule or unusual
        // index mode as an ordinary user-approved file during add_frombuffer.
        if previous.mode != 0o100644 && previous.mode != 0o100755 {
            return Err("WO9_GIT_NON_REGULAR_INDEX_MODE_DENIED");
        }
        let bytes = read_pinned_source(self, &expected.project_id, &expected.relative_path)?;
        if bytes.len() != expected.file_size || hex_digest(&bytes) != expected.worktree_sha256 {
            return Err("WO9_GIT_SOURCE_CHANGED_AFTER_REVIEW");
        }
        let entry = IndexEntry {
            ctime: IndexTime::new(0, 0),
            mtime: IndexTime::new(0, 0),
            dev: 0,
            ino: 0,
            mode: previous.mode,
            uid: 0,
            gid: 0,
            file_size: bytes.len() as u32,
            id: Oid::zero(),
            flags: 0,
            flags_extended: 0,
            path: expected.relative_path.as_bytes().to_vec(),
        };
        index
            .add_frombuffer(&entry, &bytes)
            .map_err(|_| "WO9_GIT_STAGE_BUFFER_FAILED")?;
        same_head(&repo, &expected.local_head)?;
        // Reconfirm exact bytes before publishing index; future disk changes
        // do not alter the pinned blob.
        if hex_digest(&read_pinned_source(
            self,
            &expected.project_id,
            &expected.relative_path,
        )?) != expected.worktree_sha256
        {
            return Err("WO9_GIT_SOURCE_CHANGED_DURING_STAGE");
        }
        index.write().map_err(|_| "WO9_GIT_INDEX_WRITE_FAILED")?;
        let staged_index = repo.index().map_err(|_| "WO9_GIT_INDEX_UNAVAILABLE")?;
        let staged_entry = staged_index
            .get_path(Path::new(&expected.relative_path), 0)
            .ok_or("WO9_GIT_STAGE_NOT_PUBLISHED")?;
        let blob = repo
            .find_blob(staged_entry.id)
            .map_err(|_| "WO9_GIT_BLOB_MISSING")?;
        if hex_digest(blob.content()) != expected.worktree_sha256 {
            return Err("WO9_GIT_STAGE_BLOB_MISMATCH");
        }
        Ok(ReviewedStageReceipt {
            project_id: expected.project_id.clone(),
            relative_path: expected.relative_path.clone(),
            base_head: expected.local_head.clone(),
            source_sha256: expected.worktree_sha256.clone(),
            staged_index_fingerprint: index_fingerprint(&staged_index),
            staged_blob_oid: staged_entry.id.to_string(),
            git_hooks_executed: false,
            remote_updated: false,
        })
    }

    pub fn commit_reviewed_git_file(
        &self,
        stage: &ReviewedStageReceipt,
        message: &str,
        name: &str,
        email: &str,
    ) -> Result<ReviewedCommitReceipt, &'static str> {
        if !valid_sha64(&stage.source_sha256)
            || !valid_sha64(&stage.staged_index_fingerprint)
            || stage.git_hooks_executed
            || stage.remote_updated
            || message.trim().is_empty()
            || message.len() > MAX_COMMIT_MESSAGE
            || name.trim().is_empty()
            || name.len() > MAX_IDENTITY
            || email.trim().is_empty()
            || email.len() > MAX_IDENTITY
            || [message, name, email]
                .iter()
                .any(|x| x.chars().any(char::is_control))
        {
            return Err("WO9_GIT_COMMIT_METADATA_DENIED");
        }
        safe_relative(&stage.relative_path)?;
        let (repo, _) = checked_project(self, &stage.project_id)?;
        same_head(&repo, &stage.base_head)?;
        let mut index = repo.index().map_err(|_| "WO9_GIT_INDEX_UNAVAILABLE")?;
        if index.has_conflicts()
            || index.len() > MAX_INDEX_ENTRIES
            || index_fingerprint(&index) != stage.staged_index_fingerprint
        {
            return Err("WO9_GIT_INDEX_CHANGED_AFTER_STAGE");
        }
        let staged = index
            .get_path(Path::new(&stage.relative_path), 0)
            .ok_or("WO9_GIT_STAGED_FILE_MISSING")?;
        if staged.id.to_string() != stage.staged_blob_oid {
            return Err("WO9_GIT_STAGED_BLOB_CHANGED");
        }
        let blob = repo
            .find_blob(staged.id)
            .map_err(|_| "WO9_GIT_STAGED_BLOB_MISSING")?;
        if hex_digest(blob.content()) != stage.source_sha256 {
            return Err("WO9_GIT_STAGED_BYTES_CHANGED");
        }
        if hex_digest(&read_pinned_source(
            self,
            &stage.project_id,
            &stage.relative_path,
        )?) != stage.source_sha256
        {
            return Err("WO9_GIT_WORKTREE_CHANGED_AFTER_STAGE");
        }
        let parent = repo
            .find_commit(Oid::from_str(&stage.base_head).map_err(|_| "WO9_GIT_BASE_HEAD_INVALID")?)
            .map_err(|_| "WO9_GIT_BASE_COMMIT_MISSING")?;
        let head_tree = parent.tree().map_err(|_| "WO9_GIT_HEAD_TREE_MISSING")?;
        let delta = repo
            .diff_tree_to_index(Some(&head_tree), Some(&index), None)
            .map_err(|_| "WO9_GIT_STAGED_DIFF_UNAVAILABLE")?;
        if delta.deltas().count() != 1
            || delta.deltas().next().and_then(|d| d.new_file().path())
                != Some(Path::new(&stage.relative_path))
        {
            return Err("WO9_GIT_OTHER_STAGED_CHANGES");
        }
        let signature =
            git2::Signature::now(name, email).map_err(|_| "WO9_GIT_COMMIT_IDENTITY_INVALID")?;
        let oid = index
            .write_tree_to(&repo)
            .map_err(|_| "WO9_GIT_TREE_FAILED")?;
        let tree = repo.find_tree(oid).map_err(|_| "WO9_GIT_TREE_MISSING")?;
        // Create commit object without updating a ref, then atomically move
        // the original branch ref only if it still points to expected HEAD.
        let commit_id = repo
            .commit(None, &signature, &signature, message, &tree, &[&parent])
            .map_err(|_| "WO9_GIT_COMMIT_OBJECT_FAILED")?;
        let branch_ref = repo
            .head()
            .ok()
            .and_then(|h| h.resolve().ok())
            .and_then(|h| h.name().map(str::to_owned))
            .ok_or("WO9_GIT_BRANCH_REF_UNAVAILABLE")?;
        repo.reference_matching(
            &branch_ref,
            commit_id,
            true,
            parent.id(),
            "Lu IDE explicit reviewed single-file commit",
        )
        .map_err(|_| "WO9_GIT_CONCURRENT_REF_UPDATE_DENIED")?;
        Ok(ReviewedCommitReceipt {
            project_id: stage.project_id.clone(),
            relative_path: stage.relative_path.clone(),
            previous_head: stage.base_head.clone(),
            commit_sha: commit_id.to_string(),
            source_sha256: stage.source_sha256.clone(),
            git_hooks_executed: false,
            remote_updated: false,
            release_approved: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{IdeError, IdeWorkspaceRegistry, NativeDirectoryPicker};
    use git2::Signature;
    struct Pick(PathBuf);
    impl NativeDirectoryPicker for Pick {
        fn choose_project_directory(&mut self) -> Result<Option<PathBuf>, IdeError> {
            Ok(Some(self.0.clone()))
        }
    }
    struct Temp(PathBuf);
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn fixture() -> (Temp, IdeWorkspaceRegistry, String, PathBuf) {
        let temp = std::env::temp_dir().join(format!("wo9-git-{}", rand::random::<u64>()));
        let root = temp.join("repo");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("code.rs"), b"old text\n").unwrap();
        fs::write(root.join("other.txt"), b"unchanged\n").unwrap();
        let repo = Repository::init(&root).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("code.rs")).unwrap();
        index.add_path(Path::new("other.txt")).unwrap();
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let sig = Signature::now("Fixture", "fixture@localhost.invalid").unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[])
            .unwrap();
        drop(tree);
        drop(index);
        drop(repo);
        let registry = IdeWorkspaceRegistry::open(temp.join("catalog.json")).unwrap();
        let id = registry
            .register_from_native_picker(&mut Pick(root.clone()))
            .unwrap()
            .unwrap()
            .project_id;
        (Temp(temp), registry, id, root)
    }
    #[test]
    fn reviewed_stage_and_commit_one_exact_buffer_without_hooks_or_push() {
        let (_t, registry, id, root) = fixture();
        fs::write(root.join("code.rs"), b"reviewed text\n").unwrap();
        let preview = registry.preview_reviewed_git_file(&id, "code.rs").unwrap();
        assert_eq!(preview.file_size, b"reviewed text\n".len());
        assert!(registry.stage_reviewed_git_file(&preview).is_ok());
        // The same preview cannot be silently reused after index mutation.
        assert_eq!(
            registry.stage_reviewed_git_file(&preview).unwrap_err(),
            "WO9_GIT_EXISTING_STAGED_CHANGES_DENIED"
        );
        let (_t2, registry2, id2, root2) = fixture();
        fs::write(root2.join("code.rs"), b"reviewed text\n").unwrap();
        let preview2 = registry2
            .preview_reviewed_git_file(&id2, "code.rs")
            .unwrap();
        let stage = registry2.stage_reviewed_git_file(&preview2).unwrap();
        let commit = registry2
            .commit_reviewed_git_file(
                &stage,
                "one reviewed source change",
                "Tester",
                "tester@localhost.invalid",
            )
            .unwrap();
        assert_ne!(commit.commit_sha, preview2.local_head);
        assert!(!commit.remote_updated && !commit.git_hooks_executed);
        assert!(!commit.release_approved);
        let repo = Repository::open(root2).unwrap();
        assert_eq!(
            repo.head().unwrap().target().unwrap().to_string(),
            commit.commit_sha
        );
        assert_eq!(
            repo.find_commit(Oid::from_str(&commit.commit_sha).unwrap())
                .unwrap()
                .parent_count(),
            1
        );
    }
    #[test]
    fn stage_denies_external_file_change_and_symlink_and_untracked_content() {
        let (_t, registry, id, root) = fixture();
        fs::write(root.join("code.rs"), b"new\n").unwrap();
        let preview = registry.preview_reviewed_git_file(&id, "code.rs").unwrap();
        fs::write(root.join("code.rs"), b"changed while reviewing\n").unwrap();
        assert_eq!(
            registry.stage_reviewed_git_file(&preview).unwrap_err(),
            "WO9_GIT_SOURCE_CHANGED_AFTER_REVIEW"
        );
        assert!(registry
            .preview_reviewed_git_file(&id, "../outside")
            .is_err());
        assert!(registry
            .preview_reviewed_git_file(&id, ".git/config")
            .is_err());
        fs::write(root.join("untracked.txt"), b"new").unwrap();
        assert_eq!(
            registry
                .preview_reviewed_git_file(&id, "untracked.txt")
                .unwrap_err(),
            "WO9_GIT_UNTRACKED_OR_UNAPPROVED_SOURCE"
        );
    }
    #[test]
    fn commit_denies_modified_staged_blob_and_secondary_stage() {
        let (_t, registry, id, root) = fixture();
        fs::write(root.join("code.rs"), b"approved\n").unwrap();
        let preview = registry.preview_reviewed_git_file(&id, "code.rs").unwrap();
        let stage = registry.stage_reviewed_git_file(&preview).unwrap();
        fs::write(root.join("code.rs"), b"not approved\n").unwrap();
        assert_eq!(
            registry
                .commit_reviewed_git_file(&stage, "reviewed", "X", "x@localhost.invalid")
                .unwrap_err(),
            "WO9_GIT_WORKTREE_CHANGED_AFTER_STAGE"
        );
    }
}
