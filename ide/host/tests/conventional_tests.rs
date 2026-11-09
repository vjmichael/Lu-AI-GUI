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
        "lu-dev034-{}-{:016x}",
        std::process::id(),
        rand::random::<u64>()
    ));
    let project = root.join("project");
    fs::create_dir_all(project.join("tests")).unwrap();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("tests/test_add.py"),
        b"raise Exception('never execute')",
    )
    .unwrap();
    fs::write(
        project.join("tests/engine_test.rs"),
        b"#[test] fn fail() {panic!()}",
    )
    .unwrap();
    fs::write(project.join("src/scene.test.ts"), b"process.exit(99)").unwrap();
    fs::write(project.join("tests/install.sh"), b"exit 99").unwrap();
    let registry = IdeWorkspaceRegistry::open(root.join("catalog.json")).unwrap();
    let id = registry
        .register_from_native_picker(&mut Picker(Some(project)))
        .unwrap()
        .unwrap()
        .project_id;
    (root, registry, id)
}
#[test]
fn fixed_discovery_never_executes_or_authorizes_test_files() {
    let (root, registry, id) = fixture();
    let got = registry.discover_conventional_test_files(&id).unwrap();
    assert_eq!(got.schema_version, 1);
    assert_eq!(got.project_id, id);
    assert_eq!(got.directories_checked, 7);
    assert!(got.incomplete);
    assert!(!got.process_started);
    assert!(!got.worker_grant_issued);
    assert!(!got.source_contents_exported);
    assert!(!got.discovery_is_execution_authority);
    assert_eq!(got.files.len(), 3);
    assert!(got.files.iter().all(|x| !x.runnable));
    assert!(got
        .files
        .iter()
        .any(|x| x.language == "rust" && x.recipe_id == Some("rust_tests")));
    assert!(got
        .files
        .iter()
        .any(|x| x.language == "python" && x.recipe_id.is_none()));
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn bounded_results_and_closed_project_denied() {
    let (root, registry, id) = fixture();
    for i in 0..140 {
        fs::write(
            root.join("project/tests").join(format!("test_{i:03}.py")),
            b"pass",
        )
        .unwrap();
    }
    let got = registry.discover_conventional_test_files(&id).unwrap();
    assert!(got.truncated);
    assert!(got.incomplete);
    assert!(got.files.len() <= 96);
    assert!(got
        .files
        .windows(2)
        .all(|x| x[0].relative_path < x[1].relative_path));
    assert!(registry
        .discover_conventional_test_files("ide-ffffffffffffffffffffffffffffffff")
        .is_err());
    registry.close(&id).unwrap();
    assert!(registry.discover_conventional_test_files(&id).is_err());
    fs::remove_dir_all(root).unwrap();
}
#[cfg(unix)]
#[test]
fn untrusted_symlink_not_followed() {
    use std::os::unix::fs::symlink;
    let (root, registry, id) = fixture();
    fs::write(root.join("secret.test.rs"), b"SECRET").unwrap();
    symlink(
        root.join("secret.test.rs"),
        root.join("project/tests/escaped_test.rs"),
    )
    .unwrap();
    let got = registry.discover_conventional_test_files(&id).unwrap();
    assert!(got
        .files
        .iter()
        .all(|x| x.relative_path != "tests/escaped_test.rs"));
    fs::remove_dir_all(root).unwrap();
}
