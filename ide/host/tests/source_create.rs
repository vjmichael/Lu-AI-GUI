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
        "lu-ide-native-create-{}-{:016x}",
        std::process::id(), rand::random::<u64>()
    ));
    let project = root.join("project");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/hello.rs"), b"source must survive").unwrap();
    fs::write(project.join("build.rs"), b"must never execute").unwrap();
    let registry = IdeWorkspaceRegistry::open(root.join("catalog.json")).unwrap();
    let project_id = registry.register_from_native_picker(
        &mut Picker(Some(project))
    ).unwrap().unwrap().project_id;
    (root, registry, project_id)
}

#[test]
fn create_new_empty_file_and_folder_with_verifiable_native_receipt() {
    let (root, registry, id) = fixture();
    let file = registry.create_source_entry(&id, "src", "new.py", "file").unwrap();
    assert_eq!(file.project_id, id);
    assert_eq!(file.relative_path, "src/new.py");
    assert_eq!(file.kind, "file");
    assert!(file.created);
    let preview = registry.preview_source_file(&id, "src/new.py").unwrap();
    assert_eq!(preview.text, "");
    assert_eq!(file.revision.as_deref(), Some(preview.revision.as_str()));

    let folder = registry.create_source_entry(&id, "", "docs", "directory").unwrap();
    assert_eq!(folder.kind, "directory");
    assert_eq!(folder.revision, None);
    let nested = registry.create_source_entry(&id, "docs", "readme.md", "file").unwrap();
    assert_eq!(nested.relative_path, "docs/readme.md");
    let listing = registry.list_source_directory(&id, "docs").unwrap();
    assert!(listing.entries.iter().any(|e| e.name == "readme.md"));
    assert_eq!(fs::read(root.join("project/src/hello.rs")).unwrap(),
        b"source must survive".to_vec());
    assert!(!root.join("project/marker").exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn refuses_existing_targets_case_only_aliases_and_never_truncates_source() {
    let (root, registry, id) = fixture();
    for (dir, name, kind) in [
        ("src", "hello.rs", "file"),
        ("src", "HELLO.RS", "file"),
        ("", "SRC", "directory"),
        ("", "build.rs", "file"),
        ("", "src", "file"),
    ] {
        assert_eq!(registry.create_source_entry(&id, dir, name, kind)
            .unwrap_err().code(), "SOURCE_TARGET_ALREADY_EXISTS",
            "{dir}/{name}");
    }
    assert_eq!(fs::read(root.join("project/src/hello.rs")).unwrap(),
        b"source must survive".to_vec());
    assert_eq!(fs::read(root.join("project/build.rs")).unwrap(),
        b"must never execute".to_vec());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn fail_closed_on_traversal_bad_basename_private_git_and_invalid_kind() {
    let (root, registry, id) = fixture();
    for (dir, name, kind) in [
        ("../project", "bad.rs", "file"),
        ("", "../bad.rs", "file"),
        ("src", "folder/nested", "directory"),
        ("src", "/tmp/escape", "file"),
        ("src", "a\\b.rs", "file"),
        ("src", "C:drive", "file"),
        ("src", ".git", "directory"),
        (".git", "config", "file"),
        ("src", ".lu-ide-scratch", "file"),
        ("src", "CON.rs", "file"),
        ("src", "name.", "file"),
        ("src", "name ", "file"),
        ("src", "bad*name.rs", "file"),
        ("src", "bad.rs", "execute"),
        ("src", "", "file"),
    ] {
        assert_eq!(registry.create_source_entry(&id, dir, name, kind)
            .unwrap_err().code(), "INVALID_RELATIVE_PATH", "{dir}/{name}");
    }
    assert!(!root.join("bad.rs").exists());
    registry.close(&id).unwrap();
    assert_eq!(registry.create_source_entry(&id, "src", "new.rs", "file")
        .unwrap_err().code(), "PROJECT_NOT_OPEN");
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn never_descends_symlink_parent_or_creates_outside_project() {
    use std::os::unix::fs::symlink;
    let (root, registry, id) = fixture();
    let external = root.join("outside");
    fs::create_dir(&external).unwrap();
    symlink(&external, root.join("project/src/escape")).unwrap();
    let err = registry.create_source_entry(&id, "src/escape", "poison.py", "file").unwrap_err();
    assert_eq!(err.code(), "NOT_A_DIRECTORY");
    assert!(!external.join("poison.py").exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn two_concurrent_create_new_attempts_cannot_both_succeed() {
    use std::sync::{Arc, Barrier};
    let (root, registry, id) = fixture();
    let registry = Arc::new(registry);
    let barrier = Arc::new(Barrier::new(3));
    let mut threads = Vec::new();
    for _ in 0..2 {
        let registry = Arc::clone(&registry);
        let id = id.clone();
        let barrier = Arc::clone(&barrier);
        threads.push(std::thread::spawn(move || {
            barrier.wait();
            registry.create_source_entry(&id, "src", "new.rs", "file")
        }));
    }
    barrier.wait();
    let results: Vec<_> = threads.into_iter().map(|handle| handle.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(results.iter().filter_map(|result| result.as_ref().err())
        .filter(|error| error.code() == "SOURCE_TARGET_ALREADY_EXISTS").count(), 1);
    assert!(root.join("project/src/new.rs").is_file());
    fs::remove_dir_all(root).unwrap();
}
