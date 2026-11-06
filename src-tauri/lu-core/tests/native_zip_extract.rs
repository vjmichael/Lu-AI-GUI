use std::{fs,io::Write,path::PathBuf,sync::atomic::{AtomicU64,Ordering}};
use cap_std::{ambient_authority,fs::Dir};
use lu_core::native_zip_extract::{
    extract_zip_member_new_checked,zip_member_sha256,ZipExtractError,
};
use zip::{ZipWriter,write::SimpleFileOptions,CompressionMethod};
static SEQ:AtomicU64=AtomicU64::new(0);
fn fixture()->(PathBuf,Dir){
    let root=std::env::temp_dir().join(format!("lu-zip-extract-{}-{}",
        std::process::id(),SEQ.fetch_add(1,Ordering::SeqCst)));
    fs::create_dir_all(&root).unwrap();
    let dir=Dir::open_ambient_dir(&root,ambient_authority()).unwrap();
    (root,dir)
}
fn archive(root:&PathBuf,name:&str,entries:&[(&str,&[u8])]){
    let mut z=ZipWriter::new(fs::File::create(root.join(name)).unwrap());
    for (entry,data) in entries {
        z.start_file(*entry,SimpleFileOptions::default()
            .compression_method(CompressionMethod::Stored)).unwrap();
        z.write_all(data).unwrap();
    }
    z.finish().unwrap();
}
#[test]
fn real_zip_extract_exact_member_and_never_overwrite(){
    let (root,dir)=fixture();
    archive(&root,"backup.zip",&[
        ("source/a.txt",b"alpha"),
        ("source/b.txt",b"beta"),
    ]);
    let digest=zip_member_sha256(&dir,"backup.zip","source/a.txt").unwrap();
    let receipt=extract_zip_member_new_checked(
        &dir,"backup.zip","source/a.txt",&dir,"a.txt",
        "zip-transaction-123456789",&digest
    ).unwrap();
    assert_eq!(receipt.bytes_written,5);
    assert_eq!(receipt.sha256,digest);
    assert_eq!(fs::read(root.join("a.txt")).unwrap(),b"alpha");
    assert_eq!(extract_zip_member_new_checked(
        &dir,"backup.zip","source/a.txt",&dir,"a.txt",
        "zip-transaction-987654321",&digest
    ).unwrap_err(),ZipExtractError::TargetExists);
    assert_eq!(fs::read(root.join("a.txt")).unwrap(),b"alpha");
    drop(dir);fs::remove_dir_all(root).unwrap();
}
#[test]
fn wrong_digest_is_never_published(){
    let (root,dir)=fixture();
    archive(&root,"backup.zip",&[("file.txt",b"safe")]);
    assert_eq!(extract_zip_member_new_checked(
        &dir,"backup.zip","file.txt",&dir,"different.txt",
        "zip-transaction-123456789",&"0".repeat(64)
    ).unwrap_err(),ZipExtractError::SourceChanged);
    assert!(!root.join("different.txt").exists());
    drop(dir);fs::remove_dir_all(root).unwrap();
}
#[test]
fn unsafe_members_refused_even_when_selected_member_is_safe(){
    let (root,dir)=fixture();
    archive(&root,"unsafe.zip",&[
        ("safe.txt",b"safe"),
        ("../escape.txt",b"bad"),
    ]);
    assert_eq!(zip_member_sha256(&dir,"unsafe.zip","safe.txt").unwrap_err(),
        ZipExtractError::UnsafeEntry);
    assert!(!root.join("escape.txt").exists());
    drop(dir);fs::remove_dir_all(root).unwrap();
}
#[test]
fn corrupted_stored_member_fails_crc_integrity(){
    let (root,dir)=fixture();
    archive(&root,"backup.zip",&[("file.txt",b"consistent")]);
    let mut bytes=fs::read(root.join("backup.zip")).unwrap();
    let pos=bytes.windows(b"consistent".len()).position(|w|w==b"consistent").unwrap();
    bytes[pos]=b'X';
    fs::write(root.join("backup.zip"),bytes).unwrap();
    assert_eq!(zip_member_sha256(&dir,"backup.zip","file.txt").unwrap_err(),
        ZipExtractError::InvalidZip);
    drop(dir);fs::remove_dir_all(root).unwrap();
}
#[cfg(unix)]
#[test]
fn symlink_zip_source_is_denied(){
    use std::os::unix::fs::symlink;
    let (root,dir)=fixture();
    archive(&root,"true.zip",&[("file.txt",b"safe")]);
    symlink("true.zip",root.join("alias.zip")).unwrap();
    assert_eq!(zip_member_sha256(&dir,"alias.zip","file.txt").unwrap_err(),
        ZipExtractError::NotRegularFile);
    drop(dir);fs::remove_dir_all(root).unwrap();
}

#[test]
fn deflated_zip_stream_has_real_integrity_and_bounded_extract(){
    let (root,dir)=fixture();
    let mut z=ZipWriter::new(fs::File::create(root.join("deflated.zip")).unwrap());
    z.start_file("src/compressed.txt",SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)).unwrap();
    // Not highly repetitive: compressed size should not trip the ZIP-bomb guard.
    let payload=(0..1024).map(|i|format!("{i:04x}-value-{}\n",i%31))
        .collect::<String>();
    z.write_all(payload.as_bytes()).unwrap();
    z.finish().unwrap();
    let digest=zip_member_sha256(&dir,"deflated.zip","src/compressed.txt").unwrap();
    let receipt=extract_zip_member_new_checked(
        &dir,"deflated.zip","src/compressed.txt",&dir,"compressed.txt",
        "transaction-deflate-12345",&digest
    ).unwrap();
    assert_eq!(receipt.bytes_written,payload.len() as u64);
    assert_eq!(fs::read(root.join("compressed.txt")).unwrap(),payload.as_bytes());
    drop(dir);fs::remove_dir_all(root).unwrap();
}
