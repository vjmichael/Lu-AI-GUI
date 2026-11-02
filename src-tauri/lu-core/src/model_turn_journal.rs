//! Durable, conversation-scoped audit of the external model JSON boundary.
//!
//! This is separate from the effect journal: it records evidence, NOT a grant,
//! an execution checkpoint, or permission to replay tool effects. The owning
//! trusted desktop must choose a profile-private SQLite path.

use std::{path::Path, sync::Mutex};

use crate::model_json_boundary::{JsonRepairStatus, MAX_BOUNDARY_JSON_BYTES};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelAuditKind {
    UserRequest,
    ModelOutput,
    ParsedCandidate,
    /// Host-selected capability, distinct from untrusted model candidate JSON.
    HostRouteSelected,
    Rejected,
    ToolFeedback,
    FinalAnswer,
    GenerationFailure,
}

impl ModelAuditKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::UserRequest => "USER_REQUEST",
            Self::ModelOutput => "MODEL_OUTPUT",
            Self::ParsedCandidate => "PARSED_CANDIDATE",
            Self::HostRouteSelected => "HOST_ROUTE_SELECTED",
            Self::Rejected => "REJECTED",
            Self::ToolFeedback => "TOOL_FEEDBACK",
            Self::FinalAnswer => "FINAL_ANSWER",
            Self::GenerationFailure => "GENERATION_FAILURE",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "USER_REQUEST" => Some(Self::UserRequest),
            "MODEL_OUTPUT" => Some(Self::ModelOutput),
            "PARSED_CANDIDATE" => Some(Self::ParsedCandidate),
            "HOST_ROUTE_SELECTED" => Some(Self::HostRouteSelected),
            "REJECTED" => Some(Self::Rejected),
            "TOOL_FEEDBACK" => Some(Self::ToolFeedback),
            "FINAL_ANSWER" => Some(Self::FinalAnswer),
            "GENERATION_FAILURE" => Some(Self::GenerationFailure),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelAuditError {
    Database(String),
    Poisoned,
    InvalidOrigin,
    OversizedEvent,
    InvalidStoredKind,
}

impl From<rusqlite::Error> for ModelAuditError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Database(error.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelAuditRecord {
    pub sequence: i64,
    pub kind: ModelAuditKind,
    pub payload: String,
    pub repair: Option<String>,
}

pub struct ModelTurnJournal {
    connection: Mutex<Connection>,
}

impl ModelTurnJournal {
    /// The SQLite CHECK constraint is part of the stored table schema, so a
    /// new audit kind requires a transactional table migration for old profiles.
    /// Keep sequence IDs and existing rows intact. A failed copy rolls back.
    fn install_schema(connection: &mut Connection) -> Result<(), ModelAuditError> {
        const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS lu_model_boundary_audit (
                sequence INTEGER PRIMARY KEY AUTOINCREMENT,
                conversation_id TEXT NOT NULL,
                task_id TEXT NOT NULL,
                kind TEXT NOT NULL CHECK(kind IN (
                    'USER_REQUEST','MODEL_OUTPUT','PARSED_CANDIDATE','HOST_ROUTE_SELECTED',
                    'REJECTED','TOOL_FEEDBACK','FINAL_ANSWER','GENERATION_FAILURE')),
                payload TEXT NOT NULL,
                repair TEXT,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
             );";
        const CREATE_INDEX: &str = "CREATE INDEX IF NOT EXISTS lu_model_boundary_by_task
            ON lu_model_boundary_audit(conversation_id, task_id, sequence);";
        let previous: Option<String> = connection.query_row(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name='lu_model_boundary_audit'",
            [], |row| row.get(0),
        ).optional()?;
        match previous {
            None => {
                connection.execute_batch(CREATE_TABLE)?;
                connection.execute_batch(CREATE_INDEX)?;
            }
            Some(schema) if schema.contains("'HOST_ROUTE_SELECTED'") => {
                connection.execute_batch(CREATE_INDEX)?;
            }
            Some(_) => {
                // No ALTER CHECK support in SQLite. This is an atomic rebuild
                // of this audit table ONLY, preserving the original sequence.
                let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
                tx.execute_batch("CREATE TABLE lu_model_boundary_audit_migrating (
                        sequence INTEGER PRIMARY KEY AUTOINCREMENT,
                        conversation_id TEXT NOT NULL,
                        task_id TEXT NOT NULL,
                        kind TEXT NOT NULL CHECK(kind IN (
                            'USER_REQUEST','MODEL_OUTPUT','PARSED_CANDIDATE','HOST_ROUTE_SELECTED',
                            'REJECTED','TOOL_FEEDBACK','FINAL_ANSWER','GENERATION_FAILURE')),
                        payload TEXT NOT NULL,
                        repair TEXT,
                        created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                    );
                    INSERT INTO lu_model_boundary_audit_migrating
                        (sequence,conversation_id,task_id,kind,payload,repair,created_at)
                    SELECT sequence,conversation_id,task_id,kind,payload,repair,created_at
                      FROM lu_model_boundary_audit;
                    DROP TABLE lu_model_boundary_audit;
                    ALTER TABLE lu_model_boundary_audit_migrating RENAME TO lu_model_boundary_audit;")?;
                tx.execute_batch(CREATE_INDEX)?;
                tx.commit()?;
            }
        }
        Ok(())
    }

    pub fn open(path: &Path) -> Result<Self, ModelAuditError> {
        let mut connection = Connection::open(path)?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "synchronous", "FULL")?;
        Self::install_schema(&mut connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    fn validate_origin(conversation_id: &str, task_id: &str) -> Result<(), ModelAuditError> {
        if conversation_id.trim().is_empty()
            || task_id.trim().is_empty()
            || conversation_id.len() > 128
            || task_id.len() > 128
        {
            return Err(ModelAuditError::InvalidOrigin);
        }
        Ok(())
    }

    /// Each append is committed under SQLite WAL/FULL before execution proceeds.
    /// Caller must never place model-provided IDs in the origin fields.
    pub fn append(
        &self,
        conversation_id: &str,
        task_id: &str,
        kind: ModelAuditKind,
        payload: &str,
        repair: Option<JsonRepairStatus>,
    ) -> Result<i64, ModelAuditError> {
        Self::validate_origin(conversation_id, task_id)?;
        if payload.len() > MAX_BOUNDARY_JSON_BYTES {
            return Err(ModelAuditError::OversizedEvent);
        }
        let repair = repair.map(|status| format!("{status:?}"));
        let connection = self
            .connection
            .lock()
            .map_err(|_| ModelAuditError::Poisoned)?;
        connection.execute(
            "INSERT INTO lu_model_boundary_audit
                 (conversation_id, task_id, kind, payload, repair)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![conversation_id, task_id, kind.as_str(), payload, repair],
        )?;
        Ok(connection.last_insert_rowid())
    }

    pub fn events_for(
        &self,
        conversation_id: &str,
        task_id: &str,
    ) -> Result<Vec<ModelAuditRecord>, ModelAuditError> {
        Self::validate_origin(conversation_id, task_id)?;
        let connection = self
            .connection
            .lock()
            .map_err(|_| ModelAuditError::Poisoned)?;
        let mut statement = connection.prepare(
            "SELECT sequence, kind, payload, repair FROM lu_model_boundary_audit
             WHERE conversation_id=?1 AND task_id=?2 ORDER BY sequence ASC",
        )?;
        let rows = statement.query_map(params![conversation_id, task_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })?;
        let mut events = Vec::new();
        for row in rows {
            let (sequence, kind, payload, repair) = row?;
            events.push(ModelAuditRecord {
                sequence,
                kind: ModelAuditKind::parse(&kind).ok_or(ModelAuditError::InvalidStoredKind)?,
                payload,
                repair,
            });
        }
        Ok(events)
    }
}
