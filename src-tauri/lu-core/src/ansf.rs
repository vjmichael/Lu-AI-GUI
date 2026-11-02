//! ANSF (AI-Native Session Format): exact, indexed, disk-backed conversation history.
//!
//! This is an independent Lu Core storage slice, NOT yet connected to Jan's
//! threads, prompt assembler, memory worker, or desktop renderer.
//!
//! Original messages are immutable. Summaries and chapters are navigation aids,
//! never substitutes for source text, and are never authority to invoke tools.
//! Callers must authenticate the conversation owner before reading or writing.
//! A future global library catalog indexes individual ANSF books.

use std::{path::Path, sync::Mutex};

use rusqlite::{params, Connection, OptionalExtension};

const MAX_ID_BYTES: usize = 128;
const MAX_SUMMARY_BYTES: usize = 2048;
const MAX_ENTRY_BYTES: usize = 1024 * 1024;
const MAX_PAGE: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnsfError {
    Database(String),
    InvalidInput,
    MissingSession,
    NoActiveChapter,
    ChapterAlreadyOpen,
    Locked,
}

impl From<rusqlite::Error> for AnsfError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Database(error.to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryRole {
    User,
    Assistant,
    ToolCall,
    ToolResult,
    System,
}

impl EntryRole {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::ToolCall => "tool_call",
            Self::ToolResult => "tool_result",
            Self::System => "system",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub id: i64,
    pub session_id: String,
    pub chapter_id: i64,
    pub message_id: String,
    pub role: String,
    pub summary: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chapter {
    pub id: i64,
    pub title: String,
    pub summary: String,
    pub start_entry_id: Option<i64>,
    pub end_entry_id: Option<i64>,
    pub closed: bool,
}

pub struct AnsfStore {
    pub(crate) conn: Mutex<Connection>,
}

fn valid_id(id: &str) -> bool {
    !id.trim().is_empty() && id.len() <= MAX_ID_BYTES && !id.contains('\0')
}

fn valid_short(text: &str) -> bool {
    !text.trim().is_empty() && text.len() <= MAX_SUMMARY_BYTES && !text.contains('\0')
}

impl AnsfStore {
    pub fn open(path: &Path) -> Result<Self, AnsfError> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "FULL")?;
        Self::initialize(conn)
    }

    #[cfg(test)]
    pub fn in_memory() -> Result<Self, AnsfError> {
        Self::initialize(Connection::open_in_memory()?)
    }

    fn initialize(conn: Connection) -> Result<Self, AnsfError> {
        conn.execute_batch(
            "PRAGMA foreign_keys=ON;
             CREATE TABLE IF NOT EXISTS ansf_sessions (
               id TEXT PRIMARY KEY NOT NULL,
               workspace_id TEXT NOT NULL,
               title TEXT NOT NULL,
               created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
             );
             CREATE TABLE IF NOT EXISTS ansf_chapters (
               id INTEGER PRIMARY KEY AUTOINCREMENT,
               session_id TEXT NOT NULL REFERENCES ansf_sessions(id),
               title TEXT NOT NULL,
               summary TEXT NOT NULL DEFAULT '',
               start_entry_id INTEGER,
               end_entry_id INTEGER,
               closed INTEGER NOT NULL DEFAULT 0 CHECK(closed IN (0,1))
             );
             CREATE UNIQUE INDEX IF NOT EXISTS ansf_one_active_chapter
               ON ansf_chapters(session_id) WHERE closed=0;
             CREATE INDEX IF NOT EXISTS ansf_chapters_by_session
               ON ansf_chapters(session_id,id);
             CREATE TABLE IF NOT EXISTS ansf_entries (
               id INTEGER PRIMARY KEY AUTOINCREMENT,
               session_id TEXT NOT NULL REFERENCES ansf_sessions(id),
               chapter_id INTEGER NOT NULL REFERENCES ansf_chapters(id),
               message_id TEXT NOT NULL,
               role TEXT NOT NULL CHECK(role IN
                 ('user','assistant','tool_call','tool_result','system')),
               summary TEXT NOT NULL,
               content TEXT NOT NULL,
               created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
               UNIQUE(session_id,message_id)
             );
             CREATE INDEX IF NOT EXISTS ansf_entries_by_session
               ON ansf_entries(session_id,id);
             CREATE INDEX IF NOT EXISTS ansf_entries_by_chapter
               ON ansf_entries(chapter_id,id);
             CREATE VIRTUAL TABLE IF NOT EXISTS ansf_entries_fts
               USING fts5(summary,content,content='ansf_entries',content_rowid='id');
             CREATE TRIGGER IF NOT EXISTS ansf_entries_index_insert
             AFTER INSERT ON ansf_entries BEGIN
               INSERT INTO ansf_entries_fts(rowid,summary,content)
               VALUES(new.id,new.summary,new.content);
             END;
             CREATE TABLE IF NOT EXISTS ansf_entry_edits (
               id INTEGER PRIMARY KEY AUTOINCREMENT,
               session_id TEXT NOT NULL REFERENCES ansf_sessions(id),
               original_entry_id INTEGER NOT NULL REFERENCES ansf_entries(id),
               previous_entry_id INTEGER NOT NULL UNIQUE REFERENCES ansf_entries(id),
               revised_entry_id INTEGER NOT NULL UNIQUE REFERENCES ansf_entries(id),
               revision_number INTEGER NOT NULL CHECK(revision_number>0),
               origin TEXT NOT NULL CHECK(origin IN ('verified_user','import_observed')),
               note TEXT NOT NULL,
               observed_at_utc TEXT NOT NULL
                 DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
               UNIQUE(original_entry_id,revision_number)
             );
             CREATE INDEX IF NOT EXISTS ansf_entry_edits_root
               ON ansf_entry_edits(original_entry_id,revision_number);
             CREATE TRIGGER IF NOT EXISTS ansf_edits_no_update
             BEFORE UPDATE ON ansf_entry_edits BEGIN
               SELECT RAISE(ABORT,'ANSF edit links are immutable');
             END;
             CREATE TRIGGER IF NOT EXISTS ansf_edits_no_delete
             BEFORE DELETE ON ansf_entry_edits BEGIN
               SELECT RAISE(ABORT,'ANSF edit links are immutable');
             END;
             CREATE TRIGGER IF NOT EXISTS ansf_entries_no_update
             BEFORE UPDATE ON ansf_entries BEGIN
               SELECT RAISE(ABORT,'ANSF source entries are immutable');
             END;
             CREATE TRIGGER IF NOT EXISTS ansf_entries_no_delete
             BEFORE DELETE ON ansf_entries BEGIN
               SELECT RAISE(ABORT,'ANSF source entries are immutable');
             END;",
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Create a session on first import, or verify that an existing book
    /// still belongs to the same trusted workspace. Never rebind identity or
    /// rewrite the archived title on replay.
    pub fn ensure_session(
        &self,
        session_id: &str,
        workspace_id: &str,
        title: &str,
    ) -> Result<(), AnsfError> {
        if !valid_id(session_id) || !valid_id(workspace_id) || !valid_short(title) {
            return Err(AnsfError::InvalidInput);
        }
        let conn = self.conn.lock().map_err(|_| AnsfError::Locked)?;
        conn.execute(
            "INSERT INTO ansf_sessions(id,workspace_id,title) VALUES(?1,?2,?3)
             ON CONFLICT(id) DO NOTHING",
            params![session_id, workspace_id, title],
        )?;
        let existing: String = conn.query_row(
            "SELECT workspace_id FROM ansf_sessions WHERE id=?1",
            [session_id],
            |row| row.get(0),
        )?;
        if existing != workspace_id {
            return Err(AnsfError::InvalidInput);
        }
        Ok(())
    }

    pub fn create_session(
        &self,
        session_id: &str,
        workspace_id: &str,
        title: &str,
    ) -> Result<(), AnsfError> {
        if !valid_id(session_id) || !valid_id(workspace_id) || !valid_short(title) {
            return Err(AnsfError::InvalidInput);
        }
        let conn = self.conn.lock().map_err(|_| AnsfError::Locked)?;
        conn.execute(
            "INSERT INTO ansf_sessions(id,workspace_id,title) VALUES(?1,?2,?3)",
            params![session_id, workspace_id, title],
        )?;
        Ok(())
    }

    /// Explicit topic boundary: the host's summarizer decides *when* to close
    /// a chapter and what its summary should say. No silent text rewriting.
    pub fn start_chapter(&self, session_id: &str, title: &str) -> Result<i64, AnsfError> {
        if !valid_id(session_id) || !valid_short(title) {
            return Err(AnsfError::InvalidInput);
        }
        let conn = self.conn.lock().map_err(|_| AnsfError::Locked)?;
        let session_exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM ansf_sessions WHERE id=?1)",
            [session_id],
            |row| row.get(0),
        )?;
        if !session_exists {
            return Err(AnsfError::MissingSession);
        }
        let open: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM ansf_chapters WHERE session_id=?1 AND closed=0)",
            [session_id],
            |row| row.get(0),
        )?;
        if open {
            return Err(AnsfError::ChapterAlreadyOpen);
        }
        conn.execute(
            "INSERT INTO ansf_chapters(session_id,title) VALUES(?1,?2)",
            params![session_id, title],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Append an *unaltered* source message plus its indexed summary. `message_id`
    /// must be minted by the trusted host; model-supplied IDs are not authority.
    pub fn append_entry(
        &self,
        session_id: &str,
        message_id: &str,
        role: EntryRole,
        summary: &str,
        content: &str,
    ) -> Result<i64, AnsfError> {
        if !valid_id(session_id)
            || !valid_id(message_id)
            || !valid_short(summary)
            || content.len() > MAX_ENTRY_BYTES
        {
            return Err(AnsfError::InvalidInput);
        }
        let mut conn = self.conn.lock().map_err(|_| AnsfError::Locked)?;
        let tx = conn.transaction()?;
        let chapter_id: Option<i64> = tx
            .query_row(
                "SELECT id FROM ansf_chapters
             WHERE session_id=?1 AND closed=0",
                [session_id],
                |row| row.get(0),
            )
            .optional()?;
        let chapter_id = chapter_id.ok_or(AnsfError::NoActiveChapter)?;
        tx.execute(
            "INSERT INTO ansf_entries
             (session_id,chapter_id,message_id,role,summary,content)
             VALUES(?1,?2,?3,?4,?5,?6)",
            params![
                session_id,
                chapter_id,
                message_id,
                role.as_str(),
                summary,
                content
            ],
        )?;
        let entry_id = tx.last_insert_rowid();
        tx.execute(
            "UPDATE ansf_chapters
             SET start_entry_id=COALESCE(start_entry_id,?1),end_entry_id=?1
             WHERE id=?2",
            params![entry_id, chapter_id],
        )?;
        tx.commit()?;
        Ok(entry_id)
    }

    pub fn close_chapter(&self, session_id: &str, summary: &str) -> Result<(), AnsfError> {
        if !valid_id(session_id) || !valid_short(summary) {
            return Err(AnsfError::InvalidInput);
        }
        let conn = self.conn.lock().map_err(|_| AnsfError::Locked)?;
        let changed = conn.execute(
            "UPDATE ansf_chapters SET closed=1,summary=?2
             WHERE session_id=?1 AND closed=0",
            params![session_id, summary],
        )?;
        if changed != 1 {
            return Err(AnsfError::NoActiveChapter);
        }
        Ok(())
    }

    pub fn chapters(&self, session_id: &str) -> Result<Vec<Chapter>, AnsfError> {
        if !valid_id(session_id) {
            return Err(AnsfError::InvalidInput);
        }
        let conn = self.conn.lock().map_err(|_| AnsfError::Locked)?;
        let mut stmt = conn.prepare(
            "SELECT id,title,summary,start_entry_id,end_entry_id,closed
             FROM ansf_chapters WHERE session_id=?1 ORDER BY id",
        )?;
        let rows = stmt.query_map([session_id], |r| {
            Ok(Chapter {
                id: r.get(0)?,
                title: r.get(1)?,
                summary: r.get(2)?,
                start_entry_id: r.get(3)?,
                end_entry_id: r.get(4)?,
                closed: r.get::<_, i64>(5)? == 1,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn map_entry(r: &rusqlite::Row<'_>) -> rusqlite::Result<Entry> {
        Ok(Entry {
            id: r.get(0)?,
            session_id: r.get(1)?,
            chapter_id: r.get(2)?,
            message_id: r.get(3)?,
            role: r.get(4)?,
            summary: r.get(5)?,
            content: r.get(6)?,
        })
    }

    /// Look up a host-issued stable message identifier. An importer may replay
    /// the same immutable source row after interruption without duplication.
    /// Changes to an existing ID must be tracked as new revisions, never
    /// silently used to overwrite this original entry.
    pub fn find_message(
        &self,
        session_id: &str,
        message_id: &str,
    ) -> Result<Option<Entry>, AnsfError> {
        if !valid_id(session_id) || !valid_id(message_id) {
            return Err(AnsfError::InvalidInput);
        }
        let conn = self.conn.lock().map_err(|_| AnsfError::Locked)?;
        conn.query_row(
            "SELECT id,session_id,chapter_id,message_id,role,summary,content
             FROM ansf_entries WHERE session_id=?1 AND message_id=?2",
            params![session_id, message_id],
            Self::map_entry,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn read_entry(&self, session_id: &str, entry_id: i64) -> Result<Option<Entry>, AnsfError> {
        if !valid_id(session_id) || entry_id < 1 {
            return Err(AnsfError::InvalidInput);
        }
        let conn = self.conn.lock().map_err(|_| AnsfError::Locked)?;
        conn.query_row(
            "SELECT id,session_id,chapter_id,message_id,role,summary,content
             FROM ansf_entries WHERE session_id=?1 AND id=?2",
            params![session_id, entry_id],
            Self::map_entry,
        )
        .optional()
        .map_err(Into::into)
    }

    /// Cursor pagination for virtual scrolling: older blocks remain on disk.
    /// Entry IDs are stable, not array offsets.
    pub fn page_after(
        &self,
        session_id: &str,
        after_id: i64,
        limit: usize,
    ) -> Result<Vec<Entry>, AnsfError> {
        if !valid_id(session_id) || after_id < 0 || limit == 0 || limit > MAX_PAGE {
            return Err(AnsfError::InvalidInput);
        }
        let conn = self.conn.lock().map_err(|_| AnsfError::Locked)?;
        let mut stmt = conn.prepare(
            "SELECT id,session_id,chapter_id,message_id,role,summary,content
             FROM ansf_entries WHERE session_id=?1 AND id>?2
             ORDER BY id LIMIT ?3",
        )?;
        let rows = stmt.query_map(params![session_id, after_id, limit as i64], Self::map_entry)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Local book search. A separate, future Library index will locate books;
    /// this search narrows to exact archived entries within this SQLite file.
    pub fn search(
        &self,
        session_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<Entry>, AnsfError> {
        if !valid_id(session_id)
            || query.trim().is_empty()
            || query.len() > 256
            || limit == 0
            || limit > MAX_PAGE
        {
            return Err(AnsfError::InvalidInput);
        }
        let conn = self.conn.lock().map_err(|_| AnsfError::Locked)?;
        let mut stmt = conn.prepare(
            "SELECT e.id,e.session_id,e.chapter_id,e.message_id,e.role,e.summary,e.content
             FROM ansf_entries_fts f JOIN ansf_entries e ON e.id=f.rowid
             WHERE ansf_entries_fts MATCH ?1 AND e.session_id=?2
             ORDER BY bm25(ansf_entries_fts),e.id LIMIT ?3",
        )?;
        let rows = stmt.query_map(params![query, session_id, limit as i64], Self::map_entry)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
}
