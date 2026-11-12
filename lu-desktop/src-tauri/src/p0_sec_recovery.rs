//! P0-SEC04: read-only operator security recovery, projected solely from
//! original ANSF chat identity, original reviewed worker/effect journals,
//! durable task uncertainty and P0-AUTH cancellation intents.
//!
//! No worker launch, cancellation replay, resume, permission restore or
//! automatic effect reconciliation is available from this module.
use std::path::Path;
use serde::Serialize;
use lu_core::tasks::TaskStore;
use crate::{
    chat_store::ChatStore,
    p0_auth_custody::HostCancelHistory,
    reviewed_task_continuity::sec04_reviewed_turn_security,
};

const MAX_PRIOR_ORIGINAL_TURNS:usize=24;
const MAX_VISIBLE_PROBLEMS:usize=48;

#[derive(Clone,Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct SecurityRecoveryProblem{
    pub source:&'static str,
    pub original_human_turn_id:Option<String>,
    pub task_id:Option<String>,
    pub state:String,
    pub reason:String,
}
#[derive(Clone,Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct SecurityRecoveryView{
    pub status:&'static str,
    pub problems:Vec<SecurityRecoveryProblem>,
    pub additional_problems_not_displayed:bool,
    pub older_original_turns_not_inspected:bool,
    pub pending_cancel_intents:usize,
    pub cancellation_outcomes_still_require_review:usize,
    pub original_journals_examined:usize,
    pub old_os_custody_independently_proven:bool,
    pub prior_authority_reusable:bool,
    pub automatic_retry_allowed:bool,
    pub human_reconciliation_required:bool,
}
impl SecurityRecoveryView{
    pub(crate) fn unavailable()->Self{
        Self{
            status:"ORIGINAL_SECURITY_RECOVERY_UNAVAILABLE_RECONCILE",
            problems:vec![],additional_problems_not_displayed:true,
            older_original_turns_not_inspected:true,
            pending_cancel_intents:0,
            cancellation_outcomes_still_require_review:0,
            original_journals_examined:0,
            old_os_custody_independently_proven:false,
            prior_authority_reusable:false,automatic_retry_allowed:false,
            human_reconciliation_required:true,
        }
    }
}
fn push_problem(
    view:&mut SecurityRecoveryView,record:SecurityRecoveryProblem,
){
    if view.problems.len()<MAX_VISIBLE_PROBLEMS{
        view.problems.push(record)
    }else{
        view.additional_problems_not_displayed=true
    }
}
pub(crate) fn for_original_chat(
    profile:&Path,chat:&str,chats:&ChatStore,
    background:&TaskStore,cancels:&[HostCancelHistory],
)->Result<SecurityRecoveryView,String>{
    // Establish identity from the original ANSF catalog before considering
    // any original task/effect journal name. No model-supplied journal ID.
    chats.require(chat)?;
    let mut output=SecurityRecoveryView{
        status:"BOUNDED_ORIGINAL_HISTORY_NOT_OS_CUSTODY_PROOF",
        problems:vec![],additional_problems_not_displayed:false,
        older_original_turns_not_inspected:false,
        pending_cancel_intents:0,
        cancellation_outcomes_still_require_review:0,
        original_journals_examined:0,
        old_os_custody_independently_proven:false,
        prior_authority_reusable:false,automatic_retry_allowed:false,
        human_reconciliation_required:false,
    };
    for event in cancels{
        if event.status=="CANCELLATION_REQUESTED_OUTCOME_UNKNOWN"||
            event.status=="ORIGINAL_CANCEL_INTENT_UNAVAILABLE"{
            output.pending_cancel_intents+=1;
        }else if event.status=="CANCEL_RETURNED_REQUIRES_OS_AND_EFFECT_RECONCILIATION"{
            output.cancellation_outcomes_still_require_review+=1;
        }
    }
    let (background_rows,additional)=background.security_recovery_rows(
        Some(chat),64
    ).map_err(|_|"Original native background recovery unavailable")?;
    output.additional_problems_not_displayed|=additional;
    for row in background_rows{
        push_problem(&mut output,SecurityRecoveryProblem{
            source:"original_background_tasks",
            original_human_turn_id:None,
            task_id:Some(row.task_id),
            state:row.state.as_str().into(),
            reason:row.custody_reason.unwrap_or_else(||
                "INTERRUPTED_OS_TERMINATION_NOT_PROVEN".into()),
        });
    }
    if let Some(latest)=chats.latest_authenticated_user_turn_id(chat){
        let previous=chats.prior_authenticated_turn_ids(
            chat,&latest,MAX_PRIOR_ORIGINAL_TURNS
        )?;
        output.older_original_turns_not_inspected=
            previous.len()==MAX_PRIOR_ORIGINAL_TURNS;
        let mut ids=Vec::with_capacity(previous.len()+1);
        ids.push(latest);
        ids.extend(previous);
        for turn in ids{
            let Some(original)=sec04_reviewed_turn_security(
                profile,chat,&turn
            )? else{continue};
            output.original_journals_examined+=1;
            output.additional_problems_not_displayed|=
                original.additional_native_tasks_not_inspected;
            if !original.original_origin_bound{
                push_problem(&mut output,SecurityRecoveryProblem{
                    source:"original_reviewed_journals",
                    original_human_turn_id:Some(turn),
                    task_id:None,
                    state:original.journal_status,
                    reason:"ORIGINAL_AUTHORITY_BINDING_UNVERIFIED".into(),
                });
                continue
            }
            if original.historical_effects_need_review{
                push_problem(&mut output,SecurityRecoveryProblem{
                    source:"original_reviewed_effects",
                    original_human_turn_id:Some(turn.clone()),
                    task_id:None,
                    state:original.journal_status.clone(),
                    reason:"HISTORICAL_EFFECTS_REQUIRE_RECONCILIATION".into(),
                });
            }
            for record in original.interrupted_or_uncertain_tasks{
                push_problem(&mut output,SecurityRecoveryProblem{
                    source:"original_reviewed_tasks",
                    original_human_turn_id:Some(turn.clone()),
                    task_id:Some(record.task_id),
                    state:record.state,
                    reason:record.custody_reason.unwrap_or_else(||
                        "INTERRUPTED_OS_TERMINATION_NOT_PROVEN".into()),
                });
            }
        }
    }else{
        // A legacy imported or absent original human-turn index cannot
        // prove the absence of reviewed effects in older profile files.
        output.older_original_turns_not_inspected=true;
    }
    output.human_reconciliation_required=
        !output.problems.is_empty()
        ||output.additional_problems_not_displayed
        ||output.older_original_turns_not_inspected
        ||output.pending_cancel_intents>0
        ||output.cancellation_outcomes_still_require_review>0;
    output.status=if output.human_reconciliation_required{
        "ORIGINAL_SECURITY_RECONCILIATION_REQUIRED"
    }else{
        "NO_UNRESOLVED_RECORDS_IN_BOUNDED_ORIGINAL_WINDOW"
    };
    Ok(output)
}

#[cfg(test)]
mod tests{
    use super::*;
    use std::{fs,time::{SystemTime,UNIX_EPOCH}};
    use lu_core::{
        journal::{EffectJournal,PrepareOutcome},
        contracts::{ProtoPayload,ToolInvocation},
        tasks::{TaskState,TaskStore},
    };
    use crate::reviewed_app_recovery::reconcile_reviewed_application_journals;

    fn temp()->std::path::PathBuf{
        let root=std::env::temp_dir().join(format!(
            "lu-sec04-{}-{}",std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir_all(&root).unwrap();root
    }
    fn original(ch:char,prefix:&str)->String{
        format!("{}{}",prefix,ch.to_string().repeat(32))
    }
    #[test]
    fn sec04_original_guardian_loss_effect_and_restart_are_never_cleared(){
        let root=temp();
        let chats=ChatStore::open(&root).unwrap();
        let chat=chats.create_chat().unwrap().chat_id;
        let other=chats.create_chat().unwrap().chat_id;
        let turn=original('a',"g-");
        chats.append_user_with_turn_id(&chat,&turn,"Approved original").unwrap();
        let task_path=root.join(format!("lu-reviewed-task-{turn}.sqlite"));
        let effect_path=root.join(format!("lu-reviewed-effects-{turn}.sqlite"));
        {
            let tasks=TaskStore::open(&task_path).unwrap();
            tasks.bind_reviewed_human_turn(&chat,&turn).unwrap();
            tasks.create("sec04-kernel-worker",Some(&turn),None).unwrap();
            tasks.start("sec04-kernel-worker").unwrap();
            tasks.record_process_uncertain(
                "sec04-kernel-worker","LINUX_GUARDIAN_LOST").unwrap();
            let effects=EffectJournal::open(&effect_path).unwrap();
            let call=ToolInvocation{
                call_id:"sec04-review-call".into(),
                tool_id:"lu.reviewed.git.head".into(),
                workspace_id:Some(turn.clone()),
                input:ProtoPayload{
                    type_url:"lu.test.request".into(),bytes:vec![1],
                },
            };
            assert_eq!(effects.prepare(&call).unwrap(),PrepareOutcome::New);
            effects.mark_executing(&call.call_id).unwrap();
        }
        let first=reconcile_reviewed_application_journals(&root).unwrap();
        assert_eq!(first.interrupted_tasks,1);
        assert_eq!(first.uncertain_effects,1);
        let recovered=TaskStore::open(&task_path).unwrap();
        assert_eq!(recovered.process_uncertainty("sec04-kernel-worker")
            .unwrap().as_deref(),Some("LINUX_GUARDIAN_LOST"));
        assert_eq!(recovered.get("sec04-kernel-worker").unwrap()
            .unwrap().state,TaskState::Interrupted);
        assert!(recovered.resume("sec04-kernel-worker").is_err());
        let cancellation=crate::p0_auth_custody::HostCancelEvent{
            schema:crate::p0_auth_custody::SCHEMA.into(),
            attempt_id:original('b',"au-"),
            chat_id:chat.clone(),phase:"REQUESTED".into(),
            domains:vec![],authority_restored:false,
            automatic_retry_allowed:false,
            os_custody_independently_verified:false,
        };
        chats.append_p0_auth_cancel(&cancellation).unwrap();
        let cancel_history=chats.p0_auth_cancel_history(&chat).unwrap();
        let background=TaskStore::open(&root.join("lu-background-tasks.sqlite")).unwrap();
        let view=for_original_chat(
            &root,&chat,&chats,&background,&cancel_history).unwrap();
        assert_eq!(view.status,"ORIGINAL_SECURITY_RECONCILIATION_REQUIRED");
        assert!(view.problems.iter().any(|row|
            row.reason=="LINUX_GUARDIAN_LOST"
            &&row.original_human_turn_id.as_deref()==Some(turn.as_str())));
        assert!(view.problems.iter().any(|row|
            row.reason=="HISTORICAL_EFFECTS_REQUIRE_RECONCILIATION"));
        assert!(!view.prior_authority_reusable&&!view.automatic_retry_allowed);
        assert_eq!(view.pending_cancel_intents,1);
        let denied=crate::reviewed_task_continuity::sec04_reviewed_turn_security(
            &root,&other,&turn).unwrap().unwrap();
        assert!(!denied.original_origin_bound);
        assert_eq!(denied.journal_status,"ORIGINAL_ORIGIN_MISMATCH");
        let other_view=for_original_chat(
            &root,&other,&chats,&background,&[]).unwrap();
        assert!(!other_view.problems.iter().any(|r|
            r.original_human_turn_id.as_deref()==Some(turn.as_str())));
        drop(recovered);drop(background);drop(chats);
        let again=reconcile_reviewed_application_journals(&root).unwrap();
        assert_eq!(again.interrupted_tasks,0);
        assert_eq!(again.uncertain_effects,0);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn sec04_pending_uncertainty_is_interrupted_and_unresumable(){
        let store=TaskStore::open(Path::new(":memory:")).unwrap();
        store.create("sec04-pre-admission",Some("c-target"),None).unwrap();
        store.record_process_uncertain("sec04-pre-admission","OS_CUSTODY_UNVERIFIED").unwrap();
        assert!(store.start("sec04-pre-admission").is_err());
        assert_eq!(store.reconcile_interrupted().unwrap(),1);
        assert_eq!(store.get("sec04-pre-admission").unwrap().unwrap().state,
            TaskState::Interrupted);
        assert!(store.resume("sec04-pre-admission").is_err());
        let (rows,more)=store.security_recovery_rows(Some("c-target"),64).unwrap();
        assert!(!more);
        assert_eq!(rows.len(),1);
        assert_eq!(rows[0].custody_reason.as_deref(),Some("OS_CUSTODY_UNVERIFIED"));
        assert!(store.security_recovery_rows(Some("c-other"),64)
            .unwrap().0.is_empty());
    }
}
