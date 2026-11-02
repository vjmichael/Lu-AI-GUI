use lu_core::{
    ansf::AnsfStore,
    ansf_import::{ImportError, JanImportPolicy},
    ansf_library::{AnsfLibrary, BookIndex},
    ansf_sync::{reconcile_book_index, sync_verified_snapshot, SyncError},
};
use std::{
    io::Cursor,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

fn path(name: &str) -> PathBuf {
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("lu-sync-{name}-{}-{t}.sqlite", std::process::id()))
}
fn book() -> BookIndex {
    BookIndex {
        session_id: "jan-thread".into(),
        workspace_id: "lu".into(),
        relative_path: "sessions/jan-thread.sqlite".into(),
        title: "Jan history".into(),
        summary: "Imported conversation".into(),
    }
}

#[test]
fn import_reindex_and_reopen_are_idempotent() {
    let db = path("book");
    let index = path("library");
    let content =
        r#"{"id":"m1","thread_id":"jan-thread","role":"user","content":"memory coordination"}"#;
    {
        let store = AnsfStore::open(&db).unwrap();
        store
            .create_session("jan-thread", "lu", "Jan history")
            .unwrap();
        let library = AnsfLibrary::open(&index).unwrap();
        let stats = sync_verified_snapshot(
            &store,
            &library,
            &book(),
            Cursor::new(content),
            JanImportPolicy::RejectChangedSource,
        )
        .unwrap();
        assert_eq!(stats.added, 1);
        assert!(library.get_book("jan-thread").unwrap().is_some());
        assert_eq!(library.search_chapters("Imported", 5).unwrap().len(), 1);
    }
    {
        let store = AnsfStore::open(&db).unwrap();
        let library = AnsfLibrary::open(&index).unwrap();
        let stats = sync_verified_snapshot(
            &store,
            &library,
            &book(),
            Cursor::new(content),
            JanImportPolicy::RejectChangedSource,
        )
        .unwrap();
        assert_eq!(stats.unchanged, 1);
        assert_eq!(store.page_after("jan-thread", 0, 10).unwrap().len(), 1);
        reconcile_book_index(&store, &library, &book()).unwrap();
        assert_eq!(library.search_chapters("Imported", 5).unwrap().len(), 1);
    }
    let _ = std::fs::remove_file(db);
    let _ = std::fs::remove_file(index);
}

#[test]
fn interrupted_import_can_reconcile_an_existing_book() {
    let db = path("partial");
    let index = path("catalog");
    let store = AnsfStore::open(&db).unwrap();
    store
        .create_session("jan-thread", "lu", "Jan history")
        .unwrap();
    let library = AnsfLibrary::open(&index).unwrap();
    let input = concat!(
        "{\"id\":\"first\",\"role\":\"user\",\"content\":\"first\"}\n",
        "invalid JSON\n"
    );
    assert!(matches!(
        sync_verified_snapshot(
            &store,
            &library,
            &book(),
            Cursor::new(input),
            JanImportPolicy::RejectChangedSource
        ),
        Err(SyncError::Import(ImportError::InvalidRecord {
            line: 2,
            ..
        }))
    ));
    assert!(library.get_book("jan-thread").unwrap().is_none());
    assert_eq!(store.page_after("jan-thread", 0, 10).unwrap().len(), 1);
    reconcile_book_index(&store, &library, &book()).unwrap();
    assert!(library.get_book("jan-thread").unwrap().is_some());
    let replay = concat!(
        "{\"id\":\"first\",\"role\":\"user\",\"content\":\"first\"}\n",
        "{\"id\":\"second\",\"role\":\"assistant\",\"content\":\"second\"}\n"
    );
    let stats = sync_verified_snapshot(
        &store,
        &library,
        &book(),
        Cursor::new(replay),
        JanImportPolicy::RejectChangedSource,
    )
    .unwrap();
    assert_eq!((stats.added, stats.unchanged), (1, 1));
    let _ = std::fs::remove_file(db);
    let _ = std::fs::remove_file(index);
}

#[test]
fn changed_source_fails_closed_without_reindexing_it() {
    let db = path("change");
    let index = path("change-index");
    let store = AnsfStore::open(&db).unwrap();
    store
        .create_session("jan-thread", "lu", "Jan history")
        .unwrap();
    let library = AnsfLibrary::open(&index).unwrap();
    let first = r#"{"id":"a","role":"user","content":"original"}"#;
    let edited = r#"{"id":"a","role":"user","content":"edited"}"#;
    sync_verified_snapshot(
        &store,
        &library,
        &book(),
        Cursor::new(first),
        JanImportPolicy::RejectChangedSource,
    )
    .unwrap();
    assert!(matches!(
        sync_verified_snapshot(
            &store,
            &library,
            &book(),
            Cursor::new(edited),
            JanImportPolicy::RejectChangedSource
        ),
        Err(SyncError::Import(ImportError::ChangedSource { .. }))
    ));
    assert_eq!(
        store
            .current_message("jan-thread", "a")
            .unwrap()
            .unwrap()
            .entry
            .content,
        first
    );
    let _ = std::fs::remove_file(db);
    let _ = std::fs::remove_file(index);
}

#[test]
fn ensuring_session_preserves_identity_and_original_title() {
    let db = path("session-identity");
    let store = AnsfStore::open(&db).unwrap();
    store
        .ensure_session("jan-thread", "lu", "Original title")
        .unwrap();
    store
        .ensure_session("jan-thread", "lu", "Replayed title")
        .unwrap();
    assert!(store
        .ensure_session("jan-thread", "other-workspace", "Other")
        .is_err());
    let _ = std::fs::remove_file(db);
}
