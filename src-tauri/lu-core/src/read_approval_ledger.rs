//! Host-only durable record of *decisions* about model-proposed reads.
//! This is not a capability ledger: an APPROVED row cannot execute a tool.
//! Native effect execution is separately controlled by ConversationAuthority,
//! PolicyEngine and EffectJournal. Pending records are never approved by
//! restart; the caller reconciles them as interrupted before restoring UI.
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use std::{path::Path, sync::Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadDecision { Pending, Approved, Denied, Interrupted }
impl ReadDecision {
    fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "PENDING", Self::Approved => "APPROVED",
            Self::Denied => "DENIED", Self::Interrupted => "INTERRUPTED",
        }
    }
    fn parse(s: &str) -> Option<Self> {
        match s {
            "PENDING" => Some(Self::Pending), "APPROVED" => Some(Self::Approved),
            "DENIED" => Some(Self::Denied), "INTERRUPTED" => Some(Self::Interrupted),
            _ => None,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadLedgerError { Database(String), Invalid, Duplicate, Conflict, Missing, Transition, Poisoned }
impl From<rusqlite::Error> for ReadLedgerError {
    fn from(e: rusqlite::Error) -> Self { Self::Database(e.to_string()) }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadDecisionRecord {
    pub proposal_id: String,
    pub chat_id: String,
    pub generation_id: String,
    pub file_name: String,
    pub max_bytes: u32,
    pub decision: ReadDecision,
}
pub struct ReadApprovalLedger { connection: Mutex<Connection> }
fn valid_id(s: &str) -> bool {
    !s.is_empty() && s.len() <= 128 &&
        s.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
}
fn valid_filename(s: &str) -> bool {
    !s.is_empty() && s.len() <= 255 && s != "." && s != ".." &&
    !s.chars().any(|c| matches!(c, '/' | '\\' | ':' | '\0'))
}
impl ReadApprovalLedger {
    pub fn open(path: &Path) -> Result<Self, ReadLedgerError> {
        let connection = Connection::open(path)?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "synchronous", "FULL")?;
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS lu_read_approvals(
                proposal_id TEXT PRIMARY KEY NOT NULL,
                chat_id TEXT NOT NULL,
                generation_id TEXT NOT NULL,
                file_name TEXT NOT NULL,
                max_bytes INTEGER NOT NULL CHECK (max_bytes BETWEEN 1 AND 65536),
                state TEXT NOT NULL CHECK(state IN ('PENDING','APPROVED','DENIED','INTERRUPTED')),
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            CREATE INDEX IF NOT EXISTS lu_read_approvals_state
                ON lu_read_approvals(state);
            CREATE TRIGGER IF NOT EXISTS lu_read_approvals_no_delete
                BEFORE DELETE ON lu_read_approvals BEGIN
                SELECT RAISE(ABORT,'approval audit rows are immutable records');
            END;"
        )?;
        Ok(Self { connection: Mutex::new(connection) })
    }
    pub fn stage(
        &self, proposal_id: &str, chat_id: &str, generation_id: &str,
        file_name: &str, max_bytes: u32,
    ) -> Result<(), ReadLedgerError> {
        if !valid_id(proposal_id) || !valid_id(chat_id) || !valid_id(generation_id)
            || !valid_filename(file_name) || max_bytes == 0 || max_bytes > 65536 {
            return Err(ReadLedgerError::Invalid);
        }
        let conn = self.connection.lock().map_err(|_| ReadLedgerError::Poisoned)?;
        conn.execute(
            "INSERT INTO lu_read_approvals(proposal_id,chat_id,generation_id,file_name,max_bytes,state)
             VALUES(?1,?2,?3,?4,?5,'PENDING')",
            params![proposal_id,chat_id,generation_id,file_name,max_bytes]
        ).map_err(|e| {
            if matches!(e, rusqlite::Error::SqliteFailure(ref code, _)
                if code.code == rusqlite::ErrorCode::ConstraintViolation) {
                ReadLedgerError::Duplicate
            } else { ReadLedgerError::Database(e.to_string()) }
        })?;
        Ok(())
    }
    pub fn get(&self, proposal_id: &str) -> Result<Option<ReadDecisionRecord>, ReadLedgerError> {
        if !valid_id(proposal_id) { return Err(ReadLedgerError::Invalid); }
        let conn = self.connection.lock().map_err(|_| ReadLedgerError::Poisoned)?;
        let raw: Option<(String,String,String,String,i64,String)> = conn.query_row(
            "SELECT proposal_id,chat_id,generation_id,file_name,max_bytes,state
             FROM lu_read_approvals WHERE proposal_id=?1",
            [proposal_id],
            |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))
        ).optional()?;
        raw.map(|(proposal_id,chat_id,generation_id,file_name,max_bytes,state)| {
            if !(1..=65536).contains(&max_bytes) || !valid_filename(&file_name) {
                return Err(ReadLedgerError::Invalid);
            }
            Ok(ReadDecisionRecord{
                proposal_id, chat_id, generation_id, file_name,
                max_bytes: max_bytes as u32,
                decision: ReadDecision::parse(&state).ok_or(ReadLedgerError::Invalid)?,
            })
        }).transpose()
    }
    pub fn decide(
        &self, proposal_id: &str, chat_id: &str, generation_id: &str,
        file_name: &str, max_bytes: u32, decision: ReadDecision,
    ) -> Result<(), ReadLedgerError> {
        if !matches!(decision, ReadDecision::Approved | ReadDecision::Denied)
            || !valid_id(proposal_id) || !valid_id(chat_id) || !valid_id(generation_id)
            || !valid_filename(file_name) || max_bytes == 0 || max_bytes > 65536 {
            return Err(ReadLedgerError::Invalid);
        }
        let mut conn = self.connection.lock().map_err(|_| ReadLedgerError::Poisoned)?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current: Option<(String,String,String,i64,String)> = tx.query_row(
            "SELECT chat_id,generation_id,file_name,max_bytes,state
             FROM lu_read_approvals WHERE proposal_id=?1",
            [proposal_id],
            |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))
        ).optional()?;
        let Some((owner,origin,target,limit,state)) = current else {
            return Err(ReadLedgerError::Missing);
        };
        if owner != chat_id || origin != generation_id || target != file_name
            || limit != i64::from(max_bytes) {
            return Err(ReadLedgerError::Conflict);
        }
        if state != ReadDecision::Pending.as_str() {
            return Err(ReadLedgerError::Transition);
        }
        let updated = tx.execute(
            "UPDATE lu_read_approvals SET state=?1,updated_at=CURRENT_TIMESTAMP
             WHERE proposal_id=?2 AND state='PENDING'",
            params![decision.as_str(), proposal_id],
        )?;
        if updated != 1 { return Err(ReadLedgerError::Transition); }
        tx.commit()?;
        Ok(())
    }
    /// Call only after exclusive desktop ownership is established and any
    /// previous instance is stopped. Never auto-approve interrupted requests.
    pub fn reconcile_pending(&self) -> Result<usize, ReadLedgerError> {
        let conn = self.connection.lock().map_err(|_| ReadLedgerError::Poisoned)?;
        Ok(conn.execute(
            "UPDATE lu_read_approvals SET state='INTERRUPTED', updated_at=CURRENT_TIMESTAMP
             WHERE state='PENDING'", []
        )?)
    }
}
