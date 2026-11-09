//! MEMORY-LOOP-03A — current human goal/clarification + historical recovery
//! preflight for a real loaded-model reasoning turn. Pure first-party host
//! policy, independent of the model, any scheduler, and UI-selected history.
//!
//! The caller MUST derive facts from original ANSF GoalSnapshot and the
//! host's 02C original TaskStore/EffectJournal/oracle/Job-cgroup view.
//! This is a STATUS CAPSULE, not an action grant or a live source check.
#![allow(dead_code)]

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(crate) enum GoalRead {
    Unavailable,
    Known {
        goal_id_present:bool,
        goal_revision_present:bool,
        pending:usize,
        deferred:usize,
        unknown_question_status:bool,
    },
}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(crate) enum RecoveryRead<'a> {
    Unavailable,
    Original {
        status:&'a str,
        host_safe_next_action:&'a str,
        older_human_turns_unread:bool,
    },
}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
enum GoalGate {
    Unauthenticated, Unavailable, Inconsistent, NoSavedGoal,
    PendingQuestion, DeferredQuestion, ReadyForReview,
}
impl GoalGate {
    const fn code(self)->&'static str {
        match self{
            Self::Unauthenticated=>"CURRENT_HUMAN_TURN_UNAUTHENTICATED",
            Self::Unavailable=>"ORIGINAL_GOAL_MEMORY_UNAVAILABLE",
            Self::Inconsistent=>"ORIGINAL_GOAL_STATE_INCONSISTENT",
            Self::NoSavedGoal=>"NO_SAVED_GOAL_GENERAL_CHAT",
            Self::PendingQuestion=>"PENDING_ORIGINAL_HUMAN_CLARIFICATION",
            Self::DeferredQuestion=>"DEFERRED_ORIGINAL_HUMAN_CLARIFICATION",
            Self::ReadyForReview=>"CURRENT_GOAL_REVIEWABLE_ONLY",
        }
    }
}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
enum RecoveryGate {
    Unavailable, OlderOriginalsUnread, MissingOriginal,
    IndependentConflict, OsCustodyUnproven, FreshReviewOnly,
    NoReviewedHistoryInWindow,
}
impl RecoveryGate {
    const fn code(self)->&'static str {
        match self{
            Self::Unavailable=>"HISTORICAL_RECOVERY_UNAVAILABLE",
            Self::OlderOriginalsUnread=>"OLDER_ORIGINAL_HUMAN_TURNS_UNREAD",
            Self::MissingOriginal=>"ORIGINAL_JOURNAL_RECONCILIATION_REQUIRED",
            Self::IndependentConflict=>"INDEPENDENT_SOURCE_RECONCILIATION_REQUIRED",
            Self::OsCustodyUnproven=>"ORIGINAL_OS_DESCENDANT_RECONCILIATION_REQUIRED",
            Self::FreshReviewOnly=>"HISTORICALLY_VERIFIED_FRESH_REVIEW_STILL_REQUIRED",
            Self::NoReviewedHistoryInWindow=>"NO_REVIEWED_EFFECTS_IN_BOUNDED_WINDOW",
        }
    }
    const fn has_unresolved_previous_effects(self)->bool{
        !matches!(self,Self::FreshReviewOnly|Self::NoReviewedHistoryInWindow)
    }
}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(crate) struct NextTurnHostGate {
    pub goal_status:&'static str,
    pub historical_recovery_status:&'static str,
    pub conversational_next_step:&'static str,
    /// This is an inert plan-draft review hint. It cannot create a plan,
    /// grant a native tool, bypass the current goal or schedule an effect.
    pub draft_review_context_ready:bool,
    pub pending_human_questions:usize,
    pub deferred_human_questions:usize,
    pub previous_effects_need_reconciliation:bool,
    pub execution_authority:&'static str,
    pub current_source_freshness:&'static str,
    pub prior_grant_reusable:bool,
    pub automatic_retry_allowed:bool,
    pub model_may_override_host_decision:bool,
}

fn goal_gate(authenticated:bool,facts:GoalRead)->(GoalGate,usize,usize){
    if !authenticated{return (GoalGate::Unauthenticated,0,0)}
    match facts{
        GoalRead::Unavailable=>(GoalGate::Unavailable,0,0),
        GoalRead::Known{goal_id_present,goal_revision_present,pending,deferred,
            unknown_question_status}=>{
            let status=if goal_id_present!=goal_revision_present
                ||(!goal_id_present&&(pending>0||deferred>0))
                ||unknown_question_status||pending>64||deferred>64
                ||pending.saturating_add(deferred)>64{
                GoalGate::Inconsistent
            }else if !goal_id_present {
                GoalGate::NoSavedGoal
            }else if pending>0 {
                GoalGate::PendingQuestion
            }else if deferred>0 {
                GoalGate::DeferredQuestion
            }else{
                GoalGate::ReadyForReview
            };
            (status,pending,deferred)
        }
    }
}

/// A host-supplied 02C status and safe action are BOTH required. If they
/// disagree or a newly introduced label is not recognized, fail closed.
fn recovery_gate(facts:RecoveryRead<'_>)->RecoveryGate{
    let RecoveryRead::Original{status,host_safe_next_action,
        older_human_turns_unread}=facts else{
        return RecoveryGate::Unavailable
    };
    if older_human_turns_unread||
        status=="OLDER_ORIGINAL_TURNS_NOT_INSPECTED"{
        return RecoveryGate::OlderOriginalsUnread
    }
    match status {
        "NO_REVIEWED_EFFECTS_IN_HOST_WINDOW"=>{
            if host_safe_next_action==
                "RECONCILE_ORIGINAL_HISTORY_BEFORE_NEW_ACTION"{
                RecoveryGate::NoReviewedHistoryInWindow
            }else{RecoveryGate::Unavailable}
        },
        "HISTORICAL_REVIEWED_EVIDENCE_ONLY"=>{
            match host_safe_next_action{
                "RECONCILE_ORIGINAL_HISTORY_BEFORE_NEW_ACTION"=>
                    RecoveryGate::MissingOriginal,
                "MANUAL_RECONCILIATION_BEFORE_NEW_ACTION"=>
                    RecoveryGate::IndependentConflict,
                "RECONCILE_OS_THEN_RECHECK_SOURCE_AND_REQUEST_FRESH_AUTHORIZATION"=>
                    RecoveryGate::OsCustodyUnproven,
                "RECHECK_SOURCE_AND_REQUEST_FRESH_AUTHORIZATION"=>
                    RecoveryGate::FreshReviewOnly,
                _=>RecoveryGate::Unavailable,
            }
        },
        "INCOMPLETE_ORIGINAL_HISTORY"|"NO_HOST_ORIGINAL_TURNS"=>
            RecoveryGate::MissingOriginal,
        _=>RecoveryGate::Unavailable,
    }
}

pub(crate) fn classify_current_human_turn(
    authenticated_current_turn:bool,
    goal:GoalRead,recovery:RecoveryRead<'_>
)->NextTurnHostGate{
    let (g,pending,deferred)=goal_gate(authenticated_current_turn,goal);
    let r=recovery_gate(recovery);
    let context_ready=g==GoalGate::ReadyForReview
        &&!r.has_unresolved_previous_effects();
    // The model can explain these source-backed facts or ask for the missing
    // human decision; it must never interpret an old OS receipt or context
    // memory as a reusable operation permission.
    let conversational_next_step=match g{
        GoalGate::Unauthenticated=>
            "STOP_UNAUTHENTICATED_CURRENT_HUMAN_TURN",
        GoalGate::Unavailable|GoalGate::Inconsistent=>
            "RECONCILE_ORIGINAL_ANSF_GOAL_BEFORE_PLAN_REVIEW",
        GoalGate::NoSavedGoal=>
            "GENERAL_CHAT_OR_REQUEST_USER_TO_SET_SAVED_GOAL",
        GoalGate::PendingQuestion=>
            "ANSWER_OR_EXPLICITLY_DEFER_EXISTING_QUESTION",
        GoalGate::DeferredQuestion=>
            "DO_NOT_ASSUME_DEFERRED_ANSWER_OR_REPEAT_WITHOUT_REOPENING",
        GoalGate::ReadyForReview=>match r{
            RecoveryGate::Unavailable|RecoveryGate::OlderOriginalsUnread|
            RecoveryGate::MissingOriginal=>
                "INSPECT_ORIGINAL_HISTORY_BEFORE_NEW_EFFECT",
            RecoveryGate::IndependentConflict=>
                "RECONCILE_INDEPENDENT_SOURCE_BEFORE_NEW_EFFECT",
            RecoveryGate::OsCustodyUnproven=>
                "RECONCILE_OS_CUSTODY_BEFORE_NEW_EFFECT",
            RecoveryGate::FreshReviewOnly|
            RecoveryGate::NoReviewedHistoryInWindow=>
                "REVIEW_INERT_PLAN_ONLY_ON_EXPLICIT_NEW_HUMAN_REQUEST",
        },
    };
    NextTurnHostGate{
        goal_status:g.code(),
        historical_recovery_status:r.code(),
        conversational_next_step,
        draft_review_context_ready:context_ready,
        pending_human_questions:pending,
        deferred_human_questions:deferred,
        previous_effects_need_reconciliation:
            r.has_unresolved_previous_effects(),
        execution_authority:"NOT_AUTHORIZED_FOR_EXECUTION",
        current_source_freshness:"NOT_RECHECKED",
        prior_grant_reusable:false,
        automatic_retry_allowed:false,
        model_may_override_host_decision:false,
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    fn ready()->GoalRead{
        GoalRead::Known{goal_id_present:true,goal_revision_present:true,
            pending:0,deferred:0,unknown_question_status:false}
    }
    fn verified()->RecoveryRead<'static>{
        RecoveryRead::Original{
            status:"HISTORICAL_REVIEWED_EVIDENCE_ONLY",
            host_safe_next_action:"RECHECK_SOURCE_AND_REQUEST_FRESH_AUTHORIZATION",
            older_human_turns_unread:false,
        }
    }
    #[test]
    fn fully_historical_witnessed_goal_is_review_only_not_execution(){
        let result=classify_current_human_turn(true,ready(),verified());
        assert_eq!(result.goal_status,"CURRENT_GOAL_REVIEWABLE_ONLY");
        assert!(result.draft_review_context_ready);
        assert_eq!(result.current_source_freshness,"NOT_RECHECKED");
        assert_eq!(result.execution_authority,"NOT_AUTHORIZED_FOR_EXECUTION");
        assert!(!result.prior_grant_reusable);
        assert!(!result.automatic_retry_allowed);
        assert!(!result.model_may_override_host_decision);
    }
    #[test]
    fn current_turn_must_be_authenticated_even_if_old_evidence_good(){
        let result=classify_current_human_turn(false,ready(),verified());
        assert_eq!(result.goal_status,"CURRENT_HUMAN_TURN_UNAUTHENTICATED");
        assert!(!result.draft_review_context_ready);
    }
    #[test]
    fn no_goal_is_general_chat_not_a_fabricated_plan_authority(){
        let goal=GoalRead::Known{goal_id_present:false,goal_revision_present:false,
            pending:0,deferred:0,unknown_question_status:false};
        let result=classify_current_human_turn(true,goal,verified());
        assert_eq!(result.goal_status,"NO_SAVED_GOAL_GENERAL_CHAT");
        assert!(!result.draft_review_context_ready);
        assert!(result.conversational_next_step.contains("GENERAL_CHAT"));
    }
    #[test]
    fn pending_and_not_now_are_distinct_and_never_count_as_yes(){
        for (pending,deferred,expected) in [
            (1,0,"PENDING_ORIGINAL_HUMAN_CLARIFICATION"),
            (0,1,"DEFERRED_ORIGINAL_HUMAN_CLARIFICATION"),
            (1,1,"PENDING_ORIGINAL_HUMAN_CLARIFICATION"),
        ]{
            let goal=GoalRead::Known{goal_id_present:true,
                goal_revision_present:true,pending,deferred,
                unknown_question_status:false};
            let result=classify_current_human_turn(true,goal,verified());
            assert_eq!(result.goal_status,expected);
            assert!(!result.draft_review_context_ready);
            assert_eq!(result.pending_human_questions,pending);
            assert_eq!(result.deferred_human_questions,deferred);
        }
    }
    #[test]
    fn incomplete_goal_or_unknown_question_status_fails_closed(){
        for bad in [
            GoalRead::Unavailable,
            GoalRead::Known{goal_id_present:true,goal_revision_present:false,
                pending:0,deferred:0,unknown_question_status:false},
            GoalRead::Known{goal_id_present:true,goal_revision_present:true,
                pending:0,deferred:0,unknown_question_status:true},
            GoalRead::Known{goal_id_present:false,goal_revision_present:false,
                pending:1,deferred:0,unknown_question_status:false},
        ]{
            let r=classify_current_human_turn(true,bad,verified());
            assert!(!r.draft_review_context_ready);
            assert!(r.conversational_next_step.contains("ANSF"));
        }
    }
    #[test]
    fn missing_os_conflict_or_old_original_prevents_recovered_plan_clearance(){
        for (action,expected) in [
            ("MANUAL_RECONCILIATION_BEFORE_NEW_ACTION",
                "INDEPENDENT_SOURCE_RECONCILIATION_REQUIRED"),
            ("RECONCILE_OS_THEN_RECHECK_SOURCE_AND_REQUEST_FRESH_AUTHORIZATION",
                "ORIGINAL_OS_DESCENDANT_RECONCILIATION_REQUIRED"),
            ("RECONCILE_ORIGINAL_HISTORY_BEFORE_NEW_ACTION",
                "ORIGINAL_JOURNAL_RECONCILIATION_REQUIRED"),
        ]{
            let r=classify_current_human_turn(true,ready(),
                RecoveryRead::Original{
                    status:"HISTORICAL_REVIEWED_EVIDENCE_ONLY",
                    host_safe_next_action:action,
                    older_human_turns_unread:false,
                });
            assert_eq!(r.historical_recovery_status,expected);
            assert!(r.previous_effects_need_reconciliation);
            assert!(!r.draft_review_context_ready);
            assert!(!r.prior_grant_reusable);
        }
    }
    #[test]
    fn bounded_no_history_is_not_historical_proof_or_old_permission(){
        let r=classify_current_human_turn(true,ready(),
            RecoveryRead::Original{
                status:"NO_REVIEWED_EFFECTS_IN_HOST_WINDOW",
                host_safe_next_action:
                    "RECONCILE_ORIGINAL_HISTORY_BEFORE_NEW_ACTION",
                older_human_turns_unread:false,
            });
        assert_eq!(r.historical_recovery_status,
            "NO_REVIEWED_EFFECTS_IN_BOUNDED_WINDOW");
        assert!(r.draft_review_context_ready);
        assert!(!r.prior_grant_reusable);
        assert_eq!(r.current_source_freshness,"NOT_RECHECKED");
    }
    #[test]
    fn empty_unread_or_unknown_host_status_never_implies_clearance(){
        let scenarios=[
            RecoveryRead::Unavailable,
            RecoveryRead::Original{
                status:"OLDER_ORIGINAL_TURNS_NOT_INSPECTED",
                host_safe_next_action:
                    "RECONCILE_ORIGINAL_HISTORY_BEFORE_NEW_ACTION",
                older_human_turns_unread:true,
            },
            RecoveryRead::Original{
                status:"HISTORICAL_REVIEWED_EVIDENCE_ONLY",
                host_safe_next_action:"UNKNOWN_NEW_HOST_LABEL",
                older_human_turns_unread:false,
            },
            RecoveryRead::Original{
                status:"NO_REVIEWED_EFFECTS_IN_HOST_WINDOW",
                host_safe_next_action:
                    "RECHECK_SOURCE_AND_REQUEST_FRESH_AUTHORIZATION",
                older_human_turns_unread:false,
            },
        ];
        for case in scenarios{
            let r=classify_current_human_turn(true,ready(),case);
            assert!(!r.draft_review_context_ready);
            assert!(r.previous_effects_need_reconciliation);
        }
    }
    #[test]
    fn historical_conflict_cannot_be_hidden_by_answered_questions(){
        let conflict=RecoveryRead::Original{
            status:"HISTORICAL_REVIEWED_EVIDENCE_ONLY",
            host_safe_next_action:"MANUAL_RECONCILIATION_BEFORE_NEW_ACTION",
            older_human_turns_unread:false,
        };
        let r=classify_current_human_turn(true,ready(),conflict);
        assert_eq!(r.conversational_next_step,
            "RECONCILE_INDEPENDENT_SOURCE_BEFORE_NEW_EFFECT");
        assert_eq!(r.execution_authority,"NOT_AUTHORIZED_FOR_EXECUTION");
    }
}
