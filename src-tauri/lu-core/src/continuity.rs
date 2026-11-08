//! Append-only task continuity checkpoints anchored to ANSF source entries.
//!
//! Checkpoints are derived observations, not authority and not edits to chat.
//! Hosts must authenticate session/project access before calling this API.
use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest,Sha256};
use crate::journal::{EffectJournal,EffectState};
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

#[derive(Debug,Clone,PartialEq,Eq)]
pub struct VerifiedEffectEvidence{
    pub checkpoint_id:String,
    pub call_id:String,
    pub tool_id:String,
    pub receipt_sha256:String,
    pub receipt_bytes:u64,
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

/// Context Change Monitor: a read-only comparison of two immutable, source-
/// backed checkpoints from the same task. A changed historical fact is NOT
/// approval to run a tool, and a changed plan needs human review.
#[derive(Debug,Clone,PartialEq,Eq,serde::Serialize)]
#[serde(rename_all="camelCase")]
pub struct ContextChangeReview{
    pub project_id:String,
    pub task_id:String,
    pub previous_checkpoint_id:String,
    pub current_checkpoint_id:String,
    pub next_action_changed:bool,
    pub completed_work_changed:bool,
    pub source_evidence_changed:bool,
    pub added_blockers:Vec<String>,
    pub cleared_blockers:Vec<String>,
    pub needs_review:bool,
}
pub fn compare_task_checkpoints(
    previous:&TaskCheckpoint,current:&TaskCheckpoint
)->Result<ContextChangeReview,ContinuityError>{
    if !validate(previous)||!validate(current)||
       previous.project_id!=current.project_id||
       previous.task_id!=current.task_id||
       previous.checkpoint_id==current.checkpoint_id{
        return Err(ContinuityError::InvalidInput)
    }
    let added_blockers=current.blockers.iter()
        .filter(|b|!previous.blockers.contains(b)).cloned().collect::<Vec<_>>();
    let cleared_blockers=previous.blockers.iter()
        .filter(|b|!current.blockers.contains(b)).cloned().collect::<Vec<_>>();
    let next_action_changed=previous.next_action!=current.next_action;
    let completed_work_changed=previous.completed_work!=current.completed_work;
    let source_evidence_changed=previous.conversation_id!=current.conversation_id||
        previous.source_entry_ids!=current.source_entry_ids;
    let needs_review=next_action_changed||completed_work_changed||
        source_evidence_changed||!added_blockers.is_empty()||!cleared_blockers.is_empty();
    Ok(ContextChangeReview{
        project_id:current.project_id.clone(),task_id:current.task_id.clone(),
        previous_checkpoint_id:previous.checkpoint_id.clone(),
        current_checkpoint_id:current.checkpoint_id.clone(),
        next_action_changed,completed_work_changed,source_evidence_changed,
        added_blockers,cleared_blockers,needs_review
    })
}
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
            END;
            CREATE TABLE IF NOT EXISTS continuity_verified_effects(
                checkpoint_id TEXT NOT NULL REFERENCES continuity_checkpoints(checkpoint_id),
                call_id TEXT NOT NULL,
                tool_id TEXT NOT NULL,
                receipt_sha256 TEXT NOT NULL,
                receipt_bytes INTEGER NOT NULL CHECK(receipt_bytes > 0),
                PRIMARY KEY(checkpoint_id,call_id)
            );
            CREATE TRIGGER IF NOT EXISTS continuity_effect_no_update
                BEFORE UPDATE ON continuity_verified_effects BEGIN
                SELECT RAISE(ABORT,'effect evidence is immutable');
            END;
            CREATE TRIGGER IF NOT EXISTS continuity_effect_no_delete
                BEFORE DELETE ON continuity_verified_effects BEGIN
                SELECT RAISE(ABORT,'effect evidence is immutable');
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

    /// Attach only an ALREADY-APPLIED, nonempty immutable host effect receipt
    /// to an existing source-backed checkpoint. A model cannot promote a
    /// PREPARED/EXECUTING/FAILED/UNCERTAIN effect to completed work.
    /// Host caller must establish current task scope and user confirmation.
    pub fn link_verified_effect(
        &self,checkpoint_id:&str,call_id:&str,journal:&EffectJournal
    )->Result<VerifiedEffectEvidence,ContinuityError>{
        if !valid_id(checkpoint_id)||!valid_id(call_id){
            return Err(ContinuityError::InvalidInput)
        }
        let record=journal.get(call_id)
            .map_err(|_|ContinuityError::InvalidInput)?
            .ok_or(ContinuityError::InvalidInput)?;
        if record.state!=EffectState::Applied{
            return Err(ContinuityError::InvalidInput)
        }
        let bytes=record.receipt.as_deref()
            .filter(|v|!v.is_empty()).ok_or(ContinuityError::InvalidInput)?;
        let evidence=VerifiedEffectEvidence{
            checkpoint_id:checkpoint_id.to_string(),
            call_id:call_id.to_string(),tool_id:record.tool_id,
            receipt_sha256:format!("{:x}",Sha256::digest(bytes)),
            receipt_bytes:bytes.len() as u64
        };
        let conn=self.conn.lock().map_err(|_|ContinuityError::Locked)?;
        let exists:i64=conn.query_row(
            "SELECT COUNT(*) FROM continuity_checkpoints WHERE checkpoint_id=?1",
            [checkpoint_id],|r|r.get(0)
        )?;
        if exists!=1{return Err(ContinuityError::InvalidInput)}
        conn.execute(
            "INSERT OR IGNORE INTO continuity_verified_effects
             (checkpoint_id,call_id,tool_id,receipt_sha256,receipt_bytes)
             VALUES(?1,?2,?3,?4,?5)",
            params![evidence.checkpoint_id,evidence.call_id,evidence.tool_id,
                evidence.receipt_sha256,evidence.receipt_bytes as i64]
        )?;
        let saved:VerifiedEffectEvidence=conn.query_row(
            "SELECT checkpoint_id,call_id,tool_id,receipt_sha256,receipt_bytes
             FROM continuity_verified_effects
             WHERE checkpoint_id=?1 AND call_id=?2",
            params![checkpoint_id,call_id],|r|Ok(VerifiedEffectEvidence{
                checkpoint_id:r.get(0)?,call_id:r.get(1)?,
                tool_id:r.get(2)?,receipt_sha256:r.get(3)?,
                receipt_bytes:r.get::<_,i64>(4)? as u64
            })
        )?;
        if saved!=evidence{return Err(ContinuityError::IdentityConflict)}
        Ok(saved)
    }

    /// Read-only evidence lookup revalidates the original EffectJournal.
    /// Nothing here retries a tool call or grants new effect permission.
    pub fn verified_effects_for_checkpoint(
        &self,checkpoint_id:&str,journal:&EffectJournal
    )->Result<Vec<VerifiedEffectEvidence>,ContinuityError>{
        if !valid_id(checkpoint_id){return Err(ContinuityError::InvalidInput)}
        let stored={
            let conn=self.conn.lock().map_err(|_|ContinuityError::Locked)?;
            let mut stmt=conn.prepare(
                "SELECT checkpoint_id,call_id,tool_id,receipt_sha256,receipt_bytes
                 FROM continuity_verified_effects WHERE checkpoint_id=?1
                 ORDER BY call_id LIMIT 65"
            )?;
            let rows=stmt.query_map([checkpoint_id],|r|Ok(VerifiedEffectEvidence{
                checkpoint_id:r.get(0)?,call_id:r.get(1)?,
                tool_id:r.get(2)?,receipt_sha256:r.get(3)?,
                receipt_bytes:r.get::<_,i64>(4)? as u64
            }))?.collect::<Result<Vec<_>,_>>()?;
            rows
        };
        if stored.len()>64{return Err(ContinuityError::InvalidInput)}
        for e in &stored{
            let actual=journal.get(&e.call_id)
                .map_err(|_|ContinuityError::InvalidInput)?
                .ok_or(ContinuityError::InvalidInput)?;
            let receipt=actual.receipt.as_deref().filter(|v|!v.is_empty())
                .ok_or(ContinuityError::InvalidInput)?;
            if actual.state!=EffectState::Applied||actual.tool_id!=e.tool_id||
               receipt.len() as u64!=e.receipt_bytes||
               format!("{:x}",Sha256::digest(receipt))!=e.receipt_sha256{
                return Err(ContinuityError::IdentityConflict)
            }
        }
        Ok(stored)
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


    /// Host-facing resume read tied to the *exact* authenticated conversation,
    /// project and task. Unlike `latest(project,task)`, a guessed or reused
    /// project/task identifier cannot expose a checkpoint from another chat.
    /// This does not authenticate a user by itself: trusted desktop code must
    /// independently verify conversation ownership before making this call.
    /// A remembered task record never carries authorization to invoke tools.
    /// Production host adapter: require an authenticated USER turn in this
    /// exact conversation before reading its derived task checkpoint.
    /// Historical context can never itself mint tool authority. The verifier
    /// must also establish account/conversation ownership, not just an ID match.
    pub fn latest_for_verified_user_turn(
        &self,
        project_id: &str,
        task_id: &str,
        conversation_id: &str,
        current_user_turn_id: &str,
        verifier: &dyn crate::conversation_authority::TrustedUserTurnVerifier,
    ) -> Result<Option<TaskCheckpoint>, ContinuityError> {
        if !valid_id(conversation_id)
            || !valid_id(current_user_turn_id)
            || !verifier.verify_user_turn(conversation_id, current_user_turn_id)
        {
            return Err(ContinuityError::InvalidInput);
        }
        self.latest_for_conversation(project_id, task_id, conversation_id)
    }

    pub fn latest_for_conversation(
        &self,
        project_id: &str,
        task_id: &str,
        conversation_id: &str,
    ) -> Result<Option<TaskCheckpoint>, ContinuityError> {
        if !valid_id(project_id) || !valid_id(task_id) || !valid_id(conversation_id) {
            return Err(ContinuityError::InvalidInput);
        }
        let conn = self.conn.lock().map_err(|_| ContinuityError::Locked)?;
        let row: Option<StoredCheckpoint> = conn
            .query_row(
                "SELECT checkpoint_id,project_id,task_id,conversation_id,
                        source_entry_ids,completed_work,next_action,blockers
                 FROM continuity_checkpoints
                 WHERE project_id=?1 AND task_id=?2 AND conversation_id=?3
                 ORDER BY sequence DESC LIMIT 1",
                params![project_id, task_id, conversation_id],
                |r| {
                    Ok((
                        r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?,
                        r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?,
                    ))
                },
            )
            .optional()?;
        row.map(|(checkpoint_id, project_id, task_id, conversation_id,
                  source_entry_ids, completed_work, next_action, blockers)| {
            let source_entry_ids: Vec<i64> = serde_json::from_str(&source_entry_ids)
                .map_err(|_| ContinuityError::InvalidInput)?;
            let blockers: Vec<String> = serde_json::from_str(&blockers)
                .map_err(|_| ContinuityError::InvalidInput)?;
            let checkpoint = TaskCheckpoint {
                checkpoint_id, project_id, task_id, conversation_id,
                source_entry_ids, completed_work, next_action, blockers,
            };
            if !validate(&checkpoint) {
                return Err(ContinuityError::InvalidInput);
            }
            Ok(checkpoint)
        }).transpose()
    }

    /// Two most recent immutable records, newest first. The caller MUST
    /// authenticate both cited ANSF books before comparing any source facts.
    pub fn recent_for_task(
        &self,project_id:&str,task_id:&str
    )->Result<Vec<TaskCheckpoint>,ContinuityError>{
        if !valid_id(project_id)||!valid_id(task_id){
            return Err(ContinuityError::InvalidInput)
        }
        let conn=self.conn.lock().map_err(|_|ContinuityError::Locked)?;
        let mut stmt=conn.prepare(
            "SELECT checkpoint_id,project_id,task_id,conversation_id,
                    source_entry_ids,completed_work,next_action,blockers
             FROM continuity_checkpoints
             WHERE project_id=?1 AND task_id=?2 ORDER BY sequence DESC LIMIT 2"
        )?;
        let saved=stmt.query_map(params![project_id,task_id],|r|Ok((
            r.get::<_,String>(0)?,r.get::<_,String>(1)?,
            r.get::<_,String>(2)?,r.get::<_,String>(3)?,
            r.get::<_,String>(4)?,r.get::<_,String>(5)?,
            r.get::<_,String>(6)?,r.get::<_,String>(7)?
        )))?.collect::<Result<Vec<_>,_>>()?;
        let mut out=Vec::with_capacity(saved.len());
        for (checkpoint_id,project_id,task_id,conversation_id,
             source_entry_ids,completed_work,next_action,blockers) in saved{
            let item=TaskCheckpoint{
                checkpoint_id,project_id,task_id,conversation_id,
                source_entry_ids:serde_json::from_str(&source_entry_ids)
                    .map_err(|_|ContinuityError::InvalidInput)?,
                completed_work,next_action,
                blockers:serde_json::from_str(&blockers)
                    .map_err(|_|ContinuityError::InvalidInput)?,
            };
            if !validate(&item){return Err(ContinuityError::InvalidInput)}
            out.push(item);
        }
        Ok(out)
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
