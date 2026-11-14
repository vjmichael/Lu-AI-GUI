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
    agent_plan_revisions::{AgentPlanRevisionStore,ExpectedDeliverable,RevisionStep},
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


/// Phase 2: carry the *unexecuted suffix* of a linear, source-reviewed plan
/// forward to the next separately authorized child. The previous version
/// discarded all but one step, permanently stranding 3+ step plans. Preserve
/// the kinds and expected result types: never substitute an independent
/// Git check for a different unimplemented operation.
fn remaining_reviewed_steps(
    reviewed:&[RevisionStep], next:u8,
)->Result<Vec<ProposedStep>,String>{
    if next==0 || next as usize>=reviewed.len() || reviewed.len()>32 {
        return Err("No bounded next step in reviewed plan".into());
    }
    let selected=next as usize;
    if reviewed[selected].kind!=PlanStepKind::IndependentlyCheck
        ||reviewed[selected].expected!=ExpectedDeliverable::IndependentWitness {
        return Err("Next operation has no installed reviewed worker adapter; replan required".into());
    }
    let mut remaining=Vec::with_capacity(reviewed.len()-selected);
    for (index,step) in reviewed.iter().enumerate().skip(selected) {
        if step.depends_on!=Some((index-1) as u8) {
            return Err("Nonlinear plan requires explicit human-reviewed replanning".into());
        }
        let depends_on=if index==selected {None}
            else{Some((index-selected-1) as u8)};
        remaining.push(ProposedStep{
            kind:step.kind,depends_on,expected:step.expected,
        });
    }
    Ok(remaining)
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
        crate::agent_recovery_desktop::require_parent_os_custody_for_fresh_plan(
            &host,&chat_id,&plan_id,&progress.original_human_turn_id
        )?;
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
        crate::agent_recovery_desktop::require_parent_os_custody_for_fresh_plan(
            &host,&chat_id,&plan_id,&progress.original_human_turn_id
        )?;
        stage_fresh_continuation(
            &path,root,&plan_id,&chat_id,&progress.original_human_turn_id,
            &revision_sha,step_index,&turn,budget,
            &ledger,&revisions,&scheduler,&receipts,
            &sources,&host.chats,&consent
        )?;
        let current=revisions.latest(&plan_id,&chat_id)
            .map_err(|e|format!("Current reviewed continuation unavailable: {e:?}"))?
            .ok_or("Reviewed continuation revision missing")?;
        if current.revision_sha256!=revision_sha
            ||current.review_state!="APPROVED_PLAN_ONLY" {
            return Err("Plan changed during next-step preparation".into());
        }
        let remaining=remaining_reviewed_steps(&current.steps,step_index)?;
        let child=stage_initial_draft(
            root,&host.chats,&chat_id,&turn,&remaining
        )?;
        lu_core::agent_continuation_intent::bind_child_plan(
            &path,&plan_id,&revision_sha,step_index,&turn,&child.plan_id,
        )?;
        // New child plan is NOT approved. Original plan execution has no
        // additional scheduler owner, tool grant, or model continuation.
        Ok(Some(child.plan_id))
    }).await.map_err(|_|"Native continuation preparation failed")?
}


#[cfg(test)]
mod phase2_multistep_tests {
    use super::*;
    fn step(index:usize, kind:PlanStepKind, expected:ExpectedDeliverable)->RevisionStep{
        RevisionStep{
            kind, expected,depends_on:if index==0{None}else{Some((index-1) as u8)},
            deliverable_id:format!("d-{}",format!("{index:032x}")),
        }
    }
    #[test]
    fn three_step_reviewed_execution_retains_two_step_child_and_future_dependency(){
        let original=(0..3).map(|i|step(
            i,PlanStepKind::IndependentlyCheck,ExpectedDeliverable::IndependentWitness
        )).collect::<Vec<_>>();
        let child=remaining_reviewed_steps(&original,1).unwrap();
        assert_eq!(child.len(),2);
        assert_eq!(child[0].depends_on,None);
        assert_eq!(child[1].depends_on,Some(0));
        assert_eq!(child[0].expected,ExpectedDeliverable::IndependentWitness);
        let grandchild=(0..2).map(|i|step(i,child[i].kind,child[i].expected))
            .collect::<Vec<_>>();
        assert_eq!(remaining_reviewed_steps(&grandchild,1).unwrap().len(),1);
    }
    #[test]
    fn future_unsupported_step_is_preserved_not_silently_run_as_git(){
        let original=vec![
            step(0,PlanStepKind::IndependentlyCheck,ExpectedDeliverable::IndependentWitness),
            step(1,PlanStepKind::IndependentlyCheck,ExpectedDeliverable::IndependentWitness),
            step(2,PlanStepKind::Present,ExpectedDeliverable::Explanation),
        ];
        let child=remaining_reviewed_steps(&original,1).unwrap();
        assert_eq!(child[1].kind,PlanStepKind::Present);
        assert_eq!(child[1].expected,ExpectedDeliverable::Explanation);
        let remaining=vec![step(0,child[0].kind,child[0].expected),
            step(1,child[1].kind,child[1].expected)];
        assert!(remaining_reviewed_steps(&remaining,1).is_err());
    }
    #[test]
    fn non_sequential_or_missing_predecessor_cannot_be_reindexed(){
        let mut original=(0..4).map(|i|step(i,PlanStepKind::IndependentlyCheck,
            ExpectedDeliverable::IndependentWitness)).collect::<Vec<_>>();
        original[3].depends_on=Some(1);
        assert!(remaining_reviewed_steps(&original,1).is_err());
        assert!(remaining_reviewed_steps(&original,0).is_err());
    }
}
