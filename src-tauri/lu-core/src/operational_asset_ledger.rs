//! M3 operational continuity: verified *references* to existing build assets.
//! Asset bytes stay outside ANSF; exact chat evidence is a source locator.
//! This store does not execute tools, install SDKs, grant permissions or claim
//! that a recorded package is still present after a future process restart.
//! The trusted desktop host must authenticate source IDs and file selection.
use crate::ansf_source_ids::AnsfSourceId;
use rusqlite::{params,Connection,OptionalExtension};
use sha2::{Digest,Sha256};
use std::{fs::{self,File},io::Read,path::{Path,PathBuf},sync::Mutex};

const MAX_ASSET_BYTES:u64=16*1024*1024*1024; // 16 GiB; bounded streaming hash
const MAX_ROWS:usize=128;
const BUF:usize=1024*1024;
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum AssetError{Database(String),InvalidInput,SourceDenied,UnsafeFile,
    Unavailable,Changed,Locked,IdentityConflict}
impl From<rusqlite::Error> for AssetError{
    fn from(e:rusqlite::Error)->Self{Self::Database(e.to_string())}
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct AssetProposal{
    pub record_id:String,
    pub project_id:String,
    pub logical_name:String,
    pub version:String,
    pub source_id:String,
    pub local_path:PathBuf,
    pub expected_sha256:Option<String>,
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct AssetRecord{
    pub record_id:String,
    pub project_id:String,
    pub logical_name:String,
    pub version:String,
    pub source_id:String,
    pub local_path:PathBuf,
    pub bytes:u64,
    pub sha256:String,
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum AssetPresence{PresentVerified,Missing,ChangedOrUnsafe}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum AssetReuseState{VerifiedPresent,RecordedMissing,RecordedChanged,NotRecorded}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct AssetReuseDecision{
    pub state:AssetReuseState,
    pub record:Option<AssetRecord>,
}

pub struct OperationalAssetLedger{conn:Mutex<Connection>}
fn identifier(s:&str)->bool{
    !s.is_empty()&&s.len()<=128&&
       s.bytes().all(|x|x.is_ascii_alphanumeric()||matches!(x,b'-'|b'_'|b'.'))
}
fn label(s:&str)->bool{
    !s.trim().is_empty()&&s.len()<=180&&!s.contains('\0')&&
       !s.contains('\n')&&!s.contains('\r')
}
fn hex_sha(s:&str)->bool{
    s.len()==64&&s.bytes().all(|x|x.is_ascii_hexdigit()&&!x.is_ascii_uppercase())
}
fn checked_local_path(path:&Path)->Result<(),AssetError>{
    if !path.is_absolute(){return Err(AssetError::InvalidInput)}
    let meta=fs::symlink_metadata(path).map_err(|_|AssetError::Unavailable)?;
    if meta.file_type().is_symlink()||!meta.is_file()||meta.len()==0||
       meta.len()>MAX_ASSET_BYTES{return Err(AssetError::UnsafeFile)}
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes()&0x400!=0{return Err(AssetError::UnsafeFile)}
    }
    Ok(())
}
fn digest_file(path:&Path)->Result<(u64,String),AssetError>{
    checked_local_path(path)?;
    let mut file=File::open(path).map_err(|_|AssetError::Unavailable)?;
    let start=file.metadata().map_err(|_|AssetError::Unavailable)?;
    if !start.is_file()||start.len()==0||start.len()>MAX_ASSET_BYTES{
        return Err(AssetError::UnsafeFile)
    }
    let mut digest=Sha256::new();let mut buf=vec![0u8;BUF];let mut count=0u64;
    loop{
        let n=file.read(&mut buf).map_err(|_|AssetError::Unavailable)?;
        if n==0{break}
        count=count.checked_add(n as u64).ok_or(AssetError::UnsafeFile)?;
        if count>MAX_ASSET_BYTES{return Err(AssetError::UnsafeFile)}
        digest.update(&buf[..n]);
    }
    let end=file.metadata().map_err(|_|AssetError::Unavailable)?;
    if end.len()!=start.len()||count!=start.len(){
        return Err(AssetError::Changed)
    }
    // The path can be swapped after hashing. Revalidation is mandatory before
    // a later effect, and the path itself never confers execute authority.
    Ok((count,format!("{:x}",digest.finalize())))
}
impl OperationalAssetLedger{
    pub fn open(path:&Path)->Result<Self,AssetError>{
        let conn=Connection::open(path)?;
        conn.pragma_update(None,"journal_mode","WAL")?;
        conn.pragma_update(None,"synchronous","FULL")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS lu_operational_asset_receipts(
              record_id TEXT PRIMARY KEY,
              project_id TEXT NOT NULL,logical_name TEXT NOT NULL,
              version TEXT NOT NULL,source_id TEXT NOT NULL,
              local_path TEXT NOT NULL,bytes INTEGER NOT NULL,
              sha256 TEXT NOT NULL,observed_utc TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
             );
             CREATE INDEX IF NOT EXISTS lu_operational_asset_project
               ON lu_operational_asset_receipts(project_id);"
        )?;
        conn.execute_batch(
            "CREATE TRIGGER IF NOT EXISTS lu_asset_no_update
               BEFORE UPDATE ON lu_operational_asset_receipts BEGIN
               SELECT RAISE(ABORT,'asset observations are immutable'); END;
             CREATE TRIGGER IF NOT EXISTS lu_asset_no_delete
               BEFORE DELETE ON lu_operational_asset_receipts BEGIN
               SELECT RAISE(ABORT,'asset observations are immutable'); END;"
        )?;
        Ok(Self{conn:Mutex::new(conn)})
    }
    /// Caller is the trusted host. It MUST validate that the current profile
    /// can read the source chat and that a local file was actually selected
    /// through native host UX or trusted tool evidence, not model text.
    pub fn record_verified(
        &self,p:&AssetProposal,source_allowed:impl FnOnce(&str)->bool
    )->Result<AssetRecord,AssetError>{
        if !identifier(&p.record_id)||!identifier(&p.project_id)||
           !label(&p.logical_name)||!label(&p.version)||
           AnsfSourceId::parse(&p.source_id).is_err()||
           p.expected_sha256.as_deref().is_some_and(|s|!hex_sha(s)){
            return Err(AssetError::InvalidInput)
        }
        if !source_allowed(&p.source_id){return Err(AssetError::SourceDenied)}
        let raw=p.local_path.to_str().ok_or(AssetError::InvalidInput)?;
        if raw.len()>4096{return Err(AssetError::InvalidInput)}
        let (bytes,sha256)=digest_file(&p.local_path)?;
        if p.expected_sha256.as_deref().is_some_and(|v|v!=sha256){
            return Err(AssetError::Changed)
        }
        let record=AssetRecord{
            record_id:p.record_id.clone(),project_id:p.project_id.clone(),
            logical_name:p.logical_name.clone(),version:p.version.clone(),
            source_id:p.source_id.clone(),local_path:p.local_path.clone(),
            bytes,sha256
        };
        let conn=self.conn.lock().map_err(|_|AssetError::Locked)?;
        conn.execute(
            "INSERT OR IGNORE INTO lu_operational_asset_receipts
             (record_id,project_id,logical_name,version,source_id,
              local_path,bytes,sha256)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![record.record_id,record.project_id,record.logical_name,
                record.version,record.source_id,raw,record.bytes as i64,
                record.sha256]
        )?;
        let existing=Self::get_by_id(&conn,&record.record_id)?
            .ok_or(AssetError::IdentityConflict)?;
        if existing!=record{return Err(AssetError::IdentityConflict)}
        Ok(record)
    }
    fn get_by_id(conn:&Connection,id:&str)->Result<Option<AssetRecord>,AssetError>{
        conn.query_row(
            "SELECT record_id,project_id,logical_name,version,
                    source_id,local_path,bytes,sha256
             FROM lu_operational_asset_receipts WHERE record_id=?1",
            [id],|r|{
                let bytes:i64=r.get(6)?;
                Ok(AssetRecord{
                    record_id:r.get(0)?,project_id:r.get(1)?,logical_name:r.get(2)?,
                    version:r.get(3)?,source_id:r.get(4)?,
                    local_path:PathBuf::from(r.get::<_,String>(5)?),
                    bytes:bytes.try_into().map_err(|_|rusqlite::Error::IntegralValueOutOfRange(6,bytes))?,
                    sha256:r.get(7)?
                })
            }
        ).optional().map_err(Into::into)
    }
    pub fn recent_for_project(&self,project_id:&str,limit:usize)
        ->Result<Vec<AssetRecord>,AssetError>{
        if !identifier(project_id)||limit==0||limit>MAX_ROWS{
            return Err(AssetError::InvalidInput)
        }
        let conn=self.conn.lock().map_err(|_|AssetError::Locked)?;
        let mut stmt=conn.prepare(
            "SELECT record_id FROM lu_operational_asset_receipts
             WHERE project_id=?1 ORDER BY rowid DESC LIMIT ?2"
        )?;
        let ids=stmt.query_map(params![project_id,limit as i64],|r|r.get::<_,String>(0))?
            .collect::<Result<Vec<_>,_>>()?;
        ids.into_iter().map(|id|Self::get_by_id(&conn,&id)?
            .ok_or(AssetError::IdentityConflict)).collect()
    }
    /// Re-find an already registered asset by original project/name/version,
    /// without requesting a new OS file picker or rediscovering a package.
    /// Select only currently SHA-256-verified source-authorized records; if
    /// multiple historical locations exist, fall back to a verified older one.
    /// Returns metadata only; caller still needs a fresh effect grant, and
    /// MUST revalidate or pin an opened handle at the actual use boundary.
    pub fn resolve_for_reuse(
        &self,project_id:&str,logical_name:&str,version:&str,
        mut source_allowed:impl FnMut(&str)->bool
    )->Result<AssetReuseDecision,AssetError>{
        if !identifier(project_id)||!label(logical_name)||!label(version){
            return Err(AssetError::InvalidInput)
        }
        let records={
            let conn=self.conn.lock().map_err(|_|AssetError::Locked)?;
            let mut stmt=conn.prepare(
                "SELECT record_id FROM lu_operational_asset_receipts
                 WHERE project_id=?1 AND logical_name=?2 AND version=?3
                 ORDER BY rowid DESC LIMIT 24"
            )?;
            let ids=stmt.query_map(
                params![project_id,logical_name,version],
                |r|r.get::<_,String>(0)
            )?.collect::<Result<Vec<_>,_>>()?;
            ids.into_iter().map(|id|Self::get_by_id(&conn,&id)?
                .ok_or(AssetError::IdentityConflict)).collect::<Result<Vec<_>,_>>()?
        };
        let mut stale:Option<AssetReuseDecision>=None;
        for record in records{
            if !source_allowed(&record.source_id){continue}
            let presence=self.revalidate(&record);
            match presence{
                AssetPresence::PresentVerified=>return Ok(AssetReuseDecision{
                    state:AssetReuseState::VerifiedPresent,record:Some(record)
                }),
                AssetPresence::Missing|AssetPresence::ChangedOrUnsafe=>{
                    if stale.is_none(){
                        stale=Some(AssetReuseDecision{
                            state:if presence==AssetPresence::Missing{
                                AssetReuseState::RecordedMissing
                            }else{AssetReuseState::RecordedChanged},
                            record:Some(record)
                        });
                    }
                }
            }
        }
        Ok(stale.unwrap_or(AssetReuseDecision{
            state:AssetReuseState::NotRecorded,record:None
        }))
    }

    /// A historical record is *not* a live file guarantee. Never return a
    /// usable path as an execution grant; the real tool still needs its
    /// current user task grant and pinned file handle.
    pub fn revalidate(&self,record:&AssetRecord)->AssetPresence{
        match digest_file(&record.local_path){
            Ok((len,hash)) if len==record.bytes&&hash==record.sha256=>
                AssetPresence::PresentVerified,
            Err(AssetError::Unavailable)=>AssetPresence::Missing,
            _=>AssetPresence::ChangedOrUnsafe,
        }
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use std::time::{SystemTime,UNIX_EPOCH};
    fn fixture()->PathBuf{std::env::temp_dir().join(format!(
        "lu-operational-assets-{}-{}",std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()))}
    fn proposal(path:PathBuf)->AssetProposal{
        AssetProposal{
            record_id:"asset-001".into(),project_id:"lu".into(),
            logical_name:"Windows SDK package".into(),version:"10.0-test".into(),
            source_id:AnsfSourceId::new("c-source",1).unwrap().encode(),
            local_path:path,expected_sha256:None,
        }
    }
    #[test]
    fn new_chat_reuses_verified_binary_and_detects_mutation_and_absence(){
        let root=fixture();fs::create_dir_all(&root).unwrap();
        let package=root.join("sdk.zip");fs::write(&package,b"real binary bytes").unwrap();
        let proposal=proposal(package.clone());
        {
            let ledger=OperationalAssetLedger::open(&root.join("assets.sqlite")).unwrap();
            let saved=ledger.record_verified(&proposal,|_|true).unwrap();
            assert_eq!(ledger.revalidate(&saved),AssetPresence::PresentVerified);
            assert_eq!(ledger.record_verified(&proposal,|_|true).unwrap(),saved);
        }
        let reopened=OperationalAssetLedger::open(&root.join("assets.sqlite")).unwrap();
        let saved=reopened.recent_for_project("lu",4).unwrap().remove(0);
        assert_eq!(reopened.revalidate(&saved),AssetPresence::PresentVerified);
        fs::write(&package,b"tampered package").unwrap();
        assert_eq!(reopened.revalidate(&saved),AssetPresence::ChangedOrUnsafe);
        fs::remove_file(&package).unwrap();
        assert_eq!(reopened.revalidate(&saved),AssetPresence::Missing);
        drop(reopened);fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn automatic_reuse_checks_source_and_hash_then_falls_back_to_valid_cached_location(){
        let root=fixture();fs::create_dir_all(&root).unwrap();
        let one=root.join("sdk-existing.zip");
        let two=root.join("sdk-newer.zip");
        fs::write(&one,b"safe previously verified package").unwrap();
        fs::write(&two,b"safe newly verified package").unwrap();
        let ledger=OperationalAssetLedger::open(&root.join("assets.sqlite")).unwrap();
        let a=proposal(one.clone());
        let mut b=proposal(two.clone());b.record_id="asset-002".into();
        ledger.record_verified(&a,|_|true).unwrap();
        ledger.record_verified(&b,|_|true).unwrap();
        let latest=ledger.resolve_for_reuse(
            "lu","Windows SDK package","10.0-test",|_|true).unwrap();
        assert_eq!(latest.state,AssetReuseState::VerifiedPresent);
        assert_eq!(latest.record.unwrap().record_id,"asset-002");
        fs::write(&two,b"changed newer file").unwrap();
        let fallback=ledger.resolve_for_reuse(
            "lu","Windows SDK package","10.0-test",|_|true).unwrap();
        assert_eq!(fallback.state,AssetReuseState::VerifiedPresent);
        assert_eq!(fallback.record.unwrap().record_id,"asset-001");
        assert_eq!(ledger.resolve_for_reuse(
            "lu","Windows SDK package","10.0-test",|_|false).unwrap().state,
            AssetReuseState::NotRecorded);
        fs::remove_file(&one).unwrap();
        let status=ledger.resolve_for_reuse(
            "lu","Windows SDK package","10.0-test",|_|true).unwrap();
        assert_eq!(status.state,AssetReuseState::RecordedChanged);
        assert!(status.record.is_some());
        assert_eq!(ledger.resolve_for_reuse("lu","NSIS","v1",|_|true)
            .unwrap().state,AssetReuseState::NotRecorded);
        drop(ledger);fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn refuses_unverified_source_wrong_digest_conflicting_identity_and_symlink(){
        let root=fixture();fs::create_dir_all(&root).unwrap();
        let package=root.join("engine.zip");fs::write(&package,b"engine binary").unwrap();
        let ledger=OperationalAssetLedger::open(&root.join("assets.sqlite")).unwrap();
        let mut input=proposal(package.clone());
        assert_eq!(ledger.record_verified(&input,|_|false).unwrap_err(),AssetError::SourceDenied);
        input.expected_sha256=Some("0".repeat(64));
        assert_eq!(ledger.record_verified(&input,|_|true).unwrap_err(),AssetError::Changed);
        input.expected_sha256=None;
        let old=ledger.record_verified(&input,|_|true).unwrap();
        input.logical_name="different package".into();
        assert_eq!(ledger.record_verified(&input,|_|true).unwrap_err(),AssetError::IdentityConflict);
        assert_eq!(ledger.revalidate(&old),AssetPresence::PresentVerified);
        drop(ledger);fs::remove_dir_all(root).unwrap();
    }
}
