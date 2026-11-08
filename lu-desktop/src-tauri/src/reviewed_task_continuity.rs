//! E1-P15: read-only, host-owned reconstruction of a reviewed human task.
//! This joins durable ORIGINAL chat identity, per-turn task/checkpoints, inner
//! OS effect receipts and independently verified semantic outcomes.
//! No model/JS path, argv, grant, checkpoint bytes or worker text is exposed.
//! No process launch, TaskStore::resume, effect replay or SQLite write occurs.
use std::{fs,path::{Path,PathBuf},collections::BTreeMap};
use rusqlite::{Connection,OpenFlags,OptionalExtension,params};
use serde::Serialize;
use sha2::{Digest,Sha256};

const MAX_DATABASE:u64=64*1024*1024;
const MAX_ROWS:usize=256;
const MAX_RECEIPT:i64=32*1024;

#[derive(Clone,Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct ReviewedOperationContinuity{
    pub operation:&'static str,
    pub process_state:String,
    pub outcome:&'static str,
    pub last_checkpoint:Option<u8>,
    pub checkpoint_events:u32,
    pub original_model_call_linked:bool,
    pub worker_task_linked:bool,
}
#[derive(Clone,Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct ReviewedTurnContinuity{
    pub origin_bound:bool,
    pub journal_state:&'static str,
    pub continuation:&'static str,
    pub explanation:&'static str,
    pub operations:Vec<ReviewedOperationContinuity>,
}
fn valid(value:&str,prefix:&str)->bool{
    value.len()==prefix.len()+32&&value.starts_with(prefix)
        &&value[prefix.len()..].bytes().all(|b|
            b.is_ascii_hexdigit()&&!b.is_ascii_uppercase())
}
fn no_linked_file(path:&Path)->Result<Option<fs::Metadata>,String>{
    let metadata=match fs::symlink_metadata(path){
        Ok(v)=>v,
        Err(e) if e.kind()==std::io::ErrorKind::NotFound=>return Ok(None),
        Err(_)=>return Err("Reviewed task ledger inaccessible".into())
    };
    if !metadata.is_file()||metadata.file_type().is_symlink(){
        return Err("Reviewed task ledger is not an ordinary file".into())
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes()&0x400!=0{
            return Err("Linked Windows task ledger refused".into())
        }
    }
    Ok(Some(metadata))
}
fn open_readonly(path:&Path)->Result<Option<Connection>,String>{
    let Some(metadata)=no_linked_file(path)? else{return Ok(None)};
    if metadata.len()>MAX_DATABASE{
        return Err("Reviewed task ledger exceeds bounded size".into())
    }
    for suffix in ["-wal","-shm"]{
        let mut sidecar=path.as_os_str().to_os_string();
        sidecar.push(suffix);
        let _=no_linked_file(&PathBuf::from(sidecar))?;
    }
    let flags=OpenFlags::SQLITE_OPEN_READ_ONLY|OpenFlags::SQLITE_OPEN_NO_MUTEX;
    Connection::open_with_flags(path,flags).map(Some)
        .map_err(|_|"Cannot read reviewed task journal".into())
}
fn table_exists(conn:&Connection,name:&str)->Result<bool,String>{
    let exists:i64=conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
        params![name],|r|r.get(0)
    ).map_err(|_|"Reviewed journal schema unavailable")?;
    Ok(exists==1)
}
fn operation_for<'a>(call:&'a str,tool:&str)->Option<(&'static str,&'a str)>{
    const TYPES:[(&str,&str,&str);5]=[
        ("-gh","lu.reviewed.git.head","Git HEAD"),
        ("-gs","lu.reviewed.git.status","Git tracked status"),
        ("-gb","lu.reviewed.git.branch","Git branch"),
        ("-gc","lu.reviewed.git.history","Git history"),
        ("-fm","lu.reviewed.media.facts","Media facts"),
    ];
    TYPES.iter().find_map(|(suffix,id,label)|{
        (tool==*id).then(||call.strip_suffix(*suffix).map(|outer|(*label,outer))).flatten()
    }).filter(|(_,outer)|!outer.is_empty()&&outer.len()<=115)
}
fn empty_view(state:&'static str)->ReviewedTurnContinuity{
    ReviewedTurnContinuity{
        origin_bound:false,journal_state:state,
        continuation:"RECONCILIATION_REQUIRED",
        explanation:"The original task cannot be proven from complete, matching host records. No operation will be resumed or replayed.",
        operations:Vec::new(),
    }
}
fn checkpoint(task:&Connection,task_id:&str,turn:&str)
    ->Result<(bool,Option<u8>,u32),String>{
    let found:Option<(String,i64)>=task.query_row(
        "SELECT state,progress FROM lu_tasks WHERE task_id=?1 AND workspace_id=?2",
        params![task_id,turn],
        |r|Ok((r.get(0)?,r.get(1)?))
    ).optional().map_err(|_|"Reviewed task lookup unavailable")?;
    let Some((state,_))=found else{return Ok((false,None,0))};
    if !matches!(state.as_str(),
        "COMPLETED"|"INTERRUPTED"|"FAILED"|"TIMED_OUT"|"CANCELLED"|
        "PAUSED"|"PENDING"|"RUNNING"){
        return Err("Unrecognized native task state".into())
    }
    let (count,last):(i64,Option<i64>)=task.query_row(
        "SELECT COUNT(*),MAX(progress) FROM lu_task_events
         WHERE task_id=?1 AND event='CHECKPOINT'",
        params![task_id],|r|Ok((r.get(0)?,r.get(1)?))
    ).map_err(|_|"Task checkpoint events unavailable")?;
    if !(0..=4096).contains(&count){
        return Err("Too many reviewed task checkpoints".into())
    }
    let progress=last.map(|n|u8::try_from(n)
        .map_err(|_|"Invalid checkpoint progress")).transpose()?;
    Ok((state=="COMPLETED",progress,count as u32))
}
fn expected_method(tool:&str,method:&str)->bool{
    match tool{
        "lu.reviewed.git.head"=>method=="GIT_HEAD_DIRECT_V1",
        "lu.reviewed.git.status"=>method=="GIT_STATUS_INDEX_TREE_V1",
        "lu.reviewed.git.branch"=>method=="GIT_BRANCH_DIRECT_V1",
        "lu.reviewed.git.history"=>matches!(method,
            "GIT_HISTORY_LOOSE_DAG_V1"|"GIT_HISTORY_OBJECT_DAG_V2"),
        "lu.reviewed.media.facts"=>matches!(method,
            "WAV_PCM_DIRECT_V1"|"MP4_BOXES_DIRECT_V1"),
        _=>false,
    }
}
fn outcome_for(effect:&Connection,call_id:&str,tool:&str,receipt:Option<&[u8]>)
    ->Result<&'static str,String>{
    if !table_exists(effect,"lu_independent_outcomes")?{
        return Ok("NOT_INDEPENDENTLY_CHECKED")
    }
    let row:Option<(String,String,String,Option<String>)>=effect.query_row(
        "SELECT verdict,receipt_sha256,method,observed_sha256
         FROM lu_independent_outcomes WHERE call_id=?1",
        params![call_id],|r|Ok((
            r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?
        ))
    ).optional().map_err(|_|"Independent outcome lookup unavailable")?;
    let Some((verdict,digest,method,observed))=row else{
        return Ok("NOT_INDEPENDENTLY_CHECKED")
    };
    let Some(receipt)=receipt else{return Ok("RECEIPT_MISMATCH")};
    if digest!=format!("{:x}",Sha256::digest(receipt)){
        return Ok("RECEIPT_MISMATCH")
    }
    match verdict.as_str(){
        "VERIFIED"=>{
            let valid_observed=observed.as_deref().is_some_and(|sha|
                sha.len()==64 && sha.bytes().all(|b|b.is_ascii_hexdigit()
                    &&!b.is_ascii_uppercase()));
            if !valid_observed || !expected_method(tool,&method){
                return Ok("OUTCOME_INCONSISTENT")
            }
            Ok("VERIFIED_AT_OBSERVATION")
        },
        "UNSUPPORTED"=>Ok("UNSUPPORTED"),
        "CONFLICT"=>Ok("CONFLICT"),
        _=>Err("Unknown independent outcome verdict".into())
    }
}
/// Caller MUST first authenticate chat/turn through ChatStore's original user
/// record. No model-provided ID or path reaches this API. Historical unbound
/// journals intentionally remain visible only as unresolved provenance.
pub(crate) fn reviewed_turn_continuity(
    profile:&Path,chat:&str,turn:&str,
)->Result<Option<ReviewedTurnContinuity>,String>{
    if !valid(chat,"c-")||!valid(turn,"g-"){
        return Err("Invalid persisted conversation/turn identity".into())
    }
    let task_path=profile.join(format!("lu-reviewed-task-{turn}.sqlite"));
    let effect_path=profile.join(format!("lu-reviewed-effects-{turn}.sqlite"));
    let task=open_readonly(&task_path)?;
    let effect=open_readonly(&effect_path)?;
    if task.is_none()&&effect.is_none(){return Ok(None)}
    let (Some(task),Some(effect))=(task,effect) else{
        return Ok(Some(empty_view("PARTIAL_JOURNALS")))
    };
    if !table_exists(&task,"lu_reviewed_turn_origin")?
        ||!table_exists(&task,"lu_tasks")?
        ||!table_exists(&task,"lu_task_events")?
        ||!table_exists(&effect,"lu_effects")?{
        return Ok(Some(empty_view("LEGACY_OR_MISSING_SCHEMA")))
    }
    let origin:Option<(String,String)>=task.query_row(
        "SELECT conversation_id,user_turn_id FROM lu_reviewed_turn_origin
         WHERE singleton=1",[],|r|Ok((r.get(0)?,r.get(1)?))
    ).optional().map_err(|_|"Original human task reference unavailable")?;
    if origin.as_ref().is_none_or(|(c,t)|c!=chat||t!=turn){
        return Ok(Some(empty_view("ORIGIN_MISMATCH")))
    }
    let mut stmt=effect.prepare(
        "SELECT call_id,tool_id,state,LENGTH(receipt),
                CASE WHEN LENGTH(receipt) BETWEEN 1 AND 32768
                     THEN receipt ELSE NULL END
         FROM lu_effects ORDER BY call_id LIMIT 257"
    ).map_err(|_|"Effect journal query unavailable")?;
    let rows=stmt.query_map([],|r|Ok((
        r.get::<_,String>(0)?,r.get::<_,String>(1)?,
        r.get::<_,String>(2)?,r.get::<_,Option<i64>>(3)?,
        r.get::<_,Option<Vec<u8>>>(4)?
    ))).map_err(|_|"Effect journal unreadable")?;
    let mut all=Vec::new();
    for row in rows{
        all.push(row.map_err(|_|"Damaged effect journal entry")?);
        if all.len()>MAX_ROWS{
            return Err("Reviewed effect count exceeded hard limit".into())
        }
    }
    let outer:BTreeMap<String,(String,String)>=all.iter().map(|(id,tool,state,_,_)|
        (id.clone(),(tool.clone(),state.clone()))).collect();
    let mut operations=Vec::new();
    let mut needs_review=false;
    for (call,tool,state,size,receipt) in all{
        // An unexpected or incomplete effect in this per-turn journal
        // is unresolved evidence even if another inner worker verified.
        // Read-grant rows may legitimately lack independent OS facts.
        let recognized=matches!(tool.as_str(),
            "lu.reviewed.git.head"|"lu.reviewed.git.status"|
            "lu.reviewed.git.branch"|"lu.reviewed.git.history"|
            "lu.reviewed.media.facts"|"lu.reviewed.git.repo-read"|
            "lu.reviewed.media.file-read");
        if !recognized||matches!(state.as_str(),
            "UNCERTAIN"|"EXECUTING"|"PREPARED"|"FAILED"){
            needs_review=true;
        }
        let Some((label,model_call))=operation_for(&call,&tool) else{continue};
        if !matches!(state.as_str(),
            "PREPARED"|"EXECUTING"|"APPLIED"|"FAILED"|"UNCERTAIN"){
            return Err("Unrecognized native effect state".into())
        }
        let outer_ok=outer.get(model_call)
            .is_some_and(|(kind,state)|kind==&tool&&state=="APPLIED");
        let (task_complete,progress,events)=checkpoint(&task,&call,turn)?;
        let receipt_ok=size.is_some_and(|len|len>0&&len<=MAX_RECEIPT)
            &&receipt.as_deref().is_some_and(|v|v.len() as i64==size.unwrap_or(0));
        let outcome=if state=="APPLIED" &&receipt_ok{
            outcome_for(&effect,&call,&tool,receipt.as_deref())?
        }else if state=="APPLIED"{
            "RECEIPT_MISMATCH"
        }else{"NOT_INDEPENDENTLY_CHECKED"};
        let passed=outer_ok&&task_complete&&receipt_ok
            &&outcome=="VERIFIED_AT_OBSERVATION";
        if !passed{needs_review=true;}
        operations.push(ReviewedOperationContinuity{
            operation:label,process_state:state,outcome,
            last_checkpoint:progress,checkpoint_events:events,
            original_model_call_linked:outer_ok,worker_task_linked:task_complete,
        });
    }
    if operations.is_empty(){needs_review=true}
    let (journal_state,continuation,explanation)=if needs_review{
        ("NEEDS_RECONCILIATION","RECONCILIATION_REQUIRED",
         "At least one OS effect, task checkpoint, human-call link or independently checked receipt is absent, uncertain or contradictory. Investigate before any new execution.")
    }else{
        ("VERIFIED_PAST_OPERATIONS","FRESH_REQUEST_ONLY",
         "These past operations matched their independently checked sources at observation. This is not a live process or current-source guarantee. Any further work requires a new human request and fresh authorization.")
    };
    Ok(Some(ReviewedTurnContinuity{
        origin_bound:true,journal_state,continuation,explanation,operations,
    }))
}

#[cfg(test)]
mod tests{
    use super::*;
    use lu_core::{
        contracts::{ProtoPayload,ToolInvocation},
        journal::{EffectJournal,IndependentOutcomeVerdict},
        tasks::TaskStore,
    };
    use std::time::{SystemTime,UNIX_EPOCH};
    fn fixture()->(PathBuf,String,String){
        let folder=std::env::temp_dir().join(format!(
            "lu-e1-p15-continuity-{}-{}",
            std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH)
                .unwrap().as_nanos()
        ));
        fs::create_dir_all(&folder).unwrap();
        (folder,format!("c-{:032x}",1),format!("g-{:032x}",2))
    }
    fn call(id:&str)->ToolInvocation{
        ToolInvocation{
            call_id:id.into(),tool_id:"lu.reviewed.git.head".into(),
            workspace_id:Some("test".into()),
            input:ProtoPayload{type_url:"lu.e1.request".into(),bytes:vec![1]},
        }
    }
    #[test]
    fn origin_bound_verified_past_effect_never_authorizes_resumption(){
        let (path,chat,turn)=fixture();
        let taskfile=path.join(format!("lu-reviewed-task-{turn}.sqlite"));
        let effectfile=path.join(format!("lu-reviewed-effects-{turn}.sqlite"));
        let tasks=TaskStore::open(&taskfile).unwrap();
        tasks.bind_reviewed_human_turn(&chat,&turn).unwrap();
        tasks.create("call-gh",Some(&turn),None).unwrap();
        tasks.start("call-gh").unwrap();
        tasks.checkpoint("call-gh",55,"sensitive hidden progress",b"SECRET").unwrap();
        tasks.complete("call-gh").unwrap();
        let effect=EffectJournal::open(&effectfile).unwrap();
        for id in ["call","call-gh"]{
            effect.prepare(&call(id)).unwrap();
            effect.mark_executing(id).unwrap();
            effect.mark_applied(id,b"receipt").unwrap();
        }
        let hash=format!("{:x}",Sha256::digest(b"facts"));
        effect.record_independent_outcome("call-gh",b"receipt",
            IndependentOutcomeVerdict::Verified,"GIT_HEAD_DIRECT_V1",
            Some(&hash)).unwrap();
        let projection=reviewed_turn_continuity(&path,&chat,&turn)
            .unwrap().unwrap();
        assert_eq!(projection.continuation,"FRESH_REQUEST_ONLY");
        assert_eq!(projection.operations.len(),1);
        assert_eq!(projection.operations[0].last_checkpoint,Some(55));
        assert_eq!(projection.operations[0].checkpoint_events,1);
        let shown=serde_json::to_string(&projection).unwrap();
        assert!(!shown.contains("SECRET"));
        assert!(!shown.contains("sensitive hidden progress"));
        assert!(!shown.contains("call-gh"));
        assert_eq!(reviewed_turn_continuity(&path,
            &format!("c-{:032x}",3),&turn).unwrap().unwrap().journal_state,
            "ORIGIN_MISMATCH");
        drop(effect);drop(tasks);
        // Genuine SQLite close/reopen: results are derived from durable
        // bytes, not a previously held model-turn runtime object.
        let reopened=reviewed_turn_continuity(&path,&chat,&turn)
            .unwrap().unwrap();
        assert_eq!(reopened.continuation,"FRESH_REQUEST_ONLY");
        // Independently tamper the persisted second-witness digest.
        // An APPLIED original effect is still NOT acceptable semantic proof.
        let db=Connection::open(&effectfile).unwrap();
        db.execute(
            "UPDATE lu_independent_outcomes SET receipt_sha256=?1
             WHERE call_id='call-gh'",
            rusqlite::params!["0".repeat(64)]
        ).unwrap();
        drop(db);
        let altered=reviewed_turn_continuity(&path,&chat,&turn)
            .unwrap().unwrap();
        assert_eq!(altered.continuation,"RECONCILIATION_REQUIRED");
        assert_eq!(altered.operations[0].outcome,"RECEIPT_MISMATCH");
        // A hash-compatible receipt paired with the WRONG independent
        // operation is still not an approved oracle for Git HEAD.
        let db=Connection::open(&effectfile).unwrap();
        let genuine_receipt=format!("{:x}",Sha256::digest(b"receipt"));
        db.execute(
            "UPDATE lu_independent_outcomes SET receipt_sha256=?1,
             method='GIT_BRANCH_DIRECT_V1' WHERE call_id='call-gh'",
            rusqlite::params![genuine_receipt]
        ).unwrap();
        drop(db);
        let wrong_method=reviewed_turn_continuity(&path,&chat,&turn)
            .unwrap().unwrap();
        assert_eq!(wrong_method.continuation,"RECONCILIATION_REQUIRED");
        assert_eq!(wrong_method.operations[0].outcome,"OUTCOME_INCONSISTENT");
        fs::remove_dir_all(&path).unwrap();
    }
    #[test]
    fn missing_and_uncertain_journals_never_offer_resume(){
        let (path,chat,turn)=fixture();
        assert!(reviewed_turn_continuity(&path,&chat,&turn).unwrap().is_none());
        let taskfile=path.join(format!("lu-reviewed-task-{turn}.sqlite"));
        let tasks=TaskStore::open(&taskfile).unwrap();
        tasks.bind_reviewed_human_turn(&chat,&turn).unwrap();
        assert_eq!(reviewed_turn_continuity(&path,&chat,&turn)
            .unwrap().unwrap().journal_state,"PARTIAL_JOURNALS");
        drop(tasks);fs::remove_dir_all(&path).unwrap();
    }
    #[test]
    fn unsupported_or_ambiguous_os_effect_requires_reconciliation(){
        let (path,chat,turn)=fixture();
        let tasks=TaskStore::open(&path.join(format!("lu-reviewed-task-{turn}.sqlite"))).unwrap();
        tasks.bind_reviewed_human_turn(&chat,&turn).unwrap();
        tasks.create("call-gh",Some(&turn),None).unwrap();
        tasks.start("call-gh").unwrap();
        let effects=EffectJournal::open(
            &path.join(format!("lu-reviewed-effects-{turn}.sqlite"))).unwrap();
        effects.prepare(&call("call-gh")).unwrap();
        effects.mark_executing("call-gh").unwrap();
        effects.mark_uncertain("call-gh").unwrap();
        let x=reviewed_turn_continuity(&path,&chat,&turn).unwrap().unwrap();
        assert_eq!(x.continuation,"RECONCILIATION_REQUIRED");
        assert_eq!(x.operations[0].outcome,"NOT_INDEPENDENTLY_CHECKED");
        drop(effects);drop(tasks);fs::remove_dir_all(path).unwrap();
    }
}
