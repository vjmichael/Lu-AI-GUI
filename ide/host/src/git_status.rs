//! Native read-only Git status for a user-registered project root.
//!
//! libgit2 reads repository/index/worktree metadata, but no subprocesses, Git
//! hooks, network requests, merges, fetches, or repository writes are exposed.
use std::fs;

use git2::{BranchType, Repository, Status, StatusOptions};
use serde::Serialize;

use crate::IdeWorkspaceRegistry;

const MAX_VISIBLE_CHANGES: usize = 120;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitFileChange {
    pub relative_path: String,
    pub kind: &'static str,
    pub stage: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitWorktreeInspection {
    pub state: &'static str,
    pub branch: Option<String>,
    pub local_head: Option<String>,
    pub upstream_head: Option<String>,
    pub upstream_comparison: &'static str,
    pub commits_ahead: Option<usize>,
    pub commits_behind: Option<usize>,
    pub worktree: &'static str,
    pub modified_or_staged_count: usize,
    pub untracked_count: usize,
    pub displayed_changes: Vec<GitFileChange>,
    pub truncated: bool,
    pub live_remote: &'static str,
}

fn unavailable(state: &'static str) -> GitWorktreeInspection {
    GitWorktreeInspection {
        state,
        branch: None,
        local_head: None,
        upstream_head: None,
        upstream_comparison: "not_verified",
        commits_ahead: None,
        commits_behind: None,
        worktree: "not_verified",
        modified_or_staged_count: 0,
        untracked_count: 0,
        displayed_changes: Vec::new(),
        truncated: false,
        live_remote: "not_queried",
    }
}

impl IdeWorkspaceRegistry {
    pub fn inspect_git_worktree(&self, project_id: &str) -> GitWorktreeInspection {
        let Ok(root) = self.registered_git_root(project_id) else {
            return unavailable("project_unavailable");
        };
        let Ok(repo) = Repository::open(&root) else {
            return unavailable("not_a_git_repository");
        };
        // Repository::open can discover a parent repository. Never inspect
        // anything outside the exact human-selected workspace root.
        let Some(workdir) = repo.workdir() else {
            return unavailable("bare_repository_unsupported");
        };
        let Ok(actual) = fs::canonicalize(workdir) else {
            return unavailable("worktree_unverifiable");
        };
        if actual != root {
            return unavailable("repository_outside_selected_root");
        }

        let head = repo.head().ok();
        let local_oid = head.as_ref().and_then(|h| h.target());
        let branch = head
            .as_ref()
            .and_then(|h| h.shorthand())
            .map(ToOwned::to_owned);
        let mut out = unavailable("local_repository_inspected");
        out.branch = branch;
        out.local_head = local_oid.map(|oid| oid.to_string());

        if repo.head_detached().unwrap_or(false) {
            out.upstream_comparison = "detached_head";
        } else if let (Some(ref branch_name), Some(local)) = (&out.branch, local_oid) {
            if let Ok(branch) = repo.find_branch(branch_name, BranchType::Local) {
                if let Ok(upstream) = branch.upstream() {
                    let tracking_oid = upstream.get().target();
                    out.upstream_head = tracking_oid.map(|oid| oid.to_string());
                    if let Some(upstream_oid) = tracking_oid {
                        out.upstream_comparison = if upstream_oid == local {
                            "local_tracking_equal"
                        } else {
                            "local_tracking_differs"
                        };
                        if let Ok((ahead, behind)) = repo.graph_ahead_behind(local, upstream_oid) {
                            out.commits_ahead = Some(ahead);
                            out.commits_behind = Some(behind);
                        }
                    }
                }
            }
        }

        let mut options = StatusOptions::new();
        options
            .include_untracked(true)
            .recurse_untracked_dirs(true)
            .include_ignored(false)
            .renames_head_to_index(true)
            .renames_index_to_workdir(true);
        let Ok(statuses) = repo.statuses(Some(&mut options)) else {
            out.state = "git_status_unverifiable";
            return out;
        };

        for entry in statuses.iter() {
            let status = entry.status();
            if status == Status::CURRENT {
                continue;
            }
            let new_file = status.contains(Status::WT_NEW) || status.contains(Status::INDEX_NEW);
            let kind = if status.intersects(Status::WT_DELETED | Status::INDEX_DELETED) {
                "deleted"
            } else if status.intersects(Status::WT_RENAMED | Status::INDEX_RENAMED) {
                "renamed"
            } else if new_file {
                "new"
            } else if status.intersects(Status::WT_MODIFIED | Status::INDEX_MODIFIED) {
                "modified"
            } else if status.intersects(Status::CONFLICTED) {
                "conflict"
            } else {
                "other_change"
            };
            let stage = if status.intersects(
                Status::INDEX_NEW
                    | Status::INDEX_MODIFIED
                    | Status::INDEX_DELETED
                    | Status::INDEX_RENAMED
                    | Status::INDEX_TYPECHANGE,
            ) {
                "staged_or_index"
            } else {
                "worktree"
            };
            if status.contains(Status::WT_NEW) {
                out.untracked_count += 1;
            } else {
                out.modified_or_staged_count += 1;
            }
            if out.displayed_changes.len() < MAX_VISIBLE_CHANGES {
                if let Some(path) = entry.path() {
                    out.displayed_changes.push(GitFileChange {
                        relative_path: path.to_owned(),
                        kind,
                        stage,
                    });
                }
            } else {
                out.truncated = true;
            }
        }
        out.worktree = if out.modified_or_staged_count + out.untracked_count == 0 {
            "clean"
        } else {
            "dirty"
        };
        out
    }
}
