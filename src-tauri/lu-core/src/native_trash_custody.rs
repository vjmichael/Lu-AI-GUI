//! WAVE2-DESKTOP: durable custody for reversible, same-directory file trash.
//! The model cannot create, alter or select a custody entry. Every record is
//! minted by the trusted host after native-picking and human authorization.
//! An effect receipt is checked against the independent durable EffectJournal.
//! INTERRUPTED/UNCERTAIN state is read-only; NEVER automatically re-run a write.
use std::{path::Path,sync::Mutex};
use rusqlite::{params,Connection,OptionalExtension,TransactionBehavior};
use prost::Message;
use crate::{
    journal::{EffectJournal,EffectState},
    native_file_transfer::TrashReceipt,
    native_file_trash_tool::{TRASH_ID,RESTORE_ID,TRASH_OUTPUT,RESTORE_OUTPUT},
    wire::proto,
};
const MAX_BYTES:u64=32*1024*1024;
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum CustodyState{Pending,Trashed,Restoring,Restored,Uncertain}
impl CustodyState{
    pub fn as_str(self)->&'static str{
        match self{
            Self::Pending=>"PENDING",Self::Trashed=>"TRASHED",
            Self::Restoring=>"RESTORING",Self::Restored=>"RESTORED",
            Self::Uncertain=>"UNCERTAIN",
        }
    }
    fn parse(s:&str)->Option<Self>{
        match s{
            "PENDING"=>Some(Self::Pending),"TRASHED"=>Some(Self::Trashed),
            "RESTORING"=>Some(Self::Restoring),"RESTORED"=>Some(Self::Restored),
            "UNCERTAIN"=>Some(Self::Uncertain),_=>None,
        }
    }
}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct CustodyRecord {
    pub id:String,pub chat_id:String,pub parent:String,
    pub original:String,pub trash_entry:String,
    pub bytes:u64,pub sha256:String,
    pub trash_effect_id:String,pub restore_effect_id:Option<String>,
    pub state:CustodyState,
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum CustodyError{
    Invalid,Missing,StateConflict,EffectMismatch,Poisoned,Database(String),
}
impl From<rusqlite::Error> for CustodyError{
    fn from(e:rusqlite::Error)->Self{Self::Database(e.to_string())}
}
fn id_ok(s:&str)->bool{
    (8..=128).contains(&s.len())&&s.bytes().all(|b|
        b.is_ascii_alphanumeric()||b==b'-'||b==b'_')
}
fn name_ok(s:&str)->bool{
    !s.is_empty()&&s.len()<=240&&s!="."&&s!=".."
        &&!s.starts_with(".lu-stage-")&&!s.ends_with('.')&&!s.ends_with(' ')
        &&!s.chars().any(|c|c.is_control()||matches!(c,'/'|'\\'|':'))
}
fn sha_ok(s:&str)->bool{
    s.len()==64&&s.bytes().all(|b|
        b.is_ascii_digit()||(b'a'..=b'f').contains(&b))
}
fn trash_ok(s:&str)->bool{
    s.starts_with("lu-trash-")&&id_ok(&s[9..])
}
fn valid(record:&CustodyRecord)->bool{
    id_ok(&record.id)&&id_ok(&record.chat_id)&&id_ok(&record.trash_effect_id)
        &&record.restore_effect_id.as_ref().is_none_or(|s|id_ok(s))
        &&!record.parent.is_empty()&&record.parent.len()<=4096
        &&Path::new(&record.parent).is_absolute()
        &&name_ok(&record.original)&&trash_ok(&record.trash_entry)
        &&record.bytes<=MAX_BYTES&&sha_ok(&record.sha256)
}
fn read_record(row:&rusqlite::Row<'_>)->rusqlite::Result<CustodyRecord>{
    let raw:String=row.get(9)?;
    let state=CustodyState::parse(&raw)
        .ok_or_else(||rusqlite::Error::InvalidColumnType(
            9,"state".into(),rusqlite::types::Type::Text
        ))?;
    let bytes:i64=row.get(5)?;
    Ok(CustodyRecord{
        id:row.get(0)?,chat_id:row.get(1)?,parent:row.get(2)?,
        original:row.get(3)?,trash_entry:row.get(4)?,
        bytes:bytes.try_into().map_err(|_|rusqlite::Error::IntegralValueOutOfRange(5,bytes))?,
        sha256:row.get(6)?,trash_effect_id:row.get(7)?,
        restore_effect_id:row.get(8)?,state,
    })
}
const COLUMNS:&str="id,chat_id,parent,original,trash_entry,bytes,sha256,trash_effect_id,restore_effect_id,state";
pub struct TrashCustodyLedger{db:Mutex<Connection>}
impl TrashCustodyLedger{
    pub fn open(path:&Path)->Result<Self,CustodyError>{
        let db=Connection::open(path)?;
        db.pragma_update(None,"journal_mode","WAL")?;
        db.pragma_update(None,"synchronous","FULL")?;
        db.execute_batch(r#"
            CREATE TABLE IF NOT EXISTS lu_trash_custody(
                id TEXT PRIMARY KEY NOT NULL,
                chat_id TEXT NOT NULL,
                parent TEXT NOT NULL,
                original TEXT NOT NULL,
                trash_entry TEXT UNIQUE NOT NULL,
                bytes INTEGER NOT NULL CHECK(bytes BETWEEN 0 AND 33554432),
                sha256 TEXT NOT NULL,
                trash_effect_id TEXT UNIQUE NOT NULL,
                restore_effect_id TEXT UNIQUE,
                state TEXT NOT NULL CHECK(state IN
                    ('PENDING','TRASHED','RESTORING','RESTORED','UNCERTAIN')),
                updated_at TEXT DEFAULT CURRENT_TIMESTAMP NOT NULL
            );
            CREATE INDEX IF NOT EXISTS lu_trash_by_chat ON lu_trash_custody(chat_id);
            CREATE TRIGGER IF NOT EXISTS lu_trash_no_delete
                BEFORE DELETE ON lu_trash_custody BEGIN
                SELECT RAISE(ABORT,'custody evidence cannot be deleted');
            END;
        "#)?;
        Ok(Self{db:Mutex::new(db)})
    }
    pub fn stage(&self,record:&CustodyRecord)->Result<(),CustodyError>{
        if !valid(record)||record.state!=CustodyState::Pending
            ||record.restore_effect_id.is_some(){
            return Err(CustodyError::Invalid)
        }
        let db=self.db.lock().map_err(|_|CustodyError::Poisoned)?;
        db.execute(
            "INSERT INTO lu_trash_custody(
                id,chat_id,parent,original,trash_entry,bytes,sha256,
                trash_effect_id,restore_effect_id,state
             ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,NULL,'PENDING')",
            params![record.id,record.chat_id,record.parent,record.original,
                record.trash_entry,record.bytes as i64,record.sha256,record.trash_effect_id]
        )?;
        Ok(())
    }
    pub fn get(&self,id:&str)->Result<Option<CustodyRecord>,CustodyError>{
        if !id_ok(id){return Err(CustodyError::Invalid)}
        let db=self.db.lock().map_err(|_|CustodyError::Poisoned)?;
        let query=format!("SELECT {COLUMNS} FROM lu_trash_custody WHERE id=?1");
        let record=db.query_row(&query,params![id],read_record).optional()?;
        if record.as_ref().is_some_and(|r|!valid(r)){return Err(CustodyError::Invalid)}
        Ok(record)
    }
    pub fn list_chat(&self,chat:&str)->Result<Vec<CustodyRecord>,CustodyError>{
        if !id_ok(chat){return Err(CustodyError::Invalid)}
        let db=self.db.lock().map_err(|_|CustodyError::Poisoned)?;
        let query=format!("SELECT {COLUMNS} FROM lu_trash_custody
            WHERE chat_id=?1 ORDER BY updated_at DESC, id DESC LIMIT 100");
        let mut stmt=db.prepare(&query)?;
        let rows=stmt.query_map(params![chat],read_record)?;
        let mut out=Vec::new();
        for item in rows{
            let record=item?;
            if !valid(&record){return Err(CustodyError::Invalid)}
            out.push(record);
        }
        Ok(out)
    }
    fn transition(&self,id:&str,chat:&str,prior:CustodyState,
        next:CustodyState,restore_id:Option<&str>
    )->Result<(),CustodyError>{
        let db=self.db.lock().map_err(|_|CustodyError::Poisoned)?;
        let rows=db.execute(
            "UPDATE lu_trash_custody SET state=?1,
                restore_effect_id=COALESCE(?2,restore_effect_id),
                updated_at=CURRENT_TIMESTAMP
             WHERE id=?3 AND chat_id=?4 AND state=?5",
            params![next.as_str(),restore_id,id,chat,prior.as_str()]
        )?;
        if rows!=1{return Err(CustodyError::StateConflict)}
        Ok(())
    }
    pub fn mark_uncertain(&self,id:&str,chat:&str)
        ->Result<(),CustodyError>{
        let db=self.db.lock().map_err(|_|CustodyError::Poisoned)?;
        let rows=db.execute(
            "UPDATE lu_trash_custody SET state='UNCERTAIN',
                updated_at=CURRENT_TIMESTAMP
             WHERE id=?1 AND chat_id=?2 AND state IN ('PENDING','RESTORING')",
            params![id,chat],
        )?;
        if rows!=1{return Err(CustodyError::StateConflict)}
        Ok(())
    }
    /// After old Lu process exits, all unresolved effects are read-only
    /// inspection candidates. A successful effect may still be present in
    /// the journal; this transition never automatically marks it successful.
    pub fn reconcile_interrupted(&self)->Result<usize,CustodyError>{
        let db=self.db.lock().map_err(|_|CustodyError::Poisoned)?;
        Ok(db.execute(
            "UPDATE lu_trash_custody SET state='UNCERTAIN',
                updated_at=CURRENT_TIMESTAMP
             WHERE state IN ('PENDING','RESTORING')",[]
        )?)
    }
    /// Validate the independently journaled, applied ToolReply and exact
    /// protobuf receipt. A user/model-supplied receipt is never trusted.
    fn effect_receipt(
        effects:&EffectJournal,effect_id:&str,tool_id:&str,output_type:&str
    )->Result<Vec<u8>,CustodyError>{
        let effect=effects.get(effect_id).map_err(|_|CustodyError::EffectMismatch)?
            .ok_or(CustodyError::EffectMismatch)?;
        if effect.state!=EffectState::Applied||effect.tool_id!=tool_id{
            return Err(CustodyError::EffectMismatch)
        }
        let raw=effect.receipt.ok_or(CustodyError::EffectMismatch)?;
        let reply=proto::ToolReply::decode(raw.as_slice())
            .map_err(|_|CustodyError::EffectMismatch)?;
        if reply.call_id!=effect_id||reply.output_type_url!=output_type{
            return Err(CustodyError::EffectMismatch)
        }
        match reply.outcome{
            Some(proto::tool_reply::Outcome::Output(data))=>Ok(data),
            _=>Err(CustodyError::EffectMismatch),
        }
    }
    pub fn confirm_trash(
        &self,id:&str,chat:&str,effects:&EffectJournal
    )->Result<CustodyRecord,CustodyError>{
        let record=self.get(id)?.ok_or(CustodyError::Missing)?;
        if record.chat_id!=chat||record.state!=CustodyState::Pending{
            return Err(CustodyError::StateConflict)
        }
        let bytes=Self::effect_receipt(effects,&record.trash_effect_id,TRASH_ID,TRASH_OUTPUT)?;
        let reply=proto::FileTrashResult::decode(bytes.as_slice())
            .map_err(|_|CustodyError::EffectMismatch)?;
        if reply.original_name!=record.original||reply.trash_entry!=record.trash_entry
            ||reply.bytes!=record.bytes||reply.sha256!=record.sha256{
            return Err(CustodyError::EffectMismatch)
        }
        self.transition(id,chat,CustodyState::Pending,CustodyState::Trashed,None)?;
        self.get(id)?.ok_or(CustodyError::Missing)
    }
    pub fn begin_restore(
        &self,id:&str,chat:&str,effect_id:&str,
    )->Result<CustodyRecord,CustodyError>{
        if !id_ok(effect_id){return Err(CustodyError::Invalid)}
        let record=self.get(id)?.ok_or(CustodyError::Missing)?;
        if record.chat_id!=chat||record.state!=CustodyState::Trashed{
            return Err(CustodyError::StateConflict)
        }
        // Revalidate the journaled trash custody at the approval boundary.
        self.transition(id,chat,CustodyState::Trashed,
            CustodyState::Restoring,Some(effect_id))?;
        self.get(id)?.ok_or(CustodyError::Missing)
    }
    pub fn confirm_restore(
        &self,id:&str,chat:&str,effects:&EffectJournal
    )->Result<CustodyRecord,CustodyError>{
        let record=self.get(id)?.ok_or(CustodyError::Missing)?;
        if record.chat_id!=chat||record.state!=CustodyState::Restoring{
            return Err(CustodyError::StateConflict)
        }
        let effect=record.restore_effect_id.as_deref().ok_or(CustodyError::Invalid)?;
        let bytes=Self::effect_receipt(effects,effect,RESTORE_ID,RESTORE_OUTPUT)?;
        let reply=proto::FileRestoreResult::decode(bytes.as_slice())
            .map_err(|_|CustodyError::EffectMismatch)?;
        if reply.restored_name!=record.original||reply.bytes!=record.bytes
            ||reply.sha256!=record.sha256{return Err(CustodyError::EffectMismatch)}
        self.transition(id,chat,CustodyState::Restoring,
            CustodyState::Restored,None)?;
        self.get(id)?.ok_or(CustodyError::Missing)
    }
    pub fn validated_trash_receipt(
        &self,id:&str,chat:&str,effects:&EffectJournal
    )->Result<(CustodyRecord,TrashReceipt),CustodyError>{
        let record=self.get(id)?.ok_or(CustodyError::Missing)?;
        if record.chat_id!=chat||record.state!=CustodyState::Trashed{
            return Err(CustodyError::StateConflict)
        }
        let bytes=Self::effect_receipt(effects,&record.trash_effect_id,TRASH_ID,TRASH_OUTPUT)?;
        let reply=proto::FileTrashResult::decode(bytes.as_slice())
            .map_err(|_|CustodyError::EffectMismatch)?;
        if reply.original_name!=record.original||reply.trash_entry!=record.trash_entry
            ||reply.bytes!=record.bytes||reply.sha256!=record.sha256{
            return Err(CustodyError::EffectMismatch)
        }
        Ok((record,TrashReceipt{
            original:reply.original_name,trash_entry:reply.trash_entry,
            bytes:reply.bytes,sha256:reply.sha256,
        }))
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use std::fs;
    fn sample()->CustodyRecord{
        CustodyRecord{
            id:"ct-0123456789abcdef".into(),
            chat_id:"c-0123456789abcdef0123456789abcdef".into(),
            parent:std::env::temp_dir().to_string_lossy().into_owned(),
            original:"notes.txt".into(),trash_entry:"lu-trash-g-12345678901234567890".into(),
            bytes:4,sha256:"a".repeat(64),
            trash_effect_id:"g-12345678901234567890".into(),
            restore_effect_id:None,state:CustodyState::Pending,
        }
    }
    #[test]
    fn crash_recovery_is_observation_only_and_no_deleted_custody(){
        let path=std::env::temp_dir().join(format!(
            "lu-trash-ledger-{}-{}.sqlite",std::process::id(),
            431+std::thread::current().name().unwrap_or("").len()
        ));
        let _=fs::remove_file(&path);
        let ledger=TrashCustodyLedger::open(&path).unwrap();
        let record=sample();
        ledger.stage(&record).unwrap();
        assert_eq!(ledger.get(&record.id).unwrap().unwrap().state,CustodyState::Pending);
        assert_eq!(ledger.reconcile_interrupted().unwrap(),1);
        assert_eq!(ledger.reconcile_interrupted().unwrap(),0);
        assert_eq!(ledger.get(&record.id).unwrap().unwrap().state,CustodyState::Uncertain);
        assert!(ledger.begin_restore(&record.id,&record.chat_id,"restore-123456789").is_err());
        assert!(ledger.confirm_trash(&record.id,&record.chat_id,
            &EffectJournal::open(&path.with_extension("effects.sqlite")).unwrap()).is_err());
        drop(ledger);
        let reopened=TrashCustodyLedger::open(&path).unwrap();
        assert_eq!(reopened.list_chat(&record.chat_id).unwrap().len(),1);
        drop(reopened);
        let _=fs::remove_file(&path);
        let _=fs::remove_file(path.with_extension("effects.sqlite"));
    }
}
