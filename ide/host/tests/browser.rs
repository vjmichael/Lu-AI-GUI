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
        "lu-ide-browser-{}-{:016x}",
        std::process::id(),
        rand::random::<u64>()
    ));
    let project = root.join("project");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("README.md"), "# Project\n").unwrap();
    fs::write(
        project.join("src/main.rs"),
        "fn main() { println!(\"hello\"); }\n",
    )
    .unwrap();
    fs::write(project.join("binary.dat"), [0u8, 1, 2, 3]).unwrap();
    fs::write(project.join("oversized.txt"), vec![b'x'; 512 * 1024 + 1]).unwrap();
    // A malicious build recipe must remain inert.
    fs::write(
        project.join("build.rs"),
        "std::fs::write(\"marker\", \"ran\");",
    )
    .unwrap();
    let ide = IdeWorkspaceRegistry::open(root.join("catalog.json")).unwrap();
    let id = ide
        .register_from_native_picker(&mut Picker(Some(project)))
        .unwrap()
        .unwrap()
        .project_id;
    (root, ide, id)
}

#[test]
fn browses_nested_source_and_previews_utf8_without_execution() {
    let (root, ide, id) = fixture();
    let listing = ide.list_source_directory(&id, "").unwrap();
    assert_eq!(listing.entries[0].kind, "directory");
    assert_eq!(listing.entries[0].relative_path, "src");
    assert!(listing
        .entries
        .iter()
        .any(|e| e.name == "README.md" && e.kind == "file"));
    assert!(!listing.truncated);
    let nested = ide.list_source_directory(&id, "src").unwrap();
    assert_eq!(nested.entries[0].relative_path, "src/main.rs");
    let text = ide.preview_source_file(&id, "src/main.rs").unwrap();
    assert!(text.text.contains("println!"));
    assert!(!root.join("project/marker").exists());
    ide.close(&id).unwrap();
    assert_eq!(
        ide.preview_source_file(&id, "README.md")
            .unwrap_err()
            .code(),
        "PROJECT_NOT_OPEN"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn rejects_binary_oversized_and_traversal() {
    let (root, ide, id) = fixture();
    assert_eq!(
        ide.preview_source_file(&id, "binary.dat")
            .unwrap_err()
            .code(),
        "NOT_UTF8_TEXT"
    );
    assert_eq!(
        ide.preview_source_file(&id, "oversized.txt")
            .unwrap_err()
            .code(),
        "FILE_TOO_LARGE"
    );
    for relative in [
        "../secret",
        "/etc/passwd",
        "src/../../outside",
        "C:/Windows",
        "src\\main.rs",
        "src/.",
        "src//main.rs",
    ] {
        assert_eq!(
            ide.preview_source_file(&id, relative).unwrap_err().code(),
            "INVALID_RELATIVE_PATH",
            "{relative}"
        );
    }
    assert_eq!(
        ide.list_source_directory(&id, "../").unwrap_err().code(),
        "INVALID_RELATIVE_PATH"
    );
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn external_symlinks_and_indirect_directory_links_cannot_escape() {
    use std::os::unix::fs::symlink;
    let (root, ide, id) = fixture();
    let secret = root.join("secret.txt");
    fs::write(&secret, "DO NOT LEAK").unwrap();
    symlink(&secret, root.join("project/secret-link")).unwrap();
    symlink(&root, root.join("project/external-dir")).unwrap();
    assert_eq!(
        ide.preview_source_file(&id, "secret-link")
            .unwrap_err()
            .code(),
        "NOT_A_FILE"
    );
    assert!(ide.list_source_directory(&id, "external-dir").is_err());
    let root_list = ide.list_source_directory(&id, "").unwrap();
    assert!(root_list
        .entries
        .iter()
        .any(|e| e.name == "secret-link" && e.kind == "symlink"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn nested_project_remains_visible_after_registry_restart() {
    let (root, ide, id) = fixture();
    drop(ide);
    let ide = IdeWorkspaceRegistry::open(root.join("catalog.json")).unwrap();
    assert_eq!(
        ide.list_source_directory(&id, "").unwrap_err().code(),
        "PROJECT_NOT_OPEN"
    );
    ide.reopen(&id).unwrap();
    assert_eq!(
        ide.preview_source_file(&id, "README.md").unwrap().text,
        "# Project\n"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn project_root_changes_invalidate_preview() {
    let (root, ide, id) = fixture();
    fs::rename(root.join("project"), root.join("moved")).unwrap();
    assert_eq!(
        ide.preview_source_file(&id, "README.md")
            .unwrap_err()
            .code(),
        "PROJECT_NOT_FOUND"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn listings_are_bounded() {
    let (root, ide, id) = fixture();
    for n in 0..430 {
        fs::write(
            root.join("project").join(format!("item-{n:03}.txt")),
            b"hello",
        )
        .unwrap();
    }
    let l = ide.list_source_directory(&id, "").unwrap();
    assert_eq!(l.entries.len(), 400);
    assert!(l.truncated);
    fs::remove_dir_all(root).unwrap();
}
