//! P0-AUTH: immutable host-only cancellation/recovery intents in the
//! EXISTING ANSF Library SQLite. No new worker, planner, model grant or DB.
//! Requests are written BEFORE stop attempts; missing outcomes after crash
//! remain uncertain and can NEVER be replayed as a cancellation action.
use rusqlite::{params,Connection};
use serde::{Serialize,Deserialize};
use sha2::{Digest,Sha256};
use std::collections::BTreeMap;

pub(crate) const SCHEMA:&str="lu-p0-auth-host-cancel-v1";
const MAX_EVENT_BYTES:usize=4096;
const MAX_HISTORY:usize=32;
const DOMAIN_NAMES:&[&str]=&[
    "model","browser","reader","ocr","observe","scheduled","background"
];
const STATUS_NAMES:&[&str]=&[
    "NOT_PRESENT","CANCEL_REQUESTED_NOT_REAPED",
    "NATIVE_STOP_RETURNED_CUSTODY_RECHECK_REQUIRED",
    "NATIVE_TERMINATION_REAP_RETURNED",
    "STOP_UNCERTAIN_RECONCILE","SCHEDULE_CANCEL_RECORDED",
    "SCHEDULE_CANCEL_UNCERTAIN","MODEL_CANCELLED_QUEUE_ONLY",
    "OWNER_CHANGED_RECONCILE","OBSERVE_PERMISSION_REVOKED",
];
fn digest(bytes:&[u8])->String{format!("{:x}",Sha256::digest(bytes))}
fn ident(s:&str,prefix:&str)->bool{
    s.len()==prefix.len()+32&&s.starts_with(prefix)&&
        s[prefix.len()..].bytes().all(|b|b.is_ascii_hexdigit()&&!b.is_ascii_uppercase())
}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct DomainResult{
    pub name:String,
    pub state:String,
}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct HostCancelEvent{
    pub schema:String,
    pub attempt_id:String,
    pub chat_id:String,
    pub phase:String,
    pub domains:Vec<DomainResult>,
    pub authority_restored:bool,
    pub automatic_retry_allowed:bool,
    pub os_custody_independently_verified:bool,
}
#[derive(Clone,Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct HostCancelHistory{
    pub attempt_id:String,
    pub status:String,
    pub domains:Vec<DomainResult>,
    pub source_sha256:String,
    pub authority_restored:bool,
    pub automatic_retry_allowed:bool,
    pub os_custody_independently_verified:bool,
}
fn valid(e:&HostCancelEvent)->bool{
    if e.schema!=SCHEMA||!ident(&e.attempt_id,"au-")||
        !ident(&e.chat_id,"c-")||
        !matches!(e.phase.as_str(),"REQUESTED"|"RESULT")||
        e.authority_restored||e.automatic_retry_allowed||
        e.os_custody_independently_verified||e.domains.len()>DOMAIN_NAMES.len(){
        return false
    }
    if e.phase=="REQUESTED"{return e.domains.is_empty()}
    let mut known=std::collections::BTreeSet::new();
    !e.domains.is_empty()&&e.domains.iter().all(|d|
        DOMAIN_NAMES.contains(&d.name.as_str())&&
        STATUS_NAMES.contains(&d.state.as_str())&&known.insert(d.name.as_str())
    )
}
pub(crate) fn initialize(conn:&Connection)->Result<(),String>{
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS ansf_p0_host_cancel_events(
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            attempt_id TEXT NOT NULL,chat_id TEXT NOT NULL
               REFERENCES ansf_desktop_chats(chat_id) ON DELETE CASCADE,
            phase TEXT NOT NULL CHECK(phase IN ('REQUESTED','RESULT')),
            original_json BLOB NOT NULL,
            original_sha256 TEXT NOT NULL,
            UNIQUE(attempt_id,phase)
         );
         CREATE INDEX IF NOT EXISTS ansf_p0_cancel_chat
            ON ansf_p0_host_cancel_events(chat_id,id DESC);
         CREATE TRIGGER IF NOT EXISTS ansf_p0_cancel_no_update
            BEFORE UPDATE ON ansf_p0_host_cancel_events
            BEGIN SELECT RAISE(ABORT,'Immutable original cancellation event'); END;
         CREATE TRIGGER IF NOT EXISTS ansf_p0_cancel_no_direct_delete
            BEFORE DELETE ON ansf_p0_host_cancel_events
            WHEN EXISTS(SELECT 1 FROM ansf_desktop_chats
                        WHERE chat_id=OLD.chat_id)
            BEGIN SELECT RAISE(ABORT,'Immutable original cancellation event'); END;"
    ).map_err(|_|"P0_AUTH_ANFS_CANCEL_SCHEMA_UNAVAILABLE".into())
}
/// The native original-catalog cancellation REQUESTED row is the
/// linearization point for revoked work. A RESULT alone cannot revoke
/// an operation, and an older REQUESTED must not revoke a fresh turn.
pub(crate) fn requested_watermark(
    conn:&Connection,chat:&str,
)->Result<i64,String>{
    if !ident(chat,"c-"){return Err("P0_SEC_INVALID_CHAT_SCOPE".into())}
    initialize(conn)?;
    // Never let a corrupt original REQUESTED row become a valid revocation
    // baseline. Tool dispatch calls this after each host permission check;
    // an altered row must deny execution, NOT silently restore old authority.
    let latest:Option<(i64,Vec<u8>,String)>=conn.query_row(
        "SELECT id,original_json,original_sha256
         FROM ansf_p0_host_cancel_events
         WHERE chat_id=?1 AND phase='REQUESTED'
         ORDER BY id DESC LIMIT 1",
        params![chat],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?))
    ).optional().map_err(|_|"P0_SEC_CANCEL_WATERMARK_UNAVAILABLE")?;
    let Some((sequence,source,expected_sha))=latest else{return Ok(0)};
    if source.is_empty()||source.len()>MAX_EVENT_BYTES||
        digest(&source)!=expected_sha{
        return Err("P0_SEC_CANCEL_INTENT_TAMPERED".into())
    }
    let event:HostCancelEvent=serde_json::from_slice(&source)
        .map_err(|_|"P0_SEC_CANCEL_INTENT_INVALID")?;
    if !valid(&event)||event.phase!="REQUESTED"||event.chat_id!=chat{
        return Err("P0_SEC_CANCEL_INTENT_SCOPE_INVALID".into())
    }
    Ok(sequence)
}

pub(crate) fn append(conn:&Connection,event:&HostCancelEvent)
 ->Result<(),String>{
    if !valid(event){return Err("P0_AUTH_INVALID_CANCEL_EVENT".into())}
    initialize(conn)?;
    // Every result must be linked to the original persisted REQUEST,
    // preventing fake task cancellation summaries from replacing an intent.
    if event.phase=="RESULT"{
        let previous=recent(conn,&event.chat_id,MAX_HISTORY)?;
        if !previous.iter().any(|item|
            item.attempt_id==event.attempt_id&&item.status=="CANCELLATION_REQUESTED_OUTCOME_UNKNOWN"){
            return Err("P0_AUTH_CANCEL_INTENT_MISSING_OR_ALREADY_FINAL".into())
        }
    }
    let bytes=serde_json::to_vec(event)
        .map_err(|_|"P0_AUTH_CANCEL_SERIALIZATION_FAILED")?;
    if bytes.len()>MAX_EVENT_BYTES{return Err("P0_AUTH_CANCEL_RECORD_OVERSIZE".into())}
    let original_sha=digest(&bytes);
    conn.execute(
        "INSERT INTO ansf_p0_host_cancel_events(
            attempt_id,chat_id,phase,original_json,original_sha256
         ) VALUES (?1,?2,?3,?4,?5)",
        params![&event.attempt_id,&event.chat_id,&event.phase,&bytes,&original_sha]
    ).map_err(|_|"P0_AUTH_CANCEL_APPEND_FAILED")?;
    Ok(())
}
pub(crate) fn recent(conn:&Connection,chat:&str,limit:usize)
 ->Result<Vec<HostCancelHistory>,String>{
    if !ident(chat,"c-")||limit==0||limit>MAX_HISTORY{
        return Err("P0_AUTH_CANCEL_HISTORY_SCOPE_INVALID".into())
    }
    initialize(conn)?;
    // Read enough contiguous originals to validate the latest 32 attempts,
    // not just RESULT rows, and never hide incomplete newest intents.
    let mut stmt=conn.prepare(
        "SELECT original_json,original_sha256
         FROM ansf_p0_host_cancel_events WHERE chat_id=?1
         ORDER BY id DESC LIMIT ?2"
    ).map_err(|_|"P0_AUTH_CANCEL_READ_UNAVAILABLE")?;
    let mut rows=stmt.query(params![chat,(limit*2) as i64])
        .map_err(|_|"P0_AUTH_CANCEL_QUERY_REFUSED")?;
    let mut grouped=BTreeMap::<String,(Option<HostCancelEvent>,Option<HostCancelEvent>)>::new();
    let mut order=Vec::new();
    while let Some(row)=rows.next().map_err(|_|"P0_AUTH_CANCEL_ROW_UNAVAILABLE")?{
        let bytes:Vec<u8>=row.get(0).map_err(|_|"P0_AUTH_CANCEL_BYTES_MISSING")?;
        let hash:String=row.get(1).map_err(|_|"P0_AUTH_CANCEL_HASH_MISSING")?;
        if bytes.len()>MAX_EVENT_BYTES||digest(&bytes)!=hash{
            return Err("P0_AUTH_CANCEL_ORIGINAL_TAMPERED".into())
        }
        let e:HostCancelEvent=serde_json::from_slice(&bytes)
            .map_err(|_|"P0_AUTH_CANCEL_STORED_RECORD_INVALID")?;
        if !valid(&e)||e.chat_id!=chat{
            return Err("P0_AUTH_CANCEL_ORIGINAL_AUTHORITY_INVALID".into())
        }
        if !grouped.contains_key(&e.attempt_id){order.push(e.attempt_id.clone())}
        let record=grouped.entry(e.attempt_id.clone()).or_default();
        let slot=if e.phase=="REQUESTED"{&mut record.0}else{&mut record.1};
        if slot.replace(e).is_some(){return Err("P0_AUTH_CANCEL_DUPLICATE_PHASE".into())}
    }
    let mut out=Vec::new();
    for id in order.into_iter().take(limit){
        let (requested,result)=grouped.remove(&id)
            .ok_or("P0_AUTH_CANCEL_MISSING_GROUP")?;
        // A truncated old source, absent REQUEST or unmatched RESULT
        // cannot count as completed/reaped; never present positive proof.
        let state=if requested.is_none(){
            "ORIGINAL_CANCEL_INTENT_UNAVAILABLE"
        }else if result.is_none(){
            "CANCELLATION_REQUESTED_OUTCOME_UNKNOWN"
        }else{
            "CANCEL_RETURNED_REQUIRES_OS_AND_EFFECT_RECONCILIATION"
        };
        let e=result.or(requested).ok_or("P0_AUTH_CANCEL_EVENT_LOST")?;
        let original=serde_json::to_vec(&e)
            .map_err(|_|"P0_AUTH_CANCEL_HISTORY_SERIALIZATION_FAILED")?;
        out.push(HostCancelHistory{
            attempt_id:id,status:state.into(),domains:e.domains,
            source_sha256:digest(&original),authority_restored:false,
            automatic_retry_allowed:false,os_custody_independently_verified:false
        })
    }
    Ok(out)
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn cancelled_request_without_result_is_never_evidence_of_reaped_worker(){
        let conn=Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "PRAGMA foreign_keys=ON;
             CREATE TABLE ansf_desktop_chats(chat_id TEXT PRIMARY KEY);"
        ).unwrap();
        initialize(&conn).unwrap();
        let chat=format!("c-{}","1".repeat(32));
        let attempt=format!("au-{}","2".repeat(32));
        conn.execute("INSERT INTO ansf_desktop_chats(chat_id) VALUES (?1)",[&chat]).unwrap();
        let mut row=HostCancelEvent{
            schema:SCHEMA.into(),attempt_id:attempt.clone(),chat_id:chat.clone(),
            phase:"REQUESTED".into(),domains:vec![],
            authority_restored:false,automatic_retry_allowed:false,
            os_custody_independently_verified:false
        };
        append(&conn,&row).unwrap();
        assert_eq!(recent(&conn,&chat,8).unwrap()[0].status,
            "CANCELLATION_REQUESTED_OUTCOME_UNKNOWN");
        row.phase="RESULT".into();
        row.domains=vec![DomainResult{
            name:"model".into(),state:"CANCEL_REQUESTED_NOT_REAPED".into()
        }];
        append(&conn,&row).unwrap();
        assert_eq!(recent(&conn,&chat,8).unwrap()[0].status,
            "CANCEL_RETURNED_REQUIRES_OS_AND_EFFECT_RECONCILIATION");
        assert!(append(&conn,&row).is_err());
        assert!(!recent(&conn,&chat,8).unwrap()[0].automatic_retry_allowed);
    }
    #[test]
    fn tampered_hash_blocks_original_cancellation_history(){
        let conn=Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;
            CREATE TABLE ansf_desktop_chats(chat_id TEXT PRIMARY KEY);").unwrap();
        initialize(&conn).unwrap();
        let chat=format!("c-{}","3".repeat(32));
        conn.execute("INSERT INTO ansf_desktop_chats(chat_id) VALUES (?1)",[&chat]).unwrap();
        let row=HostCancelEvent{
            schema:SCHEMA.into(),attempt_id:format!("au-{}","4".repeat(32)),
            chat_id:chat.clone(),phase:"REQUESTED".into(),domains:vec![],
            authority_restored:false,automatic_retry_allowed:false,
            os_custody_independently_verified:false
        };
        append(&conn,&row).unwrap();
        conn.execute("UPDATE ansf_p0_host_cancel_events SET original_sha256=?1",
            ["f".repeat(64)]).unwrap();
        assert!(recent(&conn,&chat,4).is_err());
    }
}
