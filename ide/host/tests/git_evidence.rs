use lu_ide_workspace::{IdeError, IdeWorkspaceRegistry, NativeDirectoryPicker};
use std::{fs, path::{Path, PathBuf}};

struct Picker(Option<PathBuf>);
impl NativeDirectoryPicker for Picker {
    fn choose_project_directory(&mut self) -> Result<Option<PathBuf>, IdeError> {
        Ok(self.0.take())
    }
}
fn fixture() -> (PathBuf, IdeWorkspaceRegistry, String, git2::Repository) {
    let root = std::env::temp_dir().join(format!(
        "lu-ide-git-evidence-{}-{:016x}", std::process::id(), rand::random::<u64>()));
    let project = root.join("project");
    fs::create_dir_all(&project).unwrap();
    let repo = git2::Repository::init(&project).unwrap();
    let registry = IdeWorkspaceRegistry::open(root.join("catalog.json")).unwrap();
    let id = registry.register_from_native_picker(&mut Picker(Some(project)))
        .unwrap().unwrap().project_id;
    (root, registry, id, repo)
}
fn commit_file(repo: &git2::Repository, project: &Path, content: &str) {
    fs::write(project.join("tracked.txt"), content).unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(Path::new("tracked.txt")).unwrap();
    index.write().unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    let signature = git2::Signature::now("IDE fixture", "fixture@example.invalid").unwrap();
    let parent = repo.head().ok().and_then(|head| head.peel_to_commit().ok());
    let parents: Vec<&git2::Commit<'_>> = parent.iter().collect();
    repo.commit(Some("HEAD"), &signature, &signature, "fixture commit", &tree, &parents).unwrap();
}

#[test]
fn staged_and_worktree_patches_are_separate_and_read_only() {
    let (root, registry, id, repo) = fixture();
    let project = root.join("project");
    commit_file(&repo, &project, "alpha\n");
    fs::write(project.join("tracked.txt"), "beta longer\n").unwrap();
    fs::write(project.join("new.txt"), "SECRET UNTRACKED CONTENT\n").unwrap();

    let unstaged = registry.inspect_git_diff(&id, "worktree").unwrap();
    assert_eq!(unstaged.scope, "worktree");
    assert_eq!(unstaged.files_changed, 1);
    assert!(unstaged.patch.contains("-alpha"));
    assert!(unstaged.patch.contains("+beta longer"));
    assert!(!unstaged.patch.contains("SECRET UNTRACKED CONTENT"));
    assert!(unstaged.untracked_excluded);
    assert!(!unstaged.truncated);
    assert!(unstaged.patch_bytes <= 64 * 1024);

    let staged_empty = registry.inspect_git_diff(&id, "staged").unwrap();
    assert_eq!(staged_empty.files_changed, 0);
    assert!(staged_empty.patch.is_empty());

    let mut index = repo.index().unwrap();
    index.add_path(Path::new("tracked.txt")).unwrap();
    index.write().unwrap();
    let staged = registry.inspect_git_diff(&id, "staged").unwrap();
    assert_eq!(staged.files_changed, 1);
    assert!(staged.patch.contains("+beta longer"));
    let worktree_empty = registry.inspect_git_diff(&id, "worktree").unwrap();
    assert_eq!(worktree_empty.files_changed, 0);
    assert!(!repo.is_bare());
    assert!(project.join("new.txt").exists());
    drop(repo);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn history_is_bounded_to_local_head_and_scope_is_validated() {
    let (root, registry, id, repo) = fixture();
    assert_eq!(registry.inspect_git_diff(&id, "all").unwrap_err(),
        "INVALID_GIT_DIFF_SCOPE");
    assert!(registry.inspect_git_history(&id).unwrap().commits.is_empty());
    let project = root.join("project");
    for i in 0..34 {
        commit_file(&repo, &project, &format!("line {i}\n"));
    }
    let history = registry.inspect_git_history(&id).unwrap();
    assert_eq!(history.commits.len(), 30);
    assert!(history.truncated);
    assert_eq!(history.live_remote, "not_queried");
    assert!(history.commits.iter().all(|c|
        c.commit_sha.len() == 40 && c.summary == "fixture commit"));
    registry.close(&id).unwrap();
    assert_eq!(registry.inspect_git_history(&id).unwrap_err(),
        "PROJECT_NOT_OPEN");
    drop(repo);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn parent_repo_is_never_searched_through_child_project() {
    let root = std::env::temp_dir().join(format!(
        "lu-ide-parent-evidence-{}-{:016x}", std::process::id(), rand::random::<u64>()));
    let project = root.join("child");
    fs::create_dir_all(&project).unwrap();
    let repo = git2::Repository::init(&root).unwrap();
    let registry = IdeWorkspaceRegistry::open(root.join("catalog.json")).unwrap();
    let id = registry.register_from_native_picker(&mut Picker(Some(project)))
        .unwrap().unwrap().project_id;
    assert!(matches!(registry.inspect_git_history(&id),
        Err("GIT_REPOSITORY_OUTSIDE_PROJECT") | Err("GIT_REPOSITORY_UNAVAILABLE")));
    drop(repo);
    fs::remove_dir_all(root).unwrap();
}
