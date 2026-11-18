//! Real archive fixtures and traversal/metadata/integrity behavior.
use cap_std::{ambient_authority,fs::Dir};
use lu_core::native_linux_archive::{
    LinuxArchiveError,LinuxArchiveFormat,detect_linux_archive,
    create_linux_archive_new,inspect_linux_archive,extract_linux_archive_member,
};
use std::{fs,path::PathBuf,sync::atomic::{AtomicU64,Ordering},process::Command};
static COUNTER:AtomicU64=AtomicU64::new(0);
fn fixture()->(PathBuf,Dir){
    let root=std::env::temp_dir().join(format!("lu-linux-archives-{}-{}",
        std::process::id(),COUNTER.fetch_add(1,Ordering::SeqCst)));
    fs::create_dir_all(&root).unwrap();
    let dir=Dir::open_ambient_dir(&root,ambient_authority()).unwrap();
    (root,dir)
}
#[test]
fn detection_covers_common_linux_source_and_backup_formats(){
    for (name,format) in [
        ("src.tar",LinuxArchiveFormat::Tar),
        ("src.tar.gz",LinuxArchiveFormat::TarGz),
        ("src.tgz",LinuxArchiveFormat::TarGz),
        ("src.tar.xz",LinuxArchiveFormat::TarXz),
        ("src.txz",LinuxArchiveFormat::TarXz),
        ("src.tar.bz2",LinuxArchiveFormat::TarBz2),
        ("src.tbz2",LinuxArchiveFormat::TarBz2),
        ("src.tar.zst",LinuxArchiveFormat::TarZst),
        ("src.tzst",LinuxArchiveFormat::TarZst),
        ("backup.gz",LinuxArchiveFormat::Gzip),
        ("backup.xz",LinuxArchiveFormat::Xz),
        ("backup.bz2",LinuxArchiveFormat::Bzip2),
        ("backup.zst",LinuxArchiveFormat::Zstd),
    ] { assert_eq!(detect_linux_archive(name),Some(format),"{name}") }
}
#[test]
fn tar_and_targz_create_inspect_extract_and_no_overwrite(){
    let (root,dir)=fixture();
    fs::write(root.join("alpha.txt"),b"alpha & beta").unwrap();
    fs::write(root.join("beta.txt"),b"second").unwrap();
    for (name,stage) in [
        ("backup.tar","transaction-123456789"),
        ("backup.tar.gz","transaction-987654321"),
    ]{
        let receipt=create_linux_archive_new(
            &dir,name,&["alpha.txt".into(),"beta.txt".into()],stage
        ).unwrap();
        assert_eq!(receipt.extracted_files,2);
        let manifest=inspect_linux_archive(&dir,name,256).unwrap();
        assert_eq!(manifest.total_entries,2);
        assert_eq!(manifest.entries[0].path,"alpha.txt");
        assert_eq!(manifest.entries[0].uncompressed_bytes,12);
        let first=&manifest.entries[0];
        let renamed=format!("extracted-{}",name.replace('.',"-"));
        let extracted=extract_linux_archive_member(
            &dir,name,&first.path,&dir,&renamed,"transaction-0123456789abcdef",
            &first.sha256
        ).unwrap();
        assert_eq!(extracted.bytes_written,12);
        assert_eq!(fs::read(root.join(&renamed)).unwrap(),b"alpha & beta");
        assert_eq!(extract_linux_archive_member(
            &dir,name,&first.path,&dir,&renamed,"transaction-fedcba9876543210",
            &first.sha256
        ).unwrap_err(),LinuxArchiveError::TargetExists);
    }
    drop(dir);fs::remove_dir_all(root).unwrap();
}
#[cfg(unix)]
#[test]
fn native_external_backends_roundtrip_xz_bzip2_and_zstd(){
    for program in ["/usr/bin/xz","/usr/bin/bzip2","/usr/bin/zstd"]{
        if !std::path::Path::new(program).exists(){return}
    }
    let (root,dir)=fixture();
    fs::write(root.join("file.txt"),b"compressed Linux file").unwrap();
    for (index,name) in ["src.tar.xz","src.tar.bz2","src.tar.zst",
       "file-copy.xz","file-copy.bz2","file-copy.zst"].into_iter().enumerate(){
        let input=if name.starts_with("src"){vec!["file.txt".into()]}
            else {vec!["file.txt".into()]};
        let stage=format!("transaction-0123456789abc-{index}");
        create_linux_archive_new(&dir,name,&input,&stage).unwrap();
        let manifest=inspect_linux_archive(&dir,name,256).unwrap();
        assert_eq!(manifest.total_entries,1);
        assert_eq!(manifest.entries[0].uncompressed_bytes,21);
        let entry=&manifest.entries[0];
        let target=format!("output-{index}.txt");
        extract_linux_archive_member(
            &dir,name,&entry.path,&dir,&target,&format!("stage-0123456789abc-{index}"),
            &entry.sha256
        ).unwrap();
        assert_eq!(fs::read(root.join(target)).unwrap(),b"compressed Linux file");
    }
    drop(dir);fs::remove_dir_all(root).unwrap();
}
#[test]
fn bad_archive_data_or_hostile_member_is_not_published(){
    let (root,dir)=fixture();
    fs::write(root.join("malformed.tar"),b"arbitrary binary not a TAR").unwrap();
    assert!(inspect_linux_archive(&dir,"malformed.tar",256).is_err());
    fs::write(root.join("plain.txt"),b"safe content").unwrap();
    create_linux_archive_new(
        &dir,"valid.tar",&["plain.txt".into()],"transaction-1122334455667788"
    ).unwrap();
    let mut data=fs::read(root.join("valid.tar")).unwrap();
    data[0..10].copy_from_slice(b"../BAD.TXT");
    fs::write(root.join("unsafe.tar"),data).unwrap();
    assert!(inspect_linux_archive(&dir,"unsafe.tar",256).is_err());
    let manifest=inspect_linux_archive(&dir,"valid.tar",256).unwrap();
    let entry=&manifest.entries[0];
    assert_eq!(extract_linux_archive_member(
        &dir,"valid.tar",&entry.path,&dir,"must-not-exist.txt",
        "transaction-123456789012",&"0".repeat(64)
    ).unwrap_err(),LinuxArchiveError::SourceChanged);
    assert!(!root.join("must-not-exist.txt").exists());
    drop(dir);fs::remove_dir_all(root).unwrap();
}
#[cfg(unix)]
#[test]
fn input_symlink_is_rejected(){
    use std::os::unix::fs::symlink;
    let (root,dir)=fixture();
    fs::write(root.join("real.txt"),b"safe").unwrap();
    symlink("real.txt",root.join("link.txt")).unwrap();
    assert_eq!(create_linux_archive_new(
        &dir,"refuse.tar",&["link.txt".into()],"transaction-123456789"
    ).unwrap_err(),LinuxArchiveError::NotFile);
    drop(dir);fs::remove_dir_all(root).unwrap();
}
