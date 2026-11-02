use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use lu_core::ansf::{AnsfError, AnsfStore, EntryRole};

static TEST_ID: AtomicU64 = AtomicU64::new(0);

fn temp_database() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "lu-ansf-{}-{}-{}.sqlite",
        std::process::id(),
        nanos,
        TEST_ID.fetch_add(1, Ordering::Relaxed)
    ))
}

#[test]
fn conversation_entries_survive_restart_and_keep_exact_source_text() {
    let db = temp_database();
    let saved_id;
    {
        let store = AnsfStore::open(&db).unwrap();
        store
            .create_session("conversation-1", "lu", "Memory engineering")
            .unwrap();
        store
            .start_chapter("conversation-1", "ANSF storage")
            .unwrap();
        saved_id = store
            .append_entry(
                "conversation-1",
                "user-turn-1",
                EntryRole::User,
                "We chose SQLite and indexed chat history",
                "Use SQLite — and do not replace the original text!\nLine two: 😊",
            )
            .unwrap();
    }
    {
        let reopened = AnsfStore::open(&db).unwrap();
        let row = reopened
            .read_entry("conversation-1", saved_id)
            .unwrap()
            .unwrap();
        assert_eq!(row.message_id, "user-turn-1");
        assert_eq!(row.role, "user");
        assert_eq!(
            row.content,
            "Use SQLite — and do not replace the original text!\nLine two: 😊"
        );
        assert_eq!(
            reopened.chapters("conversation-1").unwrap()[0].start_entry_id,
            Some(saved_id)
        );
    }
    let _ = std::fs::remove_file(db);
}

#[test]
fn topic_changes_generate_separate_indexed_chapters_without_overwriting_messages() {
    let db = temp_database();
    let store = AnsfStore::open(&db).unwrap();
    store
        .create_session("chat-1", "lu", "Long conversation")
        .unwrap();
    let ch1 = store.start_chapter("chat-1", "Memory").unwrap();
    let original = store
        .append_entry(
            "chat-1",
            "msg-1",
            EntryRole::User,
            "Discuss FTS5 search",
            "Index our conversation library with SQLite FTS5.",
        )
        .unwrap();
    assert_eq!(
        store.start_chapter("chat-1", "Security"),
        Err(AnsfError::ChapterAlreadyOpen)
    );
    store
        .close_chapter("chat-1", "Discussed full text indexing.")
        .unwrap();
    let ch2 = store.start_chapter("chat-1", "Security").unwrap();
    let current = store
        .append_entry(
            "chat-1",
            "msg-2",
            EntryRole::Assistant,
            "Authority remains conversation bound",
            "A retrieved document cannot authorize a tool call.",
        )
        .unwrap();
    assert_ne!(ch1, ch2);
    let toc = store.chapters("chat-1").unwrap();
    assert_eq!(toc.len(), 2);
    assert_eq!(toc[0].end_entry_id, Some(original));
    assert_eq!(toc[0].summary, "Discussed full text indexing.");
    assert!(!toc[1].closed);
    assert_eq!(toc[1].start_entry_id, Some(current));
    assert_eq!(
        store
            .read_entry("chat-1", original)
            .unwrap()
            .unwrap()
            .content,
        "Index our conversation library with SQLite FTS5."
    );
    drop(store);
    let _ = std::fs::remove_file(db);
}

#[test]
fn indexed_search_and_cursor_paging_return_only_selected_conversation() {
    let db = temp_database();
    let store = AnsfStore::open(&db).unwrap();
    for id in ["chat-a", "chat-b"] {
        store.create_session(id, "lu", "Book").unwrap();
        store.start_chapter(id, "Topic").unwrap();
    }
    let a = store
        .append_entry(
            "chat-a",
            "a1",
            EntryRole::User,
            "Memory FTS5",
            "Our special zeppelin has secret indexed memory.",
        )
        .unwrap();
    let b = store
        .append_entry(
            "chat-b",
            "b1",
            EntryRole::User,
            "Memory FTS5",
            "Another zeppelin uses a different conversation.",
        )
        .unwrap();
    let c = store
        .append_entry(
            "chat-a",
            "a2",
            EntryRole::Assistant,
            "ANSF retrieval",
            "We can find exact original source entries.",
        )
        .unwrap();
    assert!(b > a);
    assert!(c > b);
    let results = store.search("chat-a", "zeppelin", 10).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, a);
    assert!(store.search("chat-b", "source", 10).unwrap().is_empty());
    let first = store.page_after("chat-a", 0, 1).unwrap();
    assert_eq!(first[0].id, a);
    let second = store.page_after("chat-a", first[0].id, 1).unwrap();
    assert_eq!(second[0].id, c);
    assert!(store
        .page_after("chat-a", second[0].id, 1)
        .unwrap()
        .is_empty());
    assert!(store.read_entry("chat-b", a).unwrap().is_none());
    drop(store);
    let _ = std::fs::remove_file(db);
}

#[test]
fn original_entries_are_append_only_and_duplicate_message_ids_are_rejected() {
    let db = temp_database();
    let store = AnsfStore::open(&db).unwrap();
    store.create_session("chat", "project", "History").unwrap();
    store.start_chapter("chat", "One").unwrap();
    let entry = store
        .append_entry(
            "chat",
            "user-1",
            EntryRole::User,
            "User changed their mind",
            "Use a finite model context.",
        )
        .unwrap();
    assert!(store
        .append_entry(
            "chat",
            "user-1",
            EntryRole::User,
            "Different summary",
            "Overwrite history."
        )
        .is_err());
    let conn = rusqlite::Connection::open(&db).unwrap();
    assert!(conn
        .execute(
            "UPDATE ansf_entries SET content='tampered' WHERE id=?1",
            [entry]
        )
        .is_err());
    assert!(conn
        .execute("DELETE FROM ansf_entries WHERE id=?1", [entry])
        .is_err());
    assert_eq!(
        store.read_entry("chat", entry).unwrap().unwrap().content,
        "Use a finite model context."
    );
    drop(conn);
    drop(store);
    let _ = std::fs::remove_file(db);
}

#[test]
fn invalid_ids_unbounded_pages_and_entries_without_topic_are_denied() {
    let db = temp_database();
    let store = AnsfStore::open(&db).unwrap();
    assert_eq!(
        store.create_session("", "lu", "Bad"),
        Err(AnsfError::InvalidInput)
    );
    assert_eq!(
        store.start_chapter("missing", "Topic"),
        Err(AnsfError::MissingSession)
    );
    store.create_session("chat", "lu", "Valid").unwrap();
    assert_eq!(
        store.append_entry("chat", "msg", EntryRole::User, "Summary", "Text"),
        Err(AnsfError::NoActiveChapter),
    );
    store.start_chapter("chat", "Valid topic").unwrap();
    assert_eq!(
        store.page_after("chat", 0, 101),
        Err(AnsfError::InvalidInput)
    );
    assert_eq!(store.search("chat", "", 3), Err(AnsfError::InvalidInput));
    assert_eq!(
        store.append_entry(
            "chat",
            "msg",
            EntryRole::User,
            "Summary",
            &"X".repeat(1024 * 1024 + 1),
        ),
        Err(AnsfError::InvalidInput)
    );
    drop(store);
    let _ = std::fs::remove_file(db);
}
