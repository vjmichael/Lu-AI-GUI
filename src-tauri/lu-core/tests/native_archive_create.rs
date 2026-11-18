use cap_std::{ambient_authority,fs::Dir};
use lu_core::native_archive_create::{create_zip_new_in_dir, ArchiveCreateError};
use std::{fs,io::Read,path::PathBuf,sync::atomic::{AtomicU64,Ordering}};
static SEQ:AtomicU64=AtomicU64::new(0);
fn fixture()->(PathBuf,Dir){
    let p=std::env::temp_dir().join(format!("lu-zip-create-{}-{}",std::process::id(),SEQ.fetch_add(1,Ordering::SeqCst)));
    fs::create_dir(&p).unwrap();
    let d=Dir::open_ambient_dir(&p,ambient_authority()).unwrap();
    (p,d)
}
#[test]
fn produces_verifiable_zip_without_modifying_sources() {
    let (p,d)=fixture();
    fs::write(p.join("first.txt"),b"alpha").unwrap();
    fs::write(p.join("second.txt"),b"beta").unwrap();
    let r=create_zip_new_in_dir(&d,"out.zip",&["first.txt".into(),"second.txt".into()],"transaction-123456789").unwrap();
    assert_eq!(r.entries,2);
    assert!(r.archive_bytes>0);
    assert_eq!(r.sha256.len(),64);
    let mut z=zip::ZipArchive::new(fs::File::open(p.join("out.zip")).unwrap()).unwrap();
    assert_eq!(z.len(),2);
    let mut v=String::new();
    z.by_name("first.txt").unwrap().read_to_string(&mut v).unwrap();
    assert_eq!(v,"alpha");
    assert_eq!(fs::read(p.join("first.txt")).unwrap(),b"alpha");
    drop(z);
    assert_eq!(fs::read_dir(&p).unwrap().count(),3);
    drop(d);fs::remove_dir_all(p).unwrap();
}
#[test]
fn rejects_target_overwrite_duplicates_and_unsafe_paths() {
    let (p,d)=fixture();
    fs::write(p.join("a.txt"),b"safe").unwrap();
    fs::write(p.join("exists.zip"),b"keep").unwrap();
    let names=vec!["a.txt".to_string()];
    assert_eq!(create_zip_new_in_dir(&d,"exists.zip",&names,"transaction-123456789").unwrap_err(),ArchiveCreateError::TargetExists);
    assert_eq!(create_zip_new_in_dir(&d,"../bad.zip",&names,"transaction-123456789").unwrap_err(),ArchiveCreateError::InvalidName);
    assert_eq!(create_zip_new_in_dir(&d,"created.zip",&["../bad".into()],"transaction-123456789").unwrap_err(),ArchiveCreateError::InvalidName);
    assert_eq!(create_zip_new_in_dir(&d,"created.zip",&["a.txt".into(),"A.TXT".into()],"transaction-123456789").unwrap_err(),ArchiveCreateError::DuplicateEntry);
    assert_eq!(create_zip_new_in_dir(&d,"a.txt",&names,"transaction-123456789").unwrap_err(),ArchiveCreateError::InvalidName);
    assert_eq!(fs::read(p.join("exists.zip")).unwrap(),b"keep");
    assert!(!p.join("created.zip").exists());
    drop(d);fs::remove_dir_all(p).unwrap();
}
#[cfg(unix)]
#[test]
fn rejects_symlink_input_and_output() {
    use std::os::unix::fs::symlink;
    let (p,d)=fixture();
    fs::write(p.join("real.txt"),b"test").unwrap();
    symlink("real.txt",p.join("alias.txt")).unwrap();
    let bad=vec!["alias.txt".to_owned()];
    assert_eq!(create_zip_new_in_dir(&d,"result.zip",&bad,"transaction-123456789").unwrap_err(),ArchiveCreateError::SourceNotRegular);
    symlink("missing.zip",p.join("result.zip")).unwrap();
    assert_eq!(create_zip_new_in_dir(&d,"result.zip",&["real.txt".into()],"transaction-123456789").unwrap_err(),ArchiveCreateError::TargetExists);
    drop(d);fs::remove_dir_all(p).unwrap();
}
