//! Wave C1: read-only Git patch and commit history for the exact registered root.
//! No git CLI, hooks, fetch, push, checkout, index mutation or Lu adapter.
use crate::IdeWorkspaceRegistry;
use git2::{DiffFormat, DiffOptions, Repository, Sort};
use serde::Serialize;
use std::fs;

const MAX_PATCH_BYTES: usize = 64 * 1024;
const MAX_DIFF_FILES: usize = 80;
const MAX_HISTORY: usize = 30;
const MAX_SUMMARY_CHARS: usize = 220;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitDiffEvidence {
    pub schema_version: u32,
    pub project_id: String,
    pub scope: &'static str,
    pub local_head: Option<String>,
    pub files_changed: usize,
    pub patch: String,
    pub patch_bytes: usize,
    pub truncated: bool,
    pub untracked_excluded: bool,
    pub source_trust: &'static str,
    pub live_remote: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHistoryEntry {
    pub commit_sha: String,
    pub summary: String,
    pub parent_count: usize,
    pub committed_unix_seconds: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHistoryEvidence {
    pub schema_version: u32,
    pub project_id: String,
    pub local_head: Option<String>,
    pub commits: Vec<GitHistoryEntry>,
    pub truncated: bool,
    pub live_remote: &'static str,
}

fn registered_repo(
    registry: &IdeWorkspaceRegistry,
    project_id: &str,
) -> Result<Repository, &'static str> {
    let root = registry
        .registered_git_root(project_id)
        .map_err(|_| "PROJECT_NOT_OPEN")?;
    let repo = Repository::open(&root).map_err(|_| "GIT_REPOSITORY_UNAVAILABLE")?;
    let workdir = repo.workdir().ok_or("BARE_REPOSITORY_UNSUPPORTED")?;
    let actual = fs::canonicalize(workdir).map_err(|_| "GIT_WORKTREE_UNAVAILABLE")?;
    if actual != root {
        return Err("GIT_REPOSITORY_OUTSIDE_PROJECT");
    }
    Ok(repo)
}

impl IdeWorkspaceRegistry {
    /// The difference between HEAD and the index, or the index and worktree.
    /// The second scope deliberately omits untracked content; use git.status
    /// for the existence of untracked files. This API never stages a file.
    pub fn inspect_git_diff(
        &self,
        project_id: &str,
        scope: &str,
    ) -> Result<GitDiffEvidence, &'static str> {
        if scope != "staged" && scope != "worktree" {
            return Err("INVALID_GIT_DIFF_SCOPE");
        }
        let repo = registered_repo(self, project_id)?;
        let local_head = repo
            .head()
            .ok()
            .and_then(|h| h.target())
            .map(|oid| oid.to_string());
        let mut opts = DiffOptions::new();
        opts.context_lines(2)
            .interhunk_lines(0)
            .include_untracked(false)
            .recurse_untracked_dirs(false);
        let diff = if scope == "staged" {
            let tree = repo.head().ok().and_then(|h| h.peel_to_tree().ok());
            repo.diff_tree_to_index(tree.as_ref(), None, Some(&mut opts))
        } else {
            repo.diff_index_to_workdir(None, Some(&mut opts))
        }
        .map_err(|_| "GIT_DIFF_UNAVAILABLE")?;

        let files_changed = diff.deltas().count();
        let mut bytes = Vec::new();
        let mut truncated = files_changed > MAX_DIFF_FILES;
        let mut current_file: Option<Vec<u8>> = None;
        let mut shown_files = 0usize;
        diff.print(DiffFormat::Patch, |delta, _hunk, line| {
            let path = delta
                .new_file()
                .path()
                .or_else(|| delta.old_file().path())
                .map(|p| p.to_string_lossy().as_bytes().to_vec());
            if current_file != path {
                current_file = path;
                shown_files += 1;
            }
            if shown_files > MAX_DIFF_FILES {
                truncated = true;
                return true;
            }
            let content = line.content();
            // Prefix diff body with standard patch origin marker. Headers
            // already include their own marker in libgit2 output.
            let prefix = match line.origin() {
                '+' | '-' | ' ' => Some(line.origin() as u8),
                _ => None,
            };
            let required = content.len() + usize::from(prefix.is_some());
            if bytes.len().saturating_add(required) > MAX_PATCH_BYTES {
                truncated = true;
                return true;
            }
            if let Some(mark) = prefix {
                bytes.push(mark);
            }
            bytes.extend_from_slice(content);
            true
        })
        .map_err(|_| "GIT_DIFF_UNAVAILABLE")?;
        let patch = String::from_utf8_lossy(&bytes).into_owned();
        Ok(GitDiffEvidence {
            schema_version: 1,
            project_id: project_id.to_owned(),
            scope: if scope == "staged" {
                "staged"
            } else {
                "worktree"
            },
            local_head,
            files_changed,
            patch,
            patch_bytes: bytes.len(),
            truncated,
            untracked_excluded: true,
            source_trust: "untrusted_local_git_text",
            live_remote: "not_queried",
        })
    }

    /// History of the local HEAD only. It is not evidence that GitHub is
    /// current and does not run arbitrary revision expressions.
    pub fn inspect_git_history(
        &self,
        project_id: &str,
    ) -> Result<GitHistoryEvidence, &'static str> {
        let repo = registered_repo(self, project_id)?;
        let local_head = repo
            .head()
            .ok()
            .and_then(|h| h.target())
            .map(|oid| oid.to_string());
        let mut commits = Vec::new();
        let mut truncated = false;
        if local_head.is_some() {
            let mut walk = repo.revwalk().map_err(|_| "GIT_HISTORY_UNAVAILABLE")?;
            walk.set_sorting(Sort::TOPOLOGICAL | Sort::TIME)
                .map_err(|_| "GIT_HISTORY_UNAVAILABLE")?;
            walk.push_head().map_err(|_| "GIT_HISTORY_UNAVAILABLE")?;
            for id in walk {
                if commits.len() == MAX_HISTORY {
                    truncated = true;
                    break;
                }
                let oid = id.map_err(|_| "GIT_HISTORY_UNAVAILABLE")?;
                let commit = repo
                    .find_commit(oid)
                    .map_err(|_| "GIT_HISTORY_UNAVAILABLE")?;
                let summary: String = commit
                    .summary()
                    .unwrap_or("(no message)")
                    .chars()
                    .take(MAX_SUMMARY_CHARS)
                    .collect();
                commits.push(GitHistoryEntry {
                    commit_sha: oid.to_string(),
                    summary,
                    parent_count: commit.parent_count() as usize,
                    committed_unix_seconds: commit.time().seconds(),
                });
            }
        }
        Ok(GitHistoryEvidence {
            schema_version: 1,
            project_id: project_id.to_owned(),
            local_head,
            commits,
            truncated,
            live_remote: "not_queried",
        })
    }
}
