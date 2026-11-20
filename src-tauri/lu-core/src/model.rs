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
    model_json_boundary::parse_exact_boundary_object,
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

/// A narrowly scoped model-facing reference to arguments already bound by
/// the trusted host. The raw model call still selects the actual tool; the
/// original codec validates and encodes the host-owned immutable input.
struct BoundCurrentInputCodec {
    original: Arc<dyn ModelToolCodec>,
    fixed: Map<String, Value>,
}
impl ModelToolCodec for BoundCurrentInputCodec {
    fn input_type_url(&self) -> &'static str { self.original.input_type_url() }
    fn output_type_url(&self) -> &'static str { self.original.output_type_url() }
    fn encode(&self, args: &Map<String, Value>) -> Result<ProtoPayload, ModelToolError> {
        expect_fields(args, &["source"], &["source"])?;
        if args.get("source").and_then(Value::as_str) != Some("current_request") {
            return Err(ModelToolError::InvalidArguments("expected current approved request reference".into()));
        }
        self.original.encode(&self.fixed)
    }
    fn decode(&self, output: &ProtoPayload) -> Result<Value, ModelToolError> { self.original.decode(output) }
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
        Self::validate_definition(&definition)?;
        if self.definitions.contains_key(&definition.name) {
            return Err(ModelToolError::DuplicateToolName(definition.name));
        }
        let descriptor = registry
            .tools()
            .map_err(|_| ModelToolError::RegistryUnavailable)?
            .into_iter()
            .find(|desc| desc.id == definition.native_tool_id)
            .ok_or_else(|| {
                ModelToolError::NoNativeImplementation(definition.native_tool_id.clone())
            })?;
        Self::validate_contract(&descriptor,&definition)?;
        self.definitions.insert(definition.name.clone(), definition);
        Ok(())
    }

    fn validate_definition(definition:&ModelToolDefinition)->Result<(),ModelToolError>{
        let valid_name = !definition.name.is_empty()
            && definition.name.len() <= 64
            && definition.name.bytes().enumerate().all(|(index, c)| {
                c.is_ascii_alphabetic() || (index > 0 && (c.is_ascii_digit() || c == b'_'))
            });
        if !valid_name {
            return Err(ModelToolError::InvalidToolName);
        }
        if definition.description.trim().is_empty() {
            return Err(ModelToolError::InvalidSchema(definition.name.clone()));
        }
        // An explicit object schema with no additional properties is mandatory
        // for first-party codecs. Model servers may ignore it; codecs must not.
        if definition.parameters.get("type").and_then(Value::as_str) != Some("object")
            || definition.parameters.get("additionalProperties") != Some(&Value::Bool(false))
        {
            return Err(ModelToolError::InvalidSchema(definition.name.clone()));
        }
        Ok(())
    }

    /// Shared validation for inert registration plans and live registration.
    /// This has no registry mutation or dispatch capability.
    pub(crate) fn validate_contract(descriptor:&crate::contracts::ToolDescriptor,
        definition:&ModelToolDefinition)->Result<(),ModelToolError>{
        Self::validate_definition(definition)?;
        if descriptor.id!=definition.native_tool_id ||
            descriptor.input_type_url!=definition.codec.input_type_url() ||
            descriptor.output_type_url!=definition.codec.output_type_url(){
            return Err(ModelToolError::TypeMismatch(definition.native_tool_id.clone()));
        }
        Ok(())
    }

    /// Only the trusted host can project an exact-input utility's arguments
    /// into a reference. This is a turn-local catalog, not a reusable grant.
    /// Validate the complete original input and keep the same typed codec.
    pub(crate) fn bind_current_input(&mut self, registry: &NativeToolRegistry,
        name: &str, fixed: Map<String, Value>) -> Result<(), ModelToolError> {
        self.bind_current_input_for_effect(registry,name,fixed,ToolEffect::ReadWorkspace)
    }

    /// Only the two finite public HTTPS operations may bind a network input.
    /// This is a schema projection, not a grant; the exact-call fence is mandatory.
    pub(crate) fn bind_current_network_input(&mut self, registry: &NativeToolRegistry,
        name: &str, fixed: Map<String, Value>) -> Result<(), ModelToolError> {
        if !matches!(name,"lu_web_fetch"|"lu_web_academic_search") {
            return Err(ModelToolError::InvalidArguments("unknown finite network read".into()));
        }
        self.bind_current_input_for_effect(registry,name,fixed,ToolEffect::NetworkRead)
    }
    fn bind_current_input_for_effect(&mut self, registry: &NativeToolRegistry,
        name: &str, fixed: Map<String, Value>, effect: ToolEffect) -> Result<(), ModelToolError> {
        let previous = self.definitions.get(name).ok_or_else(|| ModelToolError::UnknownTool(name.into()))?;
        let descriptor = registry.tools().map_err(|_| ModelToolError::RegistryUnavailable)?
            .into_iter().find(|d| d.id == previous.native_tool_id)
            .ok_or_else(|| ModelToolError::NoNativeImplementation(previous.native_tool_id.clone()))?;
        if descriptor.effect != effect { return Err(ModelToolError::InvalidArguments("fixed input must match its read effect".into())); }
        let payload = previous.codec.encode(&fixed)?;
        if payload.type_url != descriptor.input_type_url { return Err(ModelToolError::TypeMismatch(descriptor.id)); }
        let definition = ModelToolDefinition {
            name: previous.name.clone(), native_tool_id: previous.native_tool_id.clone(),
            description: format!("{} The exact input is already bound to the current approved human request. Call with source=current_request; do not repeat or modify its data.", previous.description),
            parameters: serde_json::json!({"type":"object","properties":{"source":{"type":"string","const":"current_request"}},"required":["source"],"additionalProperties":false}),
            codec: Arc::new(BoundCurrentInputCodec {original: previous.codec.clone(), fixed}),
        };
        Self::validate_contract(&descriptor, &definition)?;
        self.definitions.insert(name.into(), definition);
        Ok(())
    }

    /// Compose independently registered Lu native domains into one host-owned
    /// model catalog. Composition is atomic, does not widen the turn's allowed
    /// names, and does not create an authorization grant. Callers must still
    /// use the trusted per-turn allowlist and ConversationAuthority.
    ///
    /// All schemas were checked on initial registration, but their native
    /// implementations may have been unloaded since then. Revalidate against
    /// the *current* registry before accepting even one merged name.
    pub fn absorb_registered(
        &mut self,
        registry: &NativeToolRegistry,
        other: ModelToolCatalog,
    ) -> Result<(), ModelToolError> {
        let installed = registry.tools()
            .map_err(|_| ModelToolError::RegistryUnavailable)?;
        for (name, definition) in &other.definitions {
            if self.definitions.contains_key(name) {
                return Err(ModelToolError::DuplicateToolName(name.clone()));
            }
            let descriptor = installed.iter()
                .find(|item| item.id == definition.native_tool_id)
                .ok_or_else(|| ModelToolError::NoNativeImplementation(
                    definition.native_tool_id.clone()
                ))?;
            if descriptor.input_type_url != definition.codec.input_type_url()
                || descriptor.output_type_url != definition.codec.output_type_url() {
                return Err(ModelToolError::TypeMismatch(
                    definition.native_tool_id.clone()
                ));
            }
        }
        self.definitions.extend(other.definitions);
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

    /// Discover tools for this exact turn only after rechecking the live
    /// native registry and its Protobuf type identities. Model-facing schemas
    /// remain a projection of host grants; they cannot authorize execution.
    pub fn live_schemas(
        &self,
        allowed: &BTreeSet<String>,
        registry: &NativeToolRegistry,
    ) -> Result<Vec<Value>, ModelToolError> {
        let schemas = self.schemas(allowed)?;
        let installed = registry.tools()
            .map_err(|_| ModelToolError::RegistryUnavailable)?;
        for name in allowed {
            let definition = self.definitions.get(name)
                .ok_or_else(|| ModelToolError::UnknownTool(name.clone()))?;
            let descriptor = installed.iter()
                .find(|d| d.id == definition.native_tool_id)
                .ok_or_else(|| ModelToolError::NoNativeImplementation(
                    definition.native_tool_id.clone()))?;
            if descriptor.input_type_url != definition.codec.input_type_url()
                || descriptor.output_type_url != definition.codec.output_type_url()
            {
                return Err(ModelToolError::TypeMismatch(definition.native_tool_id.clone()));
            }
        }
        Ok(schemas)
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
        // This direct codec entry point is a real model/native boundary too,
        // not only the outer NativeToolTurn. serde_json::Value would silently
        // accept duplicate object keys (including nested keys). Refuse them
        // before a codec can choose a path, operation or Protobuf argument.
        // No syntax repair is permitted at this second-stage boundary.
        let arguments = parse_exact_boundary_object(raw_json, MAX_MODEL_TOOL_ARGS_BYTES)
            .map_err(|e| ModelToolError::InvalidArguments(format!("JSON boundary rejected: {e:?}")))?;
        let input = definition.codec.encode(&arguments)?;
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
