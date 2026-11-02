//! DX-00: internal Protobuf-only, content-free diagnostic trace validation.
//!
//! Trace references must originate with the trusted host. An apparently valid
//! reference is NEVER an authority/permission token. Observations must not
//! carry prompts, filesystem paths, tool arguments, credentials or model text.
use crate::wire::proto;
use prost::Message;
use std::collections::{BTreeMap, BTreeSet};

pub const DIAGNOSTIC_PROTOCOL_VERSION: u32 = 2;
pub const MAX_DIAGNOSTIC_EVENTS: usize = 64;
pub const MAX_DIAGNOSTIC_FRAME_BYTES: usize = 8 * 1024;
pub const MAX_DIAGNOSTIC_ATTEMPTS: u32 = 32;
pub const MAX_DIAGNOSTIC_CLOCK_EPOCHS: u32 = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticWireError {
    Oversized,
    Decode,
    InvalidVersion,
    InvalidReference,
    MissingEvents,
    InvalidEvent,
    InvalidSequence,
    InvalidClock,
    Noncanonical,
}

fn valid_trace_ref(reference: &str) -> bool {
    reference.len() == 32
        && reference
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub fn validate_diagnostic_trace(
    trace: &proto::DiagnosticTrace,
) -> Result<(), DiagnosticWireError> {
    if trace.protocol_version != DIAGNOSTIC_PROTOCOL_VERSION {
        return Err(DiagnosticWireError::InvalidVersion);
    }
    if !valid_trace_ref(&trace.trace_ref) {
        return Err(DiagnosticWireError::InvalidReference);
    }
    if trace.events.is_empty() {
        return Err(DiagnosticWireError::MissingEvents);
    }
    if trace.events.len() > MAX_DIAGNOSTIC_EVENTS
        || trace.encoded_len() > MAX_DIAGNOSTIC_FRAME_BYTES
    {
        return Err(DiagnosticWireError::Oversized);
    }

    let mut last_sequence = 0u64;
    let mut last_attempt = 0u32;
    let mut seen = BTreeSet::new();
    // Repeated events from a restarted process must not be compared against
    // an earlier monotonic origin, even if the logical attempt is unchanged.
    let mut clocks: BTreeMap<(i32, u32, u32), u64> = BTreeMap::new();
    let mut last_epochs: BTreeMap<(i32, u32), u32> = BTreeMap::new();
    for event in &trace.events {
        if !(1..=21).contains(&event.stage)
            || !(1..=5).contains(&event.outcome)
            || !(1..=3).contains(&event.clock_domain)
            || !(0..=11).contains(&event.failure_code)
            || !(1..=MAX_DIAGNOSTIC_ATTEMPTS).contains(&event.attempt)
            || !(1..=MAX_DIAGNOSTIC_CLOCK_EPOCHS).contains(&event.clock_epoch)
        {
            return Err(DiagnosticWireError::InvalidEvent);
        }

        // Failure codes are fixed categories only. Never allow free-form
        // exceptions, arbitrary payloads or a failure code on a success.
        let failure = matches!(event.outcome, 3..=5);
        if (failure && event.failure_code == 0) || (!failure && event.failure_code != 0) {
            return Err(DiagnosticWireError::InvalidEvent);
        }

        if event.attempt < last_attempt {
            return Err(DiagnosticWireError::InvalidSequence);
        }
        last_attempt = event.attempt;
        // A zero-loss Protobuf trace must contain *every* ordinal, including
        // the first event. Strictly increasing alone would accept an erased
        // middle checkpoint if the remaining parent links were reparented.
        let expected = last_sequence.checked_add(1);
        if event.sequence == 0
            || event.sequence <= last_sequence
            || (trace.dropped_events == 0 && expected != Some(event.sequence))
        {
            return Err(DiagnosticWireError::InvalidSequence);
        }
        if event.parent_sequence != 0
            && (event.parent_sequence >= event.sequence
                || (!seen.contains(&event.parent_sequence) && trace.dropped_events == 0))
        {
            return Err(DiagnosticWireError::InvalidSequence);
        }
        last_sequence = event.sequence;
        seen.insert(event.sequence);

        // A new process incarnation must advance its host-minted epoch.
        // Epochs may reset on a NEW logical attempt, but never go backwards
        // within the same process domain and attempt (including interleaving).
        let domain_attempt = (event.clock_domain, event.attempt);
        if let Some(last) = last_epochs.insert(domain_attempt, event.clock_epoch) {
            if event.clock_epoch < last {
                return Err(DiagnosticWireError::InvalidClock);
            }
        }
        // Only compare monotonic times within one domain, retry and epoch;
        // an exited worker's clock does not continue after process relaunch.
        let clock_key = (event.clock_domain, event.attempt, event.clock_epoch);
        if let Some(previous) = clocks.insert(clock_key, event.elapsed_millis) {
            if event.elapsed_millis < previous {
                return Err(DiagnosticWireError::InvalidClock);
            }
        }
    }
    Ok(())
}

pub fn encode_diagnostic_trace(
    trace: &proto::DiagnosticTrace,
) -> Result<Vec<u8>, DiagnosticWireError> {
    validate_diagnostic_trace(trace)?;
    Ok(trace.encode_to_vec())
}

pub fn decode_diagnostic_trace(
    bytes: &[u8],
) -> Result<proto::DiagnosticTrace, DiagnosticWireError> {
    if bytes.is_empty() || bytes.len() > MAX_DIAGNOSTIC_FRAME_BYTES {
        return Err(DiagnosticWireError::Oversized);
    }
    let trace = proto::DiagnosticTrace::decode(bytes).map_err(|_| DiagnosticWireError::Decode)?;
    validate_diagnostic_trace(&trace)?;
    // Reject unknown/duplicate fields (prost would otherwise ignore them).
    if trace.encode_to_vec() != bytes {
        return Err(DiagnosticWireError::Noncanonical);
    }
    Ok(trace)
}
