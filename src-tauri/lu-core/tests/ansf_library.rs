use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use lu_core::ansf_library::{AnsfLibrary, BookIndex, ChapterIndex, LibraryError};

static TEST_ID: AtomicU64 = AtomicU64::new(0);

fn temp_library() -> PathBuf {
    std::env::temp_dir().join(format!(
        "lu-library-{}-{}-{}.sqlite",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        TEST_ID.fetch_add(1, Ordering::Relaxed)
    ))
}

fn book(id: &str, workspace: &str, title: &str, summary: &str) -> BookIndex {
    BookIndex {
        session_id: id.into(),
        workspace_id: workspace.into(),
        relative_path: format!("workspaces/{workspace}/sessions/{id}.sqlite"),
        title: title.into(),
        summary: summary.into(),
    }
}

fn chapter(session_id: &str, chapter_id: i64, title: &str, summary: &str) -> ChapterIndex {
    ChapterIndex {
        session_id: session_id.into(),
        chapter_id,
        title: title.into(),
        summary: summary.into(),
        start_entry_id: Some(1),
        end_entry_id: Some(12),
    }
}

#[test]
fn library_search_locates_book_and_chapter_across_workspaces_after_restart() {
    let path = temp_library();
    {
        let library = AnsfLibrary::open(&path).unwrap();
        library
            .index_book(&book(
                "session-a",
                "lu",
                "Lu Memory",
                "ANSF indexing and search",
            ))
            .unwrap();
        library
            .index_book(&book(
                "session-b",
                "zog-zoo",
                "Zog Zoo",
                "HDC KAN experimentation",
            ))
            .unwrap();
        library
            .index_chapter(&chapter(
                "session-a",
                3,
                "Conversation authority",
                "Prevent privilege escalation",
            ))
            .unwrap();
        library
            .index_chapter(&chapter("session-b", 8, "Embeddings", "HDC signed vectors"))
            .unwrap();
    }
    {
        let reopened = AnsfLibrary::open(&path).unwrap();
        assert_eq!(
            reopened.search_books("HDC", 10).unwrap()[0].session_id,
            "session-b"
        );
        assert_eq!(
            reopened.search_books("ANSF indexing", 10).unwrap()[0].session_id,
            "session-a"
        );
        assert_eq!(
            reopened.search_chapters("escalation", 10).unwrap()[0].chapter_id,
            3
        );
        let chapters = reopened.toc("session-b").unwrap();
        assert_eq!(chapters.len(), 1);
        assert_eq!(chapters[0].title, "Embeddings");
        assert_eq!(
            reopened
                .get_book("session-a")
                .unwrap()
                .unwrap()
                .workspace_id,
            "lu"
        );
    }
    let _ = std::fs::remove_file(path);
}

#[test]
fn reindex_updates_fts_without_duplicate_book_or_chapter() {
    let path = temp_library();
    let library = AnsfLibrary::open(&path).unwrap();
    library
        .index_book(&book("chat", "lu", "Original title", "oldkeyword"))
        .unwrap();
    library
        .index_chapter(&chapter("chat", 1, "Original chapter", "oldchapter"))
        .unwrap();
    library
        .index_book(&book("chat", "lu", "Renamed title", "newkeyword"))
        .unwrap();
    library
        .index_chapter(&chapter("chat", 1, "Renamed chapter", "newchapter"))
        .unwrap();
    assert!(library.search_books("oldkeyword", 10).unwrap().is_empty());
    assert_eq!(library.search_books("newkeyword", 10).unwrap().len(), 1);
    assert!(library
        .search_chapters("oldchapter", 10)
        .unwrap()
        .is_empty());
    assert_eq!(library.search_chapters("newchapter", 10).unwrap().len(), 1);
    assert_eq!(library.toc("chat").unwrap().len(), 1);
    drop(library);
    let _ = std::fs::remove_file(path);
}

#[test]
fn source_book_identity_cannot_be_silently_swapped() {
    let path = temp_library();
    let library = AnsfLibrary::open(&path).unwrap();
    library.index_book(&book("chat", "lu", "Book", "")).unwrap();
    let mut hijack = book("chat", "attacker", "Injected", "");
    assert_eq!(
        library.index_book(&hijack),
        Err(LibraryError::BookIdentityConflict)
    );
    hijack.workspace_id = "lu".into();
    hijack.relative_path = "workspaces/lu/sessions/another.sqlite".into();
    assert_eq!(
        library.index_book(&hijack),
        Err(LibraryError::BookIdentityConflict)
    );
    let same_path_other_id = BookIndex {
        session_id: "another-chat".into(),
        ..book("chat", "lu", "Other", "")
    };
    // Prevent a different session from claiming the same relative path.
    assert!(library.index_book(&same_path_other_id).is_err());
    assert_eq!(library.get_book("chat").unwrap().unwrap().title, "Book");
    drop(library);
    let _ = std::fs::remove_file(path);
}

#[test]
fn unsafe_paths_and_invalid_chapter_ranges_are_rejected() {
    let path = temp_library();
    let library = AnsfLibrary::open(&path).unwrap();
    for bad in [
        "../session.sqlite",
        "/tmp/session.sqlite",
        "C:\\Users\\X\\session.sqlite",
        "workspaces/../session.sqlite",
        "workspaces//session.sqlite",
        "workspaces/sub/evil.db",
        "workspaces/.//session.sqlite",
    ] {
        let mut b = book("bad", "lu", "Unsafe", "");
        b.relative_path = bad.into();
        assert_eq!(
            library.index_book(&b),
            Err(LibraryError::InvalidInput),
            "{bad}"
        );
    }
    assert_eq!(
        library.index_chapter(&chapter("missing", 1, "Topic", "Summary")),
        Err(LibraryError::UnknownSession)
    );
    library.index_book(&book("ok", "lu", "Valid", "")).unwrap();
    let mut invalid = chapter("ok", 1, "Chapter", "Summary");
    invalid.start_entry_id = Some(8);
    invalid.end_entry_id = Some(5);
    assert_eq!(
        library.index_chapter(&invalid),
        Err(LibraryError::InvalidInput)
    );
    invalid.start_entry_id = None;
    invalid.end_entry_id = Some(5);
    assert_eq!(
        library.index_chapter(&invalid),
        Err(LibraryError::InvalidInput)
    );
    assert_eq!(
        library.search_books("", 10),
        Err(LibraryError::InvalidInput)
    );
    assert_eq!(
        library.search_chapters("index", 101),
        Err(LibraryError::InvalidInput)
    );
    drop(library);
    let _ = std::fs::remove_file(path);
}

#[test]
fn unindex_clears_both_search_indexes_without_touching_book_bytes() {
    let root = temp_library();
    let book_file = root.with_extension("book");
    std::fs::write(&book_file, b"original ANSF book remains").unwrap();
    let library = AnsfLibrary::open(&root).unwrap();
    library
        .index_book(&book("chat", "lu", "Private notes", "specialtopic"))
        .unwrap();
    library
        .index_chapter(&chapter("chat", 1, "Memory", "anotherkeyword"))
        .unwrap();
    assert!(library.unindex_book("chat").unwrap());
    assert!(!library.unindex_book("chat").unwrap());
    assert!(library.search_books("specialtopic", 10).unwrap().is_empty());
    assert!(library
        .search_chapters("anotherkeyword", 10)
        .unwrap()
        .is_empty());
    assert!(library.toc("chat").unwrap().is_empty());
    assert_eq!(
        std::fs::read(&book_file).unwrap(),
        b"original ANSF book remains"
    );
    drop(library);
    let _ = std::fs::remove_file(root);
    let _ = std::fs::remove_file(book_file);
}

#[test]
fn search_treats_model_supplied_fts_operators_as_literal_text() {
    let path = temp_library();
    let library = AnsfLibrary::open(&path).unwrap();
    library
        .index_book(&book("chat", "lu", "Safe data", "example text"))
        .unwrap();
    for query in [
        "\" OR *",
        "example OR",
        "'; DROP TABLE library_books;",
        "content:secret",
    ] {
        let result = library.search_books(query, 5);
        assert!(
            result.is_ok(),
            "query should be parsed as safe literal: {query}"
        );
    }
    assert!(library.get_book("chat").unwrap().is_some());
    drop(library);
    let _ = std::fs::remove_file(path);
}
