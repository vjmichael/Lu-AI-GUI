//! WRITE-01E.4B: independently approved restoration evidence.
//! Merely finding an old edit/backup is not consent to restore it. Review
//! state is durable and single-use; pending review is interrupted on restart.
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use std::{path::Path, sync::Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RollbackDecision { Reviewed, Approved, Denied, Interrupted }
impl RollbackDecision {
    fn as_str(self)->&'static str {
        match self { Self::Reviewed=>"REVIEWED",Self::Approved=>"APPROVED",
            Self::Denied=>"DENIED",Self::Interrupted=>"INTERRUPTED" }
    }
    fn parse(v:&str)->Option<Self> {
        match v { "REVIEWED"=>Some(Self::Reviewed),"APPROVED"=>Some(Self::Approved),
            "DENIED"=>Some(Self::Denied),"INTERRUPTED"=>Some(Self::Interrupted),_=>None }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RollbackLedgerError { Invalid, Transition, Missing, Db(String) }
impl From<rusqlite::Error> for RollbackLedgerError {
    fn from(e:rusqlite::Error)->Self { Self::Db(e.to_string()) }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollbackRecord {
    pub review_id:String,
    pub chat_id:String,
    pub original_proposal_id:String,
    pub original_edit_id:String,
    pub target:String,
    pub filename:String,
    pub expected_current_sha256:String,
    pub restored_sha256:String,
    pub restore_bytes:u64,
    pub decision:RollbackDecision,
    pub rollback_effect_id:Option<String>,
}
pub struct RollbackApprovalLedger { db:Mutex<Connection> }
fn id(s:&str)->bool {
    !s.is_empty() && s.len()<=128 && s.bytes()
        .all(|v|v.is_ascii_alphanumeric()||v==b'-'||v==b'_')
}
fn digest(s:&str)->bool {
    s.len()==64 && s.bytes().all(|b|b.is_ascii_digit()||(b'a'..=b'f').contains(&b))
}
fn simple_name(s:&str)->bool {
    !s.is_empty() && s.len()<=200 && s!="." && s!=".." &&
    !s.starts_with(".lu-") && !s.ends_with('.') && !s.ends_with(' ') &&
    !s.chars().any(|c|c.is_control()||matches!(c,'/'|'\\'|':'))
}
fn valid(r:&RollbackRecord)->bool {
    id(&r.review_id)&&id(&r.chat_id)&&id(&r.original_proposal_id)&&
    id(&r.original_edit_id)&&simple_name(&r.filename)&&
    !r.target.is_empty()&&r.target.len()<=4096&&
    std::path::Path::new(&r.target).file_name().and_then(|n|n.to_str())==
        Some(r.filename.as_str()) &&
    digest(&r.expected_current_sha256)&&digest(&r.restored_sha256)&&
    r.restore_bytes<=64*1024&&r.rollback_effect_id.as_deref().is_none_or(id)
}
impl RollbackApprovalLedger {
    pub fn open(path:&Path)->Result<Self,RollbackLedgerError> {
        let db=Connection::open(path)?;
        db.pragma_update(None,"journal_mode","WAL")?;
        db.pragma_update(None,"synchronous","FULL")?;
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS lu_rollback_reviews(
              review_id TEXT PRIMARY KEY,
              chat_id TEXT NOT NULL,
              original_proposal_id TEXT NOT NULL,
              original_edit_id TEXT NOT NULL,
              target TEXT NOT NULL,
              filename TEXT NOT NULL,
              current_sha TEXT NOT NULL,
              restore_sha TEXT NOT NULL,
              restore_bytes INTEGER NOT NULL CHECK(restore_bytes BETWEEN 0 AND 65536),
              decision TEXT NOT NULL CHECK(decision IN ('REVIEWED','APPROVED','DENIED','INTERRUPTED')),
              rollback_effect_id TEXT UNIQUE,
              created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            CREATE INDEX IF NOT EXISTS lu_rollback_reviews_by_chat
             ON lu_rollback_reviews(chat_id,decision);
            CREATE TRIGGER IF NOT EXISTS lu_rollback_review_no_delete
             BEFORE DELETE ON lu_rollback_reviews
             BEGIN SELECT RAISE(ABORT,'rollback approval history cannot be deleted'); END;"
        )?;
        Ok(Self { db:Mutex::new(db) })
    }
    pub fn stage(&self,r:&RollbackRecord)->Result<(),RollbackLedgerError> {
        if !valid(r)||r.decision!=RollbackDecision::Reviewed||
            r.rollback_effect_id.is_some() {return Err(RollbackLedgerError::Invalid);}
        let db=self.db.lock().map_err(|_|RollbackLedgerError::Db("mutex poisoned".into()))?;
        db.execute(
            "INSERT INTO lu_rollback_reviews(
             review_id,chat_id,original_proposal_id,original_edit_id,target,
             filename,current_sha,restore_sha,restore_bytes,decision)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,'REVIEWED')",
            params![r.review_id,r.chat_id,r.original_proposal_id,r.original_edit_id,
              r.target,r.filename,r.expected_current_sha256,r.restored_sha256,
              r.restore_bytes as i64]
        )?;
        Ok(())
    }
    pub fn approve(&self,r:&RollbackRecord,effect_id:&str)->Result<(),RollbackLedgerError>{
        if !valid(r)||r.decision!=RollbackDecision::Reviewed||
            r.rollback_effect_id.is_some()||!id(effect_id) {return Err(RollbackLedgerError::Invalid);}
        let mut db=self.db.lock().map_err(|_|RollbackLedgerError::Db("mutex poisoned".into()))?;
        let tx=db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let count=tx.execute(
            "UPDATE lu_rollback_reviews SET decision='APPROVED',rollback_effect_id=?1
             WHERE review_id=?2 AND chat_id=?3 AND original_proposal_id=?4
               AND original_edit_id=?5 AND target=?6 AND filename=?7
               AND current_sha=?8 AND restore_sha=?9 AND restore_bytes=?10
               AND decision='REVIEWED' AND rollback_effect_id IS NULL",
            params![effect_id,r.review_id,r.chat_id,r.original_proposal_id,
            r.original_edit_id,r.target,r.filename,r.expected_current_sha256,
            r.restored_sha256,r.restore_bytes as i64]
        )?;
        if count!=1 {return Err(RollbackLedgerError::Transition);}
        tx.commit()?;
        Ok(())
    }
    pub fn deny(&self,review_id:&str,chat_id:&str)->Result<(),RollbackLedgerError>{
        if !id(review_id)||!id(chat_id) {return Err(RollbackLedgerError::Invalid);}
        let db=self.db.lock().map_err(|_|RollbackLedgerError::Db("mutex poisoned".into()))?;
        let count=db.execute(
            "UPDATE lu_rollback_reviews SET decision='DENIED'
             WHERE review_id=?1 AND chat_id=?2 AND decision='REVIEWED'",
            params![review_id,chat_id]
        )?;
        if count!=1 {return Err(RollbackLedgerError::Transition);}
        Ok(())
    }
    pub fn reconcile_pending(&self)->Result<usize,RollbackLedgerError>{
        let db=self.db.lock().map_err(|_|RollbackLedgerError::Db("mutex poisoned".into()))?;
        db.execute(
            "UPDATE lu_rollback_reviews SET decision='INTERRUPTED'
             WHERE decision='REVIEWED'",[]
        ).map_err(Into::into)
    }
    pub fn get(&self,review_id:&str)->Result<Option<RollbackRecord>,RollbackLedgerError> {
        if !id(review_id) {return Err(RollbackLedgerError::Invalid);}
        let db=self.db.lock().map_err(|_|RollbackLedgerError::Db("mutex poisoned".into()))?;
        let record=db.query_row(
            "SELECT review_id,chat_id,original_proposal_id,original_edit_id,target,
              filename,current_sha,restore_sha,restore_bytes,decision,rollback_effect_id
             FROM lu_rollback_reviews WHERE review_id=?1",
            [review_id],|row| Ok((
                row.get::<_,String>(0)?,row.get::<_,String>(1)?,
                row.get::<_,String>(2)?,row.get::<_,String>(3)?,
                row.get::<_,String>(4)?,row.get::<_,String>(5)?,
                row.get::<_,String>(6)?,row.get::<_,String>(7)?,
                row.get::<_,i64>(8)?,row.get::<_,String>(9)?,
                row.get::<_,Option<String>>(10)?
            ))
        ).optional()?;
        record.map(|(review_id,chat_id,original_proposal_id,original_edit_id,target,
            filename,expected_current_sha256,restored_sha256,restore_bytes,decision,
            rollback_effect_id)|{
            let decision=RollbackDecision::parse(&decision)
                .ok_or(RollbackLedgerError::Invalid)?;
            if !(0..=65536).contains(&restore_bytes) {return Err(RollbackLedgerError::Invalid);}
            let r=RollbackRecord{review_id,chat_id,original_proposal_id,original_edit_id,
                target,filename,expected_current_sha256,restored_sha256,
                restore_bytes:restore_bytes as u64,decision,rollback_effect_id};
            if !valid(&r) || (r.decision==RollbackDecision::Approved && r.rollback_effect_id.is_none()) {
                return Err(RollbackLedgerError::Invalid);
            }
            Ok(r)
        }).transpose()
    }
    pub fn recent_for_chat(&self,chat_id:&str)->Result<Vec<RollbackRecord>,RollbackLedgerError>{
        if !id(chat_id){return Err(RollbackLedgerError::Invalid);}
        let db=self.db.lock().map_err(|_|RollbackLedgerError::Db("mutex poisoned".into()))?;
        let mut stmt=db.prepare(
            "SELECT review_id FROM lu_rollback_reviews WHERE chat_id=?1
             AND decision IN ('APPROVED','INTERRUPTED') ORDER BY rowid DESC LIMIT 40"
        )?;
        let ids=stmt.query_map([chat_id],|r|r.get::<_,String>(0))?
            .collect::<Result<Vec<_>,_>>()?;
        drop(stmt);drop(db);
        ids.iter().map(|s|self.get(s)?.ok_or(RollbackLedgerError::Missing)).collect()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs,sync::atomic::{AtomicU64,Ordering}};
    static NEXT:AtomicU64=AtomicU64::new(0);
    fn path()->std::path::PathBuf {
        std::env::temp_dir().join(format!("lu-rollback-{}-{}.sqlite",
            std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)))
    }
    const OLD:&str="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const NEW:&str="bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    fn record(id:&str)->RollbackRecord {
        RollbackRecord{review_id:id.into(),chat_id:"chat-one".into(),
            original_proposal_id:"pe-001".into(),
            original_edit_id:"g-edited-first-00001".into(),
            target:"/user/Documents/notes.md".into(),filename:"notes.md".into(),
            expected_current_sha256:NEW.into(),restored_sha256:OLD.into(),
            restore_bytes:11,decision:RollbackDecision::Reviewed,rollback_effect_id:None}
    }
    #[test]
    fn reviewed_rollback_is_single_use_exact_and_durable() {
        let path=path();
        let ledger=RollbackApprovalLedger::open(&path).unwrap();
        let snapshot=record("rb-approved-001");
        ledger.stage(&snapshot).unwrap();
        assert!(ledger.approve(&record("rb-different"),"g-restore-00001").is_err());
        assert!(ledger.approve(&RollbackRecord{
            restore_bytes:10,..snapshot.clone()},"g-restore-00001").is_err());
        ledger.approve(&snapshot,"g-restore-00001").unwrap();
        assert!(ledger.approve(&snapshot,"g-restore-00002").is_err());
        assert!(ledger.deny(&snapshot.review_id,&snapshot.chat_id).is_err());
        drop(ledger);
        let reopened=RollbackApprovalLedger::open(&path).unwrap();
        let stored=reopened.get(&snapshot.review_id).unwrap().unwrap();
        assert_eq!(stored.decision,RollbackDecision::Approved);
        assert_eq!(stored.rollback_effect_id.as_deref(),Some("g-restore-00001"));
        assert_eq!(reopened.reconcile_pending().unwrap(),0);
        drop(reopened);fs::remove_file(path).unwrap();
    }
    #[test]
    fn interrupted_rollback_review_never_becomes_effect() {
        let path=path();
        let ledger=RollbackApprovalLedger::open(&path).unwrap();
        let a=record("rb-restart-0001");
        ledger.stage(&a).unwrap();
        drop(ledger);
        let after=RollbackApprovalLedger::open(&path).unwrap();
        assert_eq!(after.reconcile_pending().unwrap(),1);
        assert_eq!(after.get(&a.review_id).unwrap().unwrap().decision,
            RollbackDecision::Interrupted);
        assert!(after.approve(&a,"g-restore-00002").is_err());
        assert_eq!(after.reconcile_pending().unwrap(),0);
        drop(after);fs::remove_file(path).unwrap();
    }
}
