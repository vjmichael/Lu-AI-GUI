//! P7L durable original alternative process custody register.
//! SQLite is a host-only journal, NOT a witness of Windows Job membership or
//! Linux cgroup custody. On restart an unresolved starting/running record is
//! quarantined: PID reuse is NEVER proof the old model worker was reaped.
//! An OS owner must separately reconcile and authorize any release.
use rusqlite::{params,Connection,OptionalExtension,TransactionBehavior};
use std::{fs,io,path::{Path,PathBuf}};

#[derive(Debug,Clone,PartialEq,Eq)]
pub enum CustodyError {
    UnsafeLocation,StoreUnavailable,ExistingUnreconciled,
    WrongSession,InvalidSession,
}
fn db_error(_error:rusqlite::Error)->CustodyError{
    CustodyError::StoreUnavailable
}
fn valid_ref(s:&str)->bool{
    !s.is_empty()&&s.len()<=128&&s.bytes().all(|b|
        b.is_ascii_alphanumeric()||matches!(b,b'-'|b'_'|b'.'))
}
pub struct KoboldCustodyJournal{
    path:PathBuf,
}
impl KoboldCustodyJournal {
    /// Journal path is a trusted host-owned *absolute* file path. The
    /// containing directory must exist; neither the model nor WebView can
    /// resolve alternative journal locations.
    pub fn new(path:PathBuf)->Result<Self,CustodyError>{
        if !path.is_absolute() {return Err(CustodyError::UnsafeLocation)}
        let parent=path.parent().ok_or(CustodyError::UnsafeLocation)?;
        let meta=fs::symlink_metadata(parent)
            .map_err(|_|CustodyError::UnsafeLocation)?;
        if !meta.file_type().is_dir(){return Err(CustodyError::UnsafeLocation)}
        if let Ok(meta)=fs::symlink_metadata(&path){
            if !meta.file_type().is_file() || meta.len()>4*1024*1024 {
                return Err(CustodyError::UnsafeLocation)
            }
        }else if let Err(e)=fs::symlink_metadata(&path){
            if e.kind()!=io::ErrorKind::NotFound {
                return Err(CustodyError::UnsafeLocation)
            }
        }
        let journal=Self{path};
        let _=journal.open()?;
        Ok(journal)
    }
    fn open(&self)->Result<Connection,CustodyError>{
        let conn=Connection::open(&self.path).map_err(db_error)?;
        conn.busy_timeout(std::time::Duration::from_millis(1000))
            .map_err(db_error)?;
        conn.execute_batch("
            PRAGMA journal_mode=WAL;
            PRAGMA synchronous=FULL;
            CREATE TABLE IF NOT EXISTS p7_kobold_custody (
                id INTEGER PRIMARY KEY CHECK (id=1),
                session_id TEXT NOT NULL,
                state TEXT NOT NULL CHECK
                   (state IN ('STARTING','RUNNING','REAPED')),
                pid INTEGER,
                engine_digest TEXT NOT NULL,
                model_digest TEXT NOT NULL
            );
        ").map_err(db_error)?;
        Ok(conn)
    }
    /// Atomically claim the ONE original supervised launch. An interrupted
    /// process cannot create a second session after host restart merely
    /// because this is a different Rust object or PID.
    pub fn begin(
        &self,session:&str,engine:[u8;32],model:[u8;32]
    )->Result<(),CustodyError>{
        if !valid_ref(session)||engine==[0;32]||model==[0;32]{
            return Err(CustodyError::InvalidSession)
        }
        let mut conn=self.open()?;
        let tx=conn.transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let present=tx.query_row(
            "SELECT state FROM p7_kobold_custody WHERE id=1",[],
            |row|row.get::<_,String>(0)
        ).optional().map_err(db_error)?;
        if present.is_some_and(|state|state!="REAPED"){
            return Err(CustodyError::ExistingUnreconciled)
        }
        tx.execute(
            "INSERT INTO p7_kobold_custody
                (id,session_id,state,pid,engine_digest,model_digest)
             VALUES (1,?1,'STARTING',NULL,?2,?3)
             ON CONFLICT(id) DO UPDATE SET
                session_id=excluded.session_id,state='STARTING',
                pid=NULL,engine_digest=excluded.engine_digest,
                model_digest=excluded.model_digest",
            params![session,hex(&engine),hex(&model)]
        ).map_err(db_error)?;
        tx.commit().map_err(db_error)
    }
    pub fn mark_spawned(&self,session:&str,pid:u32)
        ->Result<(),CustodyError>{
        if !valid_ref(session)||pid==0{return Err(CustodyError::InvalidSession)}
        let count=self.open()?.execute(
            "UPDATE p7_kobold_custody SET state='RUNNING',pid=?1
             WHERE id=1 AND session_id=?2 AND state='STARTING'",
            params![i64::from(pid),session]
        ).map_err(db_error)?;
        if count==1{Ok(())}else{Err(CustodyError::WrongSession)}
    }
    /// Called by the original OS supervisor ONLY after an actual child wait,
    /// and after its owned Job/group cleanup. If uncertain leave the durable
    /// row STARTING/RUNNING and refuse all new launches on this journal.
    pub fn finish_verified(&self,session:&str,original_reaped:bool)
        ->Result<(),CustodyError>{
        if !original_reaped{return Err(CustodyError::ExistingUnreconciled)}
        if !valid_ref(session){return Err(CustodyError::InvalidSession)}
        let count=self.open()?.execute(
            "UPDATE p7_kobold_custody SET state='REAPED'
             WHERE id=1 AND session_id=?1
                   AND state IN ('STARTING','RUNNING')",
            params![session]
        ).map_err(db_error)?;
        if count==1{Ok(())}else{Err(CustodyError::WrongSession)}
    }
    pub fn pending(&self)->Result<bool,CustodyError>{
        let state=self.open()?.query_row(
            "SELECT state FROM p7_kobold_custody WHERE id=1",[],
            |r|r.get::<_,String>(0)
        ).optional().map_err(db_error)?;
        Ok(state.as_deref().is_some_and(|value|value!="REAPED"))
    }
}
fn hex(v:&[u8;32])->String{
    v.iter().map(|b|format!("{b:02x}")).collect()
}
#[cfg(test)]
mod tests{
    use super::*;
    use std::{time::{SystemTime,UNIX_EPOCH},fs};
    fn fixture()->(PathBuf,KoboldCustodyJournal){
        let now=SystemTime::now().duration_since(UNIX_EPOCH)
            .unwrap().as_nanos();
        let directory=std::env::temp_dir().join(format!(
            "lu-p7l-custody-{}-{now}",std::process::id()
        ));
        fs::create_dir(&directory).unwrap();
        let db=KoboldCustodyJournal::new(
            directory.join("engine-custody.sqlite")
        ).unwrap();
        (directory,db)
    }
    #[test]
    fn active_session_survives_new_journal_object_and_blocks_recovery(){
        let (directory,db)=fixture();
        db.begin("s1",[1;32],[2;32]).unwrap();
        db.mark_spawned("s1",1234).unwrap();
        drop(db);
        let reopen=KoboldCustodyJournal::new(
            directory.join("engine-custody.sqlite")
        ).unwrap();
        assert_eq!(reopen.pending(),Ok(true));
        assert_eq!(reopen.begin("s2",[1;32],[2;32]),
            Err(CustodyError::ExistingUnreconciled));
        assert_eq!(reopen.finish_verified("s1",false),
            Err(CustodyError::ExistingUnreconciled));
        assert_eq!(reopen.finish_verified("s2",true),
            Err(CustodyError::WrongSession));
        reopen.finish_verified("s1",true).unwrap();
        reopen.begin("s2",[1;32],[2;32]).unwrap();
        reopen.finish_verified("s2",true).unwrap();
        drop(reopen);
        fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn crash_after_admission_before_spawn_never_restarts_automatically(){
        let (directory,db)=fixture();
        db.begin("s1",[1;32],[2;32]).unwrap();
        let other=KoboldCustodyJournal::new(
            directory.join("engine-custody.sqlite")
        ).unwrap();
        assert!(other.pending().unwrap());
        assert!(matches!(other.begin("s1",[1;32],[2;32]),
            Err(CustodyError::ExistingUnreconciled)));
        other.finish_verified("s1",true).unwrap();
        drop(other);drop(db);
        fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn unknown_or_incorrect_session_cannot_release_custody(){
        let (directory,db)=fixture();
        assert_eq!(db.begin("bad/path",[1;32],[2;32]),
            Err(CustodyError::InvalidSession));
        db.begin("s1",[1;32],[2;32]).unwrap();
        assert_eq!(db.mark_spawned("s2",9),
            Err(CustodyError::WrongSession));
        assert_eq!(db.finish_verified("s2",true),
            Err(CustodyError::WrongSession));
        assert!(db.pending().unwrap());
        db.finish_verified("s1",true).unwrap();
        drop(db);
        fs::remove_dir_all(directory).unwrap();
    }
}
