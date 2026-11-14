//! P0-TOOLS-05: reviewed independent worker result -> EXACT task criterion.
//!
//! A model cannot write this table, select an OS source, certify an outcome,
//! authorize another operation, or accept a task. The operator may ONLY
//! accept/reject an original independently audited witness as *supporting
//! evidence* for an exact criterion after a new OS source selection and scan.
//! Append-only ANSF audit rows NEVER modify the PLAN05 task register/status.
use super::*;
use lu_core::{
    project_task_register as register,
    project_evidence_graph as graph,
};
use rusqlite::{Connection,params,TransactionBehavior};
use serde::{Serialize,Deserialize};

const SCHEMA:&str="lu-project-task-criterion-witness-v1";
const MAX_REVIEW_EVENTS:usize=256;

#[derive(Clone,Serialize,Deserialize,Debug)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct CriterionDecision{
    schema:String,event_id:String,register_id:String,task_id:String,
    task_version:u64,board_sha256:String,register_sha256:String,
    source_scope_sha256:String,graph_sha256:String,status_sha256:String,
    selected_head:Option<String>,source_requirement_id:String,
    acceptance_criterion:String,acceptance_criterion_sha256:String,
    handoff_id:String,receipt_sha256:String,observed_sha256:String,
    tool_id:String,independent_method:String,step_index:u8,
    verdict_at_review:String,decision:String,operator_reason:String,
    previous_event_sha256:Option<String>,
    task_completed:bool,execution_authorized:bool,
}
#[derive(Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct CriterionDecisionView{
    event_id:String,register_id:String,task_id:String,task_version:u64,
    decision:String,operator_reason:String,handoff_id:String,
    independent_method:String,tool_id:String,
    receipt_sha256:String,observed_sha256:String,
    acceptance_criterion:String,source_lease_current:bool,
    historical_verdict:String,
    task_completed:bool,execution_authorized:bool,
    evidence_acceptance_only:bool,
}
fn sha(bytes:&[u8])->String{format!("{:x}",Sha256::digest(bytes))}
fn valid_hash(s:&str)->bool{graph::valid_digest(s)}
fn review_event_valid(e:&CriterionDecision)->bool{
    e.schema==SCHEMA&&e.event_id.starts_with("ev-")&&
    e.register_id.starts_with("lu-register-")&&
    e.task_id.starts_with("lu-task-")&&
    e.task_version>0&&
    !e.handoff_id.is_empty()&&e.handoff_id.starts_with("h-")&&
    valid_hash(&e.receipt_sha256)&&valid_hash(&e.observed_sha256)&&
    valid_hash(&e.board_sha256)&&valid_hash(&e.register_sha256)&&
    valid_hash(&e.graph_sha256)&&valid_hash(&e.status_sha256)&&
    valid_hash(&e.source_scope_sha256)&&
    valid_hash(&e.acceptance_criterion_sha256)&&
    sha(e.acceptance_criterion.as_bytes())==e.acceptance_criterion_sha256&&
    e.acceptance_criterion.len()>=10&&e.acceptance_criterion.len()<=300&&
    e.operator_reason.len()>=10&&e.operator_reason.len()<=500&&
    !e.operator_reason.chars().any(|c|c.is_control()&&c!='\n')&&
    e.source_requirement_id.len()>=3&&
    matches!(e.decision.as_str(),
        "ACCEPTED_AS_SUPPORTING_EVIDENCE"|
        "REJECTED_AS_IRRELEVANT"|
        "NEEDS_MORE_INDEPENDENT_EVIDENCE")&&
    e.verdict_at_review=="VERIFIED_AT_OBSERVATION"&&
    e.step_index<=31&&
    project_task_agent_handoff::compatible_independent_method(
        &e.tool_id,&e.independent_method
    )&&
    !e.task_completed&&!e.execution_authorized&&
    e.previous_event_sha256.as_deref().is_none_or(valid_hash)
}
fn init(conn:&Connection)->Result<(),String>{
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS ansf_project_task_criterion_witness_reviews(
          register_id TEXT NOT NULL,
          task_id TEXT NOT NULL,
          sequence INTEGER NOT NULL CHECK(sequence>0),
          event_sha256 TEXT NOT NULL,
          event_json BLOB NOT NULL,
          no_execution_authority INTEGER NOT NULL CHECK(no_execution_authority=1),
          PRIMARY KEY(register_id,task_id,sequence)
         );"
    ).map_err(|_|"P0_TOOLS05_ANFS_AUDIT_SCHEMA_UNAVAILABLE".to_string())
}
fn current_revision(
    conn:&Connection,reg:&str,task:&str
)->Result<(register::TaskRegister,
          project_task_board_desktop::TaskBoard,
          register::TaskRevision),String>{
    let (_,r,_)=project_register_desktop::load_register(conn,reg)?
        .ok_or("P0_TOOLS05_REGISTER_NOT_FOUND")?;
    let board=project_task_board_desktop::read(conn,&r)?;
    let latest=register::latest(&r,task)
        .ok_or("P0_TOOLS05_TASK_NOT_FOUND")?.clone();
    Ok((r,board,latest))
}
fn history_rows(conn:&Connection,reg:&str,task:&str)
    ->Result<Vec<CriterionDecision>,String>{
    init(conn)?;
    let mut stmt=conn.prepare(
        "SELECT sequence,event_sha256,event_json,no_execution_authority
         FROM ansf_project_task_criterion_witness_reviews
         WHERE register_id=?1 AND task_id=?2 ORDER BY sequence"
    ).map_err(|_|"P0_TOOLS05_AUDIT_QUERY_UNAVAILABLE")?;
    let mut rows=stmt.query(params![reg,task])
        .map_err(|_|"P0_TOOLS05_AUDIT_QUERY_REFUSED")?;
    let mut all=Vec::new();
    let mut prior=None;
    while let Some(row)=rows.next()
        .map_err(|_|"P0_TOOLS05_AUDIT_READ_FAILED")?{
        let index:i64=row.get(0).map_err(|_|"P0_TOOLS05_SEQUENCE_LOST")?;
        let digest:String=row.get(1).map_err(|_|"P0_TOOLS05_SHA_LOST")?;
        let bytes:Vec<u8>=row.get(2).map_err(|_|"P0_TOOLS05_EVENT_LOST")?;
        let permission:i64=row.get(3).map_err(|_|"P0_TOOLS05_AUTHORITY_LOST")?;
        if all.len()>=MAX_REVIEW_EVENTS||
            index!=all.len() as i64+1||
            bytes.len()>8192||sha(&bytes)!=digest||permission!=1{
            return Err("P0_TOOLS05_ORIGINAL_AUDIT_TAMPERED".into())
        }
        let event:CriterionDecision=serde_json::from_slice(&bytes)
            .map_err(|_|"P0_TOOLS05_ORIGINAL_EVENT_INVALID")?;
        if !review_event_valid(&event)||event.register_id!=reg||
            event.task_id!=task||event.previous_event_sha256!=prior{
            return Err("P0_TOOLS05_ORIGINAL_EVENT_CHAIN_BROKEN".into())
        }
        prior=Some(digest);
        all.push(event);
    }
    Ok(all)
}
fn fresh(
    profile:&Path,root:&Path,reg:&str,task:&str,
    board:&str,register_sha:&str,version:u64,
    handoff:&str,receipt:&str,chats:&chat_store::ChatStore
)->Result<(register::TaskRevision,
         project_task_agent_handoff::ExactTaskWitness),String>{
    let revision=project_task_agent_handoff::source_anchor(
        profile,root,reg,task,board,register_sha,version
    )?;
    let witness=project_task_agent_handoff::independently_verified_task_witness(
        profile,reg,task,handoff,receipt,chats
    )?;
    if !valid_hash(&witness.receipt_sha256)||
       !valid_hash(&witness.observed_sha256){
        return Err("P0_TOOLS05_SOURCE_WITNESS_IDENTITY_INVALID".into())
    }
    Ok((revision,witness))
}
fn add(
    profile:&Path,root:&Path,chat:&chat_store::ChatStore,
    reg:&str,task:&str,expected_board:&str,expected_reg:&str,
    version:u64,handoff:&str,receipt:&str,decision:&str,reason:&str,
)->Result<CriterionDecisionView,String>{
    // Native OS approval happened before this call, but approval is not a
    // source or witness proof. Re-scan and reopen the ORIGINAL worker
    // journals after consent, then hold an immediate SQLite transaction
    // for the final immutable task register/board audit lease.
    let (revision,witness)=fresh(
        profile,root,reg,task,expected_board,expected_reg,version,
        handoff,receipt,chat
    )?;
    // Only prospective ACCEPTED supporting-evidence decisions require a
    // typed criterion-method link. Historical v1 journal rows remain valid.
    if decision=="ACCEPTED_AS_SUPPORTING_EVIDENCE"&&
       !project_task_agent_handoff::criterion_supports_method(
           &revision.acceptance_criteria,&witness.tool_id,
           &witness.independent_method
       ){
        return Err("P0_TOOLS05_ACCEPT_REQUIRES_TYPED_CRITERION_METHOD".into())
    }
    let mut conn=project_register_desktop::library(profile)?;
    let tx=conn.transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_|"P0_TOOLS05_AUDIT_BUSY")?;
    let (r,board,latest)=current_revision(&tx,reg,task)?;
    let (archived_graph,_)=project_register_desktop::archived_evidence(
        &tx,&revision.graph_sha256,&revision.status_sha256
    )?;
    if latest.version!=revision.version||
        latest.graph_sha256!=revision.graph_sha256||
        latest.status_sha256!=revision.status_sha256||
        latest.acceptance_criteria!=revision.acceptance_criteria||
        board.sha256!=expected_board||r.latest_sha256!=expected_reg||
        board.review_state!="human_approved"||
        board.source_register_sha256!=r.latest_sha256||
        r.scope_sha256!=archived_graph.project_scope_sha256{
        return Err("P0_TOOLS05_TASK_OR_SOURCE_LEASE_DRIFTED".into())
    }
    let existing=history_rows(&tx,reg,task)?;
    if existing.len()>=MAX_REVIEW_EVENTS{return Err("P0_TOOLS05_HISTORY_BOUNDED".into())}
    if existing.iter().any(|e|e.task_version==version&&
        e.handoff_id==handoff&&e.receipt_sha256==receipt){
        return Err("P0_TOOLS05_ORIGINAL_WITNESS_ALREADY_HUMAN_REVIEWED".into())
    }
    let event=CriterionDecision{
        schema:SCHEMA.into(),event_id:next_id("ev-"),register_id:reg.into(),
        task_id:task.into(),task_version:version,
        board_sha256:expected_board.into(),register_sha256:expected_reg.into(),
        source_scope_sha256:r.scope_sha256,
        graph_sha256:revision.graph_sha256.clone(),
        status_sha256:revision.status_sha256.clone(),
        selected_head:revision.selected_head.clone(),
        source_requirement_id:revision.source_requirement_id.clone(),
        acceptance_criterion:revision.acceptance_criteria.clone(),
        acceptance_criterion_sha256:sha(revision.acceptance_criteria.as_bytes()),
        handoff_id:witness.handoff_id.clone(),
        receipt_sha256:witness.receipt_sha256.clone(),
        observed_sha256:witness.observed_sha256.clone(),
        tool_id:witness.tool_id.clone(),
        independent_method:witness.independent_method.clone(),
        step_index:witness.step_index,
        verdict_at_review:"VERIFIED_AT_OBSERVATION".into(),
        decision:decision.into(),operator_reason:reason.into(),
        previous_event_sha256:existing.last().map(|e|{
            sha(&serde_json::to_vec(e).expect("validated original audit event"))
        }),
        task_completed:false,execution_authorized:false,
    };
    if !review_event_valid(&event){
        return Err("P0_TOOLS05_HUMAN_DECISION_INVALID".into())
    }
    let bytes=serde_json::to_vec(&event)
        .map_err(|_|"P0_TOOLS05_EVENT_SERIALIZATION_FAILED")?;
    if bytes.len()>8192{return Err("P0_TOOLS05_EVENT_TOO_LARGE".into())}
    let hash=sha(&bytes);
    tx.execute(
        "INSERT INTO ansf_project_task_criterion_witness_reviews(
          register_id,task_id,sequence,event_sha256,event_json,
          no_execution_authority) VALUES (?1,?2,?3,?4,?5,1)",
        params![reg,task,existing.len() as i64+1,hash,bytes]
    ).map_err(|_|"P0_TOOLS05_AUDIT_COMMIT_REFUSED")?;
    tx.commit().map_err(|_|"P0_TOOLS05_AUDIT_FINALIZE_FAILED")?;
    Ok(CriterionDecisionView{
        event_id:event.event_id,register_id:event.register_id,task_id:event.task_id,
        task_version:event.task_version,decision:event.decision,
        operator_reason:event.operator_reason,handoff_id:event.handoff_id,
        independent_method:event.independent_method,tool_id:event.tool_id,
        receipt_sha256:event.receipt_sha256,observed_sha256:event.observed_sha256,
        acceptance_criterion:event.acceptance_criterion,
        source_lease_current:true,
        historical_verdict:"VERIFIED_AT_OBSERVATION".into(),
        task_completed:false,execution_authorized:false,
        evidence_acceptance_only:true,
    })
}
fn history(profile:&Path,reg:&str,task:&str)
    ->Result<Vec<CriterionDecisionView>,String>{
    let conn=project_register_desktop::library(profile)?;
    let original=history_rows(&conn,reg,task)?;
    let (r,board,latest)=current_revision(&conn,reg,task)?;
    Ok(original.into_iter().map(|e|{
        let current=board.review_state=="human_approved"&&
            board.source_register_sha256==r.latest_sha256&&
            board.sha256==e.board_sha256&&
            r.latest_sha256==e.register_sha256&&
            latest.version==e.task_version&&
            latest.graph_sha256==e.graph_sha256&&
            latest.status_sha256==e.status_sha256&&
            latest.acceptance_criteria==e.acceptance_criterion&&
            !latest.needs_human_reconciliation;
        CriterionDecisionView{
            event_id:e.event_id,register_id:e.register_id,task_id:e.task_id,
            task_version:e.task_version,decision:e.decision,
            operator_reason:e.operator_reason,handoff_id:e.handoff_id,
            tool_id:e.tool_id,independent_method:e.independent_method,
            receipt_sha256:e.receipt_sha256,observed_sha256:e.observed_sha256,
            acceptance_criterion:e.acceptance_criterion,
            source_lease_current:current,
            historical_verdict:e.verdict_at_review,
            task_completed:false,execution_authorized:false,
            evidence_acceptance_only:true,
        }
    }).collect())
}
/// P0-MEMORY read-only: reopen the immutable original task criterion
/// decisions only for an exact current-profile register/task selector. These
/// decisions do not prove a project source is CURRENT or a task completed.
pub(super) fn original_review_history_for_memory(
    profile:&Path,register:&str,task:&str,
    chats:&chat_store::ChatStore
)->Result<serde_json::Value,String>{
    if !register.starts_with("lu-register-")||!task.starts_with("lu-task-")||
        register.len()>128||task.len()>128||
        !register.bytes().all(|b|b.is_ascii_alphanumeric()||matches!(b,b'-'|b'_'))||
        !task.bytes().all(|b|b.is_ascii_alphanumeric()||matches!(b,b'-'|b'_')){
        return Err("P0_MEMORY_TASK_CRITERION_SELECTOR_INVALID".into())
    }
    let all=history(profile,register,task)?;
    let decisions=all.iter().rev().take(4).map(|event|{
        // A previous human review cannot be inherited by a newer task
        // revision. Reopen original plan, worker and oracle only when the
        // register/board lease still matches, but NEVER infer file freshness
        // without a new OS-selected project source.
        let original_reopened=event.source_lease_current&&
            project_task_agent_handoff::independently_verified_task_witness(
                profile,register,task,&event.handoff_id,
                &event.receipt_sha256,chats
            ).is_ok();
        serde_json::json!({
            "originalHumanDecisionId":event.event_id,
            "taskVersion":event.task_version,
            "decision":event.decision,
            "operatorReason":event.operator_reason.chars()
                .take(200).collect::<String>(),
            "exactCriterion":event.acceptance_criterion.chars()
                .take(300).collect::<String>(),
            "method":event.independent_method,
            "originalReceiptSha256":event.receipt_sha256,
            "independentObservationSha256":event.observed_sha256,
            "originalWorkerOracleReopened":original_reopened,
            "sourceLeaseCurrent":event.source_lease_current,
            "sourceFilesystemRescannedThisTurn":false,
            "taskCompleted":false,"executionAuthorized":false,
            "status":if original_reopened{
                "HISTORICAL_HUMAN_EVIDENCE_DECISION_REQUIRES_FRESH_SOURCE"
            }else{
                "ORIGINAL_RECEIPT_OR_TASK_LEASE_RECONCILIATION_REQUIRED"
            }
        })
    }).collect::<Vec<_>>();
    Ok(serde_json::json!({
        "schema":"lu-p0-memory-criterion-decision-snapshot-v1",
        "originalDecisionCount":all.len(),
        "moreDecisionsNotDisplayed":all.len()>4,
        "decisions":decisions,
        "taskStatusNotChanged":true,
        "historicalHumanEvidenceReviewNotTaskCompletion":true,
        "executionAuthorized":false
    }))
}
#[tauri::command]
pub(crate) async fn lu_project_task_criterion_evidence_history(
    caller:tauri::WebviewWindow,
    state:State<'_,Arc<DesktopState>>,
    register_id:String,task_id:String,
)->Result<Vec<CriterionDecisionView>,String>{
    if caller.label()!="task-list"{
        return Err("P0_TOOLS05_REQUIRES_NATIVE_TASK_BOARD".into())
    }
    let profile=project_register_desktop::profile(state.inner())?;
    tauri::async_runtime::spawn_blocking(move||history(
        &profile,&register_id,&task_id
    )).await.map_err(|_|"P0_TOOLS05_HISTORY_INTERRUPTED")?
}
#[tauri::command]
pub(crate) async fn lu_project_task_review_independent_witness(
    app:AppHandle,caller:tauri::WebviewWindow,
    state:State<'_,Arc<DesktopState>>,
    register_id:String,task_id:String,
    expected_board_sha256:String,expected_register_sha256:String,
    expected_task_version:u64,handoff_id:String,receipt_sha256:String,
    decision:String,operator_reason:String,
)->Result<Option<CriterionDecisionView>,String>{
    if caller.label()!="task-list"{
        return Err("P0_TOOLS05_REQUIRES_NATIVE_TASK_BOARD".into())
    }
    if !matches!(decision.as_str(),
        "ACCEPTED_AS_SUPPORTING_EVIDENCE"|
        "REJECTED_AS_IRRELEVANT"|
        "NEEDS_MORE_INDEPENDENT_EVIDENCE")||
        operator_reason.trim().len()<10||operator_reason.len()>500||
        !valid_hash(&receipt_sha256)||
        !valid_hash(&expected_board_sha256)||
        !valid_hash(&expected_register_sha256)||
        expected_task_version==0{
        return Err("P0_TOOLS05_INVALID_OPERATOR_REVIEW".into())
    }
    let host=Arc::clone(state.inner());
    let profile=project_register_desktop::profile(&host)?;
    // An OS folder picker, not a model or WebView path, chooses the SOURCE.
    let chooser=app.clone();
    let selected=tauri::async_runtime::spawn_blocking(move||
        chooser.dialog().file().blocking_pick_folder()
    ).await.map_err(|_|"P0_TOOLS05_NATIVE_FOLDER_PICKER_INTERRUPTED")?;
    let Some(selected)=selected else{return Ok(None)};
    let root=selected.into_path()
        .map_err(|_|"P0_TOOLS05_NATIVE_RESOURCE_NOT_LOCAL")?;
    let before={
        let profile=profile.clone();let root=root.clone();
        let reg=register_id.clone();let task=task_id.clone();
        let board=expected_board_sha256.clone();
        let reg_sha=expected_register_sha256.clone();
        let handoff=handoff_id.clone();let receipt=receipt_sha256.clone();
        let chats=Arc::clone(&host);
        tauri::async_runtime::spawn_blocking(move||fresh(
            &profile,&root,&reg,&task,&board,&reg_sha,
            expected_task_version,&handoff,&receipt,&chats.chats
        )).await.map_err(|_|"P0_TOOLS05_PREFLIGHT_INTERRUPTED")??
    };
    // Preflight only: the append path repeats this after fresh OS approval.
    if decision=="ACCEPTED_AS_SUPPORTING_EVIDENCE"&&
       !project_task_agent_handoff::criterion_supports_method(
           &before.0.acceptance_criteria,&before.1.tool_id,
           &before.1.independent_method
       ){
        return Err("P0_TOOLS05_ACCEPT_REQUIRES_TYPED_CRITERION_METHOD".into())
    }
    let message=format!(concat!(
        "Review ONE independently audited worker result against the EXACT current project criterion?\n\n",
        "Task: {} · v{}\nRequirement: {}\n\n",
        "Criterion: {}\n\n",
        "Worker tool: {}\nIndependent method: {}\n",
        "Original effect SHA-256: {}\nIndependent observation SHA-256: {}\n\n",
        "Your decision: {}\nReason: {}\n\n",
        "The source was reselected through the OS and rescanned.\n",
        "Confirmation records ONLY a human EVIDENCE DECISION. It does not\n",
        "complete the task, change original task status, publish code,\n",
        "start a worker or authorize any subsequent step."),
        before.0.summary,before.0.version,before.0.source_requirement_id,
        before.0.acceptance_criteria,before.1.tool_id,
        before.1.independent_method,before.1.receipt_sha256,
        before.1.observed_sha256,decision,operator_reason
    );
    if !project_register_desktop::confirm(
        &app,"Lu — Human criterion evidence decision only",message
    ).await?{return Ok(None)}
    let chats=Arc::clone(&host);
    tauri::async_runtime::spawn_blocking(move||add(
        &profile,&root,&chats.chats,&register_id,&task_id,
        &expected_board_sha256,&expected_register_sha256,expected_task_version,
        &handoff_id,&receipt_sha256,&decision,&operator_reason
    ).map(Some)).await.map_err(|_|"P0_TOOLS05_DECISION_INTERRUPTED")?
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn journal_record_never_contains_task_completion_or_execution_authority(){
        let e=CriterionDecision{
            schema:SCHEMA.into(),event_id:"ev-0123".into(),
            register_id:"lu-register-1".into(),task_id:"lu-task-1".into(),
            task_version:1,board_sha256:"a".repeat(64),
            register_sha256:"a".repeat(64),source_scope_sha256:"a".repeat(64),
            graph_sha256:"a".repeat(64),status_sha256:"a".repeat(64),
            selected_head:None,source_requirement_id:"requirement".into(),
            acceptance_criterion:"Independent witness for approved source".into(),
            acceptance_criterion_sha256:sha(
                b"Independent witness for approved source"),
            handoff_id:"h-1".into(),receipt_sha256:"a".repeat(64),
            observed_sha256:"b".repeat(64),
            tool_id:"lu.reviewed.git.head".into(),
            independent_method:"GIT_HEAD_DIRECT_V1".into(),step_index:0,
            verdict_at_review:"VERIFIED_AT_OBSERVATION".into(),
            decision:"ACCEPTED_AS_SUPPORTING_EVIDENCE".into(),
            operator_reason:"Human confirmed relevance to this requirement".into(),
            previous_event_sha256:None,task_completed:false,
            execution_authorized:false,
        };
        assert!(review_event_valid(&e));
        let mut unsafe_event=e.clone();unsafe_event.task_completed=true;
        assert!(!review_event_valid(&unsafe_event));
        let mut unsafe_event=e.clone();unsafe_event.execution_authorized=true;
        assert!(!review_event_valid(&unsafe_event));
    }
}
