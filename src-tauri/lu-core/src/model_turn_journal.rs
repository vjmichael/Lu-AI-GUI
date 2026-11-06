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

/// A read-only, content-free projection of the persisted model/tool audit.
/// Raw model output, file contents, prompts and provider parameters are not
/// serialized into operator UI evidence.
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct ModelTurnAuditSummary{
    pub task_id:String,
    pub last_at:String,
    pub last_sequence:i64,
    pub model_outputs:u32,
    pub tool_feedback:u32,
    pub host_routes:u32,
    pub rejections:u32,
    pub generation_failures:u32,
    pub final_answers:u32,
    pub repaired_replies:u32,
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

    /// Recent per-task tool evidence for a single authenticated conversation.
    /// SQLite groups before returning: no model text/arguments cross this API.
    /// An entry proves recorded model/tool activity, never an authorized effect.
    pub fn summaries_for_conversation(
        &self,conversation_id:&str,limit:usize,
    )->Result<Vec<ModelTurnAuditSummary>,ModelAuditError>{
        Self::validate_origin(conversation_id,"read-only-summary")?;
        if limit==0||limit>64{return Err(ModelAuditError::InvalidOrigin)}
        let connection=self.connection.lock().map_err(|_|ModelAuditError::Poisoned)?;
        let mut statement=connection.prepare(
            "SELECT task_id,MAX(created_at),MAX(sequence),
             SUM(CASE WHEN kind='MODEL_OUTPUT' THEN 1 ELSE 0 END),
             SUM(CASE WHEN kind='TOOL_FEEDBACK' THEN 1 ELSE 0 END),
             SUM(CASE WHEN kind='HOST_ROUTE_SELECTED' THEN 1 ELSE 0 END),
             SUM(CASE WHEN kind='REJECTED' THEN 1 ELSE 0 END),
             SUM(CASE WHEN kind='GENERATION_FAILURE' THEN 1 ELSE 0 END),
             SUM(CASE WHEN kind='FINAL_ANSWER' THEN 1 ELSE 0 END),
             SUM(CASE WHEN repair IS NOT NULL THEN 1 ELSE 0 END)
             FROM lu_model_boundary_audit WHERE conversation_id=?1
             GROUP BY task_id ORDER BY MAX(sequence) DESC LIMIT ?2"
        )?;
        let rows=statement.query_map(params![conversation_id,limit as i64],|row|{
            Ok(ModelTurnAuditSummary{
                task_id:row.get(0)?,last_at:row.get(1)?,
                last_sequence:row.get(2)?,
                model_outputs:row.get::<_,i64>(3)?.clamp(0,u32::MAX as i64) as u32,
                tool_feedback:row.get::<_,i64>(4)?.clamp(0,u32::MAX as i64) as u32,
                host_routes:row.get::<_,i64>(5)?.clamp(0,u32::MAX as i64) as u32,
                rejections:row.get::<_,i64>(6)?.clamp(0,u32::MAX as i64) as u32,
                generation_failures:row.get::<_,i64>(7)?.clamp(0,u32::MAX as i64) as u32,
                final_answers:row.get::<_,i64>(8)?.clamp(0,u32::MAX as i64) as u32,
                repaired_replies:row.get::<_,i64>(9)?.clamp(0,u32::MAX as i64) as u32,
            })
        })?;
        let mut summaries=Vec::new();
        for row in rows{summaries.push(row?)}
        Ok(summaries)
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

#[cfg(test)]
mod wave3_summary_tests{
    use super::*;
    #[test]
    fn content_free_audit_survives_restart_and_never_leaks_other_chats(){
        let dir=std::env::temp_dir().join(format!(
            "lu-wave3-audit-{}-{}",std::process::id(),"summary"
        ));
        let _=std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let db=dir.join("audit.sqlite");
        {
            let ledger=ModelTurnJournal::open(&db).unwrap();
            ledger.append("chat-a","task-a",ModelAuditKind::UserRequest,
                "private user secret",None).unwrap();
            ledger.append("chat-a","task-a",ModelAuditKind::ModelOutput,
                r#"{"secret":"private file bytes"}"#,None).unwrap();
            ledger.append("chat-a","task-a",ModelAuditKind::ToolFeedback,
                "tool result: private data",None).unwrap();
            ledger.append("chat-a","task-a",ModelAuditKind::FinalAnswer,
                "private answer",None).unwrap();
            ledger.append("chat-b","task-b",ModelAuditKind::ToolFeedback,
                "other chat data",None).unwrap();
        }
        let ledger=ModelTurnJournal::open(&db).unwrap();
        let summary=ledger.summaries_for_conversation("chat-a",8).unwrap();
        assert_eq!(summary.len(),1);
        assert_eq!(summary[0].task_id,"task-a");
        assert_eq!(summary[0].tool_feedback,1);
        assert_eq!(summary[0].final_answers,1);
        assert_eq!(summary[0].model_outputs,1);
        assert_eq!(ledger.summaries_for_conversation("chat-b",8).unwrap().len(),1);
        assert!(ledger.summaries_for_conversation("chat-a",0).is_err());
        let debug=format!("{summary:?}");
        assert!(!debug.contains("secret"));
        assert!(!debug.contains("private"));
        assert!(!debug.contains("chat-b"));
        drop(ledger);std::fs::remove_dir_all(dir).unwrap();
    }
}
