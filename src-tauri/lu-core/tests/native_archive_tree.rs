use std::{fs,io::Write,sync::atomic::{AtomicU64,Ordering}};
use cap_std::{ambient_authority,fs::Dir};
use zip::{ZipWriter,write::SimpleFileOptions,CompressionMethod};
use lu_core::{
    native_archive_tree::{
        preview_archive_tree,extract_archive_tree_new,
        observe_tree_completion,TreeExtractError,
    },
    native_linux_archive::create_linux_archive_new,
};
static NEXT:AtomicU64=AtomicU64::new(0);
fn fixture()->(std::path::PathBuf,Dir){
    let root=std::env::temp_dir().join(format!("lu-archive-tree-{}-{}",
        std::process::id(),NEXT.fetch_add(1,Ordering::SeqCst)));
    fs::create_dir_all(&root).unwrap();
    let dir=Dir::open_ambient_dir(&root,ambient_authority()).unwrap();
    (root,dir)
}
fn archive(root:&std::path::Path,name:&str,evil:bool){
    let mut z=ZipWriter::new(fs::File::create(root.join(name)).unwrap());
    z.add_directory("parent/empty/",SimpleFileOptions::default()).unwrap();
    z.start_file("parent/nested/alpha.txt",SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)).unwrap();
    z.write_all(b"alpha & beta").unwrap();
    z.start_file(if evil{"../escape.txt"}else{"parent/beta.txt"},
        SimpleFileOptions::default()
            .compression_method(CompressionMethod::Stored)).unwrap();
    z.write_all(b"second").unwrap();
    z.finish().unwrap();
}
#[test]
fn zip_tree_is_bounded_new_folder_and_manifest_fully_reconciles(){
    let (root,dir)=fixture();
    archive(&root,"source.zip",false);
    let manifest=preview_archive_tree(&dir,"source.zip").unwrap();
    assert_eq!(manifest.file_count,2);
    let receipt=extract_archive_tree_new(
        &dir,"source.zip",&dir,"expanded",&manifest.sha256
    ).unwrap();
    assert_eq!(receipt.files_extracted,2);
    assert_eq!(fs::read(root.join("expanded/parent/nested/alpha.txt")).unwrap(),
        b"alpha & beta");
    assert_eq!(fs::read(root.join("expanded/parent/beta.txt")).unwrap(),b"second");
    assert!(root.join("expanded/parent/empty").is_dir());
    assert!(observe_tree_completion(&dir,"expanded",&manifest).unwrap());
    assert_eq!(extract_archive_tree_new(
        &dir,"source.zip",&dir,"expanded",&manifest.sha256
    ).unwrap_err(),TreeExtractError::TargetExists);
    fs::write(root.join("expanded/parent/beta.txt"),b"tampered").unwrap();
    assert!(!observe_tree_completion(&dir,"expanded",&manifest).unwrap());
    drop(dir);fs::remove_dir_all(root).unwrap();
}
#[test]
fn tar_source_and_incomplete_unexpected_member_are_detected(){
    let (root,dir)=fixture();
    fs::write(root.join("first.txt"),b"one").unwrap();
    fs::write(root.join("second.txt"),b"two").unwrap();
    create_linux_archive_new(
        &dir,"source.tar",&["first.txt".into(),"second.txt".into()],
        "transaction-123456789"
    ).unwrap();
    let manifest=preview_archive_tree(&dir,"source.tar").unwrap();
    let receipt=extract_archive_tree_new(
        &dir,"source.tar",&dir,"tar-output",&manifest.sha256
    ).unwrap();
    assert_eq!(receipt.files_extracted,2);
    assert!(observe_tree_completion(&dir,"tar-output",&manifest).unwrap());
    fs::write(root.join("tar-output/unexpected.txt"),b"tampered").unwrap();
    assert!(!observe_tree_completion(&dir,"tar-output",&manifest).unwrap());
    drop(dir);fs::remove_dir_all(root).unwrap();
}
#[test]
fn unsafe_entry_or_unapproved_manifest_does_not_create_directory(){
    let (root,dir)=fixture();
    archive(&root,"unsafe.zip",true);
    assert_eq!(preview_archive_tree(&dir,"unsafe.zip").unwrap_err(),
        TreeExtractError::UnsafeMember);
    assert!(!root.join("escape.txt").exists());
    archive(&root,"source.zip",false);
    assert_eq!(extract_archive_tree_new(
        &dir,"source.zip",&dir,"must-not-exist",&"0".repeat(64)
    ).unwrap_err(),TreeExtractError::ManifestChanged);
    assert!(!root.join("must-not-exist").exists());
    drop(dir);fs::remove_dir_all(root).unwrap();
}
