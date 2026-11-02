//! DX-03 foundation: deterministic proximity diagnosis from validated trace.
//! Does not infer undocumented neural causes or fabricate missing events.
//! A correlated trace is required; legacy uncorrelated log rings are NOT proof.
use crate::diagnostic_contract::{validate_diagnostic_trace, DiagnosticWireError};
use crate::wire::proto;
use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceStrength {
    ObservedFailure,
    BoundedToNextStage,
    InsufficientEvidence,
    Complete,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticFinding {
    pub schema: &'static str,
    pub attempt: u32,
    pub confirmed_until: &'static str,
    pub first_unconfirmed: &'static str,
    pub fault_region: &'static str,
    pub failure_code: &'static str,
    pub evidence_strength: EvidenceStrength,
    pub next_probe: &'static str,
    // Measured only between checkpoints in the SAME process clock domain.
    // Cross-process timing differences are unknown without calibration.
    pub observed_interval_millis: Option<u64>,
    pub dropped_events: u32,
    // Deliberately no raw trace IDs, thread/message IDs or model/tool text.
}

#[derive(Clone, Copy)]
struct StageSpec {
    value: i32,
    label: &'static str,
    region: &'static str,
    probe: &'static str,
}

const NORMAL_TURN: &[StageSpec] = &[
    StageSpec {
        value: 1,
        label: "user_submitted",
        region: "desktop_ui",
        probe: "check_composer_submit",
    },
    StageSpec {
        value: 2,
        label: "user_saved",
        region: "message_persistence",
        probe: "verify_saved_user_role",
    },
    StageSpec {
        value: 3,
        label: "ipc_invoked",
        region: "frontend_transport",
        probe: "check_native_routing",
    },
    StageSpec {
        value: 4,
        label: "native_turn_received",
        region: "tauri_ipc",
        probe: "inspect_native_command_receipt",
    },
    StageSpec {
        value: 19,
        label: "session_ready",
        region: "engine_session",
        probe: "probe_engine_readiness",
    },
    StageSpec {
        value: 5,
        label: "saved_user_attested",
        region: "host_authority",
        probe: "verify_saved_turn_scope",
    },
    StageSpec {
        value: 6,
        label: "task_authorized",
        region: "host_authority",
        probe: "inspect_permission_denial_code",
    },
    StageSpec {
        value: 7,
        label: "worker_entered",
        region: "task_supervisor",
        probe: "check_worker_lifecycle",
    },
    StageSpec {
        value: 8,
        label: "model_loop_entered",
        region: "model_runtime",
        probe: "check_generation_request",
    },
    StageSpec {
        value: 9,
        label: "model_loop_returned",
        region: "model_runtime",
        probe: "check_engine_and_tool_turn",
    },
    StageSpec {
        value: 10,
        label: "native_ipc_returned",
        region: "tauri_ipc",
        probe: "check_native_reply_delivery",
    },
    StageSpec {
        value: 11,
        label: "stream_closed",
        region: "frontend_stream",
        probe: "check_ui_message_stream",
    },
    StageSpec {
        value: 12,
        label: "assistant_saved",
        region: "message_persistence",
        probe: "verify_assistant_write",
    },
    StageSpec {
        value: 13,
        label: "assistant_rendered",
        region: "desktop_ui",
        probe: "check_message_render_state",
    },
];

fn failure_label(code: i32) -> &'static str {
    match code {
        0 => "none",
        1 => "unknown",
        2 => "input_invalid",
        3 => "authority_denied",
        4 => "persistence",
        5 => "backend_unavailable",
        6 => "resource_exhausted",
        7 => "transport",
        8 => "timeout",
        9 => "cancelled",
        10 => "protocol",
        11 => "process_exit",
        _ => "invalid",
    }
}

fn confirms_stage(event: &proto::DiagnosticEvent) -> bool {
    // Entered proves arrival at a stage, not successful completion of the
    // enclosing operation. Durability/authority/return/render stages require
    // a separately observed successful outcome.
    event.outcome == 2 || (event.outcome == 1 && matches!(event.stage, 1 | 3 | 4 | 7 | 8 | 14 | 16))
}

fn stage_detail(stage: i32) -> StageSpec {
    if let Some(stage) = NORMAL_TURN
        .iter()
        .find(|candidate| candidate.value == stage)
    {
        return *stage;
    }
    match stage {
        14 => StageSpec {
            value: stage,
            label: "tool_dispatch_entered",
            region: "native_tool",
            probe: "inspect_typed_tool_result",
        },
        15 => StageSpec {
            value: stage,
            label: "tool_dispatch_returned",
            region: "native_tool",
            probe: "inspect_typed_tool_result",
        },
        16 => StageSpec {
            value: stage,
            label: "cancel_requested",
            region: "worker_recovery",
            probe: "check_worker_exit_and_journal",
        },
        17 => StageSpec {
            value: stage,
            label: "worker_terminated",
            region: "worker_recovery",
            probe: "check_worker_exit_and_journal",
        },
        18 => StageSpec {
            value: stage,
            label: "recovery_completed",
            region: "worker_recovery",
            probe: "check_worker_exit_and_journal",
        },
        20 | 21 => StageSpec {
            value: stage,
            label: "native_turn_rejected",
            region: "host_authority",
            probe: "inspect_host_denial_class",
        },
        _ => StageSpec {
            value: stage,
            label: "unknown",
            region: "unknown",
            probe: "collect_missing_stage_evidence",
        },
    }
}

// A matching trace and attempt do not prove two events share a causal path.
// Follow bounded, validated parent links before attributing one checkpoint to
// another. A separate diagnostic root must not complete an unrelated turn.
fn has_ancestor(
    event: &proto::DiagnosticEvent,
    ancestor_sequence: u64,
    events: &[&proto::DiagnosticEvent],
) -> bool {
    if ancestor_sequence == 0 {
        return true;
    }
    let mut parent_sequence = event.parent_sequence;
    for _ in 0..events.len() {
        if parent_sequence == ancestor_sequence {
            return true;
        }
        let Some(parent) = events
            .iter()
            .find(|parent| parent.sequence == parent_sequence)
        else {
            return false;
        };
        parent_sequence = parent.parent_sequence;
    }
    false
}

/// A normal-turn hypothesis only. Cancellation and tool side branches take
/// precedence when they carry a directly observed terminal failure.
/// Dropped/partial traces are explicitly insufficient evidence.
pub fn localize_normal_turn(
    trace: &proto::DiagnosticTrace,
) -> Result<DiagnosticFinding, DiagnosticWireError> {
    validate_diagnostic_trace(trace)?;
    let attempt = trace.events.last().map(|event| event.attempt).unwrap_or(1);
    let events: Vec<_> = trace
        .events
        .iter()
        .filter(|event| event.attempt == attempt)
        .collect();

    // Prefer actual observed failures to absence-of-progress speculation.
    if let Some(failed) = events.iter().find(|event| matches!(event.outcome, 3..=5)) {
        let stage = stage_detail(failed.stage);
        let previous_event = events
            .iter()
            .rfind(|event| {
                event.sequence < failed.sequence
                    && confirms_stage(event)
                    && has_ancestor(failed, event.sequence, &events)
            })
            .copied();
        let unrelated_checkpoint = previous_event.is_none()
            && events
                .iter()
                .any(|event| event.sequence < failed.sequence && confirms_stage(event));
        // Within one attempt, a terminal failure followed by a successful
        // checkpoint (or a success and failure of the same stage) cannot be
        // reported as a unique proven failure. Distinct simultaneous failures
        // are similarly ambiguous without causal branch resolution.
        let incompatible_completion = events.iter().any(|event| {
            event.outcome == 2 && (event.sequence > failed.sequence || event.stage == failed.stage)
        });
        let multiple_failures = events
            .iter()
            .filter(|event| matches!(event.outcome, 3..=5))
            .count()
            > 1;
        let predecessor = previous_event
            .map(|event| stage_detail(event.stage).label)
            .unwrap_or("none");
        let interval = previous_event.and_then(|event| {
            if event.clock_domain == failed.clock_domain
                && event.clock_epoch == failed.clock_epoch
                && event.attempt == failed.attempt
            {
                failed.elapsed_millis.checked_sub(event.elapsed_millis)
            } else {
                // Never calculate an interval across worker restart, even
                // where both events belong to one logical native task.
                None
            }
        });
        return Ok(DiagnosticFinding {
            schema: "lu.dx.finding.v1",
            attempt,
            confirmed_until: predecessor,
            first_unconfirmed: stage.label,
            fault_region: stage.region,
            failure_code: failure_label(failed.failure_code),
            evidence_strength: if trace.dropped_events == 0
                && !unrelated_checkpoint
                && !incompatible_completion
                && !multiple_failures
            {
                EvidenceStrength::ObservedFailure
            } else {
                EvidenceStrength::InsufficientEvidence
            },
            next_probe: stage.probe,
            observed_interval_millis: interval,
            dropped_events: trace.dropped_events,
        });
    }

    // An active tool or cancellation branch is not completed until the
    // corresponding terminal stage has been observed.
    for (started, terminal, region, probe) in [
        (14, 15, "native_tool", "inspect_typed_tool_result"),
        (16, 17, "worker_recovery", "check_worker_exit_and_journal"),
    ] {
        if events.iter().any(|event| event.stage == started)
            && !events
                .iter()
                .any(|event| event.stage == terminal && event.outcome == 2)
        {
            return Ok(DiagnosticFinding {
                schema: "lu.dx.finding.v1",
                attempt,
                confirmed_until: stage_detail(started).label,
                first_unconfirmed: stage_detail(terminal).label,
                fault_region: region,
                failure_code: "none",
                evidence_strength: if trace.dropped_events == 0 {
                    EvidenceStrength::BoundedToNextStage
                } else {
                    EvidenceStrength::InsufficientEvidence
                },
                next_probe: probe,
                observed_interval_millis: None,
                dropped_events: trace.dropped_events,
            });
        }
    }

    let mut confirmed = "none";
    let mut confirmed_sequence = 0u64;
    for (index, stage) in NORMAL_TURN.iter().enumerate() {
        let stage_events: Vec<_> = events
            .iter()
            .filter(|event| event.stage == stage.value && confirms_stage(event))
            .collect();
        if let Some(event) = stage_events.iter().find(|event| {
            event.sequence > confirmed_sequence && has_ancestor(event, confirmed_sequence, &events)
        }) {
            confirmed = stage.label;
            confirmed_sequence = event.sequence;
            continue;
        }
        // A checkpoint in an unrelated branch or earlier in the attempt
        // cannot complete this ordered causal path.
        let unlinked_checkpoint = !stage_events.is_empty();
        // A later confirmed event with an earlier missing stage means the
        // trace is incomplete or miscorrelated, not a proven skipped action.
        let later_observed = NORMAL_TURN[index + 1..].iter().any(|next| {
            events
                .iter()
                .any(|event| event.stage == next.value && confirms_stage(event))
        });
        return Ok(DiagnosticFinding {
            schema: "lu.dx.finding.v1",
            attempt,
            confirmed_until: confirmed,
            first_unconfirmed: stage.label,
            fault_region: stage.region,
            failure_code: "none",
            evidence_strength: if trace.dropped_events > 0 || later_observed || unlinked_checkpoint
            {
                EvidenceStrength::InsufficientEvidence
            } else {
                EvidenceStrength::BoundedToNextStage
            },
            next_probe: stage.probe,
            observed_interval_millis: None,
            dropped_events: trace.dropped_events,
        });
    }
    Ok(DiagnosticFinding {
        schema: "lu.dx.finding.v1",
        attempt,
        confirmed_until: "assistant_rendered",
        first_unconfirmed: "none",
        fault_region: "none",
        failure_code: "none",
        evidence_strength: if trace.dropped_events == 0 {
            EvidenceStrength::Complete
        } else {
            EvidenceStrength::InsufficientEvidence
        },
        next_probe: "none",
        observed_interval_millis: None,
        dropped_events: trace.dropped_events,
    })
}
