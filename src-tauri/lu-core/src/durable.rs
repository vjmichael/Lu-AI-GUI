//! Durable native dispatch for operations whose effects must not be replayed.
//!
//! A successful effect is replayed from the persisted Protobuf receipt, not
//! re-executed. Interrupted or ambiguously failed effects are held UNCERTAIN.
//! This dispatcher is currently opt-in: Cowork is not using it yet.

use crate::{
    contracts::{ProtoPayload, ToolAuthorizer, ToolFailure, ToolInvocation, ToolOutput},
    journal::{EffectJournal, EffectState, JournalError, PrepareOutcome},
    privilege_guard::enforce_model_ceiling,
    registry::{DispatchError, NativeToolRegistry, ToolResponse},
    wire::{self, proto},
};

fn journal_error(error: JournalError) -> DispatchError {
    DispatchError::Execution(ToolFailure::new(
        "JOURNAL_FAILURE",
        format!("effect journal unavailable: {error:?}; do not retry blindly"),
        false,
    ))
}

fn halted(reason: impl Into<String>) -> DispatchError {
    DispatchError::Execution(ToolFailure::new("EFFECT_REPLAY_BLOCKED", reason, false))
}

pub struct DurableDispatcher<'a> {
    pub registry: &'a NativeToolRegistry,
    pub journal: &'a EffectJournal,
}

impl DurableDispatcher<'_> {
    pub async fn dispatch(
        &self,
        invocation: ToolInvocation,
        authorizer: &dyn ToolAuthorizer,
    ) -> ToolResponse {
        // Recheck current authorization before revealing even a cached
        // committed result. User permission revocation must take effect even
        // when a previous call_id already has a receipt.
        let descriptor = self
            .registry
            .tools()
            .map_err(|_| halted("native registry unavailable"))?
            .into_iter()
            .find(|tool| tool.id == invocation.tool_id)
            .ok_or_else(|| DispatchError::UnknownTool(invocation.tool_id.clone()))?;
        if invocation.input.type_url != descriptor.input_type_url {
            return Err(DispatchError::InputTypeMismatch {
                expected: descriptor.input_type_url,
                actual: invocation.input.type_url,
            });
        }
        // Cached receipts cannot bypass a newly applied privilege ceiling.
        enforce_model_ceiling(descriptor.effect).map_err(DispatchError::ProtectedEffect)?;
        authorizer
            .authorize(&invocation, &descriptor)
            .map_err(|e| DispatchError::Denied(e.reason))?;
        let prior = self.journal.prepare(&invocation).map_err(journal_error)?;
        match prior {
            PrepareOutcome::Existing(EffectState::Applied) => {
                let record = self
                    .journal
                    .get(&invocation.call_id)
                    .map_err(journal_error)?
                    .ok_or_else(|| halted("committed effect has no journal entry"))?;
                let bytes = record
                    .receipt
                    .ok_or_else(|| halted("committed effect has no durable receipt"))?;
                let reply = wire::decode_reply(&bytes)
                    .map_err(|_| halted("stored Protobuf effect receipt is unreadable"))?;
                if reply.call_id != invocation.call_id {
                    return Err(halted("stored receipt call ID mismatch"));
                }
                return match reply.outcome {
                    Some(proto::tool_reply::Outcome::Output(bytes)) => Ok(ToolOutput {
                        payload: ProtoPayload {
                            type_url: reply.output_type_url,
                            bytes,
                        },
                    }),
                    _ => Err(halted("committed effect has no success result")),
                };
            }
            PrepareOutcome::Existing(EffectState::Executing)
            | PrepareOutcome::Existing(EffectState::Uncertain)
            | PrepareOutcome::Existing(EffectState::Failed) => {
                return Err(halted(
                    "effect has already begun, failed, or is uncertain; reconcile before proceeding",
                ));
            }
            PrepareOutcome::New | PrepareOutcome::Existing(EffectState::Prepared) => {}
        }

        // Transition to EXECUTING is a compare-and-swap guard against two
        // concurrent callers running the same call_id.
        self.journal
            .mark_executing(&invocation.call_id)
            .map_err(journal_error)?;

        let call_id = invocation.call_id.clone();
        let response = self.registry.dispatch(invocation, authorizer).await;
        match &response {
            Ok(_) => {
                let encoded = wire::encode_reply(&call_id, &response).map_err(|error| {
                    let _ = self.journal.mark_uncertain(&call_id);
                    halted(format!(
                        "effect succeeded but receipt encoding failed: {error:?}"
                    ))
                })?;
                self.journal
                    .mark_applied(&call_id, &encoded)
                    .map_err(journal_error)?;
            }
            Err(DispatchError::Execution(_)) => {
                // We cannot infer from a tool error whether partial effects
                // occurred. Err on the side of requiring reconciliation.
                self.journal
                    .mark_uncertain(&call_id)
                    .map_err(journal_error)?;
            }
            Err(_) => {
                // No native implementation was invoked (validation/authorization
                // failed), so no side effect should have occurred.
                self.journal.mark_failed(&call_id).map_err(journal_error)?;
            }
        }
        response
    }
}
