//! Immutable ANSF message edits: revised entries are stored next to the
//! original records in SQLite and linked by permanent provenance edges.
//!
//! An edit NEVER mutates or deletes an earlier entry. The original Jan
//! `message_id` still resolves the first version. `current_message` resolves
//! the most recent linked revision for a model/UI consumer, while
//! `revision_history` exposes the full change trail.
//!
//! No model text, imported file, or linked conversation grants authority.
//! `VerifiedUser` MUST be passed only after the Tauri host has authenticated
//! the human editing this conversation. For Jan imports use `ImportObserved`,
//! which records the time Lu noticed a changed snapshot, NOT the edit time.

use rusqlite::{params, OptionalExtension};

use crate::ansf::{AnsfError, AnsfStore, Entry};

const MAX_SOURCE_BYTES: usize = 1024 * 1024;
const MAX_SUMMARY_BYTES: usize = 2048;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevisionOrigin {
    VerifiedUser,
    ImportObserved,
}

impl RevisionOrigin {
    fn as_str(self) -> &'static str {
        match self {
            Self::VerifiedUser => "verified_user",
            Self::ImportObserved => "import_observed",
        }
    }

    fn note(self) -> &'static str {
        match self {
            Self::VerifiedUser => "Appended: message was edited by user.",
            Self::ImportObserved => {
                "Appended: changed Jan source detected; original edit time unverified."
            }
        }
    }

    fn from_str(value: &str) -> Option<Self> {
        match value {
            "verified_user" => Some(Self::VerifiedUser),
            "import_observed" => Some(Self::ImportObserved),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevisionLink {
    pub original_entry_id: i64,
    pub previous_entry_id: i64,
    pub revised_entry_id: i64,
    pub revision_number: i64,
    pub origin: RevisionOrigin,
    pub note: String,
    /// UTC timestamp of the local edit event or snapshot observation.
    /// Render in the user's timezone, e.g. MMddyyyy HH:mm:ss.
    pub observed_at_utc: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentMessage {
    pub entry: Entry,
    pub revision: Option<RevisionLink>,
}

impl AnsfStore {
    /// Returns the current entry for a Jan/Lu logical message ID.
    /// This does not hide previous versions; it merely resolves the head.
    pub fn current_message(
        &self,
        session_id: &str,
        original_message_id: &str,
    ) -> Result<Option<CurrentMessage>, AnsfError> {
        let Some(original) = self.find_message(session_id, original_message_id)? else {
            return Ok(None);
        };
        let conn = self.conn.lock().map_err(|_| AnsfError::Locked)?;
        let last = conn
            .query_row(
                "SELECT original_entry_id,previous_entry_id,revised_entry_id,
                        revision_number,origin,note,observed_at_utc
                 FROM ansf_entry_edits
                 WHERE session_id=?1 AND original_entry_id=?2
                 ORDER BY revision_number DESC LIMIT 1",
                params![session_id, original.id],
                map_link,
            )
            .optional()?;
        let Some(link) = last else {
            return Ok(Some(CurrentMessage {
                entry: original,
                revision: None,
            }));
        };
        let entry = conn.query_row(
            "SELECT id,session_id,chapter_id,message_id,role,summary,content
             FROM ansf_entries
             WHERE session_id=?1 AND id=?2",
            params![session_id, link.revised_entry_id],
            map_entry,
        )?;
        Ok(Some(CurrentMessage {
            entry,
            revision: Some(link),
        }))
    }

    /// Append a new immutable entry and an edit link in one SQLite
    /// transaction. Repeating the same effective content does nothing.
    /// Returns the new link, or None when current content is unchanged.
    ///
    /// `original_message_id` is a trusted stable ID, not a model-provided
    /// instruction. The host must authenticate edit ownership separately.
    pub fn append_edited_entry(
        &self,
        session_id: &str,
        original_message_id: &str,
        summary: &str,
        new_content: &str,
        origin: RevisionOrigin,
    ) -> Result<Option<RevisionLink>, AnsfError> {
        if session_id.trim().is_empty()
            || session_id.len() > 128
            || original_message_id.trim().is_empty()
            || original_message_id.len() > 128
            || summary.trim().is_empty()
            || summary.len() > MAX_SUMMARY_BYTES
            || new_content.len() > MAX_SOURCE_BYTES
        {
            return Err(AnsfError::InvalidInput);
        }
        let mut conn = self.conn.lock().map_err(|_| AnsfError::Locked)?;
        let tx = conn.transaction()?;
        let original: Option<(i64, i64, String)> = tx
            .query_row(
                "SELECT id,chapter_id,role FROM ansf_entries
                 WHERE session_id=?1 AND message_id=?2",
                params![session_id, original_message_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        let Some((original_id, chapter_id, role)) = original else {
            return Err(AnsfError::InvalidInput);
        };
        let prior: Option<(i64, i64)> = tx
            .query_row(
                "SELECT revised_entry_id,revision_number
                 FROM ansf_entry_edits
                 WHERE session_id=?1 AND original_entry_id=?2
                 ORDER BY revision_number DESC LIMIT 1",
                params![session_id, original_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let (previous_id, sequence) = match prior {
            Some((prior_id, num)) => (prior_id, num + 1),
            None => (original_id, 1_i64),
        };
        let previous_content: String = tx.query_row(
            "SELECT content FROM ansf_entries WHERE session_id=?1 AND id=?2",
            params![session_id, previous_id],
            |r| r.get(0),
        )?;
        if previous_content == new_content {
            return Ok(None);
        }
        // Revision IDs are independent of the original Jan message ID, which
        // stays mapped to the original. No session re-binding is possible.
        let revision_message_id = format!("ansf-revision:{original_id}:{sequence}");
        tx.execute(
            "INSERT INTO ansf_entries
             (session_id,chapter_id,message_id,role,summary,content)
             VALUES(?1,?2,?3,?4,?5,?6)",
            params![
                session_id,
                chapter_id,
                revision_message_id,
                role,
                summary,
                new_content
            ],
        )?;
        let revised_id = tx.last_insert_rowid();
        tx.execute(
            "INSERT INTO ansf_entry_edits
             (session_id,original_entry_id,previous_entry_id,revised_entry_id,
              revision_number,origin,note)
             VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![
                session_id,
                original_id,
                previous_id,
                revised_id,
                sequence,
                origin.as_str(),
                origin.note()
            ],
        )?;
        let link = tx.query_row(
            "SELECT original_entry_id,previous_entry_id,revised_entry_id,
                    revision_number,origin,note,observed_at_utc
             FROM ansf_entry_edits WHERE revised_entry_id=?1",
            [revised_id],
            map_link,
        )?;
        tx.commit()?;
        Ok(Some(link))
    }

    /// Read all edit links, oldest first. The original message and every
    /// revision remain independently addressable through `read_entry`.
    pub fn revision_history(
        &self,
        session_id: &str,
        original_message_id: &str,
    ) -> Result<Vec<RevisionLink>, AnsfError> {
        let Some(original) = self.find_message(session_id, original_message_id)? else {
            return Ok(Vec::new());
        };
        let conn = self.conn.lock().map_err(|_| AnsfError::Locked)?;
        let mut stmt = conn.prepare(
            "SELECT original_entry_id,previous_entry_id,revised_entry_id,
                    revision_number,origin,note,observed_at_utc
             FROM ansf_entry_edits
             WHERE session_id=?1 AND original_entry_id=?2
             ORDER BY revision_number ASC",
        )?;
        let rows = stmt.query_map(params![session_id, original.id], map_link)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
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

fn map_link(r: &rusqlite::Row<'_>) -> rusqlite::Result<RevisionLink> {
    let origin_str: String = r.get(4)?;
    let origin = RevisionOrigin::from_str(&origin_str).ok_or(rusqlite::Error::InvalidQuery)?;
    Ok(RevisionLink {
        original_entry_id: r.get(0)?,
        previous_entry_id: r.get(1)?,
        revised_entry_id: r.get(2)?,
        revision_number: r.get(3)?,
        origin,
        note: r.get(5)?,
        observed_at_utc: r.get(6)?,
    })
}
