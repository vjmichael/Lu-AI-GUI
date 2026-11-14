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
    host_safe_continuation::{HostReviewedEvidence,HostSafeNext},
    chat_store::ChatStore,
    reviewed_task_continuity::{
        reviewed_turn_continuity,ReviewedOperationContinuity,
    },
};

const MAX_PRIOR_HUMAN_TURNS:usize=12;
const MAX_FEEDBACK_TURNS:usize=2;
// Unread operations are explicitly marked and force a reconciliation gate;
 // bounded context must never silently promote an incomplete turn.
const MAX_OPERATIONS_PER_TURN:usize=2;
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
    // A model may describe the host decision, but cannot choose, weaken, or
    // override it. Every historical row only increases recovery severity.
    let mut safe_next=HostSafeNext::RecheckSourceWithFreshHumanAuthorization;
    for turn in older{
        let Some(view)=reviewed_turn_continuity(profile,chat,&turn)? else{
            continue
        };
        let valid_origin=view.origin_bound;
        let fully_checked=view.journal_state=="VERIFIED_PAST_OPERATIONS";
        let omitted_history=view.operations.len()>MAX_OPERATIONS_PER_TURN;
        let mut turn_safe_next=HostSafeNext::for_turn_without_complete_effects(
            valid_origin,fully_checked,omitted_history,!view.operations.is_empty()
        );
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
                let decision=HostSafeNext::for_evidence(HostReviewedEvidence{
                    origin_bound:valid_origin,
                    full_journal_verified:fully_checked,
                    omitted_history,
                    effect_applied:op.process_state=="APPLIED",
                    independent_oracle:op.outcome,
                    original_model_call_linked:op.original_model_call_linked,
                    reviewed_worker_completed:op.worker_task_linked,
                    independent_observed_digest_present:
                        op.independently_observed_sha256.is_some(),
                    full_descendant_custody_attested:kernel_empty,
                });
                turn_safe_next=turn_safe_next.restrict(decision);
                operation_feedback(op,fully_checked,kernel_empty)
            }).collect::<Vec<_>>()
        }else{Vec::new()};
        safe_next=safe_next.restrict(turn_safe_next);
        history.push(json!({
            "originalHumanTurnId":turn,
            "hostSafeNextAction":turn_safe_next.code(),
            "journalStatus":if valid_origin{
                view.journal_state
            }else{"ORIGIN_UNVERIFIED_RECONCILIATION_REQUIRED"},
            "independentOutcomeCount":ops.len(),
            "additionalOperationsNotRead":omitted_history,
            "operations":ops,
            "needsHumanReconciliation":
                !valid_origin||!fully_checked||custody_unverified||
                omitted_history
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
        // This is the HOST'S own safe follow-up classification, not an
        // instruction synthesized by the AI or a reusable permission.
        "hostSafeNextAction":safe_next.code(),
        "hostSafeNextActionExplanation":safe_next.explanation(),
        "modelMayOverrideHostSafeDecision":false,
        "nextOperation":"Only the native Lu host may review any NEW effect; historical model output is not an execution grant."
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
        assert_eq!(document["hostSafeNextAction"],
            "RECONCILE_OS_THEN_RECHECK_SOURCE_AND_REQUEST_FRESH_AUTHORIZATION");
        assert_eq!(document["historyMostRecentFirst"][0]["hostSafeNextAction"],
            "RECONCILE_OS_THEN_RECHECK_SOURCE_AND_REQUEST_FRESH_AUTHORIZATION");
        assert_eq!(document["modelMayOverrideHostSafeDecision"],false);
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
        assert_eq!(damaged["hostSafeNextAction"],
            "MANUAL_RECONCILIATION_BEFORE_NEW_ACTION");
        assert_eq!(damaged["modelMayOverrideHostSafeDecision"],false);
        assert!(damaged["historyMostRecentFirst"][0]["operations"][0]
            ["independentObservedSha256"].is_null());
        drop(chats);fs::remove_dir_all(root).unwrap();
    }
    /// Test-only source producer for REAL local-Qwen comprehension scoring.
    /// The source verdict is produced by Lu's independent native Git oracle,
    /// NOT a hand-written "verified" string or a model-made tool receipt.
    /// The CI harness copies the exact post-restart model feedback out of
    /// ChatStore/TaskStore/EffectJournal, then loads real Qwen3 GGUF.
    #[test]
    #[ignore="requires explicit CI fixture output path and real native Git"]
    fn memory_loop02_real_git_oracle_feedback_qwen_fixture_export(){
        use std::{path::PathBuf,process::Command};
        use lu_core::{
            native_app_typed_git::GitReadOperation,
            reviewed_outcome_oracle::verify_git,
        };
        let output=PathBuf::from(
            std::env::var_os("LU_MEMORY_LOOP02_QWEN_FIXTURE_OUTPUT")
                .expect("CI-only native source fixture output path required")
        );
        assert!(output.is_absolute(),"Native fixture output must be host-owned absolute");
        let root=sandbox();fs::create_dir_all(&root).unwrap();
        let chats=ChatStore::open(&root).unwrap();
        let mut cases=Vec::<(String,String,String,String,String)>::new();
        for (idx,name) in [
            "source_verified","independent_conflict","interrupted_effect"
        ].iter().enumerate(){
            let chat=chats.create_chat().unwrap().chat_id;
            let original=format!("g-{:032x}",idx*2+1);
            let next=format!("g-{:032x}",idx*2+2);
            chats.append_user_with_turn_id(&chat,&original,
                "Please inspect the current Git HEAD using reviewed native execution.")
                .unwrap();
            let taskfile=root.join(format!("lu-reviewed-task-{original}.sqlite"));
            let effectfile=root.join(format!("lu-reviewed-effects-{original}.sqlite"));
            let tasks=TaskStore::open(&taskfile).unwrap();
            tasks.bind_reviewed_human_turn(&chat,&original).unwrap();
            tasks.create("review-gh",Some(&original),None).unwrap();
            tasks.start("review-gh").unwrap();
            let effects=EffectJournal::open(&effectfile).unwrap();
            let repository=root.join(format!("actual-git-{idx}"));
            fs::create_dir_all(&repository).unwrap();
            let git=|args:&[&str]|{
                let run=Command::new("git").args(args).current_dir(&repository)
                    .output().expect("Real OS Git required for Qwen fixture");
                assert!(run.status.success(),"Real Git failed: {}",String::from_utf8_lossy(&run.stderr));
                String::from_utf8(run.stdout).unwrap().trim().to_owned()
            };
            git(&["init","-q"]);
            git(&["-c","user.name=LuTest","-c","user.email=lu@example.test",
                "commit","--allow-empty","-q","-m","Real original source"]);
            let head=git(&["rev-parse","HEAD"]);
            assert!(matches!(head.len(),40|64));
            effects.prepare(&test_invocation("review-gh")).unwrap();
            effects.mark_executing("review-gh").unwrap();
            if *name=="interrupted_effect"{
                effects.mark_uncertain("review-gh").unwrap();
            }else{
                tasks.complete("review-gh").unwrap();
                effects.prepare(&test_invocation("review")).unwrap();
                effects.mark_executing("review").unwrap();
                let original_receipt=b"actual-native-git-reviewed-effect";
                effects.mark_applied("review",original_receipt).unwrap();
                effects.mark_applied("review-gh",original_receipt).unwrap();
                // Corrupt ONLY the worker's asserted result in the conflict
                // case. The independent Git oracle reads actual .git files.
                let claimed=if *name=="independent_conflict"{
                    "0".repeat(head.len())
                }else{head};
                let worker=serde_json::json!({
                    "kind":"head_commit","headCommit":claimed
                });
                let oracle=verify_git(&repository,GitReadOperation::HeadCommit,&worker);
                if *name=="source_verified"{
                    assert_eq!(oracle.verdict,IndependentOutcomeVerdict::Verified)
                }else{
                    assert_eq!(oracle.verdict,IndependentOutcomeVerdict::Conflict)
                }
                effects.record_independent_outcome(
                    "review-gh",original_receipt,oracle.verdict,oracle.method,
                    oracle.observed_sha256.as_deref()
                ).unwrap();
            }
            let prompt=String::from(
                "Based on the previous reviewed Git operation's actual evidence,                  how confident should I be and what is the safe next step?"
            );
            chats.append_user_with_turn_id(&chat,&next,&prompt).unwrap();
            let expect=match *name{
                "source_verified"=>"VERIFIED_AT_OBSERVATION",
                "independent_conflict"=>"CONFLICT",
                _=>"NOT_INDEPENDENTLY_CHECKED"
            };
            cases.push((name.to_string(),chat,next,prompt,expect.into()));
            drop(effects);drop(tasks);
        }
        drop(chats);
        // Deliberately reopen all native ANSF original books and original
        // TaskStore + EffectJournal readers through the real host facade.
        let reopened=ChatStore::open(&root).unwrap();
        let mut samples=Vec::new();
        for (name,chat,turn,prompt,expected) in cases{
            let encoded=for_next_human_turn(
                &reopened,&root,&chat,&turn,&prompt
            ).unwrap().expect("Real original source reviewed feedback absent");
            let projection:Value=serde_json::from_str(&encoded).unwrap();
            let item=&projection["historyMostRecentFirst"][0]["operations"][0];
            assert_eq!(item["independentOracleAtObservation"],expected);
            assert_eq!(projection["priorGrantReusable"],false);
            assert_eq!(projection["automaticRetryAllowed"],false);
            assert_eq!(item["descendantCustody"],"NOT_INDEPENDENTLY_PROVEN");
            let baseline=match name.as_str(){
                "source_verified"=>"SOURCE_VERIFIED_OS_CUSTODY_UNPROVEN",
                "independent_conflict"=>"INDEPENDENT_SOURCE_CONFLICT",
                _=>"UNCERTAIN_OR_UNVERIFIED"
            };
            assert_eq!(item["feedbackStatus"],baseline);
            let expected_host_action=match name.as_str(){
                "source_verified"=>
                    "RECONCILE_OS_THEN_RECHECK_SOURCE_AND_REQUEST_FRESH_AUTHORIZATION",
                _=>"MANUAL_RECONCILIATION_BEFORE_NEW_ACTION"
            };
            assert_eq!(projection["hostSafeNextAction"],expected_host_action);
            assert_eq!(projection["modelMayOverrideHostSafeDecision"],false);
            samples.push(serde_json::json!({
                "case":name,
                "currentHumanPrompt":prompt,
                "hostFeedback":projection,
                "expectedOracleAtObservation":expected,
                "expectedFeedbackStatus":baseline,
                "expectedOsCustody":"UNPROVEN",
                "expectedSourceFreshness":"NOT_RECHECKED",
                "expectedMayReuseGrant":false,
                "expectedMayAutoRetry":false
            }));
        }
        let export=serde_json::json!({
            "schema":"lu-memory-loop02-actual-native-oracle-qwen-fixtures-v1",
            "evidence":"REAL_GIT_INDEPENDENT_ON_DISK_ORACLE_AND_REAL_SQLITE_REOPEN_FIXTURE_NOT_INSTALLED_LU_APP",
            "cases":samples,
            "modelExecutionAuthority":"NONE",
        });
        let bytes=serde_json::to_vec_pretty(&export).unwrap();
        assert!(bytes.len()<=24*1024);
        fs::write(&output,&bytes).unwrap();
        println!("MEMORY_LOOP02_NATIVE_GIT_ORACLE_SOURCE_FIXTURES_CREATED={} bytes={} path={}",
            export["cases"].as_array().unwrap().len(),bytes.len(),output.display());
        drop(reopened);
        fs::remove_dir_all(root).unwrap();
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
