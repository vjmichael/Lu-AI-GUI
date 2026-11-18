//! MEMORY-LOOP-02B: deterministic safe follow-up classification.
//!
//! This is pure host policy over independently reconstructed facts. No model
//! response, stored natural-language instruction or historical tool receipt
//! may create an action grant. A decision is guidance for human review ONLY;
//! it never claims current source freshness or resumes any worker.
#![allow(dead_code)]

/// Ordered by increasing evidence/recovery severity. A partial history may
/// only strengthen a STOP; no later successful row can erase an earlier one.
#[derive(Debug,Clone,Copy,PartialEq,Eq,PartialOrd,Ord)]
pub(crate) enum HostSafeNext {
    RecheckSourceWithFreshHumanAuthorization,
    ReconcileOsCustodyThenFreshSourceAndHumanAuthorization,
    ReconcileIndependentEvidenceBeforeNewAction,
    ReconcileOriginalHistoryBeforeNewAction,
}

#[derive(Debug,Clone,Copy)]
pub(crate) struct HostReviewedEvidence<'a> {
    pub origin_bound:bool,
    pub full_journal_verified:bool,
    pub omitted_history:bool,
    pub effect_applied:bool,
    pub independent_oracle:&'a str,
    pub original_model_call_linked:bool,
    pub reviewed_worker_completed:bool,
    pub independent_observed_digest_present:bool,
    pub full_descendant_custody_attested:bool,
}

impl HostSafeNext {
    pub(crate) const fn code(self)->&'static str {
        match self {
            Self::RecheckSourceWithFreshHumanAuthorization =>
                "RECHECK_SOURCE_AND_REQUEST_FRESH_AUTHORIZATION",
            Self::ReconcileOsCustodyThenFreshSourceAndHumanAuthorization =>
                "RECONCILE_OS_THEN_RECHECK_SOURCE_AND_REQUEST_FRESH_AUTHORIZATION",
            Self::ReconcileIndependentEvidenceBeforeNewAction =>
                "MANUAL_RECONCILIATION_BEFORE_NEW_ACTION",
            Self::ReconcileOriginalHistoryBeforeNewAction =>
                "RECONCILE_ORIGINAL_HISTORY_BEFORE_NEW_ACTION",
        }
    }

    pub(crate) const fn explanation(self)->&'static str {
        match self {
            Self::RecheckSourceWithFreshHumanAuthorization =>
                "Original semantic and OS cleanup witnesses passed historically. Source freshness is not checked. Recheck the current source and obtain new human authorization before any new operation.",
            Self::ReconcileOsCustodyThenFreshSourceAndHumanAuthorization =>
                "The historical source was independently verified, but descendant termination is not proven. Reconcile OS custody first; then recheck the source and request a new human authorization.",
            Self::ReconcileIndependentEvidenceBeforeNewAction =>
                "An original worker, source oracle or receipt is incomplete, contradicted or unsupported. Reconcile the independent evidence; do not execute, replay or presume completion.",
            Self::ReconcileOriginalHistoryBeforeNewAction =>
                "Original turn identity or complete historical effect coverage cannot be established. Reconcile host-owned original records before considering any new operation.",
        }
    }

    /// Highest-severity original failure wins, regardless of row ordering.
    pub(crate) fn restrict(self, other:Self)->Self {
        std::cmp::max(self,other)
    }

    pub(crate) fn for_evidence(source:HostReviewedEvidence<'_>)->Self {
        use HostSafeNext::*;
        if !source.origin_bound||source.omitted_history {
            return ReconcileOriginalHistoryBeforeNewAction
        }
        // A single disconnected, failed or conflicting effect poisons the
        // historical completion claim, regardless of other verified rows.
        if !source.full_journal_verified||!source.effect_applied||
            source.independent_oracle!="VERIFIED_AT_OBSERVATION"||
            !source.original_model_call_linked||
            !source.reviewed_worker_completed||
            !source.independent_observed_digest_present {
            return ReconcileIndependentEvidenceBeforeNewAction
        }
        if !source.full_descendant_custody_attested {
            return ReconcileOsCustodyThenFreshSourceAndHumanAuthorization
        }
        RecheckSourceWithFreshHumanAuthorization
    }

    /// No original task/effect records or a partially read operation set
    /// cannot be interpreted as "all verified".
    pub(crate) fn for_turn_without_complete_effects(
        origin_bound:bool,all_effects_independent:bool,
        omitted_history:bool,has_operations:bool,
    )->Self{
        if !origin_bound||omitted_history||!has_operations {
            Self::ReconcileOriginalHistoryBeforeNewAction
        }else if !all_effects_independent {
            Self::ReconcileIndependentEvidenceBeforeNewAction
        }else{
            Self::RecheckSourceWithFreshHumanAuthorization
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{HostReviewedEvidence,HostSafeNext as S};
    fn known_good()->HostReviewedEvidence<'static>{
        HostReviewedEvidence {
            origin_bound:true,
            full_journal_verified:true,
            omitted_history:false,
            effect_applied:true,
            independent_oracle:"VERIFIED_AT_OBSERVATION",
            original_model_call_linked:true,
            reviewed_worker_completed:true,
            independent_observed_digest_present:true,
            full_descendant_custody_attested:true,
        }
    }

    #[test]
    fn verified_source_and_os_still_require_new_human_authorization(){
        assert_eq!(S::for_evidence(known_good()).code(),
            "RECHECK_SOURCE_AND_REQUEST_FRESH_AUTHORIZATION");
        assert!(S::for_evidence(known_good()).explanation()
            .contains("Source freshness is not checked"));
    }

    #[test]
    fn source_verified_but_os_unproven_must_reconcile_descendants_first(){
        let mut row=known_good();
        row.full_descendant_custody_attested=false;
        assert_eq!(S::for_evidence(row).code(),
            "RECONCILE_OS_THEN_RECHECK_SOURCE_AND_REQUEST_FRESH_AUTHORIZATION");
    }

    #[test]
    fn conflict_unsupported_missing_oracle_or_worker_deny_continuation(){
        for verdict in ["CONFLICT","UNSUPPORTED","RECEIPT_MISMATCH",
            "NOT_INDEPENDENTLY_CHECKED",""] {
            let mut row=known_good();
            row.independent_oracle=verdict;
            assert_eq!(S::for_evidence(row),
                S::ReconcileIndependentEvidenceBeforeNewAction, "{verdict}");
        }
        let mut cases=Vec::new();
        let mut row=known_good();row.effect_applied=false;cases.push(row);
        let mut row=known_good();row.full_journal_verified=false;cases.push(row);
        let mut row=known_good();row.original_model_call_linked=false;cases.push(row);
        let mut row=known_good();row.reviewed_worker_completed=false;cases.push(row);
        let mut row=known_good();row.independent_observed_digest_present=false;cases.push(row);
        for row in cases{
            assert_eq!(S::for_evidence(row),
                S::ReconcileIndependentEvidenceBeforeNewAction);
        }
    }

    #[test]
    fn missing_origin_truncated_history_and_empty_operations_fail_closed(){
        let mut row=known_good();
        row.origin_bound=false;
        assert_eq!(S::for_evidence(row),S::ReconcileOriginalHistoryBeforeNewAction);
        row=known_good();row.omitted_history=true;
        assert_eq!(S::for_evidence(row),S::ReconcileOriginalHistoryBeforeNewAction);
        assert_eq!(S::for_turn_without_complete_effects(true,true,false,false),
            S::ReconcileOriginalHistoryBeforeNewAction);
        assert_eq!(S::for_turn_without_complete_effects(true,true,true,true),
            S::ReconcileOriginalHistoryBeforeNewAction);
    }

    #[test]
    fn partial_history_precedence_is_order_independent_and_never_silent_pass(){
        let verified=S::for_evidence(known_good());
        let mut unproven=known_good();unproven.full_descendant_custody_attested=false;
        let os=S::for_evidence(unproven);
        let mut conflict=known_good();conflict.independent_oracle="CONFLICT";
        let bad=S::for_evidence(conflict);
        let missing=S::ReconcileOriginalHistoryBeforeNewAction;
        assert_eq!(verified.restrict(os),os);
        assert_eq!(os.restrict(verified),os);
        assert_eq!(verified.restrict(bad),bad);
        assert_eq!(bad.restrict(os),bad);
        assert_eq!(os.restrict(bad),bad);
        assert_eq!(bad.restrict(missing),missing);
        assert_eq!(missing.restrict(verified),missing);
    }

    #[test]
    fn strict_evidence_contract_cannot_turn_old_source_into_tool_permission(){
        assert_eq!(S::for_evidence(known_good()).explanation().contains(
            "new human authorization"),true);
        for s in [
            S::RecheckSourceWithFreshHumanAuthorization,
            S::ReconcileOsCustodyThenFreshSourceAndHumanAuthorization,
            S::ReconcileIndependentEvidenceBeforeNewAction,
            S::ReconcileOriginalHistoryBeforeNewAction,
        ]{
            assert!(!s.code().is_empty());
            assert!(!s.code().contains("EXECUTE_NOW"));
            assert!(!s.code().contains("RETRY"));
        }
    }
}
