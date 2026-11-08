//! SQLite-backed durable effect intent and recovery journal.
//!
//! A tool invocation MUST be marked EXECUTING before side effects begin.
//! After restart, incomplete EXECUTING entries become UNCERTAIN and MUST NOT
//! be blindly repeated. The caller reconciles uncertain effects manually or
//! with an effect-specific idempotency protocol.
//!
//! This journal records *intent*, not an entire agent checkpoint. Full task
//! state/checkpoint orchestration remains a separate implementation stage.

use std::{path::Path, sync::Mutex};

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use sha2::{Digest, Sha256};

use crate::contracts::ToolInvocation;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectState {
    Prepared,
    Executing,
    Applied,
    Failed,
    Uncertain,
}

impl EffectState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Prepared => "PREPARED",
            Self::Executing => "EXECUTING",
            Self::Applied => "APPLIED",
            Self::Failed => "FAILED",
            Self::Uncertain => "UNCERTAIN",
        }
    }
    fn parse(s: &str) -> Option<Self> {
        match s {
            "PREPARED" => Some(Self::Prepared),
            "EXECUTING" => Some(Self::Executing),
            "APPLIED" => Some(Self::Applied),
            "FAILED" => Some(Self::Failed),
            "UNCERTAIN" => Some(Self::Uncertain),
            _ => None,
        }
    }
}

/// E1-P10 independent host outcome. This is never inferred from the
/// worker exit code, model FinalAnswer or a normal EffectState::Applied.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum IndependentOutcomeVerdict{Verified,Unsupported,Conflict}
impl IndependentOutcomeVerdict{
    fn as_str(self)->&'static str{
        match self{
            Self::Verified=>"VERIFIED",
            Self::Unsupported=>"UNSUPPORTED",
            Self::Conflict=>"CONFLICT",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JournalError {
    Database(String),
    Poisoned,
    InvalidCallId,
    InvalidTransition,
    DuplicateIdConflict,
    OversizedReceipt,
    InvalidStoredState,
}

impl From<rusqlite::Error> for JournalError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Database(error.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrepareOutcome {
    New,
    Existing(EffectState),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectRecord {
    pub call_id: String,
    pub tool_id: String,
    pub fingerprint: String,
    pub state: EffectState,
    /// Opaque encoded ToolReply bytes. Enforce Protobuf validation at the
    /// boundary that stores/reads receipts.
    pub receipt: Option<Vec<u8>>,
}

pub struct EffectJournal {
    connection: Mutex<Connection>,
}

fn fingerprint(call: &ToolInvocation) -> String {
    let mut hash = Sha256::new();
    for part in [
        call.tool_id.as_bytes(),
        call.workspace_id.as_deref().unwrap_or("").as_bytes(),
        call.input.type_url.as_bytes(),
        &call.input.bytes,
    ] {
        hash.update((part.len() as u64).to_le_bytes());
        hash.update(part);
    }
    format!("{:x}", hash.finalize())
}

impl EffectJournal {
    /// Open an existing DB or create a new one. The caller controls the path
    /// and must enforce a single active owner before startup recovery.
    pub fn open(path: &Path) -> Result<Self, JournalError> {
        let connection = Connection::open(path)?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "synchronous", "FULL")?;
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS lu_effects (
                call_id TEXT PRIMARY KEY NOT NULL,
                tool_id TEXT NOT NULL,
                fingerprint TEXT NOT NULL,
                state TEXT NOT NULL CHECK(state IN
                    ('PREPARED','EXECUTING','APPLIED','FAILED','UNCERTAIN')),
                receipt BLOB,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            CREATE INDEX IF NOT EXISTS lu_effects_by_state ON lu_effects(state);
            CREATE TABLE IF NOT EXISTS lu_independent_outcomes (
                call_id TEXT PRIMARY KEY NOT NULL,
                receipt_sha256 TEXT NOT NULL,
                verdict TEXT NOT NULL
                  CHECK(verdict IN ('VERIFIED','UNSUPPORTED','CONFLICT')),
                method TEXT NOT NULL,
                observed_sha256 TEXT,
                checked_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY(call_id) REFERENCES lu_effects(call_id)
            );",
        )?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    /// An identical call_id is idempotent only when the parameters match.
    /// Returning Existing does NOT grant permission to execute again.
    pub fn prepare(&self, call: &ToolInvocation) -> Result<PrepareOutcome, JournalError> {
        if call.call_id.trim().is_empty() || call.call_id.len() > 128 {
            return Err(JournalError::InvalidCallId);
        }
        let digest = fingerprint(call);
        let mut connection = self.connection.lock().map_err(|_| JournalError::Poisoned)?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let inserted = tx.execute(
            "INSERT OR IGNORE INTO lu_effects(call_id, tool_id, fingerprint, state)
             VALUES (?1, ?2, ?3, 'PREPARED')",
            params![call.call_id, call.tool_id, digest],
        )?;
        let outcome = if inserted == 1 {
            PrepareOutcome::New
        } else {
            let (tool_id, existing_digest, state): (String, String, String) = tx.query_row(
                "SELECT tool_id, fingerprint, state FROM lu_effects WHERE call_id=?1",
                params![call.call_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )?;
            if tool_id != call.tool_id || existing_digest != digest {
                return Err(JournalError::DuplicateIdConflict);
            }
            PrepareOutcome::Existing(
                EffectState::parse(&state).ok_or(JournalError::InvalidStoredState)?,
            )
        };
        tx.commit()?;
        Ok(outcome)
    }

    fn transition(
        &self,
        call_id: &str,
        before: EffectState,
        after: EffectState,
        receipt: Option<&[u8]>,
    ) -> Result<(), JournalError> {
        if receipt.is_some_and(|r| r.len() > 8 * 1024 * 1024) {
            return Err(JournalError::OversizedReceipt);
        }
        let connection = self.connection.lock().map_err(|_| JournalError::Poisoned)?;
        let changed = connection.execute(
            "UPDATE lu_effects SET state=?1, receipt=?2, updated_at=CURRENT_TIMESTAMP
             WHERE call_id=?3 AND state=?4",
            params![after.as_str(), receipt, call_id, before.as_str()],
        )?;
        if changed != 1 {
            return Err(JournalError::InvalidTransition);
        }
        Ok(())
    }

    /// Call this, and wait for success, BEFORE performing external side effects.
    pub fn mark_executing(&self, call_id: &str) -> Result<(), JournalError> {
        self.transition(call_id, EffectState::Prepared, EffectState::Executing, None)
    }

    /// Commit a validated, encoded ToolReply as the durable applied receipt.
    pub fn mark_applied(&self, call_id: &str, receipt: &[u8]) -> Result<(), JournalError> {
        self.transition(
            call_id,
            EffectState::Executing,
            EffectState::Applied,
            Some(receipt),
        )
    }

    /// Use only when the operation is known not to have caused any side effect.
    /// Ambiguous partial failures must be UNCERTAIN, not FAILED.
    pub fn mark_failed(&self, call_id: &str) -> Result<(), JournalError> {
        self.transition(call_id, EffectState::Executing, EffectState::Failed, None)
    }

    pub fn mark_uncertain(&self, call_id: &str) -> Result<(), JournalError> {
        self.transition(
            call_id,
            EffectState::Executing,
            EffectState::Uncertain,
            None,
        )
    }

    /// Bind a second, host-only factual observation to the EXACT already
    /// APPLIED worker receipt. Writes a different durable SQLite table but
    /// does NOT alter the effect's original execution classification. An
    /// identical replay is idempotent, a conflicting replay is refused.
    /// NEVER use this table as a substitute for process custody/reaping.
    pub fn record_independent_outcome(
        &self,call_id:&str,receipt:&[u8],verdict:IndependentOutcomeVerdict,
        method:&str,observed_sha256:Option<&str>,
    )->Result<(),JournalError>{
        if call_id.trim().is_empty()||call_id.len()>128||receipt.is_empty()
            ||receipt.len()>8*1024*1024
            ||!matches!(method,
                "GIT_HEAD_DIRECT_V1"|"GIT_BRANCH_DIRECT_V1"|
                "WAV_PCM_DIRECT_V1"|"GIT_STATUS_UNSUPPORTED_V1"|
                "GIT_HISTORY_UNSUPPORTED_V1"|"GIT_HISTORY_LOOSE_DAG_V1"|
            "GIT_HISTORY_OBJECT_DAG_V2"|"GIT_STATUS_INDEX_TREE_V1"|
            "MP4_BOXES_DIRECT_V1"|"MP4_UNSUPPORTED_V1"){
            return Err(JournalError::InvalidTransition)
        }
        let valid_sha=|s:&str|s.len()==64&&s.bytes().all(|b|
            b.is_ascii_digit()||(b'a'..=b'f').contains(&b));
        if observed_sha256.is_some_and(|v|!valid_sha(v))
            ||(verdict==IndependentOutcomeVerdict::Verified)
                !=observed_sha256.is_some(){
            return Err(JournalError::InvalidTransition)
        }
        if verdict==IndependentOutcomeVerdict::Verified
            &&method.ends_with("_UNSUPPORTED_V1")
            ||verdict!=IndependentOutcomeVerdict::Unsupported
                &&method.ends_with("_UNSUPPORTED_V1"){
            return Err(JournalError::InvalidTransition)
        }
        let receipt_sha=format!("{:x}",Sha256::digest(receipt));
        let mut conn=self.connection.lock().map_err(|_|JournalError::Poisoned)?;
        let tx=conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing=tx.query_row(
            "SELECT state,receipt FROM lu_effects WHERE call_id=?1",
            params![call_id],
            |row|Ok((row.get::<_,String>(0)?,row.get::<_,Option<Vec<u8>>>(1)?))
        ).optional()?;
        let Some((state,bytes))=existing else{return Err(JournalError::InvalidTransition)};
        if state!="APPLIED"||bytes.as_deref()!=Some(receipt){
            return Err(JournalError::InvalidTransition)
        }
        let inserted=tx.execute(
            "INSERT OR IGNORE INTO lu_independent_outcomes
                (call_id,receipt_sha256,verdict,method,observed_sha256)
             VALUES (?1,?2,?3,?4,?5)",
            params![call_id,receipt_sha,verdict.as_str(),method,observed_sha256],
        )?;
        if inserted==0{
            let (hash,old_verdict,old_method,old_observed):(
                String,String,String,Option<String>
            )=tx.query_row(
                "SELECT receipt_sha256,verdict,method,observed_sha256
                 FROM lu_independent_outcomes WHERE call_id=?1",
                params![call_id],
                |row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))
            )?;
            if hash!=receipt_sha||old_verdict!=verdict.as_str()
                ||old_method!=method||old_observed.as_deref()!=observed_sha256{
                return Err(JournalError::DuplicateIdConflict)
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Run only after the previous Lu process is known to have stopped.
    /// Otherwise, this could misclassify a still-running tool as uncertain.
    /// Never silently retry these effects.
    pub fn reconcile_interrupted(&self) -> Result<usize, JournalError> {
        let connection = self.connection.lock().map_err(|_| JournalError::Poisoned)?;
        let count = connection.execute(
            "UPDATE lu_effects SET state='UNCERTAIN', updated_at=CURRENT_TIMESTAMP
             WHERE state='EXECUTING'",
            [],
        )?;
        Ok(count)
    }

    pub fn get(&self, call_id: &str) -> Result<Option<EffectRecord>, JournalError> {
        let connection = self.connection.lock().map_err(|_| JournalError::Poisoned)?;
        let row = connection
            .query_row(
                "SELECT call_id, tool_id, fingerprint, state, receipt
                 FROM lu_effects WHERE call_id=?1",
                params![call_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, Option<Vec<u8>>>(4)?,
                    ))
                },
            )
            .optional()?;
        match row {
            Some((call_id, tool_id, fingerprint, state, receipt)) => Ok(Some(EffectRecord {
                call_id,
                tool_id,
                fingerprint,
                state: EffectState::parse(&state).ok_or(JournalError::InvalidStoredState)?,
                receipt,
            })),
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod independent_outcome_tests{
    use super::*;
    use crate::contracts::{ProtoPayload,ToolInvocation};
    use std::{fs,time::{SystemTime,UNIX_EPOCH}};
    #[test]
    fn outcome_is_separate_durable_receipt_and_rejects_forged_promotion(){
        let root=std::env::temp_dir().join(format!("lu-p10-outcome-{}-{}",
            std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH)
                .unwrap().as_nanos()));
        fs::create_dir_all(&root).unwrap();
        let file=root.join("effects.sqlite");
        let call=ToolInvocation{call_id:"host-worker-1".into(),
            tool_id:"lu.reviewed.git.head".into(),workspace_id:None,
            input:ProtoPayload{type_url:"test".into(),bytes:vec![1]}};
        let digest=format!("{:x}",Sha256::digest(b"direct-HEAD"));
        {
            let journal=EffectJournal::open(&file).unwrap();
            journal.prepare(&call).unwrap();
            assert_eq!(journal.record_independent_outcome(
                &call.call_id,b"worker-claim",IndependentOutcomeVerdict::Verified,
                "GIT_HEAD_DIRECT_V1",Some(&digest)),Err(JournalError::InvalidTransition));
            journal.mark_executing(&call.call_id).unwrap();
            journal.mark_applied(&call.call_id,b"worker-claim").unwrap();
            journal.record_independent_outcome(
                &call.call_id,b"worker-claim",IndependentOutcomeVerdict::Verified,
                "GIT_HEAD_DIRECT_V1",Some(&digest)).unwrap();
            journal.record_independent_outcome(
                &call.call_id,b"worker-claim",IndependentOutcomeVerdict::Verified,
                "GIT_HEAD_DIRECT_V1",Some(&digest)).unwrap();
            assert_eq!(journal.record_independent_outcome(
                &call.call_id,b"changed",IndependentOutcomeVerdict::Verified,
                "GIT_HEAD_DIRECT_V1",Some(&digest)),Err(JournalError::InvalidTransition));
            assert_eq!(journal.record_independent_outcome(
                &call.call_id,b"worker-claim",IndependentOutcomeVerdict::Conflict,
                "GIT_HEAD_DIRECT_V1",None),Err(JournalError::DuplicateIdConflict));
        }
        {
            let journal=EffectJournal::open(&file).unwrap();
            assert_eq!(journal.get(&call.call_id).unwrap().unwrap().state,
                EffectState::Applied);
            let conn=rusqlite::Connection::open(&file).unwrap();
            let count:i64=conn.query_row(
                "SELECT COUNT(*) FROM lu_independent_outcomes
                 WHERE verdict='VERIFIED' AND call_id='host-worker-1'",
                [],|row|row.get(0)).unwrap();
            assert_eq!(count,1);
        }
        fs::remove_dir_all(root).unwrap();
    }
}
