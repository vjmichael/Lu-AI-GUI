use lu_ide_workspace::draft_vault::{DraftCheckpointInput,DraftVault};
use std::{fs,path::PathBuf};
use sha2::Digest;

fn root()->PathBuf {
    std::env::temp_dir().join(format!("lu-b2-draft-test-{}-{:016x}",
        std::process::id(),rand::random::<u64>()))
}
fn setup()->(PathBuf,DraftVault){
    let base=root();
    fs::create_dir(&base).unwrap();
    let vault=DraftVault::open(base.join("private-drafts")).unwrap();
    (base,vault)
}
fn input(expected:Option<String>,rev:u64,text:&str)->DraftCheckpointInput{
    DraftCheckpointInput{
        project_id:format!("ide-{}","a".repeat(32)),
        relative_path:"src/unwritten.rs".into(),
        buffer_id:"12345678-abcd-4321-aaaa-123456789abc".into(),
        buffer_revision:rev,
        base_disk_revision:"b".repeat(64),
        text:text.into(),expected_head:expected,
    }
}
#[test]
fn restart_reconstructs_exact_immutable_references_without_writing_project_files(){
    let (base,vault)=setup();
    let initial=vault.checkpoint(input(None,1,"before 😀\r\n")).unwrap();
    assert_eq!(initial.buffer_revision,1);
    assert_eq!(initial.draft_sha256.len(),64);
    let next=vault.checkpoint(input(Some(initial.checkpoint_id.clone()),
        2,"after 😀\r\n")).unwrap();
    assert_ne!(initial.checkpoint_id,next.checkpoint_id);
    assert_eq!(vault.load(&initial.project_id,&initial.relative_path,
        &initial.checkpoint_id).unwrap().text,"before 😀\r\n");
    drop(vault);
    let reopened=DraftVault::open(base.join("private-drafts")).unwrap();
    assert_eq!(reopened.latest(&next.project_id,&next.relative_path)
        .unwrap().unwrap().checkpoint_id,next.checkpoint_id);
    assert_eq!(reopened.load(&initial.project_id,&initial.relative_path,
        &initial.checkpoint_id).unwrap().text,initial.text);
    assert!(!base.join("project/src/unwritten.rs").exists());
    fs::remove_dir_all(base).unwrap();
}
#[test]
fn cas_rejects_stale_delayed_older_revision_and_unapproved_replacement(){
    let (base,vault)=setup();
    let first=vault.checkpoint(input(None,1,"edit")).unwrap();
    assert_eq!(vault.checkpoint(input(None,2,"wrong")).unwrap_err(),
        "DRAFT_CHECKPOINT_CONFLICT");
    assert_eq!(vault.checkpoint(input(Some(first.checkpoint_id.clone()),
        1,"older")).unwrap_err(),"DRAFT_REVISION_STALE");
    let later=vault.checkpoint(input(Some(first.checkpoint_id.clone()),2,
        "newer")).unwrap();
    assert_eq!(vault.discard(&first.project_id,&first.relative_path,
        &first.checkpoint_id).unwrap_err(),"DRAFT_CHECKPOINT_CONFLICT");
    assert_eq!(vault.latest(&first.project_id,&first.relative_path)
        .unwrap().unwrap().checkpoint_id,later.checkpoint_id);
    fs::remove_dir_all(base).unwrap();
}
#[test]
fn invalid_paths_ids_large_payload_and_tampered_bytes_are_denied(){
    let (base,vault)=setup();
    let mut data=input(None,1,"draft");
    data.relative_path="../secret".into();
    assert_eq!(vault.checkpoint(data).unwrap_err(),"DRAFT_IDENTITY_INVALID");
    let mut data=input(None,1,"draft");
    data.buffer_id="NO".into();
    assert_eq!(vault.checkpoint(data).unwrap_err(),"DRAFT_IDENTITY_INVALID");
    let mut data=input(None,1,"x");
    data.text="x".repeat(512*1024+1);
    assert_eq!(vault.checkpoint(data).unwrap_err(),"DRAFT_IDENTITY_INVALID");
    let snap=vault.checkpoint(input(None,1,"authentic draft")).unwrap();
    let path=base.join("private-drafts").join(&snap.project_id)
        .join(format!("{:x}",sha2::Sha256::digest(snap.relative_path.as_bytes())))
        .join(format!("{}.json",snap.checkpoint_id));
    fs::write(&path,b"{\"text\":\"forged\"}").unwrap();
    assert_eq!(vault.load(&snap.project_id,&snap.relative_path,
        &snap.checkpoint_id).unwrap_err(),"DRAFT_RECORD_UNTRUSTED");
    fs::remove_dir_all(base).unwrap();
}
#[test]
fn discard_removes_only_private_drafts_and_requires_exact_current_head(){
    let (base,vault)=setup();
    let source=base.join("project/source.rs");
    fs::create_dir(source.parent().unwrap()).unwrap();
    fs::write(&source,b"original source").unwrap();
    let snap=vault.checkpoint(input(None,1,"unsaved edits")).unwrap();
    vault.discard(&snap.project_id,&snap.relative_path,&snap.checkpoint_id).unwrap();
    assert!(vault.latest(&snap.project_id,&snap.relative_path).unwrap().is_none());
    assert_eq!(fs::read(&source).unwrap(),b"original source");
    fs::remove_dir_all(base).unwrap();
}
#[cfg(unix)]
#[test]
fn symlinked_private_directory_is_refused(){
    use std::os::unix::fs::symlink;
    let base=root();
    fs::create_dir(&base).unwrap();
    let other=base.join("other");
    fs::create_dir(&other).unwrap();
    symlink(&other,base.join("private-drafts")).unwrap();
    assert_eq!(DraftVault::open(base.join("private-drafts")).err(),
        Some("DRAFT_UNSAFE_DIRECTORY"));
    fs::remove_dir_all(base).unwrap();
}


#[cfg(unix)]
#[test]
fn existing_world_readable_private_draft_directory_is_denied(){
    use std::os::unix::fs::PermissionsExt;
    let base=root();
    fs::create_dir(&base).unwrap();
    let unsafe_root=base.join("private-drafts");
    fs::create_dir(&unsafe_root).unwrap();
    fs::set_permissions(&unsafe_root,fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(DraftVault::open(unsafe_root).err(),Some("DRAFT_UNSAFE_DIRECTORY"));
    fs::remove_dir_all(base).unwrap();
}
