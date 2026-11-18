use lu_core::{
    journal::EffectState,
    write_approval_ledger::WriteDecision,
    write_recovery::{assess, RecoveryAssessment as R, TargetObservation as T},
};
#[test]
fn uncertain_effect_matching_disk_does_not_become_proven_success_or_retriable() {
    let a=assess(WriteDecision::Approved,Some(EffectState::Uncertain),T::Matching);
    assert_eq!(a,R::UncertainTargetMatches);
    assert!(!a.verified_success());
    assert!(!a.may_auto_retry());
    assert_eq!(a.label(),"UNCERTAIN_BYTES_MATCH_NOT_PROOF");
}
#[test]
fn applied_receipt_and_matching_target_are_independent_required_evidence() {
    assert_eq!(assess(WriteDecision::Approved,Some(EffectState::Applied),T::Matching),
        R::AppliedTargetMatches);
    assert!(R::AppliedTargetMatches.verified_success());
    assert_eq!(assess(WriteDecision::Approved,Some(EffectState::Applied),T::Different),
        R::AppliedTargetChanged);
    assert_eq!(assess(WriteDecision::Approved,Some(EffectState::Applied),T::Missing),
        R::AppliedTargetChanged);
    assert_eq!(assess(WriteDecision::Approved,Some(EffectState::Applied),T::NotChecked),
        R::AppliedTargetUnverified);
}
#[test]
fn denied_interrupted_pending_and_missing_receipts_never_mint_new_authority() {
    for (decision,effect,expected) in [
        (WriteDecision::Denied,Some(EffectState::Applied),R::Denied),
        (WriteDecision::Interrupted,Some(EffectState::Applied),R::InterruptedNotAuthorized),
        (WriteDecision::Pending,None,R::AwaitingReview),
        (WriteDecision::Selected,None,R::AwaitingReview),
        (WriteDecision::Approved,None,R::ApprovedNoEffectRecord),
        (WriteDecision::Approved,Some(EffectState::Prepared),R::PreparedNeverDispatched),
        (WriteDecision::Approved,Some(EffectState::Executing),R::ExecutingWithoutReceipt),
        (WriteDecision::Approved,Some(EffectState::Failed),R::FailedNoVerifiedSuccess),
    ] {
        let result=assess(decision,effect,T::Matching);
        assert_eq!(result,expected);
        assert!(!result.verified_success());
        assert!(!result.may_auto_retry());
    }
}
