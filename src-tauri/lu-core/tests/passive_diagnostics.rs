//! Synthetic diagnostics-contract fixtures, NOT Windows GUI proof.
use lu_core::passive_diagnostics::{
    NativeStage, PassiveDiagnostics, PassiveTraceJournal, MAX_PASSIVE_EVENTS,
};

#[test]
fn passive_ring_is_bounded_and_sequences_are_ordered() {
    let mut trace = PassiveDiagnostics::default();
    for _ in 0..100 {
        trace.record(NativeStage::TurnReceived);
    }
    let events = trace.snapshot();
    assert_eq!(events.len(), MAX_PASSIVE_EVENTS);
    assert_eq!(events[0].sequence, 37);
    assert_eq!(events[MAX_PASSIVE_EVENTS - 1].sequence, 100);
    assert!(events
        .windows(2)
        .all(|pair| pair[0].sequence < pair[1].sequence));
    assert!(events
        .windows(2)
        .all(|pair| { pair[0].elapsed_millis <= pair[1].elapsed_millis }));
}

#[test]
fn trace_serialization_never_contains_user_content_or_identifiers() {
    let mut trace = PassiveDiagnostics::default();
    for stage in [
        NativeStage::TurnReceived,
        NativeStage::SavedUserAttested,
        NativeStage::TaskAuthorized,
        NativeStage::ModelLoopEntered,
        NativeStage::ModelLoopReturned,
        NativeStage::TurnSucceeded,
        NativeStage::CancelSignalled,
    ] {
        trace.record(stage);
    }
    let json = serde_json::to_string(&trace.snapshot()).unwrap();
    for forbidden in [
        "userTurnId",
        "threadId",
        "sessionId",
        "modelRef",
        "modelPath",
        "prompt",
        "toolArguments",
        "apiKey",
    ] {
        assert!(!json.contains(forbidden), "unexpected diagnostic content");
    }
    assert!(json.contains("native-model-loop-returned"));
    assert!(json.contains("elapsedMillis"));
}

#[test]
fn explicit_rejection_stages_do_not_grant_authority() {
    let mut trace = PassiveDiagnostics::default();
    trace.record(NativeStage::SavedUserRejected);
    trace.record(NativeStage::RecoveryUnavailable);
    trace.record(NativeStage::TurnFailed);
    assert_eq!(
        trace
            .snapshot()
            .iter()
            .map(|event| event.stage)
            .collect::<Vec<_>>(),
        vec![
            "saved-user-rejected",
            "recovery-unavailable",
            "native-turn-failed"
        ],
    );
}

#[test]
fn journal_correlates_one_host_owned_turn_without_cross_turn_mixing() {
    let journal = PassiveTraceJournal::default();
    let first = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let second = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    assert!(journal.begin(first));
    journal.record(first, NativeStage::TurnReceived);
    journal.record(first, NativeStage::ModelLoopEntered);
    let snapshot = journal.snapshot().unwrap();
    assert_eq!(snapshot.trace_ref.as_deref(), Some(first));
    assert_eq!(snapshot.clock_domain, "host");
    assert_eq!(snapshot.events.len(), 2);
    assert_eq!(snapshot.dropped_events, 0);

    assert!(journal.begin(second));
    journal.record(first, NativeStage::TurnSucceeded); // stale completion ignored
    journal.record(second, NativeStage::TurnReceived);
    let snapshot = journal.snapshot().unwrap();
    assert_eq!(snapshot.trace_ref.as_deref(), Some(second));
    assert_eq!(snapshot.events.len(), 1);
    assert_eq!(snapshot.events[0].stage, "native-turn-received");
    assert_eq!(snapshot.events[0].sequence, 1);
}

#[test]
fn snapshot_marks_overwritten_evidence_without_leaking_content() {
    let journal = PassiveTraceJournal::default();
    let reference = "0123456789abcdef0123456789abcdef";
    assert!(journal.begin(reference));
    for _ in 0..MAX_PASSIVE_EVENTS + 7 {
        journal.record(reference, NativeStage::WorkerEntered);
    }
    journal.record(reference, NativeStage::CancelSignalled);
    let snapshot = journal.snapshot().unwrap();
    assert_eq!(snapshot.events.len(), MAX_PASSIVE_EVENTS);
    assert_eq!(snapshot.dropped_events, 8);
    assert_eq!(
        snapshot.events.last().unwrap().stage,
        "native-cancel-signalled"
    );
    assert_eq!(snapshot.schema, "lu.native.host-trace.v2");
    let report = serde_json::to_string(&snapshot).unwrap();
    assert!(report.contains("droppedEvents"));
    for forbidden in [
        "threadId",
        "userTurnId",
        "modelPath",
        "prompt",
        "filePath",
        "apiKey",
        "toolArguments",
        "conversationId",
    ] {
        assert!(!report.contains(forbidden), "unexpected field {forbidden}");
    }
}

#[test]
fn journal_never_accepts_path_or_arbitrary_text_as_a_correlation_reference() {
    let journal = PassiveTraceJournal::default();
    assert!(!journal.begin("C:\\Users\\Private\\secret.txt"));
    assert!(!journal.begin("INVALID-UPPER-CASE-REF-123456789"));
    assert!(!journal.begin(""));
    assert_eq!(journal.snapshot().unwrap().trace_ref, None);
    journal.record(
        "0123456789abcdef0123456789abcdef",
        NativeStage::TaskAuthorized,
    );
    assert!(journal.snapshot().unwrap().events.is_empty());
}
