use std::{fs, path::PathBuf};
use lu_ide_workspace::{IdeError, IdeWorkspaceRegistry, NativeDirectoryPicker};

struct Picker(Option<PathBuf>);
impl NativeDirectoryPicker for Picker {
    fn choose_project_directory(&mut self) -> Result<Option<PathBuf>, IdeError> {
        Ok(self.0.take())
    }
}
fn fixture() -> (PathBuf, IdeWorkspaceRegistry, String) {
    let root = std::env::temp_dir().join(format!(
        "lu-ide-editor-{}-{:016x}",
        std::process::id(),
        rand::random::<u64>()
    ));
    let project = root.join("project");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/hello.py"), "print('before')\n").unwrap();
    fs::write(project.join("build.rs"), "malicious build script").unwrap();
    let registry = IdeWorkspaceRegistry::open(root.join("catalog.json")).unwrap();
    let id = registry
        .register_from_native_picker(&mut Picker(Some(project)))
        .unwrap()
        .unwrap()
        .project_id;
    (root, registry, id)
}

#[test]
fn saves_text_atomically_and_updates_revision_without_running_code() {
    let (root, registry, id) = fixture();
    let preview = registry.preview_source_file(&id, "src/hello.py").unwrap();
    assert_eq!(preview.revision.len(), 64);
    let saved = registry
        .save_source_file(&id, "src/hello.py", &preview.revision, "print('after')\n")
        .unwrap();
    assert_ne!(preview.revision, saved.revision);
    assert_eq!(saved.text, "print('after')\n");
    assert_eq!(
        fs::read_to_string(root.join("project/src/hello.py")).unwrap(),
        "print('after')\n"
    );
    assert!(fs::read_dir(root.join("project/src"))
        .unwrap()
        .all(|e| !e.unwrap().file_name().to_string_lossy().starts_with(".lu-ide-")));
    assert!(!root.join("project/marker").exists());

    // No-op save preserves content and revision without replacing target.
    let unchanged = registry
        .save_source_file(&id, "src/hello.py", &saved.revision, &saved.text)
        .unwrap();
    assert_eq!(unchanged.revision, saved.revision);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn rejects_stale_content_and_leaves_external_change_intact() {
    let (root, registry, id) = fixture();
    let before = registry.preview_source_file(&id, "src/hello.py").unwrap();
    fs::write(root.join("project/src/hello.py"), "print('external')\n").unwrap();
    let failure = registry
        .save_source_file(&id, "src/hello.py", &before.revision, "print('my draft')\n")
        .unwrap_err();
    assert_eq!(failure.code(), "FILE_CHANGED_ON_DISK");
    assert_eq!(
        fs::read_to_string(root.join("project/src/hello.py")).unwrap(),
        "print('external')\n"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn rejects_closed_project_traversal_invalid_revision_and_oversized_writes() {
    let (root, registry, id) = fixture();
    let preview = registry.preview_source_file(&id, "src/hello.py").unwrap();
    assert_eq!(
        registry.save_source_file(&id, "src/hello.py", "0", "hello")
            .unwrap_err().code(),
        "INVALID_FILE_REVISION"
    );
    for illegal in ["../secret", "/tmp/secret", "src/../../secret", "src\\hello.py"] {
        assert_eq!(
            registry.save_source_file(&id, illegal, &preview.revision, "bad")
                .unwrap_err().code(),
            "INVALID_RELATIVE_PATH"
        );
    }
    assert_eq!(
        registry.save_source_file(&id, "src/hello.py", &preview.revision, &"x".repeat(512 * 1024 + 1))
            .unwrap_err().code(),
        "FILE_TOO_LARGE"
    );
    assert_eq!(
        registry.save_source_file(&id, "src/hello.py", &preview.revision, "bad\0text")
            .unwrap_err().code(),
        "NOT_UTF8_TEXT"
    );
    registry.close(&id).unwrap();
    assert_eq!(
        registry.save_source_file(&id, "src/hello.py", &preview.revision, "hello")
            .unwrap_err().code(),
        "PROJECT_NOT_OPEN"
    );
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn refuses_symlink_file_target_and_preserves_unix_mode_bits() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let (root, registry, id) = fixture();
    let file = root.join("project/src/hello.py");
    fs::set_permissions(&file, fs::Permissions::from_mode(0o744)).unwrap();
    let preview = registry.preview_source_file(&id, "src/hello.py").unwrap();
    registry.save_source_file(&id, "src/hello.py", &preview.revision, "# changed\n").unwrap();
    assert_eq!(fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o744);

    let external = root.join("secret.txt");
    fs::write(&external, "secret").unwrap();
    symlink(&external, root.join("project/src/secret-link")).unwrap();
    assert_eq!(
        registry.save_source_file(&id, "src/secret-link", &"a".repeat(64), "bad")
            .unwrap_err().code(),
        "NOT_A_FILE"
    );
    assert_eq!(fs::read_to_string(external).unwrap(), "secret");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn saved_text_survives_registry_restart_and_old_revision_is_rejected() {
    let (root, registry, id) = fixture();
    let before = registry.preview_source_file(&id, "src/hello.py").unwrap();
    let saved = registry
        .save_source_file(&id, "src/hello.py", &before.revision, "print('persist')\n")
        .unwrap();
    drop(registry);
    let restarted = IdeWorkspaceRegistry::open(root.join("catalog.json")).unwrap();
    restarted.reopen(&id).unwrap();
    assert_eq!(
        restarted.preview_source_file(&id, "src/hello.py").unwrap().revision,
        saved.revision
    );
    assert_eq!(
        restarted.save_source_file(&id, "src/hello.py", &before.revision, "bad")
            .unwrap_err().code(),
        "FILE_CHANGED_ON_DISK"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn two_saves_with_same_base_revision_do_not_both_succeed() {
    use std::sync::{Arc, Barrier};
    let (root, registry, id) = fixture();
    let registry = Arc::new(registry);
    let preview = registry.preview_source_file(&id, "src/hello.py").unwrap();
    let start = Arc::new(Barrier::new(3));
    let mut threads = Vec::new();
    for i in 0..2 {
        let registry = Arc::clone(&registry);
        let start = Arc::clone(&start);
        let id = id.clone();
        let revision = preview.revision.clone();
        threads.push(std::thread::spawn(move || {
            start.wait();
            registry.save_source_file(&id, "src/hello.py", &revision, &format!("version-{i}\n"))
        }));
    }
    start.wait();
    let results: Vec<_> = threads.into_iter().map(|thread| thread.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results.iter().filter_map(|result| result.as_ref().err())
            .filter(|error| error.code() == "FILE_CHANGED_ON_DISK").count(),
        1
    );
    fs::remove_dir_all(root).unwrap();
}
