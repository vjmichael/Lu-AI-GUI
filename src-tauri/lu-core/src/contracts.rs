use std::{future::Future, pin::Pin};

/// Increment on incompatible changes to these module contracts.
pub const CORE_CONTRACT_VERSION: u16 = 1;

/// Core is always present. Optional native Core Extensions supply reusable
/// capabilities; Workspaces provide separate specialized application surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleKind {
    Core,
    CoreExtension,
    Workspace,
}

/// Declaration used for discovery, not an authorization grant.
/// The host's authorizer must separately inspect the invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolEffect {
    ReadWorkspace,
    WriteWorkspace,
    ExecuteCode,
    NetworkRead,
    NetworkWrite,
    DeviceControl,
    Elevated,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolDescriptor {
    /// Stable, qualified name, e.g. "lu.files.read" or "acme.nvp.render".
    pub id: String,
    pub display_name: String,
    pub effect: ToolEffect,
    /// Protobuf type URL of the input message, not a model-facing JSON schema.
    pub input_type_url: String,
    /// Protobuf type URL of the output message.
    pub output_type_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelContribution {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowContribution {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleManifest {
    pub id: String,
    pub display_name: String,
    pub contract_version: u16,
    pub kind: ModuleKind,
    pub tools: Vec<ToolDescriptor>,
    /// Metadata for the future UI workspace loader; no UI is loaded here.
    pub panels: Vec<PanelContribution>,
    pub workflows: Vec<WorkflowContribution>,
}

/// Internal typed-message envelope. No JSON or MCP transport is implied.
///
/// This layer checks type URLs and routes ownership. It does NOT parse bytes or
/// attest a Protobuf message is valid. Generated-schema validation belongs at
/// the future IPC/model adapters and inside the registered tool implementation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtoPayload {
    pub type_url: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolInvocation {
    pub call_id: String,
    pub tool_id: String,
    pub workspace_id: Option<String>,
    pub input: ProtoPayload,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolOutput {
    pub payload: ProtoPayload,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolFailure {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

impl ToolFailure {
    pub fn new(code: impl Into<String>, message: impl Into<String>, retryable: bool) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            retryable,
        }
    }
}

/// Async tool implementations can be Rust functions or Lu-managed local
/// workers. Neither requires an MCP server or a webview extension.
pub type ToolFuture<'a> =
    Pin<Box<dyn Future<Output = Result<ToolOutput, ToolFailure>> + Send + 'a>>;

pub trait NativeTool: Send + Sync {
    fn descriptor(&self) -> &ToolDescriptor;
    fn invoke(&self, invocation: ToolInvocation) -> ToolFuture<'_>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizationError {
    pub reason: String,
}

impl AuthorizationError {
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

/// Deny-by-default at the integration boundary: every dispatch MUST supply
/// an authorizer. A module's declared ToolEffect is not permission to act.
/// The future host PolicyEngine must authorize the exact operation and scope.
pub trait ToolAuthorizer: Send + Sync {
    fn authorize(
        &self,
        invocation: &ToolInvocation,
        descriptor: &ToolDescriptor,
    ) -> Result<(), AuthorizationError>;
}
