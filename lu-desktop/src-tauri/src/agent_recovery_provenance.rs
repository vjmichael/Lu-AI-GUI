//! MEMORY-LOOP-02C: operator-visible provenance for historical reviewed effects.
//!
//! This is a read-only projection of ORIGINAL host-authenticated ANSF turns,
//! task/effect SQLite, independent source oracles and native process custody.
//! It neither reads model-selected identifiers nor grants/resumes any operation.
use std::path::Path;
use serde::Serialize;
use lu_core::{
    agent_os_custody::HostOriginalJobCustody,
    agent_recovery_decision::IndependentProcessCustody,
};
use crate::{
    chat_store::ChatStore,
    host_safe_continuation::{HostReviewedEvidence,HostSafeNext},
    reviewed_task_continuity::{reviewed_turn_continuity,ReviewedOperationContinuity},
};

const MAX_PRIOR_HOST_TURNS:usize=12;
const MAX_DISPLAYED_OPERATIONS_PER_TURN:usize=4;

#[derive(Clone,Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct HistoricalOperationProvenance {
    pub operation:&'static str,
    pub historical_effect_state:String,
    pub independent_oracle_at_observation:&'static str,
    pub independently_observed_sha256:Option<String>,
    pub original_model_call_linked:bool,
    pub completed_worker_task_linked:bool,
    pub descendant_custody:&'static str,
    pub host_safe_next_action:&'static str,
    pub current_source_freshness:&'static str,
    pub may_execute_or_replay:bool,
}

#[derive(Clone,Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct HistoricalTurnProvenance {
    pub original_human_turn_id:String,
    pub origin_bound:bool,
    pub journal_status:&'static str,
    pub reviewed_operation_count:usize,
    pub additional_operations_not_read:bool,
    pub host_safe_next_action:&'static str,
    pub operations:Vec<HistoricalOperationProvenance>,
}

#[derive(Clone,Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct HistoricalRecoveryProvenance {
    pub schema:&'static str,
    pub status:&'static str,
    pub turns:Vec<HistoricalTurnProvenance>,
    pub older_host_turns_not_inspected:bool,
    pub host_safe_next_action:&'static str,
    pub host_safe_next_explanation:&'static str,
    pub current_source_freshness:&'static str,
    pub execution_authority:&'static str,
    pub prior_grant_reusable:bool,
    pub automatic_retry_allowed:bool,
    pub historical_effect_may_be_replayed:bool,
    pub model_may_override_host_safe_decision:bool,
}

fn report(status:&'static str,turns:Vec<HistoricalTurnProvenance>,
    window_incomplete:bool,next:HostSafeNext)->HistoricalRecoveryProvenance {
    HistoricalRecoveryProvenance{
        schema:"lu-memory-loop02c-operator-provenance-v1",
        status,turns,older_host_turns_not_inspected:window_incomplete,
        host_safe_next_action:next.code(),
        host_safe_next_explanation:next.explanation(),
        current_source_freshness:"NOT_RECHECKED",
        execution_authority:"NOT_AUTHORIZED_FOR_EXECUTION",
        prior_grant_reusable:false,automatic_retry_allowed:false,
        historical_effect_may_be_replayed:false,
        model_may_override_host_safe_decision:false,
    }
}

/// This helper accepts an OS witness exclusively from the host's independent
/// provider. The BOOLEAN is never a Tauri/UI/model argument.
fn project_operation(op:&ReviewedOperationContinuity,full_journal:bool,
    origin_bound:bool,omitted:bool,kernel_empty:bool)
    ->(HistoricalOperationProvenance,HostSafeNext) {
    let semantic=op.outcome=="VERIFIED_AT_OBSERVATION"
        &&op.process_state=="APPLIED"
        &&op.original_model_call_linked
        &&op.worker_task_linked
        &&op.independently_observed_sha256.is_some();
    let decision=HostSafeNext::for_evidence(HostReviewedEvidence{
        origin_bound,full_journal_verified:full_journal,
        omitted_history:omitted,effect_applied:op.process_state=="APPLIED",
        independent_oracle:op.outcome,
        original_model_call_linked:op.original_model_call_linked,
        reviewed_worker_completed:op.worker_task_linked,
        independent_observed_digest_present:
            op.independently_observed_sha256.is_some(),
        full_descendant_custody_attested:kernel_empty,
    });
    (HistoricalOperationProvenance{
        operation:op.operation,historical_effect_state:op.process_state.clone(),
        independent_oracle_at_observation:op.outcome,
        independently_observed_sha256:if semantic&&full_journal{
            op.independently_observed_sha256.clone()
        }else{None},
        original_model_call_linked:op.original_model_call_linked,
        completed_worker_task_linked:op.worker_task_linked,
        descendant_custody:if kernel_empty&&semantic{
            "KERNEL_EMPTY_AT_OBSERVATION"
        }else{"NOT_INDEPENDENTLY_PROVEN"},
        host_safe_next_action:decision.code(),
        current_source_freshness:"NOT_RECHECKED",
        may_execute_or_replay:false,
    },decision)
}

/// Only the host-selected profile and authenticated ANSF original human turns
/// can select historical journal names. This is not a global-history clearance:
/// capped/unknown older history is explicitly declared unread and restricts
/// the global safe next action. No caller provides a historical turn ID.
pub(crate) fn read_original_recovery(
    profile:&Path,chat:&str,chats:&ChatStore,
)->Result<HistoricalRecoveryProvenance,String>{
    chats.history_window(chat,None,"older",1)?;
    let mut worst=HostSafeNext::RecheckSourceWithFreshHumanAuthorization;
    let Some(latest)=chats.latest_authenticated_user_turn_id(chat)else{
        return Ok(report("NO_HOST_ORIGINAL_TURNS",Vec::new(),false,
            HostSafeNext::ReconcileOriginalHistoryBeforeNewAction))
    };
    let older=chats.prior_authenticated_turn_ids(
        chat,&latest,MAX_PRIOR_HOST_TURNS
    )?;
    let window_incomplete=older.len()==MAX_PRIOR_HOST_TURNS;
    let mut turns=Vec::with_capacity(older.len()+1);
    turns.push(latest);
    turns.extend(older);
    let custody=HostOriginalJobCustody::from_host_profile(profile);
    let mut reviewed=Vec::new();
    for original_turn in turns {
        let Some(view)=reviewed_turn_continuity(
            profile,chat,&original_turn
        )? else {continue};
        let full_journal=view.journal_state=="VERIFIED_PAST_OPERATIONS";
        let omitted=view.operations.len()>MAX_DISPLAYED_OPERATIONS_PER_TURN;
        let mut turn_next=HostSafeNext::for_turn_without_complete_effects(
            view.origin_bound,full_journal,omitted,!view.operations.is_empty()
        );
        let mut ops=Vec::new();
        if view.origin_bound {
            for op in view.operations.iter()
                .take(MAX_DISPLAYED_OPERATIONS_PER_TURN) {
                let is_semantically_verified=
                    full_journal&&op.outcome=="VERIFIED_AT_OBSERVATION"
                    &&op.process_state=="APPLIED"
                    &&op.original_model_call_linked&&op.worker_task_linked
                    &&op.independently_observed_sha256.is_some();
                let kernel_empty=is_semantically_verified
                    &&custody.worker_and_descendants_ended(
                        chat,&original_turn,&op.call_id
                    );
                let (projection,decision)=project_operation(
                    op,full_journal,view.origin_bound,omitted,kernel_empty
                );
                turn_next=turn_next.restrict(decision);
                ops.push(projection);
            }
        }
        worst=worst.restrict(turn_next);
        reviewed.push(HistoricalTurnProvenance{
            original_human_turn_id:original_turn,
            origin_bound:view.origin_bound,
            journal_status:view.journal_state,
            reviewed_operation_count:view.operations.len(),
            additional_operations_not_read:omitted,
            host_safe_next_action:turn_next.code(),
            operations:ops,
        });
    }
    // No reviewed history in the bounded original-human window is NOT a
    // positive claim that previously reviewed workers are gone or verified.
    if reviewed.is_empty()||window_incomplete {
        worst=worst.restrict(HostSafeNext::ReconcileOriginalHistoryBeforeNewAction);
    }
    let status=if window_incomplete{
        "OLDER_ORIGINAL_TURNS_NOT_INSPECTED"
    }else if reviewed.is_empty(){
        "NO_REVIEWED_EFFECTS_IN_HOST_WINDOW"
    }else if worst==HostSafeNext::ReconcileOriginalHistoryBeforeNewAction{
        "INCOMPLETE_ORIGINAL_HISTORY"
    }else{
        "HISTORICAL_REVIEWED_EVIDENCE_ONLY"
    };
    Ok(report(status,reviewed,window_incomplete,worst))
}

#[cfg(test)]
mod tests{
    use super::*;
    use std::{fs,time::{SystemTime,UNIX_EPOCH}};
    use sha2::{Digest,Sha256};
    use lu_core::{
        contracts::{ProtoPayload,ToolInvocation},
        journal::{EffectJournal,IndependentOutcomeVerdict},
        tasks::TaskStore,
    };
    fn fixture()->(std::path::PathBuf,ChatStore,String,String){
        let root=std::env::temp_dir().join(format!(
            "lu-memory-loop02c-{}-{}",std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let chats=ChatStore::open(&root).unwrap();
        let chat=chats.create_chat().unwrap().chat_id;
        let turn=format!("g-{}","1".repeat(32));
        chats.append_user_with_turn_id(&chat,&turn,"Read historical Git evidence").unwrap();
        (root,chats,chat,turn)
    }
    fn call(id:&str)->ToolInvocation{
        ToolInvocation{
            call_id:id.into(),tool_id:"lu.reviewed.git.head".into(),
            workspace_id:Some("review".into()),
            input:ProtoPayload{type_url:"lu.test".into(),bytes:vec![1]},
        }
    }
    fn reviewed_git(root:&Path,chat:&str,turn:&str){
        let tasks=TaskStore::open(&root.join(format!("lu-reviewed-task-{turn}.sqlite"))).unwrap();
        tasks.bind_reviewed_human_turn(chat,turn).unwrap();
        tasks.create("op-gh",Some(turn),None).unwrap();
        tasks.start("op-gh").unwrap();
        tasks.complete("op-gh").unwrap();
        let effects=EffectJournal::open(
            &root.join(format!("lu-reviewed-effects-{turn}.sqlite"))
        ).unwrap();
        for id in ["op","op-gh"]{
            effects.prepare(&call(id)).unwrap();
            effects.mark_executing(id).unwrap();
            effects.mark_applied(id,b"historical-receipt").unwrap();
        }
        let observed=format!("{:x}",Sha256::digest(b"original-git-facts"));
        effects.record_independent_outcome("op-gh",b"historical-receipt",
            IndependentOutcomeVerdict::Verified,"GIT_HEAD_DIRECT_V1",
            Some(&observed)).unwrap();
    }

    #[test]
    fn original_sqlite_provenance_survives_restart_and_digest_tamper_stops(){
        let (root,chats,chat,turn)=fixture();
        reviewed_git(&root,&chat,&turn);
        let checked=read_original_recovery(&root,&chat,&chats).unwrap();
        assert_eq!(checked.turns.len(),1);
        assert_eq!(checked.turns[0].original_human_turn_id,turn);
        assert_eq!(checked.turns[0].operations[0].independent_oracle_at_observation,
            "VERIFIED_AT_OBSERVATION");
        assert_eq!(checked.turns[0].operations[0].descendant_custody,
            "NOT_INDEPENDENTLY_PROVEN");
        assert_eq!(checked.host_safe_next_action,
            "RECONCILE_OS_THEN_RECHECK_SOURCE_AND_REQUEST_FRESH_AUTHORIZATION");
        assert!(!checked.prior_grant_reusable);
        assert!(!checked.model_may_override_host_safe_decision);
        let old=serde_json::to_string(&checked).unwrap();
        assert!(!old.contains("historical-receipt"));
        assert!(!old.contains("Read historical Git evidence"));
        assert!(!old.contains("op-gh"));
        drop(chats);
        let reopened=ChatStore::open(&root).unwrap();
        assert_eq!(old,serde_json::to_string(
            &read_original_recovery(&root,&chat,&reopened).unwrap()
        ).unwrap());
        rusqlite::Connection::open(
            root.join(format!("lu-reviewed-effects-{turn}.sqlite"))
        ).unwrap().execute(
            "UPDATE lu_independent_outcomes SET receipt_sha256=?1 WHERE call_id='op-gh'",
            ["0".repeat(64)]
        ).unwrap();
        let altered=read_original_recovery(&root,&chat,&reopened).unwrap();
        assert_eq!(altered.turns[0].operations[0].independent_oracle_at_observation,
            "RECEIPT_MISMATCH");
        assert_eq!(altered.host_safe_next_action,
            "MANUAL_RECONCILIATION_BEFORE_NEW_ACTION");
        assert!(altered.turns[0].operations[0].independently_observed_sha256.is_none());
    }

    #[test]
    fn missing_original_effects_and_cross_chat_sources_never_claim_success(){
        let (root,chats,chat,turn)=fixture();
        let tasks=TaskStore::open(
            &root.join(format!("lu-reviewed-task-{turn}.sqlite"))
        ).unwrap();
        tasks.bind_reviewed_human_turn(&chat,&turn).unwrap();
        drop(tasks);
        let missing=read_original_recovery(&root,&chat,&chats).unwrap();
        assert_eq!(missing.status,"INCOMPLETE_ORIGINAL_HISTORY");
        assert_eq!(missing.host_safe_next_action,
            "RECONCILE_ORIGINAL_HISTORY_BEFORE_NEW_ACTION");
        assert!(missing.turns[0].operations.is_empty());
        assert!(!root.join(format!("lu-reviewed-effects-{turn}.sqlite")).exists());
        let other=chats.create_chat().unwrap().chat_id;
        let other_turn=format!("g-{}","2".repeat(32));
        chats.append_user_with_turn_id(&other,&other_turn,"unrelated").unwrap();
        let other_report=read_original_recovery(&root,&other,&chats).unwrap();
        assert!(other_report.turns.is_empty());
        assert_eq!(other_report.status,"NO_REVIEWED_EFFECTS_IN_HOST_WINDOW");
        assert!(!other_report.historical_effect_may_be_replayed);
    }

    #[test]
    fn verified_historical_source_and_os_witness_still_need_fresh_review(){
        let op=ReviewedOperationContinuity{
            operation:"Git HEAD",call_id:"internal".into(),
            independently_observed_sha256:Some("a".repeat(64)),
            process_state:"APPLIED".into(),outcome:"VERIFIED_AT_OBSERVATION",
            last_checkpoint:None,checkpoint_events:0,
            original_model_call_linked:true,worker_task_linked:true
        };
        let (verified,action)=project_operation(&op,true,true,false,true);
        assert_eq!(verified.descendant_custody,"KERNEL_EMPTY_AT_OBSERVATION");
        assert_eq!(action.code(),"RECHECK_SOURCE_AND_REQUEST_FRESH_AUTHORIZATION");
        assert!(!verified.may_execute_or_replay);
        let (_,os)=project_operation(&op,true,true,false,false);
        assert_eq!(os.code(),
            "RECONCILE_OS_THEN_RECHECK_SOURCE_AND_REQUEST_FRESH_AUTHORIZATION");
        let mut conflicted=op.clone();
        conflicted.outcome="CONFLICT";
        let (view,deny)=project_operation(&conflicted,true,true,false,true);
        assert_eq!(deny.code(),"MANUAL_RECONCILIATION_BEFORE_NEW_ACTION");
        assert!(view.independently_observed_sha256.is_none());
        let (_,unread)=project_operation(&op,true,true,true,true);
        assert_eq!(unread.code(),"RECONCILE_ORIGINAL_HISTORY_BEFORE_NEW_ACTION");
    }

    #[test]
    fn capped_original_human_history_is_marked_unread_not_globally_cleared(){
        let (root,chats,chat,_)=fixture();
        for n in 2..=15 {
            let turn=format!("g-{:032x}",n);
            chats.append_user_with_turn_id(&chat,&turn,"ordinary unrelated human turn").unwrap();
        }
        let report=read_original_recovery(&root,&chat,&chats).unwrap();
        assert!(report.older_host_turns_not_inspected);
        assert_eq!(report.status,"OLDER_ORIGINAL_TURNS_NOT_INSPECTED");
        assert_eq!(report.host_safe_next_action,
            "RECONCILE_ORIGINAL_HISTORY_BEFORE_NEW_ACTION");
        assert!(!report.automatic_retry_allowed);
    }
}
