//! Group 6B-01: bounded, non-executing reviewed batch stage proposal.
//! This is preparation for future one-use OS consent. It does not stage,
//! commit, run Git hooks, update refs or grant use of another worktree.
use crate::{reviewed_git::ReviewedGitFile, IdeWorkspaceRegistry};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

const MAX_FILES: usize = 8;
const MAX_BATCH_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewedGitBatch {
    pub schema_version: u32,
    pub project_id: String,
    pub head_sha: String,
    pub index_fingerprint: String,
    pub files: Vec<ReviewedGitFile>,
    pub total_source_bytes: usize,
    pub ready_for_review: bool,
    pub stage_authorized: bool,
    pub commit_authorized: bool,
    pub git_hooks_executed: bool,
    pub remote_updated: bool,
    pub operation_performed: bool,
}

fn validate_files(files: &[String]) -> Result<(), &'static str> {
    if files.is_empty() || files.len() > MAX_FILES {
        return Err("G06B_BATCH_FILE_COUNT_DENIED");
    }
    let mut seen = BTreeSet::new();
    for path in files {
        // A list controls only reads of previously registered source
        // files. Existing reviewed_git validates every actual path again.
        if !seen.insert(path.as_str()) {
            return Err("G06B_BATCH_DUPLICATE_PATH_DENIED");
        }
    }
    Ok(())
}

impl IdeWorkspaceRegistry {
    /// Return exact current source SHA and index revision for <= 8
    /// already tracked regular files. Never publish a stage/commit grant.
    pub fn preview_reviewed_git_batch(
        &self,
        project_id: &str,
        requested_paths: &[String],
    ) -> Result<ReviewedGitBatch, &'static str> {
        validate_files(requested_paths)?;
        let mut ordered = requested_paths.to_vec();
        ordered.sort();
        let mut files = Vec::with_capacity(ordered.len());
        let mut total_bytes = 0_usize;
        let mut common_head: Option<String> = None;
        let mut common_index: Option<String> = None;
        for path in &ordered {
            let reviewed = self.preview_reviewed_git_file(project_id, path)?;
            if common_head.as_deref().is_some_and(|h| h != reviewed.local_head.as_str())
                || common_index.as_deref().is_some_and(|i| i != reviewed.index_fingerprint.as_str())
            {
                return Err("G06B_SOURCE_CHANGED_DURING_PREVIEW");
            }
            total_bytes = total_bytes.checked_add(reviewed.file_size)
                .ok_or("G06B_BATCH_SOURCE_TOO_LARGE")?;
            if total_bytes > MAX_BATCH_BYTES {
                return Err("G06B_BATCH_SOURCE_TOO_LARGE");
            }
            if common_head.is_none() { common_head = Some(reviewed.local_head.clone()); }
            if common_index.is_none() { common_index = Some(reviewed.index_fingerprint.clone()); }
            files.push(reviewed);
        }
        // Re-evaluate after the last file: earlier preview evidence can
        // become stale while the batch is being assembled.
        let first = self.preview_reviewed_git_file(project_id, &ordered[0])?;
        if first.local_head != common_head.as_deref().unwrap_or_default()
            || first.index_fingerprint != common_index.as_deref().unwrap_or_default()
            || first.worktree_sha256 != files[0].worktree_sha256
        {
            return Err("G06B_SOURCE_CHANGED_DURING_PREVIEW");
        }
        Ok(ReviewedGitBatch {
            schema_version: 1,
            project_id: project_id.to_owned(),
            head_sha: common_head.ok_or("G06B_EMPTY_BATCH")?,
            index_fingerprint: common_index.ok_or("G06B_EMPTY_BATCH")?,
            files,
            total_source_bytes: total_bytes,
            ready_for_review: true,
            stage_authorized: false,
            commit_authorized: false,
            git_hooks_executed: false,
            remote_updated: false,
            operation_performed: false,
        })
    }

    /// A read-only optimistic concurrency gate. The WebView can request a
    /// refreshed preview but cannot use it to authorize staging or commits.
    pub fn revalidate_reviewed_git_batch(
        &self,
        expected: &ReviewedGitBatch,
    ) -> Result<ReviewedGitBatch, &'static str> {
        if expected.schema_version != 1 || !expected.ready_for_review
            || expected.stage_authorized || expected.commit_authorized
            || expected.git_hooks_executed || expected.remote_updated
            || expected.operation_performed
        {
            return Err("G06B_INVALID_BATCH_EXPECTATION");
        }
        let requested: Vec<String> = expected.files.iter()
            .map(|file| file.relative_path.clone()).collect();
        validate_files(&requested)?;
        let refreshed = self.preview_reviewed_git_batch(&expected.project_id, &requested)?;
        if refreshed.head_sha != expected.head_sha
            || refreshed.index_fingerprint != expected.index_fingerprint
            || refreshed.total_source_bytes != expected.total_source_bytes
            || refreshed.files.len() != expected.files.len()
            || refreshed.files.iter().zip(&expected.files).any(|(a,b)|
                a.relative_path != b.relative_path
                || a.worktree_sha256 != b.worktree_sha256
                || a.file_size != b.file_size
                || a.local_head != b.local_head
                || a.index_fingerprint != b.index_fingerprint
            )
        {
            return Err("G06B_REVIEW_STALE");
        }
        Ok(refreshed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{IdeError, NativeDirectoryPicker};
    use git2::{Repository, Signature};
    use std::{fs, path::{Path, PathBuf}};
    struct Picker(PathBuf);
    impl NativeDirectoryPicker for Picker {
        fn choose_project_directory(&mut self) -> Result<Option<PathBuf>, IdeError> {
            Ok(Some(self.0.clone()))
        }
    }
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
    }
    fn fixture() -> (Fixture, IdeWorkspaceRegistry, String, PathBuf) {
        let base = std::env::temp_dir().join(format!("ide-g06b-{:032x}", rand::random::<u128>()));
        let root = base.join("repo");
        fs::create_dir_all(&root).unwrap();
        let repo = Repository::init(&root).unwrap();
        fs::write(root.join("one.rs"), b"original one\n").unwrap();
        fs::write(root.join("two.rs"), b"original two\n").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("one.rs")).unwrap();
        index.add_path(Path::new("two.rs")).unwrap();
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let sig = Signature::now("Fixture", "fixture@localhost.invalid").unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[]).unwrap();
        drop(tree); drop(index); drop(repo);
        let registry = IdeWorkspaceRegistry::open(base.join("catalog.json")).unwrap();
        let id = registry.register_from_native_picker(&mut Picker(root.clone())).unwrap().unwrap().project_id;
        (Fixture(base), registry, id, root)
    }
    #[test]
    fn batch_preview_is_sorted_local_and_never_grants_stage() {
        let (_t, registry, id, root) = fixture();
        fs::write(root.join("one.rs"), b"reviewed one\n").unwrap();
        fs::write(root.join("two.rs"), b"reviewed two\n").unwrap();
        let paths = vec!["two.rs".into(), "one.rs".into()];
        let preview = registry.preview_reviewed_git_batch(&id, &paths).unwrap();
        assert_eq!(preview.files.len(), 2);
        assert_eq!(preview.files[0].relative_path, "one.rs");
        assert!(!preview.operation_performed && !preview.stage_authorized && !preview.commit_authorized);
        assert!(!preview.git_hooks_executed && !preview.remote_updated);
        registry.revalidate_reviewed_git_batch(&preview).unwrap();
        assert!(Repository::open(&root).unwrap().statuses(None).unwrap().iter().count() >= 2);
    }
    #[test]
    fn duplicates_untracked_traversal_and_stale_bytes_are_denied() {
        let (_t, registry, id, root) = fixture();
        assert_eq!(
            registry.preview_reviewed_git_batch(&id, &["one.rs".into(), "one.rs".into()]).unwrap_err(),
            "G06B_BATCH_DUPLICATE_PATH_DENIED"
        );
        assert!(registry.preview_reviewed_git_batch(&id, &["../outside".into()]).is_err());
        assert!(registry.preview_reviewed_git_batch(&id, &["untracked.rs".into()]).is_err());
        let preview = registry.preview_reviewed_git_batch(&id, &["one.rs".into(), "two.rs".into()]).unwrap();
        fs::write(root.join("two.rs"), b"changed since review\n").unwrap();
        assert_eq!(registry.revalidate_reviewed_git_batch(&preview).unwrap_err(), "G06B_REVIEW_STALE");
    }
}
