//! Global ANSF library catalog: a searchable index of conversation books.
//!
//! The catalog stores *metadata and references*, not copies of chat messages.
//! It never opens, executes or follows arbitrary book paths. A trusted desktop
//! host must authenticate the caller, mint book paths, confine file access to
//! the library root, and check ownership before resolving search hits.
//!
//! Sessions are "books", chapters are their TOC, and source messages remain
//! only in the original ANSF book. FTS is an accelerator, never an authority.
//! Index writes can be replayed from a book after a crash; they are not an
//! atomic transaction spanning the catalog and the original ANSF database.

use std::{path::Path, sync::Mutex};

use rusqlite::{params, Connection, OptionalExtension};

const MAX_ID_BYTES: usize = 128;
const MAX_TEXT_BYTES: usize = 2048;
const MAX_PATH_BYTES: usize = 512;
const MAX_RESULTS: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibraryError {
    Database(String),
    InvalidInput,
    UnknownSession,
    BookIdentityConflict,
    Locked,
}

impl From<rusqlite::Error> for LibraryError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Database(error.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookIndex {
    pub session_id: String,
    pub workspace_id: String,
    /// Opaque, host-issued path relative to the library's root.
    /// Never use a search result as an unvalidated filesystem path.
    pub relative_path: String,
    pub title: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChapterIndex {
    pub session_id: String,
    pub chapter_id: i64,
    pub title: String,
    pub summary: String,
    pub start_entry_id: Option<i64>,
    pub end_entry_id: Option<i64>,
}

pub struct AnsfLibrary {
    conn: Mutex<Connection>,
}

fn valid_id(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= MAX_ID_BYTES && !value.contains('\0')
}

fn valid_title(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= MAX_TEXT_BYTES && !value.contains('\0')
}

fn valid_summary(value: &str) -> bool {
    value.len() <= MAX_TEXT_BYTES && !value.contains('\0')
}

fn valid_relative_path(value: &str) -> bool {
    // Metadata validation only: the privileged host must still enforce
    // filesystem confinement when opening the referenced book.
    value.len() <= MAX_PATH_BYTES
        && value.ends_with(".sqlite")
        && value.split('/').all(|segment| {
            !segment.is_empty()
                && segment != "."
                && segment != ".."
                && !segment.ends_with('.')
                && !segment.ends_with(' ')
                && segment
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'-' | b'_'))
        })
        && value.contains('/')
}

fn valid_entry_span(start: Option<i64>, end: Option<i64>) -> bool {
    match (start, end) {
        (None, None) => true,
        (Some(first), Some(last)) => first > 0 && last >= first,
        (Some(first), None) => first > 0,
        (None, Some(_)) => false,
    }
}

/// Treat the user query as literal words. Do not allow externally supplied
/// FTS5 operators, quote syntax, columns or wildcards to change its meaning.
/// Bound SQL parameters independently prevent SQL injection.
fn literal_fts_query(value: &str) -> Result<String, LibraryError> {
    if value.trim().is_empty() || value.len() > 256 || value.contains('\0') {
        return Err(LibraryError::InvalidInput);
    }
    Ok(value
        .split_whitespace()
        .map(|word| format!("\"{}\"", word.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" AND "))
}

impl AnsfLibrary {
    pub fn open(path: &Path) -> Result<Self, LibraryError> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "FULL")?;
        Self::initialize(conn)
    }

    fn initialize(conn: Connection) -> Result<Self, LibraryError> {
        conn.execute_batch(
            "PRAGMA foreign_keys=ON;
             CREATE TABLE IF NOT EXISTS library_books(
               session_id TEXT PRIMARY KEY NOT NULL,
               workspace_id TEXT NOT NULL,
               relative_path TEXT NOT NULL UNIQUE,
               title TEXT NOT NULL,
               summary TEXT NOT NULL,
               indexed_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
             );
             CREATE INDEX IF NOT EXISTS library_books_workspace
               ON library_books(workspace_id,indexed_at);
             CREATE VIRTUAL TABLE IF NOT EXISTS library_books_fts
               USING fts5(title,summary,content='library_books',content_rowid='rowid');
             CREATE TRIGGER IF NOT EXISTS library_books_fts_insert
             AFTER INSERT ON library_books BEGIN
               INSERT INTO library_books_fts(rowid,title,summary)
               VALUES(new.rowid,new.title,new.summary);
             END;
             CREATE TRIGGER IF NOT EXISTS library_books_fts_update
             AFTER UPDATE ON library_books BEGIN
               INSERT INTO library_books_fts(library_books_fts,rowid,title,summary)
               VALUES('delete',old.rowid,old.title,old.summary);
               INSERT INTO library_books_fts(rowid,title,summary)
               VALUES(new.rowid,new.title,new.summary);
             END;
             CREATE TRIGGER IF NOT EXISTS library_books_fts_delete
             AFTER DELETE ON library_books BEGIN
               INSERT INTO library_books_fts(library_books_fts,rowid,title,summary)
               VALUES('delete',old.rowid,old.title,old.summary);
             END;
             CREATE TABLE IF NOT EXISTS library_chapters(
               session_id TEXT NOT NULL REFERENCES library_books(session_id) ON DELETE CASCADE,
               chapter_id INTEGER NOT NULL CHECK(chapter_id > 0),
               title TEXT NOT NULL,
               summary TEXT NOT NULL,
               start_entry_id INTEGER,
               end_entry_id INTEGER,
               PRIMARY KEY(session_id,chapter_id)
             );
             CREATE INDEX IF NOT EXISTS library_chapters_session
               ON library_chapters(session_id,chapter_id);
             CREATE VIRTUAL TABLE IF NOT EXISTS library_chapters_fts
               USING fts5(title,summary,content='library_chapters',content_rowid='rowid');
             CREATE TRIGGER IF NOT EXISTS library_chapters_fts_insert
             AFTER INSERT ON library_chapters BEGIN
               INSERT INTO library_chapters_fts(rowid,title,summary)
               VALUES(new.rowid,new.title,new.summary);
             END;
             CREATE TRIGGER IF NOT EXISTS library_chapters_fts_update
             AFTER UPDATE ON library_chapters BEGIN
               INSERT INTO library_chapters_fts(library_chapters_fts,rowid,title,summary)
               VALUES('delete',old.rowid,old.title,old.summary);
               INSERT INTO library_chapters_fts(rowid,title,summary)
               VALUES(new.rowid,new.title,new.summary);
             END;
             CREATE TRIGGER IF NOT EXISTS library_chapters_fts_delete
             AFTER DELETE ON library_chapters BEGIN
               INSERT INTO library_chapters_fts(library_chapters_fts,rowid,title,summary)
               VALUES('delete',old.rowid,old.title,old.summary);
             END;",
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Idempotently index a known book. A session ID is forever bound to its
    /// workspace and book path; reindexing can update title/summary only.
    pub fn index_book(&self, book: &BookIndex) -> Result<(), LibraryError> {
        if !valid_id(&book.session_id)
            || !valid_id(&book.workspace_id)
            || !valid_relative_path(&book.relative_path)
            || !valid_title(&book.title)
            || !valid_summary(&book.summary)
        {
            return Err(LibraryError::InvalidInput);
        }
        let mut conn = self.conn.lock().map_err(|_| LibraryError::Locked)?;
        let tx = conn.transaction()?;
        let existing: Option<(String, String)> = tx
            .query_row(
                "SELECT workspace_id,relative_path FROM library_books
                 WHERE session_id=?1",
                [&book.session_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if let Some((workspace, path)) = existing {
            if workspace != book.workspace_id || path != book.relative_path {
                return Err(LibraryError::BookIdentityConflict);
            }
            tx.execute(
                "UPDATE library_books
                 SET title=?2,summary=?3,indexed_at=CURRENT_TIMESTAMP
                 WHERE session_id=?1",
                params![book.session_id, book.title, book.summary],
            )?;
        } else {
            tx.execute(
                "INSERT INTO library_books
                 (session_id,workspace_id,relative_path,title,summary)
                 VALUES(?1,?2,?3,?4,?5)",
                params![
                    book.session_id,
                    book.workspace_id,
                    book.relative_path,
                    book.title,
                    book.summary
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Mirrors a chapter TOC record. Its entries are *not* copied.
    /// The original ANSF book is the sole source of message contents.
    pub fn index_chapter(&self, chapter: &ChapterIndex) -> Result<(), LibraryError> {
        if !valid_id(&chapter.session_id)
            || chapter.chapter_id <= 0
            || !valid_title(&chapter.title)
            || !valid_summary(&chapter.summary)
            || !valid_entry_span(chapter.start_entry_id, chapter.end_entry_id)
        {
            return Err(LibraryError::InvalidInput);
        }
        let conn = self.conn.lock().map_err(|_| LibraryError::Locked)?;
        let known: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM library_books WHERE session_id=?1)",
            [&chapter.session_id],
            |r| r.get(0),
        )?;
        if !known {
            return Err(LibraryError::UnknownSession);
        }
        conn.execute(
            "INSERT INTO library_chapters
             (session_id,chapter_id,title,summary,start_entry_id,end_entry_id)
             VALUES(?1,?2,?3,?4,?5,?6)
             ON CONFLICT(session_id,chapter_id) DO UPDATE SET
               title=excluded.title,summary=excluded.summary,
               start_entry_id=excluded.start_entry_id,
               end_entry_id=excluded.end_entry_id",
            params![
                chapter.session_id,
                chapter.chapter_id,
                chapter.title,
                chapter.summary,
                chapter.start_entry_id,
                chapter.end_entry_id
            ],
        )?;
        Ok(())
    }

    pub fn get_book(&self, session_id: &str) -> Result<Option<BookIndex>, LibraryError> {
        if !valid_id(session_id) {
            return Err(LibraryError::InvalidInput);
        }
        let conn = self.conn.lock().map_err(|_| LibraryError::Locked)?;
        conn.query_row(
            "SELECT session_id,workspace_id,relative_path,title,summary
             FROM library_books WHERE session_id=?1",
            [session_id],
            |r| {
                Ok(BookIndex {
                    session_id: r.get(0)?,
                    workspace_id: r.get(1)?,
                    relative_path: r.get(2)?,
                    title: r.get(3)?,
                    summary: r.get(4)?,
                })
            },
        )
        .optional()
        .map_err(Into::into)
    }

    /// The first step of hierarchical recall: find likely books globally.
    /// Caller authorization must be checked before exposing any metadata.
    pub fn search_books(
        &self,
        query: &str,
        max_results: usize,
    ) -> Result<Vec<BookIndex>, LibraryError> {
        let expression = literal_fts_query(query)?;
        if max_results == 0 || max_results > MAX_RESULTS {
            return Err(LibraryError::InvalidInput);
        }
        let conn = self.conn.lock().map_err(|_| LibraryError::Locked)?;
        let mut stmt = conn.prepare(
            "SELECT b.session_id,b.workspace_id,b.relative_path,b.title,b.summary
             FROM library_books_fts f
             JOIN library_books b ON b.rowid=f.rowid
             WHERE library_books_fts MATCH ?1
             ORDER BY bm25(library_books_fts),b.session_id
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![expression, max_results as i64], |r| {
            Ok(BookIndex {
                session_id: r.get(0)?,
                workspace_id: r.get(1)?,
                relative_path: r.get(2)?,
                title: r.get(3)?,
                summary: r.get(4)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// The second step of hierarchical recall: search TOC summaries.
    /// A cross-book chapter result does not grant access to its content.
    pub fn search_chapters(
        &self,
        query: &str,
        max_results: usize,
    ) -> Result<Vec<ChapterIndex>, LibraryError> {
        let expression = literal_fts_query(query)?;
        if max_results == 0 || max_results > MAX_RESULTS {
            return Err(LibraryError::InvalidInput);
        }
        let conn = self.conn.lock().map_err(|_| LibraryError::Locked)?;
        let mut stmt = conn.prepare(
            "SELECT c.session_id,c.chapter_id,c.title,c.summary,
                    c.start_entry_id,c.end_entry_id
             FROM library_chapters_fts f
             JOIN library_chapters c ON c.rowid=f.rowid
             WHERE library_chapters_fts MATCH ?1
             ORDER BY bm25(library_chapters_fts),c.session_id,c.chapter_id
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![expression, max_results as i64], |r| {
            Ok(ChapterIndex {
                session_id: r.get(0)?,
                chapter_id: r.get(1)?,
                title: r.get(2)?,
                summary: r.get(3)?,
                start_entry_id: r.get(4)?,
                end_entry_id: r.get(5)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Return a book's table of contents without opening its message archive.
    pub fn toc(&self, session_id: &str) -> Result<Vec<ChapterIndex>, LibraryError> {
        if !valid_id(session_id) {
            return Err(LibraryError::InvalidInput);
        }
        let conn = self.conn.lock().map_err(|_| LibraryError::Locked)?;
        let mut stmt = conn.prepare(
            "SELECT session_id,chapter_id,title,summary,start_entry_id,end_entry_id
             FROM library_chapters WHERE session_id=?1 ORDER BY chapter_id",
        )?;
        let rows = stmt.query_map([session_id], |r| {
            Ok(ChapterIndex {
                session_id: r.get(0)?,
                chapter_id: r.get(1)?,
                title: r.get(2)?,
                summary: r.get(3)?,
                start_entry_id: r.get(4)?,
                end_entry_id: r.get(5)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Removing an index must never remove the original conversation book.
    /// Deletion and retention of original source messages belong to the host.
    pub fn unindex_book(&self, session_id: &str) -> Result<bool, LibraryError> {
        if !valid_id(session_id) {
            return Err(LibraryError::InvalidInput);
        }
        let conn = self.conn.lock().map_err(|_| LibraryError::Locked)?;
        let deleted = conn.execute(
            "DELETE FROM library_books WHERE session_id=?1",
            [session_id],
        )?;
        Ok(deleted == 1)
    }
}
