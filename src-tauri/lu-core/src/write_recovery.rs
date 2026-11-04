//! WRITE-01D reconciliation *assessment* only.
//!
//! Durable approval, effect receipt and observed target bytes are distinct
//! evidence. This classifier NEVER grants a new task or mutates either journal.
use crate::{journal::EffectState, write_approval_ledger::WriteDecision};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetObservation { Matching, Missing, Different, Unavailable, NotChecked }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryAssessment {
    Denied,
    InterruptedNotAuthorized,
    AwaitingReview,
    ApprovedNoEffectRecord,
    PreparedNeverDispatched,
    ExecutingWithoutReceipt,
    AppliedTargetMatches,
    AppliedTargetChanged,
    AppliedTargetUnverified,
    UncertainTargetMatches,
    UncertainTargetMissing,
    UncertainTargetChanged,
    UncertainTargetUnverified,
    FailedNoVerifiedSuccess,
}
impl RecoveryAssessment {
    pub fn label(self) -> &'static str {
        match self {
            Self::Denied => "DENIED_NO_ACTION",
            Self::InterruptedNotAuthorized => "INTERRUPTED_NO_REAUTHORIZATION",
            Self::AwaitingReview => "NOT_APPROVED",
            Self::ApprovedNoEffectRecord => "APPROVED_NO_EFFECT_RECORD",
            Self::PreparedNeverDispatched => "PREPARED_NO_VERIFIED_EFFECT",
            Self::ExecutingWithoutReceipt => "EXECUTING_UNCONFIRMED",
            Self::AppliedTargetMatches => "APPLIED_RECEIPT_TARGET_MATCHES",
            Self::AppliedTargetChanged => "APPLIED_RECEIPT_TARGET_DIFFERENT",
            Self::AppliedTargetUnverified => "APPLIED_RECEIPT_TARGET_UNVERIFIED",
            Self::UncertainTargetMatches => "UNCERTAIN_BYTES_MATCH_NOT_PROOF",
            Self::UncertainTargetMissing => "UNCERTAIN_TARGET_MISSING",
            Self::UncertainTargetChanged => "UNCERTAIN_TARGET_DIFFERENT",
            Self::UncertainTargetUnverified => "UNCERTAIN_TARGET_UNVERIFIED",
            Self::FailedNoVerifiedSuccess => "FAILED_NO_VERIFIED_SUCCESS",
        }
    }
    pub fn may_auto_retry(self) -> bool { false }
    pub fn verified_success(self) -> bool {
        matches!(self, Self::AppliedTargetMatches)
    }
}

pub fn assess(
    decision: WriteDecision,
    effect: Option<EffectState>,
    observation: TargetObservation,
) -> RecoveryAssessment {
    use RecoveryAssessment as R;
    if decision == WriteDecision::Denied { return R::Denied; }
    if decision == WriteDecision::Interrupted { return R::InterruptedNotAuthorized; }
    if decision != WriteDecision::Approved { return R::AwaitingReview; }
    match effect {
        None => R::ApprovedNoEffectRecord,
        Some(EffectState::Prepared) => R::PreparedNeverDispatched,
        Some(EffectState::Executing) => R::ExecutingWithoutReceipt,
        Some(EffectState::Failed) => R::FailedNoVerifiedSuccess,
        Some(EffectState::Applied) => match observation {
            TargetObservation::Matching => R::AppliedTargetMatches,
            TargetObservation::Different | TargetObservation::Missing => R::AppliedTargetChanged,
            TargetObservation::Unavailable | TargetObservation::NotChecked => R::AppliedTargetUnverified,
        },
        Some(EffectState::Uncertain) => match observation {
            TargetObservation::Matching => R::UncertainTargetMatches,
            TargetObservation::Missing => R::UncertainTargetMissing,
            TargetObservation::Different => R::UncertainTargetChanged,
            TargetObservation::Unavailable | TargetObservation::NotChecked => R::UncertainTargetUnverified,
        },
    }
}
