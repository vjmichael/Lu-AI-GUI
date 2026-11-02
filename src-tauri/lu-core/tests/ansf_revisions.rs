use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use lu_core::{
    ansf::{AnsfError, AnsfStore, EntryRole},
    ansf_revisions::RevisionOrigin,
};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn db_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "lu-ansf-edits-{}-{}-{}.db",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ))
}

fn new_store(path: &std::path::Path) -> AnsfStore {
    let store = AnsfStore::open(path).unwrap();
    store
        .create_session("chat", "workspace", "Editing history")
        .unwrap();
    store.start_chapter("chat", "Decisions").unwrap();
    store
}

#[test]
fn edit_appends_new_entry_and_immutable_link_then_resolves_current() {
    let path = db_path();
    let store = new_store(&path);
    let original = store
        .append_entry(
            "chat",
            "message-1",
            EntryRole::User,
            "Initial architecture",
            "Use the previous architecture",
        )
        .unwrap();
    let rev = store
        .append_edited_entry(
            "chat",
            "message-1",
            "Updated architecture",
            "Use the new architecture",
            RevisionOrigin::VerifiedUser,
        )
        .unwrap()
        .unwrap();
    assert_eq!(rev.original_entry_id, original);
    assert_eq!(rev.previous_entry_id, original);
    assert_eq!(rev.revision_number, 1);
    assert_eq!(rev.origin, RevisionOrigin::VerifiedUser);
    assert_eq!(rev.note, "Appended: message was edited by user.");
    assert!(rev.observed_at_utc.ends_with('Z'));
    let current = store.current_message("chat", "message-1").unwrap().unwrap();
    assert_eq!(current.entry.content, "Use the new architecture");
    assert_eq!(current.revision.unwrap(), rev);
    assert_eq!(
        store.read_entry("chat", original).unwrap().unwrap().content,
        "Use the previous architecture"
    );
    assert_eq!(
        store.find_message("chat", "message-1").unwrap().unwrap().id,
        original
    );
    drop(store);
    let reopened = AnsfStore::open(&path).unwrap();
    assert_eq!(
        reopened.revision_history("chat", "message-1").unwrap(),
        vec![rev]
    );
    assert_eq!(
        reopened
            .current_message("chat", "message-1")
            .unwrap()
            .unwrap()
            .entry
            .content,
        "Use the new architecture"
    );
    drop(reopened);
    let _ = std::fs::remove_file(path);
}

#[test]
fn multiple_edits_form_a_chain_and_repeating_current_content_does_not_duplicate() {
    let path = db_path();
    let store = new_store(&path);
    let original = store
        .append_entry("chat", "msg", EntryRole::User, "A", "A")
        .unwrap();
    let rev1 = store
        .append_edited_entry("chat", "msg", "B", "B", RevisionOrigin::VerifiedUser)
        .unwrap()
        .unwrap();
    assert!(store
        .append_edited_entry("chat", "msg", "B", "B", RevisionOrigin::VerifiedUser,)
        .unwrap()
        .is_none());
    let rev2 = store
        .append_edited_entry("chat", "msg", "C", "C", RevisionOrigin::ImportObserved)
        .unwrap()
        .unwrap();
    assert_eq!(rev1.previous_entry_id, original);
    assert_eq!(rev2.previous_entry_id, rev1.revised_entry_id);
    assert_eq!(rev2.revision_number, 2);
    assert_eq!(
        store.revision_history("chat", "msg").unwrap(),
        vec![rev1, rev2]
    );
    assert_eq!(
        store
            .current_message("chat", "msg")
            .unwrap()
            .unwrap()
            .entry
            .content,
        "C"
    );
    assert_eq!(store.page_after("chat", 0, 10).unwrap().len(), 3);
    drop(store);
    let _ = std::fs::remove_file(path);
}

#[test]
fn links_and_entries_cannot_be_updated_or_deleted_even_by_sql() {
    let path = db_path();
    let store = new_store(&path);
    store
        .append_entry("chat", "msg", EntryRole::User, "First", "Original")
        .unwrap();
    let revised = store
        .append_edited_entry(
            "chat",
            "msg",
            "Second",
            "Revised",
            RevisionOrigin::VerifiedUser,
        )
        .unwrap()
        .unwrap();
    let conn = rusqlite::Connection::open(&path).unwrap();
    assert!(conn
        .execute(
            "UPDATE ansf_entry_edits SET note='rewrote history' WHERE revised_entry_id=?1",
            [revised.revised_entry_id],
        )
        .is_err());
    assert!(conn
        .execute(
            "DELETE FROM ansf_entry_edits WHERE revised_entry_id=?1",
            [revised.revised_entry_id],
        )
        .is_err());
    assert!(conn
        .execute(
            "UPDATE ansf_entries SET content='rewritten' WHERE id=?1",
            [revised.revised_entry_id],
        )
        .is_err());
    drop(conn);
    drop(store);
    let _ = std::fs::remove_file(path);
}

#[test]
fn current_lookup_is_conversation_scoped_and_invalid_ids_are_rejected() {
    let path = db_path();
    let store = new_store(&path);
    store
        .create_session("other", "workspace", "Other chat")
        .unwrap();
    store.start_chapter("other", "Unrelated").unwrap();
    store
        .append_entry("chat", "msg", EntryRole::User, "One", "Original")
        .unwrap();
    assert!(store.current_message("other", "msg").unwrap().is_none());
    assert!(store.revision_history("other", "msg").unwrap().is_empty());
    assert_eq!(
        store.append_edited_entry(
            "other",
            "msg",
            "No source",
            "No source",
            RevisionOrigin::VerifiedUser,
        ),
        Err(AnsfError::InvalidInput)
    );
    assert_eq!(
        store.append_edited_entry("chat", "msg", "", "No", RevisionOrigin::VerifiedUser,),
        Err(AnsfError::InvalidInput)
    );
    assert!(store
        .revision_history("chat", "missing")
        .unwrap()
        .is_empty());
    drop(store);
    let _ = std::fs::remove_file(path);
}

#[test]
fn source_text_search_retains_old_wording_and_new_version_is_indexed() {
    let path = db_path();
    let store = new_store(&path);
    store
        .append_entry(
            "chat",
            "msg",
            EntryRole::User,
            "Futuristic blueprints",
            "Old wording: zeppelin",
        )
        .unwrap();
    store
        .append_edited_entry(
            "chat",
            "msg",
            "Revised blueprints",
            "New wording: asteroid",
            RevisionOrigin::VerifiedUser,
        )
        .unwrap();
    assert_eq!(store.search("chat", "zeppelin", 10).unwrap().len(), 1);
    assert_eq!(store.search("chat", "asteroid", 10).unwrap().len(), 1);
    assert_eq!(
        store
            .current_message("chat", "msg")
            .unwrap()
            .unwrap()
            .entry
            .content,
        "New wording: asteroid"
    );
    drop(store);
    let _ = std::fs::remove_file(path);
}
