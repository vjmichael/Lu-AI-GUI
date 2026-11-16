//! Host-owned read-only C06 bridge to Lu ANSF's existing SQLite FTS5.
//!
//! No Python SQLite store, no new canonical database, no model-origin grants.
//! Authorization is an explicit host callback; the supplied workspace is
//! independently checked against canonical ansf_sessions inside the query.
use crate::ansf::{AnsfError, AnsfStore};
use rusqlite::params;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizedAnsfEvidence {
    pub entry_id: i64,
    pub session_id: String,
    pub workspace_id: String,
    pub message_id: String,
    pub role: String,
    pub summary: String,
    pub content: String,
    pub original_entry_id: i64,
    pub revision_number: i64,
    pub citation_handle: String,
}

/// An authorization check supplied by the privileged Lu host. The model
/// cannot implement this trait in order to mint its own permission.
pub trait AnsfReadAuthorizer {
    fn may_read(&self, workspace_id: &str, session_id: &str) -> bool;
}

fn literal_fts_query(input: &str) -> Result<String, AnsfError> {
    if input.is_empty() || input.len() > 256 || input.contains('\0') {
        return Err(AnsfError::InvalidInput);
    }
    let terms: Vec<String> = input
        .split_whitespace()
        .filter(|part| !part.is_empty())
        .map(|part| format!("\"{}\"", part.replace('"', "\"\"")))
        .collect();
    if terms.is_empty() {
        return Err(AnsfError::InvalidInput);
    }
    Ok(terms.join(" AND "))
}

impl AnsfStore {
    /// Scoped FTS5/BM25 over only effective current ANSF source revisions.
    /// Original revisions remain immutable but are never returned as live
    /// evidence after a revised message becomes current.
    pub fn search_cognitum_evidence(
        &self,
        workspace_id: &str,
        session_id: &str,
        literal_query: &str,
        limit: usize,
        authorizer: &impl AnsfReadAuthorizer,
    ) -> Result<Vec<AuthorizedAnsfEvidence>, AnsfError> {
        if workspace_id.is_empty()
            || workspace_id.len() > 128
            || session_id.is_empty()
            || session_id.len() > 128
            || limit == 0
            || limit > 100
            || !authorizer.may_read(workspace_id, session_id)
        {
            return Err(AnsfError::InvalidInput);
        }
        let search = literal_fts_query(literal_query)?;
        let conn = self.conn.lock().map_err(|_| AnsfError::Locked)?;
        // Workspace check is performed in the SQLite predicate, not merely
        // against any authority claim supplied by a caller or model.
        let mut stmt = conn.prepare(
            "SELECT e.id,e.session_id,s.workspace_id,e.message_id,e.role,
                    e.summary,e.content,
                    COALESCE(rev.original_entry_id,e.id) AS root_id,
                    COALESCE(rev.revision_number,0) AS rev_num
             FROM ansf_entries_fts
             JOIN ansf_entries e ON e.id=ansf_entries_fts.rowid
             JOIN ansf_sessions s ON s.id=e.session_id
             LEFT JOIN ansf_entry_edits rev ON rev.revised_entry_id=e.id
             WHERE ansf_entries_fts MATCH ?1
               AND e.session_id=?2 AND s.workspace_id=?3
               AND NOT EXISTS (
                 SELECT 1 FROM ansf_entry_edits next_revision
                 WHERE next_revision.session_id=e.session_id
                   AND next_revision.previous_entry_id=e.id
               )
             ORDER BY bm25(ansf_entries_fts),e.id LIMIT ?4",
        )?;
        let evidence = stmt.query_map(
            params![search, session_id, workspace_id, limit as i64],
            |row| {
                let entry_id: i64 = row.get(0)?;
                let session_id: String = row.get(1)?;
                let original_entry_id: i64 = row.get(7)?;
                let revision_number: i64 = row.get(8)?;
                Ok(AuthorizedAnsfEvidence {
                    entry_id,
                    session_id: session_id.clone(),
                    workspace_id: row.get(2)?,
                    message_id: row.get(3)?,
                    role: row.get(4)?,
                    summary: row.get(5)?,
                    content: row.get(6)?,
                    original_entry_id,
                    revision_number,
                    // Opaque source-local citation, never a filesystem path.
                    citation_handle: format!(
                        "ansf:{}:{}:{}", session_id, original_entry_id, entry_id
                    ),
                })
            },
        )?;
        evidence.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
}
