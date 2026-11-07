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
        "lu-ide-probe-{}-{:016x}",
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
fn probe_observes_actual_file_bytes_then_missing_and_changed_resource() {
    let (root, registry, id) = fixture();
    let target = root.join("project/Delivered");
    fs::create_dir_all(&target).unwrap();
    fs::write(
        target.join("sample.exe"),
        b"fixture binary is not executable",
    )
    .unwrap();
    let first = registry
        .probe_project_resource(&id, "Delivered/sample.exe")
        .unwrap();
    assert_eq!(first.state, "bytes_observed");
    assert_eq!(
        first.byte_length,
        Some(b"fixture binary is not executable".len() as u64)
    );
    assert_eq!(first.sha256.as_ref().unwrap().len(), 64);
    assert_eq!(first.executable_usable, "not_tested");
    fs::write(target.join("sample.exe"), b"changed fixture bytes longer").unwrap();
    let changed = registry
        .probe_project_resource(&id, "Delivered/sample.exe")
        .unwrap();
    assert_eq!(changed.state, "bytes_observed");
    assert_ne!(changed.sha256, first.sha256);
    fs::remove_file(target.join("sample.exe")).unwrap();
    let missing = registry
        .probe_project_resource(&id, "Delivered/sample.exe")
        .unwrap();
    assert_eq!(missing.state, "missing_in_registered_project");
    assert!(missing.sha256.is_none());
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn targeted_probe_does_not_allow_escaping_or_opening_closed_project() {
    let (root, registry, id) = fixture();
    for bad in ["../other", "/etc/passwd", "a\\b", "a//b", "C:/Windows"] {
        assert_eq!(
            registry
                .probe_project_resource(&id, bad)
                .unwrap_err()
                .code(),
            "INVALID_RELATIVE_PATH"
        );
    }
    registry.close(&id).unwrap();
    assert_eq!(
        registry
            .probe_project_resource(&id, "test.exe")
            .unwrap_err()
            .code(),
        "PROJECT_NOT_OPEN"
    );
    fs::remove_dir_all(root).unwrap();
}
#[cfg(unix)]
#[test]
fn symlinked_probe_can_not_read_external_file() {
    use std::os::unix::fs::symlink;
    let (root, registry, id) = fixture();
    fs::write(root.join("outside.txt"), b"secret").unwrap();
    symlink(root.join("outside.txt"), root.join("project/bad.exe")).unwrap();
    let found = registry.probe_project_resource(&id, "bad.exe").unwrap();
    assert_eq!(found.state, "not_regular_file");
    assert!(found.sha256.is_none());
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn oversized_resource_is_not_hashed_or_assumed_usable() {
    let (root, registry, id) = fixture();
    let path = root.join("project/large.bin");
    let file = fs::File::create(&path).unwrap();
    file.set_len(64 * 1024 * 1024 + 1).unwrap();
    let found = registry.probe_project_resource(&id, "large.bin").unwrap();
    assert_eq!(found.state, "present_unhashed_over_limit");
    assert!(found.sha256.is_none());
    assert_eq!(found.executable_usable, "not_tested");
    fs::remove_dir_all(root).unwrap();
}
