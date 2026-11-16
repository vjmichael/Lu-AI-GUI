//! P0-NEXT: human-selected PLAN05/06 task -> ANSF goal -> AGENT01 draft.
//! A handoff is provenance, NEVER an execution authorization or task verdict.
//! Prepared rows are deliberately not auto-retried on crash; human reconciliation
//! must re-check the original source and the potentially interrupted ANSF writes.
use super::*;
use lu_core::{
    ansf_goals::{GoalEvent,GoalEventKind},
    agent_plan_ledger::PlanStepKind,
    agent_plan_revisions::{ExpectedDeliverable,RevisionStep,AgentPlanRevisionStore},
    agent_plan_ledger::AgentPlanLedger,
    agent_host_scheduler::AgentHostScheduler,
    agent_deliverable_receipts::AgentDeliverableReceipts,
    project_task_register as register,
    project_task_synthesis as synthesis,
};
use crate::agent_plan_admission::{stage_initial_draft,ProposedStep};
use rusqlite::{Connection,params,OptionalExtension,TransactionBehavior};
use serde::{Serialize,Deserialize};

const SCHEMA:&str="lu-project-task-agent-handoff-v1";
const MAX_HANDOFFS:usize=64;

#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Anchor {
    schema:String,
    register_id:String,
    task_id:String,
    task_version:u64,
    board_sha256:String,
    register_sha256:String,
    project_scope_sha256:String,
    requirement_id:String,
    graph_sha256:String,
    status_sha256:String,
    selected_head:Option<String>,
    mode:String,
    summary:String,
    description:String,
    acceptance_criteria:String,
    blockers:String,
    chat_id:String,
    user_turn_id:String,
    goal_event_id:String,
    handoff_id:String,
    no_execution_authority:bool,
}

#[derive(Clone,Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct TaskAgentEvidenceCandidate{
    step_index:u8,
    tool_id:String,
    independent_method:String,
    receipt_sha256:String,
    observed_sha256:String,
    historical_verdict:String,
    review_eligible:bool,
    criterion_method_reviewable:bool,
    original_requirement_id:String,
    task_completed:bool,
    execution_authorized:bool,
}

#[derive(Clone,Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct TaskAgentHandoffView{
    schema:&'static str,
    handoff_id:String,
    register_id:String,
    task_id:String,
    task_version:u64,
    mode:String,
    phase:String,
    chat_id:String,
    plan_id:Option<String>,
    revision_sha256:Option<String>,
    source_lease_current:bool,
    plan_review:String,
    plan_state:String,
    supervisor_state:String,
    historical_witness_linked:bool,
    independently_audited_outcomes:Vec<TaskAgentEvidenceCandidate>,
    next_safe_action:String,
    task_complete:bool,
    execution_authorized:bool,
}

fn mode_steps(mode:&str,criterion:&str)->Result<Vec<ProposedStep>,String>{
    match mode{
        "review_only"=>Ok(vec![
            ProposedStep{kind:PlanStepKind::CollectSource,depends_on:None,
                expected:ExpectedDeliverable::SourceReferences},
            ProposedStep{kind:PlanStepKind::Analyze,depends_on:Some(0),
                expected:ExpectedDeliverable::StructuredFindings},
            ProposedStep{kind:PlanStepKind::HumanReview,depends_on:Some(1),
                expected:ExpectedDeliverable::HumanDecision},
        ]),
        "reviewed_git_media_check"=>Ok(vec![
            ProposedStep{kind:PlanStepKind::IndependentlyCheck,depends_on:None,
                expected:ExpectedDeliverable::IndependentWitness},
        ]),
        "reviewed_whole_task"=>{
            let methods=super::project_task_verified_evidence::
                required_whole_task_independent_methods(criterion)
                .ok_or("CORE_C3_REQUIRES_CANONICAL_ALL_OF_CRITERIA")?;
            // One independent check per exact worker-method requirement.
            // These draft steps grant no tool use. Every step still needs
            // plan review, a fresh human turn and separately authorized native
            // execution; independent review follows original worker receipts.
            Ok(methods.iter().enumerate().map(|(index,_)|ProposedStep{
                kind:PlanStepKind::IndependentlyCheck,
                depends_on:if index==0{None}else{Some((index-1) as u8)},
                expected:ExpectedDeliverable::IndependentWitness,
            }).collect())
        },
        _=>Err("P0_NEXT_UNRECOGNIZED_PLAN_MODE".into()),
    }
}
fn digest(bytes:&[u8])->String{format!("{:x}",Sha256::digest(bytes))}
fn init(conn:&Connection)->Result<(),String>{
    conn.execute_batch("CREATE TABLE IF NOT EXISTS ansf_project_task_agent_handoffs(
        handoff_id TEXT PRIMARY KEY,
        register_id TEXT NOT NULL,
        task_id TEXT NOT NULL,
        task_version INTEGER NOT NULL CHECK(task_version>0),
        anchor_sha256 TEXT NOT NULL,
        anchor_json BLOB NOT NULL,
        chat_id TEXT NOT NULL,
        turn_id TEXT NOT NULL,
        goal_event_id TEXT NOT NULL,
        phase TEXT NOT NULL CHECK(phase IN ('PREPARED','STAGED')),
        plan_id TEXT,
        revision_sha256 TEXT,
        no_execution_authority INTEGER NOT NULL CHECK(no_execution_authority=1),
        UNIQUE(register_id,task_id,task_version)
    );").map_err(|_|"P0_NEXT_HANDOFF_SCHEMA_UNAVAILABLE")?;
    Ok(())
}

pub(super) fn source_anchor(
    profile:&Path,project:&Path,register_id:&str,task_id:&str,
    expected_board:&str,expected_register:&str,expected_version:u64,
)->Result<register::TaskRevision,String>{
    let conn=project_register_desktop::library(profile)?;
    let (_,r,_)=project_register_desktop::load_register(&conn,register_id)?
        .ok_or("P0_NEXT_ORIGINAL_REGISTER_MISSING")?;
    let board=project_task_board_desktop::read(&conn,&r)?;
    if board.review_state!="human_approved"
        ||board.source_register_sha256!=r.latest_sha256
        ||board.sha256!=expected_board
        ||r.latest_sha256!=expected_register
        ||!board.ordered_task_ids.iter().any(|id|id==task_id)
        ||!board.no_execution_authority||!r.no_execution_authority{
        return Err("P0_NEXT_TASK_PLAN_NOT_CURRENT_AND_HUMAN_REVIEWED".into())
    }
    let task=register::latest(&r,task_id)
        .ok_or("P0_NEXT_ORIGINAL_TASK_MISSING")?;
    if task.version!=expected_version
        ||task.needs_human_reconciliation||task.execution_authorized{
        return Err("P0_NEXT_TASK_REVISION_UNRECONCILED_OR_CHANGED".into())
    }
    let (original_g,original_s)=project_register_desktop::archived_evidence(
        &conn,&task.graph_sha256,&task.status_sha256)?;
    if original_g.project_scope_sha256!=r.scope_sha256
        ||task.selected_head!=original_g.selected_head{
        return Err("P0_NEXT_UNVERIFIED_SOURCE_PROVENANCE".into())
    }
    drop(conn);
    // A human-selected native folder is scanned AGAIN, after OS approval.
    // A matching stale register alone is never sufficient to bind this plan.
    let current=project_status_desktop::classify_selected(project)?;
    if current.0.project_scope_sha256!=r.scope_sha256
        ||current.0.complete!=original_g.complete
        ||current.1.complete!=original_s.complete
        ||!synthesis::same_observed_sources(&original_g,&current.0)?
        ||original_g.selected_head!=current.0.selected_head
        ||serde_json::to_vec(&original_s.findings)
            .map_err(|_|"P0_NEXT_ORIGINAL_STATUS_UNREADABLE")?
          !=serde_json::to_vec(&current.1.findings)
            .map_err(|_|"P0_NEXT_CURRENT_STATUS_UNREADABLE")?{
        return Err("P0_NEXT_PROJECT_SOURCE_DRIFT_REQUIRES_PLAN07_RECONCILIATION".into())
    }
    Ok(task.clone())
}

fn check_existing(conn:&Connection,id:&str,task:&str,version:u64)
 ->Result<(),String>{
    init(conn)?;
    let existing:Option<(String,String)>=conn.query_row(
        "SELECT handoff_id,phase FROM ansf_project_task_agent_handoffs
         WHERE register_id=?1 AND task_id=?2 AND task_version=?3",
        params![id,task,version as i64],
        |r|Ok((r.get(0)?,r.get(1)?))
    ).optional().map_err(|_|"P0_NEXT_HANDOFF_LOOKUP_FAILED")?;
    if let Some((_,phase))=existing{
        let reason=if phase=="PREPARED"{
            "P0_NEXT_PREPARED_HANDOFF_REQUIRES_MANUAL_RECOVERY"
        }else{"P0_NEXT_EXISTING_PLAN_LINK_REOPEN_IN_TASK_BOARD"};
        return Err(reason.to_string())
    }
    let count:i64=conn.query_row("SELECT COUNT(*) FROM ansf_project_task_agent_handoffs",
        [],|r|r.get(0)).map_err(|_|"P0_NEXT_HANDOFF_COUNT_FAILED")?;
    if count<0||count as usize>=MAX_HANDOFFS{
        return Err("P0_NEXT_BOUNDED_HANDOFF_CAPACITY".into())
    }
    Ok(())
}

fn insert_prepared(profile:&Path,anchor:&Anchor)->Result<(),String>{
    let mut conn=project_register_desktop::library(profile)?;
    let tx=conn.transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_|"P0_NEXT_PREPARE_TRANSACTION_BUSY")?;
    let (_,r,_)=project_register_desktop::load_register(&tx,&anchor.register_id)?
        .ok_or("P0_NEXT_REGISTER_CHANGED_DURING_PREPARE")?;
    let board=project_task_board_desktop::read(&tx,&r)?;
    let task=register::latest(&r,&anchor.task_id)
        .ok_or("P0_NEXT_TASK_CHANGED_DURING_PREPARE")?;
    if r.latest_sha256!=anchor.register_sha256
        ||board.sha256!=anchor.board_sha256
        ||board.review_state!="human_approved"
        ||board.source_register_sha256!=r.latest_sha256
        ||task.version!=anchor.task_version
        ||task.graph_sha256!=anchor.graph_sha256
        ||task.status_sha256!=anchor.status_sha256
        ||task.needs_human_reconciliation
        ||r.scope_sha256!=anchor.project_scope_sha256{
        return Err("P0_NEXT_SOURCE_LEASE_CHANGED_BEFORE_ANSF_HANDOFF".into())
    }
    check_existing(&tx,&anchor.register_id,&anchor.task_id,anchor.task_version)?;
    let bytes=serde_json::to_vec(anchor).map_err(|_|"P0_NEXT_ANCHOR_ENCODE")?;
    if bytes.len()>8192{return Err("P0_NEXT_ANCHOR_TOO_LARGE".into())}
    tx.execute("INSERT INTO ansf_project_task_agent_handoffs(
        handoff_id,register_id,task_id,task_version,anchor_sha256,anchor_json,
        chat_id,turn_id,goal_event_id,phase,plan_id,revision_sha256,no_execution_authority)
        VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,'PREPARED',NULL,NULL,1)",
        params![anchor.handoff_id,anchor.register_id,anchor.task_id,
            anchor.task_version as i64,digest(&bytes),bytes,
            anchor.chat_id,anchor.user_turn_id,anchor.goal_event_id]
    ).map_err(|_|"P0_NEXT_PREPARED_WRITE_REFUSED")?;
    tx.commit().map_err(|_|"P0_NEXT_PREPARED_COMMIT_REFUSED")?;
    Ok(())
}
fn publish(profile:&Path,anchor:&Anchor,plan:&str,revision:&str)->Result<(),String>{
    let conn=project_register_desktop::library(profile)?;
    let changed=conn.execute(
        "UPDATE ansf_project_task_agent_handoffs
         SET phase='STAGED',plan_id=?2,revision_sha256=?3
         WHERE handoff_id=?1 AND phase='PREPARED'
         AND plan_id IS NULL AND revision_sha256 IS NULL",
        params![anchor.handoff_id,plan,revision]
    ).map_err(|_|"P0_NEXT_HANDOFF_PUBLICATION_FAILED")?;
    if changed!=1{return Err("P0_NEXT_HANDOFF_PREPARED_NOT_UNIQUE".into())}
    Ok(())
}

/// Every displayed link is audited against immutable original ANSF bytes,
/// exact current task version and the existing AGENT-01 read-only ledger.
/// Historical VERIFIED status never marks the project task complete.
fn list(profile:&Path,register_id:&str,task_id:&str,
    chats:&chat_store::ChatStore)->Result<Vec<TaskAgentHandoffView>,String>{
    let conn=project_register_desktop::library(profile)?;
    init(&conn)?;
    let (_proposal,r,_)=project_register_desktop::load_register(&conn,register_id)?
        .ok_or("P0_NEXT_REGISTER_NOT_FOUND")?;
    let board=project_task_board_desktop::read(&conn,&r)?;
    let mut stmt=conn.prepare(
        "SELECT anchor_json,anchor_sha256,phase,plan_id,revision_sha256,
                chat_id,turn_id,goal_event_id,no_execution_authority
         FROM ansf_project_task_agent_handoffs
         WHERE register_id=?1 AND task_id=?2
         ORDER BY rowid DESC LIMIT 16"
    ).map_err(|_|"P0_NEXT_HANDOFF_HISTORY_QUERY")?;
    let mut rows=stmt.query(params![register_id,task_id])
        .map_err(|_|"P0_NEXT_HANDOFF_HISTORY_READ")?;
    let mut items=Vec::new();
    while let Some(row)=rows.next().map_err(|_|"P0_NEXT_HANDOFF_ROW_FAILED")?{
        let bytes:Vec<u8>=row.get(0).map_err(|_|"P0_NEXT_ANCHOR_MISSING")?;
        let sha:String=row.get(1).map_err(|_|"P0_NEXT_ANCHOR_SHA_MISSING")?;
        let phase:String=row.get(2).map_err(|_|"P0_NEXT_PHASE_MISSING")?;
        let plan:Option<String>=row.get(3).map_err(|_|"P0_NEXT_PLAN_MISSING")?;
        let revision:Option<String>=row.get(4).map_err(|_|"P0_NEXT_REVISION_MISSING")?;
        let chat:String=row.get(5).map_err(|_|"P0_NEXT_CHAT_MISSING")?;
        let turn:String=row.get(6).map_err(|_|"P0_NEXT_TURN_MISSING")?;
        let goal:String=row.get(7).map_err(|_|"P0_NEXT_GOAL_MISSING")?;
        let no_exec:i64=row.get(8).map_err(|_|"P0_NEXT_AUTHORITY_MISSING")?;
        if bytes.len()>8192||sha!=digest(&bytes)||no_exec!=1{
            return Err("P0_NEXT_ORIGINAL_HANDOFF_TAMPERED".into())
        }
        let a:Anchor=serde_json::from_slice(&bytes)
            .map_err(|_|"P0_NEXT_ORIGINAL_HANDOFF_INVALID")?;
        if a.schema!=SCHEMA||a.register_id!=register_id||a.task_id!=task_id
            ||a.chat_id!=chat||a.user_turn_id!=turn||a.goal_event_id!=goal
            ||!a.no_execution_authority
            ||!matches!(phase.as_str(),"PREPARED"|"STAGED")
            ||(phase=="STAGED"&&(plan.is_none()||revision.is_none()))
            ||(phase=="PREPARED"&&(plan.is_some()||revision.is_some())){
            return Err("P0_NEXT_HANDOFF_PROVENANCE_INVALID".into())
        }
        let current=board.review_state=="human_approved"
            &&board.source_register_sha256==r.latest_sha256
            &&board.sha256==a.board_sha256
            &&r.latest_sha256==a.register_sha256
            &&r.scope_sha256==a.project_scope_sha256
            &&register::latest(&r,task_id).is_some_and(|task|
                task.version==a.task_version&&!task.needs_human_reconciliation
                &&task.graph_sha256==a.graph_sha256
                &&task.status_sha256==a.status_sha256);
        let mut view=TaskAgentHandoffView{
            schema:SCHEMA,handoff_id:a.handoff_id.clone(),
            register_id:a.register_id.clone(),task_id:a.task_id.clone(),
            task_version:a.task_version,mode:a.mode.clone(),phase:phase.clone(),
            chat_id:a.chat_id.clone(),plan_id:plan.clone(),
            revision_sha256:revision.clone(),source_lease_current:current,
            plan_review:"NOT_YET_STAGED".into(),plan_state:"PREPARED".into(),
            supervisor_state:"NOT_CLAIMED".into(),historical_witness_linked:false,
            independently_audited_outcomes:Vec::new(),
            next_safe_action:"Human reconciliation required after prepared handoff; never replay automatically.".into(),
            task_complete:false,execution_authorized:false,
        };
        if phase=="STAGED"{
            if chats.trusted_user_turn_text(&chat,&turn).is_none()
                ||!chats.memory_events(&chat)?.iter().any(|e|
                    e.event_id==goal&&e.kind==GoalEventKind::SetGoal){
                return Err("P0_NEXT_ORIGINAL_CHAT_OR_GOAL_SOURCE_MISSING".into())
            }
            let progress=agent_plan_progress::read_chat_progress(profile,&chat,chats)?
                .into_iter().find(|p|Some(p.plan_id.as_str())==plan.as_deref())
                .ok_or("P0_NEXT_AGENT_PLAN_LOST")?;
            if progress.original_human_turn_id!=turn
                ||progress.latest_revision==0{
                return Err("P0_NEXT_AGENT_PLAN_ORIGINAL_TURN_INVALID".into())
            }
            let same_revision=progress.revision_sha256.as_deref()==revision.as_deref();
            view.plan_review=if same_revision{progress.revision_review}
                else{"SOURCE_LINK_REFERENCES_PRIOR_REVISION".into()};
            view.plan_state=progress.plan_state;
            view.supervisor_state=progress.supervisor_state;
            view.historical_witness_linked=progress.steps.iter()
                .any(|s|s.historical_witness_linked);
            // Independent history reopens ORIGINAL worker, effect and oracle
            // receipts. A scheduler state or a plan's own witness flag is
            // NEVER sufficient to create a project completion claim.
            let file=agent_plan_review::existing_plan_file(profile)?;
            let ledger=AgentPlanLedger::open_existing_readonly(&file)
                .map_err(|_|"P0_NEXT_PLAN_HISTORY_UNAVAILABLE")?;
            let revisions=AgentPlanRevisionStore::open_existing_readonly(&file)
                .map_err(|_|"P0_NEXT_REVISION_HISTORY_UNAVAILABLE")?;
            let receipts=AgentDeliverableReceipts::open_existing_readonly(&file)
                .map_err(|_|"P0_NEXT_INDEPENDENT_RECEIPTS_UNAVAILABLE")?;
            let sources=agent_plan_review::OriginalChatGoalSources{chats};
            let original=receipts.history(plan.as_deref().ok_or("P0_NEXT_PLAN_LOST")?,
                    &chat,&turn,profile,&ledger,&revisions,&sources)
                .map_err(|_|"P0_NEXT_ORIGINAL_EFFECT_ORACLE_HISTORY_INVALID")?;
            for item in original{
                let eligible=current&&same_revision
                    &&item.status=="VERIFIED_AT_OBSERVATION";
                let criterion_method_reviewable=criterion_supports_method(
                    &a.acceptance_criteria,&item.tool_id,&item.independent_method)
                    &&(a.mode!="reviewed_whole_task"||exact_whole_task_slot(
                        &a.acceptance_criteria,item.step_index,
                        &item.tool_id,&item.independent_method));
                view.independently_audited_outcomes.push(TaskAgentEvidenceCandidate{
                    step_index:item.step_index,
                    tool_id:item.tool_id,
                    independent_method:item.independent_method,
                    receipt_sha256:item.receipt_sha256,
                    observed_sha256:item.observed_sha256,
                    historical_verdict:item.status.into(),
                    review_eligible:eligible,
                    criterion_method_reviewable,
                    original_requirement_id:a.requirement_id.clone(),
                    task_completed:false,execution_authorized:false,
                });
            }
            // A verified root-plan witness is only the first criterion.
            // Continue through immutable separately reviewed AGENT-01I child
            // plans. Only exact, released original-child worker/oracle
            // receipts may be considered for the remaining source criteria.
            if current&&same_revision&&a.mode=="reviewed_whole_task"{
                let descendants=verified_whole_task_continuations(
                    profile,&file,&a,
                    plan.as_deref().ok_or("CORE_C3_ROOT_PLAN_MISSING")?,
                    revision.as_deref().ok_or("CORE_C3_ROOT_REVISION_MISSING")?,
                    &ledger,&revisions,&receipts,&sources
                )?;
                view.independently_audited_outcomes.extend(descendants);
            }
            let has_review_candidate=view.independently_audited_outcomes.iter()
                .any(|o|o.review_eligible&&o.criterion_method_reviewable);
            view.next_safe_action=if !same_revision{
                "Agent plan revision changed. This original task link is historical; review the current plan from the original chat and grant no inherited authority.".into()
            }else if !current{
                "Source register/board changed. Review current project evidence and independently replan; do not inherit previous grants.".into()
            }else if has_review_candidate{
                "One or more ORIGINAL independent historical outcomes are eligible for a NEW human source/criterion review. Use PLAN-07 native evidence refresh; the task is NOT complete.".into()
            }else{progress.continuation_preview.required_action.into()};
        }
        items.push(view);
    }
    Ok(items)
}

/// P0-TOOLS-05: read ONE original audited worker+oracle receipt from
/// the existing AGENT plan ledger. This exposes no mutable task verdict.
#[derive(Clone,Debug)]
pub(super) struct ExactTaskWitness{
    pub handoff_id:String,
    pub tool_id:String,
    pub independent_method:String,
    pub receipt_sha256:String,
    pub observed_sha256:String,
    pub step_index:u8,
}
pub(super) fn compatible_independent_method(tool:&str,method:&str)->bool{
    match tool{
        "lu.reviewed.git.head"=>method=="GIT_HEAD_DIRECT_V1",
        "lu.reviewed.git.status"=>method=="GIT_STATUS_INDEX_TREE_V1",
        "lu.reviewed.git.branch"=>method=="GIT_BRANCH_DIRECT_V1",
        "lu.reviewed.git.history"=>matches!(method,
            "GIT_HISTORY_LOOSE_DAG_V1"|"GIT_HISTORY_OBJECT_DAG_V2"),
        "lu.reviewed.media.facts"=>matches!(method,
            "WAV_PCM_DIRECT_V1"|"MP4_BOXES_DIRECT_V1"),
        // P0-TOOLS-04 specialist receipts are standalone until a NEW
        // approved exact-method plan revision/criterion mapping is built.
        // Never disguise those as an existing reviewed Git/media witness.
        _=>false
    }
}

/// P0-NEXT-CM1: classify exact original, human-reviewed criterion METHOD,
/// not the desired result. Does not establish a satisfied task or authority.
/// A V2 whole-task worker must occupy the original declared method slot.
/// A different verified tool in the correct task is NOT interchangeable.
/// Reindexed continuation drafts need a new explicit human review; they do
/// not inherit the original handoff's criterion authority.
/// Exact bounded original plan suffix. The child starts at index zero in
/// its OWN separately reviewed plan, but its first witness corresponds to
/// the next slot of the immutable parent task declaration. No tool or model
/// may rewrite that mapping, skip a step, or use a nonlinear replan.
fn linear_independent_witness_suffix(steps:&[RevisionStep],expected:usize)->bool{
    expected>0&&expected<=4&&steps.len()==expected&&
    steps.iter().enumerate().all(|(i,step)|
        step.kind==PlanStepKind::IndependentlyCheck&&
        step.expected==ExpectedDeliverable::IndependentWitness&&
        !step.deliverable_id.is_empty()&&
        step.depends_on==if i==0{None}else{Some((i-1) as u8)}
    )
}

/// Reopen *actual* original child plans, separately saved human turns,
/// approved revisions, released scheduler custody and their independent
/// first-party effect/oracle receipts. A parent claim or model final answer
/// is not sufficient. Missing/incomplete children add NO completed criterion.
/// The original handoff identity remains the only task-level review scope.
/// This is read-only; no new grants, model work, or continuation is performed.
#[allow(clippy::too_many_arguments)]
fn verified_whole_task_continuations(
    profile:&Path,file:&Path,anchor:&Anchor,
    initial_plan:&str,initial_revision:&str,
    ledger:&AgentPlanLedger,revisions:&AgentPlanRevisionStore,
    receipts:&AgentDeliverableReceipts,
    sources:&agent_plan_review::OriginalChatGoalSources<'_>,
)->Result<Vec<TaskAgentEvidenceCandidate>,String>{
    let methods=super::project_task_verified_evidence::
        required_whole_task_independent_methods(&anchor.acceptance_criteria)
        .ok_or("CORE_C3_CONTINUATION_REQUIRES_CANONICAL_METHODS")?;
    if methods.len()<=1{return Ok(Vec::new())}
    let db=Connection::open_with_flags(file,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY|
        rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX
    ).map_err(|_|"CORE_C3_ORIGINAL_CONTINUATION_JOURNAL_MISSING")?;
    let table:Option<String>=db.query_row(
        "SELECT name FROM sqlite_master WHERE type='table'
         AND name='lu_agent_continuation_children'",
        [],|r|r.get(0)
    ).optional().map_err(|_|"CORE_C3_CONTINUATION_SCHEMA_UNREADABLE")?;
    if table.is_none(){return Ok(Vec::new())}
    let scheduler=AgentHostScheduler::open_existing_readonly(file)
        .map_err(|_|"CORE_C3_ORIGINAL_SCHEDULER_UNREADABLE")?;
    let mut visited=std::collections::BTreeSet::from([initial_plan.to_owned()]);
    let mut plan=initial_plan.to_owned();
    let mut revision=initial_revision.to_owned();
    let mut turn=anchor.user_turn_id.clone();
    let mut out=Vec::new();
    for slot in 1..methods.len(){
        // A newly prepared whole-task root is still pending review; no
        // continuation link exists yet. Do NOT make the Task Board fail just
        // because another plan somewhere created the shared child table.
        let child:Option<(String,String)>=db.query_row(
            "SELECT child_plan_id,fresh_human_turn_id
             FROM lu_agent_continuation_children
             WHERE parent_plan_id=?1 AND revision_sha256=?2 AND step_index=1",
            params![plan,revision],|r|Ok((r.get(0)?,r.get(1)?))
        ).optional().map_err(|_|"CORE_C3_CONTINUATION_LINK_UNREADABLE")?;
        let Some((next,child_turn))=child else{return Ok(out)};
        let parent_revision=revisions.latest(&plan,&anchor.chat_id)
            .map_err(|_|"CORE_C3_PARENT_REVISION_UNREADABLE")?
            .ok_or("CORE_C3_PARENT_REVISION_MISSING")?;
        if parent_revision.revision_sha256!=revision||
            parent_revision.review_state!="APPROVED_PLAN_ONLY"||
            parent_revision.source_human_turn_id!=turn||
            !linear_independent_witness_suffix(
                &parent_revision.steps,methods.len()-slot+1
            ){
            return Err("CORE_C3_ORIGINAL_PLAN_CHANGED_OR_REPLANNED".into())
        }
        if !visited.insert(next.clone())||next==plan{
            return Err("CORE_C3_CONTINUATION_CYCLE_OR_REPLAY".into())
        }
        // This original AGENT-01I verifier reopens the immutable parent
        // lineage, old human intent, fresh child origin, prior independently
        // audited receipts and durable RELEASED scheduler before accepting
        // even the child *identity*.
        if !lu_core::agent_continuation_intent::reverify_child_before_execution(
            file,profile,&next,&anchor.chat_id,ledger,revisions,
            &scheduler,receipts,sources
        ).map_err(|_|"CORE_C3_CONTINUATION_LINEAGE_UNVERIFIED")?{
            return Err("CORE_C3_NOT_AN_ORIGINAL_REVIEWED_CHILD".into())
        }
        let child_revision=revisions.latest(&next,&anchor.chat_id)
            .map_err(|_|"CORE_C3_CHILD_REVISION_UNREADABLE")?
            .ok_or("CORE_C3_CHILD_REVISION_MISSING")?;
        if child_revision.source_human_turn_id!=child_turn||
            !linear_independent_witness_suffix(
                &child_revision.steps,methods.len()-slot
            ){
            return Err("CORE_C3_CHILD_PLAN_NOT_THE_REVIEWED_SUFFIX".into())
        }
        if child_revision.review_state!="APPROVED_PLAN_ONLY"{
            return Ok(out) // A child DRAFT is never an approved receipt.
        }
        let child_view=scheduler.view(&next,&anchor.chat_id,&child_turn,ledger)
            .map_err(|_|"CORE_C3_CHILD_HOST_CUSTODY_UNREADABLE")?;
        let Some(child_view)=child_view else{return Ok(out)};
        if child_view.state!="RELEASED"{
            return Ok(out) // Interrupted/cancelled/running child is not done.
        }
        if child_view.revision_sha!=child_revision.revision_sha256||
            child_view.claims.len()!=1||
            child_view.claims[0].step_index!=0||
            child_view.claims[0].claim_state!="OBSERVED_VERIFIED_HISTORICAL"{
            return Err("CORE_C3_CHILD_RELEASE_NOT_INDEPENDENTLY_VERIFIED".into())
        }
        let child_history=receipts.history(
            &next,&anchor.chat_id,&child_turn,profile,ledger,revisions,sources
        ).map_err(|_|"CORE_C3_CHILD_ORIGINAL_WORKER_OR_ORACLE_INVALID")?;
        let selected:Vec<_>=child_history.into_iter().filter(|item|
            item.revision_sha256==child_revision.revision_sha256&&
            item.step_index==0&&
            item.status=="VERIFIED_AT_OBSERVATION"
        ).collect();
        if selected.is_empty(){return Ok(out)}
        if selected.len()!=1||
            (selected[0].tool_id.as_str(),
             selected[0].independent_method.as_str())!=methods[slot]{
            return Err("CORE_C3_CHILD_WITNESS_WRONG_CRITERION".into())
        }
        let item=&selected[0];
        out.push(TaskAgentEvidenceCandidate{
            // Reindexed child step zero is cryptographically traceable to
            // the original root task's next declared criterion slot.
            step_index:slot as u8,
            tool_id:item.tool_id.clone(),
            independent_method:item.independent_method.clone(),
            receipt_sha256:item.receipt_sha256.clone(),
            observed_sha256:item.observed_sha256.clone(),
            historical_verdict:item.status.into(),
            review_eligible:true,
            criterion_method_reviewable:exact_whole_task_slot(
                &anchor.acceptance_criteria,slot as u8,
                &item.tool_id,&item.independent_method
            ),
            original_requirement_id:anchor.requirement_id.clone(),
            task_completed:false,execution_authorized:false,
        });
        plan=next;
        revision=child_revision.revision_sha256;
        turn=child_turn;
    }
    Ok(out)
}

fn exact_whole_task_slot(
    criterion:&str,step_index:u8,tool:&str,method:&str
)->bool{
    super::project_task_verified_evidence::
        required_whole_task_independent_methods(criterion)
        .and_then(|methods|methods.get(usize::from(step_index)).copied())
        .is_some_and(|required|required==(tool,method))
}

pub(super) fn criterion_supports_method(
    criterion:&str,tool:&str,method:&str
)->bool{
    if !compatible_independent_method(tool,method)||
       criterion.len()>300{return false}
    if let Some(allowed)=super::project_task_verified_evidence::
        whole_criterion_supports_method(criterion,tool,method){
        return allowed
    }
    // A narrowly typed WHOLE-task equality predicate may be supported by
    // the ORIGINAL independent Git HEAD method. Its SHA must be a single
    // exact lowercase 40-hex literal; no model-supplied path or extra clause.
    if tool=="lu.reviewed.git.head"&&method=="GIT_HEAD_DIRECT_V1"{
        if let Some(head)=criterion.strip_prefix("LU_COMPLETE_V1:GIT_HEAD_EQUALS:"){
            return head.len()==40&&head.bytes().all(|b|
                b.is_ascii_digit()||matches!(b,b'a'..=b'f'))
        }
    }
    let Some((line,body))=criterion.split_once('\n') else {
        return false
    };
    let line=line.strip_suffix('\r').unwrap_or(line);
    if !line.is_ascii()||line.bytes().any(|b|b.is_ascii_control())||
       body.trim().len()<10||body.trim().len()>240||
       body.chars().any(|c|c.is_control()&&c!='\n'&&c!='\r'){
        return false
    }
    let Some(spec)=line.strip_prefix("LU_CRITERION_METHOD_V1:") else {
        return false
    };
    let mut parts=spec.split('|');
    parts.next()==Some(tool)&&parts.next()==Some(method)&&parts.next().is_none()
}

pub(super) fn independently_verified_task_witness(
    profile:&Path,register_id:&str,task_id:&str,handoff_id:&str,
    receipt_sha256:&str,chats:&chat_store::ChatStore
)->Result<ExactTaskWitness,String>{
    let handoffs=list(profile,register_id,task_id,chats)?;
    let link=handoffs.into_iter()
        .find(|h|h.handoff_id==handoff_id)
        .ok_or("P0_TOOLS05_HANDOFF_NOT_ORIGINAL")?;
    if !matches!(link.mode.as_str(),
        "reviewed_git_media_check"|"reviewed_whole_task")||
        link.phase!="STAGED"||
        !link.source_lease_current||link.plan_id.is_none()||
        link.revision_sha256.is_none(){
        return Err("P0_TOOLS05_HANDOFF_IS_STALE_OR_INERT".into())
    }
    let mut evidence=link.independently_audited_outcomes.into_iter()
        .filter(|e|e.receipt_sha256==receipt_sha256);
    let exact=evidence.next().ok_or("P0_TOOLS05_ORIGINAL_RECEIPT_NOT_FOUND")?;
    if evidence.next().is_some()||!exact.review_eligible||
        (link.mode=="reviewed_whole_task"&&!exact.criterion_method_reviewable)||
        exact.historical_verdict!="VERIFIED_AT_OBSERVATION"||
        !compatible_independent_method(&exact.tool_id,&exact.independent_method){
        return Err("P0_TOOLS05_RECEIPT_IS_NOT_INDEPENDENTLY_AUDITED".into())
    }
    Ok(ExactTaskWitness{
        handoff_id:link.handoff_id,tool_id:exact.tool_id,
        independent_method:exact.independent_method,
        receipt_sha256:exact.receipt_sha256,
        observed_sha256:exact.observed_sha256,
        step_index:exact.step_index,
    })
}

#[tauri::command]
pub(crate) async fn lu_project_task_agent_handoffs(
    caller:tauri::WebviewWindow,state:State<'_,Arc<DesktopState>>,
    register_id:String,task_id:String,
)->Result<Vec<TaskAgentHandoffView>,String>{
    if caller.label()!="task-list"{
        return Err("P0_NEXT_REQUIRES_NATIVE_TASK_LIST".into())
    }
    let host=Arc::clone(state.inner());
    let profile=project_register_desktop::profile(&host)?;
    tauri::async_runtime::spawn_blocking(move||
        list(&profile,&register_id,&task_id,&host.chats))
        .await.map_err(|_|"P0_NEXT_HANDOFF_HISTORY_INTERRUPTED")?
}

#[tauri::command]
pub(crate) async fn lu_project_task_prepare_agent_plan(
    app:AppHandle,caller:tauri::WebviewWindow,
    state:State<'_,Arc<DesktopState>>,
    register_id:String,task_id:String,
    expected_board_sha256:String,expected_register_sha256:String,
    expected_task_version:u64,mode:String,
)->Result<Option<TaskAgentHandoffView>,String>{
    if caller.label()!="task-list"{
        return Err("P0_NEXT_REQUIRES_NATIVE_TASK_LIST".into())
    }
    if !matches!(mode.as_str(),
        "review_only"|"reviewed_git_media_check"|"reviewed_whole_task"){
        return Err("P0_NEXT_UNRECOGNIZED_PLAN_MODE".into())
    }
    let host=Arc::clone(state.inner());
    let profile=project_register_desktop::profile(&host)?;
    if !lu_core::project_evidence_graph::valid_digest(&expected_board_sha256)
        ||!lu_core::project_evidence_graph::valid_digest(&expected_register_sha256)
        ||expected_task_version==0{
        return Err("P0_NEXT_INVALID_SOURCE_LEASE".into())
    }
    let preview={
        let conn=project_register_desktop::library(&profile)?;
        let (_,r,_)=project_register_desktop::load_register(&conn,&register_id)?
            .ok_or("P0_NEXT_SOURCE_REGISTER_MISSING")?;
        let board=project_task_board_desktop::read(&conn,&r)?;
        let task=register::latest(&r,&task_id).ok_or("P0_NEXT_TASK_NOT_FOUND")?;
        if board.sha256!=expected_board_sha256
            ||r.latest_sha256!=expected_register_sha256
            ||board.review_state!="human_approved"
            ||board.source_register_sha256!=r.latest_sha256
            ||task.version!=expected_task_version
            ||task.needs_human_reconciliation{
            return Err("P0_NEXT_STALE_TASK_REVIEW".into())
        }
        check_existing(&conn,&register_id,&task_id,expected_task_version)?;
        (task.summary.clone(),task.acceptance_criteria.clone(),
            task.source_requirement_id.clone(),task.blocked_by.clone(),
            task.description.clone())
    };
    let _=mode_steps(&mode,&preview.1)?;
    let confirmed=project_register_desktop::confirm(&app,
        "Lu — Prepare source-linked agent plan only",format!(
        "Create a new ANSF conversation and separately reviewed AGENT plan for:\n\
         Task: {}\nOriginal requirement: {}\nDescription: {}\nAcceptance criterion: {}\n\
         Outstanding blocker text: {}\nTask version: {}\n\
         Mode: {}\n\nThe original source must be selected AGAIN using \
         the operating-system folder picker, then re-scanned. \
         Source/requirements are context only, never instructions or permission. \
         This does NOT run Git, FFprobe, a shell or any worker; \
         does NOT accept implementation, and NEVER grants task authority.",
        preview.0,preview.2,preview.4,preview.1,preview.3,expected_task_version,mode
    )).await?;
    if !confirmed{return Ok(None)}
    let chooser=app.clone();
    let chosen=tauri::async_runtime::spawn_blocking(move||
        chooser.dialog().file().blocking_pick_folder()
    ).await.map_err(|_|"P0_NEXT_NATIVE_FOLDER_PICKER_INTERRUPTED")?;
    let Some(chosen)=chosen else{return Ok(None)};
    let root=chosen.into_path().map_err(|_|"P0_NEXT_NATIVE_PATH_INVALID")?;
    let events=app.clone();
    tauri::async_runtime::spawn_blocking(move||{
        let task=source_anchor(&profile,&root,&register_id,&task_id,
            &expected_board_sha256,&expected_register_sha256,expected_task_version)?;
        {
            let conn=project_register_desktop::library(&profile)?;
            check_existing(&conn,&register_id,&task_id,expected_task_version)?;
        }
        // Create a dedicated conversation: never overwrite a different chat's
        // active goal or treat its previous turn as human authorization.
        let new_chat=host.chats.create_chat()?;
        let turn=next_id("g-");
        let goal_event=next_id("m-");
        let handoff_id=next_id("h-");
        let goal_id=next_id("goal-");
        let user_text=format!(
            "Project task: {} (v{}). I confirmed preparation of a NEW \
             human-reviewed Lu agent plan for the selected task. \
             Original requirement: {}. Description: {}. Acceptance criterion: {}. \
             Reported blocker: {}. Project source is separately reselected by \
             the native operator and remains untrusted data. \
             No execution, task completion or tool permission is granted.",
            task.summary,task.version,task.source_requirement_id,
            task.description,task.acceptance_criteria,task.blocked_by
        );
        let goal_text=format!(
            "Plan and independently review task {} (v{}): {}. \
             Original requirement {}. Acceptance criterion: {}. \
             Source graph SHA256 {}. No execution authorization.",
            task.task_id,task.version,task.summary,task.source_requirement_id,
            task.acceptance_criteria,task.graph_sha256
        );
        let a=Anchor{
            schema:SCHEMA.into(),register_id:register_id.clone(),
            task_id:task_id.clone(),task_version:task.version,
            board_sha256:expected_board_sha256.clone(),
            register_sha256:expected_register_sha256.clone(),
            project_scope_sha256:{
                let conn=project_register_desktop::library(&profile)?;
                let (_,r,_)=project_register_desktop::load_register(&conn,&register_id)?
                    .ok_or("P0_NEXT_PROJECT_SCOPE_LOST")?;
                r.scope_sha256
            },
            requirement_id:task.source_requirement_id.clone(),
            graph_sha256:task.graph_sha256.clone(),
            status_sha256:task.status_sha256.clone(),
            selected_head:task.selected_head.clone(),mode:mode.clone(),
            summary:task.summary.clone(),description:task.description.clone(),
            acceptance_criteria:task.acceptance_criteria.clone(),
            blockers:task.blocked_by.clone(),
            chat_id:new_chat.chat_id.clone(),user_turn_id:turn.clone(),
            goal_event_id:goal_event.clone(),handoff_id,
            no_execution_authority:true
        };
        insert_prepared(&profile,&a)?;
        host.chats.append_user_with_turn_id(&a.chat_id,&turn,&user_text)?;
        host.chats.append_goal_memory_event(&a.chat_id,GoalEvent{
            event_id:goal_event,goal_id,question_id:None,
            kind:GoalEventKind::SetGoal,text:goal_text,choice:None,comment:None
        })?;
        let draft=stage_initial_draft(
            &profile,&host.chats,&a.chat_id,&turn,&mode_steps(&mode,&task.acceptance_criteria)?
        )?;
        // A newly created plan must have the same real scheduler and
        // independent-receipt schemas expected by the existing read-only
        // progress UI. Opening these stores issues NO claim or worker grant.
        let plan_file=agent_plan_review::existing_plan_file(&profile)?;
        AgentHostScheduler::open(&plan_file)
            .map_err(|_|"P0_NEXT_SCHEDULER_SCHEMA_NOT_AVAILABLE")?;
        AgentDeliverableReceipts::open(&plan_file)
            .map_err(|_|"P0_NEXT_DELIVERABLE_SCHEMA_NOT_AVAILABLE")?;
        publish(&profile,&a,&draft.plan_id,&draft.revision_sha256)?;
        let result=list(&profile,&register_id,&task_id,&host.chats)?;
        let found=result.into_iter().find(|v|v.handoff_id==a.handoff_id)
            .ok_or("P0_NEXT_NEW_HANDOFF_NOT_READABLE")?;
        // Presentation notification only. It cannot approve a plan or dispatch.
        // If the main chat was not open, the durable ANSF conversation remains.
        let _=events.emit("lu-project-agent-handoff-created",serde_json::json!({
            "chatId":found.chat_id.clone(),"planId":found.plan_id.clone(),
            "taskId":found.task_id.clone(),"registerId":found.register_id.clone(),
            "status":"review_only_draft","executionAuthorized":false
        }));
        Ok(Some(found))
    }).await.map_err(|_|"P0_NEXT_PREPARATION_INTERRUPTED")?
}
#[cfg(test)]
mod task_method_tests{
    use super::*;
    #[test]
    fn only_independent_original_git_media_methods_are_candidates(){
        assert!(compatible_independent_method(
            "lu.reviewed.git.head","GIT_HEAD_DIRECT_V1"));
        assert!(compatible_independent_method(
            "lu.reviewed.media.facts","MP4_BOXES_DIRECT_V1"));
        for (tool,method) in [
            ("lu.reviewed.git.head","GIT_HISTORY_OBJECT_DAG_V2"),
            ("lu.reviewed.git.status","GIT_STATUS_UNSUPPORTED_V1"),
            ("lu.reviewed.pdf.info","PDFINFO_CLASSIC_PAGE_TREE_V1"),
            ("lu.reviewed.image.info","EXIFTOOL_IMAGE_DIMENSIONS_V1"),
            ("lu.reviewed.media.facts","WAV_UNSUPPORTED_V1")
        ]{
            assert!(!compatible_independent_method(tool,method),
                "method not originally admitted for independent PLAN step");
        }
    }
    #[test]
    fn exact_method_gate_refuses_generic_or_substituted_source_evidence(){
        let criterion=concat!(
            "LU_CRITERION_METHOD_V1:lu.reviewed.git.head|GIT_HEAD_DIRECT_V1\n",
            "Observe the exact original source Git HEAD for later human review."
        );
        assert!(criterion_supports_method(criterion,
            "lu.reviewed.git.head","GIT_HEAD_DIRECT_V1"));
        for (tool,method) in [
            ("lu.reviewed.git.status","GIT_STATUS_INDEX_TREE_V1"),
            ("lu.reviewed.git.head","GIT_HISTORY_OBJECT_DAG_V2"),
            ("lu.reviewed.pdf.info","PDFINFO_CLASSIC_PAGE_TREE_V1"),
        ]{
            assert!(!criterion_supports_method(criterion,tool,method));
        }
        for bad in [
            "Verify the app works",
            "prefix LU_CRITERION_METHOD_V1:lu.reviewed.git.head|GIT_HEAD_DIRECT_V1\nObserve exact Git HEAD",
            "LU_CRITERION_METHOD_V1:lu.reviewed.git.head|GIT_HEAD_DIRECT_V1",
            "LU_CRITERION_METHOD_V1:lu.reviewed.git.head|GIT_HEAD_DIRECT_V1x\nObserve exact source Git HEAD",
            "LU_CRITERION_METHOD_V1:lu.reviewed.git.head|GIT_HEAD_DIRECT_V1\nshort",
        ]{
            assert!(!criterion_supports_method(
                bad,"lu.reviewed.git.head","GIT_HEAD_DIRECT_V1"));
        }
    }
    #[test]
    fn original_reviewed_git_media_families_are_individually_typed(){
        for (tool,method) in [
            ("lu.reviewed.git.status","GIT_STATUS_INDEX_TREE_V1"),
            ("lu.reviewed.git.branch","GIT_BRANCH_DIRECT_V1"),
            ("lu.reviewed.git.history","GIT_HISTORY_OBJECT_DAG_V2"),
            ("lu.reviewed.media.facts","WAV_PCM_DIRECT_V1"),
            ("lu.reviewed.media.facts","MP4_BOXES_DIRECT_V1"),
        ]{
            let criterion=format!(
                "LU_CRITERION_METHOD_V1:{tool}|{method}\nObserve an exact original source fact before approval."
            );
            assert!(criterion_supports_method(&criterion,tool,method));
        }
        assert!(!criterion_supports_method(
            "LU_CRITERION_METHOD_V1:lu.reviewed.pdf.info|PDFINFO_CLASSIC_PAGE_TREE_V1\nObserve original source PDF metadata.",
            "lu.reviewed.pdf.info","PDFINFO_CLASSIC_PAGE_TREE_V1"));
    }
    #[test]
    fn c3_declared_whole_task_stages_each_required_worker_as_separate_check(){
        let criterion=format!(
            "LU_COMPLETE_V2:ALL_OF\nCURRENT_NATIVE_REQUIREMENT\n\
             GIT_HEAD_EQUALS:{}\nGIT_STATUS_REVIEWED\nWAV_PCM_REVIEWED",
            "a".repeat(40)
        );
        let steps=mode_steps("reviewed_whole_task",&criterion).unwrap();
        assert_eq!(steps.len(),3);
        for (index,step) in steps.iter().enumerate(){
            assert_eq!(step.kind,PlanStepKind::IndependentlyCheck);
            assert_eq!(step.expected,ExpectedDeliverable::IndependentWitness);
            assert_eq!(step.depends_on,
                if index==0{None}else{Some((index-1) as u8)});
        }
        for bad in [
            "Implement all requirements",
            "LU_COMPLETE_V1:CURRENT_NATIVE_REQUIREMENT",
            "LU_COMPLETE_V2:ALL_OF\nCURRENT_NATIVE_REQUIREMENT\nGIT_STATUS_REVIEWED\nDEPLOY",
            "LU_COMPLETE_V2:ALL_OF\nCURRENT_NATIVE_REQUIREMENT\nGIT_STATUS_REVIEWED\nGIT_STATUS_REVIEWED",
        ]{
            assert!(mode_steps("reviewed_whole_task",bad).is_err(),
                "unsupported or incomplete criteria must not stage checks");
        }
        assert_eq!(mode_steps("reviewed_git_media_check",&criterion)
            .unwrap().len(),1);
    }
    #[test]
    fn c3_reviewed_continuation_must_preserve_all_unexecuted_witness_slots(){
        let make=|n:usize|RevisionStep{
            kind:PlanStepKind::IndependentlyCheck,
            depends_on:if n==0{None}else{Some((n-1) as u8)},
            deliverable_id:format!("d-{:032x}",n+1),
            expected:ExpectedDeliverable::IndependentWitness,
        };
        let steps=(0..3).map(make).collect::<Vec<_>>();
        assert!(linear_independent_witness_suffix(&steps,3));
        let child=(0..2).map(make).collect::<Vec<_>>();
        assert!(linear_independent_witness_suffix(&child,2));
        let grandchild=vec![make(0)];
        assert!(linear_independent_witness_suffix(&grandchild,1));
        assert!(!linear_independent_witness_suffix(&child,3),
            "missing one method is incomplete");
        assert!(!linear_independent_witness_suffix(&steps,2),
            "an extra old step cannot impersonate the child suffix");
        let mut wrong=child.clone();
        wrong[0].depends_on=Some(0);
        assert!(!linear_independent_witness_suffix(&wrong,2));
        let mut wrong=child.clone();
        wrong[1].kind=PlanStepKind::Present;
        assert!(!linear_independent_witness_suffix(&wrong,2));
        let mut wrong=child.clone();
        wrong[1].expected=ExpectedDeliverable::Explanation;
        assert!(!linear_independent_witness_suffix(&wrong,2));
        let mut wrong=child;
        wrong[0].deliverable_id.clear();
        assert!(!linear_independent_witness_suffix(&wrong,2));
        assert!(!linear_independent_witness_suffix(&[],0));
    }

    #[test]
    fn every_whole_task_receipt_slot_refuses_another_declared_method(){
        let criterion=format!(
            "LU_COMPLETE_V2:ALL_OF\nCURRENT_NATIVE_REQUIREMENT\n\
             GIT_HEAD_EQUALS:{}\nGIT_STATUS_REVIEWED\nWAV_PCM_REVIEWED\nMP4_BOXES_REVIEWED",
            "a".repeat(40)
        );
        let methods=[
            ("lu.reviewed.git.head","GIT_HEAD_DIRECT_V1"),
            ("lu.reviewed.git.status","GIT_STATUS_INDEX_TREE_V1"),
            ("lu.reviewed.media.facts","WAV_PCM_DIRECT_V1"),
            ("lu.reviewed.media.facts","MP4_BOXES_DIRECT_V1"),
        ];
        for (index,(tool,method)) in methods.iter().enumerate(){
            assert!(exact_whole_task_slot(&criterion,index as u8,tool,method));
            for (other_index,(other_tool,other_method)) in methods.iter().enumerate(){
                if index!=other_index{
                    // A method that belongs to the same ALL_OF manifest
                    // cannot satisfy a different step.
                    assert!(!exact_whole_task_slot(
                        &criterion,index as u8,other_tool,other_method));
                }
            }
        }
        assert!(!exact_whole_task_slot(&criterion,4,methods[0].0,methods[0].1));
        assert!(!exact_whole_task_slot(&criterion,31,methods[0].0,methods[0].1));
        assert!(!exact_whole_task_slot(
            "LU_COMPLETE_V2:ALL_OF\nCURRENT_NATIVE_REQUIREMENT\nGIT_STATUS_REVIEWED\nGIT_STATUS_REVIEWED",
            0,methods[1].0,methods[1].1));
        assert!(!exact_whole_task_slot(
            "Verify my project is complete",0,methods[0].0,methods[0].1));
    }

    #[test]
    fn every_v2_declared_method_requires_its_own_exact_original_worker(){
        let criterion=format!(
            "LU_COMPLETE_V2:ALL_OF\nCURRENT_NATIVE_REQUIREMENT\n\
             GIT_HEAD_EQUALS:{}\nGIT_STATUS_REVIEWED\nMP4_BOXES_REVIEWED",
            "a".repeat(40)
        );
        for (tool,method) in [
            ("lu.reviewed.git.head","GIT_HEAD_DIRECT_V1"),
            ("lu.reviewed.git.status","GIT_STATUS_INDEX_TREE_V1"),
            ("lu.reviewed.media.facts","MP4_BOXES_DIRECT_V1"),
        ]{
            assert!(criterion_supports_method(&criterion,tool,method));
        }
        for (tool,method) in [
            ("lu.reviewed.media.facts","WAV_PCM_DIRECT_V1"),
            ("lu.reviewed.git.branch","GIT_BRANCH_DIRECT_V1"),
            ("lu.reviewed.git.status","GIT_HEAD_DIRECT_V1"),
        ]{
            assert!(!criterion_supports_method(&criterion,tool,method));
        }
        assert!(!criterion_supports_method(
            &format!("{criterion}\nPUBLISH RELEASE"),
            "lu.reviewed.git.status","GIT_STATUS_INDEX_TREE_V1"
        ));
    }

    #[test]
    fn exact_git_head_whole_criterion_only_admits_original_head_oracle(){
        let full=format!("LU_COMPLETE_V1:GIT_HEAD_EQUALS:{}", "a".repeat(40));
        assert!(criterion_supports_method(&full,
            "lu.reviewed.git.head","GIT_HEAD_DIRECT_V1"));
        for (tool,method) in [
            ("lu.reviewed.git.status","GIT_STATUS_INDEX_TREE_V1"),
            ("lu.reviewed.git.head","GIT_HISTORY_OBJECT_DAG_V2"),
            ("lu.reviewed.media.facts","WAV_PCM_DIRECT_V1"),
        ]{
            assert!(!criterion_supports_method(&full,tool,method));
        }
        for bad in [
            format!("{full}\nAlso publish release"),
            format!("{full}; shell"),
            format!("{full} "),
            format!("LU_COMPLETE_V1:GIT_HEAD_EQUALS:{}", "A".repeat(40)),
            format!("LU_COMPLETE_V1:GIT_HEAD_EQUALS:{}", "a".repeat(39)),
            format!("LU_COMPLETE_V1:GIT_HEAD_EQUALS:{}", "a".repeat(64))
        ]{
            assert!(!criterion_supports_method(&bad,
                "lu.reviewed.git.head","GIT_HEAD_DIRECT_V1"));
        }
    }

}
