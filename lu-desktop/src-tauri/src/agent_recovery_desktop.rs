//! AGENT-01K — explicit conversation-visible, non-mutating recovery review.
//!
//! Native host selects the profile and authenticates the real chat. The
//! browser supplies only opaque plan/chat selectors, never a filesystem path,
//! custody success, approval boolean, process handle or replay permission.
use super::*;
use serde::Serialize;
use lu_core::{
    agent_plan_ledger::AgentPlanLedger,
    agent_plan_revisions::AgentPlanRevisionStore,
    agent_host_scheduler::AgentHostScheduler,
    agent_deliverable_receipts::AgentDeliverableReceipts,
    agent_recovery_decision::reconcile_recovery,
    agent_os_custody::HostOriginalJobCustody,
};

#[derive(Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct AgentRecoveryStepView{
    step_index:u8,
    evidence_status:String,
    descendant_custody:String,
}
#[derive(Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct AgentRecoveryView{
    status:String,
    next_action:String,
    linked_generations:u8,
    independently_observed_steps:u8,
    steps:Vec<AgentRecoveryStepView>,
    execution_authority:String,
}
/// READ ONLY. This is not a "resume", "repair" or "retry" command.
/// A historical worker may have completed, but positive descendant custody
/// requires the original verified Win32 Job or genuinely delegated Linux
/// cgroup-v2 helper receipt. Neither source candidate is an accepted native
/// installation guarantee until its exact OS gates pass.
#[tauri::command]
pub(crate) async fn lu_agent_reconcile_recovery(
    state:State<'_,Arc<DesktopState>>,
    chat_id:String,plan_id:String,
)->Result<AgentRecoveryView,String>{
    let host=Arc::clone(state.inner());
    host.chats.history_window(&chat_id,None,"older",1)?;
    tauri::async_runtime::spawn_blocking(move||{
        let profile=host.catalog_path.parent()
            .ok_or("Host profile unavailable")?;
        let plan_file=crate::agent_plan_review::existing_plan_file(profile)?;
        let current=crate::agent_plan_progress::read_chat_progress(
            profile,&chat_id,&host.chats
        )?.into_iter().find(|p|p.plan_id==plan_id)
            .ok_or("Plan does not belong to this authenticated conversation")?;
        // An active or interrupted owner may still have OS descendants.
        // The recovery decision is a bounded independent observation only.
        let ledger=AgentPlanLedger::open_existing_readonly(&plan_file)
            .map_err(|e|format!("Original plan read refused: {e:?}"))?;
        let revisions=AgentPlanRevisionStore::open_existing_readonly(&plan_file)
            .map_err(|e|format!("Immutable revision read refused: {e:?}"))?;
        let scheduler=AgentHostScheduler::open_existing_readonly(&plan_file)
            .map_err(|e|format!("Host owner read refused: {e:?}"))?;
        let receipts=AgentDeliverableReceipts::open_existing_readonly(&plan_file)
            .map_err(|e|format!("Independent evidence read refused: {e:?}"))?;
        let sources=crate::agent_plan_review::OriginalChatGoalSources{
            chats:&host.chats
        };
        let checked=reconcile_recovery(
            &plan_file,profile,&chat_id,&plan_id,
            &current.original_human_turn_id,
            &ledger,&revisions,&scheduler,&receipts,&sources,
            &HostOriginalJobCustody::from_host_profile(profile)
        )?;
        Ok(AgentRecoveryView{
            status:checked.status.into(),
            next_action:checked.next_action.into(),
            linked_generations:checked.linked_generations,
            independently_observed_steps:checked.independently_observed_steps,
            steps:checked.steps.into_iter().map(|s|AgentRecoveryStepView{
                step_index:s.step_index,evidence_status:s.evidence_status.into(),
                descendant_custody:s.descendant_custody.into()
            }).collect(),
            execution_authority:checked.execution_authority.into(),
        })
    }).await.map_err(|_|"Native recovery inspection failed")?
}


/// Native admission guard: requires the PREVIOUS reviewed owner to have
/// independently checked tool evidence AND a bound kernel OS custody witness.
/// An advisory conversation history entry, SQLite APPLIED or a model claim
/// cannot satisfy this guard; it cannot grant a new tool by itself.
pub(crate) fn require_parent_os_custody_for_fresh_plan(
    host:&DesktopState,chat:&str,parent_plan:&str,original_turn:&str,
)->Result<(),String>{
    let profile=host.catalog_path.parent()
        .ok_or("Host profile unavailable")?;
    let file=crate::agent_plan_review::existing_plan_file(profile)?;
    let ledger=AgentPlanLedger::open_existing_readonly(&file)
        .map_err(|e|format!("Parent ledger unavailable: {e:?}"))?;
    let revisions=AgentPlanRevisionStore::open_existing_readonly(&file)
        .map_err(|e|format!("Parent revision unavailable: {e:?}"))?;
    let scheduler=AgentHostScheduler::open_existing_readonly(&file)
        .map_err(|e|format!("Parent scheduler unavailable: {e:?}"))?;
    let receipts=AgentDeliverableReceipts::open_existing_readonly(&file)
        .map_err(|e|format!("Parent native oracle unavailable: {e:?}"))?;
    let sources=crate::agent_plan_review::OriginalChatGoalSources{
        chats:&host.chats
    };
    let report=reconcile_recovery(
        &file,profile,chat,parent_plan,original_turn,
        &ledger,&revisions,&scheduler,&receipts,&sources,
        &HostOriginalJobCustody::from_host_profile(profile)
    )?;
    if report.status!="REVIEW_NEW_STEP_WITH_FRESH_AUTHORIZATION_ONLY"{
        return Err(format!(
            "Previous worker custody or independent result is not proven ({}).              Resolve the original native journal and OS guardian before new work.",
            report.status
        ))
    }
    Ok(())
}

/// At the ACTUAL new child worker admission, an intact ancestry link alone
/// is not enough. Reopen every ancestor's exact original OS custody, current
/// ANSF and independent outcomes and require the child to be unclaimed.
/// A root plan has no inherited permission and uses ordinary native review.
pub(crate) fn require_fresh_child_os_custody_before_worker(
    host:&DesktopState,chat:&str,plan:&str,original_turn:&str,
)->Result<(),String>{
    let profile=host.catalog_path.parent()
        .ok_or("Host profile unavailable")?;
    let file=crate::agent_plan_review::existing_plan_file(profile)?;
    // Phase 2: every separately prepared replacement in a replan chain
    // requires its ORIGINAL parent's independently verified OS custody.
    // A broken stage->child transaction, stale source revision or uncertain
    // ancestor is not a new root and never grants a fresh worker.
    let mut replan_cursor=plan.to_owned();
    let mut seen=std::collections::BTreeSet::new();
    let mut completed_chain=false;
    for _ in 0..32{
        if !seen.insert(replan_cursor.clone()){
            return Err("Replan lineage cycle or reused parent".into())
        }
        let parent=lu_core::agent_replan_lineage::inspect_replan_parent(
            &file,chat,&replan_cursor
        )?;
        let Some(parent)=parent else{
            completed_chain=true;
            break
        };
        let source=crate::agent_plan_progress::read_chat_progress(
            profile,chat,&host.chats
        )?.into_iter().find(|p|p.plan_id==parent.parent_plan_id)
            .ok_or("Replan original parent not found in authenticated chat")?;
        if source.revision_sha256.as_deref()!=Some(parent.parent_revision_sha.as_str()){
            return Err("Replan parent revision changed; no replacement execution".into())
        }
        // A never-claimed previous draft has no plan worker or old owner
        // to inherit. It may be replaced once a new human plan has its own
        // review; do not demand an impossible process-kill receipt for a
        // process that was never dispatched. Any claim, partial evidence,
        // active owner or uncertain result still requires full independent
        // recovery instead.
        if source.supervisor_state=="NOT_CLAIMED" {
            if source.steps.iter().any(|s|s.schedule_state!="NOT_SCHEDULED")
                ||matches!(source.plan_state.as_str(),
                    "CANCEL_REQUESTED"|"NEEDS_REVIEW"){
                return Err("Unverified parent history blocks replacement worker".into())
            }
        }else{
            require_parent_os_custody_for_fresh_plan(
                host,chat,&parent.parent_plan_id,&parent.original_turn_id
            )?;
        }
        replan_cursor=parent.parent_plan_id;
    }
    if !completed_chain{
        return Err("Replan lineage depth exceeded; no execution".into())
    }
    let ancestry=lu_core::agent_lineage_recovery::inspect_continuation_lineage(
        &file,chat,plan
    )?;
    match ancestry.status{
        "ROOT_PLAN_NO_CONTINUATION"=>return Ok(()),
        "LINEAGE_RECORDED_REVIEW_ONLY"=>{},
        other=>return Err(format!(
            "Unsafe continuation ancestry: {other}. No child worker admitted."
        )),
    }
    let ledger=AgentPlanLedger::open_existing_readonly(&file)
        .map_err(|e|format!("Child ledger unavailable: {e:?}"))?;
    let revisions=AgentPlanRevisionStore::open_existing_readonly(&file)
        .map_err(|e|format!("Child revision unavailable: {e:?}"))?;
    let scheduler=AgentHostScheduler::open_existing_readonly(&file)
        .map_err(|e|format!("Child scheduler unavailable: {e:?}"))?;
    let receipts=AgentDeliverableReceipts::open_existing_readonly(&file)
        .map_err(|e|format!("Child native oracle unavailable: {e:?}"))?;
    let sources=crate::agent_plan_review::OriginalChatGoalSources{
        chats:&host.chats
    };
    let report=reconcile_recovery(
        &file,profile,chat,plan,original_turn,
        &ledger,&revisions,&scheduler,&receipts,&sources,
        &HostOriginalJobCustody::from_host_profile(profile)
    )?;
    if report.status!="REVIEW_FIRST_STEP_ONLY"{
        return Err(format!(
            "Child OS custody/predecessor re-verification refused: {}",
            report.status
        ))
    }
    Ok(())
}
