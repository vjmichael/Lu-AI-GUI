//! WRITE-01E.3 durable, host-only edit review evidence.
//! A ledger decision does NOT grant tool authority; a trusted persisted USER
//! turn and a fresh exact-fenced native policy are required at dispatch.
//! Pending/selected reviews die on restart. Approved edits never auto-resume.
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use std::{path::Path, sync::Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditDecision { Pending, Selected, Approved, Denied, Interrupted }
impl EditDecision {
    fn parse(s: &str) -> Option<Self> {
        match s {
            "PENDING"=>Some(Self::Pending), "SELECTED"=>Some(Self::Selected),
            "APPROVED"=>Some(Self::Approved), "DENIED"=>Some(Self::Denied),
            "INTERRUPTED"=>Some(Self::Interrupted), _=>None,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditLedgerError { Invalid, Missing, Conflict, Transition, Unavailable(String) }
impl From<rusqlite::Error> for EditLedgerError {
    fn from(e: rusqlite::Error) -> Self { Self::Unavailable(e.to_string()) }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditDecisionRecord {
    pub proposal_id: String,
    pub chat_id: String,
    pub generation_id: String,
    pub filename: String,
    pub bytes: usize,
    pub replacement_sha256: String,
    pub original_sha256: Option<String>,
    pub review_sha256: Option<String>,
    pub selected_target: Option<String>,
    pub decision: EditDecision,
    pub effect_call_id: Option<String>,
}
pub struct EditApprovalLedger { db: Mutex<Connection> }
fn id(s: &str) -> bool {
    !s.is_empty() && s.len() <= 128 &&
        s.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
}
fn name(s: &str) -> bool {
    !s.is_empty() && s.len() <= 200 && s != "." && s != ".." &&
        !s.starts_with(".lu-") && !s.ends_with('.') && !s.ends_with(' ') &&
        !s.chars().any(|c| c.is_control() || matches!(c, '/'|'\\'|':'))
}
fn digest(s: &str) -> bool {
    s.len()==64 && s.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
impl EditApprovalLedger {
    pub fn open(path: &Path) -> Result<Self, EditLedgerError> {
        let db = Connection::open(path)?;
        db.pragma_update(None, "journal_mode", "WAL")?;
        db.pragma_update(None, "synchronous", "FULL")?;
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS lu_edit_reviews(
                proposal_id TEXT PRIMARY KEY,
                chat_id TEXT NOT NULL,
                generation_id TEXT NOT NULL,
                filename TEXT NOT NULL,
                bytes INTEGER NOT NULL CHECK(bytes BETWEEN 0 AND 16384),
                replacement_sha TEXT NOT NULL,
                original_sha TEXT,
                review_sha TEXT,
                target TEXT,
                decision TEXT NOT NULL CHECK(decision IN
                    ('PENDING','SELECTED','APPROVED','DENIED','INTERRUPTED')),
                effect_call_id TEXT UNIQUE,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            CREATE INDEX IF NOT EXISTS lu_edit_reviews_by_chat
                ON lu_edit_reviews(chat_id,decision);
            CREATE TRIGGER IF NOT EXISTS lu_edit_review_no_delete BEFORE DELETE
                ON lu_edit_reviews BEGIN
                SELECT RAISE(ABORT,'edit review evidence cannot be deleted');
            END;"
        )?;
        Ok(Self { db: Mutex::new(db) })
    }
    pub fn stage(&self, proposal_id: &str, chat: &str, generation: &str,
        filename: &str, bytes: usize, replacement_sha: &str
    ) -> Result<(), EditLedgerError> {
        if !id(proposal_id)||!id(chat)||!id(generation)||!name(filename)||
            bytes>16384||!digest(replacement_sha) { return Err(EditLedgerError::Invalid); }
        let db=self.db.lock().map_err(|_| EditLedgerError::Unavailable("lock poisoned".into()))?;
        db.execute(
            "INSERT INTO lu_edit_reviews(proposal_id,chat_id,generation_id,
                filename,bytes,replacement_sha,decision)
                VALUES(?1,?2,?3,?4,?5,?6,'PENDING')",
            params![proposal_id,chat,generation,filename,bytes as i64,replacement_sha]
        )?;
        Ok(())
    }
    pub fn choose(&self, proposal_id: &str, chat: &str, generation: &str,
        target: &str, original_sha: &str, review_sha: &str
    ) -> Result<(), EditLedgerError> {
        if !id(proposal_id)||!id(chat)||!id(generation)||!digest(original_sha)||
            !digest(review_sha)||target.is_empty()||target.len()>4096 {
            return Err(EditLedgerError::Invalid);
        }
        let db=self.db.lock().map_err(|_|EditLedgerError::Unavailable("lock poisoned".into()))?;
        let changed=db.execute(
            "UPDATE lu_edit_reviews SET target=?1,original_sha=?2,review_sha=?3,
                decision='SELECTED',updated_at=CURRENT_TIMESTAMP
                WHERE proposal_id=?4 AND chat_id=?5 AND generation_id=?6
                  AND decision IN ('PENDING','SELECTED')",
            params![target,original_sha,review_sha,proposal_id,chat,generation]
        )?;
        if changed!=1 { return Err(EditLedgerError::Transition); }
        Ok(())
    }
    pub fn approve(&self, proposal_id:&str, chat:&str, generation:&str,
        filename:&str, bytes:usize, replacement_sha:&str, target:&str,
        original_sha:&str, review_sha:&str, effect_call_id:&str
    ) -> Result<(),EditLedgerError> {
        if !id(proposal_id)||!id(chat)||!id(generation)||!id(effect_call_id)||
            !name(filename)||bytes>16384||!digest(replacement_sha)||
            !digest(original_sha)||!digest(review_sha) {
            return Err(EditLedgerError::Invalid);
        }
        let mut db=self.db.lock().map_err(|_|EditLedgerError::Unavailable("lock poisoned".into()))?;
        let tx=db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed=tx.execute(
            "UPDATE lu_edit_reviews SET decision='APPROVED',effect_call_id=?1,
                updated_at=CURRENT_TIMESTAMP
                WHERE proposal_id=?2 AND chat_id=?3 AND generation_id=?4
                  AND filename=?5 AND bytes=?6 AND replacement_sha=?7
                  AND target=?8 AND original_sha=?9 AND review_sha=?10
                  AND decision='SELECTED' AND effect_call_id IS NULL",
            params![effect_call_id,proposal_id,chat,generation,filename,bytes as i64,
                replacement_sha,target,original_sha,review_sha]
        )?;
        if changed!=1 { return Err(EditLedgerError::Transition); }
        tx.commit()?;
        Ok(())
    }
    pub fn deny(&self, proposal_id:&str, chat:&str) -> Result<(),EditLedgerError> {
        if !id(proposal_id)||!id(chat) { return Err(EditLedgerError::Invalid); }
        let db=self.db.lock().map_err(|_|EditLedgerError::Unavailable("lock poisoned".into()))?;
        let changed=db.execute(
            "UPDATE lu_edit_reviews SET decision='DENIED',updated_at=CURRENT_TIMESTAMP
             WHERE proposal_id=?1 AND chat_id=?2 AND decision IN ('PENDING','SELECTED')",
            params![proposal_id,chat]
        )?;
        if changed!=1 { return Err(EditLedgerError::Transition); }
        Ok(())
    }
    pub fn reconcile_pending(&self) -> Result<usize,EditLedgerError> {
        let db=self.db.lock().map_err(|_|EditLedgerError::Unavailable("lock poisoned".into()))?;
        db.execute(
            "UPDATE lu_edit_reviews SET decision='INTERRUPTED',updated_at=CURRENT_TIMESTAMP
             WHERE decision IN ('PENDING','SELECTED')",[]
        ).map_err(EditLedgerError::from)
    }
    pub fn get(&self, proposal_id:&str) -> Result<Option<EditDecisionRecord>,EditLedgerError> {
        if !id(proposal_id) { return Err(EditLedgerError::Invalid); }
        let db=self.db.lock().map_err(|_|EditLedgerError::Unavailable("lock poisoned".into()))?;
        let row=db.query_row(
            "SELECT proposal_id,chat_id,generation_id,filename,bytes,replacement_sha,
                original_sha,review_sha,target,decision,effect_call_id
                FROM lu_edit_reviews WHERE proposal_id=?1",
            [proposal_id], |r| Ok((
                r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,
                r.get::<_,String>(3)?,r.get::<_,i64>(4)?,r.get::<_,String>(5)?,
                r.get::<_,Option<String>>(6)?,r.get::<_,Option<String>>(7)?,
                r.get::<_,Option<String>>(8)?,r.get::<_,String>(9)?,
                r.get::<_,Option<String>>(10)?,
            ))
        ).optional()?;
        row.map(|(proposal_id,chat_id,generation_id,filename,bytes,replacement_sha,
            original_sha256,review_sha256,selected_target,decision,effect_call_id)|{
            if !id(&proposal_id)||!id(&chat_id)||!id(&generation_id)||!name(&filename)||
                !(0..=16384).contains(&bytes)||!digest(&replacement_sha)||
                original_sha256.as_deref().is_some_and(|s|!digest(s))||
                review_sha256.as_deref().is_some_and(|s|!digest(s))||
                selected_target.as_deref().is_some_and(|s|s.is_empty()||s.len()>4096)||
                effect_call_id.as_deref().is_some_and(|s|!id(s)) {
                return Err(EditLedgerError::Invalid);
            }
            let decision=EditDecision::parse(&decision).ok_or(EditLedgerError::Invalid)?;
            if decision==EditDecision::Approved &&
                (selected_target.is_none() || original_sha256.is_none() ||
                 review_sha256.is_none() || effect_call_id.is_none()) {
                return Err(EditLedgerError::Invalid);
            }
            Ok(EditDecisionRecord {
                proposal_id,chat_id,generation_id,filename,bytes:bytes as usize,
                replacement_sha256:replacement_sha,original_sha256,review_sha256,
                selected_target,decision,effect_call_id,
            })
        }).transpose()
    }
    pub fn recent_for_chat(&self, chat:&str) -> Result<Vec<EditDecisionRecord>,EditLedgerError> {
        if !id(chat) { return Err(EditLedgerError::Invalid); }
        let db=self.db.lock().map_err(|_|EditLedgerError::Unavailable("lock poisoned".into()))?;
        let mut query=db.prepare(
            "SELECT proposal_id FROM lu_edit_reviews WHERE chat_id=?1
              AND decision IN ('APPROVED','INTERRUPTED')
              ORDER BY rowid DESC LIMIT 40"
        )?;
        let ids=query.query_map([chat],|r|r.get::<_,String>(0))?
            .collect::<Result<Vec<_>,_>>()?;
        drop(query);drop(db);
        ids.iter().map(|s|self.get(s)?.ok_or(EditLedgerError::Missing)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs,sync::atomic::{AtomicU64,Ordering}};
    static NEXT:AtomicU64=AtomicU64::new(0);
    fn scratch()->std::path::PathBuf {
        std::env::temp_dir().join(format!("lu-edit-ledger-{}-{}.sqlite",
            std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)))
    }
    const A:&str="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const B:&str="bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const C:&str="cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
    #[test]
    fn selection_approval_is_exact_and_cannot_revive_after_restart() {
        let path=scratch();
        let ledger=EditApprovalLedger::open(&path).unwrap();
        ledger.stage("pe-1","chat-1","gen-1","notes.md",2,B).unwrap();
        assert!(ledger.approve("pe-1","chat-1","gen-1","notes.md",2,B,
            "/Documents/notes.md",A,C,"g-effect-1").is_err());
        ledger.choose("pe-1","chat-1","gen-1","/Documents/notes.md",A,C).unwrap();
        assert!(ledger.approve("pe-1","other-chat","gen-1","notes.md",2,B,
            "/Documents/notes.md",A,C,"g-effect-1").is_err());
        ledger.approve("pe-1","chat-1","gen-1","notes.md",2,B,
            "/Documents/notes.md",A,C,"g-effect-1").unwrap();
        drop(ledger);
        let again=EditApprovalLedger::open(&path).unwrap();
        assert_eq!(again.get("pe-1").unwrap().unwrap().decision,EditDecision::Approved);
        assert_eq!(again.reconcile_pending().unwrap(),0);
        assert!(again.approve("pe-1","chat-1","gen-1","notes.md",2,B,
            "/Documents/notes.md",A,C,"g-effect-1").is_err());
        drop(again); fs::remove_file(path).unwrap();
    }
    #[test]
    fn unapproved_selection_is_interrupted_and_never_gains_authority() {
        let path=scratch();
        let ledger=EditApprovalLedger::open(&path).unwrap();
        ledger.stage("pe-2","chat-1","gen-1","notes.txt",1,B).unwrap();
        ledger.choose("pe-2","chat-1","gen-1","/Documents/notes.txt",A,C).unwrap();
        drop(ledger);
        let again=EditApprovalLedger::open(&path).unwrap();
        assert_eq!(again.reconcile_pending().unwrap(),1);
        assert_eq!(again.get("pe-2").unwrap().unwrap().decision,EditDecision::Interrupted);
        assert!(again.approve("pe-2","chat-1","gen-1","notes.txt",1,B,
            "/Documents/notes.txt",A,C,"g-effect-2").is_err());
        drop(again); fs::remove_file(path).unwrap();
    }
}