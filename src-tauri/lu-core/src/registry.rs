use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, RwLock},
};

use crate::privilege_guard::{enforce_model_ceiling, ProtectedEffect};

use crate::contracts::{
    ModuleKind, ModuleManifest, NativeTool, ToolAuthorizer, ToolDescriptor, ToolFailure,
    ToolInvocation, ToolOutput, CORE_CONTRACT_VERSION,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    InvalidIdentifier(String),
    InvalidDescriptor(String),
    UnsupportedContractVersion(u16),
    DuplicateModule(String),
    DuplicateTool(String),
    DescriptorMismatch(String),
    CoreModuleProtected(String),
    NotWorkspaceModule(String),
    NotCoreExtensionModule(String),
    UnknownModule(String),
    /// The module is quiescing: no new dispatches are admitted. Retry unload
    /// after the current native calls have finished or been cancelled.
    CallsInFlight {
        module_id: String,
        active_calls: usize,
    },
    Poisoned,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchError {
    UnknownTool(String),
    ModuleDraining(String),
    InvalidCallId,
    WorkspaceRequired(String),
    InputTypeMismatch { expected: String, actual: String },
    Denied(String),
    ProtectedEffect(ProtectedEffect),
    Execution(ToolFailure),
    Poisoned,
}

/// Stable, non-sensitive error families for Lu's typed native-tool results.
/// Keep diagnostic details in the corresponding enum variant; do not send
/// filesystem paths, authorization reasons, or internal errors as error codes.
impl DispatchError {
    pub fn stable_code(&self) -> &'static str {
        match self {
            Self::UnknownTool(_) => "UNKNOWN_TOOL",
            Self::ModuleDraining(_) => "MODULE_DRAINING",
            Self::InvalidCallId => "INVALID_CALL_ID",
            Self::WorkspaceRequired(_) => "WORKSPACE_REQUIRED",
            Self::InputTypeMismatch { .. } => "INPUT_TYPE_MISMATCH",
            Self::Denied(_) => "TOOL_DENIED",
            Self::ProtectedEffect(_) => "PROTECTED_EFFECT",
            Self::Execution(_) => "TOOL_EXECUTION_FAILED",
            Self::Poisoned => "REGISTRY_UNAVAILABLE",
        }
    }
}

pub type ToolResponse = Result<ToolOutput, DispatchError>;

struct RegisteredTool {
    owner: String,
    descriptor: ToolDescriptor,
    implementation: Arc<dyn NativeTool>,
}

#[derive(Default)]
struct State {
    modules: BTreeMap<String, ModuleManifest>,
    tools: BTreeMap<String, RegisteredTool>,
    /// Updated under the same write lock as registration/unregistration.
    active_calls: BTreeMap<String, usize>,
    draining: BTreeSet<String>,
}

/// Holds a CE execution lease across the entire async invocation. Dropping a
/// future, including via cooperative cancellation, releases its lease. A
/// poisoned lock fails closed: unloading remains blocked, not forced.
struct CallLease {
    state: Arc<RwLock<State>>,
    owner: String,
}

impl Drop for CallLease {
    fn drop(&mut self) {
        if let Ok(mut state) = self.state.write() {
            if let Some(active) = state.active_calls.get_mut(&self.owner) {
                *active = active.saturating_sub(1);
            }
        }
    }
}

/// Independently testable, in-memory Lu-native registry. No MCP, Tauri, or
/// model-specific adapter is involved. Registry writes are atomic; dispatch
/// snapshots an Arc so the registry lock is not held across async execution.
/// CE dispatches take an in-flight lease before releasing that lock. CE unload
/// closes admission and refuses to unload while a lease is active. This is a
/// drain barrier, NOT forced cancellation or an OS process sandbox.
#[derive(Default, Clone)]
pub struct NativeToolRegistry {
    state: Arc<RwLock<State>>,
}

fn valid_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.contains('.')
        && s.split('.').all(|part| {
            !part.is_empty()
                && part.len() <= 64
                && part.bytes().enumerate().all(|(i, b)| {
                    b.is_ascii_lowercase()
                        || (i > 0 && (b.is_ascii_digit() || b == b'_' || b == b'-'))
                })
        })
}

fn valid_type_url(s: &str) -> bool {
    let Some(name) = s.strip_prefix("type.googleapis.com/") else {
        return false;
    };
    // Protobuf message names conventionally contain PascalCase identifiers.
    // This is a syntactic check only; schema-level validation is not provided.
    !name.is_empty()
        && name.len() <= 256
        && name.split('.').all(|part| {
            let mut chars = part.bytes();
            matches!(chars.next(), Some(b) if b.is_ascii_alphabetic() || b == b'_')
                && chars.all(|b| b.is_ascii_alphanumeric() || b == b'_')
        })
}

impl NativeToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// All-or-nothing module registration. Tools may not shadow any other
    /// module's registrations, including the default Lu core tools.
    pub fn register(
        &self,
        manifest: ModuleManifest,
        implementations: Vec<Arc<dyn NativeTool>>,
    ) -> Result<(), RegistryError> {
        if manifest.contract_version != CORE_CONTRACT_VERSION {
            return Err(RegistryError::UnsupportedContractVersion(
                manifest.contract_version,
            ));
        }
        if !valid_id(&manifest.id) {
            return Err(RegistryError::InvalidIdentifier(manifest.id));
        }
        let mut declared = BTreeSet::new();
        for d in &manifest.tools {
            if !valid_id(&d.id) || !d.id.starts_with(&(manifest.id.clone() + ".")) {
                return Err(RegistryError::InvalidIdentifier(d.id.clone()));
            }
            if d.display_name.trim().is_empty()
                || !valid_type_url(&d.input_type_url)
                || !valid_type_url(&d.output_type_url)
            {
                return Err(RegistryError::InvalidDescriptor(d.id.clone()));
            }
            if !declared.insert(d.id.clone()) {
                return Err(RegistryError::DuplicateTool(d.id.clone()));
            }
        }
        let mut contributed = BTreeSet::new();
        for p in &manifest.panels {
            if !valid_id(&p.id)
                || !p.id.starts_with(&(manifest.id.clone() + "."))
                || !contributed.insert(p.id.clone())
                || p.title.trim().is_empty()
            {
                return Err(RegistryError::InvalidDescriptor(p.id.clone()));
            }
        }
        for w in &manifest.workflows {
            if !valid_id(&w.id)
                || !w.id.starts_with(&(manifest.id.clone() + "."))
                || !contributed.insert(w.id.clone())
                || w.title.trim().is_empty()
            {
                return Err(RegistryError::InvalidDescriptor(w.id.clone()));
            }
        }
        if manifest.tools.len() != implementations.len() {
            return Err(RegistryError::InvalidDescriptor(
                "tool implementation count differs from manifest".into(),
            ));
        }
        let mut by_name = BTreeMap::new();
        for implementation in implementations {
            let id = implementation.descriptor().id.clone();
            if !declared.contains(&id) {
                return Err(RegistryError::DescriptorMismatch(id));
            }
            if by_name.insert(id.clone(), implementation).is_some() {
                return Err(RegistryError::DuplicateTool(id));
            }
        }
        for descriptor in &manifest.tools {
            if by_name
                .get(&descriptor.id)
                .is_none_or(|tool| tool.descriptor() != descriptor)
            {
                return Err(RegistryError::DescriptorMismatch(descriptor.id.clone()));
            }
        }

        let mut guard = self.state.write().map_err(|_| RegistryError::Poisoned)?;
        if guard.modules.contains_key(&manifest.id) {
            return Err(RegistryError::DuplicateModule(manifest.id));
        }
        for descriptor in &manifest.tools {
            if guard.tools.contains_key(&descriptor.id) {
                return Err(RegistryError::DuplicateTool(descriptor.id.clone()));
            }
        }
        for desc in &manifest.tools {
            let implementation = by_name
                .remove(&desc.id)
                .expect("descriptors were validated above");
            guard.tools.insert(
                desc.id.clone(),
                RegisteredTool {
                    owner: manifest.id.clone(),
                    descriptor: desc.clone(),
                    implementation,
                },
            );
        }
        guard.modules.insert(manifest.id.clone(), manifest);
        Ok(())
    }

    /// Unloading an installed workspace never removes core functionality.
    pub fn unregister_workspace(&self, module_id: &str) -> Result<(), RegistryError> {
        let mut guard = self.state.write().map_err(|_| RegistryError::Poisoned)?;
        let Some(module) = guard.modules.get(module_id) else {
            return Err(RegistryError::UnknownModule(module_id.to_string()));
        };
        if module.kind == ModuleKind::Core {
            return Err(RegistryError::CoreModuleProtected(module_id.to_string()));
        }
        if module.kind != ModuleKind::Workspace {
            return Err(RegistryError::NotWorkspaceModule(module_id.to_string()));
        }
        guard
            .tools
            .retain(|_, registered| registered.owner != module_id);
        guard.modules.remove(module_id);
        Ok(())
    }

    /// Quiesce and remove a CE. The first attempt closes admission before
    /// checking active calls, so even a busy response blocks new CE calls.
    /// Retry once the outstanding calls have completed or been cancelled.
    /// This does not kill native threads or subprocesses.
    pub fn unregister_extension(&self, module_id: &str) -> Result<(), RegistryError> {
        let mut guard = self.state.write().map_err(|_| RegistryError::Poisoned)?;
        let Some(module) = guard.modules.get(module_id) else {
            return Err(RegistryError::UnknownModule(module_id.to_string()));
        };
        if module.kind == ModuleKind::Core {
            return Err(RegistryError::CoreModuleProtected(module_id.to_string()));
        }
        if module.kind != ModuleKind::CoreExtension {
            return Err(RegistryError::NotCoreExtensionModule(module_id.to_string()));
        }
        guard.draining.insert(module_id.to_string());
        let active_calls = guard.active_calls.get(module_id).copied().unwrap_or(0);
        if active_calls != 0 {
            return Err(RegistryError::CallsInFlight {
                module_id: module_id.to_string(),
                active_calls,
            });
        }
        guard
            .tools
            .retain(|_, registered| registered.owner != module_id);
        guard.modules.remove(module_id);
        guard.active_calls.remove(module_id);
        guard.draining.remove(module_id);
        Ok(())
    }

    pub fn modules(&self) -> Result<Vec<ModuleManifest>, RegistryError> {
        let guard = self.state.read().map_err(|_| RegistryError::Poisoned)?;
        Ok(guard.modules.values().cloned().collect())
    }

    pub fn tools(&self) -> Result<Vec<ToolDescriptor>, RegistryError> {
        let guard = self.state.read().map_err(|_| RegistryError::Poisoned)?;
        // A quiescing CE is not available for new calls and must not be
        // advertised in a freshly requested model-facing catalog.
        Ok(guard
            .tools
            .values()
            .filter(|tool| !guard.draining.contains(&tool.owner))
            .map(|tool| tool.descriptor.clone())
            .collect())
    }

    /// The executor must not infer authorization from registration or effect
    /// labels. The host supplies a PolicyEngine-backed authorizer per call.
    pub async fn dispatch(
        &self,
        invocation: ToolInvocation,
        authorizer: &dyn ToolAuthorizer,
    ) -> ToolResponse {
        if invocation.call_id.trim().is_empty() {
            return Err(DispatchError::InvalidCallId);
        }
        let (descriptor, kind, implementation, _lease) = {
            let mut guard = self.state.write().map_err(|_| DispatchError::Poisoned)?;
            let tool = guard
                .tools
                .get(&invocation.tool_id)
                .ok_or_else(|| DispatchError::UnknownTool(invocation.tool_id.clone()))?;
            let owner = tool.owner.clone();
            let descriptor = tool.descriptor.clone();
            let implementation = Arc::clone(&tool.implementation);
            let kind = guard
                .modules
                .get(&owner)
                .expect("registry invariant: owner exists")
                .kind;
            if guard.draining.contains(&owner) {
                return Err(DispatchError::ModuleDraining(owner));
            }
            let lease = if kind == ModuleKind::CoreExtension {
                *guard.active_calls.entry(owner.clone()).or_default() += 1;
                Some(CallLease {
                    state: Arc::clone(&self.state),
                    owner,
                })
            } else {
                None
            };
            (descriptor, kind, implementation, lease)
        };
        if kind == ModuleKind::Workspace
            && invocation.workspace_id.as_deref().is_none_or(str::is_empty)
        {
            return Err(DispatchError::WorkspaceRequired(descriptor.id));
        }
        if invocation.input.type_url != descriptor.input_type_url {
            return Err(DispatchError::InputTypeMismatch {
                expected: descriptor.input_type_url,
                actual: invocation.input.type_url,
            });
        }
        // Mandatory and non-overridable: no model-driven elevation or
        // physical device control, even if a permissive authorizer is passed.
        enforce_model_ceiling(descriptor.effect).map_err(DispatchError::ProtectedEffect)?;
        authorizer
            .authorize(&invocation, &descriptor)
            .map_err(|e| DispatchError::Denied(e.reason))?;
        let result = implementation
            .invoke(invocation)
            .await
            .map_err(DispatchError::Execution)?;
        if result.payload.type_url != descriptor.output_type_url {
            return Err(DispatchError::Execution(ToolFailure::new(
                "OUTPUT_TYPE_MISMATCH",
                "native tool returned an unexpected Protobuf message type",
                false,
            )));
        }
        Ok(result)
    }
}
