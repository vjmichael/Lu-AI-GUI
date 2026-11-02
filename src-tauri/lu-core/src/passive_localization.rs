//! DX host-only interpretation of actual passive checkpoints.
//!
//! The host's passive ring is NOT the complete typed Protobuf turn trace:
//! there is no proof here of assistant persistence, render, tool-result type,
//! worker clock epochs or which backend operation failed. Never invent those
//! observations or mistake a host-minted correlation ref for authority.
use crate::passive_diagnostics::{PassiveEvent, PassiveTraceSnapshot};
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PassiveHostFinding {
    pub schema: &'static str,
    pub scope: &'static str,
    pub status: &'static str,
    pub last_observed: &'static str,
    pub first_unobserved: &'static str,
    pub fault_region: &'static str,
    pub next_probe: &'static str,
    pub dropped_events: u64,
}

fn known_stage(stage: &str) -> bool {
    matches!(
        stage,
        "native-turn-received"
            | "recovery-ready"
            | "recovery-unavailable"
            | "native-session-ready"
            | "native-session-rejected"
            | "saved-user-attested"
            | "saved-user-rejected"
            | "native-turn-registered"
            | "native-turn-busy"
            | "native-worker-entered"
            | "native-task-authorized"
            | "native-model-loop-entered"
            | "native-model-loop-returned"
            | "native-tool-dispatch-entered"
            | "native-tool-dispatch-returned"
            | "native-turn-succeeded"
            | "native-turn-failed"
            | "native-cancel-signalled"
    )
}

fn finding(
    status: &'static str,
    last_observed: &'static str,
    first_unobserved: &'static str,
    fault_region: &'static str,
    next_probe: &'static str,
    dropped_events: u64,
) -> PassiveHostFinding {
    PassiveHostFinding {
        schema: "lu.native.host-finding.v1",
        scope: "host_only",
        status,
        last_observed,
        first_unobserved,
        fault_region,
        next_probe,
        dropped_events,
    }
}

fn valid_events(events: &[PassiveEvent]) -> bool {
    if events.is_empty()
        || events.len() > 64
        // A complete, untruncated host ring must start at one.
        || events[0].sequence != 1
        || !events.iter().all(|event| known_stage(event.stage))
        || !events.windows(2).all(|pair| {
            pair[0].sequence.checked_add(1) == Some(pair[1].sequence)
                && pair[1].elapsed_millis >= pair[0].elapsed_millis
        })
    {
        return false;
    }

    // One admitted native turn has one terminal outcome. These checkpoints
    // originate in the trusted host and cannot be reordered or replayed to
    // turn a failed operation into an apparent successful one.
    let mut entered = false;
    let mut returned = false;
    let mut tool_inflight = false;
    for (index, event) in events.iter().enumerate() {
        match event.stage {
            "native-model-loop-entered" => {
                if entered || returned {
                    return false;
                }
                entered = true;
            }
            "native-model-loop-returned" => {
                if !entered || returned || tool_inflight {
                    return false;
                }
                returned = true;
            }
            "native-tool-dispatch-entered" => {
                if !entered || returned || tool_inflight {
                    return false;
                }
                tool_inflight = true;
            }
            "native-tool-dispatch-returned" => {
                if !tool_inflight || returned {
                    return false;
                }
                tool_inflight = false;
            }
            "native-turn-succeeded" => {
                // Host success is emitted only after the supervised loop
                // returns. It is not evidence of assistant save/render.
                if !returned || tool_inflight || index + 1 != events.len() {
                    return false;
                }
            }
            "native-turn-failed" if index + 1 != events.len() => return false,
            _ => {}
        }
    }
    true
}

/// Conservative host-only finding from a **trusted host-created snapshot**.
/// Does not take untrusted WebView data, create authority, or mutate the trace.
/// A passive "turn failed" event is not itself a typed failure code.
pub fn localize_passive_host_trace(snapshot: &PassiveTraceSnapshot) -> PassiveHostFinding {
    let lost = snapshot.dropped_events;
    let unavailable = || {
        finding(
            "insufficient_evidence",
            "none",
            "unknown",
            "unknown",
            "collect_scoped_host_trace",
            lost,
        )
    };
    if snapshot.schema != "lu.native.host-trace.v2"
        || snapshot.recording_state != "accepted_turn"
        || snapshot.trace_ref.as_deref().is_none_or(|reference| {
            reference.len() != 32
                || !reference
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        })
        || snapshot.clock_domain != "host"
        || !valid_events(&snapshot.events)
    {
        return unavailable();
    }

    // Dropped events can hide an earlier completion or a restart. Do not
    // attribute a boundary to the surviving tail of an incomplete ring.
    if lost > 0 {
        return unavailable();
    }

    let entries = &snapshot.events;
    let has = |code: &str| entries.iter().any(|event| event.stage == code);
    let last = entries.last().map(|e| e.stage).unwrap_or("none");
    let pending_tool = entries
        .iter()
        .filter(|event| event.stage == "native-tool-dispatch-entered")
        .count()
        > entries
            .iter()
            .filter(|event| event.stage == "native-tool-dispatch-returned")
            .count();

    if has("native-turn-succeeded") {
        // Host return does not certify durable assistant write or UI render.
        return finding(
            "host_turn_returned",
            "native-turn-succeeded",
            "assistant_persistence_unobserved",
            "host_boundary",
            "verify_assistant_persistence_and_render",
            lost,
        );
    }
    if has("native-turn-failed") {
        if pending_tool {
            // The dispatcher was invoked, but its response boundary was not
            // observed. Do not infer a tool implementation/root-cause failure.
            return finding(
                "bounded_host_boundary",
                "native-tool-dispatch-entered",
                "native-tool-dispatch-returned",
                "native_tool_dispatch",
                "inspect_native_tool_dispatch_journal",
                lost,
            );
        }
        if has("native-model-loop-entered") && !has("native-model-loop-returned") {
            // Observed model-loop entry and terminal failure. This could be an
            // inference backend, a journal, or a tool continuation failure.
            return finding(
                "bounded_host_boundary",
                "native-model-loop-entered",
                "native-model-loop-returned",
                "model_or_tool_continuation",
                "inspect_engine_and_native_tool_journal",
                lost,
            );
        }
        if has("native-model-loop-returned") {
            return finding(
                "bounded_host_boundary",
                "native-model-loop-returned",
                "native_turn_succeeded",
                "post_model_return",
                "inspect_native_reply_boundary",
                lost,
            );
        }
        return finding(
            "insufficient_evidence",
            last,
            "unknown",
            "unknown",
            "inspect_host_admission_and_worker",
            lost,
        );
    }
    if last == "native-cancel-signalled" {
        // When a running model loop receives cancellation, its latest known
        // boundary is the cancel signal, not ordinary inference progress.
        // Neither task exit nor supervisor recovery has been observed here.
        return finding(
            "in_progress",
            "native-cancel-signalled",
            "worker_terminated_unobserved",
            "worker_recovery",
            "check_worker_exit_and_journal",
            lost,
        );
    }
    if pending_tool {
        return finding(
            "in_progress",
            "native-tool-dispatch-entered",
            "native-tool-dispatch-returned",
            "native_tool_dispatch",
            "await_native_tool_dispatch_return",
            lost,
        );
    }
    if has("native-model-loop-entered") && !has("native-model-loop-returned") {
        return finding(
            "in_progress",
            "native-model-loop-entered",
            "native-model-loop-returned",
            "model_or_tool_continuation",
            "await_host_terminal_checkpoint",
            lost,
        );
    }
    finding(
        "in_progress",
        last,
        "unknown",
        "host_boundary",
        "await_host_terminal_checkpoint",
        lost,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::passive_diagnostics::{NativeStage, PassiveTraceJournal};

    const A: &str = "0123456789abcdef0123456789abcdef";
    const B: &str = "fedcba9876543210fedcba9876543210";

    #[test]
    fn real_host_journal_failure_bounds_model_or_tool_not_root_cause() {
        let journal = PassiveTraceJournal::default();
        assert!(journal.begin(A));
        for stage in [
            NativeStage::TurnReceived,
            NativeStage::WorkerEntered,
            NativeStage::ModelLoopEntered,
            NativeStage::TurnFailed,
        ] {
            journal.record(A, stage);
        }
        let evidence = localize_passive_host_trace(&journal.snapshot().unwrap());
        assert_eq!(evidence.status, "bounded_host_boundary");
        assert_eq!(evidence.last_observed, "native-model-loop-entered");
        assert_eq!(evidence.first_unobserved, "native-model-loop-returned");
        assert_eq!(evidence.fault_region, "model_or_tool_continuation");
        assert_eq!(
            evidence.next_probe,
            "inspect_engine_and_native_tool_journal"
        );
        assert_eq!(evidence.scope, "host_only");
        assert_eq!(evidence.dropped_events, 0);
    }

    #[test]
    fn successful_host_turn_does_not_certify_assistant_persistence() {
        let journal = PassiveTraceJournal::default();
        assert!(journal.begin(A));
        journal.record(A, NativeStage::TurnReceived);
        journal.record(A, NativeStage::ModelLoopEntered);
        journal.record(A, NativeStage::ModelLoopReturned);
        journal.record(A, NativeStage::TurnSucceeded);
        let finding = localize_passive_host_trace(&journal.snapshot().unwrap());
        assert_eq!(finding.status, "host_turn_returned");
        assert_eq!(finding.first_unobserved, "assistant_persistence_unobserved");
    }

    #[test]
    fn missing_or_dropped_events_can_never_pin_a_cause() {
        let journal = PassiveTraceJournal::default();
        assert!(journal.begin(A));
        journal.record(A, NativeStage::ModelLoopEntered);
        let mut snapshot = journal.snapshot().unwrap();
        snapshot.dropped_events = 1;
        assert_eq!(
            localize_passive_host_trace(&snapshot).status,
            "insufficient_evidence"
        );
        snapshot.dropped_events = 0;
        snapshot.events[0].stage = "unrecognized_external_stage";
        assert_eq!(
            localize_passive_host_trace(&snapshot).status,
            "insufficient_evidence"
        );
        snapshot.events[0].stage = "native-model-loop-entered";
        snapshot.recording_state = "admission_unobserved";
        assert_eq!(
            localize_passive_host_trace(&snapshot).status,
            "insufficient_evidence"
        );
    }

    #[test]
    fn attempted_stale_trace_cannot_inherit_an_earlier_result() {
        let journal = PassiveTraceJournal::default();
        assert!(journal.begin(A));
        journal.record(A, NativeStage::TurnSucceeded);
        assert!(journal.begin(B));
        let fresh = journal.snapshot().unwrap();
        assert_eq!(fresh.trace_ref.as_deref(), Some(B));
        assert_eq!(
            localize_passive_host_trace(&fresh).status,
            "insufficient_evidence"
        );
    }

    #[test]
    fn contradictory_host_terminal_events_never_certify_a_success() {
        let journal = PassiveTraceJournal::default();
        assert!(journal.begin(A));
        for event in [
            NativeStage::ModelLoopEntered,
            NativeStage::ModelLoopReturned,
            NativeStage::TurnSucceeded,
            NativeStage::TurnFailed,
        ] {
            journal.record(A, event);
        }
        let snapshot = journal.snapshot().unwrap();
        let result = localize_passive_host_trace(&snapshot);
        assert_eq!(result.status, "insufficient_evidence");
        assert_eq!(result.fault_region, "unknown");
    }

    #[test]
    fn terminal_followed_by_late_stage_or_duplicate_outcome_is_inconclusive() {
        for later in [
            NativeStage::TurnSucceeded,
            NativeStage::ModelLoopReturned,
            NativeStage::CancelSignalled,
        ] {
            let journal = PassiveTraceJournal::default();
            assert!(journal.begin(A));
            journal.record(A, NativeStage::ModelLoopEntered);
            journal.record(A, NativeStage::TurnFailed);
            journal.record(A, later);
            let result = localize_passive_host_trace(&journal.snapshot().unwrap());
            assert_eq!(result.status, "insufficient_evidence");
        }
    }

    #[test]
    fn impossible_model_return_order_or_success_without_return_fails_closed() {
        let journal = PassiveTraceJournal::default();
        assert!(journal.begin(A));
        journal.record(A, NativeStage::ModelLoopReturned);
        journal.record(A, NativeStage::ModelLoopEntered);
        journal.record(A, NativeStage::TurnSucceeded);
        assert_eq!(
            localize_passive_host_trace(&journal.snapshot().unwrap()).status,
            "insufficient_evidence"
        );

        assert!(journal.begin(B));
        journal.record(B, NativeStage::ModelLoopEntered);
        journal.record(B, NativeStage::TurnSucceeded);
        assert_eq!(
            localize_passive_host_trace(&journal.snapshot().unwrap()).status,
            "insufficient_evidence"
        );
    }

    #[test]
    fn observed_native_tool_dispatch_requires_ordered_boundary_pairs() {
        let journal = PassiveTraceJournal::default();
        assert!(journal.begin(A));
        for stage in [
            NativeStage::TurnReceived,
            NativeStage::ModelLoopEntered,
            NativeStage::ToolDispatchEntered,
            NativeStage::ToolDispatchReturned,
            NativeStage::ModelLoopReturned,
            NativeStage::TurnSucceeded,
        ] {
            journal.record(A, stage);
        }
        let snapshot = journal.snapshot().unwrap();
        assert_eq!(
            localize_passive_host_trace(&snapshot).status,
            "host_turn_returned"
        );
        let mut forged = snapshot.clone();
        forged.events[2].stage = "native-tool-dispatch-returned";
        assert_eq!(
            localize_passive_host_trace(&forged).status,
            "insufficient_evidence"
        );
        let mut unfinished = snapshot;
        unfinished.events.remove(3);
        for (index, event) in unfinished.events.iter_mut().enumerate() {
            event.sequence = index as u64 + 1;
        }
        assert_eq!(
            localize_passive_host_trace(&unfinished).status,
            "insufficient_evidence"
        );
    }

    #[test]
    fn a_missing_native_dispatch_return_bounds_the_actual_observed_interval() {
        let journal = PassiveTraceJournal::default();
        assert!(journal.begin(A));
        for stage in [
            NativeStage::TurnReceived,
            NativeStage::ModelLoopEntered,
            NativeStage::ToolDispatchEntered,
        ] {
            journal.record(A, stage);
        }
        let in_flight = localize_passive_host_trace(&journal.snapshot().unwrap());
        assert_eq!(in_flight.status, "in_progress");
        assert_eq!(in_flight.first_unobserved, "native-tool-dispatch-returned");
        assert_eq!(in_flight.fault_region, "native_tool_dispatch");
        assert_eq!(in_flight.next_probe, "await_native_tool_dispatch_return");

        journal.record(A, NativeStage::TurnFailed);
        let failure = localize_passive_host_trace(&journal.snapshot().unwrap());
        assert_eq!(failure.status, "bounded_host_boundary");
        assert_eq!(failure.last_observed, "native-tool-dispatch-entered");
        assert_eq!(failure.first_unobserved, "native-tool-dispatch-returned");
        assert_eq!(failure.fault_region, "native_tool_dispatch");
        assert_eq!(failure.next_probe, "inspect_native_tool_dispatch_journal");
        assert_eq!(failure.dropped_events, 0);
    }

    #[test]
    fn missing_checkpoint_numbers_cannot_certify_host_completion() {
        let journal = PassiveTraceJournal::default();
        assert!(journal.begin(A));
        for stage in [
            NativeStage::TurnReceived,
            NativeStage::WorkerEntered,
            NativeStage::ModelLoopEntered,
            NativeStage::ModelLoopReturned,
            NativeStage::TurnSucceeded,
        ] {
            journal.record(A, stage);
        }
        let original = journal.snapshot().unwrap();
        assert_eq!(original.dropped_events, 0);
        assert_eq!(
            localize_passive_host_trace(&original).status,
            "host_turn_returned"
        );

        let mut missing_middle = original.clone();
        missing_middle.events.remove(1);
        assert_eq!(
            localize_passive_host_trace(&missing_middle).status,
            "insufficient_evidence"
        );

        let mut missing_prefix = original.clone();
        missing_prefix.events.remove(0);
        assert_eq!(
            localize_passive_host_trace(&missing_prefix).status,
            "insufficient_evidence"
        );

        let mut forged_gap = original.clone();
        for event in forged_gap.events.iter_mut().skip(2) {
            event.sequence += 1;
        }
        assert_eq!(
            localize_passive_host_trace(&forged_gap).status,
            "insufficient_evidence"
        );
    }

    #[test]
    fn cancellation_during_model_loop_reports_unverified_worker_exit() {
        let journal = PassiveTraceJournal::default();
        assert!(journal.begin(A));
        journal.record(A, NativeStage::TurnReceived);
        journal.record(A, NativeStage::WorkerEntered);
        journal.record(A, NativeStage::ModelLoopEntered);
        journal.record(A, NativeStage::CancelSignalled);
        let finding = localize_passive_host_trace(&journal.snapshot().unwrap());
        assert_eq!(finding.status, "in_progress");
        assert_eq!(finding.last_observed, "native-cancel-signalled");
        assert_eq!(finding.first_unobserved, "worker_terminated_unobserved");
        assert_eq!(finding.fault_region, "worker_recovery");
        assert_eq!(finding.next_probe, "check_worker_exit_and_journal");

        // A later genuine failure remains classified from the terminal
        // host result, not as proof that cancellation killed the worker.
        journal.record(A, NativeStage::TurnFailed);
        let failure = localize_passive_host_trace(&journal.snapshot().unwrap());
        assert_eq!(failure.status, "bounded_host_boundary");
        assert_eq!(failure.fault_region, "model_or_tool_continuation");
    }

    #[test]
    fn a_cancel_signal_is_not_worker_termination() {
        let journal = PassiveTraceJournal::default();
        assert!(journal.begin(A));
        journal.record(A, NativeStage::CancelSignalled);
        let finding = localize_passive_host_trace(&journal.snapshot().unwrap());
        assert_eq!(finding.status, "in_progress");
        assert_eq!(finding.first_unobserved, "worker_terminated_unobserved");
    }
}
