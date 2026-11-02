//! DX-00 synthetic schema/negative fixtures. Not Windows process evidence.
use lu_core::diagnostic_contract::{
    decode_diagnostic_trace, encode_diagnostic_trace, DiagnosticWireError,
    DIAGNOSTIC_PROTOCOL_VERSION, MAX_DIAGNOSTIC_EVENTS, MAX_DIAGNOSTIC_FRAME_BYTES,
};
use lu_core::wire::proto;

fn sample() -> proto::DiagnosticTrace {
    proto::DiagnosticTrace {
        protocol_version: DIAGNOSTIC_PROTOCOL_VERSION,
        trace_ref: "0123456789abcdef0123456789abcdef".into(),
        dropped_events: 0,
        events: vec![
            proto::DiagnosticEvent {
                sequence: 1,
                parent_sequence: 0,
                stage: 1,
                outcome: 2,
                elapsed_millis: 30,
                clock_domain: 1,
                failure_code: 0,
                attempt: 1,
                clock_epoch: 1,
            },
            proto::DiagnosticEvent {
                sequence: 2,
                parent_sequence: 1,
                stage: 2,
                outcome: 2,
                elapsed_millis: 45,
                clock_domain: 1,
                failure_code: 0,
                attempt: 1,
                clock_epoch: 1,
            },
            proto::DiagnosticEvent {
                sequence: 3,
                parent_sequence: 2,
                stage: 3,
                outcome: 3,
                elapsed_millis: 100,
                clock_domain: 2,
                failure_code: 7,
                attempt: 1,
                clock_epoch: 1,
            },
        ],
    }
}

#[test]
fn codec_roundtrips_a_strictly_bounded_trace() {
    let original = sample();
    let bytes = encode_diagnostic_trace(&original).unwrap();
    assert_eq!(decode_diagnostic_trace(&bytes).unwrap(), original);
    assert!(bytes.len() < MAX_DIAGNOSTIC_FRAME_BYTES);
    // Provenance is an ephemeral, non-authorizing opaque reference.
    assert!(!format!("{original:?}").contains("user_turn_id"));
}

#[test]
fn reject_invalid_version_reference_and_missing_events() {
    let mut trace = sample();
    trace.protocol_version = 0;
    assert_eq!(
        encode_diagnostic_trace(&trace),
        Err(DiagnosticWireError::InvalidVersion)
    );
    let mut trace = sample();
    trace.trace_ref = "C:\\Users\\secret\\path".into();
    assert_eq!(
        encode_diagnostic_trace(&trace),
        Err(DiagnosticWireError::InvalidReference)
    );
    let mut trace = sample();
    trace.events.clear();
    assert_eq!(
        encode_diagnostic_trace(&trace),
        Err(DiagnosticWireError::MissingEvents)
    );
}

#[test]
fn reject_unspecified_event_fields_and_invalid_failure_codes() {
    for field in 0..5 {
        let mut trace = sample();
        match field {
            0 => trace.events[1].stage = 0,
            1 => trace.events[1].outcome = 99,
            2 => trace.events[1].clock_domain = 0,
            3 => trace.events[1].failure_code = 3,
            _ => trace.events[1].attempt = 0,
        }
        assert_eq!(
            encode_diagnostic_trace(&trace),
            Err(DiagnosticWireError::InvalidEvent)
        );
    }
    let mut trace = sample();
    trace.events[1].clock_epoch = 0;
    assert_eq!(
        encode_diagnostic_trace(&trace),
        Err(DiagnosticWireError::InvalidEvent)
    );
    let mut trace = sample();
    trace.events[1].clock_epoch = 33;
    assert_eq!(
        encode_diagnostic_trace(&trace),
        Err(DiagnosticWireError::InvalidEvent)
    );
    let mut trace = sample();
    trace.events[2].failure_code = 0;
    assert_eq!(
        encode_diagnostic_trace(&trace),
        Err(DiagnosticWireError::InvalidEvent)
    );
}

#[test]
fn reject_reordered_missing_parents_and_forged_cross_clock_timings() {
    let mut trace = sample();
    trace.events[1].sequence = 1;
    assert_eq!(
        encode_diagnostic_trace(&trace),
        Err(DiagnosticWireError::InvalidSequence)
    );
    let mut trace = sample();
    trace.events[1].parent_sequence = 999;
    assert_eq!(
        encode_diagnostic_trace(&trace),
        Err(DiagnosticWireError::InvalidSequence)
    );
    let mut trace = sample();
    trace.events[1].elapsed_millis = 20;
    assert_eq!(
        encode_diagnostic_trace(&trace),
        Err(DiagnosticWireError::InvalidClock)
    );

    let mut trace = sample();
    // Clocks from WebView and host legitimately overlap in absolute numbers.
    trace.events[2].elapsed_millis = 1;
    assert!(encode_diagnostic_trace(&trace).is_ok());
}

#[test]
fn allow_explicitly_recorded_loss_but_never_treat_loss_as_success() {
    let mut trace = sample();
    trace.events.remove(1);
    assert_eq!(
        encode_diagnostic_trace(&trace),
        Err(DiagnosticWireError::InvalidSequence)
    );
    trace.dropped_events = 1;
    assert!(encode_diagnostic_trace(&trace).is_ok());
}

#[test]
fn reject_unreported_missing_sequence_even_with_reparented_causal_edges() {
    let trace = sample();
    // Reparenting makes the remaining chain look legitimate, but evidence
    // with ordinal 2 missing is still incomplete.
    let mut erased = trace.clone();
    erased.events.remove(1);
    erased.events[1].parent_sequence = 1;
    assert_eq!(
        encode_diagnostic_trace(&erased),
        Err(DiagnosticWireError::InvalidSequence)
    );

    // A missing prefix must not certify a suffix as a complete turn.
    let mut prefix = trace.clone();
    prefix.events.remove(0);
    prefix.events[0].parent_sequence = 0;
    assert_eq!(
        encode_diagnostic_trace(&prefix),
        Err(DiagnosticWireError::InvalidSequence)
    );

    // A fake gap with otherwise intact parent links is equally invalid.
    let mut gap = trace.clone();
    gap.events[2].sequence = 4;
    assert_eq!(
        encode_diagnostic_trace(&gap),
        Err(DiagnosticWireError::InvalidSequence)
    );

    // Explicitly accounted evidence loss can be represented, but must be
    // classified as insufficient, never complete, by the localizer.
    erased.dropped_events = 1;
    assert!(encode_diagnostic_trace(&erased).is_ok());
    prefix.dropped_events = 1;
    assert!(encode_diagnostic_trace(&prefix).is_ok());
}

#[test]
fn oversized_and_noncanonical_fields_are_rejected() {
    let mut trace = sample();
    trace.events = vec![trace.events[0]; MAX_DIAGNOSTIC_EVENTS + 1];
    assert_eq!(
        encode_diagnostic_trace(&trace),
        Err(DiagnosticWireError::Oversized)
    );
    assert_eq!(
        decode_diagnostic_trace(&vec![0; MAX_DIAGNOSTIC_FRAME_BYTES + 1]),
        Err(DiagnosticWireError::Oversized)
    );
    let valid = encode_diagnostic_trace(&sample()).unwrap();
    let mut with_unknown = valid.clone();
    with_unknown.extend_from_slice(&[0x98, 0x06, 0x01]);
    assert_eq!(
        decode_diagnostic_trace(&with_unknown),
        Err(DiagnosticWireError::Noncanonical)
    );
    let mut with_duplicate = valid;
    // Duplicate a protocol-v2 field, not the old v1 field: the decoder
    // must reject the frame for noncanonical repetition, not version mismatch.
    with_duplicate.extend_from_slice(&[0x08, 0x02]);
    assert_eq!(
        decode_diagnostic_trace(&with_duplicate),
        Err(DiagnosticWireError::Noncanonical)
    );
}

#[test]
fn new_worker_epoch_allows_clock_reset_within_one_attempt_but_not_reentry() {
    let mut trace = sample();
    let mut restarted = trace.events[2];
    restarted.sequence = 4;
    restarted.parent_sequence = 3;
    restarted.stage = 8; // Model loop entered in a restarted worker.
    restarted.outcome = 1;
    restarted.failure_code = 0;
    restarted.clock_epoch = 2;
    restarted.elapsed_millis = 1; // New process monotonic origin.
    trace.events.push(restarted);
    assert!(encode_diagnostic_trace(&trace).is_ok());
    assert_eq!(
        decode_diagnostic_trace(&encode_diagnostic_trace(&trace).unwrap()).unwrap(),
        trace
    );

    // An earlier epoch cannot reappear after the restarted worker, even
    // if the new event timestamp would otherwise appear monotonic.
    let mut old = trace.events[3];
    old.sequence = 5;
    old.parent_sequence = 4;
    old.clock_epoch = 1;
    old.elapsed_millis = 999;
    trace.events.push(old);
    assert_eq!(
        encode_diagnostic_trace(&trace),
        Err(DiagnosticWireError::InvalidClock)
    );
}

#[test]
fn reset_without_new_epoch_and_old_protocol_are_rejected() {
    let mut trace = sample();
    let mut resumed = trace.events[2];
    resumed.sequence = 4;
    resumed.parent_sequence = 3;
    resumed.clock_epoch = 1; // Same worker clock, illegally backwards.
    resumed.elapsed_millis = 1;
    trace.events.push(resumed);
    assert_eq!(
        encode_diagnostic_trace(&trace),
        Err(DiagnosticWireError::InvalidClock)
    );

    let mut legacy = sample();
    legacy.protocol_version = 1;
    assert_eq!(
        encode_diagnostic_trace(&legacy),
        Err(DiagnosticWireError::InvalidVersion)
    );
}
