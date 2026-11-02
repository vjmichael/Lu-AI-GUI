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
            CREATE INDEX IF NOT EXISTS lu_effects_by_state ON lu_effects(state);",
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
