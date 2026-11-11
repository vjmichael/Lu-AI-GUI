//! PLAN-07C: operator-only suggestions review. This deliberately CANNOT
//! mark work completed, revalidate exact criteria, run any tool, or rewrite
//! the immutable PLAN05 register. Human approval is review consent only.
//! Pending/terminal events are append-only in the EXISTING ANSF SQLite.
use super::*;
use lu_core::{project_task_register as register,project_task_updates as updates,
    project_task_update_model as model,project_task_synthesis as synthesis,
    project_evidence_graph as graph,project_status_classifier as status};
use serde::{Serialize,Deserialize};
use rusqlite::{Connection,params,OptionalExtension,TransactionBehavior};
use std::{path::PathBuf,sync::OnceLock,collections::BTreeSet};

const SCHEMA:&str="lu-project-task-human-update-review-v1";
const MAX_EVENTS:usize=800;
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
enum Phase{Prepared,ApprovedForReview,Refused,Cancelled,Interrupted,StaleRejected}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct ReviewEvent{
    schema:String,review_id:String,sequence:u64,
    previous_sha256:Option<String>,phase:Phase,
    host_instance:String,register_id:String,expected_register_sha256:String,
    preview_sha256:String,original_graph_sha256:String,
    original_status_sha256:String,task_id:String,expected_task_version:u64,
    original_requirement_id:String,model_suggestion_sha256:Option<String>,
    model_action:Option<model::Action>,operator_note:String,
    human_confirmed:bool,no_completion_authority:bool,
    no_execution_authority:bool,sha256:String,
}
fn host_instance()->&'static str{
    static INSTANCE:OnceLock<String>=OnceLock::new();
    INSTANCE.get_or_init(||next_id("lu-plan07c-process-")).as_str()
}
fn hash(event:&ReviewEvent)->Result<String,String>{
    let mut unsigned=event.clone();unsigned.sha256.clear();
    Ok(format!("{:x}",Sha256::digest(
        serde_json::to_vec(&unsigned).map_err(|_|"PLAN07C_EVENT_HASH_SERIALIZE")?
    )))
}
fn init(c:&Connection)->Result<(),String>{
    c.execute_batch("CREATE TABLE IF NOT EXISTS ansf_project_task_update_review_events(
        review_id TEXT NOT NULL,
        sequence INTEGER NOT NULL CHECK(sequence BETWEEN 1 AND 2),
        event_sha256 TEXT NOT NULL UNIQUE,
        previous_sha256 TEXT,
        register_id TEXT NOT NULL,
        event_json BLOB NOT NULL,
        no_completion_authority INTEGER NOT NULL CHECK(no_completion_authority=1),
        no_execution_authority INTEGER NOT NULL CHECK(no_execution_authority=1),
        PRIMARY KEY(review_id,sequence)
    );").map_err(|_|"PLAN07C_EVENT_TABLE_FAILED".to_string())
}
fn valid(e:&ReviewEvent)->Result<(),String>{
    if e.schema!=SCHEMA||!e.no_completion_authority||
       !e.no_execution_authority||!graph::valid_digest(&e.preview_sha256)||
       !graph::valid_digest(&e.expected_register_sha256)||
       !graph::valid_digest(&e.original_graph_sha256)||
       !graph::valid_digest(&e.original_status_sha256)||
       !e.task_id.starts_with("lu-task-")||
       !e.register_id.starts_with("lu-register-")||
       e.host_instance.len()>128||e.host_instance.len()<8||
       e.operator_note.len()<5||e.operator_note.len()>250||
       e.operator_note.chars().any(|c|c.is_control()&&c!='\n')||
       e.expected_task_version==0||
       e.sha256!=hash(e)?||
       (e.sequence==1&&(e.phase!=Phase::Prepared||e.human_confirmed||
           e.previous_sha256.is_some()))||
       (e.sequence==2&&(e.phase==Phase::Prepared||
           !e.previous_sha256.is_some()))||
       (e.phase==Phase::ApprovedForReview&&!e.human_confirmed)||
       (e.phase!=Phase::ApprovedForReview&&e.human_confirmed){
        return Err("PLAN07C_INVALID_EVENT_OR_EXECUTION_AUTHORITY".into())
    }
    Ok(())
}
fn original(c:&Connection,id:&str)->Result<Vec<ReviewEvent>,String>{
    if !id.starts_with("lu-plan07c-review-")||id.len()>115{
        return Err("PLAN07C_REVIEW_ID_INVALID".into())
    }
    init(c)?;
    let mut st=c.prepare("SELECT event_json,event_sha256,previous_sha256,
        no_completion_authority,no_execution_authority,sequence,register_id FROM
        ansf_project_task_update_review_events
        WHERE review_id=?1 ORDER BY sequence")
        .map_err(|_|"PLAN07C_EVENT_QUERY_FAILED")?;
    let raw=st.query_map([id],|r|Ok((
        r.get::<_,Vec<u8>>(0)?,r.get::<_,String>(1)?,
        r.get::<_,Option<String>>(2)?,r.get::<_,i64>(3)?,r.get::<_,i64>(4)?,
        r.get::<_,i64>(5)?,r.get::<_,String>(6)?
    ))).map_err(|_|"PLAN07C_EVENT_STREAM_FAILED")?;
    let mut events=Vec::new();
    for row in raw{
        let (bytes,digest,previous,complete,execute,seq,scope)=row
            .map_err(|_|"PLAN07C_EVENT_ROW_INVALID")?;
        if bytes.len()>8192||complete!=1||execute!=1{
            return Err("PLAN07C_EVENT_BYTES_OR_AUTHORITY_INVALID".into())
        }
        let e:ReviewEvent=serde_json::from_slice(&bytes)
            .map_err(|_|"PLAN07C_EVENT_JSON_TAMPERED")?;
        valid(&e)?;
        if e.review_id!=id||e.sha256!=digest||e.previous_sha256!=previous||
           e.sequence!=seq as u64||e.register_id!=scope||
           e.sequence!=events.len() as u64+1||
           e.previous_sha256!=events.last().map(|x:&ReviewEvent|x.sha256.clone()){
            return Err("PLAN07C_EVENT_HISTORY_OR_SHA_TAMPERED".into())
        }
        if let Some(first)=events.first(){
            let original:&ReviewEvent=first;
            if e.phase==Phase::Prepared||
               e.register_id!=original.register_id||
               e.expected_register_sha256!=original.expected_register_sha256||
               e.preview_sha256!=original.preview_sha256||
               e.original_graph_sha256!=original.original_graph_sha256||
               e.original_status_sha256!=original.original_status_sha256||
               e.task_id!=original.task_id||
               e.expected_task_version!=original.expected_task_version||
               e.original_requirement_id!=original.original_requirement_id||
               e.model_suggestion_sha256!=original.model_suggestion_sha256||
               e.model_action!=original.model_action||
               e.operator_note!=original.operator_note||
               e.host_instance!=original.host_instance{
                return Err("PLAN07C_REVIEW_DECISION_CHANGED_ORIGINAL_FACTS".into())
            }
        }
        events.push(e);
        if events.len()>2{return Err("PLAN07C_MORE_THAN_ONE_DECISION".into())}
    }
    Ok(events)
}
fn append(profile:&Path,mut event:ReviewEvent)->Result<ReviewEvent,String>{
    let mut c=project_register_desktop::library(profile)?;init(&c)?;
    let tx=c.transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_|"PLAN07C_REVIEW_BEGIN_IMMEDIATE")?;
    let previous=original(&tx,&event.review_id)?;
    if event.sequence==1{
        if !previous.is_empty(){return Err("PLAN07C_REVIEW_TOKEN_REPLAY".into())}
        let count:i64=tx.query_row("SELECT COUNT(DISTINCT review_id) FROM
            ansf_project_task_update_review_events WHERE register_id=?1",
            [&event.register_id],|r|r.get(0))
            .map_err(|_|"PLAN07C_REVIEW_CAPACITY_QUERY")?;
        if count<0||count>=MAX_EVENTS as i64{
            return Err("PLAN07C_ORIGINAL_REVIEW_JOURNAL_FULL".into())
        }
    }else if event.sequence==2{
        if previous.len()!=1||previous[0].phase!=Phase::Prepared||
           event.previous_sha256!=Some(previous[0].sha256.clone())||
           event.register_id!=previous[0].register_id||
           event.expected_register_sha256!=previous[0].expected_register_sha256||
           event.preview_sha256!=previous[0].preview_sha256||
           event.original_graph_sha256!=previous[0].original_graph_sha256||
           event.original_status_sha256!=previous[0].original_status_sha256||
           event.task_id!=previous[0].task_id||
           event.expected_task_version!=previous[0].expected_task_version||
           event.original_requirement_id!=previous[0].original_requirement_id||
           event.model_suggestion_sha256!=previous[0].model_suggestion_sha256||
           event.model_action!=previous[0].model_action||
           event.operator_note!=previous[0].operator_note||
           event.host_instance!=previous[0].host_instance{
            return Err("PLAN07C_REVIEW_REPLAY_OR_REPLACED_SOURCE".into())
        }
    }else{return Err("PLAN07C_BAD_REVIEW_SEQUENCE".into())}
    if event.phase==Phase::ApprovedForReview{
        let (_,now,_)=project_register_desktop::load_register(&tx,&event.register_id)?
            .ok_or("PLAN07C_OPERATOR_REGISTER_DISAPPEARED")?;
        if now.latest_sha256!=event.expected_register_sha256{
            return Err("PLAN07C_SOURCE_REGISTER_CHANGED_DURING_APPROVAL".into())
        }
    }
    if event.sha256.is_empty(){event.sha256=hash(&event)?}
    valid(&event)?;
    let json=serde_json::to_vec(&event)
        .map_err(|_|"PLAN07C_DECISION_SERIALIZE")?;
    tx.execute("INSERT INTO ansf_project_task_update_review_events(
         review_id,sequence,event_sha256,previous_sha256,register_id,
         event_json,no_completion_authority,no_execution_authority)
         VALUES(?1,?2,?3,?4,?5,?6,1,1)",
         params![event.review_id,event.sequence as i64,event.sha256,
             event.previous_sha256,event.register_id,json]
    ).map_err(|_|"PLAN07C_REVIEW_IMMUTABLE_INSERT_FAILED")?;
    let read=original(&tx,&event.review_id)?;
    if read.last().map(|x|x.sha256.as_str())!=Some(event.sha256.as_str())||
       read.len()!=event.sequence as usize{
        return Err("PLAN07C_APPEND_READBACK_MISMATCH".into())
    }
    tx.commit().map_err(|_|"PLAN07C_REVIEW_FSYNC_FAILED")?;
    Ok(event)
}
fn snapshot(profile:&Path,preview_id:&str,task_id:&str,
    selected_model:Option<&str>,note:&str)->Result<ReviewEvent,String>{
    if note.len()<5||note.len()>250||
       note.chars().any(|c|c.is_control()&&c!='\n'){
        return Err("PLAN07C_HUMAN_NOTE_REQUIRED".into())
    }
    let (p,r,_,g,s)=project_updates_desktop::reload_audited(profile,preview_id)?;
    let candidate=p.candidates.iter().find(|x|x.task_id==task_id)
        .ok_or("PLAN07C_ORIGINAL_TASK_NOT_IN_PREVIEW")?;
    let v=register::latest(&r,task_id).ok_or("PLAN07C_ORIGINAL_TASK_MISSING")?;
    if candidate.expected_version!=v.version||
       candidate.original_requirement_id!=v.source_requirement_id{
        return Err("PLAN07C_TASK_VERSION_OR_ID_CHANGED".into())
    }
    let (model_digest,action)=if let Some(digest)=selected_model{
        if !graph::valid_digest(digest){return Err("PLAN07C_BAD_ORIGINAL_MODEL_SHA".into())}
        let original=project_update_model_desktop::read_latest(profile,preview_id)?
            .ok_or("PLAN07C_ORIGINAL_MODEL_RECEIPT_NOT_FOUND")?;
        if original.digest_sha256!=digest||
           original.expected_register_sha256!=r.latest_sha256{
            return Err("PLAN07C_ORIGINAL_MODEL_SUGGESTION_STALE".into())
        }
        let item=original.suggestions.iter().find(|x|
            x.task_id==task_id&&x.original_requirement_id==v.source_requirement_id&&
            x.expected_version==v.version&&x.suggested_only&&
            !x.independently_accepted&&!x.task_complete&&!x.execution_authorized
        ).ok_or("PLAN07C_MODEL_CANNOT_REMAP_ORIGINAL_TASK")?;
        (Some(digest.to_string()),Some(item.action))
    }else{(None,None)};
    Ok(ReviewEvent{
        schema:SCHEMA.into(),review_id:next_id("lu-plan07c-review-"),
        sequence:1,previous_sha256:None,phase:Phase::Prepared,
        host_instance:host_instance().to_string(),register_id:r.register_id,
        expected_register_sha256:r.latest_sha256,
        preview_sha256:p.digest_sha256,original_graph_sha256:g.sha256,
        original_status_sha256:s.digest_sha256,task_id:task_id.into(),
        expected_task_version:v.version,
        original_requirement_id:v.source_requirement_id.clone(),
        model_suggestion_sha256:model_digest,model_action:action,
        operator_note:note.into(),human_confirmed:false,
        no_completion_authority:true,no_execution_authority:true,
        sha256:String::new(),
    })
}
fn decide(profile:&Path,pending:&ReviewEvent,phase:Phase,root:Option<&Path>)
 ->Result<ReviewEvent,String>{
    if pending.phase!=Phase::Prepared||pending.sequence!=1||
       pending.host_instance!=host_instance(){
        return Err("PLAN07C_WRONG_PROCESS_OR_PENDING_TOKEN".into())
    }
    let mut terminal=pending.clone();
    terminal.sequence=2;terminal.previous_sha256=Some(pending.sha256.clone());
    terminal.sha256.clear();terminal.phase=phase;
    terminal.human_confirmed=phase==Phase::ApprovedForReview;
    if phase==Phase::ApprovedForReview{
        let root=root.ok_or("PLAN07C_NATIVE_SOURCE_SELECTION_REQUIRED")?;
        let (p,r,_,g,s)=project_updates_desktop::reload_audited(
            profile,&pending.preview_sha256)?;
        if p.expected_register_sha256!=pending.expected_register_sha256||
           r.latest_sha256!=pending.expected_register_sha256||
           g.sha256!=pending.original_graph_sha256||
           s.digest_sha256!=pending.original_status_sha256||
           register::latest(&r,&pending.task_id)
                .is_none_or(|v|v.version!=pending.expected_task_version||
                    v.source_requirement_id!=pending.original_requirement_id){
            return Err("PLAN07C_REVIEW_SOURCE_LEASE_CHANGED".into())
        }
        // Repeat the bounded scanner after the OS confirmation. Native
        // bytes and selected exact HEAD must match reviewed originals.
        let current=project_status_desktop::classify_selected(root)?;
        if current.0.project_scope_sha256!=r.scope_sha256||
           !synthesis::same_observed_sources(&g,&current.0)?||
           s.complete!=current.1.complete||
           serde_json::to_value(&s.findings)
                .map_err(|_|"PLAN07C_OLD_FINDINGS_ENCODE")?!=
           serde_json::to_value(&current.1.findings)
                .map_err(|_|"PLAN07C_NEW_FINDINGS_ENCODE")?{
            return Err("PLAN07C_HUMAN_REVIEW_SOURCE_CHANGED".into())
        }
        // Model suggestions must still be the exact unmodified original.
        // A changed/corrupted original model receipt after confirmation is
        // denied, not silently downgraded to deterministic evidence.
        if let Some(model_sha)=pending.model_suggestion_sha256.as_deref(){
            let proof=project_update_model_desktop::read_latest(
                profile,&pending.preview_sha256)?
                .ok_or("PLAN07C_APPROVED_MODEL_RECEIPT_DISAPPEARED")?;
            if proof.digest_sha256!=model_sha||
               !proof.suggestions.iter().any(|item|
                   item.task_id==pending.task_id&&
                   item.original_requirement_id==pending.original_requirement_id&&
                   item.expected_version==pending.expected_task_version&&
                   Some(item.action)==pending.model_action){
                return Err("PLAN07C_APPROVED_MODEL_RECEIPT_CHANGED".into())
            }
        }
        // Approval of evidence FOLLOW-UP never appends PLAN05 revisions:
        // only the separate source-reconcile and exact-criterion review
        // commands may change original human-owned task states.
    }else if root.is_some(){return Err("PLAN07C_NONAPPROVAL_CANNOT_SELECT_SOURCE".into())}
    append(profile,terminal)
}
fn review_ids(c:&Connection,id:&str)->Result<Vec<String>,String>{
    init(c)?;
    let count:i64=c.query_row("SELECT COUNT(DISTINCT review_id) FROM
        ansf_project_task_update_review_events WHERE register_id=?1",
        [id],|r|r.get(0)).map_err(|_|"PLAN07C_REVIEW_HISTORY_COUNT")?;
    if count<0||count>MAX_EVENTS as i64{
        return Err("PLAN07C_BOUNDED_JOURNAL_CAPACITY_EXCEEDED".into())
    }
    let mut st=c.prepare("SELECT DISTINCT review_id FROM
        ansf_project_task_update_review_events WHERE register_id=?1
        ORDER BY review_id LIMIT ?2")
        .map_err(|_|"PLAN07C_REGISTER_HISTORY_PREPARE_FAILED")?;
    let ids=st.query_map(params![id,MAX_EVENTS as i64],|r|r.get::<_,String>(0))
        .map_err(|_|"PLAN07C_HISTORY_SELECT_FAILED")?;
    let mut result=Vec::new();
    for row in ids{result.push(row.map_err(|_|"PLAN07C_REVIEW_LIST_INVALID")?)}
    Ok(result)
}
fn history(profile:&Path,register_id:&str,recover:bool)
 ->Result<Vec<ReviewEvent>,String>{
    if !register_id.starts_with("lu-register-"){
        return Err("PLAN07C_INVALID_REGISTER_ID".into())
    }
    let c=project_register_desktop::library(profile)?;
    let _=project_register_desktop::load_register(&c,register_id)?
        .ok_or("PLAN07C_ORIGINAL_REGISTER_NOT_FOUND")?;
    let ids=review_ids(&c,register_id)?;
    drop(c);
    let mut result=Vec::new();
    for id in ids{
        let c=project_register_desktop::library(profile)?;
        let mut events=original(&c,&id)?;
        drop(c);
        if events.is_empty(){return Err("PLAN07C_ORPHAN_REVIEW_ID".into())}
        if events.len()==1&&events[0].host_instance!=host_instance()&&recover{
            let mut event=events[0].clone();
            event.sequence=2;event.previous_sha256=Some(events[0].sha256.clone());
            event.phase=Phase::Interrupted;event.human_confirmed=false;
            event.sha256.clear();
            // A process may race restart with another opener; never claim
            // approval from a recovered, previously incomplete dialog.
            match append(profile,event){
                Ok(saved)=>events.push(saved),
                Err(_)=>{
                    let c=project_register_desktop::library(profile)?;
                    events=original(&c,&id)?;
                    if events.len()!=2||events[1].phase==Phase::ApprovedForReview{
                        return Err("PLAN07C_RESTART_RECOVERY_CONFLICT".into())
                    }
                }
            }
        }
        result.push(events.last().cloned().ok_or("PLAN07C_EMPTY_HISTORY")?);
    }
    Ok(result)
}
fn operator_window(caller:&tauri::WebviewWindow)->Result<(),String>{
    if caller.label()!="task-list"{return Err("PLAN07C_REQUIRES_HUMAN_TASK_WINDOW".into())}
    Ok(())
}
#[tauri::command]
pub(crate) async fn lu_project_update_review_history(
    caller:tauri::WebviewWindow,state:State<'_,Arc<DesktopState>>,
    register_id:String
)->Result<Vec<ReviewEvent>,String>{
    operator_window(&caller)?;
    let profile=project_register_desktop::profile(state.inner())?;
    tauri::async_runtime::spawn_blocking(move||
        history(&profile,&register_id,true))
        .await.map_err(|_|"PLAN07C_HISTORY_RESTART_THREAD_INTERRUPTED")?
}
#[tauri::command]
pub(crate) async fn lu_project_update_review_decide(
    app:AppHandle,caller:tauri::WebviewWindow,
    state:State<'_,Arc<DesktopState>>,
    preview_sha256:String,task_id:String,
    model_suggestion_sha256:Option<String>,operator_note:String
)->Result<ReviewEvent,String>{
    operator_window(&caller)?;
    let profile=project_register_desktop::profile(state.inner())?;
    let scoped=profile.clone();
    // Durable prepared receipt is committed BEFORE the native approval
    // dialog so process interruption cannot be mistaken for consent.
    let prepared=tauri::async_runtime::spawn_blocking(move||
        append(&scoped,snapshot(&scoped,&preview_sha256,&task_id,
            model_suggestion_sha256.as_deref(),&operator_note)?)
    ).await.map_err(|_|"PLAN07C_PREPARE_WORKER_INTERRUPTED")??;
    let selected=prepared.model_suggestion_sha256.as_deref()
        .map(|x|format!("Original model suggestion SHA: {x}"))
        .unwrap_or_else(||"Deterministic native evidence suggestion only".into());
    let message=format!("Approve this task's NEXT HUMAN EVIDENCE FOLLOW-UP?\n\n\
            Original task: {} v{}\n\
            Original requirement: {}\n\
            Exact graph: {}\n\
            Register revision SHA: {}\n{}\n\
            Your review note: {}\n\n\
            APPROVE means that you want to investigate this evidence next. \
            It DOES NOT accept the requirement, complete the task, \
            approve installation, or authorize any worker, file or Git action. \
            REFUSE records a durable explicit refusal.\n\
            If you approve, select the identical original project through \
            the native folder picker for a fresh after-decision source scan.",
            prepared.task_id,prepared.expected_task_version,
            prepared.original_requirement_id,prepared.original_graph_sha256,
            prepared.expected_register_sha256,selected,prepared.operator_note);
    let dialog=app.clone();
    let approval=tauri::async_runtime::spawn_blocking(move||
        dialog.dialog().message(message)
            .title("Lu — Human evidence follow-up decision")
            .buttons(tauri_plugin_dialog::MessageDialogButtons::OkCancelCustom(
                "Approve evidence review only".into(),
                "Refuse evidence review".into()
            )).blocking_show()
    ).await.map_err(|_|"PLAN07C_REVIEW_OS_DIALOG_INTERRUPTED")?;
    if !approval{
        return decide(&profile,&prepared,Phase::Refused,None)
    }
    let chooser=app.clone();
    let path=tauri::async_runtime::spawn_blocking(move||
        chooser.dialog().file().blocking_pick_folder()
    ).await.map_err(|_|"PLAN07C_AFTER_APPROVAL_PICKER_INTERRUPTED")?;
    let Some(path)=path else{
        return decide(&profile,&prepared,Phase::Cancelled,None)
    };
    let root=path.into_path().map_err(|_|"PLAN07C_PICKED_PATH_INVALID")?;
    let finish=tauri::async_runtime::spawn_blocking({
        let profile=profile.clone();let pending=prepared.clone();
        move||decide(&profile,&pending,Phase::ApprovedForReview,Some(&root))
    }).await.map_err(|_|"PLAN07C_POST_DIALOG_RECHECK_INTERRUPTED")?;
    match finish{
        Ok(event)=>Ok(event),
        Err(e)=>{
            // Record the rejected after-decision change; do not forge an
            // approval or resume on restart.
            let _=decide(&profile,&prepared,Phase::StaleRejected,None)?;
            Err(e)
        }
    }
}


fn same_source(
    a:&(graph::EvidenceGraph,status::StatusReport),
    b:&(graph::EvidenceGraph,status::StatusReport)
)->Result<bool,String>{
    Ok(a.0.project_scope_sha256==b.0.project_scope_sha256&&
        synthesis::same_observed_sources(&a.0,&b.0)?&&
        a.1.complete==b.1.complete&&
        serde_json::to_value(&a.1.findings)
            .map_err(|_|"PLAN07C_PRIOR_CLASSIFIER_ENCODE")?==
        serde_json::to_value(&b.1.findings)
            .map_err(|_|"PLAN07C_CURRENT_CLASSIFIER_ENCODE")?)
}
fn exact_preflight(profile:&Path,root:&Path,register_id:&str,task_id:&str,
    expected_version:u64)->Result<(graph::EvidenceGraph,status::StatusReport,String),String>{
    let conn=project_register_desktop::library(profile)?;
    let (p,r,_)=project_register_desktop::load_register(&conn,register_id)?
        .ok_or("PLAN07C_EXACT_ORIGINAL_REGISTER_MISSING")?;
    let task=register::latest(&r,task_id)
        .ok_or("PLAN07C_EXACT_TASK_NOT_IN_ORIGINAL_REGISTER")?;
    if task.version!=expected_version{
        return Err("PLAN07C_EXACT_TASK_VERSION_STALE".into())
    }
    let original=project_register_desktop::archived_evidence(
        &conn,&task.graph_sha256,&task.status_sha256)?;
    let observed=project_status_desktop::classify_selected(root)?;
    if p.project_scope_sha256!=observed.0.project_scope_sha256||
       !same_source(&original,&observed)?{
        return Err("PLAN07C_RECONCILE_NATIVE_SOURCE_BEFORE_HUMAN_REVALIDATION".into())
    }
    let f=observed.1.findings.iter()
        .find(|x|x.requirement_id==task.source_requirement_id)
        .ok_or("PLAN07C_EXACT_ORIGINAL_REQUIREMENT_MISSING")?;
    let native_only=observed.0.complete&&observed.1.complete&&
        f.primary==status::Status::Accepted&&
        f.evidence_health==status::EvidenceHealth::Current&&
        f.confidence==status::Confidence::Grounded&&
        !f.conflicting_claims;
    let message=format!(
        "Independently revalidate the EXACT original requirement against \
         current native project evidence?\n\n\
         Task: {} v{}\nOriginal wording: {}\n\
         Human acceptance criterion: {}\n\
         Original requirement ID: {}\n\
         Native classifier status: {:?}\n\
         Native source health: {:?}\n\
         Exact native criterion currently verifiable: {}\n\
         Exact HEAD: {}\nGraph SHA: {}\n\
         Current register SHA: {}\n\n\
         Approval writes ONE HUMAN REVALIDATION revision of only this \
         requirement. It never establishes the broader engineering task is \
         complete, installed, merged, human-tested or authorized for a tool. \
         Model suggestions are NOT part of this decision. Refusal writes \
         no task revision; the original register is preserved.",
         task_id,task.version,task.summary,task.acceptance_criteria,
         task.source_requirement_id,f.primary,f.evidence_health,native_only,
         observed.0.selected_head.as_deref().unwrap_or("unverified"),
         observed.0.sha256,r.latest_sha256
    );
    Ok((observed.0,observed.1,message))
}
fn exact_commit(profile:&Path,root:&Path,register_id:&str,task_id:&str,
    expected_version:u64,reviewed:&(graph::EvidenceGraph,status::StatusReport))
 ->Result<register::TaskRegister,String>{
    let now=project_status_desktop::classify_selected(root)?;
    if !same_source(reviewed,&now)?{
        return Err("PLAN07C_EXACT_SOURCE_CHANGED_AFTER_OS_APPROVAL".into())
    }
    let c=project_register_desktop::library(profile)?;
    let (p,r,_)=project_register_desktop::load_register(&c,register_id)?
        .ok_or("PLAN07C_REVALIDATION_REGISTER_GONE")?;
    let old=register::latest(&r,task_id)
        .ok_or("PLAN07C_REVALIDATION_TASK_GONE")?;
    if old.version!=expected_version||
       old.source_requirement_id.is_empty(){
        return Err("PLAN07C_REVALIDATION_TASK_CHANGED".into())
    }
    let original=project_register_desktop::archived_evidence(
        &c,&old.graph_sha256,&old.status_sha256)?;
    drop(c);
    if !same_source(&original,&now)?{
        return Err("PLAN07C_RECONCILIATION_REQUIRED_BEFORE_REVALIDATION".into())
    }
    let next=register::human_revalidate(
        &r,&p,task_id,expected_version,&now.0,&now.1)?;
    if next.events.len()!=r.events.len()+1||
       next.events.last().is_none_or(|e|
           e.kind!=register::RevisionKind::HumanRevalidated||
           e.revision.execution_authorized||
           e.revision.source_requirement_id!=old.source_requirement_id||
           e.revision.summary!=old.summary||
           e.revision.acceptance_criteria!=old.acceptance_criteria){
        return Err("PLAN07C_EXACT_REVALIDATION_CHANGED_ORIGINAL_PROVENANCE".into())
    }
    project_register_desktop::persist(profile,&p,&next,Some(&r.latest_sha256))
}
#[tauri::command]
pub(crate) async fn lu_project_update_review_exact_requirement(
    app:AppHandle,caller:tauri::WebviewWindow,
    state:State<'_,Arc<DesktopState>>,
    register_id:String,task_id:String,expected_version:u64
)->Result<Option<register::TaskRegister>,String>{
    operator_window(&caller)?;
    if !register_id.starts_with("lu-register-")||
       !task_id.starts_with("lu-task-")||expected_version==0{
        return Err("PLAN07C_INVALID_EXACT_HUMAN_REVIEW_IDS".into())
    }
    let chooser=app.clone();
    let selected=tauri::async_runtime::spawn_blocking(move||
        chooser.dialog().file().blocking_pick_folder()
    ).await.map_err(|_|"PLAN07C_EXACT_NATIVE_PICKER_INTERRUPTED")?;
    let Some(selected)=selected else{return Ok(None)};
    let root=selected.into_path().map_err(|_|"PLAN07C_EXACT_PATH_INVALID")?;
    let profile=project_register_desktop::profile(state.inner())?;
    let preview_root=root.clone();let preview_profile=profile.clone();
    let preview_register=register_id.clone();let preview_task=task_id.clone();
    let (graph,stat,message)=tauri::async_runtime::spawn_blocking(move||
        exact_preflight(&preview_profile,&preview_root,&preview_register,
            &preview_task,expected_version)
    ).await.map_err(|_|"PLAN07C_EXACT_NATIVE_PREFLIGHT_INTERRUPTED")??;
    if !project_register_desktop::confirm(
        &app,"Lu — Independently verify exact native requirement",message
    ).await?{return Ok(None)}
    tauri::async_runtime::spawn_blocking(move||
        exact_commit(&profile,&root,&register_id,&task_id,expected_version,
            &(graph,stat)).map(Some)
    ).await.map_err(|_|"PLAN07C_EXACT_NATIVE_COMMIT_INTERRUPTED")?
}


#[cfg(test)]
mod tests{
    use super::*;
    use lu_core::{project_task_synthesis as synth,project_task_update_model as suggestions};
    struct Original{
        profile:PathBuf,root:PathBuf,proposal:synth::ProjectTaskProposal,
        register:register::TaskRegister,preview:updates::Preview,
    }
    fn fixture(name:&str)->Original{
        let profile=std::env::temp_dir().join(format!(
            "lu-plan07c-{}-{}",name,std::process::id()));
        let _=fs::remove_dir_all(&profile);
        let root=profile.join("original-project");
        fs::create_dir_all(root.join("docs")).unwrap();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(profile.join("ansf")).unwrap();
        fs::write(root.join("docs/REQUIREMENTS.md"),
            b"# Original project\n- [ ] Must include src/required.rs\n").unwrap();
        let c=Connection::open(profile.join("ansf/library.db")).unwrap();
        c.execute_batch("CREATE TABLE original_human_memory(value INTEGER);
            INSERT INTO original_human_memory VALUES(7073);").unwrap();
        drop(c);
        let (g,s)=project_status_desktop::classify_selected(&root).unwrap();
        let capsule=synth::build_capsule(&g,&s).unwrap();
        let req=&capsule.requirements[0];
        let raw=serde_json::json!({"schema":synth::RAW_SCHEMA,"tasks":[{
            "requirementId":req.requirement_id,"status":req.status,
            "summary":"Verify original required source",
            "description":"Independently inspect the original source and runtime proof",
            "priority":"P1","group":"01 Source",
            "evidenceIds":[req.requirement_id],"dependencyIndices":[],
            "acceptanceCriteria":"Host verifies named source plus separate human acceptance",
            "blockedBy":"Missing original required source implementation"
        }]}).to_string();
        let proposal=synth::validate_model_response(&g,&s,&raw,
            &synth::model_prompt(&capsule).unwrap(),
            "m-test-synthetic","s-test-synthetic","g-test-synthetic").unwrap();
        project_task_desktop::persist_proposal(
            &profile,&g,&s,&proposal,&raw).unwrap();
        let imported=register::import(&proposal).unwrap();
        let register=project_register_desktop::persist(
            &profile,&proposal,&imported,None).unwrap();
        fs::write(root.join("src/required.rs"),
            b"fn independently_present(){}\n").unwrap();
        let preview=project_updates_desktop::analyze_selected(
            &profile,&root,&register.register_id).unwrap();
        assert!(preview.semantic_change);
        Original{profile,root,proposal,register,preview}
    }
    #[test]
    fn original_ansf_refusal_approval_cancel_and_crash_recovery_never_accept_work(){
        let x=fixture("decision-restart");
        let task=&x.proposal.tasks[0].task_id;
        let note="Human requires separate evidence and original tests";
        let refused=append(&x.profile,snapshot(
            &x.profile,&x.preview.digest_sha256,task,None,note).unwrap()).unwrap();
        let denied=decide(&x.profile,&refused,Phase::Refused,None).unwrap();
        assert_eq!(denied.phase,Phase::Refused);
        assert!(!denied.human_confirmed);
        assert!(decide(&x.profile,&refused,Phase::ApprovedForReview,Some(&x.root)).is_err());
        let approved=append(&x.profile,snapshot(
            &x.profile,&x.preview.digest_sha256,task,None,note).unwrap()).unwrap();
        let yes=decide(&x.profile,&approved,
            Phase::ApprovedForReview,Some(&x.root)).unwrap();
        assert_eq!(yes.phase,Phase::ApprovedForReview);
        assert!(yes.human_confirmed&&yes.no_completion_authority&&
            yes.no_execution_authority);
        let canceled=append(&x.profile,snapshot(
            &x.profile,&x.preview.digest_sha256,task,None,note).unwrap()).unwrap();
        assert_eq!(decide(&x.profile,&canceled,Phase::Cancelled,None).unwrap().phase,
            Phase::Cancelled);
        let stale=append(&x.profile,snapshot(
            &x.profile,&x.preview.digest_sha256,task,None,note).unwrap()).unwrap();
        fs::write(x.root.join("src/required.rs"),b"changed_original_bytes").unwrap();
        assert!(decide(&x.profile,&stale,
            Phase::ApprovedForReview,Some(&x.root)).is_err());
        assert_eq!(decide(&x.profile,&stale,Phase::StaleRejected,None)
            .unwrap().phase,Phase::StaleRejected);
        // Simulate an authenticated review begun by a *previous* host
        // instance and interrupted before the OS dialog returned.
        let mut interrupted=snapshot(
            &x.profile,&x.preview.digest_sha256,task,None,note).unwrap();
        interrupted.host_instance="lu-plan07c-previous-process-before-crash".into();
        append(&x.profile,interrupted).unwrap();
        let history=history(&x.profile,&x.register.register_id,true).unwrap();
        assert_eq!(history.len(),5);
        assert!(history.iter().any(|e|e.phase==Phase::Interrupted));
        assert!(history.iter().all(|e|e.no_execution_authority&&
            e.no_completion_authority));
        let reopened=history(&x.profile,&x.register.register_id,true).unwrap();
        assert_eq!(reopened.len(),history.len());
        let c=project_register_desktop::library(&x.profile).unwrap();
        let (_p,unchanged,_)=project_register_desktop::load_register(
            &c,&x.register.register_id).unwrap().unwrap();
        assert_eq!(unchanged.latest_sha256,x.register.latest_sha256);
        assert_eq!(unchanged.events.len(),x.register.events.len());
        assert!(!register::latest(&unchanged,task)
            .unwrap().independently_verified_requirement);
        let memory:i64=c.query_row("SELECT value FROM original_human_memory",
            [],|r|r.get(0)).unwrap();
        assert_eq!(memory,7073);
        // Even an immutable receipt field cannot be silently swapped in
        // SQLite and then promoted to a valid human decision.
        c.execute("UPDATE ansf_project_task_update_review_events
            SET event_json=X'7b6e6f70657d' WHERE review_id=?1 AND sequence=1",
            [&refused.review_id]).unwrap();
        assert!(history(&x.profile,&x.register.register_id,false).is_err());
        drop(c);fs::remove_dir_all(x.profile).unwrap();
    }
    #[test]
    fn exact_human_requirement_requires_reconciled_native_source_and_preserves_correction(){
        let x=fixture("exact-criterion");
        let task=&x.proposal.tasks[0].task_id;
        assert!(exact_preflight(&x.profile,&x.root,
            &x.register.register_id,task,1).is_err());
        let c=project_register_desktop::library(&x.profile).unwrap();
        let (_,old,_,g,s)=project_updates_desktop::reload_audited(
            &x.profile,&x.preview.digest_sha256).unwrap();
        drop(c);
        let source=register::source_changed(&old,&x.proposal,&g,&s).unwrap();
        let saved=project_register_desktop::persist(
            &x.profile,&x.proposal,&source,Some(&old.latest_sha256)).unwrap();
        let current=register::latest(&saved,task).unwrap();
        let corrected=register::human_correct(
            &saved,&x.proposal,task,current.version,&register::HumanCorrection{
                summary:"Human corrected the exact source acceptance wording".into(),
                description:"Check original source plus separate integration and installed gates".into(),
                priority:"P0".into(),blocked_by:"Native integration still needs independent tests".into(),
                acceptance_criteria:"Human verifies native source and separately installed behavior".into(),
                reason:"Preserve corrected original acceptance scope".into()
            }).unwrap();
        let current=project_register_desktop::persist(
            &x.profile,&x.proposal,&corrected,Some(&saved.latest_sha256)).unwrap();
        let version=register::latest(&current,task).unwrap().version;
        let (native_g,native_s,message)=exact_preflight(
            &x.profile,&x.root,&x.register.register_id,task,version).unwrap();
        assert!(message.contains("Original wording"));
        let result=exact_commit(&x.profile,&x.root,
            &x.register.register_id,task,version,&(native_g,native_s)).unwrap();
        let verified=register::latest(&result,task).unwrap();
        assert_eq!(verified.summary,
            "Human corrected the exact source acceptance wording");
        assert_eq!(verified.acceptance_criteria,
            "Human verifies native source and separately installed behavior");
        assert!(verified.independently_verified_requirement);
        assert!(!verified.needs_human_reconciliation);
        assert!(!verified.execution_authorized);
        assert_eq!(result.events.last().unwrap().kind,register::RevisionKind::HumanRevalidated);
        assert!(exact_commit(&x.profile,&x.root,&x.register.register_id,
            task,version,&(g,s)).is_err());
        // Changing any evidence after reviewing the OS dialog must deny.
        let newer=register::latest(&result,task).unwrap().version;
        let reviewed=project_status_desktop::classify_selected(&x.root).unwrap();
        fs::write(x.root.join("src/required.rs"),
            b"fn changed_during_operator_dialog(){}\n").unwrap();
        assert!(exact_commit(&x.profile,&x.root,&x.register.register_id,
            task,newer,&reviewed).is_err());
        fs::remove_dir_all(x.profile).unwrap();
    }
    #[test]
    fn original_gguf_model_suggestion_is_review_only_and_receipt_tamper_denied(){
        let x=fixture("model-reference");
        let task=&x.proposal.tasks[0].task_id;
        let (_,_,_,g,s)=project_updates_desktop::reload_audited(
            &x.profile,&x.preview.digest_sha256).unwrap();
        let prompt=suggestions::prompt(&x.preview,&g,&s).unwrap();
        let row=x.preview.candidates.iter().find(|c|&c.task_id==task).unwrap();
        let raw=serde_json::json!({
            "schema":suggestions::RAW_SCHEMA,"previewSha256":x.preview.digest_sha256,
            "candidates":[{
                "taskId":row.task_id,"requirementId":row.original_requirement_id,
                "status":row.current_status,"action":"collect_evidence",
                "evidenceIds":[row.original_requirement_id],
                "reason":"Human should review original native evidence after source changes",
                "nextEvidence":"Collect independent source and integration compiler receipts"
            }]
        }).to_string();
        let model=suggestions::validate(&x.preview,&g,&s,&raw,&prompt,
            "m-test-original-synthetic","s-test-local","g-test-local").unwrap();
        project_update_model_desktop::persist(
            &x.profile,&model,&prompt,&raw).unwrap();
        let submitted=append(&x.profile,snapshot(
            &x.profile,&x.preview.digest_sha256,task,
            Some(&model.digest_sha256),
            "Review original model suggestion only, without accepting work"
        ).unwrap()).unwrap();
        assert!(submitted.model_suggestion_sha256.is_some());
        let current=decide(&x.profile,&submitted,
            Phase::ApprovedForReview,Some(&x.root)).unwrap();
        assert_eq!(current.phase,Phase::ApprovedForReview);
        let c=project_register_desktop::library(&x.profile).unwrap();
        let (_p,r,_)=project_register_desktop::load_register(
            &c,&x.register.register_id).unwrap().unwrap();
        assert_eq!(r.latest_sha256,x.register.latest_sha256);
        assert!(!register::latest(&r,task).unwrap().independently_verified_requirement);
        drop(c);
        let another=append(&x.profile,snapshot(
            &x.profile,&x.preview.digest_sha256,task,
            Some(&model.digest_sha256),
            "Human review of unmodified original model citation"
        ).unwrap()).unwrap();
        let c=project_register_desktop::library(&x.profile).unwrap();
        c.execute("UPDATE ansf_project_task_update_model_suggestions
            SET original_reply_text=X'7b6e6f70657d'
            WHERE suggestion_sha256=?1",[&model.digest_sha256]).unwrap();
        drop(c);
        assert!(decide(&x.profile,&another,
            Phase::ApprovedForReview,Some(&x.root)).is_err());
        assert_eq!(decide(&x.profile,&another,
            Phase::StaleRejected,None).unwrap().phase,Phase::StaleRejected);
        fs::remove_dir_all(x.profile).unwrap();
    }
}
