use cap_std::{ambient_authority, fs::Dir};
use lu_core::native_file_transfer::{copy_new_in_dir, move_new_in_dir, TransferError};
use std::{fs, path::PathBuf, sync::atomic::{AtomicU64, Ordering}};
static SEQ: AtomicU64 = AtomicU64::new(0);
fn fixture() -> (PathBuf, Dir) {
    let path=std::env::temp_dir().join(format!("lu-transfer-{}-{}",std::process::id(),SEQ.fetch_add(1,Ordering::SeqCst)));
    fs::create_dir(&path).unwrap();
    let d=Dir::open_ambient_dir(&path,ambient_authority()).unwrap();
    (path,d)
}
#[test]
fn copy_is_no_clobber_and_does_not_mutate_source() {
    let (p,d)=fixture();
    fs::write(p.join("in.txt"),b"alpha").unwrap();
    let r=copy_new_in_dir(&d,"in.txt","out.txt","transaction-123456789").unwrap();
    assert_eq!(r.bytes,5);
    assert!(!r.source_removed);
    assert_eq!(fs::read(p.join("in.txt")).unwrap(),b"alpha");
    assert_eq!(fs::read(p.join("out.txt")).unwrap(),b"alpha");
    assert_eq!(copy_new_in_dir(&d,"in.txt","out.txt","transaction-987654321").unwrap_err(), TransferError::TargetExists);
    assert_eq!(fs::read_dir(&p).unwrap().count(),2);
    drop(d);fs::remove_dir_all(p).unwrap();
}
#[test]
fn move_is_no_clobber_and_removes_only_source() {
    let (p,d)=fixture();
    fs::write(p.join("before.txt"),b"data").unwrap();
    let r=move_new_in_dir(&d,"before.txt","after.txt").unwrap();
    assert!(r.source_removed);
    assert!(!p.join("before.txt").exists());
    assert_eq!(fs::read(p.join("after.txt")).unwrap(),b"data");
    fs::write(p.join("other.txt"),b"keep").unwrap();
    assert_eq!(move_new_in_dir(&d,"after.txt","other.txt").unwrap_err(),TransferError::TargetExists);
    assert_eq!(fs::read(p.join("other.txt")).unwrap(),b"keep");
    drop(d);fs::remove_dir_all(p).unwrap();
}
#[test]
fn refuses_traversal_names_and_stage_aliases() {
    let (p,d)=fixture();
    fs::write(p.join("source"),b"test").unwrap();
    for name in ["../evil","a/b","a\\b","C:evil",".lu-stage-hijack","end.","trail ",""] {
        assert_eq!(move_new_in_dir(&d,"source",name).unwrap_err(),TransferError::InvalidName);
    }
    assert_eq!(move_new_in_dir(&d,"source","SOURCE").unwrap_err(),TransferError::SameTarget);
    assert_eq!(copy_new_in_dir(&d,"source","target","short").unwrap_err(),TransferError::InvalidStage);
    assert!(!p.join("target").exists());
    drop(d);fs::remove_dir_all(p).unwrap();
}
#[cfg(unix)]
#[test]
fn refuses_symlink_sources_and_existing_symlink_targets() {
    use std::os::unix::fs::symlink;
    let (p,d)=fixture();
    fs::write(p.join("source"),b"actual").unwrap();
    symlink("source",p.join("linked")).unwrap();
    assert_eq!(move_new_in_dir(&d,"linked","new").unwrap_err(),TransferError::SourceNotRegular);
    assert_eq!(copy_new_in_dir(&d,"source","linked","transaction-123456789").unwrap_err(),TransferError::TargetExists);
    assert!(!p.join("new").exists());
    drop(d);fs::remove_dir_all(p).unwrap();
}
