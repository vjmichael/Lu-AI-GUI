use lu_ide_workspace::{
    inventory_history::{InventorySnapshots, SnapshotError},
    IdeError, IdeWorkspaceRegistry, NativeDirectoryPicker,
};
use std::{fs, path::PathBuf};

struct Picker(Option<PathBuf>);
impl NativeDirectoryPicker for Picker {
    fn choose_project_directory(&mut self) -> Result<Option<PathBuf>, IdeError> {
        Ok(self.0.take())
    }
}

fn fixture() -> (PathBuf, IdeWorkspaceRegistry, String, InventorySnapshots) {
    let root = std::env::temp_dir().join(format!(
        "lu-ide-evidence-{}-{:016x}",
        std::process::id(),
        rand::random::<u64>()
    ));
    let project = root.join("project");
    fs::create_dir_all(project.join("Delivered")).unwrap();
    fs::write(project.join("Delivered/alpha.exe"), b"version-one").unwrap();
    let registry = IdeWorkspaceRegistry::open(root.join("catalog.json")).unwrap();
    let id = registry.register_from_native_picker(&mut Picker(Some(project)))
        .unwrap().unwrap().project_id;
    let snapshots = InventorySnapshots::new(root.join("evidence-baselines.json")).unwrap();
    (root, registry, id, snapshots)
}

#[test]
fn restart_recovers_baseline_and_identifies_changed_and_missing_artifacts() {
    let (root, registry, id, snapshots) = fixture();
    assert!(!snapshots.compare(&registry, &id).unwrap().baseline_present);
    let baseline = snapshots.capture(&registry, &id).unwrap();
    assert_eq!(baseline.artifacts.len(), 1);
    assert_eq!(baseline.artifacts[0].sha256.as_ref().unwrap().len(), 64);
    fs::write(root.join("project/Delivered/alpha.exe"), b"version-two").unwrap();
    let different = snapshots.compare(&registry, &id).unwrap();
    assert_eq!(different.entries.len(), 1);
    assert_eq!(different.entries[0].status, "changed_bytes");

    drop(snapshots);
    let snapshots = InventorySnapshots::new(root.join("evidence-baselines.json")).unwrap();
    fs::remove_file(root.join("project/Delivered/alpha.exe")).unwrap();
    let missing = snapshots.compare(&registry, &id).unwrap();
    assert_eq!(missing.entries[0].status, "missing_in_scanned_scope");
    assert_eq!(missing.live_remote, "not_queried");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn snapshots_never_claim_executable_readiness() {
    let (root, registry, id, snapshots) = fixture();
    let baseline = snapshots.capture(&registry, &id).unwrap();
    assert_eq!(baseline.scope, "bounded_known_project_directories_only");
    assert_eq!(baseline.artifacts[0].digest_status, "sha256_observed");
    let same = snapshots.compare(&registry, &id).unwrap();
    assert_eq!(same.entries[0].status, "matched_bytes");
    registry.close(&id).unwrap();
    assert_eq!(
        snapshots.capture(&registry, &id).unwrap_err().code(),
        "PROJECT_NOT_OPEN"
    );
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn snapshot_store_rejects_symlinked_destination() {
    use std::os::unix::fs::symlink;
    let (root, _registry, _id, _snapshots) = fixture();
    symlink(root.join("catalog.json"), root.join("unsafe.json")).unwrap();
    assert!(matches!(
        InventorySnapshots::new(root.join("unsafe.json")),
        Err(SnapshotError::Corrupt)
    ));
    fs::remove_dir_all(root).unwrap();
}
