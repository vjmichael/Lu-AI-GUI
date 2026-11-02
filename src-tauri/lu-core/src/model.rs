//! Model-facing tool catalog: external JSON -> validated internal Protobuf.
//!
//! The model sees only explicitly selected, bounded JSON schemas. The catalog
//! is inert: it cannot execute a tool, authorize an effect, or start MCP.
//! Dedicated codecs must validate *every* argument, including unknown fields,
//! before producing an internal generated Protobuf message.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use serde_json::{Map, Value};

use crate::{
    contracts::{ProtoPayload, ToolEffect, ToolInvocation},
    registry::{DispatchError, NativeToolRegistry, ToolResponse},
};

pub const MAX_MODEL_TOOL_ARGS_BYTES: usize = 64 * 1024;
pub const MAX_ADVERTISED_TOOLS: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelToolError {
    InvalidToolName,
    DuplicateToolName(String),
    UnknownTool(String),
    NoNativeImplementation(String),
    TypeMismatch(String),
    InvalidSchema(String),
    InvalidResult(String),
    InvalidArguments(String),
    OversizedArguments,
    ToolBudgetExceeded,
    RegistryUnavailable,
}

pub trait ModelToolCodec: Send + Sync {
    /// Exact type URL that must match the registered native tool descriptor.
    fn input_type_url(&self) -> &'static str;
    fn output_type_url(&self) -> &'static str;
    fn encode(&self, args: &Map<String, Value>) -> Result<ProtoPayload, ModelToolError>;
    fn decode(&self, output: &ProtoPayload) -> Result<Value, ModelToolError>;
}

pub struct ModelToolDefinition {
    /// OpenAI-compatible short tool name; must be unique among advertised tools.
    pub name: String,
    pub native_tool_id: String,
    pub description: String,
    /// External, model-visible JSON Schema. Schemas are not an authority grant.
    pub parameters: Value,
    pub codec: Arc<dyn ModelToolCodec>,
}

pub struct ModelToolCatalog {
    definitions: BTreeMap<String, ModelToolDefinition>,
}

impl ModelToolCatalog {
    pub fn new() -> Self {
        Self {
            definitions: BTreeMap::new(),
        }
    }

    /// Register a model boundary ONLY for a tool that already exists in the
    /// native registry. Keeping this separate prevents schema/name drift.
    pub fn insert(
        &mut self,
        registry: &NativeToolRegistry,
        definition: ModelToolDefinition,
    ) -> Result<(), ModelToolError> {
        let valid_name = !definition.name.is_empty()
            && definition.name.len() <= 64
            && definition.name.bytes().enumerate().all(|(index, c)| {
                c.is_ascii_alphabetic() || (index > 0 && (c.is_ascii_digit() || c == b'_'))
            });
        if !valid_name {
            return Err(ModelToolError::InvalidToolName);
        }
        if self.definitions.contains_key(&definition.name) {
            return Err(ModelToolError::DuplicateToolName(definition.name));
        }
        if definition.description.trim().is_empty() {
            return Err(ModelToolError::InvalidSchema(definition.name));
        }
        // An explicit object schema with no additional properties is mandatory
        // for first-party codecs. Model servers may ignore it; codecs must not.
        if definition.parameters.get("type").and_then(Value::as_str) != Some("object")
            || definition.parameters.get("additionalProperties") != Some(&Value::Bool(false))
        {
            return Err(ModelToolError::InvalidSchema(definition.name));
        }
        let descriptor = registry
            .tools()
            .map_err(|_| ModelToolError::RegistryUnavailable)?
            .into_iter()
            .find(|desc| desc.id == definition.native_tool_id)
            .ok_or_else(|| {
                ModelToolError::NoNativeImplementation(definition.native_tool_id.clone())
            })?;
        if descriptor.input_type_url != definition.codec.input_type_url()
            || descriptor.output_type_url != definition.codec.output_type_url()
        {
            return Err(ModelToolError::TypeMismatch(definition.native_tool_id));
        }
        self.definitions.insert(definition.name.clone(), definition);
        Ok(())
    }

    /// A host-owned route descriptor is inert until its full native
    /// implementation, effect and advertised per-turn schema all agree.
    /// Never accept a manifest/model-supplied descriptor as authorization.
    #[allow(clippy::too_many_arguments)]
    pub fn matches_reviewed_route(
        &self,
        registry: &NativeToolRegistry,
        admitted_schemas: &[Value],
        public_name: &str,
        native_tool_id: &str,
        effect: ToolEffect,
        input_type_url: &str,
        argument_name: &str,
    ) -> bool {
        let Some(definition) = self.definitions.get(public_name) else {
            return false;
        };
        if definition.native_tool_id != native_tool_id
            || definition.codec.input_type_url() != input_type_url
            || definition.parameters.get("type").and_then(Value::as_str) != Some("object")
            || definition.parameters.get("additionalProperties") != Some(&Value::Bool(false))
            || definition
                .parameters
                .pointer(&format!("/properties/{argument_name}/type"))
                .and_then(Value::as_str)
                != Some("string")
            || definition.parameters.get("required") != Some(&serde_json::json!([argument_name]))
        {
            return false;
        }
        let expected = serde_json::json!({
            "type": "function",
            "function": {
                "name": definition.name,
                "description": definition.description,
                "parameters": definition.parameters,
            }
        });
        if !admitted_schemas.contains(&expected) {
            return false;
        }
        registry.tools().ok().is_some_and(|tools| {
            tools.iter().any(|descriptor| {
                descriptor.id == native_tool_id
                    && descriptor.effect == effect
                    && descriptor.input_type_url == input_type_url
                    && descriptor.output_type_url == definition.codec.output_type_url()
            })
        })
    }

    /// Project a host-authorized set of native IDs onto this registry's
    /// public model names. Never authorize from names supplied by the model.
    /// The resulting names are still checked independently at dispatch.
    pub fn names_for_native_ids(&self, native_ids: &BTreeSet<String>) -> BTreeSet<String> {
        self.definitions
            .iter()
            .filter(|(_, definition)| native_ids.contains(&definition.native_tool_id))
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Only publish an explicit allow-list, e.g. the current chat context,
    /// installed workspace, and model capability. No accidental "all tools".
    pub fn schemas(&self, allowed: &BTreeSet<String>) -> Result<Vec<Value>, ModelToolError> {
        if allowed.len() > MAX_ADVERTISED_TOOLS {
            return Err(ModelToolError::ToolBudgetExceeded);
        }
        let mut schemas = Vec::new();
        for name in allowed {
            let definition = self
                .definitions
                .get(name)
                .ok_or_else(|| ModelToolError::UnknownTool(name.clone()))?;
            schemas.push(serde_json::json!({
                "type": "function",
                "function": {
                    "name": definition.name,
                    "description": definition.description,
                    "parameters": definition.parameters
                }
            }));
        }
        Ok(schemas)
    }

    /// Translate a typed native outcome into an explicit model-facing
    /// success/error result. Caller must ensure the tool was advertised to
    /// THIS conversation; receiving a result does not expand permission.
    pub fn decode_result(
        &self,
        allowed: &BTreeSet<String>,
        public_name: &str,
        outcome: &ToolResponse,
    ) -> Result<Value, ModelToolError> {
        if !allowed.contains(public_name) {
            return Err(ModelToolError::UnknownTool(public_name.to_string()));
        }
        let definition = self
            .definitions
            .get(public_name)
            .ok_or_else(|| ModelToolError::UnknownTool(public_name.to_string()))?;
        match outcome {
            Ok(output) => {
                if output.payload.type_url != definition.codec.output_type_url() {
                    return Err(ModelToolError::InvalidResult(
                        "unexpected native tool output type".into(),
                    ));
                }
                let value = definition.codec.decode(&output.payload)?;
                Ok(serde_json::json!({
                    "ok": true, "tool": public_name, "result": value
                }))
            }
            Err(error) => {
                let (code, message, retryable) = match error {
                    DispatchError::UnknownTool(_) => {
                        ("UNKNOWN_TOOL", "native tool is unavailable".into(), false)
                    }
                    DispatchError::ModuleDraining(_) => (
                        "EXTENSION_DRAINING",
                        "native extension is shutting down; retry after it is re-enabled".into(),
                        true,
                    ),
                    DispatchError::InvalidCallId => {
                        ("INVALID_CALL_ID", "invalid tool call ID".into(), false)
                    }
                    DispatchError::WorkspaceRequired(_) => (
                        "WORKSPACE_REQUIRED",
                        "active workspace is required".into(),
                        false,
                    ),
                    DispatchError::InputTypeMismatch { .. } => (
                        "INPUT_TYPE_MISMATCH",
                        "invalid tool argument type".into(),
                        false,
                    ),
                    DispatchError::Denied(_) => (
                        "PERMISSION_DENIED",
                        "tool action was not authorized".into(),
                        false,
                    ),
                    DispatchError::ProtectedEffect(_) => (
                        "PRIVILEGE_BLOCKED",
                        "this capability cannot be delegated to AI agents".into(),
                        false,
                    ),
                    DispatchError::Execution(failure) => (
                        failure.code.as_str(),
                        failure.message.clone(),
                        failure.retryable,
                    ),
                    DispatchError::Poisoned => (
                        "REGISTRY_UNAVAILABLE",
                        "native tool registry unavailable".into(),
                        true,
                    ),
                };
                Ok(serde_json::json!({
                    "ok": false,
                    "tool": public_name,
                    "error": {
                        "code": code,
                        "message": message,
                        "retryable": retryable
                    }
                }))
            }
        }
    }

    /// Parse untrusted model tool-call JSON without repair/coercion; the caller
    /// must check the public tool name is in this turn's advertised allow-list.
    /// This returns a typed invocation, NOT an authorization decision.
    pub fn encode_call(
        &self,
        allowed: &BTreeSet<String>,
        public_name: &str,
        call_id: &str,
        workspace_id: Option<&str>,
        raw_json: &str,
    ) -> Result<ToolInvocation, ModelToolError> {
        if !allowed.contains(public_name) {
            return Err(ModelToolError::UnknownTool(public_name.to_string()));
        }
        if raw_json.len() > MAX_MODEL_TOOL_ARGS_BYTES {
            return Err(ModelToolError::OversizedArguments);
        }
        if call_id.trim().is_empty() || call_id.len() > 128 {
            return Err(ModelToolError::InvalidArguments("invalid call ID".into()));
        }
        let definition = self
            .definitions
            .get(public_name)
            .ok_or_else(|| ModelToolError::UnknownTool(public_name.to_string()))?;
        let value: Value = serde_json::from_str(raw_json)
            .map_err(|e| ModelToolError::InvalidArguments(format!("malformed JSON: {e}")))?;
        let arguments = value
            .as_object()
            .ok_or_else(|| ModelToolError::InvalidArguments("expected JSON object".into()))?;
        let input = definition.codec.encode(arguments)?;
        if input.type_url != definition.codec.input_type_url() {
            return Err(ModelToolError::TypeMismatch(
                definition.native_tool_id.clone(),
            ));
        }
        Ok(ToolInvocation {
            call_id: call_id.into(),
            tool_id: definition.native_tool_id.clone(),
            workspace_id: workspace_id.map(str::to_string),
            input,
        })
    }
}

impl Default for ModelToolCatalog {
    fn default() -> Self {
        Self::new()
    }
}

/// Shared strict argument helpers for bundled first-party model codecs.
pub fn expect_fields(
    args: &Map<String, Value>,
    allowed: &[&str],
    required: &[&str],
) -> Result<(), ModelToolError> {
    for key in args.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(ModelToolError::InvalidArguments(format!(
                "unexpected argument: {key}"
            )));
        }
    }
    for field in required {
        if !args.contains_key(*field) {
            return Err(ModelToolError::InvalidArguments(format!(
                "missing required argument: {field}"
            )));
        }
    }
    Ok(())
}

pub fn optional_uint(
    args: &Map<String, Value>,
    name: &str,
    default: u32,
    maximum: u32,
) -> Result<u32, ModelToolError> {
    match args.get(name) {
        None => Ok(default),
        Some(value) => {
            let n = value.as_u64().ok_or_else(|| {
                ModelToolError::InvalidArguments(format!("{name} must be a positive integer"))
            })?;
            if n == 0 || n > u64::from(maximum) {
                return Err(ModelToolError::InvalidArguments(format!(
                    "{name} must be between 1 and {maximum}"
                )));
            }
            Ok(n as u32)
        }
    }
}
