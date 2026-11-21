//! Acceptance-only audit of the original journal, not a process authority.
use lu_ide_execution_supervisor::{JobPhase, Receipt};

/// DAP gateway cancellation now always passes through ReleasePending. Require
/// the exact original request, verified empty tree and independent release
/// terminal, in order, under the same job/plan. A bare Cancelled label is not
/// evidence of the stronger gateway protocol. The caller has already verified
/// the durable supervisor journal and still must verify the real OS tree.
pub fn verified_gateway_cancel(history: &[Receipt], job: &str, plan: &str) -> bool {
    let Some(last) = history.len().checked_sub(3).and_then(|n| history.get(n..)) else {
        return false;
    };
    let [request, reaped, terminal] = last else {
        return false;
    };
    let phases = [
        (
            JobPhase::CancelRequested,
            "cancel_requested_before_os_kill",
            "tree_termination_pending",
        ),
        (
            JobPhase::ReleasePending,
            "os_cancel_tree_reaped_service_release_pending",
            "native_supervisor_tree_kill_evidence",
        ),
        (
            JobPhase::Cancelled,
            "native_cancel_service_release_verified",
            "tree_empty_and_independent_service_ack",
        ),
    ];
    last.iter()
        .zip(phases)
        .all(|(entry, (phase, outcome, evidence))| {
            entry.body.job_id == job
                && entry.body.plan_sha256 == plan
                && entry.body.session_id == request.body.session_id
                && entry.body.project_id == request.body.project_id
                && entry.body.phase == phase
                && entry.body.outcome_code == outcome
                && entry.body.evidence_code == evidence
        })
        && request.body.sequence.checked_add(1) == Some(reaped.body.sequence)
        && reaped.body.sequence.checked_add(1) == Some(terminal.body.sequence)
        && reaped.prior_hash == request.entry_hash
        && terminal.prior_hash == reaped.entry_hash
        && request.entry_hash != reaped.entry_hash
        && reaped.entry_hash != terminal.entry_hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use lu_ide_execution_supervisor::ReceiptBody;
    fn fixture() -> Vec<Receipt> {
        [
            (
                JobPhase::CancelRequested,
                "cancel_requested_before_os_kill",
                "tree_termination_pending",
            ),
            (
                JobPhase::ReleasePending,
                "os_cancel_tree_reaped_service_release_pending",
                "native_supervisor_tree_kill_evidence",
            ),
            (
                JobPhase::Cancelled,
                "native_cancel_service_release_verified",
                "tree_empty_and_independent_service_ack",
            ),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, (phase, outcome, evidence))| Receipt {
            body: ReceiptBody {
                version: 1,
                sequence: i as u64 + 1,
                event_id: format!("fixture-{i}"),
                job_id: "job".into(),
                project_id: "project".into(),
                plan_sha256: "plan".into(),
                phase,
                observed_unix_ms: 0,
                session_id: "session".into(),
                outcome_code: outcome.into(),
                evidence_code: evidence.into(),
            },
            prior_hash: format!("hash-{i}"),
            entry_hash: format!("hash-{}", i + 1),
        })
        .collect()
    }
    #[test]
    fn exact_three_phase_gateway_history_required() {
        let good = fixture();
        assert!(verified_gateway_cancel(&good, "job", "plan"));
        let mut missing = good.clone();
        missing.remove(1);
        assert!(!verified_gateway_cancel(&missing, "job", "plan"));
        for i in 0..3 {
            let mut changed = good.clone();
            changed[i].body.plan_sha256 = "other".into();
            assert!(!verified_gateway_cancel(&changed, "job", "plan"));
            let mut changed = good.clone();
            changed[i].body.outcome_code = "forged".into();
            assert!(!verified_gateway_cancel(&changed, "job", "plan"));
        }
        let mut wrong_session = good.clone();
        wrong_session[1].body.session_id = "different-session".into();
        assert!(!verified_gateway_cancel(&wrong_session, "job", "plan"));
        let mut wrong_project = good.clone();
        wrong_project[2].body.project_id = "different-project".into();
        assert!(!verified_gateway_cancel(&wrong_project, "job", "plan"));
        let mut gap = good.clone();
        gap[2].body.sequence += 1;
        assert!(!verified_gateway_cancel(&gap, "job", "plan"));
        let mut missing_ack = good.clone();
        missing_ack[2].body.evidence_code = "not_verified".into();
        assert!(!verified_gateway_cancel(&missing_ack, "job", "plan"));
        let mut reordered = good.clone();
        reordered.swap(0, 1);
        assert!(!verified_gateway_cancel(&reordered, "job", "plan"));
        let mut broken_chain = good;
        broken_chain[2].prior_hash = "unrelated".into();
        assert!(!verified_gateway_cancel(&broken_chain, "job", "plan"));
    }
}
