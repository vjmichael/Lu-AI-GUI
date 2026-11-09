//! AGENT-01I: only the installed host can request a new CHILD plan.
//! The released parent owner is never recycled. Human confirmation here
//! stages an immutable continuation intent, and creates a separate inert plan.
//! That child still requires its own plan-review turn and a fresh OS picker /
//! operation confirmation / saved execution turn before any broker dispatch.
use super::*;
use lu_core::{
    agent_continuation_intent::stage_fresh_continuation,
    agent_host_scheduler::{AgentHostScheduler,HostFreshStepConsent,ScheduleBudget},
    agent_plan_ledger::AgentPlanLedger,
    agent_plan_revisions::{AgentPlanRevisionStore,ExpectedDeliverable},
    agent_deliverable_receipts::AgentDeliverableReceipts,
    agent_plan_ledger::PlanStepKind,
};
use crate::agent_plan_admission::{ProposedStep,stage_initial_draft};

struct NextStepConsent{
    chat:String,turn:String,plan:String,sha:String,step:u8,budget:ScheduleBudget,
}
impl HostFreshStepConsent for NextStepConsent{
    fn explicitly_requested_step(
        &self,chat:&str,turn:&str,plan:&str,sha:&str,step:u8,
        budget:&ScheduleBudget,
    )->bool{
        self.chat==chat &&self.turn==turn &&self.plan==plan
            &&self.sha==sha &&self.step==step &&self.budget==*budget
    }
}
fn single_step_budget()->ScheduleBudget{
    ScheduleBudget{
        max_claims:1,model_token_ceiling:1536,tool_call_ceiling:1,
        wall_clock_ceiling_ms:60_000,output_ceiling_bytes:65_536,
    }
}

/// Native OS confirmation is a fresh *preparation request*, never a grant.
/// The second plan is deliberately PENDING_HUMAN_REVIEW, and its first
/// independently checked step uses existing lu_agent_run_reviewed_step only
/// after a SECOND saved review and THIRD distinct saved tool authorization.
#[tauri::command]
pub(crate) async fn lu_agent_prepare_next_step(
    app:AppHandle,state:State<'_,Arc<DesktopState>>,
    chat_id:String,plan_id:String,revision_sha:String,step_index:u8,
)->Result<Option<String>,String>{
    let host=Arc::clone(state.inner());
    host.chats.history_window(&chat_id,None,"older",1)?;
    tauri::async_runtime::spawn_blocking(move||{
        if step_index==0||step_index>=32{
            return Err("Continuation must follow an existing verified step".into())
        }
        let root=host.catalog_path.parent()
            .ok_or("Original host profile unavailable")?;
        let progress=agent_plan_progress::read_chat_progress(
            root,&chat_id,&host.chats
        )?.into_iter().find(|p|p.plan_id==plan_id)
            .ok_or("No authenticated original plan for this chat")?;
        if progress.revision_sha256.as_deref()!=Some(revision_sha.as_str())
            ||progress.revision_review!="APPROVED_PLAN_ONLY"
            ||progress.supervisor_state!="RELEASED"
            ||progress.plan_state=="CANCEL_REQUESTED"
            ||progress.plan_state=="NEEDS_REVIEW"{
            return Err("Original verified and released plan required".into())
        }
        let next=progress.steps.get(step_index as usize)
            .ok_or("Continuation step missing from original plan")?;
        if next.depends_on!=Some(step_index-1)
            ||next.schedule_state!="NOT_SCHEDULED"
            ||next.purpose!="INDEPENDENT_CHECK"
            ||next.expected_deliverable!="INDEPENDENT_WITNESS"{
            return Err("Only one sequential independent witness can be prepared".into())
        }
        let prompt=format!(
            "Prepare a NEW separately reviewed plan for original plan {} \
             revision {} step {}?\n\nThis will NOT execute the step. \
             It will create a saved human request and immutable new-plan draft. \
             You must separately review that new plan, select a real source, \
             and authorize its one tool operation. The previous plan owner's \
             tool permission and claim will NEVER be reused.",
            plan_id,revision_sha,step_index+1
        );
        if !app.dialog().message(prompt)
            .title("Lu — Prepare fresh continuation (no tool grant)")
            .buttons(tauri_plugin_dialog::MessageDialogButtons::OkCancelCustom(
                "Prepare new plan".into(),"Cancel".into()
            )).blocking_show(){
            return Ok(None)
        }
        let turn=next_id("g-");
        host.chats.append_user_with_turn_id(
            &chat_id,&turn,
            &format!("I request a separately authorized new reviewed plan for \
              original plan {} revision {} step {}. No execution permission \
              is given by this preparation request.",plan_id,revision_sha,step_index+1)
        )?;
        let path=agent_plan_review::existing_plan_file(root)?;
        let ledger=AgentPlanLedger::open(&path)
            .map_err(|e|format!("Original plan ledger error: {e:?}"))?;
        let revisions=AgentPlanRevisionStore::open(&path)
            .map_err(|e|format!("Original revision error: {e:?}"))?;
        let receipts=AgentDeliverableReceipts::open(&path)
            .map_err(|e|format!("Independent witness error: {e:?}"))?;
        let scheduler=AgentHostScheduler::open(&path)
            .map_err(|e|format!("Original host fence error: {e:?}"))?;
        let sources=agent_plan_review::OriginalChatGoalSources{chats:&host.chats};
        let budget=single_step_budget();
        let consent=NextStepConsent{
            chat:chat_id.clone(),turn:turn.clone(),
            plan:plan_id.clone(),sha:revision_sha.clone(),
            step:step_index,budget,
        };
        stage_fresh_continuation(
            &path,root,&plan_id,&chat_id,&progress.original_human_turn_id,
            &revision_sha,step_index,&turn,budget,
            &ledger,&revisions,&scheduler,&receipts,
            &sources,&host.chats,&consent
        )?;
        let child=stage_initial_draft(
            root,&host.chats,&chat_id,&turn,
            &[ProposedStep{
                kind:PlanStepKind::IndependentlyCheck,
                depends_on:None,
                expected:ExpectedDeliverable::IndependentWitness,
            }]
        )?;
        lu_core::agent_continuation_intent::bind_child_plan(
            &path,&plan_id,&revision_sha,step_index,&turn,&child.plan_id,
        )?;
        // New child plan is NOT approved. Original plan execution has no
        // additional scheduler owner, tool grant, or model continuation.
        Ok(Some(child.plan_id))
    }).await.map_err(|_|"Native continuation preparation failed")?
}
