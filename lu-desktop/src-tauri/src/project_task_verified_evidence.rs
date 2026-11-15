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
// CORE-MERGE-C3C — original whole-task COMPLETION is a separate user
// decision and source-verified journal, not an AGENT step receipt or model
// answer. V1 can independently decide ONLY a task whose entire human-
// reviewed criterion is the exact native-source predicate below. Other
// compound/free-text/app-specific criteria remain BLOCKED for explicit
// reviewed replanning and specialist result validators.
const COMPLETION_SCHEMA:&str="lu-c3-whole-task-completion-decision-v1";
const NATIVE_SOURCE_ONLY_CONTRACT:&str="LU_COMPLETE_V1:CURRENT_NATIVE_REQUIREMENT";
const MAX_COMPLETION_EVENTS:usize=128;
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct CompletionDecision{
    schema:String,event_id:String,register_id:String,task_id:String,
    task_version:u64,board_sha256:String,register_sha256:String,
    graph_sha256:String,status_sha256:String,
    acceptance_criterion_sha256:String,decision:String,operator_reason:String,
    previous_event_sha256:Option<String>,
    // At-observation evidence; not an execution grant or a lifetime source lease.
    source_rescanned:bool,native_requirement_accepted:bool,
    all_criteria_independently_verified:bool,dependencies_completed:bool,
    human_authorized:bool,task_completed_at_observation:bool,
    execution_authorized:bool,
}
#[derive(Clone,Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct CompletionDecisionView{
    event_id:String,decision:String,task_version:u64,
    operator_reason:String,source_rescanned_at_decision:bool,
    all_criteria_independently_verified_at_decision:bool,
    task_completed_at_observation:bool,
    // History queries do not rescan a selected native project. A previous
    // completion is historical until independently reopened in reconcile.
    current_source_reverified_in_this_view:bool,
    execution_authorized:bool,
}
fn completion_decision_valid(e:&CompletionDecision)->bool{
    let complete=e.decision=="COMPLETED_AT_REVIEW";
    e.schema==COMPLETION_SCHEMA&&e.event_id.starts_with("ev-")&&
    e.register_id.starts_with("lu-register-")&&e.task_id.starts_with("lu-task-")&&
    e.task_version>0&&valid_hash(&e.board_sha256)&&
    valid_hash(&e.register_sha256)&&valid_hash(&e.graph_sha256)&&
    valid_hash(&e.status_sha256)&&valid_hash(&e.acceptance_criterion_sha256)&&
    e.operator_reason.len()>=10&&e.operator_reason.len()<=500&&
    !e.operator_reason.chars().any(|c|c.is_control()&&c!='\n')&&
    matches!(e.decision.as_str(),
        "COMPLETED_AT_REVIEW"|"BLOCKED"|"UNCERTAIN"|"REPLAN_REQUIRED")&&
    e.human_authorized&&!e.execution_authorized&&
    e.task_completed_at_observation==complete&&
    (!complete || (e.source_rescanned&&
        e.native_requirement_accepted&&
        e.all_criteria_independently_verified&&
        e.dependencies_completed))&&
    (complete||(!e.source_rescanned&&!e.native_requirement_accepted&&
        !e.all_criteria_independently_verified&&!e.dependencies_completed))&&
    e.previous_event_sha256.as_deref().is_none_or(valid_hash)
}
fn completion_init(conn:&Connection)->Result<(),String>{
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS ansf_project_task_completion_decisions(
            register_id TEXT NOT NULL,task_id TEXT NOT NULL,
            sequence INTEGER NOT NULL CHECK(sequence>0),
            event_sha256 TEXT NOT NULL,event_json BLOB NOT NULL,
            no_execution_authority INTEGER NOT NULL CHECK(no_execution_authority=1),
            PRIMARY KEY(register_id,task_id,sequence)
        );"
    ).map_err(|_|"CORE_C3_COMPLETION_SCHEMA_FAILED".into())
}
fn completion_rows(conn:&Connection,reg:&str,task:&str)
    ->Result<Vec<CompletionDecision>,String>{
    completion_init(conn)?;
    let mut stmt=conn.prepare(
        "SELECT sequence,event_sha256,event_json,no_execution_authority
           FROM ansf_project_task_completion_decisions
          WHERE register_id=?1 AND task_id=?2 ORDER BY sequence"
    ).map_err(|_|"CORE_C3_COMPLETION_HISTORY_QUERY_FAILED")?;
    let mut rows=stmt.query(params![reg,task])
        .map_err(|_|"CORE_C3_COMPLETION_HISTORY_OPEN_FAILED")?;
    let mut out=Vec::new();
    let mut prev=None;
    while let Some(row)=rows.next()
        .map_err(|_|"CORE_C3_COMPLETION_HISTORY_READ_FAILED")?{
        let seq:i64=row.get(0).map_err(|_|"CORE_C3_COMPLETION_SEQUENCE_FAILED")?;
        let hash:String=row.get(1).map_err(|_|"CORE_C3_COMPLETION_HASH_FAILED")?;
        let raw:Vec<u8>=row.get(2).map_err(|_|"CORE_C3_COMPLETION_EVENT_FAILED")?;
        let authority:i64=row.get(3).map_err(|_|"CORE_C3_COMPLETION_AUTHORITY_FAILED")?;
        if out.len()>=MAX_COMPLETION_EVENTS||seq!=out.len() as i64+1||
            raw.len()>4096||sha(&raw)!=hash||authority!=1{
            return Err("CORE_C3_COMPLETION_HISTORY_TAMPERED".into())
        }
        let record:CompletionDecision=serde_json::from_slice(&raw)
            .map_err(|_|"CORE_C3_COMPLETION_EVENT_INVALID")?;
        if !completion_decision_valid(&record)||
            record.register_id!=reg||record.task_id!=task||
            record.previous_event_sha256!=prev{
            return Err("CORE_C3_COMPLETION_CHAIN_BROKEN".into())
        }
        prev=Some(hash);
        out.push(record);
    }
    Ok(out)
}
fn completion_history(profile:&Path,reg:&str,task:&str)
    ->Result<Vec<CompletionDecisionView>,String>{
    let conn=project_register_desktop::library(profile)?;
    Ok(completion_rows(&conn,reg,task)?.into_iter().map(|e|CompletionDecisionView{
        event_id:e.event_id,decision:e.decision,task_version:e.task_version,
        operator_reason:e.operator_reason,
        source_rescanned_at_decision:e.source_rescanned,
        all_criteria_independently_verified_at_decision:
            e.all_criteria_independently_verified,
        task_completed_at_observation:e.task_completed_at_observation,
        current_source_reverified_in_this_view:false,
        execution_authorized:false,
    }).collect())
}
fn full_source_contract(criterion:&str)->bool{
    // A generic typed method, witness, model summary or freely worded
    // checklist cannot prove ALL parts of a task. An exact, single, native
    // predicate is the only machine-decided whole-task case in C3C v1.
    criterion==NATIVE_SOURCE_ONLY_CONTRACT
}
fn no_unresolved_text_blocker(blocked:&str)->bool{
    matches!(blocked.trim().to_ascii_lowercase().as_str(),
        "none"|"no blockers"|"n/a"|"not blocked"|"-")
}
fn dependency_completion_current(
    conn:&Connection,r:&register::TaskRegister,board_sha:&str,
    task:&register::TaskRevision,
)->Result<bool,String>{
    for dependency in &task.depends_on{
        if dependency==&task.task_id{return Ok(false)}
        let Some(dep)=register::latest(r,dependency)else{return Ok(false)};
        if !full_source_contract(&dep.acceptance_criteria)||
            dep.needs_human_reconciliation||dep.execution_authorized||
            !dep.independently_verified_requirement||
            dep.observed_status!=lu_core::project_status_classifier::Status::Accepted||
            !no_unresolved_text_blocker(&dep.blocked_by){
            return Ok(false)
        }
        let rows=completion_rows(conn,&r.register_id,dependency)?;
        let Some(last)=rows.last()else{return Ok(false)};
        if last.decision!="COMPLETED_AT_REVIEW"||
            !last.task_completed_at_observation||
            last.task_version!=dep.version||
            last.board_sha256!=board_sha||
            last.register_sha256!=r.latest_sha256||
            last.graph_sha256!=dep.graph_sha256||
            last.status_sha256!=dep.status_sha256||
            last.acceptance_criterion_sha256!=sha(dep.acceptance_criteria.as_bytes()){
            return Ok(false)
        }
    }
    Ok(true)
}
fn native_completion_decision_button(decision:&str)->&'static str{
    match decision{
        "COMPLETED_AT_REVIEW"=>"Mark whole task complete",
        "BLOCKED"=>"Record blocked task",
        "UNCERTAIN"=>"Record uncertain task",
        "REPLAN_REQUIRED"=>"Require reviewed task replan",
        _=>"Cancel unsupported task decision",
    }
}
fn completion_native_eligible(
    task:&register::TaskRevision,deps:bool
)->bool{
    full_source_contract(&task.acceptance_criteria)&&
    task.independently_verified_requirement&&
    task.observed_status==lu_core::project_status_classifier::Status::Accepted&&
    !task.needs_human_reconciliation&&!task.execution_authorized&&
    no_unresolved_text_blocker(&task.blocked_by)&&deps
}
fn completed_for_current_source(
    conn:&Connection,r:&register::TaskRegister,board_sha:&str,
    task:&register::TaskRevision,
)->Result<bool,String>{
    let original=completion_rows(conn,&r.register_id,&task.task_id)?;
    let Some(last)=original.last()else{return Ok(false)};
    if !completion_native_eligible(task,
        dependency_completion_current(conn,r,board_sha,task)?) {
        return Ok(false)
    }
    Ok(last.decision=="COMPLETED_AT_REVIEW"&&
        last.task_completed_at_observation&&
        last.all_criteria_independently_verified&&
        last.task_version==task.version&&last.board_sha256==board_sha&&
        last.register_sha256==r.latest_sha256&&
        last.graph_sha256==task.graph_sha256&&
        last.status_sha256==task.status_sha256&&
        last.acceptance_criterion_sha256==sha(task.acceptance_criteria.as_bytes()))
}
fn completion_append(
    profile:&Path,root:Option<&Path>,reg:&str,task:&str,
    expected_board:&str,expected_register:&str,version:u64,
    decision:&str,reason:&str,
)->Result<CompletionDecisionView,String>{
    let completed=decision=="COMPLETED_AT_REVIEW";
    if !matches!(decision,
        "COMPLETED_AT_REVIEW"|"BLOCKED"|"UNCERTAIN"|"REPLAN_REQUIRED")||
        reason.trim().len()<10||reason.len()>500||
        !valid_hash(expected_board)||!valid_hash(expected_register)||version==0{
        return Err("CORE_C3_COMPLETION_OPERATOR_DECISION_INVALID".into())
    }
    // The original OS-approved selection is rescanned AFTER native consent;
    // no stored witness, old scan or model output can replace this proof.
    let reopened=if completed{
        let selected=root.ok_or("CORE_C3_COMPLETION_NATIVE_SOURCE_REQUIRED")?;
        Some(project_task_agent_handoff::source_anchor(
            profile,selected,reg,task,expected_board,expected_register,version
        )?)
    }else{None};
    let mut conn=project_register_desktop::library(profile)?;
    let tx=conn.transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_|"CORE_C3_COMPLETION_LEDGER_BUSY")?;
    let (r,board,latest)=current_revision(&tx,reg,task)?;
    if board.sha256!=expected_board||r.latest_sha256!=expected_register||
        latest.version!=version||latest.execution_authorized||
        (completed&&(board.review_state!="human_approved"||
            board.source_register_sha256!=r.latest_sha256||
            latest.needs_human_reconciliation)){
        return Err("CORE_C3_COMPLETION_CURRENT_TASK_LEASE_DRIFTED".into())
    }
    let deps=if completed{
        dependency_completion_current(&tx,&r,&board.sha256,&latest)?
    }else{false};
    if completed{
        let original=reopened.as_ref()
            .ok_or("CORE_C3_COMPLETION_ORIGINAL_SCAN_MISSING")?;
        if !completion_native_eligible(original,deps)||
            original.graph_sha256!=latest.graph_sha256||
            original.status_sha256!=latest.status_sha256||
            original.acceptance_criteria!=latest.acceptance_criteria||
            original.source_requirement_id!=latest.source_requirement_id{
            return Err("CORE_C3_COMPLETION_ALL_CRITERIA_NOT_INDEPENDENTLY_VERIFIED".into())
        }
    }
    let earlier=completion_rows(&tx,reg,task)?;
    if earlier.len()>=MAX_COMPLETION_EVENTS{
        return Err("CORE_C3_COMPLETION_HISTORY_BOUNDED".into())
    }
    if completed&&completed_for_current_source(&tx,&r,&board.sha256,&latest)?{
        return Err("CORE_C3_COMPLETION_ALREADY_RECORDED".into())
    }
    let event=CompletionDecision{
        schema:COMPLETION_SCHEMA.into(),event_id:next_id("ev-"),
        register_id:reg.into(),task_id:task.into(),task_version:version,
        board_sha256:board.sha256,register_sha256:r.latest_sha256,
        graph_sha256:latest.graph_sha256,
        status_sha256:latest.status_sha256,
        acceptance_criterion_sha256:sha(latest.acceptance_criteria.as_bytes()),
        decision:decision.into(),operator_reason:reason.into(),
        previous_event_sha256:earlier.last().map(|e|
            sha(&serde_json::to_vec(e).expect("audited completion record"))),
        source_rescanned:completed,native_requirement_accepted:completed,
        all_criteria_independently_verified:completed,
        dependencies_completed:completed,human_authorized:true,
        task_completed_at_observation:completed,execution_authorized:false,
    };
    if !completion_decision_valid(&event){
        return Err("CORE_C3_COMPLETION_EVENT_FAILED_VALIDATION".into())
    }
    let bytes=serde_json::to_vec(&event)
        .map_err(|_|"CORE_C3_COMPLETION_EVENT_SERIALIZE_FAILED")?;
    if bytes.len()>4096{return Err("CORE_C3_COMPLETION_EVENT_TOO_LARGE".into())}
    let hash=sha(&bytes);
    tx.execute(
        "INSERT INTO ansf_project_task_completion_decisions(
            register_id,task_id,sequence,event_sha256,event_json,no_execution_authority
        ) VALUES(?1,?2,?3,?4,?5,1)",
        params![reg,task,earlier.len() as i64+1,hash,bytes]
    ).map_err(|_|"CORE_C3_COMPLETION_APPEND_REFUSED")?;
    tx.commit().map_err(|_|"CORE_C3_COMPLETION_COMMIT_UNCERTAIN")?;
    Ok(CompletionDecisionView{
        event_id:event.event_id,decision:event.decision,
        task_version:event.task_version,operator_reason:event.operator_reason,
        source_rescanned_at_decision:event.source_rescanned,
        all_criteria_independently_verified_at_decision:
            event.all_criteria_independently_verified,
        task_completed_at_observation:event.task_completed_at_observation,
        current_source_reverified_in_this_view:completed,
        execution_authorized:false,
    })
}
#[tauri::command]
pub(crate) async fn lu_project_task_completion_history(
    caller:tauri::WebviewWindow,state:State<'_,Arc<DesktopState>>,
    register_id:String,task_id:String,
)->Result<Vec<CompletionDecisionView>,String>{
    if caller.label()!="task-list"{
        return Err("CORE_C3_COMPLETION_REQUIRES_NATIVE_TASK_BOARD".into())
    }
    let profile=project_register_desktop::profile(state.inner())?;
    tauri::async_runtime::spawn_blocking(move||
        completion_history(&profile,&register_id,&task_id)
    ).await.map_err(|_|"CORE_C3_COMPLETION_HISTORY_INTERRUPTED")?
}
#[tauri::command]
pub(crate) async fn lu_project_task_decide_completion(
    app:AppHandle,caller:tauri::WebviewWindow,
    state:State<'_,Arc<DesktopState>>,
    register_id:String,task_id:String,
    expected_board_sha256:String,expected_register_sha256:String,
    expected_task_version:u64,decision:String,operator_reason:String,
)->Result<Option<CompletionDecisionView>,String>{
    if caller.label()!="task-list"{
        return Err("CORE_C3_COMPLETION_REQUIRES_NATIVE_TASK_BOARD".into())
    }
    if !matches!(decision.as_str(),
        "COMPLETED_AT_REVIEW"|"BLOCKED"|"UNCERTAIN"|"REPLAN_REQUIRED")||
        operator_reason.trim().len()<10||operator_reason.len()>500||
        !valid_hash(&expected_board_sha256)||
        !valid_hash(&expected_register_sha256)||expected_task_version==0{
        return Err("CORE_C3_COMPLETION_OPERATOR_INPUT_REFUSED".into())
    }
    let host=Arc::clone(state.inner());
    let profile=project_register_desktop::profile(&host)?;
    let original= {
        let conn=project_register_desktop::library(&profile)?;
        let (r,board,task)=current_revision(&conn,&register_id,&task_id)?;
        if board.sha256!=expected_board_sha256||
            r.latest_sha256!=expected_register_sha256||
            task.version!=expected_task_version||task.execution_authorized||
            (decision=="COMPLETED_AT_REVIEW"&&
                (board.review_state!="human_approved"||
                 board.source_register_sha256!=r.latest_sha256||
                 task.needs_human_reconciliation)){
            return Err("CORE_C3_COMPLETION_UNREVIEWED_OR_CHANGED_TASK".into())
        }
        (task,r,board)
    };
    let complete=decision=="COMPLETED_AT_REVIEW";
    let picked=if complete{
        let app=app.clone();
        let chosen=tauri::async_runtime::spawn_blocking(move||
            app.dialog().file().blocking_pick_folder()
        ).await.map_err(|_|"CORE_C3_COMPLETION_OS_PICKER_INTERRUPTED")?;
        let Some(chosen)=chosen else{return Ok(None)};
        let root=chosen.into_path()
            .map_err(|_|"CORE_C3_COMPLETION_INVALID_NATIVE_SOURCE")?;
        let original_task=project_task_agent_handoff::source_anchor(
            &profile,&root,&register_id,&task_id,
            &expected_board_sha256,&expected_register_sha256,expected_task_version
        )?;
        let conn=project_register_desktop::library(&profile)?;
        if !completion_native_eligible(
            &original_task,dependency_completion_current(
                &conn,&original.1,&expected_board_sha256,&original_task
            )?
        ){
            return Err("CORE_C3_COMPLETION_NOT_ALL_MACHINE_CHECKABLE_CRITERIA_VERIFIED".into())
        }
        Some(root)
    }else{None};
    // OS-native human decision, not WebView or model text. This is the only
    // ingress for positive completion and for explicit blocked/replan status.
    let message=format!(
        "Record an original WHOLE-TASK decision?\n\n\
        Task: {} (version {})\n\
        Requirement: {}\nExact criterion: {}\n\
        Blocker text: {}\nSource register: {}\nTask board: {}\n\n\
        Decision: {}\nHuman reason: {}\n\n\
        {}\n\n\
        This decision NEVER authorizes a future worker, code execution, \
        publishing, or model self-approval. Original source and dependency \
        evidence will be checked again before the SQLite commit.",
        original.0.summary,original.0.version,
        original.0.source_requirement_id,original.0.acceptance_criteria,
        original.0.blocked_by,expected_register_sha256,
        expected_board_sha256,decision,operator_reason,
        if complete{
            "Only the ENTIRE exact LU_COMPLETE_V1:CURRENT_NATIVE_REQUIREMENT \
             machine criterion can be completed in this implementation. \
             The OS-selected original project has been independently rescanned."
        }else{
            "This records BLOCKED, UNCERTAIN or REPLAN_REQUIRED. It does \
             not certify any completion and overrides the earlier status."
        }
    );
    // Do NOT reuse the review-only native dialog label to authorize
    // completion. A click on "Confirm review only" could not be treated as
    // a separate explicit whole-task completion decision. The OS-owned
    // button describes the exact status being committed.
    let button=native_completion_decision_button(&decision).to_string();
    let native=app.clone();
    let confirmed=tauri::async_runtime::spawn_blocking(move||
        native.dialog().message(message)
            .title("Lu — Explicit whole-task human decision")
            .buttons(tauri_plugin_dialog::MessageDialogButtons::OkCancelCustom(
                button,"Cancel".into()
            )).blocking_show()
    ).await.map_err(|_|"CORE_C3_COMPLETION_NATIVE_APPROVAL_INTERRUPTED")?;
    if !confirmed{return Ok(None)}
    tauri::async_runtime::spawn_blocking(move||
        completion_append(
            &profile,picked.as_deref(),&register_id,&task_id,
            &expected_board_sha256,&expected_register_sha256,
            expected_task_version,&decision,&operator_reason
        ).map(Some)
    ).await.map_err(|_|"CORE_C3_COMPLETION_DECISION_INTERRUPTED")?
}

/// C3 read-only completion reconciliation. The original PLAN-05 register
/// can certify a CURRENT native source requirement; an independently reviewed
/// Git/FFprobe outcome is supporting evidence, not a task-completion event.
#[derive(Clone,Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct TaskCompletionReconciliationView{
    schema:&'static str,
    register_id:String,
    task_id:String,
    task_version:u64,
    source_rescanned_this_review:bool,
    native_requirement_accepted:bool,
    reopened_human_criterion_witnesses:usize,
    original_human_review_count:usize,
    history_fully_inspected:bool,
    status:&'static str,
    completion_decision_count:usize,
    task_completed:bool,
    execution_authorized:bool,
}
fn completion_status(native:bool,reviewed:bool,full:bool)->&'static str{
    if !full{return "STOP_REVIEW_HISTORY_RECONCILIATION_REQUIRED"}
    match (native,reviewed){
        (true,true)=>"CURRENT_NATIVE_REQUIREMENT_AND_HISTORICAL_WITNESS_SUPPORTED",
        (true,false)=>"CURRENT_NATIVE_REQUIREMENT_ACCEPTED_NO_MATCHING_WITNESS",
        (false,true)=>"HISTORICAL_WITNESS_ONLY_NATIVE_REQUIREMENT_UNACCEPTED",
        (false,false)=>"REQUIRES_NATIVE_REQUIREMENT_AND_CRITERION_REVIEW",
    }
}
fn reconcile_completion(
    profile:&Path,selected:&Path,register_id:&str,task_id:&str,
    board_sha:&str,register_sha:&str,version:u64,
    chats:&chat_store::ChatStore,
)->Result<TaskCompletionReconciliationView,String>{
    // This re-enters the existing PLAN-07 ORIGINAL native-source scanner
    // after an OS-picked folder, not a WebView-supplied path or model claim.
    let current=project_task_agent_handoff::source_anchor(
        profile,selected,register_id,task_id,board_sha,register_sha,version
    )?;
    let native=current.independently_verified_requirement
        &&current.observed_status==lu_core::project_status_classifier::Status::Accepted
        &&!current.needs_human_reconciliation;
    let original=history(profile,register_id,task_id)?;
    // An unexpectedly long history cannot be silently truncated to claim
    // absence or presence of a sufficiently reviewed original witness.
    const MAX_REOPENED:usize=16;
    let full=original.len()<=MAX_REOPENED;
    let mut independently_reopened=0;
    if full{
        for decision in &original{
            if decision.decision!="ACCEPTED_AS_SUPPORTING_EVIDENCE"
                ||!decision.source_lease_current
                ||decision.task_version!=version
                ||decision.acceptance_criterion!=current.acceptance_criteria
                ||!project_task_agent_handoff::criterion_supports_method(
                    &current.acceptance_criteria,
                    &decision.tool_id,&decision.independent_method
                ){continue}
            // Reopen the ORIGINAL saved human turn, immutable plan, effect
            // journal and independent oracle. A serialized review event is
            // insufficient, and no historical grant may be reused.
            let worker=project_task_agent_handoff::independently_verified_task_witness(
                profile,register_id,task_id,&decision.handoff_id,
                &decision.receipt_sha256,chats
            )?;
            if worker.tool_id!=decision.tool_id
                ||worker.independent_method!=decision.independent_method
                ||worker.observed_sha256!=decision.observed_sha256
                ||worker.receipt_sha256!=decision.receipt_sha256{
                return Err("CORE_C3_ORIGINAL_WORKER_REVIEW_CONFLICT".into())
            }
            independently_reopened+=1;
        }
    }
    // Only the latest separately human-authorized, hash-chain audited
    // completion event can promote this result. Recheck the *same current*
    // native source lease and all declared task dependencies. No old worker
    // receipt or user-visible model summary is a completion event.
    let conn=project_register_desktop::library(profile)?;
    let (r,board,latest)=current_revision(&conn,register_id,task_id)?;
    let decisions=completion_rows(&conn,register_id,task_id)?;
    let complete=native&&
        completed_for_current_source(&conn,&r,&board.sha256,&latest)?;
    Ok(TaskCompletionReconciliationView{
        schema:"lu-core-c3-task-completion-reconciliation-v1",
        register_id:register_id.into(),task_id:task_id.into(),
        task_version:version,source_rescanned_this_review:true,
        native_requirement_accepted:native,
        reopened_human_criterion_witnesses:independently_reopened,
        original_human_review_count:original.len(),
        history_fully_inspected:full,
        status:if complete{
            "WHOLE_TASK_HUMAN_DECISION_CURRENT_SOURCE_REVERIFIED"
        }else{completion_status(native,independently_reopened>0,full)},
        completion_decision_count:decisions.len(),
        task_completed:complete,execution_authorized:false,
    })
}
#[tauri::command]
pub(crate) async fn lu_project_task_reconcile_completion(
    app:AppHandle,caller:tauri::WebviewWindow,
    state:State<'_,Arc<DesktopState>>,
    register_id:String,task_id:String,
    expected_board_sha256:String,expected_register_sha256:String,
    expected_task_version:u64,
)->Result<Option<TaskCompletionReconciliationView>,String>{
    if caller.label()!="task-list"{
        return Err("CORE_C3_REQUIRES_NATIVE_TASK_BOARD".into())
    }
    if expected_task_version==0
        ||!valid_hash(&expected_board_sha256)
        ||!valid_hash(&expected_register_sha256){
        return Err("CORE_C3_INVALID_EXPECTED_SOURCE_LEASE".into())
    }
    let host=Arc::clone(state.inner());
    let profile=project_register_desktop::profile(&host)?;
    let chooser=app.clone();
    let picked=tauri::async_runtime::spawn_blocking(move||
        chooser.dialog().file().blocking_pick_folder()
    ).await.map_err(|_|"CORE_C3_NATIVE_FOLDER_PICKER_INTERRUPTED")?;
    let Some(picked)=picked else{return Ok(None)};
    let selected=picked.into_path()
        .map_err(|_|"CORE_C3_NATIVE_PROJECT_NOT_LOCAL")?;
    tauri::async_runtime::spawn_blocking(move||reconcile_completion(
        &profile,&selected,&register_id,&task_id,
        &expected_board_sha256,&expected_register_sha256,
        expected_task_version,&host.chats
    ).map(Some)).await.map_err(|_|"CORE_C3_RECONCILIATION_INTERRUPTED")?
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
    fn whole_task_completion_contract_is_explicit_and_not_model_self_approval(){
        assert!(full_source_contract("LU_COMPLETE_V1:CURRENT_NATIVE_REQUIREMENT"));
        for bad in [
            "All tasks complete","LU_CRITERION_METHOD_V1:lu.reviewed.git.head|GIT_HEAD_DIRECT_V1\nRead HEAD",
            "LU_COMPLETE_V1:CURRENT_NATIVE_REQUIREMENT\nAlso deploy",
            "LU_COMPLETE_V1:CURRENT_NATIVE_REQUIREMENT;PUBLISH",
            "LU_COMPLETE_V1:CURRENT_NATIVE_REQUIREMENT ",
            "LU_COMPLETE_V1:CURRENT_NATIVE_REQUIREMENT_AND_WORKER",
        ]{
            assert!(!full_source_contract(bad),"unverified clause: {bad}");
        }
        assert!(no_unresolved_text_blocker("None"));
        assert!(!no_unresolved_text_blocker("Awaiting approval"));
        assert!(!no_unresolved_text_blocker("P0 Git worker verification"));
    }
    fn completion_fixture(decision:&str)->CompletionDecision{
        let yes=decision=="COMPLETED_AT_REVIEW";
        CompletionDecision{
            schema:COMPLETION_SCHEMA.into(),event_id:"ev-fixture".into(),
            register_id:"lu-register-fixture".into(),
            task_id:"lu-task-fixture".into(),task_version:1,
            board_sha256:"a".repeat(64),register_sha256:"b".repeat(64),
            graph_sha256:"c".repeat(64),status_sha256:"d".repeat(64),
            acceptance_criterion_sha256:sha(
                NATIVE_SOURCE_ONLY_CONTRACT.as_bytes()),
            decision:decision.into(),operator_reason:"Native owner reviewed this original source".into(),
            previous_event_sha256:None,
            source_rescanned:yes,native_requirement_accepted:yes,
            all_criteria_independently_verified:yes,
            dependencies_completed:yes,human_authorized:true,
            task_completed_at_observation:yes,execution_authorized:false,
        }
    }
    #[test]
    fn complete_and_noncomplete_decisions_have_strict_separate_authority(){
        let accepted=completion_fixture("COMPLETED_AT_REVIEW");
        assert!(completion_decision_valid(&accepted));
        for corrupt in [
            {let mut x=accepted.clone();x.source_rescanned=false;x},
            {let mut x=accepted.clone();x.dependencies_completed=false;x},
            {let mut x=accepted.clone();x.human_authorized=false;x},
            {let mut x=accepted.clone();x.execution_authorized=true;x},
            {let mut x=accepted.clone();x.all_criteria_independently_verified=false;x},
            {let mut x=accepted.clone();x.native_requirement_accepted=false;x}
        ]{assert!(!completion_decision_valid(&corrupt))}
        for state in ["BLOCKED","UNCERTAIN","REPLAN_REQUIRED"]{
            let event=completion_fixture(state);
            assert!(completion_decision_valid(&event));
            let mut forged=event.clone();forged.task_completed_at_observation=true;
            assert!(!completion_decision_valid(&forged));
        }
    }

    #[test]
    fn distinct_native_confirmation_makes_completion_and_safe_replan_explicit(){
        assert_eq!(native_completion_decision_button("COMPLETED_AT_REVIEW"),
            "Mark whole task complete");
        assert_eq!(native_completion_decision_button("REPLAN_REQUIRED"),
            "Require reviewed task replan");
        for state in ["BLOCKED","UNCERTAIN","REPLAN_REQUIRED"]{
            assert!(!native_completion_decision_button(state)
                .to_ascii_lowercase().contains("complete"),
                "a conservative state cannot look like task completion");
        }
        assert_eq!(native_completion_decision_button("UNTRUSTED"),
            "Cancel unsupported task decision");
    }
    #[test]
    fn source_only_completion_policy_rejects_unchecked_dependencies_and_blockers(){
        let valid=register::TaskRevision{
            task_id:"lu-task-test".into(),version:1,
            source_requirement_id:"R-1".into(),summary:"Native original source".into(),
            description:"Recheck exactly one current native source fact".into(),
            group:"Source proofs".into(),priority:"P1".into(),
            blocked_by:"none".into(),
            acceptance_criteria:NATIVE_SOURCE_ONLY_CONTRACT.into(),
            depends_on:vec![],evidence_ids:vec!["R-1".into()],
            original_model_proposal_sha256:"a".repeat(64),
            graph_sha256:"b".repeat(64),status_sha256:"c".repeat(64),
            selected_head:None,
            observed_status:lu_core::project_status_classifier::Status::Accepted,
            observed_at_unix:0,needs_human_reconciliation:false,
            independently_verified_requirement:true,execution_authorized:false
        };
        assert!(completion_native_eligible(&valid,true));
        assert!(!completion_native_eligible(&valid,false));
        let mut v=valid.clone();v.blocked_by="Awaiting code changes".into();
        assert!(!completion_native_eligible(&v,true));
        let mut v=valid.clone();v.acceptance_criteria=
            "Implement feature, test on Windows and deploy".into();
        assert!(!completion_native_eligible(&v,true));
        let mut v=valid.clone();v.needs_human_reconciliation=true;
        assert!(!completion_native_eligible(&v,true));
        let mut v=valid.clone();v.independently_verified_requirement=false;
        assert!(!completion_native_eligible(&v,true));
        let mut v=valid;v.execution_authorized=true;
        assert!(!completion_native_eligible(&v,true));
    }
    #[test]
    fn original_completion_decision_hash_chain_rejects_forged_completion_after_restart(){
        let folder=std::env::temp_dir().join(format!(
            "lu-c3-completion-{}-{}",
            std::process::id(),std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(&folder).unwrap();
        let file=folder.join("journal.sqlite");
        {
            let conn=Connection::open(&file).unwrap();
            completion_init(&conn).unwrap();
            let a=completion_fixture("BLOCKED");
            let raw=serde_json::to_vec(&a).unwrap();
            let digest=sha(&raw);
            conn.execute(
                "INSERT INTO ansf_project_task_completion_decisions
                (register_id,task_id,sequence,event_sha256,event_json,no_execution_authority)
                VALUES(?1,?2,1,?3,?4,1)",
                params![&a.register_id,&a.task_id,&digest,&raw]
            ).unwrap();
            assert_eq!(completion_rows(&conn,&a.register_id,&a.task_id).unwrap().len(),1);
        }
        {
            let conn=Connection::open(&file).unwrap();
            let rows=completion_rows(&conn,"lu-register-fixture","lu-task-fixture").unwrap();
            assert_eq!(rows[0].decision,"BLOCKED");
            let event=completion_fixture("COMPLETED_AT_REVIEW");
            let mut second=event.clone();
            second.previous_event_sha256=Some(sha(&serde_json::to_vec(&rows[0]).unwrap()));
            let raw=serde_json::to_vec(&second).unwrap();
            conn.execute(
                "INSERT INTO ansf_project_task_completion_decisions
                (register_id,task_id,sequence,event_sha256,event_json,no_execution_authority)
                VALUES(?1,?2,2,?3,?4,1)",
                params![&second.register_id,&second.task_id,sha(&raw),raw]
            ).unwrap();
            assert_eq!(completion_rows(&conn,"lu-register-fixture","lu-task-fixture")
                .unwrap().len(),2);
            conn.execute("UPDATE ansf_project_task_completion_decisions
                SET event_json=?1 WHERE sequence=2",[b"{}".as_slice()]).unwrap();
            assert!(completion_rows(&conn,"lu-register-fixture","lu-task-fixture").is_err());
        }
        std::fs::remove_dir_all(folder).unwrap();
    }
    #[test]
    fn completion_reconciliation_cannot_turn_prior_receipts_into_task_authority(){
        assert_eq!(completion_status(true,true,true),
            "CURRENT_NATIVE_REQUIREMENT_AND_HISTORICAL_WITNESS_SUPPORTED");
        assert_eq!(completion_status(false,true,true),
            "HISTORICAL_WITNESS_ONLY_NATIVE_REQUIREMENT_UNACCEPTED");
        assert_eq!(completion_status(true,false,true),
            "CURRENT_NATIVE_REQUIREMENT_ACCEPTED_NO_MATCHING_WITNESS");
        assert_eq!(completion_status(true,true,false),
            "STOP_REVIEW_HISTORY_RECONCILIATION_REQUIRED");
    }
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
