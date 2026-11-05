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
        "lu-ide-review-{}-{:016x}",
        std::process::id(),
        rand::random::<u64>()
    ));
    let project = root.join("project");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/main.rs"), "fn main() {\n    println!(\"hello\");\n}\n").unwrap();
    let registry = IdeWorkspaceRegistry::open(root.join("catalog.json")).unwrap();
    let id = registry
        .register_from_native_picker(&mut Picker(Some(project)))
        .unwrap()
        .unwrap()
        .project_id;
    (root, registry, id)
}

#[test]
fn review_is_read_only_and_requires_correct_revision() {
    let (root, registry, id) = fixture();
    let original = registry.preview_source_file(&id, "src/main.rs").unwrap();
    let replacement = "fn main() {\n    println!(\"goodbye\");\n}\n";
    let preview = registry
        .review_source_change(&id, "src/main.rs", &original.revision, replacement)
        .unwrap();
    assert!(!preview.unchanged);
    assert_eq!(preview.removed_lines, 1);
    assert_eq!(preview.added_lines, 1);
    assert!(!preview.truncated);
    assert_eq!(preview.expected_revision, original.revision);
    assert_ne!(preview.proposed_revision, original.revision);
    assert!(preview.lines.iter().any(|line| line.kind == "added" && line.text.contains("goodbye")));
    // A review is purely advisory; it cannot modify disk or start a process.
    assert_eq!(fs::read_to_string(root.join("project/src/main.rs")).unwrap(), original.text);
    assert!(fs::read_dir(root.join("project/src"))
        .unwrap()
        .all(|entry| !entry.unwrap().file_name().to_string_lossy().starts_with(".lu-ide-")));
    let no_change = registry
        .review_source_change(&id, "src/main.rs", &original.revision, &original.text)
        .unwrap();
    assert!(no_change.unchanged);
    assert_eq!((no_change.removed_lines, no_change.added_lines), (0, 0));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn stale_version_and_invalid_paths_cannot_be_reviewed() {
    let (root, registry, id) = fixture();
    let old = registry.preview_source_file(&id, "src/main.rs").unwrap();
    fs::write(root.join("project/src/main.rs"), "changed externally\n").unwrap();
    assert_eq!(
        registry
            .review_source_change(&id, "src/main.rs", &old.revision, "new")
            .unwrap_err()
            .code(),
        "FILE_CHANGED_ON_DISK"
    );
    assert_eq!(
        registry
            .review_source_change(&id, "../outside", &old.revision, "new")
            .unwrap_err()
            .code(),
        "INVALID_RELATIVE_PATH"
    );
    assert_eq!(
        registry
            .review_source_change(&id, "src/main.rs", "invalid", "new")
            .unwrap_err()
            .code(),
        "INVALID_FILE_REVISION"
    );
    assert_eq!(
        registry
            .review_source_change(&id, "src/main.rs", &old.revision, &"x".repeat(512 * 1024 + 1))
            .unwrap_err()
            .code(),
        "FILE_TOO_LARGE"
    );
    assert_eq!(
        registry
            .review_source_change(&id, "src/main.rs", &old.revision, "hello\0world")
            .unwrap_err()
            .code(),
        "NOT_UTF8_TEXT"
    );
    registry.close(&id).unwrap();
    assert_eq!(
        registry
            .review_source_change(&id, "src/main.rs", &old.revision, "new")
            .unwrap_err()
            .code(),
        "PROJECT_NOT_OPEN"
    );
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn external_symlinks_are_not_reviewable() {
    use std::os::unix::fs::symlink;
    let (root, registry, id) = fixture();
    let target = root.join("secret");
    fs::write(&target, "top secret").unwrap();
    symlink(&target, root.join("project/src/link")).unwrap();
    let revision = "0".repeat(64);
    assert_eq!(
        registry
            .review_source_change(&id, "src/link", &revision, "safe")
            .unwrap_err()
            .code(),
        "NOT_A_FILE"
    );
    assert_eq!(fs::read_to_string(target).unwrap(), "top secret");
    fs::remove_dir_all(root).unwrap();
}
