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
// Fail closed if the ORIGINAL immutable human criterion history exceeds the
// bounded number we can reopen during whole-task completion reconciliation.
const MAX_REOPENED_CRITERION_REVIEWS:usize=16;

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
const GIT_HEAD_EQUALS_PREFIX:&str="LU_COMPLETE_V1:GIT_HEAD_EQUALS:";
const ALL_OF_PREFIX:&str="LU_COMPLETE_V2:ALL_OF\n";
const MAX_COMPLETION_EVENTS:usize=128;
/// An exact, bounded declaration of ALL required predicates. No free-text
/// criterion, arbitrary process output or model-selected tool is a predicate.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
enum WholeCriterion<'a>{
    Native,
    GitHead(&'a str),
    GitStatusReviewed,
    WavPcmReviewed,
    Mp4BoxesReviewed,
}
impl WholeCriterion<'_>{
    fn method(self)->Option<(&'static str,&'static str)>{
        match self{
            Self::Native=>None,
            Self::GitHead(_)=>Some(("lu.reviewed.git.head","GIT_HEAD_DIRECT_V1")),
            Self::GitStatusReviewed=>Some((
                "lu.reviewed.git.status","GIT_STATUS_INDEX_TREE_V1")),
            Self::WavPcmReviewed=>Some((
                "lu.reviewed.media.facts","WAV_PCM_DIRECT_V1")),
            Self::Mp4BoxesReviewed=>Some((
                "lu.reviewed.media.facts","MP4_BOXES_DIRECT_V1"))
        }
    }
    fn order(self)->u8{
        match self{
            Self::Native=>0,Self::GitHead(_)=>1,
            Self::GitStatusReviewed=>2,Self::WavPcmReviewed=>3,
            Self::Mp4BoxesReviewed=>4,
        }
    }
}
fn exact_git_sha(head:&str)->bool{
    head.len()==40&&head.bytes().all(|b|
        b.is_ascii_digit()||matches!(b,b'a'..=b'f'))
}
fn whole_criteria(criterion:&str)->Option<Vec<WholeCriterion<'_>>>{
    if criterion.len()>300{return None}
    if criterion==NATIVE_SOURCE_ONLY_CONTRACT{
        return Some(vec![WholeCriterion::Native])
    }
    if let Some(head)=criterion.strip_prefix(GIT_HEAD_EQUALS_PREFIX){
        return exact_git_sha(head).then_some(vec![WholeCriterion::GitHead(head)])
    }
    let body=criterion.strip_prefix(ALL_OF_PREFIX)?;
    let lines:Vec<_>=body.split('\n').collect();
    if lines.len()<2||lines.len()>5||lines.first()!=Some(&"CURRENT_NATIVE_REQUIREMENT"){
        return None
    }
    let mut result=Vec::with_capacity(lines.len());
    let mut last=None;
    for line in lines{
        let clause=if line=="CURRENT_NATIVE_REQUIREMENT"{
            WholeCriterion::Native
        }else if let Some(head)=line.strip_prefix("GIT_HEAD_EQUALS:"){
            if !exact_git_sha(head){return None}
            WholeCriterion::GitHead(head)
        }else{
            match line{
                "GIT_STATUS_REVIEWED"=>WholeCriterion::GitStatusReviewed,
                "WAV_PCM_REVIEWED"=>WholeCriterion::WavPcmReviewed,
                "MP4_BOXES_REVIEWED"=>WholeCriterion::Mp4BoxesReviewed,
                _=>return None
            }
        };
        // Strict canonical ordering, no duplicates, hidden extra conditions,
        // CRLF ambiguity, multi-HEAD aliasing or empty lines.
        if last.is_some_and(|p|p>=clause.order()){return None}
        last=Some(clause.order());
        result.push(clause);
    }
    Some(result)
}
/// Only the methods explicitly declared by an exact, entire multi-criterion
/// manifest can be reviewed for eventual whole-task acceptance.
pub(super) fn whole_criterion_supports_method(
    criterion:&str,tool:&str,method:&str
)->Option<bool>{
    if !criterion.starts_with("LU_COMPLETE_V2:"){return None}
    let clauses=whole_criteria(criterion);
    Some(clauses.is_some_and(|items|items.into_iter().any(|item|
        item.method()==Some((tool,method))
    )))
}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct CompletionDecision{
    schema:String,event_id:String,register_id:String,task_id:String,
    task_version:u64,board_sha256:String,register_sha256:String,
    graph_sha256:String,status_sha256:String,
    acceptance_criterion_sha256:String,decision:String,operator_reason:String,
    previous_event_sha256:Option<String>,
    // Older native-only events predate this optional receipt binding. Omitting
    // None retains their exact serialized bytes and immutable hash chain.
    #[serde(default,skip_serializing_if="Option::is_none")]
    task_specific_witness_sha256:Option<String>,
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
    e.task_specific_witness_sha256.as_deref().is_none_or(valid_hash)&&
    (complete||e.task_specific_witness_sha256.is_none())&&
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
fn git_head_expected(criterion:&str)->Option<&str>{
    whole_criteria(criterion)?.into_iter().find_map(|item|
        if let WholeCriterion::GitHead(head)=item{Some(head)}else{None}
    )
}
fn full_source_contract(criterion:&str)->bool{
    whole_criteria(criterion).is_some()
}
fn current_whole_task_criterion(task:&register::TaskRevision)->bool{
    let Some(clauses)=whole_criteria(&task.acceptance_criteria)else{
        return false
    };
    clauses.into_iter().all(|clause|match clause{
        WholeCriterion::GitHead(expected)=>
            // PLAN-03 independently rescans live OS-selected Git HEAD.
            task.selected_head.as_deref()==Some(expected),
        // These clauses must ALSO obtain separate current independent
        // audited, human-reviewed worker witnesses before completion.
        _=>true
    })
}
fn no_unresolved_text_blocker(blocked:&str)->bool{
    matches!(blocked.trim().to_ascii_lowercase().as_str(),
        "none"|"no blockers"|"n/a"|"not blocked"|"-")
}
fn dependency_completion_current(
    conn:&Connection,r:&register::TaskRegister,board_sha:&str,
    task:&register::TaskRevision,
)->Result<bool,String>{
    // A direct parent with an old "COMPLETED_AT_REVIEW" record is NOT
    // sufficient if ITS own prerequisites were later rescinded. Verify the
    // transitive DAG against the exact current register/board source lease.
    // Bound by the original PLAN05 16-task source register, reject cycles,
    // absent IDs, stale decisions, and any conservative override.
    fn verify(
        conn:&Connection,r:&register::TaskRegister,board_sha:&str,
        id:&str,visiting:&mut std::collections::BTreeSet<String>,
        done:&mut std::collections::BTreeSet<String>,
    )->Result<bool,String>{
        if done.contains(id){return Ok(true)}
        if visiting.len()>=16||!visiting.insert(id.to_owned()){
            return Ok(false)
        }
        let Some(dep)=register::latest(r,id)else{return Ok(false)};
        // Dependency histories do not OS-reselect and re-open each ancestor's
        // reviewed Git worker. Until then, only native-source parents qualify.
        if dep.acceptance_criteria!=NATIVE_SOURCE_ONLY_CONTRACT||
            dep.needs_human_reconciliation||dep.execution_authorized||
            !dep.independently_verified_requirement||
            dep.observed_status!=lu_core::project_status_classifier::Status::Accepted||
            !no_unresolved_text_blocker(&dep.blocked_by){
            return Ok(false)
        }
        let rows=completion_rows(conn,&r.register_id,id)?;
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
        for prerequisite in &dep.depends_on{
            if !verify(conn,r,board_sha,prerequisite,visiting,done)?{
                return Ok(false)
            }
        }
        visiting.remove(id);
        done.insert(id.to_owned());
        Ok(true)
    }
    let mut visiting=std::collections::BTreeSet::from([task.task_id.clone()]);
    let mut done=std::collections::BTreeSet::new();
    for prerequisite in &task.depends_on{
        if !verify(conn,r,board_sha,prerequisite,&mut visiting,&mut done)?{
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
    current_whole_task_criterion(task)&&
    task.independently_verified_requirement&&
    task.observed_status==lu_core::project_status_classifier::Status::Accepted&&
    !task.needs_human_reconciliation&&!task.execution_authorized&&
    no_unresolved_text_blocker(&task.blocked_by)&&deps
}
// A tool method alone is NOT a complete criterion. For a narrow exact-Git
// HEAD task, a human-reviewed independent original worker witness must match
// the same immutable source/task lease, then be reopened from original logs.
fn reviewed_git_head_event_matches(
    e:&CriterionDecision,r:&register::TaskRegister,board_sha:&str,
    task:&register::TaskRevision,
)->bool{
    let Some(expected)=git_head_expected(&task.acceptance_criteria)else{
        return false
    };
    task.selected_head.as_deref()==Some(expected)&&
    e.decision=="ACCEPTED_AS_SUPPORTING_EVIDENCE"&&
    e.verdict_at_review=="VERIFIED_AT_OBSERVATION"&&
    e.tool_id=="lu.reviewed.git.head"&&
    e.independent_method=="GIT_HEAD_DIRECT_V1"&&
    e.register_id==r.register_id&&e.task_id==task.task_id&&
    e.task_version==task.version&&e.board_sha256==board_sha&&
    e.register_sha256==r.latest_sha256&&
    e.source_scope_sha256==r.scope_sha256&&
    e.graph_sha256==task.graph_sha256&&
    e.status_sha256==task.status_sha256&&
    e.selected_head.as_deref()==Some(expected)&&
    e.source_requirement_id==task.source_requirement_id&&
    e.acceptance_criterion==task.acceptance_criteria&&
    e.acceptance_criterion_sha256==sha(task.acceptance_criteria.as_bytes())
}
// A generic method name or model-generated summary is never an outcome.
// Each declaration requires the exact matching original human-reviewed,
// independently audited first-party worker, its current source lease and
// task version, and the original OS-reopened receipt.
fn reviewed_whole_method_event_matches(
    e:&CriterionDecision,r:&register::TaskRegister,board_sha:&str,
    task:&register::TaskRevision,tool:&str,method:&str,
)->bool{
    if tool=="lu.reviewed.git.head"&&method=="GIT_HEAD_DIRECT_V1"{
        return reviewed_git_head_event_matches(e,r,board_sha,task)
    }
    whole_criterion_supports_method(&task.acceptance_criteria,tool,method)
        ==Some(true)&&
    e.decision=="ACCEPTED_AS_SUPPORTING_EVIDENCE"&&
    e.verdict_at_review=="VERIFIED_AT_OBSERVATION"&&
    e.tool_id==tool&&e.independent_method==method&&
    e.register_id==r.register_id&&e.task_id==task.task_id&&
    e.task_version==task.version&&e.board_sha256==board_sha&&
    e.register_sha256==r.latest_sha256&&
    e.source_scope_sha256==r.scope_sha256&&
    e.graph_sha256==task.graph_sha256&&
    e.status_sha256==task.status_sha256&&
    e.selected_head==task.selected_head&&
    e.source_requirement_id==task.source_requirement_id&&
    e.acceptance_criterion==task.acceptance_criteria&&
    e.acceptance_criterion_sha256==sha(task.acceptance_criteria.as_bytes())
}
/// Pure, fail-closed selection, suitable for hostile negative/restart tests.
/// One separate last review for EACH method is required. Later rejected or
/// uncertain evidence for a method rescinds its older favorable review.
fn select_all_required_reviews<'a>(
    rows:&'a[CriterionDecision],r:&register::TaskRegister,board_sha:&str,
    task:&register::TaskRevision,
)->Option<Vec<&'a CriterionDecision>>{
    let clauses=whole_criteria(&task.acceptance_criteria)?;
    let mut reviews=Vec::new();
    for clause in clauses{
        let Some((tool,method))=clause.method()else{continue};
        let selected=rows.iter().rev().find(|event|
            event.tool_id==tool&&event.independent_method==method)?;
        // Preserve V1's stronger global-last-event revocation rule.
        if task.acceptance_criteria.starts_with(GIT_HEAD_EQUALS_PREFIX)&&
            rows.last().map(|e|e.event_id.as_str())!=Some(selected.event_id.as_str()){
            return None
        }
        if !reviewed_whole_method_event_matches(
            selected,r,board_sha,task,tool,method
        ){return None}
        reviews.push(selected);
    }
    Some(reviews)
}
#[derive(Clone,Debug)]
struct WholeTaskWitnessSet{
    // Original immutable criterion decisions, not model claims.
    reviewed:Vec<CriterionDecision>,
    // V1 single original Git witness retains its existing receipt hash. V2
    // binds ALL ordered method/receipt/review identities to one digest.
    receipt_binding:Option<String>,
}
fn selected_witness_binding(
    criterion:&str,reviews:&[&CriterionDecision]
)->Option<String>{
    if criterion==NATIVE_SOURCE_ONLY_CONTRACT{return None}
    if criterion.starts_with(GIT_HEAD_EQUALS_PREFIX){
        return reviews.first().map(|e|e.receipt_sha256.clone())
    }
    if !criterion.starts_with(ALL_OF_PREFIX)||reviews.is_empty(){
        return None
    }
    let bound:Vec<_>=reviews.iter().map(|e|(
        e.tool_id.as_str(),e.independent_method.as_str(),
        e.event_id.as_str(),e.receipt_sha256.as_str(),
        e.observed_sha256.as_str(),
    )).collect();
    Some(sha(&serde_json::to_vec(&bound)
        .expect("bounded original reviewed witness tuple")))
}
fn independently_reopened_whole_task_witnesses(
    profile:&Path,conn:&Connection,r:&register::TaskRegister,
    board_sha:&str,task:&register::TaskRevision,
    chats:&chat_store::ChatStore,
)->Result<Option<WholeTaskWitnessSet>,String>{
    if !current_whole_task_criterion(task){return Ok(None)}
    let rows=history_rows(conn,&r.register_id,&task.task_id)?;
    if !all_human_criterion_history_inspected(rows.len()){
        return Err("CORE_C3_COMPLETION_REVIEW_HISTORY_NOT_FULLY_INSPECTABLE".into())
    }
    let Some(reviews)=select_all_required_reviews(&rows,r,board_sha,task)
        else{return Ok(None)};
    for item in &reviews{
        let actual=project_task_agent_handoff::independently_verified_task_witness(
            profile,&r.register_id,&task.task_id,
            &item.handoff_id,&item.receipt_sha256,chats
        )?;
        if actual.handoff_id!=item.handoff_id||
            actual.tool_id!=item.tool_id||
            actual.independent_method!=item.independent_method||
            actual.receipt_sha256!=item.receipt_sha256||
            actual.observed_sha256!=item.observed_sha256||
            actual.step_index!=item.step_index{
            return Err("CORE_C3_ORIGINAL_CRITERION_WORKER_CONFLICT".into())
        }
    }
    Ok(Some(WholeTaskWitnessSet{
        receipt_binding:selected_witness_binding(
            &task.acceptance_criteria,&reviews),
        reviewed:reviews.into_iter().cloned().collect()
    }))
}
fn completion_receipt_matches(
    task:&register::TaskRevision,last:&CompletionDecision,
    reviewed_witness_binding:Option<&str>,
)->bool{
    if !full_source_contract(&task.acceptance_criteria){return false}
    if task.acceptance_criteria==NATIVE_SOURCE_ONLY_CONTRACT{
        return last.task_specific_witness_sha256.is_none()&&
            reviewed_witness_binding.is_none()
    }
    // V1 Git binds its original independently reopened receipt. V2 binds
    // EVERY method/review/oracle identity in a canonical witness-set digest.
    reviewed_witness_binding.is_some()&&
        last.task_specific_witness_sha256.as_deref()==reviewed_witness_binding
}
fn completed_for_current_source(
    conn:&Connection,r:&register::TaskRegister,board_sha:&str,
    task:&register::TaskRevision,reviewed_head_receipt:Option<&str>,
)->Result<bool,String>{
    let original=completion_rows(conn,&r.register_id,&task.task_id)?;
    let Some(last)=original.last()else{return Ok(false)};
    if !completion_native_eligible(task,
        dependency_completion_current(conn,r,board_sha,task)?) {
        return Ok(false)
    }
    Ok(completion_receipt_matches(task,last,reviewed_head_receipt)&&
        last.decision=="COMPLETED_AT_REVIEW"&&
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
    decision:&str,reason:&str,chats:&chat_store::ChatStore,
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
    // Reopen EACH original method/receipt declared in the ENTIRE current
    // criterion set, after source rescan and independently of model text.
    // The later OS-native completion dialog is a distinct authorization.
    let reviewed_set=if completed{
        let db=project_register_desktop::library(profile)?;
        let (r,b,current)=current_revision(&db,reg,task)?;
        let original=reopened.as_ref().ok_or("CORE_C3_MISSING_RESCANNED_SOURCE")?;
        if b.sha256!=expected_board||r.latest_sha256!=expected_register||
            current.graph_sha256!=original.graph_sha256||
            current.status_sha256!=original.status_sha256||
            current.acceptance_criteria!=original.acceptance_criteria||
            current.selected_head!=original.selected_head{
            return Err("CORE_C3_ALL_CRITERIA_SOURCE_DRIFT".into())
        }
        Some(independently_reopened_whole_task_witnesses(
            profile,&db,&r,&b.sha256,&current,chats
        )?.ok_or("CORE_C3_EVERY_REQUIRED_CRITERION_NEEDS_ORIGINAL_WITNESS")?)
    }else{None};
    let mut conn=project_register_desktop::library(profile)?;
    let tx=conn.transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_|"CORE_C3_COMPLETION_LEDGER_BUSY")?;
    let (r,board,latest)=current_revision(&tx,reg,task)?;
    // A positive decision cannot be saved if subsequent reconciliation
    // would omit any original human criterion review from its audit.
    // Recheck under the same write transaction: concurrent review additions
    // must never turn a stale positive into a purported complete task.
    if completed&&!all_human_criterion_history_inspected(
        history_rows(&tx,reg,task)?.len()
    ){
        return Err("CORE_C3_COMPLETION_REVIEW_HISTORY_NOT_FULLY_INSPECTABLE".into())
    }
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
    if completed{
        let approved=reviewed_set.as_ref()
            .ok_or("CORE_C3_REQUIRED_CRITERION_WITNESSES_MISSING")?;
        let rows=history_rows(&tx,reg,task)?;
        let selected=select_all_required_reviews(
            &rows,&r,&board.sha256,&latest
        ).ok_or("CORE_C3_REQUIRED_CRITERION_REVIEW_RESCINDED")?;
        if selected.len()!=approved.reviewed.len()||
            selected.iter().zip(&approved.reviewed).any(|(a,b)|
                a.event_id!=b.event_id||
                a.receipt_sha256!=b.receipt_sha256||
                a.observed_sha256!=b.observed_sha256||
                a.tool_id!=b.tool_id||
                a.independent_method!=b.independent_method){
            return Err("CORE_C3_REQUIRED_CRITERION_WITNESS_DRIFTED".into())
        }
    }
    let earlier=completion_rows(&tx,reg,task)?;
    if earlier.len()>=MAX_COMPLETION_EVENTS{
        return Err("CORE_C3_COMPLETION_HISTORY_BOUNDED".into())
    }
    if completed&&completed_for_current_source(
        &tx,&r,&board.sha256,&latest,
        reviewed_set.as_ref().and_then(|s|s.receipt_binding.as_deref())
    )?{
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
        task_specific_witness_sha256:reviewed_set.as_ref()
            .and_then(|s|s.receipt_binding.clone()),
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
        if independently_reopened_whole_task_witnesses(
            &profile,&conn,&original.1,&expected_board_sha256,
            &original_task,&host.chats
        )?.is_none(){
            return Err("CORE_C3_EVERY_REQUIRED_CRITERION_NEEDS_ORIGINAL_WITNESS".into())
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
            "Only a complete typed LU_COMPLETE_V1 single criterion or \
             canonical LU_COMPLETE_V2:ALL_OF source+independent-method set \
             can finish. Every declared method has a distinct CURRENT \
             human-reviewed native worker/OS-oracle receipt; all are \
             independently reopened after consent. Unchecked/free-text \
             requirements remain BLOCKED. The OS-selected original project \
             and transitive task dependencies are independently rescanned."
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
            expected_task_version,&decision,&operator_reason,&host.chats
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
    required_criterion_count:usize,
    all_declared_criteria_independently_verified:bool,
    task_completed:bool,
    execution_authorized:bool,
}
fn all_human_criterion_history_inspected(count:usize)->bool{
    count<=MAX_REOPENED_CRITERION_REVIEWS
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
    let full=all_human_criterion_history_inspected(original.len());
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
    // Recheck the source and exact current task lease as well as EVERY
    // distinct original worker/method. A partial multi-criterion set
    // can never inherit an earlier completion event.
    let source_current=r.latest_sha256==register_sha&&
        board.sha256==board_sha&&
        board.review_state=="human_approved"&&
        board.source_register_sha256==r.latest_sha256&&
        latest.version==version&&!latest.execution_authorized&&
        latest.source_requirement_id==current.source_requirement_id&&
        latest.graph_sha256==current.graph_sha256&&
        latest.status_sha256==current.status_sha256&&
        latest.acceptance_criteria==current.acceptance_criteria&&
        latest.selected_head==current.selected_head;
    let required_criterion_count=whole_criteria(
        &latest.acceptance_criteria
    ).map_or(0,|c|c.len());
    let reviewed_set=if full&&native&&source_current{
        independently_reopened_whole_task_witnesses(
            profile,&conn,&r,&board.sha256,&latest,chats
        )?
    }else{None};
    let all_declared_criteria_independently_verified=
        source_current&&native&&reviewed_set.is_some()&&
        completion_native_eligible(&latest,
            dependency_completion_current(&conn,&r,&board.sha256,&latest)?)&&
        required_criterion_count>0;
    let complete=full&&all_declared_criteria_independently_verified&&
        completed_for_current_source(
            &conn,&r,&board.sha256,&latest,
            reviewed_set.as_ref().and_then(|s|s.receipt_binding.as_deref())
        )?;
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
        }else if !full{
            "STOP_REVIEW_HISTORY_RECONCILIATION_REQUIRED"
        }else if !all_declared_criteria_independently_verified{
            "REQUIRES_EVERY_DECLARED_CRITERION_INDEPENDENT_PROOF"
        }else{
            "ALL_CRITERIA_VERIFIED_AWAITING_SEPARATE_HUMAN_TASK_DECISION"
        },
        completion_decision_count:decisions.len(),
        required_criterion_count,
        all_declared_criteria_independently_verified,
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
        assert!(full_source_contract(&format!(
            "LU_COMPLETE_V1:GIT_HEAD_EQUALS:{}", "a".repeat(40))));
        for bad in [
            "LU_COMPLETE_V1:GIT_HEAD_EQUALS:HEAD",
            "LU_COMPLETE_V1:GIT_HEAD_EQUALS:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaA",
            "LU_COMPLETE_V1:GIT_HEAD_EQUALS:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\\nDeploy",
            "LU_COMPLETE_V1:GIT_HEAD_EQUALS:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa;run",
            "LU_COMPLETE_V1:GIT_HEAD_EQUALS:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa ",
            "LU_COMPLETE_V1:GIT_HEAD_EQUALS:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",

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
            previous_event_sha256:None,task_specific_witness_sha256:None,
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
        let mut forged=accepted.clone();
        forged.task_specific_witness_sha256=Some("broken".into());
        assert!(!completion_decision_valid(&forged));
        let mut conservative=completion_fixture("UNCERTAIN");
        conservative.task_specific_witness_sha256=Some("a".repeat(64));
        assert!(!completion_decision_valid(&conservative));
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
    fn all_of_manifest_requires_every_canonical_exact_machine_clause(){
        let head="a".repeat(40);
        let complete=format!(
            "LU_COMPLETE_V2:ALL_OF\nCURRENT_NATIVE_REQUIREMENT\n\
             GIT_HEAD_EQUALS:{head}\nGIT_STATUS_REVIEWED\nWAV_PCM_REVIEWED"
        );
        let parsed=whole_criteria(&complete).unwrap();
        assert_eq!(parsed.len(),4);
        assert_eq!(git_head_expected(&complete),Some(head.as_str()));
        assert!(full_source_contract(&complete));
        for (tool,method) in [
            ("lu.reviewed.git.head","GIT_HEAD_DIRECT_V1"),
            ("lu.reviewed.git.status","GIT_STATUS_INDEX_TREE_V1"),
            ("lu.reviewed.media.facts","WAV_PCM_DIRECT_V1")
        ]{
            assert_eq!(whole_criterion_supports_method(
                &complete,tool,method),Some(true));
        }
        assert_eq!(whole_criterion_supports_method(
            &complete,"lu.reviewed.media.facts","MP4_BOXES_DIRECT_V1"
        ),Some(false));
        for invalid in [
            format!("{complete}\nAlso deploy"),
            format!("{complete}\n"),
            format!("LU_COMPLETE_V2:ALL_OF\nCURRENT_NATIVE_REQUIREMENT\n\
                GIT_HEAD_EQUALS:{head}\nGIT_HEAD_EQUALS:{head}"),
            format!("LU_COMPLETE_V2:ALL_OF\nGIT_HEAD_EQUALS:{head}\n\
                CURRENT_NATIVE_REQUIREMENT"),
            format!("LU_COMPLETE_V2:ALL_OF\nCURRENT_NATIVE_REQUIREMENT\n\
                GIT_STATUS_REVIEWED\nGIT_HEAD_EQUALS:{head}"),
            "LU_COMPLETE_V2:ALL_OF\nCURRENT_NATIVE_REQUIREMENT".into(),
            "LU_COMPLETE_V2:ALL_OF\nCURRENT_NATIVE_REQUIREMENT\n\
                GIT_STATUS_REVIEWED;EXEC".into(),
            "LU_COMPLETE_V2:ALL_OF\nCURRENT_NATIVE_REQUIREMENT\n\
                GIT_STATUS_REVIEWED\r\n".into(),
        ]{
            assert!(!full_source_contract(&invalid),
                "malformed/hidden criterion must fail closed: {invalid}");
            assert_eq!(whole_criterion_supports_method(
                &invalid,"lu.reviewed.git.status","GIT_STATUS_INDEX_TREE_V1"
            ),Some(false));
        }
    }
    #[test]
    fn all_of_distinct_method_reviews_are_required_and_rescinded_individually(){
        let head="a".repeat(40);
        let criterion=format!(
            "LU_COMPLETE_V2:ALL_OF\nCURRENT_NATIVE_REQUIREMENT\n\
             GIT_HEAD_EQUALS:{head}\nGIT_STATUS_REVIEWED\nWAV_PCM_REVIEWED"
        );
        let task=register::TaskRevision{
            task_id:"lu-task-all".into(),version:3,
            source_requirement_id:"R-all".into(),summary:"Multi criterion".into(),
            description:"All original typed source and worker facts".into(),
            group:"Reviewed".into(),priority:"P1".into(),blocked_by:"none".into(),
            acceptance_criteria:criterion.clone(),depends_on:vec![],
            evidence_ids:vec!["R-all".into()],
            original_model_proposal_sha256:"a".repeat(64),
            graph_sha256:"b".repeat(64),status_sha256:"c".repeat(64),
            selected_head:Some(head.clone()),
            observed_status:lu_core::project_status_classifier::Status::Accepted,
            observed_at_unix:0,needs_human_reconciliation:false,
            independently_verified_requirement:true,execution_authorized:false,
        };
        let register=register::TaskRegister{
            schema:register::SCHEMA.into(),register_id:"lu-register-all".into(),
            scope_sha256:"d".repeat(64),
            original_proposal_sha256:"e".repeat(64),events:vec![],
            latest_sha256:"f".repeat(64),no_execution_authority:true,
        };
        let board="0".repeat(64);
        let event=|tool:&str,method:&str,id:&str|CriterionDecision{
            schema:SCHEMA.into(),event_id:id.into(),
            register_id:register.register_id.clone(),
            task_id:task.task_id.clone(),task_version:task.version,
            board_sha256:board.clone(),
            register_sha256:register.latest_sha256.clone(),
            source_scope_sha256:register.scope_sha256.clone(),
            graph_sha256:task.graph_sha256.clone(),
            status_sha256:task.status_sha256.clone(),
            selected_head:task.selected_head.clone(),
            source_requirement_id:task.source_requirement_id.clone(),
            acceptance_criterion:task.acceptance_criteria.clone(),
            acceptance_criterion_sha256:sha(criterion.as_bytes()),
            handoff_id:format!("h-{id}"),receipt_sha256:sha(id.as_bytes()),
            observed_sha256:sha(format!("observed-{id}").as_bytes()),
            tool_id:tool.into(),independent_method:method.into(),
            step_index:0,verdict_at_review:"VERIFIED_AT_OBSERVATION".into(),
            decision:"ACCEPTED_AS_SUPPORTING_EVIDENCE".into(),
            operator_reason:"Separate original native human witness review".into(),
            previous_event_sha256:None,
            task_completed:false,execution_authorized:false,
        };
        let head_event=event("lu.reviewed.git.head","GIT_HEAD_DIRECT_V1","ev-head");
        let status=event("lu.reviewed.git.status","GIT_STATUS_INDEX_TREE_V1","ev-status");
        let wav=event("lu.reviewed.media.facts","WAV_PCM_DIRECT_V1","ev-wav");
        let all=vec![head_event.clone(),status.clone(),wav.clone()];
        assert!(all.iter().all(review_event_valid));
        assert!(completion_native_eligible(&task,true));
        assert!(select_all_required_reviews(&all,&register,&board,&task)
            .is_some_and(|r|r.len()==3));
        for removed in 0..3{
            let mut partial=all.clone();partial.remove(removed);
            assert!(select_all_required_reviews(&partial,&register,&board,&task)
                .is_none(),"one missing original worker must block");
        }
        let reviews=select_all_required_reviews(&all,&register,&board,&task).unwrap();
        let binding=selected_witness_binding(&criterion,&reviews).unwrap();
        assert!(valid_hash(&binding));
        let mut completion=completion_fixture("COMPLETED_AT_REVIEW");
        assert!(!completion_receipt_matches(&task,&completion,Some(&binding)));
        completion.task_specific_witness_sha256=Some(binding.clone());
        assert!(completion_receipt_matches(&task,&completion,Some(&binding)));
        assert!(!completion_receipt_matches(&task,&completion,None));
        assert!(!completion_receipt_matches(&task,&completion,
            Some("1".repeat(64).as_str())));
        // Later explicit human negative revokes just that criterion even if
        // all earlier positives remain in the append-only review journal.
        let mut rescinded=all.clone();
        let mut negative=status.clone();
        negative.event_id="ev-status-rejected".into();
        negative.decision="NEEDS_MORE_INDEPENDENT_EVIDENCE".into();
        rescinded.push(negative);
        assert!(select_all_required_reviews(
            &rescinded,&register,&board,&task
        ).is_none());
        let mut stale=all.clone();
        stale[1].task_version+=1;
        assert!(select_all_required_reviews(&stale,&register,&board,&task)
            .is_none());
        let mut stale=all.clone();
        stale[2].register_sha256="0".repeat(64);
        assert!(select_all_required_reviews(&stale,&register,&board,&task)
            .is_none());
        let mut stale=task.clone();
        stale.selected_head=Some("1".repeat(40));
        assert!(!current_whole_task_criterion(&stale));
        assert!(select_all_required_reviews(&all,&register,&board,&stale)
            .is_none());
        // A model completion/review-only step can never substitute for a
        // CURRENT native OS source, all original receipts and human decision.
        let mut unverified=task;unverified.independently_verified_requirement=false;
        assert!(!completion_native_eligible(&unverified,true));
        let mut denied=completion;
        denied.human_authorized=false;
        assert!(!completion_decision_valid(&denied));
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
        let mut git=valid.clone();
        let head="a".repeat(40);
        git.acceptance_criteria=format!("LU_COMPLETE_V1:GIT_HEAD_EQUALS:{head}");
        git.selected_head=Some(head.clone());
        assert!(completion_native_eligible(&git,true));
        assert!(!completion_native_eligible(&git,false));
        git.selected_head=Some("b".repeat(40));
        assert!(!completion_native_eligible(&git,true));
        git.selected_head=None;
        assert!(!completion_native_eligible(&git,true));
        git.selected_head=Some(head);
        git.acceptance_criteria.push_str("; also deploy it");
        assert!(!completion_native_eligible(&git,true));
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
    fn git_head_whole_task_requires_exact_human_review_and_current_source(){
        let head="a".repeat(40);
        let task=register::TaskRevision{
            task_id:"lu-task-git".into(),version:2,
            source_requirement_id:"R-git".into(),
            summary:"Git head proof".into(),description:"Verify exact Git commit".into(),
            group:"Native checks".into(),priority:"P1".into(),blocked_by:"none".into(),
            acceptance_criteria:format!("LU_COMPLETE_V1:GIT_HEAD_EQUALS:{head}"),
            depends_on:vec![],evidence_ids:vec!["R-git".into()],
            original_model_proposal_sha256:"f".repeat(64),
            graph_sha256:"a".repeat(64),status_sha256:"b".repeat(64),
            selected_head:Some(head.clone()),
            observed_status:lu_core::project_status_classifier::Status::Accepted,
            observed_at_unix:5,needs_human_reconciliation:false,
            independently_verified_requirement:true,execution_authorized:false,
        };
        let register=register::TaskRegister{
            schema:register::SCHEMA.into(),register_id:"lu-register-git".into(),
            scope_sha256:"c".repeat(64),
            original_proposal_sha256:"d".repeat(64),events:vec![],
            latest_sha256:"e".repeat(64),no_execution_authority:true,
        };
        let board="1".repeat(64);
        let base=CriterionDecision{
            schema:SCHEMA.into(),event_id:"ev-original".into(),
            register_id:register.register_id.clone(),task_id:task.task_id.clone(),
            task_version:task.version,board_sha256:board.clone(),
            register_sha256:register.latest_sha256.clone(),
            source_scope_sha256:register.scope_sha256.clone(),
            graph_sha256:task.graph_sha256.clone(),
            status_sha256:task.status_sha256.clone(),
            selected_head:Some(head),
            source_requirement_id:task.source_requirement_id.clone(),
            acceptance_criterion:task.acceptance_criteria.clone(),
            acceptance_criterion_sha256:sha(task.acceptance_criteria.as_bytes()),
            handoff_id:"h-original".into(),receipt_sha256:"2".repeat(64),
            observed_sha256:"3".repeat(64),
            tool_id:"lu.reviewed.git.head".into(),
            independent_method:"GIT_HEAD_DIRECT_V1".into(),
            step_index:0,verdict_at_review:"VERIFIED_AT_OBSERVATION".into(),
            decision:"ACCEPTED_AS_SUPPORTING_EVIDENCE".into(),
            operator_reason:"Original human reviewed actual Git head".into(),
            previous_event_sha256:None,task_completed:false,execution_authorized:false,
        };
        assert!(review_event_valid(&base));
        assert!(reviewed_git_head_event_matches(&base,&register,&board,&task));
        let receipt=base.receipt_sha256.as_str();
        let mut done=completion_fixture("COMPLETED_AT_REVIEW");
        done.task_specific_witness_sha256=Some(receipt.into());
        assert!(completion_decision_valid(&done));
        assert!(completion_receipt_matches(&task,&done,Some(receipt)));
        assert!(!completion_receipt_matches(&task,&done,None));
        assert!(!completion_receipt_matches(&task,&done,Some("4".repeat(64).as_str())));
        for invalid in [
            {let mut x=base.clone();x.decision="REJECTED_AS_IRRELEVANT".into();x},
            {let mut x=base.clone();x.tool_id="lu.reviewed.git.status".into();x},
            {let mut x=base.clone();x.independent_method="GIT_STATUS_INDEX_TREE_V1".into();x},
            {let mut x=base.clone();x.source_scope_sha256="0".repeat(64);x},
            {let mut x=base.clone();x.register_sha256="0".repeat(64);x},
            {let mut x=base.clone();x.selected_head=Some("f".repeat(40));x},
            {let mut x=base.clone();x.task_version+=1;x},
            {let mut x=base.clone();x.acceptance_criterion.push_str("; deploy");x},
            {let mut x=base.clone();x.verdict_at_review="UNCERTAIN".into();x},
            {let mut x=base.clone();x.task_id="lu-task-substitution".into();x},
        ]{
            assert!(!reviewed_git_head_event_matches(
                &invalid,&register,&board,&task));
        }
        let mut stale=task.clone();
        stale.selected_head=Some("f".repeat(40));
        assert!(!current_whole_task_criterion(&stale));
        assert!(!reviewed_git_head_event_matches(&base,&register,&board,&stale));
        let mut unsupported=task;
        unsupported.acceptance_criteria="Perform all tests and deploy".into();
        assert!(!completion_native_eligible(&unsupported,true));
        assert!(!completion_receipt_matches(&unsupported,&done,Some(receipt)));
    }
    #[test]
    fn transitive_source_dependency_rescission_invalidates_parent_completion(){
        fn task(id:&str,depends:Vec<String>)->register::TaskRevision{
            register::TaskRevision{
                task_id:id.into(),version:1,source_requirement_id:"R-1".into(),
                summary:"Original native requirement".into(),
                description:"Strict original deterministic native source".into(),
                group:"Source".into(),priority:"P1".into(),blocked_by:"none".into(),
                acceptance_criteria:NATIVE_SOURCE_ONLY_CONTRACT.into(),
                depends_on:depends,evidence_ids:vec!["R-1".into()],
                original_model_proposal_sha256:"e".repeat(64),
                graph_sha256:"c".repeat(64),status_sha256:"d".repeat(64),
                selected_head:None,
                observed_status:lu_core::project_status_classifier::Status::Accepted,
                observed_at_unix:0,needs_human_reconciliation:false,
                independently_verified_requirement:true,execution_authorized:false
            }
        }
        fn signed(conn:&Connection,event:&CompletionDecision,seq:i64){
            let bytes=serde_json::to_vec(event).unwrap();
            conn.execute(
                "INSERT INTO ansf_project_task_completion_decisions(
                    register_id,task_id,sequence,event_sha256,event_json,no_execution_authority
                ) VALUES(?1,?2,?3,?4,?5,1)",
                params![event.register_id,event.task_id,seq,sha(&bytes),bytes]
            ).unwrap();
        }
        let db=Connection::open_in_memory().unwrap();
        completion_init(&db).unwrap();
        let id="lu-register-fixture";
        let base="lu-task-base";
        let mid="lu-task-mid";
        let root="lu-task-root";
        let tasks=[
            task(base,vec![]),
            task(mid,vec![base.into()]),
            task(root,vec![mid.into()])
        ];
        let events=tasks.iter().enumerate().map(|(i,t)|register::TaskEvent{
            sequence:i as u64+1,
            kind:register::RevisionKind::Imported,
            previous_sha256:None,reason:"Original fixture source admission".into(),
            revision:t.clone(),sha256:"f".repeat(64),
        }).collect();
        let register=register::TaskRegister{
            schema:"lu-project-task-register-v1".into(),
            register_id:id.into(),scope_sha256:"a".repeat(64),
            original_proposal_sha256:"e".repeat(64),events,
            latest_sha256:"b".repeat(64),no_execution_authority:true
        };
        let mut original_base=completion_fixture("COMPLETED_AT_REVIEW");
        original_base.task_id=base.into();
        let mut original_mid=original_base.clone();
        original_mid.task_id=mid.into();
        signed(&db,&original_base,1);
        signed(&db,&original_mid,1);
        assert!(dependency_completion_current(
            &db,&register,&"a".repeat(64),&tasks[2]
        ).unwrap(),"two-layer dependencies must reopen original decisions");
        // Test a cycle while all original positive receipts still exist.
        // Otherwise an earlier conservative negative might mask the bug.
        // A cyclic dependency cannot be legitimized by an old signed row.
        let cyc=task(root,vec![mid.into()]);
        let mut new_mid=task(mid,vec![root.into()]);
        new_mid.version=1;
        let mut cyclic=register.clone();
        cyclic.events.push(register::TaskEvent{
            sequence:4,kind:register::RevisionKind::HumanCorrection,
            previous_sha256:None,reason:"Source cycle test".into(),
            revision:new_mid,sha256:"f".repeat(64)
        });
        assert!(!dependency_completion_current(
            &db,&cyclic,&"a".repeat(64),&cyc
        ).unwrap());
        let mut uncertain=completion_fixture("UNCERTAIN");
        uncertain.task_id=base.into();
        uncertain.previous_event_sha256=Some(sha(
            &serde_json::to_vec(&original_base).unwrap()
        ));
        signed(&db,&uncertain,2);
        assert!(!dependency_completion_current(
            &db,&register,&"a".repeat(64),&tasks[2]
        ).unwrap(),"uncertain grandparent invalidates descendant completion");
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
        assert!(all_human_criterion_history_inspected(0));
        assert!(all_human_criterion_history_inspected(
            MAX_REOPENED_CRITERION_REVIEWS
        ));
        assert!(!all_human_criterion_history_inspected(
            MAX_REOPENED_CRITERION_REVIEWS+1
        ),"an omitted original human review invalidates a completion claim");
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
