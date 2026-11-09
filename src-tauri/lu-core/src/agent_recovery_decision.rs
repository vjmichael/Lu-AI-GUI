//! AGENT-01K: independent, read-only reconstruction of recovery state.
//!
//! Reviewable evidence is never the same thing as a usable tool grant.
//! The original ANSF goal, immutable plan revision, scheduler claim, actual
//! worker task/effect SQLite and independent oracle are all re-opened. A
//! historical APPLIED receipt does not attest OS descendant termination.
//! This module does not create an execution owner, issue a permission,
//! repair ambiguous effects, retry a process, or resume the loaded model.
use std::{collections::BTreeSet,path::Path};
use rusqlite::{params,Connection,OpenFlags,OptionalExtension};
use crate::{
    agent_plan_ledger::AgentPlanLedger,
    agent_plan_revisions::{AgentPlanRevisionStore,HostPlanReferenceVerifier},
    agent_host_scheduler::{AgentHostScheduler,SchedulerView},
    agent_deliverable_receipts::{AgentDeliverableReceipts,HistoricalDeliverable},
    agent_lineage_recovery::inspect_continuation_lineage,
    agent_continuation_intent::reverify_child_before_execution,
};

const MAX_GENERATIONS:usize=32;

/// Only a trusted host OS custodian may implement this; never construct a
/// true witness from a model, front-end, historical SQLite APPLIED or timer.
/// This interface deliberately carries original human/worker identities.
pub trait IndependentProcessCustody:Send+Sync{
    fn worker_and_descendants_ended(
        &self,chat:&str,execution_turn:&str,effect_call:&str
    )->bool;
}
/// Installed production fallback until a real Windows Job Object / Linux
/// process or cgroup guardian provides independently checked custody proof.
pub struct NoIndependentCustodyProof;
impl IndependentProcessCustody for NoIndependentCustodyProof{
    fn worker_and_descendants_ended(
        &self,_:&str,_:&str,_:&str
    )->bool{false}
}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct ReconciledStep{
    pub step_index:u8,
    pub human_turn_id:String,
    pub evidence_status:&'static str,
    pub descendant_custody:&'static str,
}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct RecoveryDecision{
    pub status:&'static str,
    pub next_action:&'static str,
    pub linked_generations:u8,
    pub independently_observed_steps:u8,
    pub steps:Vec<ReconciledStep>,
    pub execution_authority:&'static str,
}
fn decision(
    status:&'static str,next_action:&'static str,n:usize,steps:Vec<ReconciledStep>
)->RecoveryDecision{
    RecoveryDecision{
        status,next_action,linked_generations:n.min(MAX_GENERATIONS) as u8,
        independently_observed_steps:steps.iter()
            .filter(|s|s.evidence_status=="VERIFIED_AT_OBSERVATION").count()
            .min(32) as u8,
        steps,execution_authority:"NOT_AUTHORIZED_FOR_EXECUTION",
    }
}
fn deny(status:&'static str,action:&'static str,n:usize)->RecoveryDecision{
    decision(status,action,n,Vec::new())
}
const INSPECT:&str="Inspect original task, effect, oracle and host process custody. Do not retry automatically.";
const REAUTHORIZE:&str="Ask the human to review a fresh exact step, then independently authorize its native operation. Prior permissions cannot be replayed.";

/// Exact historical claims are matched with independently observed actual
/// task/effect/oracle tuples. A released scheduler or a prior model answer
/// alone never makes a step completed, a process ended, or a new tool allowed.
fn classify(
    owner:Option<&SchedulerView>,revision:&str,history:&[HistoricalDeliverable],
    chat:&str,custody:&dyn IndependentProcessCustody,generations:usize,
)->RecoveryDecision{
    let Some(owner)=owner else{
        if !history.is_empty(){
            return deny("STOP_WITNESS_WITHOUT_OWNER",INSPECT,generations)
        }
        return deny("REVIEW_FIRST_STEP_ONLY",REAUTHORIZE,generations)
    };
    if owner.revision_sha!=revision||owner.epoch!=1{
        return deny("STOP_STALE_SCHEDULER_REVISION",INSPECT,generations)
    }
    if owner.state!="RELEASED"{
        return deny(match owner.state.as_str(){
            "OWNED"=>"STOP_HOST_STILL_OWNED_OR_UNKNOWN",
            "CANCEL_REQUESTED"=>"STOP_CANCEL_UNCERTAIN",
            "RECOVERY_REQUIRED"=>"STOP_ABANDONED_HOST_CUSTODY",
            _=>"STOP_UNKNOWN_OWNER_STATE",
        },INSPECT,generations)
    }
    if owner.claims.is_empty()||owner.claims.len()!=history.len()
        ||owner.requested_claims as usize!=owner.claims.len(){
        return deny("STOP_MISSING_OR_EXTRA_WORKER_RECEIPTS",INSPECT,generations)
    }
    let mut seen_calls=BTreeSet::new();
    let mut seen_turns=BTreeSet::new();
    let mut steps=Vec::new();
    for claim in &owner.claims{
        if claim.revision_sha!=revision
            ||claim.claim_state!="OBSERVED_VERIFIED_HISTORICAL"
            ||claim.plan_id.is_empty()
            ||!seen_turns.insert(claim.original_request_turn.clone()){
            return deny("STOP_UNCERTAIN_OR_DUPLICATE_STEP",INSPECT,generations)
        }
        let matching:Vec<&HistoricalDeliverable>=history.iter().filter(|row|
            row.revision_sha256==revision &&row.step_index==claim.step_index
        ).collect();
        if matching.len()!=1{
            return deny("STOP_CONFLICTING_INDEPENDENT_WITNESS",INSPECT,generations)
        }
        let row=matching[0];
        if row.status!="VERIFIED_AT_OBSERVATION"
            ||row.execution_turn_id!=claim.original_request_turn
            ||!seen_calls.insert(row.effect_call_id.clone()){
            return deny("STOP_UNVERIFIED_WORKER_ORACLE",INSPECT,generations)
        }
        let stopped=custody.worker_and_descendants_ended(
            chat,&row.execution_turn_id,&row.effect_call_id
        );
        steps.push(ReconciledStep{
            step_index:claim.step_index,
            human_turn_id:row.execution_turn_id.clone(),
            evidence_status:"VERIFIED_AT_OBSERVATION",
            descendant_custody:if stopped{"INDEPENDENTLY_ATTESTED"}else{"NOT_PROVEN"},
        });
    }
    if steps.iter().any(|s|s.descendant_custody=="NOT_PROVEN"){
        return decision("STOP_DESCENDANT_CUSTODY_UNPROVEN",
            "Historical worker output was independently rechecked, but full descendant termination has not been independently established. Review OS custody before new work.",
            generations,steps)
    }
    decision("REVIEW_NEW_STEP_WITH_FRESH_AUTHORIZATION_ONLY",
        REAUTHORIZE,generations,steps)
}
/// A lineage table is not a current oracle. Reopen every ancestor's original
/// independent evidence: a second-generation child must not overlook a
/// stale grandparent simply because its immediate parent was once verified.
#[allow(clippy::too_many_arguments)]
fn recheck_ancestors(
    file:&Path,profile:&Path,plan:&str,chat:&str,
    ledger:&AgentPlanLedger,revisions:&AgentPlanRevisionStore,
    scheduler:&AgentHostScheduler,receipts:&AgentDeliverableReceipts,
    sources:&dyn HostPlanReferenceVerifier,
)->Result<(),String>{
    let db=Connection::open_with_flags(file,
        OpenFlags::SQLITE_OPEN_READ_ONLY|OpenFlags::SQLITE_OPEN_NO_MUTEX
    ).map_err(|e|format!("Cannot read ancestor lineage: {e}"))?;
    let has_link:Option<String>=db.query_row(
        "SELECT name FROM sqlite_master WHERE type='table'
         AND name='lu_agent_continuation_children'",
        [],|r|r.get(0)
    ).optional().map_err(|e|e.to_string())?;
    if has_link.is_none(){return Ok(())}
    let mut child=plan.to_owned();
    let mut seen=BTreeSet::new();
    for _ in 0..MAX_GENERATIONS{
        if !seen.insert(child.clone()){
            return Err("Cycle in independently checked continuation ancestry".into())
        }
        let parent:Option<String>=db.query_row(
            "SELECT parent_plan_id FROM lu_agent_continuation_children
             WHERE child_plan_id=?1",params![child],|r|r.get(0)
        ).optional().map_err(|e|e.to_string())?;
        let Some(parent)=parent else{return Ok(())};
        match reverify_child_before_execution(
            file,profile,&child,chat,ledger,revisions,scheduler,receipts,sources
        ){
            Ok(true)=>{},
            _=>return Err("Live parent source, claim or independent oracle unverified".into())
        }
        child=parent;
    }
    Err("Continuation ancestry exceeds bounded independent recheck".into())
}

/// Evaluate a single authenticated original plan WITHOUT any new authority.
/// Callers provide existing, previously opened evidence stores and a genuine
/// host selected profile (never a model/WebView supplied path).
#[allow(clippy::too_many_arguments)]
pub fn reconcile_recovery(
    file:&Path,profile:&Path,chat:&str,plan:&str,original_turn:&str,
    ledger:&AgentPlanLedger,revisions:&AgentPlanRevisionStore,
    scheduler:&AgentHostScheduler,receipts:&AgentDeliverableReceipts,
    sources:&dyn HostPlanReferenceVerifier,
    custody:&dyn IndependentProcessCustody,
)->Result<RecoveryDecision,String>{
    let ancestry=inspect_continuation_lineage(file,chat,plan)?;
    let generations=ancestry.generations as usize;
    if ancestry.status.starts_with("STOP_"){
        return Ok(deny("STOP_LINEAGE_RECONCILIATION",INSPECT,generations))
    }
    let original=ledger.snapshot(plan,chat,original_turn)
        .map_err(|e|format!("Original human plan unavailable: {e:?}"))?
        .ok_or("Original human plan missing")?;
    if matches!(original.state.as_str(),"NEEDS_REVIEW"|"CANCEL_REQUESTED"){
        return Ok(deny("STOP_INTERRUPTED_OR_CANCELLED_PLAN",INSPECT,generations))
    }
    let current=revisions.latest(plan,chat)
        .map_err(|e|format!("Immutable revision unavailable: {e:?}"))?
        .ok_or("No current immutable revision")?;
    if current.review_state!="APPROVED_PLAN_ONLY"{
        return Ok(deny("STOP_PLAN_REVIEW_REQUIRED",REAUTHORIZE,generations))
    }
    if !sources.goal_event_is_current(chat,&current.goal_id,&current.goal_event_id)
        ||current.assumption_sources.iter().any(|s|
            !sources.assumption_source_is_readable(chat,s)){
        return Ok(deny("STOP_GOAL_OR_SOURCE_CHANGED",INSPECT,generations))
    }
    if recheck_ancestors(
        file,profile,plan,chat,ledger,revisions,scheduler,receipts,sources
    ).is_err(){
        return Ok(deny("STOP_ANCESTOR_ORACLE_UNVERIFIED",INSPECT,generations))
    }
    let owner=scheduler.view(plan,chat,original_turn,ledger)
        .map_err(|e|format!("Scheduler cannot be independently observed: {e:?}"))?;
    // Do NOT skip receipt history when the owner is missing: an orphaned
    // historical result might exist without a matching current fence.
    let history=receipts.history(
        plan,chat,original_turn,profile,ledger,revisions,sources
    ).map_err(|e|format!("Native task/effect oracle inspection failed: {e:?}"))?;
    if history.len()>32{
        return Err("Unbounded historical worker receipts".into())
    }
    Ok(classify(owner.as_ref(),&current.revision_sha256,&history,
        chat,custody,generations))
}

#[cfg(test)]
mod tests{
    use super::*;
    use crate::agent_host_scheduler::{ScheduledProposal,ScheduleBudget};
    fn turn(n:u8)->String{format!("g-{n:032x}")}
    fn sha()->String{"a".repeat(64)}
    fn sample()->(SchedulerView,Vec<HistoricalDeliverable>){
        let rev=sha();
        let fresh=turn(9);
        let budget=ScheduleBudget{max_claims:1,model_token_ceiling:1024,
            tool_call_ceiling:1,wall_clock_ceiling_ms:1000,
            output_ceiling_bytes:4096};
        let owner=SchedulerView{
            state:"RELEASED".into(),epoch:1,revision_sha:rev.clone(),
            requested_claims:1,claims:vec![ScheduledProposal{
                plan_id:format!("p-{:032x}",2),revision_sha:rev.clone(),
                step_index:0,claim_id:format!("q-{:032x}",3),
                original_request_turn:fresh.clone(),
                claim_state:"OBSERVED_VERIFIED_HISTORICAL".into(),epoch:1,
                budget,
            }],execution:"NOT_AUTHORIZED_FOR_EXECUTION"
        };
        let history=vec![HistoricalDeliverable{
            revision_sha256:rev,execution_turn_id:fresh,
            step_index:0,deliverable_id:format!("d-{:032x}",4),
            tool_id:"lu.reviewed.git.head".into(),
            effect_call_id:"independent-0-gh".into(),
            receipt_sha256:sha(),independent_method:"GIT_HEAD_DIRECT_V1".into(),
            observed_sha256:sha(),status:"VERIFIED_AT_OBSERVATION",
            continuation:"NOT_AUTHORIZED_FOR_EXECUTION",
        }];
        (owner,history)
    }
    struct Stopped(bool);
    impl IndependentProcessCustody for Stopped{
        fn worker_and_descendants_ended(
            &self,_:&str,_:&str,_:&str
        )->bool{self.0}
    }
    #[test]
    fn untrusted_os_exit_never_allows_a_new_step(){
        let (owner,history)=sample();
        let report=classify(Some(&owner),&sha(),&history,&turn(1),
            &NoIndependentCustodyProof,0);
        assert_eq!(report.status,"STOP_DESCENDANT_CUSTODY_UNPROVEN");
        assert_eq!(report.independently_observed_steps,1);
        assert_eq!(report.execution_authority,"NOT_AUTHORIZED_FOR_EXECUTION");
        assert_eq!(report.steps[0].descendant_custody,"NOT_PROVEN");
    }
    #[test]
    fn independent_custody_still_requires_new_human_authorization(){
        let (owner,history)=sample();
        let report=classify(Some(&owner),&sha(),&history,&turn(1),
            &Stopped(true),1);
        assert_eq!(report.status,"REVIEW_NEW_STEP_WITH_FRESH_AUTHORIZATION_ONLY");
        assert_eq!(report.execution_authority,"NOT_AUTHORIZED_FOR_EXECUTION");
        assert_eq!(report.linked_generations,1);
    }
    #[test]
    fn missing_or_rebound_original_effect_never_becomes_success(){
        let (owner,mut history)=sample();
        history[0].execution_turn_id=turn(10);
        assert_eq!(classify(Some(&owner),&sha(),&history,&turn(1),
            &Stopped(true),0).status,"STOP_UNVERIFIED_WORKER_ORACLE");
        history[0].execution_turn_id=turn(9);
        history[0].status="NEEDS_RECONCILIATION";
        assert_eq!(classify(Some(&owner),&sha(),&history,&turn(1),
            &Stopped(true),0).status,"STOP_UNVERIFIED_WORKER_ORACLE");
        history.clear();
        assert_eq!(classify(Some(&owner),&sha(),&history,&turn(1),
            &Stopped(true),0).status,"STOP_MISSING_OR_EXTRA_WORKER_RECEIPTS");
    }
    #[test]
    fn old_owner_cancel_and_unknown_host_are_all_fenced(){
        let (mut owner,history)=sample();
        for (state,expected) in [
            ("OWNED","STOP_HOST_STILL_OWNED_OR_UNKNOWN"),
            ("CANCEL_REQUESTED","STOP_CANCEL_UNCERTAIN"),
            ("RECOVERY_REQUIRED","STOP_ABANDONED_HOST_CUSTODY")
        ]{
            owner.state=state.into();
            assert_eq!(classify(Some(&owner),&sha(),&history,&turn(1),
                &Stopped(true),0).status,expected);
        }
        owner.state="RELEASED".into();
        assert_eq!(classify(None,&sha(),&history,&turn(1),
            &Stopped(true),0).status,"STOP_WITNESS_WITHOUT_OWNER");
        assert_eq!(classify(Some(&owner),&"b".repeat(64),&history,&turn(1),
            &Stopped(true),0).status,"STOP_STALE_SCHEDULER_REVISION");
    }
}
