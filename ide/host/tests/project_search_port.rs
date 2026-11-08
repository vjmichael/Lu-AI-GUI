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
        "lu-ide-search-{}-{:016x}",
        std::process::id(),
        rand::random::<u64>()
    ));
    let source = root.join("project");
    fs::create_dir_all(&source).unwrap();
    let registry = IdeWorkspaceRegistry::open(root.join("catalog.json")).unwrap();
    let id = registry
        .register_from_native_picker(&mut Picker(Some(source)))
        .unwrap()
        .unwrap()
        .project_id;
    (root, registry, id)
}

#[test]
fn search_returns_exact_utf8_bytes_utf16_columns_and_saved_revision() {
    let (root, registry, id) = fixture();
    let source = root.join("project/src");
    fs::create_dir_all(&source).unwrap();
    let content = "α😀\r\nemoji 😀needle\r\nNEEDLE again";
    fs::write(source.join("source.rs"), content.as_bytes()).unwrap();
    let result = registry.find_in_project(&id, "needle", false).unwrap();
    assert_eq!(result.schema_version, 1);
    assert_eq!(result.files_scanned, 1);
    assert_eq!(result.matches.len(), 2);
    assert!(!result.truncated);
    let first = &result.matches[0];
    assert_eq!(first.relative_path, "src/source.rs");
    assert_eq!(first.line, 2);
    assert_eq!(first.column_utf16, 9);
    assert_eq!(first.match_length_utf16, 6);
    assert_eq!(first.from_byte as usize, content.find("needle").unwrap());
    assert_eq!(first.to_byte, first.from_byte + 6);
    assert_eq!(first.revision.len(), 64);
    assert_eq!(first.revision, result.matches[1].revision);
    assert!(first.preview.contains("needle"));
    assert_eq!(result.matches[1].line, 3);
    assert_eq!(result.matches[1].column_utf16, 1);
    let sensitive = registry.find_in_project(&id, "needle", true).unwrap();
    assert_eq!(sensitive.matches.len(), 1);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn huge_or_binary_or_generated_inputs_are_never_searched_as_source() {
    let (root, registry, id) = fixture();
    let project = root.join("project");
    fs::create_dir_all(project.join("node_modules")).unwrap();
    fs::write(project.join("node_modules/untrusted.js"), b"needle").unwrap();
    fs::write(project.join("binary.bin"), b"needle\0secret").unwrap();
    fs::write(project.join("huge.txt"), vec![b'x'; 300_000]).unwrap();
    fs::write(project.join("fine.txt"), b"one needle").unwrap();
    let result = registry.find_in_project(&id, "needle", true).unwrap();
    assert_eq!(result.matches.len(), 1);
    assert_eq!(result.matches[0].relative_path, "fine.txt");
    assert!(result.skipped_files >= 2);
    assert_eq!(
        result.scope,
        "saved_utf8_text_excluding_generated_directories"
    );
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn symlink_file_and_directory_cannot_escape_selected_root() {
    use std::os::unix::fs::symlink;
    let (root, registry, id) = fixture();
    let project = root.join("project");
    fs::create_dir_all(root.join("outside")).unwrap();
    fs::write(root.join("outside/secret.txt"), b"needle").unwrap();
    symlink(root.join("outside"), project.join("linked-directory")).unwrap();
    symlink(
        root.join("outside/secret.txt"),
        project.join("linked-file.txt"),
    )
    .unwrap();
    let result = registry.find_in_project(&id, "needle", true).unwrap();
    assert!(result.matches.is_empty());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn malformed_query_and_closed_project_fail_closed() {
    let (root, registry, id) = fixture();
    for query in ["", "  ", "\n", "abc\0def"] {
        assert_eq!(
            registry
                .find_in_project(&id, query, false)
                .unwrap_err()
                .code(),
            "INVALID_SEARCH_QUERY"
        );
    }
    assert_eq!(
        registry
            .find_in_project(&id, &"x".repeat(129), false)
            .unwrap_err()
            .code(),
        "INVALID_SEARCH_QUERY"
    );
    registry.close(&id).unwrap();
    assert_eq!(
        registry
            .find_in_project(&id, "source", true)
            .unwrap_err()
            .code(),
        "PROJECT_NOT_OPEN"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn fixed_hit_budget_never_silently_claims_complete_results() {
    let (root, registry, id) = fixture();
    let project = root.join("project");
    fs::write(project.join("many.txt"), "z".repeat(300)).unwrap();
    let result = registry.find_in_project(&id, "z", true).unwrap();
    assert_eq!(result.matches.len(), 240);
    assert!(result.truncated);
    fs::remove_dir_all(root).unwrap();
}
