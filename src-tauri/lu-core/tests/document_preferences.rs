use lu_core::document_preferences::{DocumentPreferences,DocumentPreferenceError};
use std::{fs,path::PathBuf,sync::atomic::{AtomicU64,Ordering},time::{SystemTime,UNIX_EPOCH}};
static FIXTURE_SEQUENCE: AtomicU64=AtomicU64::new(0);
fn root()->PathBuf{
    let p=std::env::temp_dir().join(format!("lu-docs-pref-{}-{}-{}",
        std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos(),
        FIXTURE_SEQUENCE.fetch_add(1,Ordering::Relaxed)));
    fs::create_dir_all(&p).unwrap();p
}
#[test]
fn uses_system_documents_until_explicit_native_selection_and_survives_reopen(){
    let root=root();
    let docs=root.join("Documents");
    let alt=root.join("WorkNotes");
    fs::create_dir_all(&docs).unwrap();fs::create_dir_all(&alt).unwrap();
    let file=root.join("settings.sqlite");
    let db=DocumentPreferences::open(&file).unwrap();
    assert!(!db.is_custom().unwrap());
    assert_eq!(db.selected(&docs).unwrap(),docs.canonicalize().unwrap());
    db.set_from_native_picker(&alt).unwrap();
    assert!(db.is_custom().unwrap());
    assert_eq!(db.selected(&docs).unwrap(),alt.canonicalize().unwrap());
    drop(db);
    let db=DocumentPreferences::open(&file).unwrap();
    assert_eq!(db.selected(&docs).unwrap(),alt.canonicalize().unwrap());
    db.reset_to_system_documents(&docs).unwrap();
    assert!(!db.is_custom().unwrap());
    assert_eq!(db.selected(&docs).unwrap(),docs.canonicalize().unwrap());
    drop(db);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn refuses_symlinks_missing_directories_and_never_silently_uses_app_data(){
    let root=root();let docs=root.join("Documents");
    let file=root.join("settings.sqlite");
    let db=DocumentPreferences::open(&file).unwrap();
    assert_eq!(db.selected(&docs),Err(DocumentPreferenceError::InvalidDirectory));
    assert_eq!(db.set_from_native_picker(&root.join("missing")),
        Err(DocumentPreferenceError::InvalidDirectory));
    fs::create_dir_all(&docs).unwrap();
    #[cfg(unix)] {
        use std::os::unix::fs::symlink;
        let link=root.join("shortcut");
        symlink(&docs,&link).unwrap();
        assert_eq!(db.set_from_native_picker(&link),
            Err(DocumentPreferenceError::InvalidDirectory));
    }
    assert_eq!(db.selected(&docs).unwrap(),docs.canonicalize().unwrap());
    drop(db);fs::remove_dir_all(root).unwrap();
}

#[test]
fn saved_file_link_requires_approved_record_matching_actual_applied_effect() {
    use lu_core::{
        document_preferences::verified_created_folder,
        journal::{EffectRecord,EffectState},
        native_create_new::TOOL_ID,
        write_approval_ledger::{WriteDecision,WriteDecisionRecord},
    };
    let root=root();let docs=root.join("Documents");
    fs::create_dir_all(&docs).unwrap();
    let mut approval=WriteDecisionRecord{
        proposal_id:"pw-one".into(),chat_id:"chat-a".into(),generation_id:"g-model".into(),
        file_name:"report.md".into(),bytes:10,sha256:"a".repeat(64),
        decision:WriteDecision::Approved,
        selected_target:Some(docs.join("report.md").to_string_lossy().into_owned()),
        effect_call_id:Some("g-effect-one".into()),
    };
    let mut effect=EffectRecord {
        call_id:"g-effect-one".into(),tool_id:TOOL_ID.into(),
        fingerprint:"hash".into(),state:EffectState::Applied,receipt:Some(vec![1]),
    };
    assert_eq!(verified_created_folder(&approval,&effect).unwrap(),docs);
    effect.state=EffectState::Uncertain;
    assert!(verified_created_folder(&approval,&effect).is_err());
    effect.state=EffectState::Applied;
    effect.receipt=None;
    assert!(verified_created_folder(&approval,&effect).is_err());
    effect.receipt=Some(vec![1]);
    effect.call_id="g-other".into();
    assert!(verified_created_folder(&approval,&effect).is_err());
    effect.call_id="g-effect-one".into();
    approval.decision=WriteDecision::Interrupted;
    assert!(verified_created_folder(&approval,&effect).is_err());
    approval.decision=WriteDecision::Approved;
    approval.selected_target=Some(docs.join("different.txt").to_string_lossy().into_owned());
    assert!(verified_created_folder(&approval,&effect).is_err());
    fs::remove_dir_all(root).unwrap();
}
