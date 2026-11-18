//! WRITE-01C: host-only durable, *non-authorizing* write approval records.
//! Approval does not grant execution, nor can it be renewed after restart.
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use std::{path::Path, sync::Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteDecision { Pending, Selected, Approved, Denied, Interrupted }
impl WriteDecision {
    fn value(self) -> &'static str {
        match self {
            Self::Pending=>"PENDING", Self::Selected=>"SELECTED",
            Self::Approved=>"APPROVED", Self::Denied=>"DENIED",
            Self::Interrupted=>"INTERRUPTED",
        }
    }
    fn from_db(value: &str) -> Option<Self> {
        match value {
            "PENDING"=>Some(Self::Pending), "SELECTED"=>Some(Self::Selected),
            "APPROVED"=>Some(Self::Approved), "DENIED"=>Some(Self::Denied),
            "INTERRUPTED"=>Some(Self::Interrupted), _=>None,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteLedgerError { Invalid, Missing, Conflict, Transition, Poisoned, Database(String) }
impl From<rusqlite::Error> for WriteLedgerError {
    fn from(e: rusqlite::Error)->Self { Self::Database(e.to_string()) }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteDecisionRecord {
    pub proposal_id: String,
    pub chat_id: String,
    pub generation_id: String,
    pub file_name: String,
    pub bytes: u32,
    pub sha256: String,
    pub decision: WriteDecision,
    pub selected_target: Option<String>,
    pub effect_call_id: Option<String>,
}
pub struct WriteApprovalLedger { db: Mutex<Connection> }
fn valid_id(s: &str) -> bool {
    !s.is_empty() && s.len() <= 128
        && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
fn valid_name(s: &str) -> bool {
    !s.is_empty() && s.len() <= 255 && s != "." && s != ".."
        && !s.starts_with(".lu-stage-")
        && !s.chars().any(|c| matches!(c, '/' | '\\' | ':' | '\0'))
}
fn valid_digest(s: &str) -> bool {
    s.len()==64 && s.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
impl WriteApprovalLedger {
    pub fn open(path: &Path) -> Result<Self,WriteLedgerError> {
        let db = Connection::open(path)?;
        db.pragma_update(None,"journal_mode","WAL")?;
        db.pragma_update(None,"synchronous","FULL")?;
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS lu_write_approvals(
                proposal_id TEXT PRIMARY KEY,
                chat_id TEXT NOT NULL,
                generation_id TEXT NOT NULL,
                file_name TEXT NOT NULL,
                bytes INTEGER NOT NULL CHECK(bytes BETWEEN 0 AND 16384),
                sha256 TEXT NOT NULL,
                decision TEXT NOT NULL CHECK(decision IN
                   ('PENDING','SELECTED','APPROVED','DENIED','INTERRUPTED')),
                selected_target TEXT,
                effect_call_id TEXT UNIQUE,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
             );
             CREATE INDEX IF NOT EXISTS lu_write_approval_state
                 ON lu_write_approvals(decision);
             CREATE TRIGGER IF NOT EXISTS lu_write_no_delete
                 BEFORE DELETE ON lu_write_approvals BEGIN
                 SELECT RAISE(ABORT,'write approval audit rows cannot be deleted');
             END;"
        )?;
        Ok(Self{db:Mutex::new(db)})
    }
    pub fn stage(
        &self,id:&str,chat:&str,generation:&str,name:&str,bytes:usize,sha:&str
    )->Result<(),WriteLedgerError>{
        if !valid_id(id)||!valid_id(chat)||!valid_id(generation)||!valid_name(name)
            ||bytes>16*1024||!valid_digest(sha) { return Err(WriteLedgerError::Invalid); }
        let db=self.db.lock().map_err(|_|WriteLedgerError::Poisoned)?;
        db.execute(
            "INSERT INTO lu_write_approvals
             (proposal_id,chat_id,generation_id,file_name,bytes,sha256,decision)
             VALUES (?1,?2,?3,?4,?5,?6,'PENDING')",
            params![id,chat,generation,name,bytes as i64,sha]
        )?;
        Ok(())
    }
    pub fn get(&self,id:&str)->Result<Option<WriteDecisionRecord>,WriteLedgerError>{
        if !valid_id(id){return Err(WriteLedgerError::Invalid);}
        let db=self.db.lock().map_err(|_|WriteLedgerError::Poisoned)?;
        let raw:Option<(String,String,String,String,i64,String,String,Option<String>,Option<String>)>=db.query_row(
            "SELECT proposal_id,chat_id,generation_id,file_name,bytes,sha256,decision,
                    selected_target,effect_call_id FROM lu_write_approvals WHERE proposal_id=?1",
            [id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,
                         r.get(5)?,r.get(6)?,r.get(7)?,r.get(8)?))
        ).optional()?;
        raw.map(|(proposal_id,chat_id,generation_id,file_name,bytes,sha256,decision,selected_target,effect_call_id)|{
            if !(0..=16384).contains(&bytes)||!valid_digest(&sha256)||!valid_name(&file_name){
                return Err(WriteLedgerError::Invalid);
            }
            Ok(WriteDecisionRecord{proposal_id,chat_id,generation_id,file_name,
                bytes:bytes as u32,sha256,decision:WriteDecision::from_db(&decision)
                    .ok_or(WriteLedgerError::Invalid)?,selected_target,effect_call_id})
        }).transpose()
    }
    pub fn choose_target(
        &self,id:&str,chat:&str,generation:&str,exact_target:&str
    )->Result<(),WriteLedgerError>{
        if !valid_id(id)||!valid_id(chat)||!valid_id(generation)
            ||exact_target.is_empty()||exact_target.len()>4096{
            return Err(WriteLedgerError::Invalid);
        }
        let db=self.db.lock().map_err(|_|WriteLedgerError::Poisoned)?;
        if db.execute(
            "UPDATE lu_write_approvals SET decision='SELECTED',selected_target=?1,
              updated_at=CURRENT_TIMESTAMP
              WHERE proposal_id=?2 AND chat_id=?3 AND generation_id=?4
              AND decision IN ('PENDING','SELECTED')",
            params![exact_target,id,chat,generation]
        )?!=1 {return Err(WriteLedgerError::Transition);}
        Ok(())
    }
    pub fn approve(
        &self,id:&str,chat:&str,generation:&str,name:&str,bytes:usize,sha:&str,
        exact_target:&str,effect_call_id:&str
    )->Result<(),WriteLedgerError>{
        if !valid_id(id)||!valid_id(chat)||!valid_id(generation)||!valid_id(effect_call_id)
            ||!valid_name(name)||bytes>16384||!valid_digest(sha) {
            return Err(WriteLedgerError::Invalid);
        }
        let mut db=self.db.lock().map_err(|_|WriteLedgerError::Poisoned)?;
        let tx=db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed=tx.execute(
            "UPDATE lu_write_approvals SET decision='APPROVED',effect_call_id=?1,
                 updated_at=CURRENT_TIMESTAMP
             WHERE proposal_id=?2 AND chat_id=?3 AND generation_id=?4
                 AND file_name=?5 AND bytes=?6 AND sha256=?7
                 AND selected_target=?8 AND decision='SELECTED'
                 AND effect_call_id IS NULL",
            params![effect_call_id,id,chat,generation,name,bytes as i64,sha,exact_target]
        )?;
        if changed!=1{return Err(WriteLedgerError::Transition);}
        tx.commit()?;
        Ok(())
    }
    pub fn deny(&self,id:&str,chat:&str)->Result<(),WriteLedgerError>{
        if !valid_id(id)||!valid_id(chat){return Err(WriteLedgerError::Invalid);}
        let db=self.db.lock().map_err(|_|WriteLedgerError::Poisoned)?;
        if db.execute(
            "UPDATE lu_write_approvals SET decision='DENIED',updated_at=CURRENT_TIMESTAMP
             WHERE proposal_id=?1 AND chat_id=?2 AND decision IN ('PENDING','SELECTED')",
            params![id,chat]
        )?!=1{return Err(WriteLedgerError::Transition);}
        Ok(())
    }
    /// Called ONLY after exclusive process ownership is acquired.
    /// No prior model content survives in the pending executor queue.
    pub fn reconcile_pending(&self)->Result<usize,WriteLedgerError>{
        let db=self.db.lock().map_err(|_|WriteLedgerError::Poisoned)?;
        Ok(db.execute(
            "UPDATE lu_write_approvals SET decision='INTERRUPTED',updated_at=CURRENT_TIMESTAMP
             WHERE decision IN ('PENDING','SELECTED')",[]
        )?)
    }
    pub fn recent_for_chat(&self,chat:&str)->Result<Vec<WriteDecisionRecord>,WriteLedgerError>{
        if !valid_id(chat){return Err(WriteLedgerError::Invalid);}
        let db=self.db.lock().map_err(|_|WriteLedgerError::Poisoned)?;
        let mut statement=db.prepare(
            "SELECT proposal_id FROM lu_write_approvals WHERE chat_id=?1
             AND decision IN ('APPROVED','INTERRUPTED') ORDER BY rowid DESC LIMIT 40"
        )?;
        let ids=statement.query_map([chat],|r|r.get::<_,String>(0))?
            .collect::<Result<Vec<_>,_>>()?;
        drop(statement);drop(db);
        ids.iter().map(|id|self.get(id)?.ok_or(WriteLedgerError::Missing)).collect()
    }
}
