//! Crash-recoverable ANSF book/catalog coordination.
//!
//! Only a trusted desktop host may select the input stream, authenticate the
//! conversation and mint the book path. This module does not open Jan files,
//! grant model authority or decide whether an edited snapshot is newer.
use std::io::BufRead;

use crate::{
    ansf::{AnsfError, AnsfStore},
    ansf_import::{import_jan_jsonl_with_policy, ImportError, ImportStats, JanImportPolicy},
    ansf_library::{AnsfLibrary, BookIndex, ChapterIndex, LibraryError},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncError {
    Import(ImportError),
    Store(AnsfError),
    Library(LibraryError),
    SessionMismatch,
}

impl From<ImportError> for SyncError {
    fn from(error: ImportError) -> Self {
        Self::Import(error)
    }
}
impl From<AnsfError> for SyncError {
    fn from(error: AnsfError) -> Self {
        Self::Store(error)
    }
}
impl From<LibraryError> for SyncError {
    fn from(error: LibraryError) -> Self {
        Self::Library(error)
    }
}

/// Rebuild searchable metadata from an already-persisted ANSF book.
///
/// This is intentionally idempotent. The ANSF book and global library have
/// independent SQLite transactions; an interruption between them is repaired
/// by rerunning this function. No source messages are copied into the catalog.
pub fn reconcile_book_index(
    store: &AnsfStore,
    library: &AnsfLibrary,
    book: &BookIndex,
) -> Result<(), SyncError> {
    // Do not allow an index record to refer to a non-existent ANSF session.
    // A host still MUST independently authenticate ownership and confine paths.
    let chapters = store.chapters(&book.session_id)?;
    if chapters.is_empty() {
        return Err(SyncError::SessionMismatch);
    }
    library.index_book(book)?;
    for chapter in chapters {
        library.index_chapter(&ChapterIndex {
            session_id: book.session_id.clone(),
            chapter_id: chapter.id,
            title: chapter.title,
            summary: chapter.summary,
            start_entry_id: chapter.start_entry_id,
            end_entry_id: chapter.end_entry_id,
        })?;
    }
    Ok(())
}

/// Import a host-verified Jan snapshot then reconcile global search metadata.
///
/// The importer durably commits individual entries before catalog indexing.
/// If importing fails midway, valid preceding entries remain available;
/// a subsequent replay deduplicates them and can call reconcile_book_index.
/// The default policy must remain RejectChangedSource. Only the trusted host,
/// after proving newer snapshot chronology, may opt into append-only revisions.
pub fn sync_verified_snapshot<R: BufRead>(
    store: &AnsfStore,
    library: &AnsfLibrary,
    book: &BookIndex,
    snapshot: R,
    policy: JanImportPolicy,
) -> Result<ImportStats, SyncError> {
    let stats = import_jan_jsonl_with_policy(store, &book.session_id, snapshot, policy)?;
    reconcile_book_index(store, library, book)?;
    Ok(stats)
}
