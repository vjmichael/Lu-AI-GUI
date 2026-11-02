//! Append-only task continuity checkpoints anchored to ANSF source entries.
//!
//! Checkpoints are derived observations, not authority and not edits to chat.
//! Hosts must authenticate session/project access before calling this API.
use rusqlite::{params, Connection, OptionalExtension};
use std::{path::Path, sync::Mutex};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContinuityError {
    Database(String),
    InvalidInput,
    IdentityConflict,
    Locked,
}
impl From<rusqlite::Error> for ContinuityError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Database(error.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskCheckpoint {
    pub checkpoint_id: String,
    pub project_id: String,
    pub task_id: String,
    pub conversation_id: String,
    pub source_entry_ids: Vec<i64>,
    pub completed_work: String,
    pub next_action: String,
    pub blockers: Vec<String>,
}

type StoredCheckpoint = (
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
);

pub struct ContinuityStore {
    conn: Mutex<Connection>,
}

fn valid_id(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
}
fn valid_text(value: &str) -> bool {
    value.len() <= 4096 && !value.contains('\0')
}
fn validate(c: &TaskCheckpoint) -> bool {
    valid_id(&c.checkpoint_id)
        && valid_id(&c.project_id)
        && valid_id(&c.task_id)
        && valid_id(&c.conversation_id)
        && valid_text(&c.completed_work)
        && valid_text(&c.next_action)
        && c.source_entry_ids.len() <= 64
        && c.source_entry_ids.iter().all(|id| *id > 0)
        && c.blockers.len() <= 32
        && c.blockers.iter().all(|s| valid_text(s))
}

impl ContinuityStore {
    pub fn open(path: &Path) -> Result<Self, ContinuityError> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "FULL")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS continuity_checkpoints (
                sequence INTEGER PRIMARY KEY AUTOINCREMENT,
                checkpoint_id TEXT NOT NULL UNIQUE,
                project_id TEXT NOT NULL,
                task_id TEXT NOT NULL,
                conversation_id TEXT NOT NULL,
                source_entry_ids TEXT NOT NULL,
                completed_work TEXT NOT NULL,
                next_action TEXT NOT NULL,
                blockers TEXT NOT NULL,
                created_at_utc TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            CREATE INDEX IF NOT EXISTS continuity_by_task
                ON continuity_checkpoints(project_id,task_id,sequence);
            CREATE TRIGGER IF NOT EXISTS continuity_no_update
                BEFORE UPDATE ON continuity_checkpoints BEGIN
                SELECT RAISE(ABORT,'continuity checkpoints are immutable');
            END;
            CREATE TRIGGER IF NOT EXISTS continuity_no_delete
                BEFORE DELETE ON continuity_checkpoints BEGIN
                SELECT RAISE(ABORT,'continuity checkpoints are immutable');
            END;",
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Verify that each cited source entry exists in the intended ANSF
    /// conversation before writing derived task state. The caller must still
    /// authenticate access to that conversation and its project.
    pub fn append_verified(
        &self,
        checkpoint: &TaskCheckpoint,
        source: &crate::ansf::AnsfStore,
    ) -> Result<(), ContinuityError> {
        if !validate(checkpoint) {
            return Err(ContinuityError::InvalidInput);
        }
        for entry_id in &checkpoint.source_entry_ids {
            let known = source
                .read_entry(&checkpoint.conversation_id, *entry_id)
                .map_err(|_| ContinuityError::InvalidInput)?;
            if known.is_none() {
                return Err(ContinuityError::InvalidInput);
            }
        }
        self.append(checkpoint)
    }

    /// Idempotent replay; identical IDs may not be rebound to different facts.
    /// Source entry validity is checked against ANSF by the trusted host.
    pub fn append(&self, checkpoint: &TaskCheckpoint) -> Result<(), ContinuityError> {
        if !validate(checkpoint) {
            return Err(ContinuityError::InvalidInput);
        }
        let sources = serde_json::to_string(&checkpoint.source_entry_ids)
            .map_err(|_| ContinuityError::InvalidInput)?;
        let blockers = serde_json::to_string(&checkpoint.blockers)
            .map_err(|_| ContinuityError::InvalidInput)?;
        let conn = self.conn.lock().map_err(|_| ContinuityError::Locked)?;
        conn.execute(
            "INSERT INTO continuity_checkpoints
            (checkpoint_id,project_id,task_id,conversation_id,source_entry_ids,
             completed_work,next_action,blockers)
            VALUES (?1,?2,?3,?4,?5,?6,?7,?8)
            ON CONFLICT(checkpoint_id) DO NOTHING",
            params![
                checkpoint.checkpoint_id,
                checkpoint.project_id,
                checkpoint.task_id,
                checkpoint.conversation_id,
                sources,
                checkpoint.completed_work,
                checkpoint.next_action,
                blockers
            ],
        )?;
        let saved = conn.query_row(
            "SELECT project_id,task_id,conversation_id,source_entry_ids,
                completed_work,next_action,blockers
             FROM continuity_checkpoints WHERE checkpoint_id=?1",
            [&checkpoint.checkpoint_id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, String>(6)?,
                ))
            },
        )?;
        let expected = (
            checkpoint.project_id.clone(),
            checkpoint.task_id.clone(),
            checkpoint.conversation_id.clone(),
            sources,
            checkpoint.completed_work.clone(),
            checkpoint.next_action.clone(),
            blockers,
        );
        if saved != expected {
            return Err(ContinuityError::IdentityConflict);
        }
        Ok(())
    }

    /// Scoped history lookup: never infer permission to inspect a second chat
    /// from a task or project name alone.
    pub fn recent_for_conversation(
        &self,
        conversation_id: &str,
        limit: usize,
    ) -> Result<Vec<TaskCheckpoint>, ContinuityError> {
        if !valid_id(conversation_id) || limit == 0 || limit > 30 {
            return Err(ContinuityError::InvalidInput);
        }
        let conn = self.conn.lock().map_err(|_| ContinuityError::Locked)?;
        let mut stmt = conn.prepare(
            "SELECT checkpoint_id,project_id,task_id,conversation_id,
                    source_entry_ids,completed_work,next_action,blockers
             FROM continuity_checkpoints WHERE conversation_id=?1
             ORDER BY sequence DESC LIMIT ?2",
        )?;
        let records = stmt.query_map(params![conversation_id, limit as i64], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, String>(7)?,
            ))
        })?;
        let mut result = Vec::new();
        for row in records {
            let (
                checkpoint_id,
                project_id,
                task_id,
                conversation_id,
                source_entry_ids,
                completed_work,
                next_action,
                blockers,
            ) = row?;
            result.push(TaskCheckpoint {
                checkpoint_id,
                project_id,
                task_id,
                conversation_id,
                source_entry_ids: serde_json::from_str(&source_entry_ids)
                    .map_err(|_| ContinuityError::InvalidInput)?,
                completed_work,
                next_action,
                blockers: serde_json::from_str(&blockers)
                    .map_err(|_| ContinuityError::InvalidInput)?,
            });
        }
        Ok(result)
    }

    pub fn latest(
        &self,
        project_id: &str,
        task_id: &str,
    ) -> Result<Option<TaskCheckpoint>, ContinuityError> {
        if !valid_id(project_id) || !valid_id(task_id) {
            return Err(ContinuityError::InvalidInput);
        }
        let conn = self.conn.lock().map_err(|_| ContinuityError::Locked)?;
        let raw: Option<StoredCheckpoint> = conn
            .query_row(
                "SELECT checkpoint_id,project_id,task_id,conversation_id,
              source_entry_ids,completed_work,next_action,blockers
             FROM continuity_checkpoints
             WHERE project_id=?1 AND task_id=?2 ORDER BY sequence DESC LIMIT 1",
                params![project_id, task_id],
                |r| {
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        r.get(2)?,
                        r.get(3)?,
                        r.get(4)?,
                        r.get(5)?,
                        r.get(6)?,
                        r.get(7)?,
                    ))
                },
            )
            .optional()?;
        raw.map(
            |(
                checkpoint_id,
                project_id,
                task_id,
                conversation_id,
                source_entry_ids,
                completed_work,
                next_action,
                blockers,
            )| {
                Ok(TaskCheckpoint {
                    checkpoint_id,
                    project_id,
                    task_id,
                    conversation_id,
                    source_entry_ids: serde_json::from_str(&source_entry_ids)
                        .map_err(|_| ContinuityError::InvalidInput)?,
                    completed_work,
                    next_action,
                    blockers: serde_json::from_str(&blockers)
                        .map_err(|_| ContinuityError::InvalidInput)?,
                })
            },
        )
        .transpose()
    }
}
