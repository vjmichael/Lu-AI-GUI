//! IDE-MERGE-02: carry forward DEV-01 large-project index safety contracts
//! through the current native Quick Open and bounded directory-page APIs.
//! Historical source_index.rs is intentionally NOT imported.
use lu_ide_workspace::{IdeError, IdeWorkspaceRegistry, NativeDirectoryPicker};
use std::{fs, path::PathBuf};

struct Picker(Option<PathBuf>);
impl NativeDirectoryPicker for Picker {
    fn choose_project_directory(&mut self) -> Result<Option<PathBuf>, IdeError> {
        Ok(self.0.take())
    }
}

fn fixture() -> (PathBuf, IdeWorkspaceRegistry, String) {
    let root = std::env::temp_dir().join(format!(
        "lu-ide-merge02-quick-open-{}-{:016x}",
        std::process::id(),
        rand::random::<u64>()
    ));
    let project = root.join("project");
    fs::create_dir_all(&project).unwrap();
    let registry = IdeWorkspaceRegistry::open(root.join("catalog.json")).unwrap();
    let id = registry
        .register_from_native_picker(&mut Picker(Some(project)))
        .unwrap()
        .unwrap()
        .project_id;
    (root, registry, id)
}

#[test]
fn native_quick_open_reaches_beyond_legacy_five_hundred_without_reading_sources() {
    let (root, registry, id) = fixture();
    let project = root.join("project");
    for n in 0..540 {
        fs::write(project.join(format!("other-{n:04}.rs")), b"contents must remain unread")
            .unwrap();
    }
    fs::write(project.join("find-late-target.rs"), b"never execute or open me").unwrap();
    let result = registry.quick_open_source_paths(&id, "find-late-target").unwrap();
    assert!(result.matches.iter().any(|m| m.relative_path == "find-late-target.rs"));
    assert!(result.entries_examined > 500);
    assert!(result.matches.len() <= 60);
    assert!(result.read_only);
    assert!(!result.source_files_read && !result.worktree_modified && !result.process_started);
    assert!(!project.join("unexpected-execution").exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn directory_pages_are_sorted_bounded_and_cannot_turn_cursors_into_path_authority() {
    let (root, registry, id) = fixture();
    let project = root.join("project");
    for n in 0..145 {
        fs::write(project.join(format!("item-{n:03}.rs")), b"safe").unwrap();
    }
    let first = registry.list_source_directory_page(&id, "", None).unwrap();
    assert_eq!(first.entries.len(), 120);
    assert_eq!(first.entries.first().unwrap().name, "item-000.rs");
    assert_eq!(first.entries.last().unwrap().name, "item-119.rs");
    assert!(first.truncated && !first.scan_limited && first.read_only);
    let cursor = first.next_after.as_deref().expect("bounded page must provide cursor");
    let second = registry.list_source_directory_page(&id, "", Some(cursor)).unwrap();
    assert_eq!(second.entries.len(), 25);
    assert_eq!(second.entries.first().unwrap().name, "item-120.rs");
    assert_eq!(second.entries.last().unwrap().name, "item-144.rs");
    assert!(!second.truncated && second.next_after.is_none() && second.read_only);
    for cursor in ["../secret", "src/other", "a\\b", ""] {
        assert_eq!(
            registry.list_source_directory_page(&id, "", Some(cursor))
                .unwrap_err().code(),
            "INVALID_RELATIVE_PATH",
            "{cursor}"
        );
    }
    assert_eq!(
        registry.quick_open_source_paths(&id, "../").unwrap_err().code(),
        "INVALID_SEARCH_QUERY"
    );
    registry.close(&id).unwrap();
    assert_eq!(
        registry.quick_open_source_paths(&id, "item").unwrap_err().code(),
        "PROJECT_NOT_OPEN"
    );
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn project_quick_open_skips_generated_folders_and_external_symlinks() {
    use std::os::unix::fs::symlink;
    let (root, registry, id) = fixture();
    let project = root.join("project");
    fs::create_dir_all(project.join("node_modules")).unwrap();
    fs::write(project.join("node_modules/hidden-secret.rs"), b"never index").unwrap();
    fs::write(root.join("outside-secret.rs"), b"external").unwrap();
    symlink(root.join("outside-secret.rs"), project.join("linked-secret.rs")).unwrap();
    symlink(&root, project.join("linked-directory")).unwrap();
    let result = registry.quick_open_source_paths(&id, "secret").unwrap();
    assert!(result.matches.is_empty());
    assert!(result.read_only && !result.source_files_read && !result.process_started);
    fs::remove_dir_all(root).unwrap();
}
