use lu_ide_workspace::draft_recovery::DraftRecoveryStore;
use std::{fs, path::PathBuf};
fn fixture()->(PathBuf, String, String) {
    let root=std::env::temp_dir().join(format!("lu-ide-drafts-{}-{:016x}",
        std::process::id(),rand::random::<u64>()));
    fs::create_dir_all(&root).unwrap();
    let id=format!("ide-{:032x}",rand::random::<u128>());
    (root,id,"src/main.rs".to_owned())
}
fn initial_sha()->String{"a".repeat(64)}

#[test]
fn checkpoint_survives_native_app_restart_and_cannot_silently_overwrite_previous_session() {
    let (root,id,path)=fixture();
    let vault=DraftRecoveryStore::open(root.join("draft-recovery")).unwrap();
    let first=vault.checkpoint(&id,&path,&initial_sha(),1,"fn draft() {}")
        .unwrap();
    assert_eq!(first.buffer_revision,1);
    assert!(!first.from_previous_session);
    assert!(first.draft_sha256.len()==64);
    drop(vault);
    let newer=DraftRecoveryStore::open(root.join("draft-recovery")).unwrap();
    let listed=newer.list(&id).unwrap();
    assert_eq!(listed.len(),1);
    assert!(listed[0].from_previous_session);
    assert_eq!(newer.checkpoint(&id,&path,&initial_sha(),2,"fn new() {}")
        .unwrap_err().code(),"DRAFT_RECOVERY_PREVIOUS_SESSION_REVIEW_REQUIRED");
    let adopt=newer.adopt(&id,&path,&first.draft_sha256).unwrap();
    assert_eq!(adopt.draft_text,"fn draft() {}");
    let saved=newer.checkpoint(&id,&path,&initial_sha(),2,"fn next() {}")
        .unwrap();
    assert_eq!(saved.buffer_revision,2);
    assert!(!saved.from_previous_session);
    assert_eq!(newer.forget(&id,&path,&first.draft_sha256)
        .unwrap_err().code(),"DRAFT_RECOVERY_STALE");
    newer.forget(&id,&path,&saved.draft_sha256).unwrap();
    assert!(newer.list(&id).unwrap().is_empty());
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn mutation_revision_cannot_regress_or_replace_same_revision_with_different_text(){
    let (root,id,path)=fixture();
    let v=DraftRecoveryStore::open(root.join("draft-recovery")).unwrap();
    v.checkpoint(&id,&path,&initial_sha(),5,"new draft").unwrap();
    assert_eq!(v.checkpoint(&id,&path,&initial_sha(),4,"old draft")
        .unwrap_err().code(),"DRAFT_RECOVERY_STALE");
    assert_eq!(v.checkpoint(&id,&path,&initial_sha(),5,"other")
        .unwrap_err().code(),"DRAFT_RECOVERY_STALE");
    assert_eq!(v.checkpoint(&id,&path,&"b".repeat(64),6,"other")
        .unwrap_err().code(),"DRAFT_RECOVERY_STALE");
    assert_eq!(v.list(&id).unwrap().len(),1);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn invalid_host_input_and_large_buffers_fail_without_making_a_checkpoint(){
    let (root,id,path)=fixture();
    let v=DraftRecoveryStore::open(root.join("draft-recovery")).unwrap();
    for bad in ["../secret","/tmp/absolute",".git/config","a\\b","a/../../other"] {
        assert_eq!(v.checkpoint(&id,bad,&initial_sha(),1,"invalid")
            .unwrap_err().code(),"DRAFT_RECOVERY_INVALID");
    }
    assert_eq!(v.checkpoint(&id,&path,"invalid",1,"bad")
        .unwrap_err().code(),"DRAFT_RECOVERY_INVALID");
    assert_eq!(v.checkpoint(&id,&path,&initial_sha(),0,"bad")
        .unwrap_err().code(),"DRAFT_RECOVERY_INVALID");
    assert_eq!(v.checkpoint(&id,&path,&initial_sha(),1,&"x".repeat(512*1024+1))
        .unwrap_err().code(),"DRAFT_RECOVERY_INVALID");
    assert!(v.list(&id).unwrap().is_empty());
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn tampered_json_not_silently_recovered_or_erased(){
    let (root,id,path)=fixture();
    let v=DraftRecoveryStore::open(root.join("draft-recovery")).unwrap();
    v.checkpoint(&id,&path,&initial_sha(),1,"my draft").unwrap();
    let vault=root.join("draft-recovery");
    let record=fs::read_dir(vault).unwrap().map(|entry|entry.unwrap().path())
        .find(|path|path.extension().is_some_and(|ext|ext=="json")).unwrap();
    fs::write(&record,b"{\"version\":1,\"forged\":true}").unwrap();
    assert_eq!(v.list(&id).unwrap_err().code(),"DRAFT_RECOVERY_INVALID");
    assert_eq!(v.adopt(&id,&path,&initial_sha()).unwrap_err().code(),
        "DRAFT_RECOVERY_INVALID");
    fs::remove_dir_all(root).unwrap();
}
#[cfg(unix)]
#[test]
fn vault_root_symlink_is_rejected_even_if_target_is_a_real_directory(){
    use std::os::unix::fs::symlink;
    let (root,_,_)=fixture();
    fs::create_dir(root.join("outside")).unwrap();
    symlink(root.join("outside"),root.join("draft-recovery")).unwrap();
    assert_eq!(DraftRecoveryStore::open(root.join("draft-recovery"))
        .err().unwrap().code(),"DRAFT_RECOVERY_INVALID");
    fs::remove_dir_all(root).unwrap();
}
