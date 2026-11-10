//! IDE Group 6A-01/02: bounded, read-only local branch and linked-worktree inventory.
//! Only an OS-picker-registered project ID may select a repository. No Git
//! CLI, hooks, ref updates, checkout, remote requests or linked-root grants.
use crate::IdeWorkspaceRegistry;
use git2::{BranchType, Repository};
use serde::Serialize;
use std::{fs, path::Path};

const MAX_BRANCHES: usize = 96;
const MAX_WORKTREES: usize = 48;
const MAX_LABEL_BYTES: usize = 200;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalBranchEntry {
    pub name: String,
    pub tip_sha: Option<String>,
    pub current: bool,
    pub upstream_name: Option<String>,
    pub upstream_sha: Option<String>,
    pub ahead: Option<usize>,
    pub behind: Option<usize>,
    pub upstream_state: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkedWorktreeEntry {
    pub name: String,
    pub current_project: bool,
    pub path_state: &'static str,
    pub administrative_state: &'static str,
    pub registered_for_access: bool,
    pub absolute_path_exposed: bool,
    pub locked_state: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitRepositoryInventory {
    pub schema_version: u32,
    pub project_id: String,
    pub state: &'static str,
    pub repository_kind: &'static str,
    pub head_state: &'static str,
    pub current_branch: Option<String>,
    pub current_head_sha: Option<String>,
    pub branches: Vec<LocalBranchEntry>,
    pub branches_truncated: bool,
    pub worktrees: Vec<LinkedWorktreeEntry>,
    pub worktrees_truncated: bool,
    pub remote_queried: bool,
    pub git_process_started: bool,
    pub refs_or_files_modified: bool,
    pub linked_worktree_access_granted: bool,
}

fn safe_label(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= MAX_LABEL_BYTES
        && !s.chars().any(char::is_control)
}

fn initial(project_id: &str, state: &'static str) -> GitRepositoryInventory {
    GitRepositoryInventory {
        schema_version: 1,
        project_id: project_id.to_owned(),
        state,
        repository_kind: "unknown",
        head_state: "unverifiable",
        current_branch: None,
        current_head_sha: None,
        branches: Vec::new(),
        branches_truncated: false,
        worktrees: Vec::new(),
        worktrees_truncated: false,
        remote_queried: false,
        git_process_started: false,
        refs_or_files_modified: false,
        linked_worktree_access_granted: false,
    }
}

fn repo_at_selected_root(root: &Path) -> Result<Repository, &'static str> {
    let repo = Repository::open(root).map_err(|_| "not_a_git_repository")?;
    let workdir = repo.workdir().ok_or("bare_repository_not_registered")?;
    let actual = fs::canonicalize(workdir).map_err(|_| "worktree_unverifiable")?;
    if actual != root {
        return Err("repository_outside_selected_root");
    }
    Ok(repo)
}

impl IdeWorkspaceRegistry {
    /// No caller-selected ref, URL, path or command. External linked worktree
    /// locations are never returned or opened, even when libgit2 lists them.
    pub fn inspect_git_repository_inventory(&self, project_id: &str) -> GitRepositoryInventory {
        let Ok(root) = self.registered_git_root(project_id) else {
            return initial(project_id, "registered_project_unavailable");
        };
        let Ok(repo) = repo_at_selected_root(&root) else {
            return initial(project_id, "repository_not_eligible");
        };
        let mut out = initial(project_id, "local_inventory_inspected");
        out.repository_kind = if repo.is_worktree() { "linked_worktree" } else { "main_worktree" };
        let detached = repo.head_detached().unwrap_or(false);
        let unborn = repo.head().is_err() && repo.is_empty().unwrap_or(false);
        out.head_state = if detached {
            "detached"
        } else if unborn {
            "unborn"
        } else if repo.head().is_ok() {
            "attached"
        } else {
            "unverifiable"
        };
        if let Ok(head) = repo.head() {
            out.current_head_sha = head.target().map(|id| id.to_string());
            if !detached {
                out.current_branch = head.shorthand().filter(|s| safe_label(s)).map(str::to_owned);
            }
        }
        // Bound enumeration before performing expensive ancestry comparisons.
        let iter = match repo.branches(Some(BranchType::Local)) {
            Ok(iter) => iter,
            Err(_) => {
                out.state = "local_branches_unverifiable";
                return out;
            }
        };
        for entry in iter {
            if out.branches.len() >= MAX_BRANCHES {
                out.branches_truncated = true;
                break;
            }
            let Ok((branch, _kind)) = entry else {
                out.state = "local_branches_partially_unverifiable";
                continue;
            };
            let Ok(Some(name)) = branch.name() else {
                out.state = "local_branches_partially_unverifiable";
                continue;
            };
            if !safe_label(name) {
                out.state = "local_branches_partially_unverifiable";
                continue;
            }
            let tip = branch.get().target();
            let mut row = LocalBranchEntry {
                name: name.to_owned(),
                tip_sha: tip.map(|id| id.to_string()),
                current: branch.is_head(),
                upstream_name: None,
                upstream_sha: None,
                ahead: None,
                behind: None,
                upstream_state: "not_configured_or_unverifiable",
            };
            if let Ok(upstream) = branch.upstream() {
                row.upstream_state = "stored_tracking_only";
                row.upstream_name = upstream.name().ok().flatten().filter(|s| safe_label(s)).map(str::to_owned);
                let upstream_tip = upstream.get().target();
                row.upstream_sha = upstream_tip.map(|id| id.to_string());
                if let (Some(local), Some(remote)) = (tip, upstream_tip) {
                    if let Ok((ahead, behind)) = repo.graph_ahead_behind(local, remote) {
                        row.ahead = Some(ahead);
                        row.behind = Some(behind);
                    } else {
                        row.upstream_state = "tracking_ancestry_unverifiable";
                    }
                }
            }
            out.branches.push(row);
        }
        out.branches.sort_by(|a, b| a.name.cmp(&b.name));

        // libgit2 worktree names refer to administrative records. Never
        // auto-register these locations or inspect files inside them.
        match repo.worktrees() {
            Ok(names) => {
                for name in names.iter().flatten() {
                    if out.worktrees.len() >= MAX_WORKTREES {
                        out.worktrees_truncated = true;
                        break;
                    }
                    if !safe_label(name) {
                        out.state = "linked_worktrees_partially_unverifiable";
                        continue;
                    }
                    let row = if let Ok(worktree) = repo.find_worktree(name) {
                        // Reading libgit2's stored path is NOT authorization to
                        // stat, validate or open it. Only compare it with our
                        // already checked registered root, without disk I/O.
                        let current = worktree.path() == root.as_path();
                        LinkedWorktreeEntry {
                            name: name.to_owned(),
                            current_project: current,
                            path_state: if current { "registered_root_only" } else { "unregistered_path_not_inspected" },
                            administrative_state: "listed_not_validated",
                            registered_for_access: false,
                            absolute_path_exposed: false,
                            locked_state: "not_assessed",
                        }
                    } else {
                        LinkedWorktreeEntry {
                            name: name.to_owned(),
                            current_project: false,
                            path_state: "unverifiable",
                            administrative_state: "invalid_or_stale",
                            registered_for_access: false,
                            absolute_path_exposed: false,
                            locked_state: "not_assessed",
                        }
                    };
                    out.worktrees.push(row);
                }
                out.worktrees.sort_by(|a, b| a.name.cmp(&b.name));
            }
            Err(_) => {
                out.state = "linked_worktrees_unverifiable";
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{IdeError, NativeDirectoryPicker};
    use git2::Signature;
    use std::path::PathBuf;

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
        let base = std::env::temp_dir().join(format!("ide-g06a-{:032x}", rand::random::<u128>()));
        let dir = base.join("project");
        fs::create_dir_all(&dir).unwrap();
        let repo = Repository::init(&dir).unwrap();
        fs::write(dir.join("source.rs"), b"original\n").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("source.rs")).unwrap();
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let sig = Signature::now("IDE Fixture", "fixture@localhost.invalid").unwrap();
        let initial = repo.commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[]).unwrap();
        let parent = repo.find_commit(initial).unwrap();
        repo.branch("feature", &parent, false).unwrap();
        drop(parent);
        drop(tree);
        drop(index);
        drop(repo);
        let registry = IdeWorkspaceRegistry::open(base.join("catalog.json")).unwrap();
        let id = registry.register_from_native_picker(&mut Picker(dir.clone())).unwrap().unwrap().project_id;
        (Fixture(base), registry, id, dir)
    }
    #[test]
    fn branch_inventory_is_local_bounded_and_nonexecuting() {
        let (_temp, registry, id, _dir) = fixture();
        let result = registry.inspect_git_repository_inventory(&id);
        assert_eq!(result.state, "local_inventory_inspected");
        assert_eq!(result.repository_kind, "main_worktree");
        assert_eq!(result.head_state, "attached");
        assert!(result.branches.iter().any(|b| b.name == "feature" && !b.current));
        assert!(result.branches.iter().any(|b| b.current && b.tip_sha.is_some()));
        assert!(!result.git_process_started && !result.remote_queried);
        assert!(!result.refs_or_files_modified && !result.linked_worktree_access_granted);
    }
    #[test]
    fn unregistered_project_does_not_expose_repository_data() {
        let (_temp, registry, _id, _dir) = fixture();
        let result = registry.inspect_git_repository_inventory("ide-unknown");
        assert_eq!(result.state, "registered_project_unavailable");
        assert!(result.branches.is_empty() && result.worktrees.is_empty());
    }
    #[test]
    fn linked_worktree_is_listed_without_authorizing_external_root() {
        let (temp, registry, id, dir) = fixture();
        let repo = Repository::open(&dir).unwrap();
        let other = temp.0.join("not-registered-linked");
        repo.worktree("secondary", &other, None).unwrap();
        let result = registry.inspect_git_repository_inventory(&id);
        assert!(result.worktrees.iter().any(|w|
            w.name == "secondary" && !w.registered_for_access && !w.absolute_path_exposed
        ));
        assert!(!result.linked_worktree_access_granted);
    }
}
