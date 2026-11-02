//! DX-15A: convert real, observed native-host ring events into a validated,
//! content-free Protobuf witness. This is NOT a complete causal conversation
//! trace and never invents frontend, worker-process, or persistence checkpoints.

use crate::{
    diagnostic_contract::{
        decode_diagnostic_trace, encode_diagnostic_trace, DiagnosticWireError,
        DIAGNOSTIC_PROTOCOL_VERSION,
    },
    passive_diagnostics::PassiveTraceSnapshot,
    wire::proto,
};

/// A wire-validated projection of host observations only. Source ordinals and
/// the original single host monotonic clock are retained without changing
/// their meaning. Zero parents intentionally mean causal ancestry unverified.
#[derive(Debug)]
pub struct HostTypedWitness {
    pub trace: proto::DiagnosticTrace,
    pub omitted_stage_count: u32,
    pub wire_bytes: usize,
}

fn classified_host_observation(stage: &str) -> Option<(i32, i32, i32)> {
    // Protocol stage / observed outcome / fixed failure category.
    // Host terminal success is NOT native IPC return, assistant save, or
    // render, so it is deliberately omitted from this typed projection.
    match stage {
        "native-turn-received" => Some((4, 1, 0)),
        "native-session-ready" => Some((19, 2, 0)),
        "saved-user-attested" => Some((5, 2, 0)),
        "native-task-authorized" => Some((6, 2, 0)),
        "native-worker-entered" => Some((7, 1, 0)),
        "native-model-loop-entered" => Some((8, 1, 0)),
        "native-model-loop-returned" => Some((9, 2, 0)),
        "native-tool-dispatch-entered" => Some((14, 1, 0)),
        "native-tool-dispatch-returned" => Some((15, 2, 0)),
        "native-cancel-signalled" => Some((16, 1, 0)),
        "native-turn-failed" => Some((21, 3, 1)),
        _ => None,
    }
}

/// A source-ring observation is independently necessary before any typed
/// witness is offered. Reparenting and typed stage synthesis are forbidden.
pub fn bridge_observed_host_trace(
    source: &PassiveTraceSnapshot,
) -> Result<Option<HostTypedWitness>, DiagnosticWireError> {
    if source.recording_state != "accepted_turn" || source.clock_domain != "host" {
        return Ok(None);
    }
    let Some(reference) = source.trace_ref.as_ref() else {
        return Ok(None);
    };
    if source.events.is_empty() {
        return Ok(None);
    }
    // Fail closed on missing source ordinals even if the typed projection
    // would otherwise account for intentionally unmapped stage names.
    if source.dropped_events == 0
        && source.events.iter().enumerate().any(|(index, event)| {
            event.sequence != index as u64 + 1
                || (index > 0 && event.elapsed_millis < source.events[index - 1].elapsed_millis)
        })
    {
        return Err(DiagnosticWireError::InvalidSequence);
    }

    let mut events = Vec::new();
    let mut omitted = 0u32;
    for event in &source.events {
        if let Some((stage, outcome, failure_code)) = classified_host_observation(event.stage) {
            events.push(proto::DiagnosticEvent {
                sequence: event.sequence,
                parent_sequence: 0,
                stage,
                outcome,
                elapsed_millis: event.elapsed_millis,
                clock_domain: 2,
                failure_code,
                attempt: 1,
                clock_epoch: 1,
            });
        } else {
            omitted = omitted
                .checked_add(1)
                .ok_or(DiagnosticWireError::Oversized)?;
        }
    }
    if events.is_empty() {
        return Ok(None);
    }
    let losses = source
        .dropped_events
        .checked_add(u64::from(omitted))
        .ok_or(DiagnosticWireError::Oversized)?;
    let dropped_events = u32::try_from(losses).map_err(|_| DiagnosticWireError::Oversized)?;
    let trace = proto::DiagnosticTrace {
        protocol_version: DIAGNOSTIC_PROTOCOL_VERSION,
        trace_ref: reference.clone(),
        dropped_events,
        events,
    };
    let encoded = encode_diagnostic_trace(&trace)?;
    if decode_diagnostic_trace(&encoded)? != trace {
        return Err(DiagnosticWireError::Noncanonical);
    }
    Ok(Some(HostTypedWitness {
        trace,
        omitted_stage_count: omitted,
        wire_bytes: encoded.len(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        diagnostic_localization::{localize_normal_turn, EvidenceStrength},
        passive_diagnostics::{NativeStage, PassiveTraceJournal},
    };

    const REFERENCE: &str = "0123456789abcdef0123456789abcdef";

    fn admitted() -> PassiveTraceSnapshot {
        let journal = PassiveTraceJournal::default();
        assert!(journal.begin(REFERENCE));
        for stage in [
            NativeStage::TurnReceived,
            NativeStage::RecoveryReady,
            NativeStage::SessionReady,
            NativeStage::SavedUserAttested,
            NativeStage::TurnRegistered,
            NativeStage::WorkerEntered,
            NativeStage::TaskAuthorized,
            NativeStage::ModelLoopEntered,
            NativeStage::ModelLoopReturned,
            NativeStage::TurnSucceeded,
        ] {
            journal.record(REFERENCE, stage);
        }
        journal.snapshot().unwrap()
    }

    #[test]
    fn real_host_observations_cross_the_strict_protobuf_codec_without_causal_invention() {
        let source = admitted();
        let witness = bridge_observed_host_trace(&source).unwrap().unwrap();
        assert_eq!(witness.trace.trace_ref, REFERENCE);
        assert_eq!(witness.trace.protocol_version, DIAGNOSTIC_PROTOCOL_VERSION);
        assert_eq!(witness.omitted_stage_count, 3);
        assert_eq!(witness.trace.dropped_events, 3);
        assert_eq!(witness.trace.events.len(), 7);
        assert!(witness.wire_bytes > 0);
        assert_eq!(witness.trace.events[0].sequence, 1);
        assert_eq!(witness.trace.events[0].stage, 4);
        assert_eq!(witness.trace.events.last().unwrap().stage, 9);
        assert!(witness.trace.events.iter().all(|event| {
            event.parent_sequence == 0 && event.clock_domain == 2 && event.clock_epoch == 1
        }));
        let finding = localize_normal_turn(&witness.trace).unwrap();
        assert_eq!(
            finding.evidence_strength,
            EvidenceStrength::InsufficientEvidence
        );
    }

    #[test]
    fn actual_host_dispatch_events_preserve_typed_entry_return_without_claiming_tool_success() {
        let journal = PassiveTraceJournal::default();
        assert!(journal.begin(REFERENCE));
        for stage in [
            NativeStage::TurnReceived,
            NativeStage::ModelLoopEntered,
            NativeStage::ToolDispatchEntered,
            NativeStage::ToolDispatchReturned,
            NativeStage::ModelLoopReturned,
            NativeStage::TurnSucceeded,
        ] {
            journal.record(REFERENCE, stage);
        }
        let witness = bridge_observed_host_trace(&journal.snapshot().unwrap())
            .unwrap()
            .unwrap();
        let stages: Vec<_> = witness
            .trace
            .events
            .iter()
            .map(|event| event.stage)
            .collect();
        assert_eq!(stages, [4, 8, 14, 15, 9]);
        assert_eq!(witness.trace.dropped_events, 1);
        assert_eq!(witness.trace.events[3].outcome, 2);
        assert!(witness
            .trace
            .events
            .iter()
            .all(|event| event.parent_sequence == 0));
        assert_eq!(
            localize_normal_turn(&witness.trace)
                .unwrap()
                .evidence_strength,
            EvidenceStrength::InsufficientEvidence
        );
    }

    #[test]
    fn missing_or_forged_source_events_never_make_a_valid_witness() {
        let mut source = admitted();
        source.events.remove(2);
        assert!(matches!(
            bridge_observed_host_trace(&source),
            Err(DiagnosticWireError::InvalidSequence)
        ));
        let mut source = admitted();
        source.events[0].sequence = 2;
        assert!(matches!(
            bridge_observed_host_trace(&source),
            Err(DiagnosticWireError::InvalidSequence)
        ));
        let mut source = admitted();
        source.trace_ref = Some("not a host reference".into());
        assert!(matches!(
            bridge_observed_host_trace(&source),
            Err(DiagnosticWireError::InvalidReference)
        ));
        let mut source = admitted();
        source.recording_state = "admission_unobserved";
        assert!(bridge_observed_host_trace(&source).unwrap().is_none());
    }

    #[test]
    fn failure_and_cancel_are_observed_but_not_worker_termination() {
        let journal = PassiveTraceJournal::default();
        assert!(journal.begin(REFERENCE));
        for stage in [
            NativeStage::TurnReceived,
            NativeStage::WorkerEntered,
            NativeStage::ModelLoopEntered,
            NativeStage::CancelSignalled,
            NativeStage::TurnFailed,
        ] {
            journal.record(REFERENCE, stage);
        }
        let witness = bridge_observed_host_trace(&journal.snapshot().unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(witness.omitted_stage_count, 0);
        assert_eq!(witness.trace.events.last().unwrap().stage, 21);
        assert_eq!(witness.trace.events.last().unwrap().outcome, 3);
        assert_eq!(witness.trace.events.last().unwrap().failure_code, 1);
        assert!(witness.trace.events.iter().any(|event| event.stage == 16));
        assert!(!witness.trace.events.iter().any(|event| event.stage == 17));
        assert_eq!(
            localize_normal_turn(&witness.trace)
                .unwrap()
                .evidence_strength,
            EvidenceStrength::InsufficientEvidence
        );
    }

    #[test]
    fn accounted_loss_remains_loss_and_never_becomes_proof_of_completion() {
        let mut source = admitted();
        source.events.remove(2);
        source.dropped_events = 1;
        let witness = bridge_observed_host_trace(&source).unwrap().unwrap();
        assert!(witness.trace.dropped_events > 0);
        assert_eq!(
            localize_normal_turn(&witness.trace)
                .unwrap()
                .evidence_strength,
            EvidenceStrength::InsufficientEvidence
        );
    }
}
