use lu_ide_workspace::{IdeError, IdeWorkspaceRegistry, NativeDirectoryPicker};
use std::{fs, path::{Path, PathBuf}};

struct Picker(Option<PathBuf>);
impl NativeDirectoryPicker for Picker {
    fn choose_project_directory(&mut self) -> Result<Option<PathBuf>, IdeError> {
        Ok(self.0.take())
    }
}

#[test]
fn native_git_status_reads_real_dirty_and_untracked_work_without_running_git() {
    let root = std::env::temp_dir().join(format!(
        "lu-ide-git-status-{}-{:016x}", std::process::id(), rand::random::<u64>()
    ));
    let project = root.join("project");
    fs::create_dir_all(&project).unwrap();
    let repo = git2::Repository::init(&project).unwrap();
    fs::write(project.join("tracked.txt"), b"original\n").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(Path::new("tracked.txt")).unwrap();
    index.write().unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    let author = git2::Signature::now("IDE fixture", "fixture@example.invalid").unwrap();
    repo.commit(Some("HEAD"), &author, &author, "initial fixture", &tree, &[]).unwrap();

    let registry = IdeWorkspaceRegistry::open(root.join("catalog.json")).unwrap();
    let id = registry.register_from_native_picker(&mut Picker(Some(project.clone())))
        .unwrap().unwrap().project_id;

    let clean = registry.inspect_git_worktree(&id);
    assert_eq!(clean.state, "local_repository_inspected");
    assert_eq!(clean.worktree, "clean");
    assert_eq!(clean.live_remote, "not_queried");
    assert_eq!(clean.modified_or_staged_count, 0);
    assert_eq!(clean.untracked_count, 0);

    fs::write(project.join("tracked.txt"), b"modified\n").unwrap();
    fs::write(project.join("untracked.txt"), b"new\n").unwrap();
    let dirty = registry.inspect_git_worktree(&id);
    assert_eq!(dirty.worktree, "dirty");
    assert_eq!(dirty.modified_or_staged_count, 1);
    assert_eq!(dirty.untracked_count, 1);
    assert!(dirty.displayed_changes.iter().any(|c|
        c.relative_path == "tracked.txt" && c.kind == "modified"));
    assert!(dirty.displayed_changes.iter().any(|c|
        c.relative_path == "untracked.txt" && c.kind == "new"));

    registry.close(&id).unwrap();
    assert_eq!(registry.inspect_git_worktree(&id).state, "project_unavailable");
    drop(repo);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn native_git_inspection_refuses_parent_repository_discovery() {
    let root = std::env::temp_dir().join(format!(
        "lu-ide-parent-git-{}-{:016x}", std::process::id(), rand::random::<u64>()
    ));
    fs::create_dir_all(root.join("project")).unwrap();
    let repo = git2::Repository::init(&root).unwrap();
    let registry = IdeWorkspaceRegistry::open(root.join("catalog.json")).unwrap();
    let id = registry.register_from_native_picker(&mut Picker(Some(root.join("project"))))
        .unwrap().unwrap().project_id;
    let status = registry.inspect_git_worktree(&id);
    assert!(matches!(status.state,
        "repository_outside_selected_root" | "not_a_git_repository"));
    drop(repo);
    fs::remove_dir_all(root).unwrap();
}
