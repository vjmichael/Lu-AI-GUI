//! Immutable image export history, not a source-image editor or undo engine.
//! Host-only PREPARED -> APPLIED/UNCERTAIN receipts. Never auto-replay an
//! uncertain filesystem write; history reports historical evidence, not proof
//! that the exported path or current bytes still exist.
use rusqlite::{params,Connection};
use serde::Serialize;
use std::path::Path;

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum RevisionState {Prepared,Applied,Uncertain}
impl RevisionState {
    pub fn as_str(self)->&'static str{
        match self {Self::Prepared=>"PREPARED",Self::Applied=>"APPLIED",Self::Uncertain=>"UNCERTAIN"}
    }
}
#[derive(Debug,Clone,Serialize,PartialEq,Eq)]
#[serde(rename_all="camelCase")]
pub struct ImageRevisionHistoryRow {
    pub id:String,
    pub base_ref:String,
    pub base_sha256:String,
    pub revision_sha256:String,
    pub file_name:String,
    pub width:u32,
    pub height:u32,
    pub recorded_bytes:u64,
    pub state:String,
}
#[derive(Debug)]
pub enum RevisionHistoryError {
    Invalid,Database,NotPrepared,
}
pub struct ImageRevisionLedger { db:Connection }
fn scoped_chat(v:&str)->bool{
    v.starts_with("c-")&&v.len()<=128
        &&v.bytes().all(|x|x.is_ascii_alphanumeric()||x==b'-'||x==b'_')
}
fn valid_id(v:&str)->bool{
    v.starts_with("image-rev-")&&v.len()=="image-rev-".len()+32
        &&v["image-rev-".len()..].bytes().all(|x|x.is_ascii_hexdigit())
}
fn hash_ok(v:&str)->bool{
    v.len()==64&&v.bytes().all(|x|x.is_ascii_hexdigit())
}
fn filename_ok(v:&str)->bool{
    !v.is_empty()&&v.len()<=200&&v!="."&&v!=".."
       &&!v.starts_with(".lu-stage-")&&!v.ends_with('.')
       &&!v.ends_with(' ')&&!v.chars().any(|c|c.is_control()||matches!(c,'/'|'\\'|':'))
       &&v.to_ascii_lowercase().ends_with(".png")
}
fn valid_dims(width:u32,height:u32)->bool{
    width>0&&height>0&&width<=8192&&height<=8192
        &&u64::from(width)*u64::from(height)<=32_000_000
}
impl ImageRevisionLedger {
    pub fn open(path:&Path)->Result<Self,RevisionHistoryError>{
        if !path.is_absolute()&&!path.to_str().is_some_and(|s|s==":memory:"){
            return Err(RevisionHistoryError::Invalid)
        }
        let db=Connection::open(path).map_err(|_|RevisionHistoryError::Database)?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;
            CREATE TABLE IF NOT EXISTS lu_image_revision_history(
                id TEXT PRIMARY KEY NOT NULL,
                chat_id TEXT NOT NULL,
                base_ref TEXT NOT NULL,
                base_sha256 TEXT NOT NULL,
                revision_sha256 TEXT NOT NULL,
                file_name TEXT NOT NULL,
                width INTEGER NOT NULL,
                height INTEGER NOT NULL,
                recorded_bytes INTEGER NOT NULL DEFAULT 0,
                state TEXT NOT NULL CHECK(state IN ('PREPARED','APPLIED','UNCERTAIN')),
                created_at INTEGER NOT NULL DEFAULT (unixepoch())
            );
            CREATE INDEX IF NOT EXISTS lu_image_revision_chat
                ON lu_image_revision_history(chat_id,created_at DESC,id DESC);")
            .map_err(|_|RevisionHistoryError::Database)?;
        Ok(Self{db})
    }
    /// Persist the intended output before touching the user-selected file.
    pub fn prepare(&mut self,chat:&str,row:&ImageRevisionHistoryRow)
        ->Result<(),RevisionHistoryError>{
        if !scoped_chat(chat)||!valid_id(&row.id)||!hash_ok(&row.base_sha256)
           ||!hash_ok(&row.revision_sha256)||!filename_ok(&row.file_name)
           ||!row.base_ref.starts_with("img-")||row.base_ref.len()!=36
           ||!row.base_ref[4..].bytes().all(|x|x.is_ascii_hexdigit())
           ||!valid_dims(row.width,row.height)
           ||row.recorded_bytes!=0||row.state!="PREPARED"{
            return Err(RevisionHistoryError::Invalid);
        }
        self.db.execute("INSERT INTO lu_image_revision_history(
            id,chat_id,base_ref,base_sha256,revision_sha256,file_name,
            width,height,recorded_bytes,state
        ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,0,'PREPARED')",
        params![row.id,chat,row.base_ref,row.base_sha256,row.revision_sha256,
            row.file_name,row.width,row.height])
            .map_err(|_|RevisionHistoryError::Database)?;
        Ok(())
    }
    /// Requires the re-opened output bytes to have already matched the
    /// host's intended SHA; this ledger never itself grants filesystem access.
    pub fn applied(&mut self,chat:&str,id:&str,hash:&str,bytes:u64)
        ->Result<(),RevisionHistoryError>{
        if !scoped_chat(chat)||!valid_id(id)||!hash_ok(hash)||bytes==0
           ||bytes>128*1024*1024{return Err(RevisionHistoryError::Invalid);}
        let changed=self.db.execute("UPDATE lu_image_revision_history
            SET state='APPLIED',recorded_bytes=?4
            WHERE id=?1 AND chat_id=?2 AND revision_sha256=?3 AND state='PREPARED'",
            params![id,chat,hash,bytes])
            .map_err(|_|RevisionHistoryError::Database)?;
        if changed!=1{return Err(RevisionHistoryError::NotPrepared)}
        Ok(())
    }
    /// Once output publication might have begun, never report a clean
    /// failure or retry automatically even if the writer attempted cleanup.
    pub fn uncertain(&mut self,chat:&str,id:&str)
        ->Result<(),RevisionHistoryError>{
        if !scoped_chat(chat)||!valid_id(id){return Err(RevisionHistoryError::Invalid);}
        let changed=self.db.execute("UPDATE lu_image_revision_history
            SET state='UNCERTAIN' WHERE id=?1 AND chat_id=?2 AND state='PREPARED'",
            params![id,chat]).map_err(|_|RevisionHistoryError::Database)?;
        if changed!=1{return Err(RevisionHistoryError::NotPrepared)}
        Ok(())
    }
    /// Call once at app startup, before admitting new image exports.
    pub fn reconcile_interrupted(&mut self)->Result<usize,RevisionHistoryError>{
        self.db.execute("UPDATE lu_image_revision_history
            SET state='UNCERTAIN' WHERE state='PREPARED'",[])
            .map_err(|_|RevisionHistoryError::Database)
    }
    pub fn history(&self,chat:&str,limit:usize)
        ->Result<Vec<ImageRevisionHistoryRow>,RevisionHistoryError>{
        if !scoped_chat(chat)||!(1..=100).contains(&limit){
            return Err(RevisionHistoryError::Invalid);
        }
        let mut query=self.db.prepare("SELECT id,base_ref,base_sha256,
            revision_sha256,file_name,width,height,recorded_bytes,state
            FROM lu_image_revision_history WHERE chat_id=?1
            ORDER BY created_at DESC,id DESC LIMIT ?2")
            .map_err(|_|RevisionHistoryError::Database)?;
        let rows=query.query_map(params![chat,limit as i64],|r|Ok(ImageRevisionHistoryRow{
            id:r.get(0)?,base_ref:r.get(1)?,
            base_sha256:r.get(2)?,revision_sha256:r.get(3)?,
            file_name:r.get(4)?,width:r.get(5)?,height:r.get(6)?,
            recorded_bytes:r.get(7)?,state:r.get(8)?,
        })).map_err(|_|RevisionHistoryError::Database)?;
        rows.collect::<Result<Vec<_>,_>>()
            .map_err(|_|RevisionHistoryError::Database)
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    fn sample()->ImageRevisionHistoryRow{
        ImageRevisionHistoryRow{
            id:format!("image-rev-{}", "a".repeat(32)),
            base_ref:format!("img-{}","b".repeat(32)),
            base_sha256:"c".repeat(64),
            revision_sha256:"d".repeat(64),
            file_name:"revision.png".into(),width:64,height:32,
            recorded_bytes:0,state:"PREPARED".into(),
        }
    }
    #[test]
    fn immutable_history_is_chat_scoped_and_uncertain_never_replays(){
        let mut db=ImageRevisionLedger::open(Path::new(":memory:")).unwrap();
        let first=sample();
        db.prepare("c-first",&first).unwrap();
        assert!(db.history("c-other",10).unwrap().is_empty());
        assert_eq!(db.history("c-first",10).unwrap()[0].state,"PREPARED");
        db.applied("c-first",&first.id,&first.revision_sha256,100).unwrap();
        assert_eq!(db.history("c-first",10).unwrap()[0].recorded_bytes,100);
        assert_eq!(db.history("c-first",10).unwrap()[0].state,"APPLIED");
        assert!(matches!(db.applied("c-first",&first.id,&first.revision_sha256,100),
            Err(RevisionHistoryError::NotPrepared)));
        let mut second=sample();second.id=format!("image-rev-{}", "f".repeat(32));
        db.prepare("c-first",&second).unwrap();
        assert_eq!(db.reconcile_interrupted().unwrap(),1);
        let states:Vec<_>=db.history("c-first",10).unwrap().into_iter().map(|x|x.state).collect();
        assert!(states.contains(&"UNCERTAIN".into()));
        assert!(states.contains(&"APPLIED".into()));
        assert!(db.applied("c-first",&second.id,&second.revision_sha256,100).is_err());
        assert!(db.history("c-second",10).unwrap().is_empty());
    }
    #[test]
    fn unsafe_filename_or_wrong_digest_does_not_become_applied(){
        let mut db=ImageRevisionLedger::open(Path::new(":memory:")).unwrap();
        let mut bad=sample();bad.file_name="bad/../revision.png".to_owned();
        assert!(db.prepare("c-first",&bad).is_err());
        let ok=sample();db.prepare("c-first",&ok).unwrap();
        assert!(db.applied("c-first",&ok.id,&"0".repeat(64),100).is_err());
        db.uncertain("c-first",&ok.id).unwrap();
        assert!(db.applied("c-first",&ok.id,&ok.revision_sha256,100).is_err());
    }
}
