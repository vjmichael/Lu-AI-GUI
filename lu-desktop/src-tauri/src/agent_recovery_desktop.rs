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
/// A historical worker may have completed, but the installed desktop has no
/// independently accepted persistent descendant custody verifier wired yet.
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
