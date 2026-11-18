//! WRITE-01B: first-party, create-new-only native write with exact-target
//! policy and existing durable Protobuf dispatcher. NOT desktop-enabled.
//! Host selects an already validated private directory and issues a trusted
//! USER-turn authority; neither model text nor tool discovery can grant it.
use std::{path::Path, sync::Arc};
use cap_std::{ambient_authority, fs::Dir};
use prost::Message;
use sha2::{Digest, Sha256};
use crate::{
    contracts::{AuthorizationError, ModuleKind, ModuleManifest, NativeTool, ProtoPayload,
        ToolAuthorizer, ToolDescriptor, ToolEffect, ToolFailure, ToolFuture, ToolInvocation,
        ToolOutput, CORE_CONTRACT_VERSION},
    native_atomic_write::{create_new_atomic_in_dir, AtomicWriteError, MAX_ATOMIC_WRITE_BYTES},
    registry::{NativeToolRegistry, RegistryError},
    wire::proto,
};

pub const TOOL_ID: &str = "lu.create_new_file.commit";
pub const REQUEST_TYPE: &str = "type.googleapis.com/lu.v1.CeWriteRequest";
pub const RESULT_TYPE: &str = "type.googleapis.com/lu.v1.CeWriteResult";

/// Not a reusable grant; a host-owned per-call argument fence used in
/// addition to ConversationAuthority and PolicyEngine.
pub struct ExactCreateNewPolicy<'a> {
    pub host_policy: &'a dyn ToolAuthorizer,
    pub expected_target: &'a str,
    pub expected_sha256: &'a str,
}

fn simple_target(target: &str) -> bool {
    !target.is_empty() && target.len() <= 255 && target != "." && target != ".."
        && !target.starts_with(".lu-stage-")
        && !target.chars().any(|c| matches!(c, '/' | '\\' | ':' | '\0'))
}

impl ToolAuthorizer for ExactCreateNewPolicy<'_> {
    fn authorize(
        &self, invocation: &ToolInvocation, descriptor: &ToolDescriptor
    ) -> Result<(), AuthorizationError> {
        if descriptor.id != TOOL_ID || descriptor.effect != ToolEffect::WriteWorkspace
            || invocation.tool_id != TOOL_ID || invocation.input.type_url != REQUEST_TYPE
            || !simple_target(self.expected_target)
            || invocation.input.bytes.len() > MAX_ATOMIC_WRITE_BYTES + 4096 {
            return Err(AuthorizationError::new("create-new contract not authorized"));
        }
        let request = proto::CeWriteRequest::decode(invocation.input.bytes.as_slice())
            .map_err(|_| AuthorizationError::new("invalid create-new protobuf request"))?;
        if request.relative_path != self.expected_target
            || request.data.len() > MAX_ATOMIC_WRITE_BYTES
            || format!("{:x}", Sha256::digest(&request.data)) != self.expected_sha256 {
            return Err(AuthorizationError::new("create-new target or bytes differ from approval"));
        }
        let content = std::str::from_utf8(&request.data)
            .map_err(|_| AuthorizationError::new("requested write was not UTF-8"))?;
        crate::write_format::identify_text_format(&request.relative_path, content)
            .map_err(|_| AuthorizationError::new("unsupported or malformed text format"))?;
        self.host_policy.authorize(invocation, descriptor)
    }
}

fn write_failure(error: AtomicWriteError) -> ToolFailure {
    let code = match error {
        AtomicWriteError::ExistingTarget => "TARGET_EXISTS",
        AtomicWriteError::PublishedNeedsReconciliation => "WRITE_UNCERTAIN",
        AtomicWriteError::StageCollision => "STAGING_COLLISION",
        AtomicWriteError::InvalidTarget | AtomicWriteError::InvalidStage => "BAD_ARGUMENTS",
        AtomicWriteError::Oversized => "TOO_LARGE",
        _ => "WRITE_FAILURE",
    };
    // Never expose local filesystem paths or implementation internals through
    // the tool result. The durable dispatcher marks all failed writes UNCERTAIN.
    ToolFailure::new(code, "create-new failed or requires reconciliation", false)
}

pub struct CreateNewTool {
    descriptor: ToolDescriptor,
    trusted_root: Arc<Dir>,
}

impl NativeTool for CreateNewTool {
    fn descriptor(&self) -> &ToolDescriptor { &self.descriptor }
    fn invoke(&self, invocation: ToolInvocation) -> ToolFuture<'_> {
        let root = Arc::clone(&self.trusted_root);
        Box::pin(async move {
            if invocation.input.type_url != REQUEST_TYPE {
                return Err(ToolFailure::new("BAD_PROTOBUF", "wrong request contract", false));
            }
            let request = proto::CeWriteRequest::decode(invocation.input.bytes.as_slice())
                .map_err(|_| ToolFailure::new("BAD_PROTOBUF", "invalid write request", false))?;
            // Do not allow model-selected directories or stage IDs.
            if !simple_target(&request.relative_path)
                || request.data.len() > MAX_ATOMIC_WRITE_BYTES
            {
                return Err(ToolFailure::new("BAD_ARGUMENTS", "invalid create-new target or size", false));
            }
            let content = std::str::from_utf8(&request.data)
                .map_err(|_| ToolFailure::new(
                    "INVALID_TEXT", "create-new text tool only accepts UTF-8", false
                ))?;
            crate::write_format::identify_text_format(&request.relative_path, content)
                .map_err(|_| ToolFailure::new(
                    "INVALID_TEXT_FORMAT", "filename and UTF-8 content failed text-format admission", false
                ))?;
            let call_id = invocation.call_id;
            tokio::task::spawn_blocking(move || {
                let receipt = create_new_atomic_in_dir(
                    &root, &request.relative_path, &call_id, &request.data
                ).map_err(write_failure)?;
                let result = proto::CeWriteResult {
                    bytes_written: receipt.bytes_written as u64,
                    committed: true,
                };
                Ok(ToolOutput {
                    payload: ProtoPayload {
                        type_url: RESULT_TYPE.to_owned(),
                        bytes: result.encode_to_vec(),
                    },
                })
            }).await.map_err(|_| ToolFailure::new(
                "WRITE_UNCERTAIN", "native file worker outcome unknown", false
            ))?
        })
    }
}

/// Registers only a trusted host's destination directory; registration
/// doesn't grant the model permission to call it.
pub fn register_create_new(
    registry: &NativeToolRegistry,
    trusted_directory: &Path,
) -> Result<(), RegistryError> {
    let root = Dir::open_ambient_dir(trusted_directory, ambient_authority())
        .map_err(|_| RegistryError::InvalidDescriptor(TOOL_ID.into()))?;
    register_create_new_pinned(registry, Arc::new(root))
}

/// Register an already-opened host-selected directory. The host retains
/// this capability through approval and dispatch, preventing path retargeting.
pub fn register_create_new_pinned(
    registry: &NativeToolRegistry,
    root: Arc<Dir>,
) -> Result<(), RegistryError> {
    let descriptor = ToolDescriptor {
        id: TOOL_ID.into(),
        display_name: "Create a new file without overwriting".into(),
        effect: ToolEffect::WriteWorkspace,
        input_type_url: REQUEST_TYPE.into(),
        output_type_url: RESULT_TYPE.into(),
    };
    registry.register(
        ModuleManifest {
            id: "lu.create_new_file".into(),
            display_name: "Lu atomic create-new tool".into(),
            contract_version: CORE_CONTRACT_VERSION,
            kind: ModuleKind::Core,
            tools: vec![descriptor.clone()],
            panels: vec![],
            workflows: vec![],
        },
        vec![Arc::new(CreateNewTool { descriptor, trusted_root: root })],
    )
}
