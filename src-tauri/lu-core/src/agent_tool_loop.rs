//! Lu-owned, model-agnostic native tool-turn controller.
//!
//! The model emits untrusted JSON at the external inference boundary. The
//! trusted host issues origin/scope, selects exposed schemas from existing
//! grants, validates each argument via registered Protobuf codecs, then
//! dispatches under ConversationAuthority. This does NOT yet operate the
//! GGUF streaming engine or persist a transcript; adapters integrate later.
//! There is no dependency on Jan Cowork or MCP.

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

use crate::{
    contracts::{ToolAuthorizer, ToolEffect},
    conversation_authority::{AuthorityError, BoundCallOrigin, ConversationAuthority},
    journal::EffectJournal,
    loop_guard::{LoopGuard, LoopVerdict, LoopReason},
    model::{ModelToolCatalog, ModelToolError, MAX_ADVERTISED_TOOLS},
    model_json_boundary::{
        parse_model_boundary, serialize_host_tool_feedback_boundary, JsonBoundaryError, JsonRepairStatus,
    },
    registry::NativeToolRegistry,
};

pub const MAX_MODEL_FRAME_BYTES: usize = 64 * 1024;

// Compiled-in host observation with no model-supplied identifiers or text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolDispatchCheckpoint {
    Entered,
    Returned,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ToolTurnOutcome {
    /// Trusted host should deliver the bounded structured result back to the
    /// model, then request its next inference step. Not a permission token.
    ToolResult {
        call_id: String,
        tool_name: String,
        data: Value,
    },
    /// Completed model answer. Persist only after trusted host validates and
    /// commits it to the corresponding conversation.
    FinalText(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolTurnError {
    Unauthorized(AuthorityError),
    Catalog(ModelToolError),
    MalformedModelReply,
    UnexpectedFields,
    TooLarge,
    NoAuthorizedTools,
    CounterOverflow,
    LoopTerminated,
    AlreadyFinished,
    Revoked,
    ReadOnlyTurnViolation,
    /// A durable dispatcher rejected or could not establish an effect receipt.
    /// Do not continue with another model call or retry the same effect.
    DurableDispatchFailed,
}

pub struct NativeToolTurn {
    origin: BoundCallOrigin,
    workspace_id: String,
    advertised: BTreeSet<String>,
    steps: u32,
    done: bool,
    last_repair: JsonRepairStatus,
    read_only: bool,
    loop_guard: LoopGuard,
}

impl NativeToolTurn {
    /// The origin and workspace are provided by a trusted host only, never
    /// created from model-provided role/IDs or a WebView argument.
    pub fn new(
        catalog: &ModelToolCatalog,
        authority: &ConversationAuthority,
        origin: BoundCallOrigin,
        host_workspace_id: String,
        requested_names: &BTreeSet<String>,
    ) -> Result<Self, ToolTurnError> {
        if host_workspace_id.trim().is_empty() {
            return Err(ToolTurnError::MalformedModelReply);
        }
        let permitted = authority
            .permitted_tool_ids(&origin)
            .map_err(ToolTurnError::Unauthorized)?;
        if permitted.is_empty() {
            return Err(ToolTurnError::Revoked);
        }
        let supported = catalog.names_for_native_ids(&permitted);
        let advertised: BTreeSet<String> =
            requested_names.intersection(&supported).cloned().collect();
        if advertised.is_empty() {
            return Err(ToolTurnError::NoAuthorizedTools);
        }
        if advertised.len() > MAX_ADVERTISED_TOOLS {
            return Err(ToolTurnError::Catalog(ModelToolError::ToolBudgetExceeded));
        }
        catalog
            .schemas(&advertised)
            .map_err(ToolTurnError::Catalog)?;
        Ok(Self {
            origin,
            workspace_id: host_workspace_id,
            advertised,
            steps: 0,
            done: false,
            last_repair: JsonRepairStatus::Unchanged,
            read_only: false,
            loop_guard: LoopGuard::default(),
        })
    }

    /// Production desktop read-only turn. Validate the selected registry
    /// descriptors at admission and again at dispatch, in case registrations
    /// change between model schema advertisement and execution.
    pub fn new_read_only(
        catalog: &ModelToolCatalog,
        registry: &NativeToolRegistry,
        authority: &ConversationAuthority,
        origin: BoundCallOrigin,
        host_workspace_id: String,
        requested_names: &BTreeSet<String>,
    ) -> Result<Self, ToolTurnError> {
        let mut turn = Self::new(
            catalog,
            authority,
            origin,
            host_workspace_id,
            requested_names,
        )?;
        let descriptors = registry
            .tools()
            .map_err(|_| ToolTurnError::ReadOnlyTurnViolation)?;
        for name in &turn.advertised {
            // encode_call resolves the catalog name to its native ID with a
            // typed empty payload only at dispatch. Here use the host-owned
            // native IDs projected by the catalog instead.
            let matching = descriptors.iter().any(|descriptor| {
                descriptor.effect == ToolEffect::ReadWorkspace
                    && catalog
                        .names_for_native_ids(&BTreeSet::from([descriptor.id.clone()]))
                        .contains(name)
            });
            if !matching {
                return Err(ToolTurnError::ReadOnlyTurnViolation);
            }
        }
        turn.read_only = true;
        Ok(turn)
    }

    pub fn schemas(&self, catalog: &ModelToolCatalog) -> Result<Vec<Value>, ToolTurnError> {
        catalog
            .schemas(&self.advertised)
            .map_err(ToolTurnError::Catalog)
    }

    pub fn step_count(&self) -> u32 {
        self.steps
    }

    /// Host-only structural loop diagnostics. No tool arguments are exposed.
    pub fn loop_attempt_count(&self) -> u64 {
        self.loop_guard.attempted()
    }

    /// Host-only: forbid any further model response/dispatch after a task
    /// is cancelled, revoked or its execution result becomes uncertain.
    /// This does not undo a tool effect that already completed.
    pub fn halt(&mut self) {
        self.done = true;
    }

    /// A previous valid grant never authorizes a later model continuation.
    /// Check the host task's *current* authority after every async tool call,
    /// not only before model dispatch. Other unadvertised grants cannot be
    /// acquired from model output.
    pub fn ensure_continuation_authorized(
        &self,
        authority: &ConversationAuthority,
    ) -> Result<(), ToolTurnError> {
        if self.done {
            return Err(ToolTurnError::AlreadyFinished);
        }
        let permitted = authority
            .permitted_tool_ids(&self.origin)
            .map_err(ToolTurnError::Unauthorized)?;
        if permitted.is_empty() {
            return Err(ToolTurnError::Revoked);
        }
        Ok(())
    }

    /// Authenticated host scope, for private durable audit only.
    pub fn audit_scope(&self) -> (&str, &str) {
        (&self.origin.conversation_id, &self.origin.task_id)
    }

    /// Diagnostics only. Repair status is never an authorization grant.
    pub fn last_json_repair(&self) -> JsonRepairStatus {
        self.last_repair
    }

    /// Expected model envelopes:
    /// {"kind":"tool","name":"lu_files_read_text","arguments":{"path":"x.txt"}}
    /// {"kind":"final","text":"..."}
    /// Reject unknown fields, arrays, duplicate names, and unadvertised tools.
    /// Call IDs are generated by Lu from the authenticated task, never the LLM.
    pub async fn handle_model_reply(
        &mut self,
        raw: &str,
        catalog: &ModelToolCatalog,
        registry: &NativeToolRegistry,
        authority: &ConversationAuthority,
        host_policy: &dyn ToolAuthorizer,
    ) -> Result<ToolTurnOutcome, ToolTurnError> {
        self.handle_model_reply_observed(raw, catalog, registry, authority, host_policy, &|_| {})
            .await
    }

    /// Observes the real bound-dispatch invocation and returned response.
    /// Returned does not mean a tool was allowed or that an effect succeeded.
    pub async fn handle_model_reply_observed(
        &mut self,
        raw: &str,
        catalog: &ModelToolCatalog,
        registry: &NativeToolRegistry,
        authority: &ConversationAuthority,
        host_policy: &dyn ToolAuthorizer,
        observe: &(dyn Fn(ToolDispatchCheckpoint) + Send + Sync),
    ) -> Result<ToolTurnOutcome, ToolTurnError> {
        self.handle_model_reply_with_effect_journal(
            raw,
            catalog,
            registry,
            authority,
            host_policy,
            observe,
            None,
        )
        .await
    }

    /// Trusted host-only durable path. A reviewed host route MUST use this,
    /// never the legacy in-memory dispatcher: prepared/executing/applied state
    /// prevents replay after a crash or ambiguous subprocess failure.
    #[allow(clippy::too_many_arguments)]
    pub async fn handle_model_reply_observed_durable(
        &mut self,
        raw: &str,
        catalog: &ModelToolCatalog,
        registry: &NativeToolRegistry,
        authority: &ConversationAuthority,
        host_policy: &dyn ToolAuthorizer,
        observe: &(dyn Fn(ToolDispatchCheckpoint) + Send + Sync),
        effects: &EffectJournal,
    ) -> Result<ToolTurnOutcome, ToolTurnError> {
        self.handle_model_reply_with_effect_journal(
            raw,
            catalog,
            registry,
            authority,
            host_policy,
            observe,
            Some(effects),
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn handle_model_reply_with_effect_journal(
        &mut self,
        raw: &str,
        catalog: &ModelToolCatalog,
        registry: &NativeToolRegistry,
        authority: &ConversationAuthority,
        host_policy: &dyn ToolAuthorizer,
        observe: &(dyn Fn(ToolDispatchCheckpoint) + Send + Sync),
        effects: Option<&EffectJournal>,
    ) -> Result<ToolTurnOutcome, ToolTurnError> {
        if self.done {
            return Err(ToolTurnError::AlreadyFinished);
        }
        if raw.len() > MAX_MODEL_FRAME_BYTES {
            return Err(ToolTurnError::TooLarge);
        }
        // Syntax repair proposes a candidate only. Protobuf codec validation,
        // conversation authority and host policy still decide execution.
        let parsed = match parse_model_boundary(raw) {
            Ok(parsed) => parsed,
            Err(error) => {
                // A small model may answer ordinary chat in prose despite the
                // JSON-final instruction. Plain prose is display data only:
                // it can never name a tool, provide arguments or request an
                // effect. JSON-looking, fenced and role-control output still
                // fails closed at the model/tool boundary.
                let text = raw.trim();
                let plain_final = !text.is_empty()
                    && !matches!(text.as_bytes()[0], b'{' | b'[' | b'"' | b'`')
                    && !text.contains("<|")
                    && !text.contains("\"kind\"")
                    && !text
                        .chars()
                        .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'));
                // A short ASCII integer is a JSON scalar, not an object. It
                // may be shown as chat text but can never request a tool.
                let integer_final = error == JsonBoundaryError::NonObject
                    && text.len() <= 64
                    && text.bytes().all(|byte| byte.is_ascii_digit());
                if plain_final && (error == JsonBoundaryError::Malformed || integer_final) {
                    let current = authority
                        .permitted_tool_ids(&self.origin)
                        .map_err(ToolTurnError::Unauthorized)?;
                    if current.is_empty() {
                        return Err(ToolTurnError::Revoked);
                    }
                    self.done = true;
                    return Ok(ToolTurnOutcome::FinalText(text.to_owned()));
                }
                return Err(match error {
                    JsonBoundaryError::TooLarge => ToolTurnError::TooLarge,
                    _ => ToolTurnError::MalformedModelReply,
                });
            }
        };
        self.last_repair = parsed.repair;
        let object = &parsed.object;
        // Small models sometimes omit the "kind" discriminator on an
        // otherwise minimal final reply. Only an EXACT single string field
        // may become display data. This branch has no path to tool dispatch,
        // and cannot accept an object carrying extra control/argument fields.
        if !object.contains_key("kind") && object.len() == 1 {
            let display_text = ["text", "answer", "response"]
                .into_iter()
                .find_map(|name| object.get(name).and_then(Value::as_str))
                .filter(|text| !text.trim().is_empty());
            if let Some(text) = display_text {
                let current = authority
                    .permitted_tool_ids(&self.origin)
                    .map_err(ToolTurnError::Unauthorized)?;
                if current.is_empty() {
                    return Err(ToolTurnError::Revoked);
                }
                self.done = true;
                return Ok(ToolTurnOutcome::FinalText(text.to_owned()));
            }
        }
        let kind = object
            .get("kind")
            .and_then(Value::as_str)
            .ok_or(ToolTurnError::MalformedModelReply)?;

        if kind == "final" {
            if object.len() != 2 || !object.contains_key("text") {
                return Err(ToolTurnError::UnexpectedFields);
            }
            let text = object
                .get("text")
                .and_then(Value::as_str)
                .filter(|v| !v.trim().is_empty())
                .ok_or(ToolTurnError::MalformedModelReply)?;
            // Revocation also blocks finalization. The outer host must validate
            // conversation ownership before persisting the final answer.
            let current = authority
                .permitted_tool_ids(&self.origin)
                .map_err(ToolTurnError::Unauthorized)?;
            if current.is_empty() {
                return Err(ToolTurnError::Revoked);
            }
            self.done = true;
            return Ok(ToolTurnOutcome::FinalText(text.to_owned()));
        }

        if kind != "tool" {
            return Err(ToolTurnError::MalformedModelReply);
        }
        if object.len() != 3 || !object.contains_key("name") || !object.contains_key("arguments") {
            return Err(ToolTurnError::UnexpectedFields);
        }
        let name = object
            .get("name")
            .and_then(Value::as_str)
            .ok_or(ToolTurnError::MalformedModelReply)?;
        if !self.advertised.contains(name) {
            return Err(ToolTurnError::Catalog(ModelToolError::UnknownTool(
                name.into(),
            )));
        }
        let args = object
            .get("arguments")
            .and_then(Value::as_object)
            .ok_or(ToolTurnError::MalformedModelReply)?;
        let encoded =
            serde_json::to_string(args).map_err(|_| ToolTurnError::MalformedModelReply)?;
        let digest = Sha256::digest(self.origin.task_id.as_bytes());
        let prefix = digest
            .iter()
            .take(12)
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        // There is NO fixed count of tool calls. Only numeric overflow,
        // progress evidence, task deadlines and normal permissions bound the turn.
        let next_step = self.steps.checked_add(1)
            .ok_or(ToolTurnError::CounterOverflow)?;
        let call_id = format!("lu-{prefix}-{next_step}");
        let invocation = catalog
            .encode_call(
                &self.advertised,
                name,
                &call_id,
                Some(&self.workspace_id),
                &encoded,
            )
            .map_err(ToolTurnError::Catalog)?;

        let descriptors = registry.tools()
            .map_err(|_| ToolTurnError::ReadOnlyTurnViolation)?;
        let effect = descriptors.iter()
            .find(|descriptor| descriptor.id == invocation.tool_id)
            .map(|descriptor| descriptor.effect)
            .ok_or(ToolTurnError::ReadOnlyTurnViolation)?;
        if self.read_only && effect != ToolEffect::ReadWorkspace {
            return Err(ToolTurnError::ReadOnlyTurnViolation);
        }

        // Progress-aware guard runs after valid JSON/Protobuf input, but
        // before native authorization/effects. No raw-count cap, and same-file
        // edits with different operations are not assumed to be a loop.
        // An unproductive blocked attempt still contributes to the
        // host-owned no-progress escalation, never to an OS effect.
        let guard_verdict = self.loop_guard.check(name, args);
        match guard_verdict {
            LoopVerdict::Kill(_) => {
                self.halt();
                return Err(ToolTurnError::LoopTerminated);
            }
            LoopVerdict::Block(reason) => {
                self.steps = next_step;
                return Ok(ToolTurnOutcome::ToolResult {
                    call_id,
                    tool_name: name.into(),
                    data: serde_json::json!({
                        "ok":false,
                        "tool":name,
                        "error":{
                            "code":"LOOP_DETECTED",
                            "message":reason.message(),
                            "retryable":false
                        }
                    }),
                });
            }
            LoopVerdict::Allow | LoopVerdict::Warn(_) => {}
        }
        let warning: Option<LoopReason> = match guard_verdict {
            LoopVerdict::Warn(reason) => Some(reason),
            _ => None,
        };

        // A tool can be advertised earlier, then revoked. Host checks exact
        // origin, tool effect and workspace scope again at dispatch time.
        self.steps = next_step;
        observe(ToolDispatchCheckpoint::Entered);
        let response = if let Some(effects) = effects {
            authority
                .dispatch_bound_durable(
                    registry,
                    effects,
                    invocation,
                    self.origin.clone(),
                    host_policy,
                )
                .await
        } else {
            authority
                .dispatch_bound(registry, invocation, self.origin.clone(), host_policy)
                .await
        };
        observe(ToolDispatchCheckpoint::Returned);
        if effects.is_some() && response.is_err() {
            // A durable effect may be UNCERTAIN after partial execution or
            // receipt failure. The model must not treat an error payload as
            // permission to perform another call or to retry this one.
            self.halt();
            return Err(ToolTurnError::DurableDispatchFailed);
        }
        let mut data = catalog
            .decode_result(&self.advertised, name, &response)
            .map_err(ToolTurnError::Catalog)?;
        // A decoded native result is the only evidence eligible to advance
        // the loop guard; new arguments, call IDs and messages never are.
        // For writes, progression requires a strong typed change receipt.
        self.loop_guard.observe_feedback(name, args, &data, effect);
        if let Some(reason) = warning {
            // Fixed host language only; never echo user content/arguments.
            // Keep outer tool feedback schema unchanged (ok/tool/result).
            if let Some(result) = data.get_mut("result").and_then(Value::as_object_mut) {
                result.insert("_lu_loop_warning".into(), Value::String(reason.message().into()));
            }
        }
        Ok(ToolTurnOutcome::ToolResult {
            call_id,
            tool_name: name.into(),
            data,
        })
    }

    pub fn model_feedback(result: &ToolTurnOutcome) -> Option<String> {
        match result {
            ToolTurnOutcome::ToolResult {
                call_id,
                tool_name,
                data,
            } => serialize_host_tool_feedback_boundary(
                call_id, tool_name, data, MAX_MODEL_FRAME_BYTES,
            ).ok(),
            ToolTurnOutcome::FinalText(_) => None,
        }
    }
}
