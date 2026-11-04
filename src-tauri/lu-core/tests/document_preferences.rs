use lu_core::document_preferences::{DocumentPreferences,DocumentPreferenceError};
use std::{fs,path::PathBuf,time::{SystemTime,UNIX_EPOCH}};
fn root()->PathBuf{
    let p=std::env::temp_dir().join(format!("lu-docs-pref-{}-{}",
        std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
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
