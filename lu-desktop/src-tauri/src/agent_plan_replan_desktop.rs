//! Phase 2 — explicit human-directed replanning after changed/failed work.
//! A new plan is a NEW inert draft. It never borrows an earlier authority
//! token, OS worker or receipt, and an incomplete parent link blocks dispatch.
use super::*;
use serde::Deserialize;
use lu_core::{
    agent_plan_ledger::PlanStepKind,
    agent_plan_revisions::ExpectedDeliverable,
    agent_replan_lineage::{stage_human_replan,bind_replan_child},
};
use crate::agent_plan_admission::{ProposedStep,stage_initial_draft};

#[derive(Clone,Debug,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct HumanReplanStep {
    kind:String,
    depends_on:Option<u8>,
    expected:String,
}
fn decode_steps(request:&[HumanReplanStep])->Result<Vec<ProposedStep>,String>{
    if request.is_empty()||request.len()>8{
        return Err("Provide 1–8 bounded human-reviewed steps".into())
    }
    let mut steps=Vec::with_capacity(request.len());
    for (i,step) in request.iter().enumerate(){
        let kind=match step.kind.as_str(){
            "COLLECT_SOURCE"=>PlanStepKind::CollectSource,
            "INSPECT"=>PlanStepKind::Inspect,
            "ANALYZE"=>PlanStepKind::Analyze,
            "INDEPENDENT_CHECK"=>PlanStepKind::IndependentlyCheck,
            "HUMAN_REVIEW"=>PlanStepKind::HumanReview,
            "PRESENT"=>PlanStepKind::Present,
            _=>return Err("Unknown replan step kind".into()),
        };
        let expected=match step.expected.as_str(){
            "SOURCE_REFERENCES"=>ExpectedDeliverable::SourceReferences,
            "STRUCTURED_FINDINGS"=>ExpectedDeliverable::StructuredFindings,
            "INDEPENDENT_WITNESS"=>ExpectedDeliverable::IndependentWitness,
            "HUMAN_DECISION"=>ExpectedDeliverable::HumanDecision,
            "ARTIFACT_REFERENCE"=>ExpectedDeliverable::ArtifactReference,
            "EXPLANATION"=>ExpectedDeliverable::Explanation,
            _=>return Err("Unknown replan expected deliverable".into()),
        };
        if (i==0&&step.depends_on.is_some())
            ||step.depends_on.is_some_and(|parent|parent as usize>=i){
            return Err("Cyclic or disconnected replan dependency".into())
        }
        steps.push(ProposedStep{kind,depends_on:step.depends_on,expected});
    }
    Ok(steps)
}
fn allowed_reason(reason:&str)->bool{
    matches!(reason,"REVISE_DEPENDENCIES"|"CORRECT_RESULT"|"CHANGE_APPROACH"|"GOAL_CHANGED")
}
/// Replanning can begin while a previous operation needs recovery because
/// staging is NON-EXECUTING. The independent OS/oracle ancestry gate
/// blocks any replacement worker until the old effect is truly accounted for.
#[tauri::command]
pub(crate) async fn lu_agent_stage_replan(
    app:AppHandle,state:State<'_,Arc<DesktopState>>,
    chat_id:String,plan_id:String,revision_sha:String,
    reason:String,steps:Vec<HumanReplanStep>,
)->Result<Option<String>,String>{
    if !allowed_reason(&reason){return Err("Unreviewed replan reason".into())}
    let replacement=decode_steps(&steps)?;
    let host=Arc::clone(state.inner());
    host.chats.history_window(&chat_id,None,"older",1)?;
    tauri::async_runtime::spawn_blocking(move||{
        let root=host.catalog_path.parent().ok_or("Lu plan profile unavailable")?;
        let current=agent_plan_progress::read_chat_progress(
            root,&chat_id,&host.chats
        )?.into_iter().find(|p|p.plan_id==plan_id)
            .ok_or("Original plan is not in this authenticated chat")?;
        if current.revision_sha256.as_deref()!=Some(revision_sha.as_str()){
            return Err("Source plan revision changed; inspect the current plan".into())
        }
        let summary=replacement.iter().enumerate().map(|(i,s)|
            format!("{}: {:?}, dependency {:?}, expected {:?}",
                i+1,s.kind,s.depends_on,s.expected)
        ).collect::<Vec<_>>().join("\n");
        let prompt=format!(
            "Request a NEW plan after original plan {} revision {}?\n\
             Reason: {}\n{}\n\n\
             A fresh saved human turn and immutable replan link will be \
             created. Prior uncertain workers may STILL be running. \
             This does NOT retry a worker, approve a plan, authorize any \
             tool, clear OS custody or grant future execution.",
            plan_id,revision_sha,reason,summary
        );
        if !app.dialog().message(prompt)
            .title("Lu — New human-reviewed replan draft (no execution)")
            .buttons(tauri_plugin_dialog::MessageDialogButtons::OkCancelCustom(
                "Draft replacement".into(),"Cancel".into()
            )).blocking_show(){
            return Ok(None)
        }
        let fresh=next_id("g-");
        host.chats.append_user_with_turn_id(
            &chat_id,&fresh,&format!(
                "I request a new human-reviewed plan, replacing plan {} \
                 revision {} for reason {}. Proposed steps:\n{}\n\
                 No prior effect, approval or process authority may be reused.",
                plan_id,revision_sha,reason,summary
            )
        )?;
        let file=agent_plan_review::existing_plan_file(root)?;
        stage_human_replan(
            &file,&chat_id,&plan_id,&current.original_human_turn_id,
            &revision_sha,&fresh,&reason,&host.chats
        )?;
        let draft=stage_initial_draft(
            root,&host.chats,&chat_id,&fresh,&replacement
        )?;
        bind_replan_child(
            &file,&chat_id,&plan_id,&revision_sha,&fresh,&draft.plan_id
        )?;
        // Exact replacement still requires lu_agent_review_plan and a NEW
        // native picker/explicit per-step worker authorization.
        Ok(Some(draft.plan_id))
    }).await.map_err(|_|"Native human replan task interrupted")?
}

#[cfg(test)]
mod tests{
    use super::*;
    fn step(kind:&str,dep:Option<u8>,expected:&str)->HumanReplanStep{
        HumanReplanStep{kind:kind.into(),depends_on:dep,expected:expected.into()}
    }
    #[test]
    fn replan_requires_bounded_human_graph_not_a_model_permission(){
        let proposed=vec![
            step("COLLECT_SOURCE",None,"SOURCE_REFERENCES"),
            step("INDEPENDENT_CHECK",Some(0),"INDEPENDENT_WITNESS"),
            step("PRESENT",Some(1),"EXPLANATION")
        ];
        let decoded=decode_steps(&proposed).unwrap();
        assert_eq!(decoded.len(),3);
        assert_eq!(decoded[2].kind,PlanStepKind::Present);
        assert!(!allowed_reason("GRANT_NATIVE_TOOL"));
        assert!(decode_steps(&[step("EXECUTE_SHELL",None,"ARTIFACT_REFERENCE")]).is_err());
        assert!(decode_steps(&[step("INDEPENDENT_CHECK",Some(0),"INDEPENDENT_WITNESS")]).is_err());
        assert!(decode_steps(&[
            step("INDEPENDENT_CHECK",None,"INDEPENDENT_WITNESS"),
            step("INDEPENDENT_CHECK",Some(1),"INDEPENDENT_WITNESS")
        ]).is_err());
    }
}
