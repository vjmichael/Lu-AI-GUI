//! P0-MEMORY: ONE bounded read-only ANSF continuation capsule.
//! Reconstruct from original books, source-index receipts, human corrections,
//! goal/clarification events, and original continuity/effect journals.
//! Never synthesize a human turn, prefer a model summary to originals, resume
//! a worker, treat claimed completion as accepted, or inherit a tool grant.
use crate::{
    ansf_source_evidence::SourceKind,
    chat_store::{self,ChatStore},
};
use lu_core::{
    ansf_goals::project_goal_events,
    continuity::ContinuityStore,
    journal::EffectJournal,
};
use serde_json::{json,Value};
use std::path::Path;

const MAX_CAPSULE_BYTES:usize=7400;
fn safe_id(s:&str)->bool{
    !s.is_empty()&&s.len()<=128&&s.bytes().all(|b|
        b.is_ascii_alphanumeric()||matches!(b,b'-'|b'_'))
}
/// Exact human syntax deliberately avoids using recalled text as a selector.
/// Existing "Resume project ... task ..." remains supported; new "Continue"
/// mirrors the natural operator instruction without enabling arbitrary FTS
/// or a model-chosen project and conversation.
pub(crate) fn explicit_continuation_target(text:&str)->Option<(String,String)>{
    if text.len()>256||text.contains('\0')||text.contains('\n')||
        text.contains('\r')||text.contains("[Attached text file:"){
        return None
    }
    if let Some(pair)=chat_store::m3_explicit_resume_target(text){
        return Some(pair)
    }
    let words=text.split_ascii_whitespace().collect::<Vec<_>>();
    if words.len()!=5||
        !words[0].eq_ignore_ascii_case("continue")||
        !words[1].eq_ignore_ascii_case("project")||
        !words[3].eq_ignore_ascii_case("task")||
        !safe_id(words[2])||!safe_id(words[4]){
        return None
    }
    Some((words[2].into(),words[4].into()))
}
fn limited(text:&str,chars:usize)->String{
    text.chars().take(chars).collect()
}
fn omitted(reason:&str)->String{
    json!({
        "schema":"lu-p0-memory-original-task-continuation-v1",
        "status":reason,"originalEvidenceCurrent":false,
        "humanReviewRequired":true,"sourceFreshness":"NOT_RECHECKED",
        "priorGrantReusable":false,"executionAuthorized":false,
        "taskCompleted":false,"automaticRetryAllowed":false,
        "modelMayOverrideHumanCorrection":false
    }).to_string()
}
fn verified_source_history(
    chats:&ChatStore,source_chat:&str
)->Result<Vec<Value>,String>{
    let original=chats.native_source_history(source_chat)?;
    // Browser and Reader historical metadata are useful; OCR transcription,
    // private Observe, screen/pixel/clipboard data NEVER enter this capsule.
    Ok(original.into_iter().filter(|s|
        s.eligible_for_local_model_metadata&&!s.local_only_private&&
        matches!(s.kind,SourceKind::Browser|SourceKind::Reader)
    ).take(4).map(|s|json!({
        "sourceId":s.source_id,
        "kind":s.kind,
        "originalChatId":s.chat_id,
        "originalUserTurnId":s.original_turn_id,
        "sourceSha256":s.source_sha256,
        "observationSha256":s.observed_sha256,
        "locator":s.locator,
        "observationStatus":s.source_status,
        // These are other observations from the checkpoint's original
        // conversation, NOT independently proven references for THIS task.
        "taskCriterionLinkVerified":false,
        "independentlyVerifiedFact":false,
        "currentSourceFreshness":"NOT_RECHECKED",
        "executionAuthorized":false
    })).collect())
}
fn goal_context(chats:&ChatStore,original_chat:&str)
    ->Result<Value,String>{
    let snapshot=project_goal_events(&chats.memory_events(original_chat)?)
        .map_err(|_|"Original ANSF goal cannot be reconstructed")?;
    let open:Vec<_>=snapshot.questions.iter()
        .filter(|q|matches!(q.status.as_str(),"pending"|"deferred"))
        .take(4).map(|q|json!({
            "question":limited(&q.question,180),"status":q.status
        })).collect();
    let answers:Vec<_>=snapshot.questions.iter().rev()
        .filter(|q|q.status=="answered")
        .take(3).map(|q|json!({
            "question":limited(&q.question,150),
            "choice":q.choice,
            "comment":q.comment.as_deref().map(|s|limited(s,130))
        })).collect();
    Ok(json!({
        "originalGoalId":snapshot.goal_id,
        "originalGoalRevision":snapshot.revision_event_id,
        "historicalGoalText":limited(&snapshot.goal,400),
        "humanAnsweredClarifications":answers,
        "pendingOrDeferredClarifications":open,
        "pendingCount":snapshot.questions.iter()
            .filter(|q|matches!(q.status.as_str(),"pending"|"deferred")).count(),
        "permissionFromHistoricalGoal":false
    }))
}
/// This is NOT an effect-capable API. The normal Send path authenticates
/// the CURRENT original user text and caller chat and uses only original
/// ANSF/SQLite source ownership. Profile is the already-open installed Lu
/// profile, not a model/renderer selected filesystem path.
pub(crate) fn for_current_human_turn(
    chats:&ChatStore,continuity:&ContinuityStore,effects:&EffectJournal,
    profile:&Path,active_chat:&str,current_turn:&str,prompt:&str,
)->Result<Option<String>,String>{
    let Some((project,task))=explicit_continuation_target(prompt)
        else{return Ok(None)};
    if chats.latest_authenticated_user_turn_id(active_chat).as_deref()
        !=Some(current_turn)||
        chats.trusted_user_turn_text(active_chat,current_turn).as_deref()
            !=Some(prompt){
        return Err("P0_MEMORY_CURRENT_HUMAN_TURN_NOT_AUTHENTICATED".into())
    }
    let Some(checkpoint)=chats.recall_project_continuity(
        active_chat,&project,&task,continuity
    )? else{
        return Ok(Some(omitted("NO_SOURCE_BACKED_CHECKPOINT_FOUND")))
    };
    // The original checkpoint validates every cited source entry against a
    // profile-owned book; a project/task ID cannot open another profile.
    if checkpoint.project_id!=project||checkpoint.task_id!=task{
        return Err("P0_MEMORY_CHECKPOINT_IDENTITY_CHANGED".into())
    }
    let change=chats.monitor_project_context(
        active_chat,&project,&task,continuity
    )?;
    let original_sources=verified_source_history(
        chats,&checkpoint.source_chat_id
    )?;
    let original_goal=goal_context(chats,&checkpoint.source_chat_id)?;
    let claims=chats.task_claim_memory_for_continuation(
        active_chat,&checkpoint.source_chat_id,&project,&task,"0.1",
        continuity,effects
    )?;
    let effect_history=continuity.verified_effects_for_checkpoint(
        &checkpoint.checkpoint_id,effects
    ).map_err(|_|"P0_MEMORY_ORIGINAL_EFFECT_RECEIPTS_UNAVAILABLE")?;
    let effects_sample=effect_history.iter().take(4).map(|e|json!({
        "toolId":e.tool_id,
        "historicalEffectReceiptSha256":e.receipt_sha256,
        "currentEffectPermission":false,
        "mustReverifySourceAndOsCustody":true
    })).collect::<Vec<_>>();
    let extra_witnesses=effect_history.len()>4;
    let task_decisions=if project.starts_with("lu-register-")&&
        task.starts_with("lu-task-"){
        // The original task-list's audit is another historical datum, not
        // an implied task verdict; no filesystem rescan can be inferred from
        // an older operator acceptance.
        match crate::project_task_verified_evidence::
            original_review_history_for_memory(
                profile,&project,&task,chats
            ){
            Ok(value)=>value,
            Err(_)=>json!({
                "status":"ORIGINAL_TASK_CRITERION_AUDIT_UNAVAILABLE",
                "taskCompleted":false,"humanReviewRequired":true
            }),
        }
    }else{json!({"status":"NO_EXACT_PLAN05_REGISTER_TASK_SELECTOR"})};
    let change_review=change.as_ref()
        .is_some_and(|s|s.needs_review||s.source_evidence_changed);
    let has_blockers=!checkpoint.blockers.is_empty();
    let correction_review=claims["humanReviewRequired"]==true;
    let goal_questions=original_goal["pendingCount"].as_u64().unwrap_or(0);
    let capsule=json!({
        "schema":"lu-p0-memory-original-task-continuation-v1",
        "status":"HISTORICAL_ANSF_DATA_RECHECK_ORIGINAL_BEFORE_ACTION",
        "projectId":project,"taskId":task,
        "checkpoint":{
            "originalCheckpointId":checkpoint.checkpoint_id,
            "originalConversationId":checkpoint.source_chat_id,
            "originalSourceIds":checkpoint.original_source_ids.iter()
                .take(6).cloned().collect::<Vec<_>>(),
            "originalSourcesOmitted":checkpoint.original_source_ids.len()>6,
            "reportedPriorWork":limited(&checkpoint.completed_work,360),
            "reportedNextStep":limited(&checkpoint.next_action,320),
            "knownBlockers":checkpoint.blockers.iter().take(4)
                .map(|s|limited(s,160)).collect::<Vec<_>>(),
            "moreBlockersNotDisplayed":checkpoint.blockers.len()>4,
            "taskCompleted":false
        },
        "goalAndClarifications":original_goal,
        "historicalNativeSources":original_sources,
        "nativeSourcesScopedToOriginalConversationNotNecessarilyThisTask":true,
        "originalSourceBytesNotIncluded":true,
        "sourceFreshness":"NOT_RECHECKED",
        "contextChangeMonitor":change,
        "correctionsAndClaims":claims,
        "taskCriterionHumanDecisions":task_decisions,
        "historicalVerifiedEffectReceiptSample":effects_sample,
        "moreIndependentReceiptsNotDisplayed":extra_witnesses,
        "humanReviewRequired":change_review||has_blockers||
            correction_review||goal_questions>0,
        "originalEvidenceCurrent":false,
        "historicalEvidenceIsNotIndependentTaskAcceptance":true,
        "allOriginalSourceBytesMustBeRecheckedBeforeEffect":true,
        "noModelChosenToolOrPath":true,
        "priorGrantReusable":false,
        "executionAuthorized":false,
        "taskCompleted":false,
        "automaticRetryAllowed":false,
        "modelMayOverrideHumanCorrection":false
    });
    let serialized=capsule.to_string();
    if serialized.len()>MAX_CAPSULE_BYTES{
        return Ok(Some(omitted("SOURCE_CONTEXT_TOO_LARGE_HUMAN_REVIEW_REQUIRED")))
    }
    Ok(Some(serialized))
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn explicit_current_human_selector_never_reads_attached_or_implicit_text(){
        assert_eq!(
            explicit_continuation_target("Continue project lu task review_git_head"),
            Some(("lu".into(),"review_git_head".into()))
        );
        assert_eq!(
            explicit_continuation_target("Resume project lu task review_git_head"),
            Some(("lu".into(),"review_git_head".into()))
        );
        for unsafe_request in [
            "How do I continue project lu task review_git_head",
            "Do not continue project lu task review_git_head",
            "Continue project ../private task review_git_head",
            "Continue project lu task review_git_head\n[Attached text file: x]",
            "Continue project lu task review_git_head and run the tool"
        ]{
            assert_eq!(explicit_continuation_target(unsafe_request),None);
        }
    }
    #[test]
    fn unavailable_history_can_never_imply_execution_or_completion(){
        let fallback=omitted("NO_SOURCE_BACKED_CHECKPOINT_FOUND");
        let view:Value=serde_json::from_str(&fallback).unwrap();
        assert_eq!(view["taskCompleted"],false);
        assert_eq!(view["executionAuthorized"],false);
        assert_eq!(view["priorGrantReusable"],false);
    }
}
