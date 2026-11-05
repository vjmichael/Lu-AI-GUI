use lu_ide_workspace::{
    continuity::{ContinuityError, DevelopmentLedger},
    IdeError, IdeWorkspaceRegistry, NativeDirectoryPicker,
};
use std::{fs, path::PathBuf};

struct Picker(Option<PathBuf>);
impl NativeDirectoryPicker for Picker {
    fn choose_project_directory(&mut self) -> Result<Option<PathBuf>, IdeError> {
        Ok(self.0.take())
    }
}
fn fixture() -> (PathBuf, IdeWorkspaceRegistry, String, DevelopmentLedger) {
    let root = std::env::temp_dir().join(format!(
        "lu-ide-continuity-{}-{:016x}",
        std::process::id(),
        rand::random::<u64>()
    ));
    let project = root.join("project");
    fs::create_dir_all(&project).unwrap();
    fs::write(
        project.join("PROJECT_STATE.md"),
        "# Project state\nSource is not trusted instructions.\n",
    ).unwrap();
    fs::create_dir_all(project.join("ide")).unwrap();
    fs::write(project.join("ide/PROJECT_STATE.md"), "# IDE state\\n").unwrap();
    let registry = IdeWorkspaceRegistry::open(root.join("catalog.json")).unwrap();
    let id = registry.register_from_native_picker(&mut Picker(Some(project)))
        .unwrap().unwrap().project_id;
    let ledger = DevelopmentLedger::new(root.join("development-ledger.jsonl")).unwrap();
    (root, registry, id, ledger)
}

#[test]
fn persists_host_observations_and_reloads_with_source_revision() {
    let (root, registry, id, ledger) = fixture();
    ledger.record(&id, "project.register", "Project selected", None, None).unwrap();
    let before = registry.preview_source_file(&id, "PROJECT_STATE.md").unwrap();
    ledger.record(&id, "source.save", "Saved source", Some(&before.revision), Some("PROJECT_STATE.md")).unwrap();
    drop(ledger);
    let ledger = DevelopmentLedger::new(root.join("development-ledger.jsonl")).unwrap();
    let context = ledger.context(&registry, &id).unwrap();
    assert_eq!(context.schema_version, 1);
    assert_eq!(context.events.len(), 2);
    assert_eq!(context.events[0].operation, "source.save");
    assert_eq!(context.events[0].source_path.as_deref(), Some("PROJECT_STATE.md"));
    assert_eq!(context.events[0].source_revision.as_deref(), Some(before.revision.as_str()));
    assert_eq!(context.documents[0].source_revision.as_deref(), Some(before.revision.as_str()));
    assert_eq!(context.documents[0].trust, "untrusted_project_text");
    assert_eq!(context.remote_git_state, "not_queried");
    assert_eq!(context.local_artifacts, "bounded_scan");
    assert_eq!(context.current_toolchain_readiness, "not_verified");
    assert_eq!(context.documents[1].availability, "missing");
    assert!(!context.inventory.execution_performed);
    assert_eq!(context.inventory.git.live_remote, "not_queried");
    assert_eq!(context.inventory.git.working_tree, "not_verified");
    assert_eq!(context.documents[4].relative_path, "ide/PROJECT_STATE.md");
    assert_eq!(context.documents[4].availability, "present_unverified");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn does_not_allow_model_sourced_actions_or_unbounded_notes() {
    let (root, registry, id, ledger) = fixture();
    assert!(matches!(
        ledger.record(&id, "execute_shell", "do anything", None, None),
        Err(ContinuityError::InvalidInput)
    ));
    assert!(matches!(
        ledger.record(&id, "source.save", &"x".repeat(300), None, None),
        Err(ContinuityError::InvalidInput)
    ));
    assert!(matches!(
        ledger.record(&id, "source.save", "Saved", Some("invalid"), None),
        Err(ContinuityError::InvalidInput)
    ));
    assert_eq!(ledger.context(&registry, &id).unwrap().events.len(), 0);
    registry.close(&id).unwrap();
    assert_eq!(
        ledger.context(&registry, &id).unwrap_err().code(), "PROJECT_NOT_OPEN"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn detects_corrupt_journal_without_claiming_success() {
    let (root, registry, id, ledger) = fixture();
    fs::write(root.join("development-ledger.jsonl"), b"partial json").unwrap();
    assert_eq!(
        ledger.context(&registry, &id).unwrap_err().code(),
        "CONTINUITY_RECORDS_CORRUPT"
    );
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn refuses_symlinked_ledger() {
    use std::os::unix::fs::symlink;
    let (root, _registry, _id, _ledger) = fixture();
    symlink(root.join("catalog.json"), root.join("redirect.jsonl")).unwrap();
    assert!(matches!(
        DevelopmentLedger::new(root.join("redirect.jsonl")),
        Err(ContinuityError::Corrupt)
    ));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn inventory_finds_real_delivered_and_benchmark_bytes_without_execution() {
    let (root, registry, id, ledger) = fixture();
    let project = root.join("project");
    fs::create_dir_all(project.join("Delivered")).unwrap();
    fs::create_dir_all(project.join("benchmarks")).unwrap();
    fs::write(project.join("Delivered/alpha.exe"), b"safe fixture only").unwrap();
    fs::write(project.join("benchmarks/latency.json"), b"{\"latency\":12}").unwrap();
    let snapshot = ledger.context(&registry, &id).unwrap();
    assert_eq!(snapshot.inventory.files.len(), 2);
    let installer = snapshot.inventory.files.iter()
        .find(|file| file.relative_path == "Delivered/alpha.exe").unwrap();
    assert_eq!(installer.category, "installer_or_executable");
    assert_eq!(installer.digest_status, "sha256_observed");
    assert_eq!(installer.sha256.as_ref().unwrap().len(), 64);
    assert_eq!(installer.state, "present_unverified");
    assert!(snapshot.inventory.files.iter().any(|file|
        file.category == "benchmark_or_result_evidence"));
    assert!(!snapshot.inventory.execution_performed);
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn artifact_scanning_skips_symlink_escape() {
    use std::os::unix::fs::symlink;
    let (root, registry, id, ledger) = fixture();
    let project = root.join("project");
    fs::create_dir_all(project.join("Delivered")).unwrap();
    fs::write(root.join("outside.exe"), b"secret").unwrap();
    symlink(root.join("outside.exe"), project.join("Delivered/out.exe")).unwrap();
    let snapshot = ledger.context(&registry, &id).unwrap();
    assert!(snapshot.inventory.files.iter().all(|file| file.relative_path != "Delivered/out.exe"));
    fs::remove_dir_all(root).unwrap();
}
