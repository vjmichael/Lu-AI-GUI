//! AGENT-01I: durable, non-executing fresh-human continuation intent.
//!
//! This module prepares a *new* review request after an independently verified
//! historical predecessor. It NEVER reopens a released scheduler owner, issues
//! ConversationAuthority, or launches a worker. A later reviewed child-plan
//! execution path needs its own fresh host authorization and OS custody gate.
use std::path::Path;
use rusqlite::{params,Connection,OptionalExtension,TransactionBehavior};
use crate::{
    agent_host_scheduler::{AgentHostScheduler,ScheduleBudget,HostFreshStepConsent},
    agent_plan_ledger::{AgentPlanLedger,PlanStepKind},
    agent_plan_revisions::{AgentPlanRevisionStore,ExpectedDeliverable,HostPlanReferenceVerifier},
    agent_deliverable_receipts::AgentDeliverableReceipts,
    conversation_authority::TrustedUserTurnVerifier,
};

#[derive(Clone,Debug,PartialEq,Eq)]
pub struct ContinuationIntent {
    pub plan_id:String,
    pub revision_sha256:String,
    pub step_index:u8,
    pub fresh_human_turn_id:String,
    /// No caller may interpret this as an executable permission.
    pub execution_authority:&'static str,
}
fn opaque_id(v:&str,prefix:&str)->bool {
    v.len()==prefix.len()+32 && v.starts_with(prefix)
        &&v[prefix.len()..].bytes().all(|b|b.is_ascii_hexdigit()&&!b.is_ascii_uppercase())
}
fn lowercase_sha(v:&str)->bool {
    v.len()==64 && v.bytes().all(|b|b.is_ascii_digit()||(b'a'..=b'f').contains(&b))
}
fn schema(db:&Connection)->Result<(),String>{
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS lu_agent_continuation_intents(
            plan_id TEXT NOT NULL REFERENCES lu_agent_plans(plan_id),
            revision_sha256 TEXT NOT NULL,
            step_index INTEGER NOT NULL CHECK(step_index BETWEEN 1 AND 31),
            fresh_human_turn_id TEXT NOT NULL,
            state TEXT NOT NULL CHECK(state='AWAITING_NEW_HOST_REVIEW'),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY(plan_id,revision_sha256,step_index),
            UNIQUE(plan_id,fresh_human_turn_id)
        );
        CREATE TRIGGER IF NOT EXISTS lu_agent_continuation_no_delete
          BEFORE DELETE ON lu_agent_continuation_intents
          BEGIN SELECT RAISE(ABORT,'Continuation intent cannot be erased'); END;
        CREATE TRIGGER IF NOT EXISTS lu_agent_continuation_no_update
          BEFORE UPDATE ON lu_agent_continuation_intents
          BEGIN SELECT RAISE(ABORT,'Continuation intent is immutable'); END;"
    ).map_err(|e|format!("Continuation schema rejected: {e}"))
}

/// Stage a human's fresh request against the ORIGINAL plan, while refusing
/// stale or uncertain predecessor work. Each cross-journal observation is
/// re-read before this SQLite write; it is not an atomic OS/source snapshot.
/// Thus the only result is a durable REVIEW intent, never a tool/worker grant.
#[allow(clippy::too_many_arguments)]
pub fn stage_fresh_continuation(
    file:&Path,profile:&Path,plan:&str,chat:&str,origin_turn:&str,
    revision_sha:&str,step:u8,fresh_turn:&str,budget:ScheduleBudget,
    ledger:&AgentPlanLedger,revisions:&AgentPlanRevisionStore,
    scheduler:&AgentHostScheduler,receipts:&AgentDeliverableReceipts,
    sources:&dyn HostPlanReferenceVerifier,
    human:&dyn TrustedUserTurnVerifier,consent:&dyn HostFreshStepConsent,
)->Result<ContinuationIntent,String>{
    if !opaque_id(plan,"p-")||!opaque_id(chat,"c-")
        ||!opaque_id(origin_turn,"g-")||!opaque_id(fresh_turn,"g-")
        ||!lowercase_sha(revision_sha)||step==0||step>31
        ||fresh_turn==origin_turn ||!budget.bounded()||budget.max_claims!=1 {
        return Err("Invalid original scope, fresh turn, step or bounded budget".into())
    }
    if !human.verify_user_turn(chat,fresh_turn)
        ||!consent.explicitly_requested_step(chat,fresh_turn,plan,revision_sha,step,&budget){
        return Err("A separate authenticated human step request is required".into())
    }
    let original=ledger.snapshot(plan,chat,origin_turn)
        .map_err(|e|format!("Cannot verify original plan: {e:?}"))?
        .ok_or("Original plan missing")?;
    if matches!(original.state.as_str(),"NEEDS_REVIEW"|"CANCEL_REQUESTED") {
        return Err("Original task was interrupted or needs a new review".into())
    }
    let revision=revisions.latest(plan,chat)
        .map_err(|e|format!("Cannot inspect revision: {e:?}"))?
        .ok_or("Immutable plan revision missing")?;
    if revision.revision_sha256!=revision_sha
        ||!sources.goal_event_is_current(chat,&revision.goal_id,&revision.goal_event_id)
        ||revision.assumption_sources.iter().any(|a|!sources.assumption_source_is_readable(chat,a))
        ||revision.review_state!="APPROVED_PLAN_ONLY"
        ||fresh_turn==revision.source_human_turn_id
        ||revision.review_turn_id.as_deref()==Some(fresh_turn) {
        return Err("Changed, unreviewed or non-fresh plan revision".into())
    }
    let selected=revision.steps.get(step as usize)
        .ok_or("Continuation step outside reviewed plan")?;
    if selected.kind!=PlanStepKind::IndependentlyCheck
        ||selected.expected!=ExpectedDeliverable::IndependentWitness {
        return Err("Unsupported reviewed operation for continuation".into())
    }
    let owner=scheduler.view(plan,chat,origin_turn,ledger)
        .map_err(|e|format!("Cannot inspect host fence: {e:?}"))?
        .ok_or("No previous supervised execution")?;
    if owner.state!="RELEASED"||owner.revision_sha!=revision_sha {
        return Err("Previous host is not cleanly released; no takeover".into())
    }
    if owner.claims.iter().any(|c|c.claim_state!="OBSERVED_VERIFIED_HISTORICAL") {
        return Err("Uncertain prior step blocks continuation".into())
    }
    // Require sequential continuation and a recorded predecessor. Do not
    // skip a step or accept a historical receipt from a different revision.
    let parent=step-1;
    if selected.depends_on!=Some(parent)
        ||owner.claims.len()!=step as usize
        ||owner.claims.iter().enumerate().any(|(i,c)|
            c.step_index as usize!=i ||c.revision_sha!=revision_sha) {
        return Err("Continuation dependency or sequence is unverified".into())
    }
    let history=receipts.history(
        plan,chat,origin_turn,profile,ledger,revisions,sources
    ).map_err(|e|format!("Independent evidence recheck failed: {e:?}"))?;
    if !(0..=parent).all(|index|history.iter().any(|row|
        row.revision_sha256==revision_sha &&row.step_index==index
        &&row.status=="VERIFIED_AT_OBSERVATION")){
        return Err("Predecessor has no current independent history witness".into())
    }
    let mut db=Connection::open(file).map_err(|e|e.to_string())?;
    db.pragma_update(None,"foreign_keys","ON").map_err(|e|e.to_string())?;
    db.pragma_update(None,"synchronous","FULL").map_err(|e|e.to_string())?;
    schema(&db)?;
    let tx=db.transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|e|e.to_string())?;
    // Recheck source identity and the immutable scheduler fence *inside* the
    // same SQLite write transaction. External oracle/ANSF may still change,
    // which is why this record never confers dispatch permission.
    let original_row:(String,String,String)=tx.query_row(
        "SELECT conversation_id,human_turn_id,state FROM lu_agent_plans WHERE plan_id=?1",
        params![plan],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))
    ).map_err(|e|e.to_string())?;
    if original_row.0!=chat||original_row.1!=origin_turn
        ||matches!(original_row.2.as_str(),"CANCEL_REQUESTED"|"NEEDS_REVIEW") {
        return Err("Original plan changed during continuation staging".into())
    }
    let owner_row:(String,String)=tx.query_row(
        "SELECT state,revision_sha256 FROM lu_agent_schedule_owners WHERE plan_id=?1",
        params![plan],|r|Ok((r.get(0)?,r.get(1)?))
    ).map_err(|e|e.to_string())?;
    if owner_row.0!="RELEASED"||owner_row.1!=revision_sha {
        return Err("Host fence changed during continuation staging".into())
    }
    let latest:String=tx.query_row(
        "SELECT revision_sha256 FROM lu_agent_plan_revisions WHERE plan_id=?1 ORDER BY revision_index DESC LIMIT 1",
        params![plan],|r|r.get(0)
    ).map_err(|e|e.to_string())?;
    if latest!=revision_sha {return Err("Plan revision changed during staging".into())}
    let verified:i64=tx.query_row(
        "SELECT COUNT(*) FROM lu_agent_schedule_proposals WHERE plan_id=?1 AND revision_sha256=?2 AND step_index<?3 AND state='OBSERVED_VERIFIED_HISTORICAL'",
        params![plan,revision_sha,step],|r|r.get(0)
    ).map_err(|e|e.to_string())?;
    if verified!=i64::from(step) {return Err("Historical sequence changed".into())}
    let existing:Option<String>=tx.query_row(
        "SELECT fresh_human_turn_id FROM lu_agent_continuation_intents
         WHERE plan_id=?1 AND revision_sha256=?2 AND step_index=?3",
        params![plan,revision_sha,step],|r|r.get(0)
    ).optional().map_err(|e|e.to_string())?;
    if let Some(old)=existing {
        if old!=fresh_turn {return Err("Another human turn already staged this step".into())}
    }else{
        tx.execute(
            "INSERT INTO lu_agent_continuation_intents
             (plan_id,revision_sha256,step_index,fresh_human_turn_id,state)
             VALUES(?1,?2,?3,?4,'AWAITING_NEW_HOST_REVIEW')",
            params![plan,revision_sha,step,fresh_turn]
        ).map_err(|e|format!("Continuation conflict or journal failure: {e}"))?;
    }
    tx.commit().map_err(|e|e.to_string())?;
    Ok(ContinuationIntent {
        plan_id:plan.to_owned(),revision_sha256:revision_sha.to_owned(),
        step_index:step,fresh_human_turn_id:fresh_turn.to_owned(),
        execution_authority:"NOT_AUTHORIZED_FOR_EXECUTION",
    })
}


/// Link the staged original intent to an independently registered CHILD plan.
/// The child has its own plan identity, human origin, revision, later human
/// review and fresh scheduler owner. This link is immutable and is NOT an
/// execution permit. Failure between the two durable writes is inert and
/// requires manual recovery; no operation is automatically replayed.
pub fn bind_child_plan(
    file:&Path,parent:&str,revision:&str,step:u8,
    human_turn:&str,child:&str,
)->Result<(),String>{
    if !opaque_id(parent,"p-")||!opaque_id(child,"p-")
        ||parent==child||!lowercase_sha(revision)
        ||!opaque_id(human_turn,"g-")||step==0||step>31 {
        return Err("Invalid immutable child lineage identity".into())
    }
    let mut db=Connection::open(file).map_err(|e|e.to_string())?;
    db.pragma_update(None,"foreign_keys","ON").map_err(|e|e.to_string())?;
    db.pragma_update(None,"synchronous","FULL").map_err(|e|e.to_string())?;
    schema(&db)?;
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS lu_agent_continuation_children(
            parent_plan_id TEXT NOT NULL REFERENCES lu_agent_plans(plan_id),
            revision_sha256 TEXT NOT NULL,
            step_index INTEGER NOT NULL CHECK(step_index BETWEEN 1 AND 31),
            fresh_human_turn_id TEXT NOT NULL,
            child_plan_id TEXT NOT NULL UNIQUE REFERENCES lu_agent_plans(plan_id),
            PRIMARY KEY(parent_plan_id,revision_sha256,step_index),
            UNIQUE(parent_plan_id,fresh_human_turn_id)
        );
        CREATE TRIGGER IF NOT EXISTS lu_agent_continuation_child_no_delete
          BEFORE DELETE ON lu_agent_continuation_children
          BEGIN SELECT RAISE(ABORT,'Immutable child plan link'); END;
        CREATE TRIGGER IF NOT EXISTS lu_agent_continuation_child_no_update
          BEFORE UPDATE ON lu_agent_continuation_children
          BEGIN SELECT RAISE(ABORT,'Immutable child plan link'); END;"
    ).map_err(|e|e.to_string())?;
    let tx=db.transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|e|e.to_string())?;
    let staged:Option<String>=tx.query_row(
        "SELECT fresh_human_turn_id FROM lu_agent_continuation_intents
          WHERE plan_id=?1 AND revision_sha256=?2 AND step_index=?3",
        params![parent,revision,step],|r|r.get(0)
    ).optional().map_err(|e|e.to_string())?;
    if staged.as_deref()!=Some(human_turn){
        return Err("No exact fresh human continuation intent".into())
    }
    let source:Option<(String,String)>=tx.query_row(
        "SELECT conversation_id,human_turn_id FROM lu_agent_plans
          WHERE plan_id=?1",
        params![parent],|r|Ok((r.get(0)?,r.get(1)?))
    ).optional().map_err(|e|e.to_string())?;
    let child_origin:Option<(String,String)>=tx.query_row(
        "SELECT conversation_id,human_turn_id FROM lu_agent_plans
          WHERE plan_id=?1",
        params![child],|r|Ok((r.get(0)?,r.get(1)?))
    ).optional().map_err(|e|e.to_string())?;
    let Some((chat,_))=source else{return Err("Original parent plan missing".into())};
    if child_origin.as_ref()!=Some(&(chat,human_turn.to_owned())){
        return Err("Child is not a new plan from the exact fresh human turn".into())
    }
    let existing:Option<(String,String)>=tx.query_row(
        "SELECT child_plan_id,fresh_human_turn_id
          FROM lu_agent_continuation_children
          WHERE parent_plan_id=?1 AND revision_sha256=?2 AND step_index=?3",
        params![parent,revision,step],|r|Ok((r.get(0)?,r.get(1)?))
    ).optional().map_err(|e|e.to_string())?;
    if let Some((old_child,old_human))=existing{
        if old_child!=child||old_human!=human_turn{
            return Err("Another child already reserved for this step".into())
        }
    }else{
        tx.execute(
            "INSERT INTO lu_agent_continuation_children(
              parent_plan_id,revision_sha256,step_index,fresh_human_turn_id,child_plan_id
             ) VALUES(?1,?2,?3,?4,?5)",
            params![parent,revision,step,human_turn,child]
        ).map_err(|e|format!("Child plan lineage conflict: {e}"))?;
    }
    tx.commit().map_err(|e|e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scope_and_sha_are_strict_and_lowercase(){
        assert!(opaque_id(&format!("g-{}","a".repeat(32)),"g-"));
        assert!(!opaque_id(&format!("g-{}","A".repeat(32)),"g-"));
        assert!(!opaque_id(&format!("p-{}","a".repeat(32)),"g-"));
        assert!(lowercase_sha(&"b".repeat(64)));
        assert!(!lowercase_sha(&"B".repeat(64)));
    }
    #[test]
    fn durable_intent_is_immutable_and_not_execution_authority(){
        let db=Connection::open_in_memory().unwrap();
        db.execute_batch("PRAGMA foreign_keys=OFF; CREATE TABLE lu_agent_plans(plan_id TEXT PRIMARY KEY);").unwrap();
        schema(&db).unwrap();
        let p=format!("p-{}","a".repeat(32));
        let r="b".repeat(64);
        let t=format!("g-{}","c".repeat(32));
        db.execute("INSERT INTO lu_agent_plans(plan_id) VALUES(?1)",params![p]).unwrap();
        db.execute("INSERT INTO lu_agent_continuation_intents
            (plan_id,revision_sha256,step_index,fresh_human_turn_id,state)
            VALUES(?1,?2,1,?3,'AWAITING_NEW_HOST_REVIEW')",params![p,r,t]).unwrap();
        assert!(db.execute("UPDATE lu_agent_continuation_intents SET state='AWAITING_NEW_HOST_REVIEW'",[]).is_err());
        assert!(db.execute("DELETE FROM lu_agent_continuation_intents",[]).is_err());
        assert!(db.execute("INSERT INTO lu_agent_continuation_intents
            (plan_id,revision_sha256,step_index,fresh_human_turn_id,state)
            VALUES(?1,?2,2,?3,'AWAITING_NEW_HOST_REVIEW')",params![p,r,t]).is_err());
    }
}
