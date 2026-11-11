//! IDE G06C-01: local-only, read-only branch operation review.
//!
//! A branch name is a selector, never a ref-update target, shell argument,
//! filesystem path, network request, authorization or operation grant.
//! This module deliberately has NO merge, rebase, cherry-pick, revert or stash
//! executor. There is no trusted transition from its digest to execution.
use crate::IdeWorkspaceRegistry;
use git2::{BranchType, Oid, Reference, Repository, RepositoryState, Status, StatusOptions};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

const MAX_BRANCH_BYTES: usize = 160;
const MAX_DIRTY_COUNT: usize = 512;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BranchOperationKind {
    Merge,
    Rebase,
    CherryPick,
    Revert,
    Stash,
}

impl BranchOperationKind {
    fn name(self) -> &'static str {
        match self {
            Self::Merge => "merge",
            Self::Rebase => "rebase",
            Self::CherryPick => "cherry_pick",
            Self::Revert => "revert",
            Self::Stash => "stash",
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchOperationReview {
    pub schema_version: u32,
    pub project_id: String,
    pub operation: BranchOperationKind,
    pub current_branch: String,
    pub current_head: String,
    pub selected_local_branch: Option<String>,
    pub selected_tip: Option<String>,
    pub merge_base: Option<String>,
    pub current_only_commits: Option<usize>,
    pub selected_only_commits: Option<usize>,
    pub relationship: &'static str,
    pub git_state: String,
    pub index_conflicts: bool,
    pub index_lock_present: bool,
    pub dirty_entries: usize,
    pub dirty_truncated: bool,
    pub untracked_present: bool,
    pub worktree_clean: bool,
    pub blockers: Vec<&'static str>,
    /// A read-only evidence identity; explicitly NEVER a grant or capability.
    pub evidence_sha256: String,
    pub operation_authorized: bool,
    pub operation_executed: bool,
    pub execution_route_available: bool,
    pub remote_queried: bool,
    pub git_process_started: bool,
    pub repository_modified: bool,
}

fn safe_local_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_BRANCH_BYTES
        && name != "HEAD"
        && !name.starts_with("refs/")
        && name.is_ascii()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'-' | b'_' | b'.'))
        && !name
            .split('/')
            .any(|part| part.is_empty() || part.starts_with('.'))
        && Reference::is_valid_name(&format!("refs/heads/{name}"))
}

fn exact_repo(
    registry: &IdeWorkspaceRegistry,
    project_id: &str,
) -> Result<Repository, &'static str> {
    let root = registry
        .registered_git_root(project_id)
        .map_err(|_| "G06C_PROJECT_NOT_OPEN")?;
    let repo = Repository::open(&root).map_err(|_| "G06C_REPOSITORY_UNAVAILABLE")?;
    if repo.is_bare() || repo.is_worktree() {
        return Err("G06C_REQUIRES_ORDINARY_LOCAL_REPO");
    }
    let workdir = repo.workdir().ok_or("G06C_REQUIRES_WORKDIR")?;
    if fs::canonicalize(workdir).map_err(|_| "G06C_WORKDIR_UNVERIFIABLE")? != root {
        return Err("G06C_REPOSITORY_OUTSIDE_PROJECT");
    }
    // A linked worktree's .git is an indirection; never follow it to a
    // separately registered checkout or acquire its authority implicitly.
    let dotgit = root.join(".git");
    let meta = fs::symlink_metadata(&dotgit).map_err(|_| "G06C_GITDIR_UNAVAILABLE")?;
    if !meta.is_dir() || meta.file_type().is_symlink() {
        return Err("G06C_LINKED_OR_ALIASED_GITDIR");
    }
    if fs::canonicalize(&dotgit).map_err(|_| "G06C_GITDIR_UNVERIFIABLE")?
        != fs::canonicalize(repo.path()).map_err(|_| "G06C_GITDIR_UNVERIFIABLE")?
    {
        return Err("G06C_EXTERNAL_GITDIR");
    }
    Ok(repo)
}

fn dirty_state(repo: &Repository) -> Result<(usize, bool, bool), &'static str> {
    let mut options = StatusOptions::new();
    options
        .include_untracked(true)
        .recurse_untracked_dirs(false)
        .include_ignored(false)
        .renames_head_to_index(false)
        .renames_index_to_workdir(false);
    let entries = repo
        .statuses(Some(&mut options))
        .map_err(|_| "G06C_STATUS_UNVERIFIABLE")?;
    let mut count = 0usize;
    let mut untracked = false;
    for entry in entries.iter() {
        if entry.status() != Status::CURRENT {
            count = count.saturating_add(1);
            untracked |= entry
                .status()
                .intersects(Status::WT_NEW | Status::INDEX_NEW);
        }
    }
    Ok((
        count.min(MAX_DIRTY_COUNT),
        count > MAX_DIRTY_COUNT,
        untracked,
    ))
}

fn digest(parts: &[&str]) -> String {
    let mut hash = Sha256::new();
    hash.update(b"lu-ide-g06c01-local-branch-review-v1\n");
    for part in parts {
        hash.update((part.len() as u64).to_le_bytes());
        hash.update(part.as_bytes());
    }
    format!("{:x}", hash.finalize())
}

fn local_commit(repo: &Repository, name: &str) -> Result<Oid, &'static str> {
    let branch = repo
        .find_branch(name, BranchType::Local)
        .map_err(|_| "G06C_LOCAL_BRANCH_NOT_FOUND")?;
    let direct = branch
        .get()
        .target()
        .ok_or("G06C_LOCAL_BRANCH_NOT_DIRECT")?;
    repo.find_commit(direct)
        .map_err(|_| "G06C_LOCAL_BRANCH_NOT_COMMIT")?;
    Ok(direct)
}

impl IdeWorkspaceRegistry {
    /// Observe one local operation candidate. Does not grant permission to
    /// stage, checkout, update a ref, stash, mutate an index, or run Git.
    pub fn review_local_branch_operation(
        &self,
        project_id: &str,
        operation: BranchOperationKind,
        selected_branch: Option<&str>,
    ) -> Result<BranchOperationReview, &'static str> {
        let selection = match (operation, selected_branch) {
            (BranchOperationKind::Stash, None) => None,
            (BranchOperationKind::Stash, Some(_)) => return Err("G06C_STASH_TARGET_DENIED"),
            (_, Some(branch)) if safe_local_name(branch) => Some(branch),
            (_, Some(_)) => return Err("G06C_INVALID_LOCAL_BRANCH"),
            (_, None) => return Err("G06C_LOCAL_BRANCH_REQUIRED"),
        };
        let repo = exact_repo(self, project_id)?;
        if repo.head_detached().unwrap_or(true) {
            return Err("G06C_DETACHED_HEAD_DENIED");
        }
        let head_ref = repo.head().map_err(|_| "G06C_HEAD_UNAVAILABLE")?;
        let raw_name = head_ref.name().ok_or("G06C_HEAD_NOT_LOCAL_REF")?;
        let name = raw_name
            .strip_prefix("refs/heads/")
            .ok_or("G06C_HEAD_NOT_LOCAL_REF")?;
        if !safe_local_name(name) {
            return Err("G06C_CURRENT_BRANCH_UNSUPPORTED");
        }
        let head = head_ref.target().ok_or("G06C_HEAD_NOT_DIRECT")?;
        repo.find_commit(head).map_err(|_| "G06C_HEAD_NOT_COMMIT")?;
        let index = repo.index().map_err(|_| "G06C_INDEX_UNVERIFIABLE")?;
        let conflicted = index.has_conflicts();
        let index_lock = Path::new(repo.path()).join("index.lock").exists();
        let (dirty, truncated, untracked) = dirty_state(&repo)?;
        let mut blockers = Vec::new();
        if repo.state() != RepositoryState::Clean {
            blockers.push("git_operation_in_progress");
        }
        if repo.is_shallow() {
            blockers.push("shallow_history_unverified");
        }
        if conflicted {
            blockers.push("index_conflicts");
        }
        if index_lock {
            blockers.push("git_index_lock_requires_manual_review");
        }
        if truncated {
            blockers.push("too_many_changed_entries");
        }

        let mut selected_tip = None;
        let mut common_base = None;
        let mut current_only = None;
        let mut target_only = None;
        let relationship;

        if let Some(other_name) = selection {
            let other = local_commit(&repo, other_name)?;
            selected_tip = Some(other.to_string());
            let base = repo.merge_base(head, other).ok();
            common_base = base.map(|id| id.to_string());
            if let Ok((ahead, behind)) = repo.graph_ahead_behind(head, other) {
                current_only = Some(ahead);
                target_only = Some(behind);
            }
            relationship = if name == other_name {
                blockers.push("same_branch");
                "same_branch"
            } else if head == other {
                blockers.push("same_commit");
                "same_commit"
            } else if base.is_none() {
                blockers.push("unrelated_histories");
                "unrelated_histories"
            } else if base == Some(head) {
                "selected_descends_from_current"
            } else if base == Some(other) {
                "current_descends_from_selected"
            } else {
                "diverged"
            };
            match operation {
                BranchOperationKind::CherryPick | BranchOperationKind::Revert => {
                    let commit = repo
                        .find_commit(other)
                        .map_err(|_| "G06C_SELECTED_COMMIT_UNAVAILABLE")?;
                    if commit.parent_count() != 1 {
                        blockers.push("selected_root_or_merge_commit");
                    }
                    if operation == BranchOperationKind::Revert
                        && !repo.graph_descendant_of(head, other).unwrap_or(false)
                    {
                        blockers.push("revert_commit_not_in_current_history");
                    }
                }
                _ => {}
            }
            if dirty != 0 {
                blockers.push("worktree_or_index_dirty");
            }
        } else {
            relationship = "not_applicable";
            if dirty == 0 {
                blockers.push("nothing_to_stash");
            }
            if untracked {
                // Stashing untracked/ignored files is a distinct destructive
                // policy choice; this review does not silently include them.
                blockers.push("untracked_stash_policy_unset");
            }
        }
        let selected_sha = selected_tip.as_deref().unwrap_or("");
        let base_sha = common_base.as_deref().unwrap_or("");
        let dirty_text = dirty.to_string();
        let flags = format!(
            "state={:?};conflict={conflicted};lock={index_lock};dirty={dirty};truncated={truncated};untracked={untracked};shallow={}",
            repo.state(),
            repo.is_shallow()
        );
        let evidence_sha256 = digest(&[
            project_id,
            operation.name(),
            name,
            &head.to_string(),
            selection.unwrap_or(""),
            selected_sha,
            base_sha,
            relationship,
            &dirty_text,
            &flags,
        ]);
        Ok(BranchOperationReview {
            schema_version: 1,
            project_id: project_id.to_owned(),
            operation,
            current_branch: name.to_owned(),
            current_head: head.to_string(),
            selected_local_branch: selection.map(str::to_owned),
            selected_tip,
            merge_base: common_base,
            current_only_commits: current_only,
            selected_only_commits: target_only,
            relationship,
            git_state: format!("{:?}", repo.state()),
            index_conflicts: conflicted,
            index_lock_present: index_lock,
            dirty_entries: dirty,
            dirty_truncated: truncated,
            untracked_present: untracked,
            worktree_clean: dirty == 0 && !truncated,
            blockers,
            evidence_sha256,
            operation_authorized: false,
            operation_executed: false,
            execution_route_available: false,
            remote_queried: false,
            git_process_started: false,
            repository_modified: false,
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

    struct Fixture {
        base: PathBuf,
        registry: IdeWorkspaceRegistry,
        id: String,
        root: PathBuf,
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.base);
        }
    }

    fn fixture() -> Fixture {
        let base = std::env::temp_dir().join(format!("ide-g06c01-{:032x}", rand::random::<u128>()));
        let root = base.join("repo");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("tracked.txt"), b"initial\n").unwrap();
        fs::write(root.join("unrelated.keep"), b"keep me\n").unwrap();
        let repo = Repository::init(&root).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("tracked.txt")).unwrap();
        index.add_path(Path::new("unrelated.keep")).unwrap();
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let signature = Signature::now("G06C fixture", "g06c@localhost.invalid").unwrap();
        let first = repo
            .commit(Some("HEAD"), &signature, &signature, "initial", &tree, &[])
            .unwrap();
        drop(tree);
        let base_commit = repo.find_commit(first).unwrap();
        repo.branch("feature", &base_commit, false).unwrap();
        let old_tree = base_commit.tree().unwrap();
        let mut builder = repo.treebuilder(Some(&old_tree)).unwrap();
        let new_blob = repo.blob(b"new feature\n").unwrap();
        builder.insert("tracked.txt", new_blob, 0o100644).unwrap();
        let new_tree_oid = builder.write().unwrap();
        let new_tree = repo.find_tree(new_tree_oid).unwrap();
        repo.commit(
            Some("refs/heads/feature"),
            &signature,
            &signature,
            "feature change",
            &new_tree,
            &[&base_commit],
        )
        .unwrap();
        drop(new_tree);
        drop(builder);
        drop(old_tree);
        drop(base_commit);
        drop(index);
        drop(repo);
        let registry = IdeWorkspaceRegistry::open(base.join("catalog.json")).unwrap();
        let id = registry
            .register_from_native_picker(&mut Picker(root.clone()))
            .unwrap()
            .unwrap()
            .project_id;
        Fixture {
            base,
            registry,
            id,
            root,
        }
    }

    fn snapshot(root: &Path) -> (Vec<u8>, Vec<u8>, Vec<u8>, String) {
        let repo = Repository::open(root).unwrap();
        let head_sha = repo.head().unwrap().target().unwrap().to_string();
        (
            fs::read(root.join("tracked.txt")).unwrap(),
            fs::read(root.join("unrelated.keep")).unwrap(),
            fs::read(root.join(".git/index")).unwrap(),
            head_sha,
        )
    }

    #[test]
    fn merge_rebase_cherry_pick_preview_real_graph_without_modification() {
        let x = fixture();
        let before = snapshot(&x.root);
        for operation in [
            BranchOperationKind::Merge,
            BranchOperationKind::Rebase,
            BranchOperationKind::CherryPick,
        ] {
            let p = x
                .registry
                .review_local_branch_operation(&x.id, operation, Some("feature"))
                .unwrap();
            assert_eq!(p.relationship, "selected_descends_from_current");
            assert_eq!(p.current_only_commits, Some(0));
            assert_eq!(p.selected_only_commits, Some(1));
            assert_eq!(p.merge_base.as_deref(), Some(p.current_head.as_str()));
            assert!(p.blockers.is_empty());
            assert!(p.worktree_clean);
            assert_eq!(p.evidence_sha256.len(), 64);
            assert!(!p.operation_authorized);
            assert!(!p.operation_executed && !p.execution_route_available);
            assert!(!p.remote_queried && !p.repository_modified && !p.git_process_started);
        }
        assert_eq!(before, snapshot(&x.root));
        assert!(!x.root.join(".git/index.lock").exists());
    }

    #[test]
    fn refuse_nonlocal_name_detached_and_same_branch() {
        let x = fixture();
        for other in [
            "../feature",
            "refs/heads/feature",
            "origin/../feature",
            "feature.lock",
            "feature\0bad",
            "feature;touch-pwned",
            "HEAD",
            "feature@{1}",
            "",
            "feature\\evil",
        ] {
            assert!(
                x.registry
                    .review_local_branch_operation(&x.id, BranchOperationKind::Merge, Some(other),)
                    .is_err(),
                "{other:?}"
            );
        }
        assert!(x
            .registry
            .review_local_branch_operation(
                &x.id,
                BranchOperationKind::Merge,
                Some("origin/feature"),
            )
            .is_err());
        assert_eq!(
            x.registry
                .review_local_branch_operation(&x.id, BranchOperationKind::Merge, None)
                .unwrap_err(),
            "G06C_LOCAL_BRANCH_REQUIRED"
        );
        assert_eq!(
            x.registry
                .review_local_branch_operation(&x.id, BranchOperationKind::Stash, Some("feature"))
                .unwrap_err(),
            "G06C_STASH_TARGET_DENIED"
        );
        let current = Repository::open(&x.root)
            .unwrap()
            .head()
            .unwrap()
            .shorthand()
            .unwrap()
            .to_owned();
        let review = x
            .registry
            .review_local_branch_operation(&x.id, BranchOperationKind::Merge, Some(&current))
            .unwrap();
        assert!(review.blockers.contains(&"same_branch"));
    }

    #[test]
    fn stash_dirty_guard_and_foreign_lock_never_deleted() {
        let x = fixture();
        let clean = x
            .registry
            .review_local_branch_operation(&x.id, BranchOperationKind::Stash, None)
            .unwrap();
        assert!(clean.blockers.contains(&"nothing_to_stash"));
        fs::write(x.root.join("tracked.txt"), b"edited, unsaved\n").unwrap();
        fs::write(x.root.join("scratch.tmp"), b"local untracked\n").unwrap();
        let lock = x.root.join(".git/index.lock");
        fs::write(&lock, b"FOREIGN_GIT_OWNER").unwrap();
        let locked = x
            .registry
            .review_local_branch_operation(&x.id, BranchOperationKind::Merge, Some("feature"))
            .unwrap();
        assert!(locked.blockers.contains(&"worktree_or_index_dirty"));
        assert!(locked
            .blockers
            .contains(&"git_index_lock_requires_manual_review"));
        assert!(locked.dirty_entries > 0 && locked.untracked_present);
        let stash = x
            .registry
            .review_local_branch_operation(&x.id, BranchOperationKind::Stash, None)
            .unwrap();
        assert!(stash.blockers.contains(&"untracked_stash_policy_unset"));
        assert_eq!(fs::read(lock).unwrap(), b"FOREIGN_GIT_OWNER");
        assert_eq!(
            fs::read(x.root.join("tracked.txt")).unwrap(),
            b"edited, unsaved\n"
        );
        assert!(!stash.operation_authorized);
    }

    #[test]
    fn revert_requires_commit_reachable_in_selected_history() {
        let x = fixture();
        let review = x
            .registry
            .review_local_branch_operation(&x.id, BranchOperationKind::Revert, Some("feature"))
            .unwrap();
        assert!(review
            .blockers
            .contains(&"revert_commit_not_in_current_history"));
        let original = snapshot(&x.root);
        assert_eq!(snapshot(&x.root), original);
    }

    #[test]
    fn registered_project_is_mandatory_no_parent_repo_leak() {
        let x = fixture();
        let err = x
            .registry
            .review_local_branch_operation(
                "unknown-project",
                BranchOperationKind::Merge,
                Some("feature"),
            )
            .unwrap_err();
        assert_eq!(err, "G06C_PROJECT_NOT_OPEN");
        let repo = Repository::open(&x.root).unwrap();
        let initial = repo.head().unwrap().target().unwrap();
        repo.set_head_detached(initial).unwrap();
        assert_eq!(
            x.registry
                .review_local_branch_operation(&x.id, BranchOperationKind::Stash, None)
                .unwrap_err(),
            "G06C_DETACHED_HEAD_DENIED"
        );
    }
}
