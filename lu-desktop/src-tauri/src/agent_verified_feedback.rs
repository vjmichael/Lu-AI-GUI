//! MEMORY-LOOP-02: independent reviewed outcome feedback for the *next*
//! authenticated human reasoning turn, NOT tool authority or a source cache.
//!
//! A previous assistant reply, tool output, imported transcript or model
//! prediction cannot select an original turn. We look up previous turn IDs
//! only through the active chat's host-authenticated indexed ANSF originals.
//! Then we independently reopen the original, per-turn reviewed TaskStore,
//! effect journal, independent oracle and OS custody witness. No worker is
//! launched, no journal is updated, and no old tool grants are inherited.
use std::path::Path;
use serde_json::{json,Value};
use lu_core::{
    agent_os_custody::HostOriginalJobCustody,
    agent_recovery_decision::IndependentProcessCustody,
};
use crate::{
    chat_store::ChatStore,
    reviewed_task_continuity::{
        reviewed_turn_continuity,ReviewedOperationContinuity,
    },
};

const MAX_PRIOR_HUMAN_TURNS:usize=12;
const MAX_FEEDBACK_TURNS:usize=2;
const MAX_OPERATIONS_PER_TURN:usize=3;
const MAX_FEEDBACK_BYTES:usize=3400;

fn operation_feedback(
    op:&ReviewedOperationContinuity,
    all_effects_independent:bool,
    kernel_empty:bool,
)->Value{
    // Historical independent semantic observation and original worker/task
    // completion are necessary, but NOT sufficient to guarantee descendants
    // ended or the underlying Git/media source is still fresh today.
    let semantic=op.outcome=="VERIFIED_AT_OBSERVATION"&&
        op.process_state=="APPLIED"&&op.original_model_call_linked&&
        op.worker_task_linked&&op.independently_observed_sha256.is_some();
    let result=if !semantic{
        match op.outcome{
            "CONFLICT"=>"INDEPENDENT_SOURCE_CONFLICT",
            "UNSUPPORTED"=>"INDEPENDENT_SOURCE_UNSUPPORTED",
            _=>"UNCERTAIN_OR_UNVERIFIED",
        }
    }else if !all_effects_independent{
        "PARTIAL_TASK_REQUIRES_RECONCILIATION"
    }else if kernel_empty{
        "HISTORICALLY_VERIFIED_WITH_OS_EMPTY_WITNESS"
    }else{
        "SOURCE_VERIFIED_OS_CUSTODY_UNPROVEN"
    };
    json!({
        "reviewedOperation":op.operation,
        "historicalEffectState":op.process_state,
        "independentOracleAtObservation":op.outcome,
        // Include a non-executable digest of facts observed by the
        // independent source oracle, NOT raw model text/worker output.
        "independentObservedSha256":if semantic{
            op.independently_observed_sha256.as_deref()
        }else{None},
        "originalModelCallLinked":op.original_model_call_linked,
        "completedWorkerTaskLinked":op.worker_task_linked,
        "descendantCustody":if kernel_empty{
            "KERNEL_EMPTY_AT_OBSERVATION"
        }else{"NOT_INDEPENDENTLY_PROVEN"},
        "feedbackStatus":result,
        "currentSourceFreshness":"NOT_RECHECKED",
        "mayExecuteOrReplay":false
    })
}

/// Data-only snapshot for the CURRENT original human turn in a normal local
/// GGUF/plan reasoning request. No prompt phrase or model token chooses an
/// older turn; immutable ANSF source ordering does that. The host supplies
/// the installed profile and authenticates the active chat.
pub(crate) fn for_next_human_turn(
    chats:&ChatStore,profile:&Path,chat:&str,current_turn:&str,
    current_prompt:&str,
)->Result<Option<String>,String>{
    if chats.latest_authenticated_user_turn_id(chat).as_deref()!=Some(current_turn)
        ||chats.trusted_user_turn_text(chat,current_turn)
            .as_deref()!=Some(current_prompt){
        return Err("Next-turn feedback lacks a current authenticated human turn".into())
    }
    let older=chats.prior_authenticated_turn_ids(
        chat,current_turn,MAX_PRIOR_HUMAN_TURNS
    )?;
    let custody=HostOriginalJobCustody::from_host_profile(profile);
    let mut history=Vec::new();
    for turn in older{
        let Some(view)=reviewed_turn_continuity(profile,chat,&turn)? else{
            continue
        };
        let valid_origin=view.origin_bound;
        let fully_checked=view.journal_state=="VERIFIED_PAST_OPERATIONS";
        let mut custody_unverified=false;
        let ops=if valid_origin{
            view.operations.iter().take(MAX_OPERATIONS_PER_TURN).map(|op|{
                let kernel_empty=if op.outcome=="VERIFIED_AT_OBSERVATION"
                    &&op.process_state=="APPLIED"
                    &&op.original_model_call_linked&&op.worker_task_linked{
                    custody.worker_and_descendants_ended(
                        chat,&turn,&op.call_id
                    )
                }else{false};
                if !kernel_empty{custody_unverified=true}
                operation_feedback(op,fully_checked,kernel_empty)
            }).collect::<Vec<_>>()
        }else{Vec::new()};
        history.push(json!({
            "originalHumanTurnId":turn,
            "journalStatus":if valid_origin{
                view.journal_state
            }else{"ORIGIN_UNVERIFIED_RECONCILIATION_REQUIRED"},
            "independentOutcomeCount":ops.len(),
            "additionalOperationsNotRead":
                view.operations.len()>MAX_OPERATIONS_PER_TURN,
            "operations":ops,
            "needsHumanReconciliation":
                !valid_origin||!fully_checked||custody_unverified||
                view.operations.len()>MAX_OPERATIONS_PER_TURN
        }));
        if history.len()==MAX_FEEDBACK_TURNS{break}
    }
    if history.is_empty(){return Ok(None)}
    // Host-authored feedback is *history*, not a current truth claim. Source
    // freshness must be rechecked before any new external effect, even if the
    // previous independent oracle genuinely verified the earlier result.
    let envelope=json!({
        "schema":"lu-memory-loop02-reviewed-outcome-v1",
        "status":"HISTORICAL_INDEPENDENT_FEEDBACK_DATA_ONLY",
        "scope":"SAME_CHAT_HOST_AUTHENTICATED_PRIOR_TURNS",
        "historyMostRecentFirst":history,
        "historicalEffectMayBeReplayed":false,
        "priorGrantReusable":false,
        "executionAuthority":"NOT_AUTHORIZED_FOR_EXECUTION",
        "automaticRetryAllowed":false,
        "currentSourceFreshness":"NOT_RECHECKED",
        "nextOperation":"Requires a NEW current human resource review, fresh authority, source/goal verification, independent effect oracle and OS process custody."
    });
    let encoded=serde_json::to_string(&envelope)
        .map_err(|_|"Cannot serialize historical independent feedback")?;
    if encoded.len()>MAX_FEEDBACK_BYTES{
        return Err("Historical feedback exceeds the bounded model window".into())
    }
    Ok(Some(encoded))
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
    fn sandbox()->std::path::PathBuf{
        std::env::temp_dir().join(format!(
            "lu-memory-loop02-{}-{}",std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ))
    }
    fn test_invocation(id:&str)->ToolInvocation{
        ToolInvocation{
            call_id:id.into(),tool_id:"lu.reviewed.git.head".into(),
            workspace_id:Some("test".into()),
            input:ProtoPayload{type_url:"lu.test.reviewed".into(),bytes:vec![5]},
        }
    }
    #[test]
    fn source_oracle_verdict_and_kernel_custody_are_separate(){
        use crate::reviewed_task_continuity::ReviewedOperationContinuity;
        let basic=ReviewedOperationContinuity{
            operation:"Git HEAD",call_id:"call-gh".into(),
            independently_observed_sha256:Some("b".repeat(64)),
            process_state:"APPLIED".into(),outcome:"VERIFIED_AT_OBSERVATION",
            last_checkpoint:None,checkpoint_events:0,
            original_model_call_linked:true,worker_task_linked:true
        };
        let source_only=operation_feedback(&basic,true,false);
        assert_eq!(source_only["feedbackStatus"],
            "SOURCE_VERIFIED_OS_CUSTODY_UNPROVEN");
        assert_eq!(source_only["descendantCustody"],
            "NOT_INDEPENDENTLY_PROVEN");
        assert_eq!(source_only["currentSourceFreshness"],"NOT_RECHECKED");
        let with_kernel=operation_feedback(&basic,true,true);
        assert_eq!(with_kernel["feedbackStatus"],
            "HISTORICALLY_VERIFIED_WITH_OS_EMPTY_WITNESS");
        let partial=operation_feedback(&basic,false,true);
        assert_eq!(partial["feedbackStatus"],
            "PARTIAL_TASK_REQUIRES_RECONCILIATION");
        for (outcome,status) in [
            ("CONFLICT","INDEPENDENT_SOURCE_CONFLICT"),
            ("UNSUPPORTED","INDEPENDENT_SOURCE_UNSUPPORTED"),
            ("RECEIPT_MISMATCH","UNCERTAIN_OR_UNVERIFIED"),
            ("NOT_INDEPENDENTLY_CHECKED","UNCERTAIN_OR_UNVERIFIED"),
        ]{
            let mut invalid=basic.clone();
            invalid.outcome=outcome;
            let shown=operation_feedback(&invalid,false,false);
            assert_eq!(shown["feedbackStatus"],status);
            assert!(shown["independentObservedSha256"].is_null());
            assert_eq!(shown["mayExecuteOrReplay"],false);
        }
    }
    #[test]
    fn authenticated_original_feedback_persists_after_restart_and_fails_on_tamper(){
        let root=sandbox();fs::create_dir_all(&root).unwrap();
        let chats=ChatStore::open(&root).unwrap();
        let chat=chats.create_chat().unwrap().chat_id;
        let previous=format!("g-{}","1".repeat(32));
        let current=format!("g-{}","2".repeat(32));
        let other=format!("g-{}","3".repeat(32));
        chats.append_user_with_turn_id(&chat,&previous,
            "Please inspect the reviewed Git source").unwrap();
        let taskfile=root.join(format!("lu-reviewed-task-{previous}.sqlite"));
        let effectfile=root.join(format!("lu-reviewed-effects-{previous}.sqlite"));
        let tasks=TaskStore::open(&taskfile).unwrap();
        tasks.bind_reviewed_human_turn(&chat,&previous).unwrap();
        tasks.create("review-gh",Some(&previous),None).unwrap();
        tasks.start("review-gh").unwrap();
        tasks.complete("review-gh").unwrap();
        let effects=EffectJournal::open(&effectfile).unwrap();
        for id in ["review","review-gh"]{
            effects.prepare(&test_invocation(id)).unwrap();
            effects.mark_executing(id).unwrap();
            effects.mark_applied(id,b"bounded-native-receipt").unwrap();
        }
        let observed=format!("{:x}",Sha256::digest(b"independent-source-fact"));
        effects.record_independent_outcome("review-gh",
            b"bounded-native-receipt",IndependentOutcomeVerdict::Verified,
            "GIT_HEAD_DIRECT_V1",Some(&observed)).unwrap();
        let prompt="Explain our previous verified result";
        chats.append_user_with_turn_id(&chat,&current,prompt).unwrap();
        assert_eq!(chats.prior_authenticated_turn_ids(&chat,&current,12).unwrap(),
            vec![previous.clone()]);
        let before=for_next_human_turn(&chats,&root,&chat,&current,prompt)
            .unwrap().expect("original checked reviewed turn");
        let document:Value=serde_json::from_str(&before).unwrap();
        let operation=&document["historyMostRecentFirst"][0]["operations"][0];
        assert_eq!(operation["independentOracleAtObservation"],
            "VERIFIED_AT_OBSERVATION");
        assert_eq!(operation["feedbackStatus"],
            "SOURCE_VERIFIED_OS_CUSTODY_UNPROVEN");
        assert_eq!(operation["independentObservedSha256"],observed);
        assert_eq!(document["executionAuthority"],"NOT_AUTHORIZED_FOR_EXECUTION");
        assert_eq!(document["automaticRetryAllowed"],false);
        assert!(!before.contains("bounded-native-receipt"));
        assert!(!before.contains("Please inspect the reviewed Git source"));
        assert!(!before.contains("review-gh"));
        assert!(for_next_human_turn(&chats,&root,&chat,&other,prompt).is_err());
        drop(effects);drop(tasks);drop(chats);
        let chats=ChatStore::open(&root).unwrap();
        let restarted=for_next_human_turn(&chats,&root,&chat,&current,prompt)
            .unwrap().unwrap();
        assert_eq!(before,restarted,"memory projection must survive SQLite restart");
        // The independently recorded digest no longer matches the real
        // historical effect: past APPLIED must NOT become verified feedback.
        rusqlite::Connection::open(&effectfile).unwrap().execute(
            "UPDATE lu_independent_outcomes SET receipt_sha256=?1
             WHERE call_id='review-gh'",
            ["0".repeat(64)]
        ).unwrap();
        let changed=for_next_human_turn(&chats,&root,&chat,&current,prompt)
            .unwrap().unwrap();
        let damaged:Value=serde_json::from_str(&changed).unwrap();
        assert_eq!(
            damaged["historyMostRecentFirst"][0]["operations"][0]["feedbackStatus"],
            "UNCERTAIN_OR_UNVERIFIED"
        );
        assert!(damaged["historyMostRecentFirst"][0]["operations"][0]
            ["independentObservedSha256"].is_null());
        drop(chats);fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn no_cross_chat_feedback_and_no_original_journal_creation(){
        let root=sandbox();fs::create_dir_all(&root).unwrap();
        let chats=ChatStore::open(&root).unwrap();
        let one=chats.create_chat().unwrap().chat_id;
        let two=chats.create_chat().unwrap().chat_id;
        let past=format!("g-{}","4".repeat(32));
        let now=format!("g-{}","5".repeat(32));
        chats.append_user_with_turn_id(&one,&past,"Another chat's operation")
            .unwrap();
        chats.append_user_with_turn_id(&two,&now,"This chat has no prior effects")
            .unwrap();
        assert!(for_next_human_turn(
            &chats,&root,&two,&now,"This chat has no prior effects"
        ).unwrap().is_none());
        assert!(!root.join(format!("lu-reviewed-task-{past}.sqlite")).exists());
        assert!(for_next_human_turn(
            &chats,&root,&one,&now,"This chat has no prior effects"
        ).is_err());
        drop(chats);fs::remove_dir_all(root).unwrap();
    }
}
