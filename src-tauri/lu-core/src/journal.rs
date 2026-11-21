//! SQLite-backed durable effect intent and recovery journal.
//!
//! A tool invocation MUST be marked EXECUTING before side effects begin.
//! After restart, incomplete EXECUTING entries become UNCERTAIN and MUST NOT
//! be blindly repeated. The caller reconciles uncertain effects manually or
//! with an effect-specific idempotency protocol.
//!
//! This journal records *intent*, not an entire agent checkpoint. Full task
//! state/checkpoint orchestration remains a separate implementation stage.

use std::{path::Path, sync::Mutex, time::Duration};

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
    /// A prior owner's OS worker shutdown remains unproven. Do not admit
    /// another DLC effect merely because a SQLite row can be rewritten.
    IpcRestartNeedsCustody,
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

/// Host-owned guard for the one active Lu IPC effect journal. SQLite's
/// EXCLUSIVE locking mode holds the OS database lock until this object drops,
/// including across the short transactions of the ordinary effect dispatcher.
/// A second Core process cannot acquire the same journal while one owns it.
///
/// This guard is NOT proof that an orphaned OS worker has stopped. It refuses
/// new admissions when startup sees unresolved prior execution, until an
/// independent verified kernel/SCM/cgroup custodian reconciles that lifecycle.
pub struct ExclusiveIpcEffectJournal {
    journal: EffectJournal,
    startup_executing: u64,
    startup_uncertain: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpcRestartDisposition {
    Ready,
    NeedsCustody { executing: u64, uncertain: u64 },
}
impl ExclusiveIpcEffectJournal {
    pub fn acquire(path: &Path) -> Result<Self, JournalError> {
        let journal=EffectJournal::open_exclusive_for_ipc(path)?;
        let (executing,uncertain)=journal.ipc_pending_counts()?;
        Ok(Self{journal,startup_executing:executing,startup_uncertain:uncertain})
    }
    /// Read-only status, not a clearance or OS-custody receipt.
    pub fn restart_disposition(&self)->IpcRestartDisposition{
        if self.startup_executing==0 && self.startup_uncertain==0{
            IpcRestartDisposition::Ready
        }else{
            IpcRestartDisposition::NeedsCustody{
                executing:self.startup_executing,
                uncertain:self.startup_uncertain,
            }
        }
    }
    /// The only production IPC journal accessor. Startup is closed to new
    /// effects if an interrupted/uncertain earlier IPC effect needs independent
    /// OS worker reconciliation. Never auto-mark it FAILED or retry it.
    pub fn ready_journal(&self)->Result<&EffectJournal,JournalError>{
        if self.restart_disposition()!=IpcRestartDisposition::Ready{
            return Err(JournalError::IpcRestartNeedsCustody);
        }
        Ok(&self.journal)
    }
}

/// Canonical digest of the exact invocation; callers may bind a reviewed effect without granting authority.
pub fn fingerprint(call: &ToolInvocation) -> String {
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

/// Background witnesses are deliberately distinct from one another. This
/// check binds a host observation to its exact typed successful receipt;
/// it does not itself establish kernel custody or an independent observation.
fn background_witness_matches(call_id:&str,method:&str,receipt:&[u8])->bool{
    let expected=match method{
        "BACKGROUND_QUEUE_DIRECT_V1"=>"type.googleapis.com/lu.v1.ReviewedScanQueued",
        "BACKGROUND_QUEUE_CANCEL_DIRECT_V1"=>"type.googleapis.com/lu.v1.ReviewedScanQueueCancelled",
        "BACKGROUND_JOB_CANCEL_DIRECT_V1"|"BACKGROUND_KERNEL_CANCEL_DIRECT_V1"=>"type.googleapis.com/lu.v1.BoundJobCancelled",
        _=>return true,
    };
    crate::wire::decode_reply(receipt).is_ok_and(|r|r.call_id==call_id&&r.output_type_url==expected&&
        matches!(r.outcome,Some(crate::wire::proto::tool_reply::Outcome::Output(ref bytes)) if !bytes.is_empty()))
}

impl EffectJournal {
    /// Open an existing DB or create a new one. The caller controls the path
    /// and must enforce a single active owner before startup recovery.
    pub fn open(path: &Path) -> Result<Self, JournalError> {
        Self::open_impl(path,false)
    }

    /// Durable single-owner journal for authenticated DLC execution. A live
    /// competing Lu Core instance must fail rather than race journal recovery.
    /// SQLITE EXCLUSIVE WAL locking is process-bound and releases on crash;
    /// unlike a create_new PID file it cannot remain falsely owned forever.
    pub fn open_exclusive_for_ipc(path:&Path)->Result<Self,JournalError>{
        Self::open_impl(path,true)
    }

    fn open_impl(path:&Path,exclusive_ipc:bool)->Result<Self,JournalError>{
        let connection = Connection::open(path)?;
        if exclusive_ipc {
            // Set BEFORE first WAL access so SQLite takes an actual OS-wide
            // exclusive lock. A busy timeout makes takeover bounded.
            connection.busy_timeout(Duration::from_millis(150))?;
            connection.pragma_update(None,"locking_mode","EXCLUSIVE")?;
        }
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
        if exclusive_ipc {
            let mode:String=connection.query_row(
                "PRAGMA locking_mode",[],|row|row.get(0))?;
            if mode!="exclusive"{
                return Err(JournalError::IpcRestartNeedsCustody);
            }
            // Acquire/retain the OS lock now, before admitting ANY DLC frame.
            // This must succeed even if the file was previously in WAL mode.
            connection.execute_batch("BEGIN IMMEDIATE; COMMIT;")?;
        }
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    /// Startup-only factual state inspection. A nonempty count is a refusal
    /// to admit a fresh DLC session until independent worker-custody review.
    pub fn ipc_pending_counts(&self)->Result<(u64,u64),JournalError>{
        let connection=self.connection.lock().map_err(|_|JournalError::Poisoned)?;
        let executing:i64=connection.query_row(
            "SELECT COUNT(*) FROM lu_effects WHERE state='EXECUTING'",
            [],|row|row.get(0))?;
        let uncertain:i64=connection.query_row(
            "SELECT COUNT(*) FROM lu_effects WHERE state='UNCERTAIN'",
            [],|row|row.get(0))?;
        Ok((executing.try_into().map_err(|_|JournalError::InvalidStoredState)?,
            uncertain.try_into().map_err(|_|JournalError::InvalidStoredState)?))
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
            "MP4_BOXES_DIRECT_V1"|"MP4_UNSUPPORTED_V1"|"FILE_TRANSFER_DIRECT_V1"|"FILE_CUSTODY_DIRECT_V1"|"ARCHIVE_TREE_DIRECT_V1"|"OFFICE_CREATE_DIRECT_V1"|"OFFICE_REVISION_DIRECT_V1"|"TAR_CREATE_DIRECT_V1"|"ARCHIVE_MEMBER_DIRECT_V1"|"BACKGROUND_JOB_CANCEL_DIRECT_V1"|"BACKGROUND_KERNEL_CANCEL_DIRECT_V1"|"BACKGROUND_QUEUE_CANCEL_DIRECT_V1"|"BACKGROUND_QUEUE_DIRECT_V1"){
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
            "SELECT state,receipt,tool_id FROM lu_effects WHERE call_id=?1",
            params![call_id],
            |row|Ok((row.get::<_,String>(0)?,row.get::<_,Option<Vec<u8>>>(1)?,row.get::<_,String>(2)?))
        ).optional()?;
        let Some((state,bytes,tool_id))=existing else{return Err(JournalError::InvalidTransition)};
        if state!="APPLIED"||bytes.as_deref()!=Some(receipt)||
            !background_witness_matches(call_id,method,receipt)||
            (method=="FILE_TRANSFER_DIRECT_V1"&&!matches!(tool_id.as_str(),"lu.files.transfer.copy_new"|"lu.files.transfer.move_new"))||
            (method=="FILE_CUSTODY_DIRECT_V1"&&!matches!(tool_id.as_str(),"lu.files.reversible.trash"|"lu.files.reversible.restore"))||
            (method=="ARCHIVE_TREE_DIRECT_V1"&&tool_id!="lu.archive.tree.extract")||
            (method=="TAR_CREATE_DIRECT_V1"&&tool_id!="lu.archive.linux.create")||
            (method=="ARCHIVE_MEMBER_DIRECT_V1"&&!matches!(tool_id.as_str(),"lu.archive.zip.extract"|"lu.archive.iso.extract"|"lu.archive.linux.extract"))||
            (matches!(method,"BACKGROUND_JOB_CANCEL_DIRECT_V1"|"BACKGROUND_KERNEL_CANCEL_DIRECT_V1"|"BACKGROUND_QUEUE_CANCEL_DIRECT_V1")&&tool_id!="lu.bgcancel.run")||
            (method=="BACKGROUND_QUEUE_DIRECT_V1"&&tool_id!="lu.bgstart.run")||
            (method=="OFFICE_CREATE_DIRECT_V1"&&!matches!(tool_id.as_str(),"lu.docx.create"|"lu.pptx.create"|"lu.spreadsheet.create"))||
            (method=="OFFICE_REVISION_DIRECT_V1"&&!matches!(tool_id.as_str(),"lu.office.revisions.create"|"lu.office.batch.revision.create")){
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

    /// MEMORY-LOOP-03B: historical, read-only second-witness lookup.
    /// A task checkpoint and APPLIED receipt alone are insufficient.
    /// Reopen the original journal, compare the actual receipt bytes,
    /// source-oracle method for the exact reviewed tool, and independently
    /// observed digest. This is NOT a live source check or OS custody proof.
    pub fn independent_verified_receipt(
        &self,call_id:&str,tool_id:&str,receipt_sha256:&str
    )->Result<bool,JournalError>{
        let valid_sha=|s:&str|s.len()==64&&s.bytes().all(|b|
            b.is_ascii_digit()||(b'a'..=b'f').contains(&b));
        if call_id.is_empty()||call_id.len()>128||
           !valid_sha(receipt_sha256){return Ok(false)}
        let conn=self.connection.lock().map_err(|_|JournalError::Poisoned)?;
        let row:Option<(String,String,Option<Vec<u8>>,String,String,String,Option<String>)>=
            conn.query_row(
                "SELECT e.tool_id,e.state,e.receipt,o.verdict,
                        o.method,o.receipt_sha256,o.observed_sha256
                 FROM lu_effects e
                 JOIN lu_independent_outcomes o ON o.call_id=e.call_id
                 WHERE e.call_id=?1",
                [call_id],|r|Ok((
                    r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,
                    r.get(4)?,r.get(5)?,r.get(6)?
                ))
            ).optional()?;
        let Some((stored_tool,state,bytes,verdict,method,observed_receipt,observed_source))=
            row else{return Ok(false)};
        let expected_method=match tool_id{
            "lu.reviewed.git.head"=>method=="GIT_HEAD_DIRECT_V1",
            "lu.reviewed.git.branch"=>method=="GIT_BRANCH_DIRECT_V1",
            "lu.reviewed.git.status"=>method=="GIT_STATUS_INDEX_TREE_V1",
            "lu.reviewed.git.history"=>
                matches!(method.as_str(),
                    "GIT_HISTORY_LOOSE_DAG_V1"|"GIT_HISTORY_OBJECT_DAG_V2"),
            "lu.reviewed.media.facts"=>
                matches!(method.as_str(),"WAV_PCM_DIRECT_V1"|"MP4_BOXES_DIRECT_V1"),
            "lu.files.transfer.copy_new"|"lu.files.transfer.move_new"=>method=="FILE_TRANSFER_DIRECT_V1",
            "lu.files.reversible.trash"|"lu.files.reversible.restore"=>method=="FILE_CUSTODY_DIRECT_V1",
            "lu.archive.tree.extract"=>method=="ARCHIVE_TREE_DIRECT_V1",
            "lu.archive.linux.create"=>method=="TAR_CREATE_DIRECT_V1",
            "lu.archive.zip.extract"|"lu.archive.iso.extract"|"lu.archive.linux.extract"=>method=="ARCHIVE_MEMBER_DIRECT_V1",
            "lu.docx.create"|"lu.pptx.create"|"lu.spreadsheet.create"=>method=="OFFICE_CREATE_DIRECT_V1",
            "lu.office.revisions.create"|"lu.office.batch.revision.create"=>method=="OFFICE_REVISION_DIRECT_V1",
            "lu.bgstart.run"=>method=="BACKGROUND_QUEUE_DIRECT_V1",
            "lu.bgcancel.run"=>matches!(method.as_str(),"BACKGROUND_JOB_CANCEL_DIRECT_V1"|"BACKGROUND_KERNEL_CANCEL_DIRECT_V1"|"BACKGROUND_QUEUE_CANCEL_DIRECT_V1"),
            _=>false,
        };
        let Some(receipt)=bytes else{return Ok(false)};
        if receipt.is_empty()||receipt.len()>8*1024*1024{
            return Ok(false)
        }
        let actual=format!("{:x}",Sha256::digest(&receipt));
        let source_is_valid=observed_source.as_deref()
            .is_some_and(valid_sha);
        Ok(stored_tool==tool_id&&state=="APPLIED"&&verdict=="VERIFIED"
            &&expected_method&&source_is_valid&&background_witness_matches(call_id,&method,&receipt)
            &&actual==receipt_sha256&&actual==observed_receipt)
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
    fn historical_second_witness_requires_matching_tool_method_and_unchanged_receipt(){
        let root=std::env::temp_dir().join(format!("lu-03b-witness-{}-{}",
            std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH)
                .unwrap().as_nanos()));
        fs::create_dir_all(&root).unwrap();
        let path=root.join("journal.sqlite");
        let call=ToolInvocation{
            call_id:"reviewed-03b-gh".into(),tool_id:"lu.reviewed.git.head".into(),
            workspace_id:None,
            input:ProtoPayload{type_url:"lu.test".into(),bytes:vec![2]},
        };
        let receipt=b"real-original-receipt";
        let receipt_sha=format!("{:x}",Sha256::digest(receipt));
        let source_sha=format!("{:x}",Sha256::digest(b"independent-source"));
        {
            let db=EffectJournal::open(&path).unwrap();
            db.prepare(&call).unwrap();
            assert!(!db.independent_verified_receipt(
                &call.call_id,&call.tool_id,&receipt_sha).unwrap());
            db.mark_executing(&call.call_id).unwrap();
            db.mark_applied(&call.call_id,receipt).unwrap();
            assert!(!db.independent_verified_receipt(
                &call.call_id,&call.tool_id,&receipt_sha).unwrap());
            db.record_independent_outcome(
                &call.call_id,receipt,IndependentOutcomeVerdict::Verified,
                "GIT_HEAD_DIRECT_V1",Some(&source_sha)).unwrap();
            assert!(db.independent_verified_receipt(
                &call.call_id,&call.tool_id,&receipt_sha).unwrap());
            assert!(!db.independent_verified_receipt(
                &call.call_id,"lu.reviewed.git.branch",&receipt_sha).unwrap());
            assert!(!db.independent_verified_receipt(
                &call.call_id,&call.tool_id,&"0".repeat(64)).unwrap());
        }
        // All results are re-read from SQLite. A changed oracle digest,
        // mismatched method or damaged original receipt invalidates recall.
        let db=EffectJournal::open(&path).unwrap();
        let conn=Connection::open(&path).unwrap();
        conn.execute(
            "UPDATE lu_independent_outcomes SET receipt_sha256=?1 WHERE call_id=?2",
            params!["0".repeat(64),call.call_id]
        ).unwrap();
        assert!(!db.independent_verified_receipt(
            &call.call_id,&call.tool_id,&receipt_sha).unwrap());
        conn.execute(
            "UPDATE lu_independent_outcomes SET receipt_sha256=?1,method='GIT_BRANCH_DIRECT_V1'
             WHERE call_id=?2",params![receipt_sha,call.call_id]
        ).unwrap();
        assert!(!db.independent_verified_receipt(
            &call.call_id,&call.tool_id,&receipt_sha).unwrap());
        drop(conn);drop(db);
        fs::remove_dir_all(root).unwrap();
    }
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

#[cfg(test)]
mod ipc_exclusive_owner_tests {
    use super::*;
    use crate::contracts::{ProtoPayload,ToolInvocation};
    use std::{fs,time::{SystemTime,UNIX_EPOCH}};
    fn db()->(std::path::PathBuf,std::path::PathBuf){
        let root=std::env::temp_dir().join(format!(
            "lu-ipc-exclusive-{}-{}",std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir_all(&root).unwrap();
        let path=root.join("ipc-effects.sqlite");
        (root,path)
    }
    #[test]
    fn actual_sqlite_owner_lock_rejects_concurrent_core_and_releases_after_drop(){
        let (root,path)=db();
        let owner=ExclusiveIpcEffectJournal::acquire(&path).unwrap();
        assert_eq!(owner.restart_disposition(),IpcRestartDisposition::Ready);
        assert!(owner.ready_journal().is_ok());
        assert!(ExclusiveIpcEffectJournal::acquire(&path).is_err(),
            "two live Lu Core owners must not hold the same IPC journal");
        drop(owner);
        let recovered=ExclusiveIpcEffectJournal::acquire(&path).unwrap();
        assert_eq!(recovered.restart_disposition(),IpcRestartDisposition::Ready);
        drop(recovered);
        let _=fs::remove_dir_all(root);
    }
    #[test]
    fn interrupted_and_uncertain_effects_remain_blocked_after_new_owner(){
        let (root,path)=db();
        let call=ToolInvocation{
            call_id:"ipc-restart-original-call".into(),
            tool_id:"lu.test.effect".into(),workspace_id:Some("scope-a".into()),
            input:ProtoPayload{type_url:"lu.test.Input".into(),bytes:vec![1]},
        };
        {
            let first=ExclusiveIpcEffectJournal::acquire(&path).unwrap();
            let j=first.ready_journal().unwrap();
            j.prepare(&call).unwrap();
            j.mark_executing(&call.call_id).unwrap();
        }
        let second=ExclusiveIpcEffectJournal::acquire(&path).unwrap();
        assert_eq!(second.restart_disposition(),
            IpcRestartDisposition::NeedsCustody{executing:1,uncertain:0});
        assert!(matches!(second.ready_journal(),
            Err(JournalError::IpcRestartNeedsCustody)));
        drop(second);
        // This test models the original host having definitively stopped,
        // NOT a production OS-custody proof. Production must not auto-call
        // the raw journal reconciliation API merely from a PID or timestamp.
        let inspected=EffectJournal::open(&path).unwrap();
        assert_eq!(inspected.get(&call.call_id).unwrap().unwrap().state,
            EffectState::Executing);
        assert_eq!(inspected.reconcile_interrupted().unwrap(),1);
        drop(inspected);
        let third=ExclusiveIpcEffectJournal::acquire(&path).unwrap();
        assert_eq!(third.restart_disposition(),
            IpcRestartDisposition::NeedsCustody{executing:0,uncertain:1});
        assert!(matches!(third.ready_journal(),
            Err(JournalError::IpcRestartNeedsCustody)));
        drop(third);
        let _=fs::remove_dir_all(root);
    }
}
