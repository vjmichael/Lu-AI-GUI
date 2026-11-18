use lu_core::native_atomic_write::{
    create_new_atomic, AtomicWriteError, MAX_ATOMIC_WRITE_BYTES,
};
use std::{fs, path::PathBuf, sync::atomic::{AtomicU64, Ordering}, time::{SystemTime, UNIX_EPOCH}};
static SERIAL: AtomicU64 = AtomicU64::new(0);
fn root() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "lu-atomic-new-{}-{}-{}", std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos(),
        SERIAL.fetch_add(1, Ordering::Relaxed),
    ));
    fs::create_dir_all(&path).unwrap();
    path
}
const STAGE1: &str = "trusted-host-stage-001";
const STAGE2: &str = "trusted-host-stage-002";

#[test]
fn published_new_file_contains_exact_bytes_with_digest_and_no_staging_alias() {
    let dir = root();
    let receipt = create_new_atomic(&dir, "report.txt", STAGE1, b"Hello Lu\n").unwrap();
    assert_eq!(receipt.target, "report.txt");
    assert_eq!(receipt.bytes_written, 9);
    assert_eq!(receipt.sha256.len(), 64);
    assert_eq!(fs::read(dir.join("report.txt")).unwrap(), b"Hello Lu\n");
    assert!(!dir.join(format!(".lu-stage-{STAGE1}.tmp")).exists());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn never_overwrites_existing_target_even_when_staging_succeeds() {
    let dir = root();
    fs::write(dir.join("existing.txt"), b"valuable original").unwrap();
    assert_eq!(
        create_new_atomic(&dir, "existing.txt", STAGE1, b"model wants overwrite"),
        Err(AtomicWriteError::ExistingTarget),
    );
    assert_eq!(fs::read(dir.join("existing.txt")).unwrap(), b"valuable original");
    assert!(!dir.join(format!(".lu-stage-{STAGE1}.tmp")).exists());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn refuses_orphaned_staging_identity_without_touching_existing_bytes() {
    let dir = root();
    let stage = dir.join(format!(".lu-stage-{STAGE1}.tmp"));
    fs::write(&stage, b"unreconciled previous attempt").unwrap();
    assert_eq!(
        create_new_atomic(&dir, "other.txt", STAGE1, b"do not silently reuse"),
        Err(AtomicWriteError::StageCollision),
    );
    assert_eq!(fs::read(&stage).unwrap(), b"unreconciled previous attempt");
    assert!(!dir.join("other.txt").exists());
    create_new_atomic(&dir, "other.txt", STAGE2, b"separately approved")
        .unwrap();
    assert_eq!(fs::read(dir.join("other.txt")).unwrap(), b"separately approved");
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn model_style_paths_and_unbounded_data_are_rejected_without_side_effects() {
    let dir = root();
    for name in ["", ".", "..", "../outside", "a/b", "a\\b", "C:secret",
                 ".lu-stage-stolen.tmp", "/etc/passwd"] {
        assert_eq!(
            create_new_atomic(&dir, name, STAGE1, b"unsafe"),
            Err(AtomicWriteError::InvalidTarget),
            "unsafe name: {name:?}"
        );
    }
    assert_eq!(
        create_new_atomic(&dir, "good.txt", "a", b"x"),
        Err(AtomicWriteError::InvalidStage)
    );
    assert_eq!(
        create_new_atomic(&dir, "good.txt", STAGE1, &vec![b'x'; MAX_ATOMIC_WRITE_BYTES + 1]),
        Err(AtomicWriteError::Oversized)
    );
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 0);
    fs::remove_dir_all(dir).unwrap();
}

#[cfg(unix)]
#[test]
fn refuses_symlink_directory_root() {
    use std::os::unix::fs::symlink;
    let dir = root();
    let alias = dir.with_extension("symlink");
    symlink(&dir, &alias).unwrap();
    assert_eq!(
        create_new_atomic(&alias, "unsafe.txt", STAGE1, b"x"),
        Err(AtomicWriteError::RootUnavailable)
    );
    assert!(!dir.join("unsafe.txt").exists());
    fs::remove_file(alias).unwrap();
    fs::remove_dir_all(dir).unwrap();
}

#[cfg(unix)]
#[test]
fn pinned_directory_publication_resists_parent_path_replacement() {
    use cap_std::{ambient_authority, fs::Dir};
    use lu_core::native_atomic_write::create_new_atomic_in_dir;

    let dir=root();
    let pinned=Dir::open_ambient_dir(&dir, ambient_authority()).unwrap();
    let renamed=dir.with_extension("original");
    fs::rename(&dir,&renamed).unwrap();
    fs::create_dir(&dir).unwrap();
    fs::write(dir.join("decoy.txt"), b"attacker-controlled replacement").unwrap();
    let receipt=create_new_atomic_in_dir(
        &pinned, "approved.json", STAGE1, br#"{"from":"trusted handle"}"#
    ).unwrap();
    assert_eq!(receipt.target,"approved.json");
    assert_eq!(fs::read(renamed.join("approved.json")).unwrap(),
        br#"{"from":"trusted handle"}"#);
    assert!(!dir.join("approved.json").exists(),
        "path replacement must not redirect an approved write");
    assert!(!renamed.join(format!(".lu-stage-{STAGE1}.tmp")).exists());
    drop(pinned);
    fs::remove_dir_all(renamed).unwrap();
    fs::remove_dir_all(dir).unwrap();
}
