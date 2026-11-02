//! Trusted-host workspace registration and instance lifecycle.
//!
//! Installing a module does NOT load arbitrary code. The host supplies a
//! vetted ModuleManifest and independently constructed NativeTool instances.
//! This layer binds an active workspace instance to the owning module before
//! authorizing tool use.

use std::{
    collections::BTreeMap,
    sync::{Arc, RwLock},
};

use crate::{
    contracts::{
        AuthorizationError, ModuleKind, ModuleManifest, NativeTool, ToolAuthorizer, ToolDescriptor,
        ToolInvocation,
    },
    registry::{DispatchError, NativeToolRegistry, RegistryError, ToolResponse},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceState {
    Active,
    Suspended,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceInstance {
    pub id: String,
    pub module_id: String,
    pub state: WorkspaceState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceError {
    NotWorkspaceModule,
    UnknownModule(String),
    UnknownInstance(String),
    DuplicateInstance(String),
    InstanceInUse(String),
    InvalidInstanceId,
    Poisoned,
    Registry(RegistryError),
}

#[derive(Clone, Default)]
pub struct WorkspaceHost {
    registry: NativeToolRegistry,
    instances: Arc<RwLock<BTreeMap<String, WorkspaceInstance>>>,
}

impl WorkspaceHost {
    pub fn new(registry: NativeToolRegistry) -> Self {
        Self {
            registry,
            instances: Arc::default(),
        }
    }

    pub fn registry(&self) -> &NativeToolRegistry {
        &self.registry
    }

    /// For now, the caller must be a trusted host that already vetted the
    /// module and built implementations. Untrusted package loading/signatures
    /// are not implemented in this initial module.
    pub fn register_workspace(
        &self,
        manifest: ModuleManifest,
        tools: Vec<Arc<dyn NativeTool>>,
    ) -> Result<(), WorkspaceError> {
        if manifest.kind != ModuleKind::Workspace {
            return Err(WorkspaceError::NotWorkspaceModule);
        }
        self.registry
            .register(manifest, tools)
            .map_err(WorkspaceError::Registry)
    }

    pub fn open_instance(&self, module_id: &str, instance_id: &str) -> Result<(), WorkspaceError> {
        if instance_id.trim().is_empty() || instance_id.len() > 128 {
            return Err(WorkspaceError::InvalidInstanceId);
        }
        // Lock ordering is instances -> registry here AND in unregister.
        // This prevents opening a stale instance while uninstalling a module.
        let mut instances = self
            .instances
            .write()
            .map_err(|_| WorkspaceError::Poisoned)?;
        if !self
            .registry
            .modules()
            .map_err(WorkspaceError::Registry)?
            .iter()
            .any(|module| module.id == module_id && module.kind == ModuleKind::Workspace)
        {
            return Err(WorkspaceError::UnknownModule(module_id.to_string()));
        }
        if instances.contains_key(instance_id) {
            return Err(WorkspaceError::DuplicateInstance(instance_id.to_string()));
        }
        instances.insert(
            instance_id.to_string(),
            WorkspaceInstance {
                id: instance_id.to_string(),
                module_id: module_id.to_string(),
                state: WorkspaceState::Active,
            },
        );
        Ok(())
    }

    fn update_state(&self, instance_id: &str, state: WorkspaceState) -> Result<(), WorkspaceError> {
        let mut instances = self
            .instances
            .write()
            .map_err(|_| WorkspaceError::Poisoned)?;
        let instance = instances
            .get_mut(instance_id)
            .ok_or_else(|| WorkspaceError::UnknownInstance(instance_id.to_string()))?;
        instance.state = state;
        Ok(())
    }

    pub fn suspend_instance(&self, instance_id: &str) -> Result<(), WorkspaceError> {
        self.update_state(instance_id, WorkspaceState::Suspended)
    }

    pub fn resume_instance(&self, instance_id: &str) -> Result<(), WorkspaceError> {
        self.update_state(instance_id, WorkspaceState::Active)
    }

    pub fn close_instance(&self, instance_id: &str) -> Result<(), WorkspaceError> {
        let mut instances = self
            .instances
            .write()
            .map_err(|_| WorkspaceError::Poisoned)?;
        if instances.remove(instance_id).is_none() {
            return Err(WorkspaceError::UnknownInstance(instance_id.to_string()));
        }
        Ok(())
    }

    pub fn unregister_workspace(&self, module_id: &str) -> Result<(), WorkspaceError> {
        // Hold this lock while removing the module so open_instance cannot race
        // ahead and create an instance after the active-instance check.
        let instances = self
            .instances
            .write()
            .map_err(|_| WorkspaceError::Poisoned)?;
        if instances.values().any(|x| x.module_id == module_id) {
            return Err(WorkspaceError::InstanceInUse(module_id.to_string()));
        }
        self.registry
            .unregister_workspace(module_id)
            .map_err(WorkspaceError::Registry)
    }

    pub fn instances(&self) -> Result<Vec<WorkspaceInstance>, WorkspaceError> {
        let instances = self
            .instances
            .read()
            .map_err(|_| WorkspaceError::Poisoned)?;
        Ok(instances.values().cloned().collect())
    }

    fn instance_owns(&self, instance_id: &str, module_id: &str) -> Result<bool, WorkspaceError> {
        let instances = self
            .instances
            .read()
            .map_err(|_| WorkspaceError::Poisoned)?;
        Ok(instances
            .get(instance_id)
            .is_some_and(|x| x.module_id == module_id && x.state == WorkspaceState::Active))
    }

    /// The workspace package identifier and the running instance identifier
    /// are distinct. A tool call may run only from an active instance of the
    /// module that registered it, AND the host policy must allow the action.
    pub async fn dispatch(
        &self,
        invocation: ToolInvocation,
        policy: &dyn ToolAuthorizer,
    ) -> ToolResponse {
        let owner = self
            .registry
            .modules()
            .map_err(|_| DispatchError::Poisoned)?
            .into_iter()
            .find(|m| m.tools.iter().any(|t| t.id == invocation.tool_id));
        let Some(module) = owner else {
            return Err(DispatchError::UnknownTool(invocation.tool_id));
        };
        let auth = ScopedAuthorizer {
            host: self,
            owner: &module.id,
            kind: module.kind,
            policy,
        };
        self.registry.dispatch(invocation, &auth).await
    }
}

struct ScopedAuthorizer<'a> {
    host: &'a WorkspaceHost,
    owner: &'a str,
    kind: ModuleKind,
    policy: &'a dyn ToolAuthorizer,
}

impl ToolAuthorizer for ScopedAuthorizer<'_> {
    fn authorize(
        &self,
        invocation: &ToolInvocation,
        descriptor: &ToolDescriptor,
    ) -> Result<(), AuthorizationError> {
        if self.kind == ModuleKind::Workspace {
            let instance_id = invocation.workspace_id.as_deref().unwrap_or("");
            if !self
                .host
                .instance_owns(instance_id, self.owner)
                .unwrap_or(false)
            {
                return Err(AuthorizationError::new(
                    "workspace instance inactive, missing, or not owned by this module",
                ));
            }
        }
        self.policy.authorize(invocation, descriptor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        contracts::{ProtoPayload, ToolEffect, ToolFuture, ToolOutput, CORE_CONTRACT_VERSION},
        registry::DispatchError,
    };

    struct Dummy(ToolDescriptor);
    impl NativeTool for Dummy {
        fn descriptor(&self) -> &ToolDescriptor {
            &self.0
        }
        fn invoke(&self, _: ToolInvocation) -> ToolFuture<'_> {
            Box::pin(async {
                Ok(ToolOutput {
                    payload: ProtoPayload {
                        type_url: "type.googleapis.com/lu.v1.Done".into(),
                        bytes: vec![],
                    },
                })
            })
        }
    }
    struct Allow;
    impl ToolAuthorizer for Allow {
        fn authorize(
            &self,
            _: &ToolInvocation,
            _: &ToolDescriptor,
        ) -> Result<(), AuthorizationError> {
            Ok(())
        }
    }
    fn descriptor() -> ToolDescriptor {
        ToolDescriptor {
            id: "acme.nvp.render".into(),
            display_name: "Render".into(),
            effect: ToolEffect::ExecuteCode,
            input_type_url: "type.googleapis.com/lu.v1.Request".into(),
            output_type_url: "type.googleapis.com/lu.v1.Done".into(),
        }
    }
    fn call(instance: &str) -> ToolInvocation {
        ToolInvocation {
            call_id: "call-1".into(),
            tool_id: "acme.nvp.render".into(),
            workspace_id: Some(instance.into()),
            input: ProtoPayload {
                type_url: "type.googleapis.com/lu.v1.Request".into(),
                bytes: vec![],
            },
        }
    }

    #[tokio::test]
    async fn only_active_instances_of_the_right_package_can_dispatch() {
        let host = WorkspaceHost::default();
        let d = descriptor();
        host.register_workspace(
            ModuleManifest {
                id: "acme.nvp".into(),
                display_name: "NVP".into(),
                contract_version: CORE_CONTRACT_VERSION,
                kind: ModuleKind::Workspace,
                tools: vec![d.clone()],
                panels: vec![],
                workflows: vec![],
            },
            vec![Arc::new(Dummy(d))],
        )
        .unwrap();
        assert!(matches!(
            host.dispatch(call("missing"), &Allow).await,
            Err(DispatchError::Denied(_))
        ));
        host.open_instance("acme.nvp", "nvp-1").unwrap();
        assert!(host.dispatch(call("nvp-1"), &Allow).await.is_ok());
        host.suspend_instance("nvp-1").unwrap();
        assert!(matches!(
            host.dispatch(call("nvp-1"), &Allow).await,
            Err(DispatchError::Denied(_))
        ));
        host.resume_instance("nvp-1").unwrap();
        assert!(matches!(
            host.unregister_workspace("acme.nvp"),
            Err(WorkspaceError::InstanceInUse(_))
        ));
        host.close_instance("nvp-1").unwrap();
        host.unregister_workspace("acme.nvp").unwrap();
    }
}
