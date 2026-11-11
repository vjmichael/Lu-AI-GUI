//! PLAN-06: independently hosted native task list. All durable state is
//! append-only in the EXISTING ANSF SQLite library; there is no model path.
use super::*;
use lu_core::{project_evidence_graph as graph,project_status_classifier as status,
    project_task_register as register,project_task_synthesis as synthesis};
use rusqlite::{Connection,params,OptionalExtension,TransactionBehavior};
use serde::{Deserialize,Serialize};
use std::collections::{BTreeSet,BTreeMap};
use std::path::PathBuf;

const BOARD_SCHEMA:&str="lu-project-task-board-v1";
const EXPORT_SCHEMA:&str="lu-human-reviewed-project-plan-export-v1";

#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct TaskBoard{
    pub schema:String,pub register_id:String,pub scope_sha256:String,
    pub source_register_sha256:String,pub sequence:u64,
    pub ordered_task_ids:Vec<String>,pub review_state:String,
    pub reason:String,pub previous_sha256:Option<String>,
    pub sha256:String,pub no_execution_authority:bool,
}
#[derive(Clone,Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct BoardTask{
    task_id:String,version:u64,group:String,priority:String,
    summary:String,description:String,blocked_by:String,
    acceptance_criteria:String,source_requirement_id:String,
    observed_status:status::Status,needs_human_reconciliation:bool,
    independently_verified_requirement:bool,selected_head:Option<String>,
    graph_sha256:String,status_sha256:String,evidence_count:usize,
}
#[derive(Clone,Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct BoardView{
    board:TaskBoard,source_register_sha256:String,review_current:bool,
    project_scope_sha256:String,original_proposal_sha256:String,
    tasks:Vec<BoardTask>,event_count:usize,
    historical_approval_count:usize,
}
fn digest(board:&TaskBoard)->Result<String,String>{
    let mut unsigned=board.clone();
    unsigned.sha256.clear();
    Ok(format!("{:x}",Sha256::digest(
        serde_json::to_vec(&unsigned).map_err(|_|"PLAN06_BOARD_SERIALIZE")?
    )))
}
fn ids(r:&register::TaskRegister)->Vec<String>{
    r.events.iter().filter(|e|e.kind==register::RevisionKind::Imported)
        .map(|e|e.revision.task_id.clone()).collect()
}
fn verify_board(b:&TaskBoard,r:&register::TaskRegister)->Result<(),String>{
    let all=ids(r);
    let expected:BTreeSet<&str>=all.iter().map(String::as_str).collect();
    let provided:BTreeSet<&str>=b.ordered_task_ids.iter().map(String::as_str).collect();
    if b.schema!=BOARD_SCHEMA||b.register_id!=r.register_id||
        b.scope_sha256!=r.scope_sha256||!b.no_execution_authority||
        b.ordered_task_ids.len()!=all.len()||expected.len()!=all.len()||
        provided.len()!=b.ordered_task_ids.len()||provided!=expected||
        !matches!(b.review_state.as_str(),"draft"|"human_approved")||
        b.reason.len()<5||b.reason.len()>250||
        b.source_register_sha256.len()!=64||
        !graph::valid_digest(&b.source_register_sha256)||
        b.sha256!=digest(b)?{
        return Err("PLAN06_INVALID_OR_UNAUTHORIZED_BOARD".into())
    }
    Ok(())
}
fn baseline_at(r:&register::TaskRegister,source_sha:&str)->Result<TaskBoard,String>{
    let mut b=TaskBoard{
        schema:BOARD_SCHEMA.into(),register_id:r.register_id.clone(),
        scope_sha256:r.scope_sha256.clone(),
        source_register_sha256:source_sha.into(),
        sequence:0,ordered_task_ids:ids(r),review_state:"draft".into(),
        reason:"Imported original PLAN05 task order, not human approved".into(),
        previous_sha256:None,sha256:String::new(),
        no_execution_authority:true,
    };
    b.sha256=digest(&b)?;
    verify_board(&b,r)?;
    Ok(b)
}
fn baseline(r:&register::TaskRegister)->Result<TaskBoard,String>{
    baseline_at(r,&r.latest_sha256)
}
fn init(conn:&Connection)->Result<(),String>{
    conn.execute_batch("CREATE TABLE IF NOT EXISTS ansf_project_task_board_revisions(
        register_id TEXT NOT NULL,
        sequence INTEGER NOT NULL CHECK(sequence>0),
        source_register_sha256 TEXT NOT NULL,
        previous_sha256 TEXT,
        snapshot_sha256 TEXT NOT NULL,
        snapshot_json BLOB NOT NULL,
        no_execution_authority INTEGER NOT NULL CHECK(no_execution_authority=1),
        PRIMARY KEY(register_id,sequence)
    );").map_err(|_|"PLAN06_BOARD_SCHEMA_FAILED")?;
    Ok(())
}
fn read(conn:&Connection,r:&register::TaskRegister)->Result<TaskBoard,String>{
    init(conn)?;
    let mut stmt=conn.prepare("SELECT sequence,source_register_sha256,previous_sha256,
        snapshot_sha256,snapshot_json,no_execution_authority
        FROM ansf_project_task_board_revisions WHERE register_id=?1 ORDER BY sequence")
        .map_err(|_|"PLAN06_BOARD_HISTORY_QUERY")?;
    let mut rows=stmt.query([&r.register_id]).map_err(|_|"PLAN06_BOARD_HISTORY_OPEN")?;
    let mut prior=baseline(r)?;
    let mut n=0_u64;
    while let Some(row)=rows.next().map_err(|_|"PLAN06_BOARD_HISTORY_READ")?{
        let seq:i64=row.get(0).map_err(|_|"PLAN06_BOARD_SEQUENCE")?;
        let source:String=row.get(1).map_err(|_|"PLAN06_BOARD_SOURCE")?;
        let prev:Option<String>=row.get(2).map_err(|_|"PLAN06_BOARD_PREVIOUS")?;
        let sha:String=row.get(3).map_err(|_|"PLAN06_BOARD_SHA")?;
        let bytes:Vec<u8>=row.get(4).map_err(|_|"PLAN06_BOARD_BYTES")?;
        let no_exec:i64=row.get(5).map_err(|_|"PLAN06_BOARD_AUTHORITY")?;
        n+=1;
        // Board sequence zero is anchored to the source register at the
        // FIRST review, not its possibly newer revision after later edits.
        // Otherwise a subsequent PLAN05 correction makes valid historical
        // board snapshots fail audit at startup.
        if n==1{prior=baseline_at(r,&source)?;}
        if n>1024||seq!=n as i64||no_exec!=1||bytes.len()>131072{
            return Err("PLAN06_INVALID_BOARD_ROW".into())
        }
        let b:TaskBoard=serde_json::from_slice(&bytes)
            .map_err(|_|"PLAN06_BOARD_JSON_CORRUPT")?;
        verify_board(&b,r)?;
        if b.sequence!=n||b.sha256!=sha||
            b.previous_sha256!=Some(prior.sha256.clone())||
            prev!=b.previous_sha256||source!=b.source_register_sha256{
            return Err("PLAN06_BOARD_CHAIN_BROKEN".into())
        }
        // The original PLAN05 register version must have existed in the
        // verified immutable revision archive; not invented by a renderer.
        let exists:i64=conn.query_row(
            "SELECT COUNT(*) FROM ansf_project_task_register_revisions
             WHERE register_id=?1 AND latest_sha256=?2",
            params![r.register_id,b.source_register_sha256],|row|row.get(0)
        ).map_err(|_|"PLAN06_SOURCE_REGISTER_LOOKUP_FAILED")?;
        if exists!=1{return Err("PLAN06_BOARD_SOURCE_REGISTER_NOT_ARCHIVED".into())}
        prior=b;
    }
    Ok(prior)
}
fn next_board(r:&register::TaskRegister,old:&TaskBoard,order:Vec<String>,
    review_state:&str,reason:&str)->Result<TaskBoard,String>{
    if old.sequence>=1024{return Err("PLAN06_MAX_BOARD_REVISIONS".into())}
    let mut next=TaskBoard{
        schema:BOARD_SCHEMA.into(),register_id:r.register_id.clone(),
        scope_sha256:r.scope_sha256.clone(),
        source_register_sha256:r.latest_sha256.clone(),
        sequence:old.sequence+1,ordered_task_ids:order,
        review_state:review_state.into(),reason:reason.into(),
        previous_sha256:Some(old.sha256.clone()),sha256:String::new(),
        no_execution_authority:true,
    };
    next.sha256=digest(&next)?;
    verify_board(&next,r)?;
    Ok(next)
}
fn write(profile:&Path,id:&str,expected_board:&str,expected_source:&str,
    next_order:Vec<String>,approve:bool)->Result<TaskBoard,String>{
    let mut conn=project_register_desktop::library(profile)?;
    let tx=conn.transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_|"PLAN06_BOARD_IMMEDIATE_TRANSACTION")?;
    let (_p,r,_)=project_register_desktop::load_register(&tx,id)?
        .ok_or("PLAN06_REGISTER_NOT_FOUND")?;
    if expected_source!=r.latest_sha256{
        return Err("PLAN06_STALE_REGISTER_HEAD".into())
    }
    let old=read(&tx,&r)?;
    if expected_board!=old.sha256{
        return Err("PLAN06_STALE_BOARD_HEAD".into())
    }
    let next=if approve{
        if next_order!=old.ordered_task_ids||
           old.source_register_sha256!=r.latest_sha256{
            return Err("PLAN06_APPROVAL_REQUIRES_CURRENT_REVIEWED_ORDER".into())
        }
        if old.review_state=="human_approved"{
            return Err("PLAN06_PLAN_ALREADY_APPROVED".into())
        }
        for task_id in &old.ordered_task_ids{
            let last=register::latest(&r,task_id)
                .ok_or("PLAN06_MISSING_ORIGINAL_TASK")?;
            // A source-changed task is not safe to approve as a current plan
            // until its exact original requirement was explicitly re-reviewed.
            if last.needs_human_reconciliation&&r.events.iter().rev()
                .find(|e|e.revision.task_id==*task_id)
                .is_some_and(|e|e.kind==register::RevisionKind::SourceChanged){
                return Err("PLAN06_SOURCE_DRIFT_REQUIRES_HUMAN_RECONCILIATION".into())
            }
        }
        next_board(&r,&old,next_order,"human_approved",
            "Human approved reviewed plan order, NOT execution or task acceptance")?
    }else{
        if next_order==old.ordered_task_ids&&old.source_register_sha256==r.latest_sha256{
            return Err("PLAN06_NO_ORDER_CHANGE".into())
        }
        next_board(&r,&old,next_order,"draft",
            "Human reordered task plan without accepting implementation")?
    };
    let bytes=serde_json::to_vec(&next).map_err(|_|"PLAN06_BOARD_ENCODE")?;
    tx.execute("INSERT INTO ansf_project_task_board_revisions(
        register_id,sequence,source_register_sha256,previous_sha256,
        snapshot_sha256,snapshot_json,no_execution_authority
        )VALUES(?1,?2,?3,?4,?5,?6,1)",
        params![next.register_id,next.sequence as i64,next.source_register_sha256,
            next.previous_sha256,next.sha256,bytes]
    ).map_err(|_|"PLAN06_BOARD_APPEND_FAILED")?;
    let back:Vec<u8>=tx.query_row(
        "SELECT snapshot_json FROM ansf_project_task_board_revisions
         WHERE register_id=?1 AND sequence=?2",
        params![id,next.sequence as i64],|row|row.get(0)
    ).map_err(|_|"PLAN06_BOARD_READBACK_MISSING")?;
    if back!=bytes{return Err("PLAN06_BOARD_READBACK_CHANGED".into())}
    tx.commit().map_err(|_|"PLAN06_BOARD_COMMIT_FAILED")?;
    Ok(next)
}
fn view(r:&register::TaskRegister,b:TaskBoard)->Result<BoardView,String>{
    verify_board(&b,r)?;
    let mut tasks=Vec::new();
    for id in &b.ordered_task_ids{
        let v=register::latest(r,id).ok_or("PLAN06_UNKNOWN_VIEW_TASK")?;
        tasks.push(BoardTask{
            task_id:id.clone(),version:v.version,group:v.group.clone(),
            priority:v.priority.clone(),summary:v.summary.clone(),
            description:v.description.clone(),blocked_by:v.blocked_by.clone(),
            acceptance_criteria:v.acceptance_criteria.clone(),
            source_requirement_id:v.source_requirement_id.clone(),
            observed_status:v.observed_status,needs_human_reconciliation:v.needs_human_reconciliation,
            independently_verified_requirement:v.independently_verified_requirement,
            selected_head:v.selected_head.clone(),graph_sha256:v.graph_sha256.clone(),
            status_sha256:v.status_sha256.clone(),evidence_count:v.evidence_ids.len(),
        })
    }
    Ok(BoardView{
        review_current:b.review_state=="human_approved"&&
            b.source_register_sha256==r.latest_sha256,
        board:b,source_register_sha256:r.latest_sha256.clone(),
        project_scope_sha256:r.scope_sha256.clone(),
        original_proposal_sha256:r.original_proposal_sha256.clone(),
        tasks,event_count:r.events.len(),
        historical_approval_count:0,
    })
}
fn load_views(profile:&Path)->Result<Vec<BoardView>,String>{
    let conn=project_register_desktop::library(profile)?;
    let mut stmt=conn.prepare("SELECT DISTINCT register_id
        FROM ansf_project_task_register_revisions ORDER BY register_id LIMIT 32")
        .map_err(|_|"PLAN06_LIST_QUERY")?;
    let rows=stmt.query_map([],|row|row.get::<_,String>(0))
        .map_err(|_|"PLAN06_LIST_FETCH")?;
    let keys=rows.collect::<Result<Vec<_>,_>>()
        .map_err(|_|"PLAN06_LIST_COLLECT")?;
    let mut out=Vec::new();
    for id in keys{
        let (_p,r,_)=project_register_desktop::load_register(&conn,&id)?
            .ok_or("PLAN06_REGISTER_LOST")?;
        let board=read(&conn,&r)?;
        let mut v=view(&r,board)?;
        v.historical_approval_count=conn.query_row(
            "SELECT COUNT(*) FROM ansf_project_task_board_revisions
             WHERE register_id=?1 AND CAST(snapshot_json AS TEXT) LIKE ?2",
            params![id,"%\"reviewState\":\"human_approved\"%"],|row|row.get::<_,i64>(0)
        ).map_err(|_|"PLAN06_HISTORY_COUNT_FAILED")?.max(0) as usize;
        out.push(v)
    }
    Ok(out)
}
fn evidence(profile:&Path,id:&str,task_id:&str)
 ->Result<serde_json::Value,String>{
    let conn=project_register_desktop::library(profile)?;
    let (_p,r,_)=project_register_desktop::load_register(&conn,id)?
        .ok_or("PLAN06_REGISTER_NOT_FOUND")?;
    let _=read(&conn,&r)?;
    let v=register::latest(&r,task_id).ok_or("PLAN06_TASK_NOT_FOUND")?;
    let (g,s)=project_register_desktop::archived_evidence(
        &conn,&v.graph_sha256,&v.status_sha256)?;
    let finding=s.findings.iter()
        .find(|f|f.requirement_id==v.source_requirement_id);
    let mut relevant:BTreeSet<String>=v.evidence_ids.iter().cloned().collect();
    relevant.insert(v.source_requirement_id.clone());
    for edge in &g.edges{
        if edge.from==v.source_requirement_id{
            relevant.insert(edge.to.clone());
        }
    }
    let nodes=g.nodes.iter().filter(|n|relevant.contains(&n.id))
        .take(64).collect::<Vec<_>>();
    let edges=g.edges.iter().filter(|e|
        relevant.contains(&e.from)&&relevant.contains(&e.to))
        .take(96).collect::<Vec<_>>();
    let history=r.events.iter().filter(|e|e.revision.task_id==task_id)
        .map(|e|serde_json::json!({
            "sequence":e.sequence,"kind":e.kind,"reason":e.reason,
            "version":e.revision.version,"sourceHead":e.revision.selected_head,
            "verifiedExactRequirement":e.revision.independently_verified_requirement
        })).collect::<Vec<_>>();
    Ok(serde_json::json!({
        "schema":"lu-plan06-task-evidence-v1",
        "taskId":task_id,"registerId":id,
        "originalRequirementId":v.source_requirement_id,
        "sourceGraphSha256":g.sha256,"sourceStatusSha256":s.digest_sha256,
        "selectedHead":g.selected_head,"observedAtUnix":s.observed_at_unix,
        "finding":finding,"nodes":nodes,"edges":edges,"history":history,
        "scopeComplete":g.complete&&s.complete,"noExecutionAuthority":true,
        "warning":"Source citations and native path tests are not authorization or general completion evidence."
    }))
}
fn export_bytes(profile:&Path,id:&str,expected_board:&str,
    expected_source:&str)->Result<Vec<u8>,String>{
    let conn=project_register_desktop::library(profile)?;
    let (p,r,_)=project_register_desktop::load_register(&conn,id)?
        .ok_or("PLAN06_REGISTER_NOT_FOUND")?;
    let b=read(&conn,&r)?;
    if b.sha256!=expected_board||r.latest_sha256!=expected_source{
        return Err("PLAN06_EXPORT_STALE_REVIEW".into())
    }
    let v=view(&r,b)?;
    let outcome=serde_json::json!({
        "schema":EXPORT_SCHEMA,"source":"LU_NATIVE_ANSF_REVIEWED_RECORDS",
        "originalProposalSha256":p.digest_sha256,
        "originalModelReplySha256":p.original_reply_sha256,
        "boardSha256":v.board.sha256,
        "sourceRegisterSha256":r.latest_sha256,
        "humanApprovalCurrent":v.review_current,
        "planReviewState":v.board.review_state,
        "projectScopeSha256":r.scope_sha256,
        "tasks":v.tasks,"revisionEvents":r.events,
        "noExecutionAuthority":true,"acceptedTaskExecution":false,
        "warning":"Plan approval means a person reviewed ordering. It never grants tool execution or proves task completion."
    });
    let bytes=serde_json::to_vec_pretty(&outcome).map_err(|_|"PLAN06_EXPORT_ENCODE")?;
    if bytes.len()>2*1024*1024{return Err("PLAN06_EXPORT_TOO_LARGE".into())}
    Ok(bytes)
}
fn save_json_new(path:&Path,bytes:&[u8])->Result<String,String>{
    if path.extension().and_then(|s|s.to_str())!=Some("json"){
        return Err("PLAN06_EXPORT_REQUIRES_JSON_SUFFIX".into())
    }
    if path.exists(){return Err("PLAN06_EXPORT_REFUSES_OVERWRITE".into())}
    let parent=path.parent().ok_or("PLAN06_NO_EXPORT_PARENT")?;
    let meta=fs::symlink_metadata(parent).map_err(|_|"PLAN06_EXPORT_PARENT_MISSING")?;
    if !meta.is_dir()||meta.file_type().is_symlink(){
        return Err("PLAN06_EXPORT_UNSAFE_PARENT".into())
    }
    let mut file=std::fs::OpenOptions::new().write(true).create_new(true).open(path)
        .map_err(|_|"PLAN06_EXPORT_CREATE_NEW_FAILED")?;
    use std::io::Write as _;
    if let Err(_)=file.write_all(bytes).and_then(|_|file.sync_all()){
        drop(file);let _=fs::remove_file(path);
        return Err("PLAN06_EXPORT_WRITE_OR_SYNC_FAILED".into())
    }
    drop(file);
    let actual=fs::read(path).map_err(|_|"PLAN06_EXPORT_READBACK_FAILED")?;
    if actual!=bytes{return Err("PLAN06_EXPORT_INDEPENDENT_READBACK_FAILED".into())}
    Ok(format!("{:x}",Sha256::digest(bytes)))
}
fn require_board(caller:&tauri::WebviewWindow)->Result<(),String>{
    if caller.label()!="task-list"{
        return Err("PLAN06_REQUIRES_INDEPENDENT_OPERATOR_TASK_WINDOW".into())
    }
    Ok(())
}
fn profile_for(state:&Arc<DesktopState>)->Result<PathBuf,String>{
    project_register_desktop::profile(state)
}
#[tauri::command]
pub(crate) async fn lu_project_board_list(
    caller:tauri::WebviewWindow,state:State<'_,Arc<DesktopState>>
)->Result<Vec<BoardView>,String>{
    require_board(&caller)?;
    let profile=profile_for(state.inner())?;
    tauri::async_runtime::spawn_blocking(move||load_views(&profile))
        .await.map_err(|_|"PLAN06_BOARD_LIST_INTERRUPTED")?
}
#[tauri::command]
pub(crate) async fn lu_project_board_evidence(
    caller:tauri::WebviewWindow,state:State<'_,Arc<DesktopState>>,
    register_id:String,task_id:String
)->Result<serde_json::Value,String>{
    require_board(&caller)?;
    let profile=profile_for(state.inner())?;
    tauri::async_runtime::spawn_blocking(move||
        evidence(&profile,&register_id,&task_id))
        .await.map_err(|_|"PLAN06_EVIDENCE_INTERRUPTED")?
}
#[tauri::command]
pub(crate) async fn lu_project_board_reorder(
    app:AppHandle,caller:tauri::WebviewWindow,
    state:State<'_,Arc<DesktopState>>,register_id:String,
    expected_board_sha256:String,expected_register_sha256:String,
    ordered_task_ids:Vec<String>
)->Result<Option<TaskBoard>,String>{
    require_board(&caller)?;
    let profile=profile_for(state.inner())?;
    if ordered_task_ids.len()>16||!graph::valid_digest(&expected_board_sha256)||
        !graph::valid_digest(&expected_register_sha256){
        return Err("PLAN06_INVALID_REORDER_REQUEST".into())
    }
    if !project_register_desktop::confirm(&app,"Lu — Reorder human plan",
        format!("Save this reviewed sequence of {} tasks?\n\
        This appends a durable draft revision. It does NOT accept a task, \
        execute code, grant model tools or approve source changes.",ordered_task_ids.len())
    ).await?{return Ok(None)}
    tauri::async_runtime::spawn_blocking(move||
        write(&profile,&register_id,&expected_board_sha256,
            &expected_register_sha256,ordered_task_ids,false).map(Some))
        .await.map_err(|_|"PLAN06_REORDER_INTERRUPTED")?
}
#[tauri::command]
pub(crate) async fn lu_project_board_approve(
    app:AppHandle,caller:tauri::WebviewWindow,
    state:State<'_,Arc<DesktopState>>,register_id:String,
    expected_board_sha256:String,expected_register_sha256:String
)->Result<Option<TaskBoard>,String>{
    require_board(&caller)?;
    let profile=profile_for(state.inner())?;
    let review=load_views(&profile)?.into_iter().find(|v|v.board.register_id==register_id)
        .ok_or("PLAN06_NO_REGISTER_TO_APPROVE")?;
    if review.board.sha256!=expected_board_sha256||
       review.source_register_sha256!=expected_register_sha256{
        return Err("PLAN06_PLAN_CHANGED_BEFORE_APPROVAL".into())
    }
    let order=review.board.ordered_task_ids;
    if !project_register_desktop::confirm(&app,"Lu — Approve task plan only",
        format!("Approve the ordering and wording of {} reviewed tasks?\n\
        The tasks remain unexecuted; this DOES NOT grant an action, tool \
        permission, scheduling, source acceptance or completion. \
        Changes after this approval invalidate its current status.",order.len())
    ).await?{return Ok(None)}
    tauri::async_runtime::spawn_blocking(move||
        write(&profile,&register_id,&expected_board_sha256,
            &expected_register_sha256,order,true).map(Some))
        .await.map_err(|_|"PLAN06_PLAN_APPROVAL_INTERRUPTED")?
}
#[tauri::command]
pub(crate) async fn lu_project_board_correct(
    app:AppHandle,caller:tauri::WebviewWindow,
    state:State<'_,Arc<DesktopState>>,register_id:String,task_id:String,
    expected_version:u64,correction:register::HumanCorrection
)->Result<Option<BoardView>,String>{
    require_board(&caller)?;
    if !project_register_desktop::confirm(&app,"Lu — Confirm task wording",
        format!("Save your own correction of {} version {}?\n\
        Reason: {}\nSummary: {}\nAcceptance criterion: {}\n\
        Original requirement and source evidence cannot be changed. \
        This invalidates prior plan approval and NEVER authorizes execution.",
        task_id,expected_version,correction.reason,correction.summary,correction.acceptance_criteria)
    ).await?{return Ok(None)}
    let profile=profile_for(state.inner())?;
    tauri::async_runtime::spawn_blocking(move||{
        let conn=project_register_desktop::library(&profile)?;
        let (p,old,_)=project_register_desktop::load_register(&conn,&register_id)?
            .ok_or("PLAN06_REGISTER_NOT_FOUND")?;
        drop(conn);
        let corrected=register::human_correct(&old,&p,&task_id,
            expected_version,&correction)?;
        let new=project_register_desktop::persist(
            &profile,&p,&corrected,Some(&old.latest_sha256))?;
        let conn=project_register_desktop::library(&profile)?;
        let b=read(&conn,&new)?;
        view(&new,b).map(Some)
    }).await.map_err(|_|"PLAN06_CORRECTION_INTERRUPTED")?
}
#[tauri::command]
pub(crate) async fn lu_project_board_export(
    app:AppHandle,caller:tauri::WebviewWindow,
    state:State<'_,Arc<DesktopState>>,register_id:String,
    expected_board_sha256:String,expected_register_sha256:String
)->Result<Option<String>,String>{
    require_board(&caller)?;
    let profile=profile_for(state.inner())?;
    // First verify immutable history against the selected original ANSF
    // source, then ask for explicit OS review and a new destination file.
    export_bytes(&profile,&register_id,&expected_board_sha256,
        &expected_register_sha256)?;
    if !project_register_desktop::confirm(&app,"Lu — Export reviewed plan",
        "Export the selected plan as a new JSON file? This includes the \
        original evidence identity and human revision history. It contains \
        NO execution grant or task-completion claim and will never overwrite \
        an existing file.".into()).await?{return Ok(None)}
    let picker=app.clone();
    let Some(path)=tauri::async_runtime::spawn_blocking(move||
        picker.dialog().file().add_filter("JSON",&["json"]).blocking_save_file()
    ).await.map_err(|_|"PLAN06_EXPORT_DIALOG_INTERRUPTED")? else{
        return Ok(None)
    };
    let destination=path.into_path().map_err(|_|"PLAN06_EXPORT_PATH_INVALID")?;
    tauri::async_runtime::spawn_blocking(move||{
        let data=export_bytes(&profile,&register_id,
            &expected_board_sha256,&expected_register_sha256)?;
        save_json_new(&destination,&data).map(Some)
    }).await.map_err(|_|"PLAN06_EXPORT_IO_INTERRUPTED")?
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn human_order_and_review_never_grant_authority(){
        let r=register::TaskRegister{
            schema:register::SCHEMA.into(),register_id:"lu-register-test".into(),
            scope_sha256:"a".repeat(64),original_proposal_sha256:"b".repeat(64),
            events:vec![],latest_sha256:"c".repeat(64),no_execution_authority:true
        };
        let base=baseline(&r).unwrap();
        assert_eq!(base.sequence,0);
        assert_eq!(base.review_state,"draft");
        let approved=next_board(&r,&base,vec![],"human_approved",
            "Human reviewed task order, no execution").unwrap();
        assert!(approved.no_execution_authority);
        assert_eq!(approved.previous_sha256,Some(base.sha256.clone()));
        verify_board(&approved,&r).unwrap();
        let mut malicious=approved.clone();malicious.no_execution_authority=false;
        assert!(verify_board(&malicious,&r).is_err());
        let mut tampered=approved;tampered.reason="forged acceptance".into();
        assert!(verify_board(&tampered,&r).is_err());
    }
    #[test]
    fn export_requires_new_json_and_preserves_existing_bytes(){
        let dir=std::env::temp_dir().join(format!("lu-plan06-export-{}",std::process::id()));
        let _=fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let file=dir.join("plan.json");
        let bytes=b"{\"noExecutionAuthority\":true}";
        let digest=save_json_new(&file,bytes).unwrap();
        assert_eq!(digest,format!("{:x}",Sha256::digest(bytes)));
        assert!(save_json_new(&file,b"overwrite").is_err());
        assert_eq!(fs::read(&file).unwrap(),bytes);
        assert!(save_json_new(&dir.join("plan.exe"),bytes).is_err());
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn ansf_approval_restart_source_revision_and_corruption_matrix(){
        let profile=std::env::temp_dir().join(format!(
            "lu-plan06-native-board-{}",std::process::id()));
        let root=profile.join("original-project");
        let _=fs::remove_dir_all(&profile);
        fs::create_dir_all(root.join("docs")).unwrap();
        fs::create_dir_all(profile.join("ansf")).unwrap();
        fs::write(root.join("docs/REQUIREMENTS.md"),
            "# Requirements\n- [ ] Must include src/missing.rs\n").unwrap();
        let conn=Connection::open(profile.join("ansf/library.db")).unwrap();
        conn.execute_batch("CREATE TABLE preserved_memory(x INTEGER);
            INSERT INTO preserved_memory VALUES(606);").unwrap();
        drop(conn);
        let (g,s)=project_status_desktop::classify_selected(&root).unwrap();
        let cap=synthesis::build_capsule(&g,&s).unwrap();
        let req=&cap.requirements[0];
        let raw=serde_json::json!({"schema":synthesis::RAW_SCHEMA,"tasks":[{
            "requirementId":req.requirement_id,"status":req.status,
            "summary":"Verify original source",
            "description":"Human review of the exact original requirement and source",
            "priority":"P1","group":"01 Source checks",
            "evidenceIds":[req.requirement_id],"dependencyIndices":[],
            "acceptanceCriteria":"Verify exact original source file using native metadata",
            "blockedBy":"Missing accepted original source receipt"
        }]}).to_string();
        let proposal=synthesis::validate_model_response(
            &g,&s,&raw,&synthesis::model_prompt(&cap).unwrap(),
            "m-actual-contract","s-test","g-test").unwrap();
        project_task_desktop::persist_proposal(&profile,&g,&s,&proposal,&raw).unwrap();
        let imported=register::import(&proposal).unwrap();
        let saved=project_register_desktop::persist(
            &profile,&proposal,&imported,None).unwrap();
        let conn=project_register_desktop::library(&profile).unwrap();
        let board=read(&conn,&saved).unwrap();
        assert_eq!(board.sequence,0);
        assert!(!view(&saved,board.clone()).unwrap().review_current);
        drop(conn);
        let approved=write(&profile,&saved.register_id,&board.sha256,
            &saved.latest_sha256,board.ordered_task_ids.clone(),true).unwrap();
        assert_eq!(approved.sequence,1);
        let conn=project_register_desktop::library(&profile).unwrap();
        let reread=read(&conn,&saved).unwrap();
        assert_eq!(reread.sha256,approved.sha256);
        assert!(view(&saved,reread).unwrap().review_current);
        let source=evidence(&profile,&saved.register_id,
            &proposal.tasks[0].task_id).unwrap();
        assert!(source["finding"]["requirementId"].is_string());
        let exported=export_bytes(&profile,&saved.register_id,
            &approved.sha256,&saved.latest_sha256).unwrap();
        let json:serde_json::Value=serde_json::from_slice(&exported).unwrap();
        assert_eq!(json["humanApprovalCurrent"],true);
        assert_eq!(json["noExecutionAuthority"],true);
        assert_eq!(json["acceptedTaskExecution"],false);
        drop(conn);
        let changed=register::human_correct(&saved,&proposal,
            &proposal.tasks[0].task_id,1,&register::HumanCorrection{
                summary:"Human reviewed task name".into(),
                description:"Human clarifies source and acceptance without execution".into(),
                priority:"P0".into(),blocked_by:"Native evidence still missing".into(),
                acceptance_criteria:"Independently review selected source HEAD and native proof".into(),
                reason:"Human clarified order and original acceptance scope".into()
            }).unwrap();
        let corrected=project_register_desktop::persist(
            &profile,&proposal,&changed,Some(&saved.latest_sha256)).unwrap();
        let conn=project_register_desktop::library(&profile).unwrap();
        let stale=read(&conn,&corrected).unwrap();
        assert_eq!(stale.sha256,approved.sha256);
        assert!(!view(&corrected,stale.clone()).unwrap().review_current);
        assert!(write(&profile,&saved.register_id,&stale.sha256,
            &saved.latest_sha256,stale.ordered_task_ids.clone(),true).is_err());
        drop(conn);
        let rebase=write(&profile,&corrected.register_id,&stale.sha256,
            &corrected.latest_sha256,stale.ordered_task_ids.clone(),false).unwrap();
        assert_eq!(rebase.review_state,"draft");
        let accepted=write(&profile,&corrected.register_id,&rebase.sha256,
            &corrected.latest_sha256,rebase.ordered_task_ids.clone(),true).unwrap();
        assert_eq!(accepted.sequence,3);
        let conn=project_register_desktop::library(&profile).unwrap();
        assert_eq!(read(&conn,&corrected).unwrap().sha256,accepted.sha256);
        let intact:i64=conn.query_row(
            "SELECT x FROM preserved_memory",[],|row|row.get(0)).unwrap();
        assert_eq!(intact,606);
        conn.execute("UPDATE ansf_project_task_board_revisions
            SET snapshot_json=X'7b6f6f7073' WHERE register_id=?1 AND sequence=1",
            [&corrected.register_id]).unwrap();
        assert!(read(&conn,&corrected).is_err());
        drop(conn);
        fs::remove_dir_all(profile).unwrap();
    }

}
