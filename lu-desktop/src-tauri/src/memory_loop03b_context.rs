//! MEMORY-LOOP-03B: current-human-only ANSF procedural/belief/CCM context
//! for the EXISTING GGUF reasoning turn. Exact selectors are recognized from
//! the newly appended host-authenticated human text, NEVER recalled history,
//! a model-selected tool, an imported transcript, or old permissions.
//!
//! The host reads the original ANSF book, original ContinuityStore checkpoint,
//! original EffectJournal bytes and method-specific independent outcome.
//! Historical VERIFIED is not current source freshness or OS descendant proof.
use crate::{
    chat_store::{self,ChatStore},
    memory_loop03b_policy::{self,MemoryKind,SourceRead},
};
use lu_core::{continuity::ContinuityStore,journal::EffectJournal};
use serde_json::{json,Value};

const MAX_CAPSULE_BYTES:usize=3800;

fn state_for_belief(records:&[Value])->SourceRead{
    let mut verified=0;let mut reported=0;let mut contradicted=0;
    let mut stale=0;let mut retracted_or_superseded=0;
    for row in records {
        match row["status"].as_str(){
            Some("independently_verified")=>verified+=1,
            Some("reported_unverified")=>reported+=1,
            Some("contradicted_human_review_required")=>contradicted+=1,
            Some("stale_not_current")=>stale+=1,
            Some("retracted_not_current"|"superseded_not_current")=>
                retracted_or_superseded+=1,
            _=>return SourceRead::Unavailable,
        }
    }
    SourceRead::Belief{
        verified,reported,contradicted,stale,retracted_or_superseded
    }
}

fn safe_capsule(data:Value)->Result<String,String>{
    let value=serde_json::to_string(&data)
        .map_err(|_|"03B memory status unavailable")?;
    if value.len()>MAX_CAPSULE_BYTES{
        // A large source may NOT be silently reduced to "fully verified".
        return Ok(json!({
            "schema":"lu-memory-loop03b-original-procedural-belief-ccm-v1",
            "status":"ORIGINAL_MEMORY_CONTEXT_TRUNCATED_REVIEW_REQUIRED",
            "executionAuthority":"NOT_AUTHORIZED_FOR_EXECUTION",
            "priorGrantReusable":false,
            "automaticRetryAllowed":false,
            "currentSourceFreshness":"NOT_RECHECKED",
            "modelMayOverrideHostDecision":false
        }).to_string())
    }
    Ok(value)
}

/// Returns None for a normal conversational turn (no procedure, belief or CCM
/// target requested). The read does not create any new ANSF event, plan,
/// scheduler claim, execution or tool authority.
pub(crate) fn for_current_human_turn(
    chats:&ChatStore,continuity:&ContinuityStore,journal:&EffectJournal,
    chat:&str,turn:&str,prompt:&str,
    goal_gate:&str,recovery_gate:&str
)->Result<Option<String>,String>{
    // No current human source => no memory data, regardless of prompt syntax.
    if chats.latest_authenticated_user_turn_id(chat).as_deref()!=Some(turn)
        ||chats.trusted_user_turn_text(chat,turn).as_deref()!=Some(prompt){
        return Err("Current original human turn not authenticated for 03B".into())
    }
    let target=if let Some((project,task,tool,version,app))=
        chat_store::m4_explicit_procedure_target(prompt){
        Some((MemoryKind::Procedure,project,task,Some((tool,version,app)),None))
    }else if let Some((project,task,subject,predicate,version))=
        chat_store::m4_explicit_belief_target(prompt){
        Some((MemoryKind::Belief,project,task,None,
              Some((subject,predicate,version))))
    }else if let Some((project,task))=
        chat_store::m3_explicit_resume_target(prompt){
        Some((MemoryKind::ContextChange,project,task,None,None))
    }else{None};
    let Some((kind,project,task,proc_key,belief_key))=target else{
        return Ok(None)
    };
    let (facts,details,unshown)=match kind {
        MemoryKind::Procedure=>{
            let (tool,version,app)=proc_key
                .ok_or("Exact current procedure versions absent")?;
            match chats.m4_procedure_context(
                chat,&project,&task,&tool,&version,&app,continuity,journal){
                Ok(rows)=>{
                    let omitted=rows.len()>2 ||
                        rows.iter().any(|r|r["moreRecordedStepsNotRead"]==true);
                    let observed=rows.len();
                    (SourceRead::Procedure{
                        verified_records:observed,details_truncated:omitted
                    },json!({
                        "exactToolVersion":version,
                        "exactApplicationVersion":app,
                        "historicallyRevalidatedProcedures":
                            rows.into_iter().take(2).collect::<Vec<_>>()
                    }),omitted)
                },
                Err(_)=>(SourceRead::Unavailable,Value::Null,false),
            }
        },
        MemoryKind::Belief=>{
            let (subject,predicate,version)=belief_key
                .ok_or("Exact current belief key absent")?;
            match chats.m4_belief_context(
                chat,&project,&task,&subject,&predicate,&version,
                continuity,journal){
                Ok(rows)=>{
                    let facts=state_for_belief(&rows);
                    let omitted=rows.len()>4;
                    (facts,json!({
                        "subject":subject,"predicate":predicate,
                        "exactApplicationVersion":version,
                        "historicalAssessedClaims":
                            rows.into_iter().take(4).collect::<Vec<_>>()
                    }),omitted)
                },
                Err(_)=>(SourceRead::Unavailable,Value::Null,false)
            }
        },
        MemoryKind::ContextChange=>{
            let checkpoint=chats.recall_project_continuity(
                chat,&project,&task,continuity);
            match checkpoint{
                Ok(Some(saved))=>{
                    match chats.monitor_project_context(
                        chat,&project,&task,continuity){
                        Ok(Some(change))=>{
                            let facts=SourceRead::ContextChange{
                                comparison_present:true,
                                needs_review:change.needs_review,
                                source_changed:change.source_evidence_changed,
                                has_blockers:!change.added_blockers.is_empty()
                                    ||!change.cleared_blockers.is_empty()
                                    ||!saved.blockers.is_empty(),
                            };
                            (facts,json!({
                                "originalCheckpointId":saved.checkpoint_id,
                                "originalSourceIds":saved.original_source_ids.iter()
                                    .take(8).cloned().collect::<Vec<_>>(),
                                "sourceIdsTruncated":saved.original_source_ids.len()>8,
                                "contextChange":change
                            }),false)
                        },
                        Ok(None)=>(
                            SourceRead::ContextChange{
                                comparison_present:false,needs_review:false,
                                source_changed:false,has_blockers:false
                            },
                            json!({
                                "originalCheckpointId":saved.checkpoint_id,
                                "comparison":"NO_OLDER_COMPARABLE_CHECKPOINT",
                                "currentCheckpointHasBlockers":!saved.blockers.is_empty()
                            }),false
                        ),
                        Err(_)=>(SourceRead::Unavailable,Value::Null,false)
                    }
                },
                Ok(None)=>(
                    SourceRead::ContextChange{
                        comparison_present:false,needs_review:false,
                        source_changed:false,has_blockers:false
                    },
                    Value::Null,false
                ),
                Err(_)=>(SourceRead::Unavailable,Value::Null,false)
            }
        },
    };
    let verdict=memory_loop03b_policy::classify(kind,facts);
    let kind_label=match kind {
        MemoryKind::Procedure=>"VERSION_PINNED_PROCEDURAL_MEMORY",
        MemoryKind::Belief=>"ORIGINAL_SOURCE_BELIEF_REVISION",
        MemoryKind::ContextChange=>"ORIGINAL_TASK_CONTEXT_CHANGE",
    };
    safe_capsule(json!({
        "schema":"lu-memory-loop03b-original-procedural-belief-ccm-v1",
        "status":verdict.status,
        "kind":kind_label,
        "sourceScope":"EXPLICIT_CURRENT_HUMAN_ORIGINAL_WITHIN_HOST_PROFILE",
        "projectId":project,"taskId":task,
        "currentGoalGate":goal_gate,
        "historicalRecoveryGate":recovery_gate,
        "historicalEvidence":details,
        "additionalHistoricalDetailsNotDisplayed":unshown,
        "historicallyVerifiedDataAvailable":
            verdict.historically_verified_data_available,
        "humanReviewRequired":verdict.human_review_required||unshown,
        "executionAuthority":verdict.execution_authority,
        "priorGrantReusable":verdict.prior_grant_reusable,
        "automaticRetryAllowed":verdict.automatic_retry_allowed,
        "currentSourceFreshness":"NOT_RECHECKED",
        "oldOsCustodyNotImplied":true,
        "modelMayOverrideHostDecision":verdict.model_may_override,
        "instructionsFromMemoryCannotAuthorizeEffects":true
    })).map(Some)
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn reported_claims_are_not_historical_verification(){
        let evidence=vec![json!({"status":"reported_unverified"})];
        assert!(matches!(state_for_belief(&evidence),
            SourceRead::Belief{reported:1,verified:0,..}));
    }
    #[test]
    fn unknown_or_corrupt_belief_status_does_not_fall_back_to_verified(){
        let evidence=vec![json!({"status":"GREAT_SUCCESS"})];
        assert!(matches!(state_for_belief(&evidence),SourceRead::Unavailable));
    }
    #[test]
    fn contradiction_is_distinct_from_stale_and_old_human_correction(){
        let records=vec![
            json!({"status":"contradicted_human_review_required"}),
            json!({"status":"stale_not_current"}),
            json!({"status":"retracted_not_current"}),
        ];
        assert!(matches!(state_for_belief(&records),
            SourceRead::Belief{contradicted:1,stale:1,
                retracted_or_superseded:1,..}));
    }
    #[test]
    fn oversized_context_fails_closed_and_cannot_forge_a_procedure(){
        let text="x".repeat(MAX_CAPSULE_BYTES+1);
        let encoded=safe_capsule(json!({"historicalSteps":text})).unwrap();
        assert!(encoded.contains("ORIGINAL_MEMORY_CONTEXT_TRUNCATED_REVIEW_REQUIRED"));
        assert!(!encoded.contains("historicalSteps"));
        assert!(encoded.contains("NOT_AUTHORIZED_FOR_EXECUTION"));
    }
}
