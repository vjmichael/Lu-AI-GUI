use lu_core::operational_memory::{
    OperationalError, OperationalMemory, ResourceKind, ResourceSpec, ResourceState,
};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_root() -> PathBuf {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let root = std::env::temp_dir().join(format!(
        "lu-operational-ledger-{}-{nanos}", std::process::id()
    ));
    fs::create_dir(&root).unwrap();
    root
}
fn resource(path: &Path, id: &str, workspace: &str) -> ResourceSpec {
    ResourceSpec {
        resource_id: id.into(),
        workspace_id: workspace.into(),
        display_name: "Existing compiler".into(),
        kind: ResourceKind::Executable,
        absolute_path: path.to_str().unwrap().into(),
        platform: std::env::consts::OS.into(),
        architecture: std::env::consts::ARCH.into(),
        version: "1.0-test".into(),
        expected_sha256: None,
    }
}

#[test]
fn remembers_proven_commands_without_claiming_old_proofs_are_current() {
    let root = temp_root();
    let db = root.join("operational.sqlite");
    let exe = root.join("compiler-test.exe");
    fs::write(&exe, b"verified-local-binary").unwrap();
    let rec = resource(&exe,"compiler", "lu");
    {
        let ledger = OperationalMemory::open(&db).unwrap();
        ledger.remember(&rec).unwrap();
        assert_eq!(ledger.recall("lu","compiler").unwrap().unwrap().state,
            ResourceState::KnownHistorically);
        assert_eq!(ledger.probe("lu","compiler").unwrap().state,
            ResourceState::PresentUnverified);
        // Host asserts this only AFTER its own run/test succeeds. The ledger
        // itself never executes or grants the command.
        ledger.record_host_success("lu","compiler","cargo-check",
            "cargo check --locked --manifest-path project/Cargo.toml").unwrap();
        let snapshot = ledger.probe("lu","compiler").unwrap();
        assert_eq!(snapshot.state,ResourceState::VerifiedUsable);
        assert_eq!(snapshot.last_success.as_ref().unwrap().recipe_id,"cargo-check");
        assert_eq!(snapshot.last_success.as_ref().unwrap().source_sha256.as_ref().unwrap().len(),64);
    }
    {
        // A new conversation/process can FIND the old compiler and recipe
        // without rediscovery, but must revalidate execution readiness.
        let reopened = OperationalMemory::open(&db).unwrap();
        let snapshot = reopened.recall("lu","compiler").unwrap().unwrap();
        assert_eq!(snapshot.state,ResourceState::KnownHistorically);
        assert!(snapshot.last_success.is_some());
        assert_eq!(reopened.probe("lu","compiler").unwrap().state,
            ResourceState::PresentUnverified);
        fs::write(&exe,b"replaced executable").unwrap();
        assert_eq!(reopened.probe("lu","compiler").unwrap().state,
            ResourceState::Changed);
        assert_eq!(reopened.record_host_success("lu","compiler","cargo-check","ok"),
            Err(OperationalError::NotReady));
        fs::remove_file(&exe).unwrap();
        assert_eq!(reopened.probe("lu","compiler").unwrap().state,
            ResourceState::Missing);
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn project_scoping_conflicts_platform_and_hash_are_not_authority() {
    let root = temp_root();
    let db = root.join("operational.sqlite");
    let exe = root.join("tool.bin");
    fs::write(&exe,b"tool").unwrap();
    let ledger = OperationalMemory::open(&db).unwrap();
    let spec = resource(&exe,"nsis","project-a");
    ledger.remember(&spec).unwrap();
    ledger.remember(&spec).unwrap(); // exact replay is idempotent
    assert!(ledger.recall("project-b","nsis").unwrap().is_none());
    assert_eq!(ledger.list("project-a",10).unwrap().len(),1);
    assert!(ledger.list("project-b",10).unwrap().is_empty());
    let mut replaced=spec.clone();
    replaced.absolute_path=root.join("wrong").to_str().unwrap().into();
    assert_eq!(ledger.remember(&replaced),Err(OperationalError::Conflict));
    let mut invalid=spec.clone();
    invalid.resource_id="../escape".into();
    assert_eq!(ledger.remember(&invalid),Err(OperationalError::InvalidInput));
    let mut other_os=spec.clone();
    other_os.resource_id="other-platform".into();
    other_os.platform="other-platform".into();
    ledger.remember(&other_os).unwrap();
    assert_eq!(ledger.probe("project-a","other-platform").unwrap().state,
        ResourceState::Incompatible);
    let mut hashed=spec.clone();
    hashed.resource_id="hashed".into();
    hashed.expected_sha256=Some(format!("{:x}",Sha256::digest(b"different")));
    ledger.remember(&hashed).unwrap();
    assert_eq!(ledger.probe("project-a","hashed").unwrap().state,
        ResourceState::Changed);
    assert_eq!(ledger.record_host_success("project-a","hashed","check","success"),
        Err(OperationalError::NotReady));
    drop(ledger);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn directories_are_discoverable_but_not_attested_to_have_working_tools() {
    let root = temp_root();
    let ledger = OperationalMemory::open(&root.join("ops.sqlite")).unwrap();
    let mut rec=resource(&root,"delivered","workspace");
    rec.kind=ResourceKind::Directory;
    ledger.remember(&rec).unwrap();
    assert_eq!(ledger.probe("workspace","delivered").unwrap().state,
        ResourceState::PresentUnverified);
    ledger.record_host_success("workspace","delivered","inspect-delivered",
        "Host scanned the delivered folder").unwrap();
    // A directory may have changed or lack an executable tool. Never call
    // its prior successful scan a verified usable toolchain.
    assert_eq!(ledger.probe("workspace","delivered").unwrap().state,
        ResourceState::PresentUnverified);
    drop(ledger);
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn symlink_never_proves_resource_ready() {
    use std::os::unix::fs::symlink;
    let root = temp_root();
    let real=root.join("real-tool");
    let alias=root.join("alias-tool");
    fs::write(&real,b"content").unwrap();
    symlink(&real,&alias).unwrap();
    let ledger = OperationalMemory::open(&root.join("ops.sqlite")).unwrap();
    ledger.remember(&resource(&alias,"alias","workspace")).unwrap();
    assert_eq!(ledger.probe("workspace","alias").unwrap().state,
        ResourceState::Unverifiable);
    drop(ledger);
    fs::remove_dir_all(root).unwrap();
}
