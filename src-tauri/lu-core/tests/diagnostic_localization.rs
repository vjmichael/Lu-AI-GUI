//! DX-03 synthetic fault localization fixtures; NOT installed Windows proof.
use lu_core::diagnostic_localization::{localize_normal_turn, EvidenceStrength};
use lu_core::wire::proto;

fn all_success() -> proto::DiagnosticTrace {
    let stages = [1, 2, 3, 4, 19, 5, 6, 7, 8, 9, 10, 11, 12, 13];
    let mut previous = 0;
    let events = stages
        .iter()
        .enumerate()
        .map(|(index, stage)| {
            let sequence = index as u64 + 1;
            let event = proto::DiagnosticEvent {
                sequence,
                parent_sequence: previous,
                stage: *stage,
                outcome: 2,
                elapsed_millis: sequence * 10,
                clock_domain: 2,
                failure_code: 0,
                attempt: 1,
                clock_epoch: 1,
            };
            previous = sequence;
            event
        })
        .collect();
    proto::DiagnosticTrace {
        protocol_version: 2,
        trace_ref: "abcdef0123456789abcdef0123456789".into(),
        dropped_events: 0,
        events,
    }
}

#[test]
fn a_complete_trace_is_a_completed_operation_not_a_failure() {
    let report = localize_normal_turn(&all_success()).unwrap();
    assert_eq!(report.evidence_strength, EvidenceStrength::Complete);
    assert_eq!(report.first_unconfirmed, "none");
    assert_eq!(report.confirmed_until, "assistant_rendered");
}

#[test]
fn stopped_after_host_model_loop_points_to_return_boundary_not_react() {
    let mut trace = all_success();
    trace.events.truncate(9);
    let report = localize_normal_turn(&trace).unwrap();
    assert_eq!(report.confirmed_until, "model_loop_entered");
    assert_eq!(report.first_unconfirmed, "model_loop_returned");
    assert_eq!(report.fault_region, "model_runtime");
    assert_eq!(
        report.evidence_strength,
        EvidenceStrength::BoundedToNextStage
    );
}

#[test]
fn observed_model_entry_without_a_return_localizes_to_model_completion() {
    let mut trace = all_success();
    trace.events.truncate(9);
    trace.events[7].outcome = 1; // Worker entry is not a worker success.
    trace.events[8].outcome = 1; // Model loop began; no completion observed.
    let report = localize_normal_turn(&trace).unwrap();
    assert_eq!(report.confirmed_until, "model_loop_entered");
    assert_eq!(report.first_unconfirmed, "model_loop_returned");
    assert_eq!(
        report.evidence_strength,
        EvidenceStrength::BoundedToNextStage
    );
}

/// Reproduce the stage boundary reported by the real Windows WebView2
/// failure (run 40944293463): the host entered the native model loop, but
/// the model turn failed before a successful model-loop return was observed.
/// This synthetic fixture validates *diagnostic interpretation*, not the
/// real engine bug itself or a successful installed Windows response.
#[test]
fn windows_native_model_loop_failure_is_bounded_to_the_model_runtime() {
    let mut trace = all_success();
    trace.events.truncate(10);
    let failed = trace.events.last_mut().unwrap();
    assert_eq!(failed.stage, 9); // model_loop_returned boundary
    failed.outcome = 3;
    failed.failure_code = 5; // observed backend_unavailable

    let result = localize_normal_turn(&trace).unwrap();
    assert_eq!(result.confirmed_until, "model_loop_entered");
    assert_eq!(result.first_unconfirmed, "model_loop_returned");
    assert_eq!(result.fault_region, "model_runtime");
    assert_eq!(result.failure_code, "backend_unavailable");
    assert_eq!(result.evidence_strength, EvidenceStrength::ObservedFailure);
    assert_eq!(result.observed_interval_millis, Some(10));
    assert_eq!(result.next_probe, "check_engine_and_tool_turn");

    // A restarted worker's monotonic clock has a different origin:
    // never invent a negative or cross-process elapsed interval.
    let mut restarted = trace;
    let failed = restarted.events.last_mut().unwrap();
    failed.clock_epoch = 2;
    failed.elapsed_millis = 1;
    let result = localize_normal_turn(&restarted).unwrap();
    assert_eq!(result.evidence_strength, EvidenceStrength::ObservedFailure);
    assert_eq!(result.observed_interval_millis, None);
}

#[test]
fn actual_transport_failure_is_observed_not_inferred() {
    let mut trace = all_success();
    trace.events.truncate(11);
    let failed = trace.events.last_mut().unwrap();
    failed.outcome = 3;
    failed.failure_code = 7;
    let report = localize_normal_turn(&trace).unwrap();
    assert_eq!(report.confirmed_until, "model_loop_returned");
    assert_eq!(report.first_unconfirmed, "native_ipc_returned");
    assert_eq!(report.fault_region, "tauri_ipc");
    assert_eq!(report.failure_code, "transport");
    assert_eq!(report.evidence_strength, EvidenceStrength::ObservedFailure);
    assert_eq!(report.observed_interval_millis, Some(10));
    let mut second = trace;
    second.events.last_mut().unwrap().clock_domain = 3;
    let report = localize_normal_turn(&second).unwrap();
    assert_eq!(report.observed_interval_millis, None);
}

#[test]
fn missing_records_or_out_of_order_confirmation_never_claim_a_root_cause() {
    let mut trace = all_success();
    trace.dropped_events = 1;
    trace.events.remove(8);
    let report = localize_normal_turn(&trace).unwrap();
    assert_eq!(report.first_unconfirmed, "model_loop_entered");
    assert_eq!(
        report.evidence_strength,
        EvidenceStrength::InsufficientEvidence
    );
    let mut trace = all_success();
    trace.events.remove(8);
    // Even without an explicit drop counter, later success does not prove
    // a missing predecessor was skipped. Fix the recorded parent edge.
    trace.events[8].parent_sequence = 8;
    // Reparenting cannot make an unreported missing sequence trustworthy.
    assert!(localize_normal_turn(&trace).is_err());
}

#[test]
fn abandoned_tool_dispatch_localizes_the_native_tool_boundary() {
    let mut trace = all_success();
    trace.events.truncate(9);
    trace.events.push(proto::DiagnosticEvent {
        sequence: 10,
        parent_sequence: 9,
        stage: 14,
        outcome: 1,
        elapsed_millis: 100,
        clock_domain: 2,
        failure_code: 0,
        attempt: 1,
        clock_epoch: 1,
    });
    let report = localize_normal_turn(&trace).unwrap();
    assert_eq!(report.confirmed_until, "tool_dispatch_entered");
    assert_eq!(report.first_unconfirmed, "tool_dispatch_returned");
    assert_eq!(report.fault_region, "native_tool");
}

#[test]
fn cancelled_request_requires_terminal_worker_state_evidence() {
    let mut trace = all_success();
    trace.events.truncate(8);
    trace.events.push(proto::DiagnosticEvent {
        sequence: 9,
        parent_sequence: 8,
        stage: 16,
        outcome: 1,
        elapsed_millis: 100,
        clock_domain: 2,
        failure_code: 0,
        attempt: 1,
        clock_epoch: 1,
    });
    let report = localize_normal_turn(&trace).unwrap();
    assert_eq!(report.first_unconfirmed, "worker_terminated");
    assert_eq!(report.fault_region, "worker_recovery");
}

#[test]
fn never_merge_an_earlier_attempt_with_the_last_retry() {
    let mut trace = all_success();
    let next = proto::DiagnosticEvent {
        sequence: 15,
        parent_sequence: 14,
        stage: 1,
        outcome: 2,
        elapsed_millis: 1,
        clock_domain: 1,
        failure_code: 0,
        attempt: 2,
        clock_epoch: 1,
    };
    trace.events.push(next);
    let report = localize_normal_turn(&trace).unwrap();
    assert_eq!(report.attempt, 2);
    assert_eq!(report.confirmed_until, "user_submitted");
    assert_eq!(report.first_unconfirmed, "user_saved");
}

#[test]
fn contradictory_success_and_failure_in_one_attempt_are_inconclusive() {
    let mut trace = all_success();
    // Full assistant render was already observed before this later,
    // contradictory same-attempt failure claim appeared.
    trace.events.push(proto::DiagnosticEvent {
        sequence: 15,
        parent_sequence: 14,
        stage: 13,
        outcome: 3,
        elapsed_millis: 150,
        clock_domain: 2,
        failure_code: 4,
        attempt: 1,
        clock_epoch: 1,
    });
    let result = localize_normal_turn(&trace).unwrap();
    assert_eq!(
        result.evidence_strength,
        EvidenceStrength::InsufficientEvidence
    );

    let mut followed_by_success = all_success();
    followed_by_success.events[9].outcome = 3;
    followed_by_success.events[9].failure_code = 5;
    let result = localize_normal_turn(&followed_by_success).unwrap();
    assert_eq!(
        result.evidence_strength,
        EvidenceStrength::InsufficientEvidence
    );

    let mut two_failures = all_success();
    two_failures.events.truncate(10);
    two_failures.events[9].outcome = 3;
    two_failures.events[9].failure_code = 5;
    two_failures.events.push(proto::DiagnosticEvent {
        sequence: 11,
        parent_sequence: 10,
        stage: 14,
        outcome: 3,
        elapsed_millis: 110,
        clock_domain: 2,
        failure_code: 7,
        attempt: 1,
        clock_epoch: 1,
    });
    let result = localize_normal_turn(&two_failures).unwrap();
    assert_eq!(
        result.evidence_strength,
        EvidenceStrength::InsufficientEvidence
    );
}

#[test]
fn invalid_observations_are_rejected_before_analysis() {
    let mut trace = all_success();
    trace.events[3].stage = 99;
    assert!(localize_normal_turn(&trace).is_err());
}

#[test]
fn restarted_worker_is_not_a_same_clock_interval_even_within_one_attempt() {
    let mut trace = all_success();
    trace.events.truncate(11);
    let failed = trace.events.last_mut().unwrap();
    failed.outcome = 3;
    failed.failure_code = 7;
    // The host observes a failed return from a restarted worker, whose
    // monotonic clock begins at zero. This is still one *task attempt*.
    failed.clock_epoch = 2;
    failed.elapsed_millis = 1;
    let report = localize_normal_turn(&trace).unwrap();
    assert_eq!(report.first_unconfirmed, "native_ipc_returned");
    assert_eq!(report.evidence_strength, EvidenceStrength::ObservedFailure);
    assert_eq!(report.observed_interval_millis, None);
}

#[test]
fn disconnected_assistant_render_does_not_certify_turn_completion() {
    let mut trace = all_success();
    // The UI claims success, but its event is an independent root rather
    // than a descendant of the verified assistant write.
    trace.events.last_mut().unwrap().parent_sequence = 0;
    let report = localize_normal_turn(&trace).unwrap();
    assert_eq!(report.confirmed_until, "assistant_saved");
    assert_eq!(report.first_unconfirmed, "assistant_rendered");
    assert_eq!(
        report.evidence_strength,
        EvidenceStrength::InsufficientEvidence
    );
}

#[test]
fn out_of_order_checkpoints_are_not_a_completed_causal_turn() {
    let mut trace = all_success();
    // Both stages appear and their sequence numbers are monotonic, but
    // the transport was recorded before the confirmed save.
    trace.events[1].stage = 3; // IPC invoked
    trace.events[2].stage = 2; // USER saved
    let report = localize_normal_turn(&trace).unwrap();
    assert_eq!(report.confirmed_until, "user_saved");
    assert_eq!(report.first_unconfirmed, "ipc_invoked");
    assert_eq!(
        report.evidence_strength,
        EvidenceStrength::InsufficientEvidence
    );
}

#[test]
fn unrelated_failure_must_not_borrow_a_different_branch_clock() {
    let mut trace = all_success();
    trace.events.truncate(11);
    let failed = trace.events.last_mut().unwrap();
    failed.outcome = 3;
    failed.failure_code = 7;
    failed.parent_sequence = 0; // This failure has no causal predecessor.
    let report = localize_normal_turn(&trace).unwrap();
    assert_eq!(report.first_unconfirmed, "native_ipc_returned");
    assert_eq!(report.confirmed_until, "none");
    assert_eq!(report.observed_interval_millis, None);
    assert_eq!(
        report.evidence_strength,
        EvidenceStrength::InsufficientEvidence
    );
}
