use lu_ide_workspace::{
    workspace_layout::{RememberedSource, WorkspaceLayout, WorkspaceLayoutStore},
    IdeError, IdeWorkspaceRegistry, NativeDirectoryPicker,
};
use std::{fs, path::PathBuf};

struct Picker(Option<PathBuf>);
impl NativeDirectoryPicker for Picker {
    fn choose_project_directory(&mut self) -> Result<Option<PathBuf>, IdeError> {
        Ok(self.0.take())
    }
}
fn fixture() -> (PathBuf, IdeWorkspaceRegistry, WorkspaceLayoutStore, WorkspaceLayout) {
    let root = std::env::temp_dir().join(format!(
        "lu-ide-b9-navigation-{}-{:016x}",
        std::process::id(),
        rand::random::<u64>()
    ));
    let project = root.join("project");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/main.rs"), b"fn main() {}\n").unwrap();
    let ide = IdeWorkspaceRegistry::open(root.join("projects.json")).unwrap();
    let project_id = ide
        .register_from_native_picker(&mut Picker(Some(project.clone())))
        .unwrap()
        .unwrap()
        .project_id;
    let source = ide.preview_source_file(&project_id, "src/main.rs").unwrap();
    let store = WorkspaceLayoutStore::open(root.join("private-navigation")).unwrap();
    let layout = WorkspaceLayout {
        schema_version: 1,
        project_id,
        files: vec![RememberedSource {
            relative_path: "src/main.rs".into(),
            saved_revision: source.revision,
            was_dirty: true,
        }],
        primary_path: Some("src/main.rs".into()),
        secondary_path: None,
        split_open: false,
        tab_window_start: 0,
    };
    (root, ide, store, layout)
}

#[test]
fn actual_disk_layout_roundtrips_only_after_registry_reopens() {
    let (root, ide, store, layout) = fixture();
    store.save(&ide, layout.clone()).unwrap();
    assert_eq!(store.read(&ide, &layout.project_id).unwrap(), Some(layout.clone()));
    let private_file = root.join("private-navigation")
        .join(format!("{}.json", layout.project_id));
    let content = fs::read_to_string(&private_file).unwrap();
    assert!(!content.contains("fn main()"));
    assert!(!content.contains("private draft"));
    assert!(!content.contains(root.to_str().unwrap()));
    ide.close(&layout.project_id).unwrap();
    assert_eq!(
        store.read(&ide, &layout.project_id).unwrap_err(),
        "B9_PROJECT_NOT_OPEN"
    );
    drop(ide);
    drop(store);
    let next = IdeWorkspaceRegistry::open(root.join("projects.json")).unwrap();
    let reopened = WorkspaceLayoutStore::open(root.join("private-navigation")).unwrap();
    assert_eq!(
        reopened.read(&next, &layout.project_id).unwrap_err(),
        "B9_PROJECT_NOT_OPEN"
    );
    next.reopen(&layout.project_id).unwrap();
    assert_eq!(reopened.read(&next, &layout.project_id).unwrap(), Some(layout));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn stale_saved_revision_is_refused_without_overwriting_last_good_layout() {
    let (root, ide, store, layout) = fixture();
    store.save(&ide, layout.clone()).unwrap();
    fs::write(root.join("project/src/main.rs"), b"fn main() { changed(); }\n").unwrap();
    assert_eq!(store.save(&ide, layout.clone()).unwrap_err(), "B9_SOURCE_CHANGED");
    assert_eq!(store.read(&ide, &layout.project_id).unwrap(), Some(layout));
    assert!(fs::read_to_string(root.join("project/src/main.rs"))
        .unwrap().contains("changed"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn explicit_forget_does_not_delete_project_source_or_draft_vault() {
    let (root, ide, store, layout) = fixture();
    store.save(&ide, layout.clone()).unwrap();
    let private_drafts = root.join("private-draft-recovery");
    fs::create_dir_all(&private_drafts).unwrap();
    fs::write(private_drafts.join("unrelated-checkpoint"), b"private draft").unwrap();
    assert_eq!(store.clear(&ide, &layout.project_id).unwrap(), true);
    assert_eq!(store.clear(&ide, &layout.project_id).unwrap(), false);
    assert!(store.read(&ide, &layout.project_id).unwrap().is_none());
    assert!(root.join("project/src/main.rs").exists());
    assert_eq!(
        fs::read(private_drafts.join("unrelated-checkpoint")).unwrap(),
        b"private draft"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn unregister_purges_layout_even_if_project_was_closed() {
    let (root, ide, store, layout) = fixture();
    store.save(&ide, layout.clone()).unwrap();
    ide.close(&layout.project_id).unwrap();
    assert_eq!(
        store.purge_for_unregister(&ide, &layout.project_id).unwrap(),
        true
    );
    ide.unregister(&layout.project_id).unwrap();
    assert!(!root.join("private-navigation")
        .join(format!("{}.json", layout.project_id)).exists());
    assert!(root.join("project/src/main.rs").exists());
    assert_eq!(
        store.purge_for_unregister(&ide, &layout.project_id).unwrap_err(),
        "B9_PROJECT_NOT_REGISTERED"
    );
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn symlinked_record_and_unsafe_private_dir_fail_closed() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let (root, ide, store, layout) = fixture();
    let record_path = root.join("private-navigation")
        .join(format!("{}.json", layout.project_id));
    fs::write(root.join("external.txt"), b"external record").unwrap();
    symlink(root.join("external.txt"), &record_path).unwrap();
    assert_eq!(
        store.read(&ide, &layout.project_id).unwrap_err(),
        "B9_UNTRUSTED_RECORD"
    );
    assert_eq!(
        store.save(&ide, layout.clone()).unwrap_err(),
        "B9_UNTRUSTED_RECORD"
    );
    fs::remove_file(record_path).unwrap();
    fs::set_permissions(root.join("private-navigation"),
        fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(
        store.save(&ide, layout).unwrap_err(), "B9_UNSAFE_STORAGE"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn deleted_source_cannot_be_cached_as_restart_authority() {
    let (root, ide, store, layout) = fixture();
    fs::remove_file(root.join("project/src/main.rs")).unwrap();
    assert_eq!(
        store.save(&ide, layout).unwrap_err(), "B9_SOURCE_UNAVAILABLE"
    );
    fs::remove_dir_all(root).unwrap();
}
