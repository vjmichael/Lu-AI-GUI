//! MEMORY-LOOP-03D/03E: reviewed original-turn index with crash-publication fence.
//!
//! This does NOT migrate/copy APPLIED effects into the general chat journal.
//! Every historical read reopens the exact real per-turn SQLite journal,
//! revalidates the raw receipt, oracle method, source anchor and original
//! authenticated human turn. A model supplies no filenames or permissions.
//! Historical evidence is *always* NOT_RECHECKED and NEVER a tool grant.
use crate::chat_store::{self,ChatStore};
use lu_core::{
    ansf_belief_revision::{Claim,ClaimReceipt},
    ansf_verified_procedures::ProcedureCandidate,
    continuity::{ContinuityStore,TaskCheckpoint,VerifiedEffectEvidence},
    journal::{EffectJournal,EffectState},
    supervised_tool_generation::{NativeTurnRecord,NativeTurnReport},
};
use rusqlite::{params,Connection,OpenFlags,OptionalExtension,TransactionBehavior};
use sha2::{Digest,Sha256};
use serde_json::Value;
use std::{fs,path::{Path,PathBuf},time::{SystemTime,UNIX_EPOCH}};

const INDEX_NAME:&str="lu-reviewed-memory-evidence.sqlite";
const MAX_DB_BYTES:u64=128*1024*1024;
const MAX_EFFECT_BYTES:usize=16_384;
const PROJECT:&str="lu";
const APP_VERSION:&str="0.1";
const TOOL_VERSION:&str="1.0";
#[derive(Clone,Copy)]
struct Operation {
    name:&'static str,tool:&'static str,suffix:&'static str,
    task:&'static str,subject:&'static str,predicate:&'static str,
}
const OPERATIONS:[Operation;2]=[
    Operation{name:"lu_reviewed_git_head",tool:"lu.reviewed.git.head",
        suffix:"gh",task:"review_git_head",subject:"git",predicate:"head"},
    Operation{name:"lu_reviewed_media_facts",tool:"lu.reviewed.media.facts",
        suffix:"fm",task:"review_media_facts",subject:"media",predicate:"duration_ms"}
];
fn op_for_name(name:&str)->Option<Operation>{
    OPERATIONS.into_iter().find(|o|o.name==name)
}
fn op_for_target(task:&str,tool:Option<&str>,subject:Option<&str>,predicate:Option<&str>)
    ->Option<Operation>{
    OPERATIONS.into_iter().find(|o|o.task==task&&
        tool.is_none_or(|v|v==o.tool)&&
        subject.is_none_or(|v|v==o.subject)&&
        predicate.is_none_or(|v|v==o.predicate))
}
fn safe_id(s:&str)->bool{
    !s.is_empty()&&s.len()<=128&&
    s.bytes().all(|b|b.is_ascii_alphanumeric()||matches!(b,b'-'|b'_'|b'.'))
}
fn turn_id(s:&str)->bool{
    s.len()==34&&s.starts_with("g-")&&s[2..].bytes()
        .all(|b|b.is_ascii_digit()||(b'a'..=b'f').contains(&b))
}
fn hex_sha(s:&str)->bool{
    s.len()==64&&s.bytes().all(|b|b.is_ascii_digit()||(b'a'..=b'f').contains(&b))
}
fn digest(b:&[u8])->String{format!("{:x}",Sha256::digest(b))}
fn err<E:std::fmt::Debug>(e:E)->String{format!("{e:?}")}
fn regular_file(path:&Path)->Result<(),String>{
    let meta=fs::symlink_metadata(path).map_err(err)?;
    if meta.file_type().is_symlink()||!meta.file_type().is_file()||
        meta.len()==0||meta.len()>MAX_DB_BYTES {
        return Err("Reviewed evidence source is not a bounded regular file".into())
    }
    Ok(())
}
fn profile_root(root:&Path)->Result<PathBuf,String>{
    let canonical=fs::canonicalize(root).map_err(err)?;
    if !fs::symlink_metadata(root).map_err(err)?.is_dir()
        ||fs::symlink_metadata(root).map_err(err)?.file_type().is_symlink(){
        return Err("Reviewed evidence profile is not a real owned directory".into())
    }
    Ok(canonical)
}
fn index_path(root:&Path)->PathBuf{root.join(INDEX_NAME)}
fn journal_path(root:&Path,turn:&str)->Result<PathBuf,String>{
    if !turn_id(turn){return Err("Reviewed original turn is malformed".into())}
    Ok(root.join(format!("lu-reviewed-effects-{turn}.sqlite")))
}
fn read_real_journal(root:&Path,turn:&str)->Result<EffectJournal,String>{
    let path=journal_path(root,turn)?;
    regular_file(&path)?;
    // The host never accepts a file path from the user/model; SQLite is the
    // ORIGINAL per-turn journal and must already exist before a read.
    EffectJournal::open(&path).map_err(err)
}
fn ensure_schema(conn:&Connection)->Result<(),String>{
    conn.execute_batch("
        PRAGMA journal_mode=WAL;
        PRAGMA synchronous=FULL;
        CREATE TABLE IF NOT EXISTS lu_reviewed_memory_evidence(
          call_id TEXT PRIMARY KEY,
          chat_id TEXT NOT NULL,
          user_turn_id TEXT NOT NULL,
          original_source_entry_id INTEGER NOT NULL,
          original_source_sha256 TEXT NOT NULL,
          checkpoint_id TEXT NOT NULL,
          project_id TEXT NOT NULL,
          task_id TEXT NOT NULL,
          tool_id TEXT NOT NULL,
          tool_version TEXT NOT NULL,
          application_version TEXT NOT NULL,
          receipt_sha256 TEXT NOT NULL,
          verified_fact TEXT NOT NULL,
          registered_unix INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS lu_reviewed_memory_lookup
          ON lu_reviewed_memory_evidence(
            chat_id,project_id,task_id,tool_id,registered_unix DESC);
        -- A verified receipt locator alone is not a completed ANSF projection.
        -- Legacy/crash-interrupted rows have no publication marker.
        CREATE TABLE IF NOT EXISTS lu_reviewed_memory_publication(
          call_id TEXT PRIMARY KEY,
          published_unix INTEGER NOT NULL
        );
    ").map_err(err)
}
fn require_published(conn:&Connection,call:&str)->Result<(),String>{
    let mark:Option<i64>=conn.query_row(
        "SELECT published_unix FROM lu_reviewed_memory_publication WHERE call_id=?1",
        [call],|r|r.get(0)
    ).optional().map_err(err)?;
    if mark.is_none_or(|v|v<=0){
        return Err("Reviewed evidence projection incomplete; operator reconciliation required".into())
    }
    Ok(())
}
struct Row {
    call:String,chat:String,turn:String,source_id:i64,
    source_sha:String,checkpoint:String,project:String,task:String,
    tool:String,version:String,app:String,receipt_sha:String,
    fact:String,
}
fn read_row(r:&rusqlite::Row<'_>)->rusqlite::Result<Row>{
    Ok(Row{call:r.get(0)?,chat:r.get(1)?,turn:r.get(2)?,
        source_id:r.get(3)?,source_sha:r.get(4)?,
        checkpoint:r.get(5)?,project:r.get(6)?,task:r.get(7)?,
        tool:r.get(8)?,version:r.get(9)?,app:r.get(10)?,
        receipt_sha:r.get(11)?,fact:r.get(12)?})
}
fn verify_row(chats:&ChatStore,continuity:&ContinuityStore,root:&Path,r:&Row)->Result<EffectJournal,String>{
    if !safe_id(&r.call)||!safe_id(&r.chat)||!turn_id(&r.turn)
       ||!safe_id(&r.checkpoint)||r.source_id<=0
       ||!hex_sha(&r.source_sha)||!hex_sha(&r.receipt_sha)
       ||r.project!=PROJECT||r.version!=TOOL_VERSION||r.app!=APP_VERSION
       ||r.fact.len()>512||r.fact.is_empty(){
        return Err("Malformed reviewed memory receipt index".into())
    }
    let op=op_for_target(&r.task,Some(&r.tool),None,None)
        .ok_or("Unknown reviewed source operation")?;
    let book=chats.require(&r.chat)?;
    let (text,src)=book.authenticated_chat_turn(&r.chat,&r.turn)
        .map_err(err)?.ok_or("Original reviewed human turn missing")?;
    if src!=r.source_id||digest(text.as_bytes())!=r.source_sha{
        return Err("Original human source revision does not match".into())
    }
    // E1-P15's existing read-only journal inspector independently binds
    // this per-turn task/effect pair to the exact original human chat.
    // A valid standalone receipt cannot launder another task's identity.
    let origin=crate::reviewed_task_continuity::reviewed_turn_continuity(
        root,&r.chat,&r.turn
    )?.ok_or("Original task/effect journal pair missing")?;
    if !origin.origin_bound{
        return Err("Original reviewed human task origin is unbound".into())
    }
    let journal=read_real_journal(root,&r.turn)?;
    let record=journal.get(&r.call).map_err(err)?
        .ok_or("Original worker effect missing")?;
    if record.tool_id!=op.tool||record.state!=EffectState::Applied
        ||record.receipt.as_deref().is_none_or(|b|b.is_empty()||
            b.len()>MAX_EFFECT_BYTES||digest(b)!=r.receipt_sha){
        return Err("Original worker receipt is missing or changed".into())
    }
    if !journal.independent_verified_receipt(
        &r.call,&r.tool,&r.receipt_sha).map_err(err)?{
        return Err("Independent source oracle is no longer verified".into())
    }
    let raw:Value=serde_json::from_slice(record.receipt.as_deref().unwrap())
        .map_err(err)?;
    let observed=if op.suffix=="gh"{
        raw["headCommit"].as_str().filter(|s|s.len()==40&&s.bytes()
            .all(|b|b.is_ascii_digit()||(b'a'..=b'f').contains(&b)))
            .ok_or("Reviewed Git HEAD receipt malformed")?.to_string()
    }else{
        raw["durationMilliseconds"].as_u64()
            .filter(|n|*n<=u32::MAX as u64)
            .ok_or("Reviewed media duration receipt malformed")?.to_string()
    };
    if observed!=r.fact{return Err("Indexed reviewed source fact mismatch".into())}
    let linked=continuity.verified_effects_for_checkpoint(&r.checkpoint,&journal)
        .map_err(err)?;
    if !linked.iter().any(|e|e.call_id==r.call&&e.tool_id==r.tool
        &&e.receipt_sha256==r.receipt_sha){
        return Err("Indexed checkpoint lacks the original verified effect".into())
    }
    Ok(journal)
}
/// Only the host records a reviewed operation *after* real worker execution,
/// independent factual source verification, and an original authenticated
/// human request. No model- or memory-facing write API is registered.
pub(crate) fn record_from_reviewed_turn(
    chats:&ChatStore,continuity:&ContinuityStore,profile:&Path,
    chat:&str,turn:&str,effects:&EffectJournal,
    result:&NativeTurnReport
)->Result<usize,String>{
    if !safe_id(chat)||!turn_id(turn)||result.tool_calls==0{
        return Err("No authenticated reviewed turn to index".into())
    }
    let book=chats.require(chat)?;
    let (original_text,source_id)=book.authenticated_chat_turn(chat,turn)
        .map_err(err)?.ok_or("Reviewed effect has no original authenticated user")?;
    let root=profile_root(profile)?;
    let known=journal_path(&root,turn)?;
    regular_file(&known)?;
    // The journal used by this reviewed turn is the SAME per-turn file to
    // which E1 actually wrote, not a shadow journal opened by the model.
    let actual=read_real_journal(&root,turn)?;
    let index=index_path(&root);
    if let Ok(meta)=fs::symlink_metadata(&index){
        if meta.file_type().is_symlink()||!meta.is_file()||
            meta.len()>MAX_DB_BYTES{
            return Err("Reviewed memory index is not a safe local database".into())
        }
    }
    let mut conn=Connection::open(&index).map_err(err)?;
    ensure_schema(&conn)?;
    let mut recorded=0;
    for record in &result.records{
        let NativeTurnRecord::ToolFeedback(value)=record else{continue};
        let Some(op)=value["name"].as_str().and_then(op_for_name) else{continue};
        // UNSUPPORTED or CONFLICT is never promoted to verified memory.
        if value["response"]["result"]["independentOutcome"]["status"]!="VERIFIED"{
            continue
        }
        let outer=value["call_id"].as_str().filter(|s|safe_id(s))
            .ok_or("Reviewed source call identity missing")?;
        let inner=format!("{outer}-{}",op.suffix);
        if !safe_id(&inner){return Err("Reviewed effect identity overflow".into())}
        let receipt=effects.get(&inner).map_err(err)?
            .ok_or("Actual reviewed inner effect missing")?;
        let bytes=receipt.receipt.as_deref()
            .filter(|b|!b.is_empty()&&b.len()<=MAX_EFFECT_BYTES)
            .ok_or("Native source receipt missing")?;
        let receipt_sha=digest(bytes);
        if receipt.state!=EffectState::Applied||receipt.tool_id!=op.tool||
            !effects.independent_verified_receipt(&inner,op.tool,&receipt_sha).map_err(err)?{
            return Err("Native source failed independent oracle recheck".into())
        }
        let source:Value=serde_json::from_slice(bytes).map_err(err)?;
        let fact=if op.suffix=="gh"{
            source["headCommit"].as_str().filter(|s|s.len()==40&&s.bytes()
                .all(|b|b.is_ascii_digit()||(b'a'..=b'f').contains(&b)))
                .ok_or("Malformed Git oracle source")?.to_owned()
        }else{
            source["durationMilliseconds"].as_u64()
                .filter(|n|*n<=u32::MAX as u64)
                .ok_or("Malformed media oracle source")?.to_string()
        };
        // Reopen through the original path to catch wrong journal selection.
        let original_effect=actual.get(&inner).map_err(err)?
            .ok_or("Reviewed turn is bound to a different SQLite journal")?;
        if original_effect.tool_id!=op.tool||
            original_effect.receipt.as_deref()!=Some(bytes){
            return Err("Reviewed effect was not written to its original journal".into())
        }
        let prior:Option<Row>=conn.query_row(
            "SELECT call_id,chat_id,user_turn_id,original_source_entry_id,
                    original_source_sha256,checkpoint_id,project_id,task_id,
                    tool_id,tool_version,application_version,receipt_sha256,
                    verified_fact FROM lu_reviewed_memory_evidence WHERE call_id=?1",
            [&inner],read_row
        ).optional().map_err(err)?;
        if let Some(indexed)=prior{
            let expected_source=digest(original_text.as_bytes());
            if indexed.chat!=chat||indexed.turn!=turn||indexed.source_id!=source_id
                ||indexed.source_sha!=expected_source
                ||indexed.tool!=op.tool||indexed.fact!=fact
                ||indexed.receipt_sha!=receipt_sha{
                return Err("Previously indexed reviewed evidence was rebound".into())
            }
            let valid=verify_row(chats,continuity,&root,&indexed)?;
            drop(valid);
            // Do not auto-republish an interrupted or old 03D row;
            // that could resurrect a later human correction.
            require_published(&conn,&indexed.call)?;
            recorded+=1;
            continue
        }
        let previous:Option<String>=conn.query_row(
            "SELECT verified_fact FROM lu_reviewed_memory_evidence
             WHERE chat_id=?1 AND project_id=?2 AND task_id=?3 AND tool_id=?4
             ORDER BY rowid DESC LIMIT 1",
            params![chat,PROJECT,op.task,op.tool],|r|r.get(0)
        ).optional().map_err(err)?;
        let source_changed=previous.as_ref().is_some_and(|old|old!=&fact);
        let cp=format!("cp-03d-{}",digest(inner.as_bytes()));
        let source_sha=digest(original_text.as_bytes());
        let evidence=TaskCheckpoint{
            checkpoint_id:cp.clone(),project_id:PROJECT.into(),task_id:op.task.into(),
            conversation_id:chat.to_owned(),source_entry_ids:vec![source_id],
            completed_work:format!("Independent reviewed {} fact at observation",op.tool),
            next_action:if source_changed{
                "Human review of changed original source required".into()
            }else{
                "Historical source evidence only; fresh execution requires approval".into()
            },
            blockers:if source_changed{
                vec!["Reviewed source fact differs from prior observation".into()]
            }else{vec![]}
        };
        continuity.append_verified(&evidence,&book).map_err(err)?;
        let linked=continuity.link_verified_effect(&cp,&inner,&actual)
            .map_err(err)?;
        if linked.receipt_sha256!=receipt_sha{
            return Err("Original checkpoint and journal have different receipts".into())
        }
        let now=SystemTime::now().duration_since(UNIX_EPOCH)
            .map_err(err)?.as_secs();
        let tx=conn.transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(err)?;
        tx.execute(
            "INSERT OR IGNORE INTO lu_reviewed_memory_evidence
             (call_id,chat_id,user_turn_id,original_source_entry_id,
              original_source_sha256,checkpoint_id,project_id,task_id,
              tool_id,tool_version,application_version,receipt_sha256,
              verified_fact,registered_unix)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
            params![inner,chat,turn,source_id,source_sha,cp,
                PROJECT,op.task,op.tool,TOOL_VERSION,APP_VERSION,
                receipt_sha,fact,now as i64]
        ).map_err(err)?;
        let indexed:Row=tx.query_row(
            "SELECT call_id,chat_id,user_turn_id,original_source_entry_id,
                    original_source_sha256,checkpoint_id,project_id,task_id,
                    tool_id,tool_version,application_version,receipt_sha256,
                    verified_fact FROM lu_reviewed_memory_evidence WHERE call_id=?1",
            [&inner],read_row
        ).map_err(err)?;
        if indexed.chat!=chat||indexed.turn!=turn||indexed.source_id!=source_id||
            indexed.source_sha!=source_sha||indexed.checkpoint!=cp||
            indexed.call!=inner||indexed.tool!=op.tool||indexed.fact!=fact||
            indexed.receipt_sha!=receipt_sha{
            return Err("Reviewed evidence index already bound to different source".into())
        }
        tx.commit().map_err(err)?;
        // Source-backed derived projection; old permissions are NOT stored.
        // A changed value is intentionally blocked until human review and
        // never silently supersedes the old human-understood procedure.
        if !source_changed{
            let verify=|e:&VerifiedEffectEvidence|actual
                .independent_verified_receipt(
                    &e.call_id,&e.tool_id,&e.receipt_sha256).unwrap_or(false);
            book.promote_verified_procedure(&ProcedureCandidate{
                procedure_id:format!("proc-03d-{}",digest(inner.as_bytes())),
                project_id:PROJECT.into(),task_id:op.task.into(),session_id:chat.into(),
                source_entry_id:source_id,checkpoint_id:cp.clone(),
                call_id:inner.clone(),tool_id:op.tool.into(),
                tool_version:TOOL_VERSION.into(),application_version:APP_VERSION.into(),
                title:format!("Review historical {} observation",op.tool),
                prerequisites:vec!["Current source and fresh human grant required".into()],
                steps:vec!["Read historical result without executing any worker".into()],
                failure_conditions:vec!["Source or tool changed; request review".into()],
                replaces_id:None
            },continuity,&actual,verify).map_err(err)?;
            book.record_verified_claim(&Claim{
                claim_id:format!("claim-03d-{}",digest(inner.as_bytes())),
                session_id:chat.into(),source_entry_id:source_id,
                project_id:PROJECT.into(),task_id:op.task.into(),
                subject_key:op.subject.into(),predicate_key:op.predicate.into(),
                value:fact,application_version:APP_VERSION.into(),
                expires_unix:now+86_400,supersedes_id:None,
            },ClaimReceipt{
                checkpoint_id:cp,call_id:inner,tool_id:op.tool.into()
            },continuity,&actual,|e:&VerifiedEffectEvidence|actual
                .independent_verified_receipt(
                    &e.call_id,&e.tool_id,&e.receipt_sha256
                ).unwrap_or(false)).map_err(err)?;
        }
        // Publish only after checkpoint, independent effect link and every
        // required M4 procedure and belief projection have committed.
        let published=SystemTime::now().duration_since(UNIX_EPOCH)
            .map_err(err)?.as_secs() as i64;
        conn.execute(
            "INSERT OR IGNORE INTO lu_reviewed_memory_publication
             (call_id,published_unix) VALUES (?1,?2)",
            params![inner,published]
        ).map_err(err)?;
        require_published(&conn,&inner)?;
        recorded+=1;
    }
    Ok(recorded)
}
/// Exact authenticated CURRENT-human selector only. The index supplies
/// a turn, not a caller/model-selected path. It never opens the wrong chat
/// and never falls back to a global journal after a recorded lookup fails.
pub(crate) fn resolve_for_current_human(
    chats:&ChatStore,continuity:&ContinuityStore,profile:&Path,
    chat:&str,current_turn:&str,prompt:&str
)->Result<Option<EffectJournal>,String>{
    if !safe_id(chat)||!turn_id(current_turn)
        ||chats.latest_authenticated_user_turn_id(chat).as_deref()!=Some(current_turn)
        ||chats.trusted_user_turn_text(chat,current_turn).as_deref()!=Some(prompt){
        return Err("Current original human turn not authenticated for reviewed memory".into())
    }
    let requested=if let Some((project,task,tool,version,app))=
        chat_store::m4_explicit_procedure_target(prompt){
        if project!=PROJECT||version!=TOOL_VERSION||app!=APP_VERSION{return Ok(None)}
        op_for_target(&task,Some(&tool),None,None)
    }else if let Some((project,task,subject,predicate,version))=
        chat_store::m4_explicit_belief_target(prompt){
        if project!=PROJECT||version!=APP_VERSION{return Ok(None)}
        op_for_target(&task,None,Some(&subject),Some(&predicate))
    }else{None};
    let Some(op)=requested else{return Ok(None)};
    let root=profile_root(profile)?;
    let path=index_path(&root);
    if !path.exists(){return Ok(None)}
    regular_file(&path)?;
    let conn=Connection::open_with_flags(&path,OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(err)?;
    let row:Option<Row>=conn.query_row(
        "SELECT call_id,chat_id,user_turn_id,original_source_entry_id,
                original_source_sha256,checkpoint_id,project_id,task_id,
                tool_id,tool_version,application_version,receipt_sha256,
                verified_fact
         FROM lu_reviewed_memory_evidence
         WHERE chat_id=?1 AND project_id=?2 AND task_id=?3 AND tool_id=?4
         ORDER BY rowid DESC LIMIT 1",
        params![chat,PROJECT,op.task,op.tool],read_row
    ).optional().map_err(err)?;
    let Some(row)=row else{return Ok(None)};
    if row.chat!=chat||row.project!=PROJECT||row.task!=op.task{
        return Err("Cross-chat reviewed evidence access refused".into())
    }
    // The newest selected record must itself be published: no fallback.
    require_published(&conn,&row.call)?;
    verify_row(chats,continuity,&root,&row).map(Some)
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory_loop03b_context::for_current_human_turn;
    use lu_core::{
        contracts::{ToolInvocation,ProtoPayload},
        journal::IndependentOutcomeVerdict,
    };
    use std::time::{SystemTime,UNIX_EPOCH};

    fn tmp()->PathBuf{
        std::env::temp_dir().join(format!(
            "lu-m03d-index-{}-{}",std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ))
    }
    fn turn(i:u32)->String{format!("g-{i:032x}")}
    fn journal_with_git(root:&Path,chat:&str,t:&str,outer:&str,head:&str,independent:bool)
        ->EffectJournal{
        let task=lu_core::tasks::TaskStore::open(
            &root.join(format!("lu-reviewed-task-{t}.sqlite"))
        ).unwrap();
        task.bind_reviewed_human_turn(chat,t).unwrap();
        let path=journal_path(root,t).unwrap();
        let journal=EffectJournal::open(&path).unwrap();
        let call=format!("{outer}-gh");
        journal.prepare(&ToolInvocation{
            call_id:call.clone(),tool_id:"lu.reviewed.git.head".into(),
            workspace_id:Some(t.into()),
            input:ProtoPayload{
                type_url:"lu.memory03d.TestOnlyFixture".into(),
                bytes:b"NOT_AN_OS_EXECUTION".to_vec()
            }
        }).unwrap();
        journal.mark_executing(&call).unwrap();
        let bytes=serde_json::to_vec(&serde_json::json!({
            "callId":call,"headCommit":head
        })).unwrap();
        journal.mark_applied(&call,&bytes).unwrap();
        if independent{
            journal.record_independent_outcome(
                &call,&bytes,IndependentOutcomeVerdict::Verified,
                "GIT_HEAD_DIRECT_V1",Some(&digest(head.as_bytes()))
            ).unwrap();
        }
        journal
    }
    fn reported(outer:&str,head:&str)->NativeTurnReport{
        NativeTurnReport{
            answer:"Historical worker result only".into(),
            model_generations:2,tool_calls:1,host_routed_calls:0,
            records:vec![NativeTurnRecord::ToolFeedback(serde_json::json!({
                "name":"lu_reviewed_git_head",
                "call_id":outer,
                "response":{"result":{
                    "headCommit":head,
                    "independentOutcome":{"status":"VERIFIED"}
                }}
            }))]
        }
    }
    fn recalled(chats:&ChatStore,continuity:&ContinuityStore,
        root:&Path,chat:&str,i:u32,prompt:&str)->Value{
        let t=turn(i);
        chats.append_user_with_turn_id(chat,&t,prompt).unwrap();
        let original=resolve_for_current_human(chats,continuity,root,chat,&t,prompt)
            .unwrap().expect("read source selected from registered original turn");
        let raw=for_current_human_turn(chats,continuity,&original,
            chat,&t,prompt,"CURRENT_GOAL_REVIEWABLE_ONLY",
            "NO_REVIEWED_EFFECTS_IN_BOUNDED_WINDOW"
        ).unwrap().unwrap();
        let v:Value=serde_json::from_str(&raw).unwrap();
        assert_eq!(v["executionAuthority"],"NOT_AUTHORIZED_FOR_EXECUTION");
        assert_eq!(v["priorGrantReusable"],false);
        assert_eq!(v["automaticRetryAllowed"],false);
        assert_eq!(v["currentSourceFreshness"],"NOT_RECHECKED");
        v
    }

    #[test]
    fn indexed_original_worker_receipt_is_readable_after_restart_without_grant(){
        // These test journal bytes are synthetic: only G5's ignored
        // Windows/Linux OS test demonstrates a genuine reviewed worker.
        let root=tmp();fs::create_dir_all(&root).unwrap();
        let chats=ChatStore::open(&root).unwrap();
        let chat=chats.create_chat().unwrap().chat_id;
        let original=turn(1);
        chats.append_user_with_turn_id(&chat,&original,
            "Authorize reviewed Git HEAD observation").unwrap();
        let journal=journal_with_git(&root,&chat,&original,"case-one",&"a".repeat(40),true);
        let continuity=ContinuityStore::open(&root.join("continuity.sqlite")).unwrap();
        let report=reported("case-one",&"a".repeat(40));
        assert_eq!(record_from_reviewed_turn(
            &chats,&continuity,&root,&chat,&original,&journal,&report
        ).unwrap(),1);
        // At-least-once completion persistence does not duplicate history.
        assert_eq!(record_from_reviewed_turn(
            &chats,&continuity,&root,&chat,&original,&journal,&report
        ).unwrap(),1);
        let select="Recall procedure project lu task review_git_head tool lu.reviewed.git.head tool-version 1.0 application-version 0.1";
        let inspected=recalled(&chats,&continuity,&root,&chat,2,select);
        assert_eq!(inspected["status"],"HISTORICAL_SOURCE_ORACLE_VERIFIED_PROCEDURE");
        assert_eq!(inspected["historicallyVerifiedDataAvailable"],true);
        assert_eq!(inspected["humanReviewRequired"],false);
        drop(journal);drop(chats);drop(continuity);
        let again=ChatStore::open(&root).unwrap();
        let cp=ContinuityStore::open(&root.join("continuity.sqlite")).unwrap();
        let belief="Recall belief project lu task review_git_head subject git predicate head version 0.1";
        let value=recalled(&again,&cp,&root,&chat,3,belief);
        assert_eq!(value["status"],"HISTORICAL_SOURCE_ORACLE_VERIFIED_BELIEF");
        assert_eq!(value["historicalEvidence"]["historicalAssessedClaims"][0]["reportedValue"],"a".repeat(40));
        let other=again.create_chat().unwrap().chat_id;
        again.append_user_with_turn_id(&other,&turn(99),select).unwrap();
        assert!(resolve_for_current_human(
            &again,&cp,&root,&other,&turn(99),select).unwrap().is_none());
        assert!(resolve_for_current_human(
            &again,&cp,&root,&chat,&turn(888),select).is_err());
        drop(cp);drop(again);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn changed_source_becomes_ccm_review_and_historical_grant_never_replays(){
        let root=tmp();fs::create_dir_all(&root).unwrap();
        let chats=ChatStore::open(&root).unwrap();
        let chat=chats.create_chat().unwrap().chat_id;
        let t1=turn(1);
        chats.append_user_with_turn_id(&chat,&t1,"First original Git request").unwrap();
        let first=journal_with_git(&root,&chat,&t1,"earlier",&"a".repeat(40),true);
        let continuity=ContinuityStore::open(&root.join("continuity.sqlite")).unwrap();
        assert_eq!(record_from_reviewed_turn(
            &chats,&continuity,&root,&chat,&t1,&first,&reported("earlier",&"a".repeat(40))
        ).unwrap(),1);
        let t2=turn(2);
        chats.append_user_with_turn_id(&chat,&t2,
            "Human authorized a separate reviewed Git operation").unwrap();
        let newer=journal_with_git(&root,&chat,&t2,"later",&"b".repeat(40),true);
        assert_eq!(record_from_reviewed_turn(
            &chats,&continuity,&root,&chat,&t2,&newer,&reported("later",&"b".repeat(40))
        ).unwrap(),1);
        let change=chats.monitor_project_context(
            &chat,"lu","review_git_head",&continuity
        ).unwrap().expect("two source-backed checkpoints");
        assert!(change.needs_review&&change.source_evidence_changed);
        let selector="Recall procedure project lu task review_git_head tool lu.reviewed.git.head tool-version 1.0 application-version 0.1";
        let v=recalled(&chats,&continuity,&root,&chat,3,selector);
        assert_eq!(v["status"],"NO_CURRENTLY_REVALIDATED_VERIFIED_PROCEDURE");
        assert_eq!(v["historicallyVerifiedDataAvailable"],false);
        assert!(v["humanReviewRequired"]);
        let belief="Recall belief project lu task review_git_head subject git predicate head version 0.1";
        let b=recalled(&chats,&continuity,&root,&chat,4,belief);
        assert_ne!(b["status"],"HISTORICAL_SOURCE_ORACLE_VERIFIED_BELIEF");
        assert_eq!(b["executionAuthority"],"NOT_AUTHORIZED_FOR_EXECUTION");
        drop(newer);drop(first);drop(chats);drop(continuity);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn absent_corrupt_or_replaced_originals_fail_closed_not_global_journal(){
        let root=tmp();fs::create_dir_all(&root).unwrap();
        let chats=ChatStore::open(&root).unwrap();
        let chat=chats.create_chat().unwrap().chat_id;
        let t1=turn(1);
        chats.append_user_with_turn_id(&chat,&t1,"Host approved Git source").unwrap();
        let j=journal_with_git(&root,&chat,&t1,"case-corrupt",&"a".repeat(40),true);
        let cp=ContinuityStore::open(&root.join("continuity.sqlite")).unwrap();
        record_from_reviewed_turn(
            &chats,&cp,&root,&chat,&t1,&j,&reported("case-corrupt",&"a".repeat(40))
        ).unwrap();
        let selector="Recall procedure project lu task review_git_head tool lu.reviewed.git.head tool-version 1.0 application-version 0.1";
        chats.append_user_with_turn_id(&chat,&turn(2),selector).unwrap();
        let conn=Connection::open(index_path(&root)).unwrap();
        conn.execute("UPDATE lu_reviewed_memory_evidence SET receipt_sha256=?1",
            ["f".repeat(64)]).unwrap();
        assert!(resolve_for_current_human(
            &chats,&cp,&root,&chat,&turn(2),selector).is_err());
        conn.execute("UPDATE lu_reviewed_memory_evidence SET receipt_sha256=?1",
            [digest(j.get("case-corrupt-gh").unwrap().unwrap().receipt.as_deref().unwrap())]
        ).unwrap();
        assert!(resolve_for_current_human(
            &chats,&cp,&root,&chat,&turn(2),selector).unwrap().is_some());
        drop(conn);drop(j);
        // The path is fixed by the host original turn: no fallback to a
        // different journal or an on-disk APPLIED copy is permitted.
        fs::remove_file(journal_path(&root,&t1).unwrap()).unwrap();
        assert!(resolve_for_current_human(
            &chats,&cp,&root,&chat,&turn(2),selector).is_err());
        drop(cp);drop(chats);
        fs::remove_dir_all(root).unwrap();
    }


    #[test]
    fn interrupted_projection_and_forged_checkpoint_cannot_reach_chat(){
        let root=tmp();fs::create_dir_all(&root).unwrap();
        let chats=ChatStore::open(&root).unwrap();
        let chat=chats.create_chat().unwrap().chat_id;
        let original=turn(1);
        chats.append_user_with_turn_id(&chat,&original,
            "Original human approved Git source read").unwrap();
        let journal=journal_with_git(&root,&chat,&original,"crash-proof",&"a".repeat(40),true);
        let cp=ContinuityStore::open(&root.join("continuity.sqlite")).unwrap();
        let report=reported("crash-proof",&"a".repeat(40));
        record_from_reviewed_turn(&chats,&cp,&root,&chat,&original,&journal,&report)
            .unwrap();
        let current=turn(2);
        let selector="Recall procedure project lu task review_git_head tool lu.reviewed.git.head tool-version 1.0 application-version 0.1";
        chats.append_user_with_turn_id(&chat,&current,selector).unwrap();
        let index=Connection::open(index_path(&root)).unwrap();
        index.execute("DELETE FROM lu_reviewed_memory_publication WHERE call_id=?1",
            ["crash-proof-gh"]).unwrap();
        let unavailable=resolve_for_current_human(
            &chats,&cp,&root,&chat,&current,selector
        ).err().expect("unpublished original must fail");
        assert!(unavailable.contains("projection incomplete"));
        // Replaying the old report is not a fresh authorization or repair.
        assert!(record_from_reviewed_turn(
            &chats,&cp,&root,&chat,&original,&journal,&report
        ).is_err());
        // Even a fabricated marker cannot substitute for the original
        // checkpoint-to-independent-effect witness.
        index.execute(
            "INSERT INTO lu_reviewed_memory_publication(call_id,published_unix)
             VALUES(?1,?2)",params!["crash-proof-gh",1i64]
        ).unwrap();
        index.execute(
            "UPDATE lu_reviewed_memory_evidence SET checkpoint_id=?1 WHERE call_id=?2",
            params!["cp-03d-wrong","crash-proof-gh"]
        ).unwrap();
        assert!(resolve_for_current_human(
            &chats,&cp,&root,&chat,&current,selector
        ).is_err());
        // Restore the original continuity link, then tamper only the host
        // TaskStore origin. The receipt and checkpoint still match, but
        // this historical source must not become available to another chat.
        let real_checkpoint=format!("cp-03d-{}",digest(b"crash-proof-gh"));
        index.execute(
            "UPDATE lu_reviewed_memory_evidence SET checkpoint_id=?1 WHERE call_id=?2",
            params![real_checkpoint,"crash-proof-gh"]
        ).unwrap();
        assert!(resolve_for_current_human(
            &chats,&cp,&root,&chat,&current,selector
        ).unwrap().is_some());
        let tasks=Connection::open(root.join(format!(
            "lu-reviewed-task-{original}.sqlite"
        ))).unwrap();
        tasks.execute(
            "UPDATE lu_reviewed_turn_origin SET conversation_id=?1",
            [format!("c-{}","f".repeat(32))]
        ).unwrap();
        assert!(resolve_for_current_human(
            &chats,&cp,&root,&chat,&current,selector
        ).is_err());
        drop(tasks);
        drop(index);drop(journal);drop(cp);drop(chats);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_model_like_result_without_oracle_cannot_be_recorded(){
        let root=tmp();fs::create_dir_all(&root).unwrap();
        let chats=ChatStore::open(&root).unwrap();
        let chat=chats.create_chat().unwrap().chat_id;
        let t1=turn(1);
        chats.append_user_with_turn_id(&chat,&t1,"Reviewed Git consent").unwrap();
        let j=journal_with_git(&root,&chat,&t1,"no-oracle",&"a".repeat(40),false);
        let cp=ContinuityStore::open(&root.join("continuity.sqlite")).unwrap();
        assert!(record_from_reviewed_turn(
            &chats,&cp,&root,&chat,&t1,&j,&reported("no-oracle",&"a".repeat(40))
        ).is_err());
        assert!(!index_path(&root).exists()||
            Connection::open(index_path(&root)).unwrap().query_row(
                "SELECT COUNT(*) FROM lu_reviewed_memory_evidence",
                [],|r|r.get::<_,i64>(0)).unwrap()==0);
        drop(j);drop(cp);drop(chats);fs::remove_dir_all(root).unwrap();
    }
}
