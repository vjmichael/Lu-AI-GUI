//! Group 6A-05: bounded Git object-history and blame evidence.
//! Only registered project IDs + validated project-relative tracked file names.
//! No worktree read, subprocess, rev expression, ref write, network or hooks.
use crate::{browser::validate_relative, IdeWorkspaceRegistry};
use git2::{BlameOptions, Repository, Sort};
use serde::Serialize;
use std::fs;

const MAX_HUNKS: usize = 96;
const MAX_FILE_BYTES: usize = 512 * 1024;
const MAX_WALK_COMMITS: usize = 2000;
const MAX_FILE_COMMITS: usize = 40;
const MAX_SUMMARY_CHARS: usize = 180;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitFileBlameEntry {
    pub final_commit_sha: String,
    pub final_start_line: usize,
    pub line_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitFileHistoryEntry {
    pub commit_sha: String,
    pub summary: String,
    pub committed_unix_seconds: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitFileHistoryEvidence {
    pub schema_version: u32,
    pub project_id: String,
    pub relative_path: String,
    pub head_sha: String,
    pub blame: Vec<GitFileBlameEntry>,
    pub blame_truncated: bool,
    pub commits: Vec<GitFileHistoryEntry>,
    pub history_truncated: bool,
    pub worktree_read: bool,
    pub network_queried: bool,
    pub git_process_started: bool,
    pub source_trust: &'static str,
}

fn repo_at_root(registry: &IdeWorkspaceRegistry, project_id: &str) -> Result<Repository, &'static str> {
    let root = registry.registered_git_root(project_id).map_err(|_| "G06A_PROJECT_NOT_OPEN")?;
    let repo = Repository::open(&root).map_err(|_| "G06A_REPOSITORY_UNAVAILABLE")?;
    let actual = fs::canonicalize(repo.workdir().ok_or("G06A_BARE_UNSUPPORTED")?)
        .map_err(|_| "G06A_ROOT_UNVERIFIABLE")?;
    if actual != root { return Err("G06A_REPOSITORY_OUTSIDE_PROJECT"); }
    Ok(repo)
}

impl IdeWorkspaceRegistry {
    pub fn inspect_git_file_history(
        &self,
        project_id: &str,
        relative_path: &str,
    ) -> Result<GitFileHistoryEvidence, &'static str> {
        let parts = validate_relative(relative_path, false)
            .map_err(|_| "G06A_INVALID_RELATIVE_FILE")?;
        if parts.first().is_some_and(|s| s.eq_ignore_ascii_case(".git"))
            || relative_path.len() > 240
        { return Err("G06A_CONTROL_PATH_DENIED"); }
        let repo = repo_at_root(self, project_id)?;
        let head = repo.head().and_then(|h| h.peel_to_commit())
            .map_err(|_| "G06A_HEAD_UNAVAILABLE")?;
        let tree = head.tree().map_err(|_| "G06A_HEAD_TREE_UNAVAILABLE")?;
        let object = tree.get_path(std::path::Path::new(relative_path))
            .map_err(|_| "G06A_TRACKED_PATH_REQUIRED")?;
        if object.kind() != Some(git2::ObjectType::Blob)
            || !matches!(object.filemode(), 0o100644 | 0o100755) {
            return Err("G06A_TRACKED_REGULAR_FILE_REQUIRED");
        }
        let blob = repo.find_blob(object.id()).map_err(|_| "G06A_BLOB_UNAVAILABLE")?;
        if blob.size() > MAX_FILE_BYTES {
            return Err("G06A_FILE_TOO_LARGE_FOR_BLAME");
        }
        let mut opts = BlameOptions::new();
        opts.newest_commit(head.id());
        let blame = repo.blame_file(std::path::Path::new(relative_path), Some(&mut opts))
            .map_err(|_| "G06A_BLAME_UNAVAILABLE")?;
        let mut hunks = Vec::new();
        let mut blame_truncated = false;
        for hunk in blame.iter() {
            if hunks.len() >= MAX_HUNKS { blame_truncated = true; break; }
            hunks.push(GitFileBlameEntry {
                final_commit_sha: hunk.final_commit_id().to_string(),
                final_start_line: hunk.final_start_line(),
                line_count: hunk.lines_in_hunk(),
            });
        }
        let mut walk = repo.revwalk().map_err(|_| "G06A_HISTORY_UNAVAILABLE")?;
        walk.set_sorting(Sort::TOPOLOGICAL | Sort::TIME)
            .map_err(|_| "G06A_HISTORY_UNAVAILABLE")?;
        walk.push(head.id()).map_err(|_| "G06A_HISTORY_UNAVAILABLE")?;
        // Path-constrained ancestry: diff each commit against its first
        // parent. This intentionally reports first-parent path changes, not
        // complete rename-following across all merge parents.
        let mut commits = Vec::new();
        let mut history_truncated = false;
        for (scanned, oid) in walk.enumerate() {
            if scanned >= MAX_WALK_COMMITS {
                history_truncated = true;
                break;
            }
            let oid = oid.map_err(|_| "G06A_HISTORY_UNAVAILABLE")?;
            let commit = repo.find_commit(oid).map_err(|_| "G06A_HISTORY_UNAVAILABLE")?;
            let current_tree = commit.tree().map_err(|_| "G06A_HISTORY_UNAVAILABLE")?;
            let previous = if commit.parent_count() > 0 {
                Some(commit.parent(0).and_then(|p| p.tree()).map_err(|_| "G06A_HISTORY_UNAVAILABLE")?)
            } else { None };
            let mut diff_opts = git2::DiffOptions::new();
            diff_opts.pathspec(relative_path).disable_pathspec_match(true);
            let diff = repo.diff_tree_to_tree(previous.as_ref(), Some(&current_tree), Some(&mut diff_opts))
                .map_err(|_| "G06A_HISTORY_UNAVAILABLE")?;
            if diff.deltas().next().is_none() { continue; }
            if commits.len() >= MAX_FILE_COMMITS { history_truncated = true; break; }
            commits.push(GitFileHistoryEntry {
                commit_sha: oid.to_string(),
                summary: commit.summary().unwrap_or("(no message)").chars().take(MAX_SUMMARY_CHARS).collect(),
                committed_unix_seconds: commit.time().seconds(),
            });
        }
        Ok(GitFileHistoryEvidence {
            schema_version: 1, project_id: project_id.to_owned(),
            relative_path: relative_path.to_owned(),
            head_sha: head.id().to_string(),
            blame: hunks, blame_truncated, commits, history_truncated,
            worktree_read: false, network_queried: false, git_process_started: false,
            source_trust: "local_committed_git_objects_not_live_remote",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{IdeError, NativeDirectoryPicker};
    use git2::Signature;
    use std::{fs, path::PathBuf};
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
    fn fixture() -> (Fixture, IdeWorkspaceRegistry, String) {
        let base = std::env::temp_dir().join(format!("ide-g06a-blame-{:032x}", rand::random::<u128>()));
        let dir = base.join("repo");
        fs::create_dir_all(&dir).unwrap();
        let repo = Repository::init(&dir).unwrap();
        fs::write(dir.join("file.txt"), b"first line\n").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("file.txt")).unwrap();
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let sig = Signature::now("Fixture", "fixture@localhost.invalid").unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "initial file", &tree, &[]).unwrap();
        drop(tree);
        drop(index);
        drop(repo);
        let registry = IdeWorkspaceRegistry::open(base.join("catalog.json")).unwrap();
        let id = registry.register_from_native_picker(&mut Picker(dir)).unwrap().unwrap().project_id;
        (Fixture(base), registry, id)
    }
    #[test]
    fn local_blame_and_file_history_do_not_execute_or_read_worktree_bytes() {
        let (_tmp, registry, id) = fixture();
        let evidence = registry.inspect_git_file_history(&id, "file.txt").unwrap();
        assert_eq!(evidence.blame.len(), 1);
        assert_eq!(evidence.blame[0].final_start_line, 1);
        assert_eq!(evidence.commits.len(), 1);
        assert_eq!(evidence.commits[0].summary, "initial file");
        assert!(!evidence.worktree_read && !evidence.git_process_started && !evidence.network_queried);
    }
    #[test]
    fn path_traversal_and_untracked_file_blame_are_denied() {
        let (_tmp, registry, id) = fixture();
        for path in ["../outside", ".git/config", "/etc/passwd", "untracked.rs", "file.txt/../file.txt"] {
            assert!(registry.inspect_git_file_history(&id, path).is_err());
        }
    }
}
