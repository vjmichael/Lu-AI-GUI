//! Mythos Guard regression tests: a permissive model authorizer must NEVER
//! turn native tool registration into root/admin or sprinkler access.

use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use lu_core::{
    durable::DurableDispatcher,
    journal::EffectJournal,
    policy::{CapabilityGrant, PolicyEngine, PolicyError},
    privilege_guard::{enforce_model_ceiling, ProtectedEffect},
    registry::DispatchError,
    wire, AuthorizationError, ModuleKind, ModuleManifest, NativeTool, NativeToolRegistry,
    ProtoPayload, ToolAuthorizer, ToolDescriptor, ToolEffect, ToolFuture, ToolInvocation,
    ToolOutput, CORE_CONTRACT_VERSION,
};

struct MisconfiguredAllowEverything;
impl ToolAuthorizer for MisconfiguredAllowEverything {
    fn authorize(&self, _: &ToolInvocation, _: &ToolDescriptor) -> Result<(), AuthorizationError> {
        Ok(())
    }
}

struct DangerousTool {
    descriptor: ToolDescriptor,
    counter: Arc<AtomicUsize>,
}

impl NativeTool for DangerousTool {
    fn descriptor(&self) -> &ToolDescriptor {
        &self.descriptor
    }
    fn invoke(&self, _: ToolInvocation) -> ToolFuture<'_> {
        self.counter.fetch_add(1, Ordering::SeqCst);
        Box::pin(async {
            Ok(ToolOutput {
                payload: ProtoPayload {
                    type_url: "type.googleapis.com/lu.v1.Done".into(),
                    bytes: Vec::new(),
                },
            })
        })
    }
}

fn tool(effect: ToolEffect) -> (NativeToolRegistry, ToolInvocation, Arc<AtomicUsize>) {
    let registry = NativeToolRegistry::new();
    let counter = Arc::new(AtomicUsize::new(0));
    let descriptor = ToolDescriptor {
        id: "lu.core.protected".into(),
        display_name: "Hypothetical protected operation".into(),
        effect,
        input_type_url: "type.googleapis.com/lu.v1.Request".into(),
        output_type_url: "type.googleapis.com/lu.v1.Done".into(),
    };
    registry
        .register(
            ModuleManifest {
                id: "lu.core".into(),
                display_name: "Lu core test".into(),
                contract_version: CORE_CONTRACT_VERSION,
                kind: ModuleKind::Core,
                tools: vec![descriptor.clone()],
                panels: vec![],
                workflows: vec![],
            },
            vec![Arc::new(DangerousTool {
                descriptor: descriptor.clone(),
                counter: Arc::clone(&counter),
            })],
        )
        .unwrap();
    let invocation = ToolInvocation {
        call_id: "mythos-tries-root".into(),
        tool_id: descriptor.id,
        workspace_id: Some("garden".into()),
        input: ProtoPayload {
            type_url: "type.googleapis.com/lu.v1.Request".into(),
            bytes: Vec::new(),
        },
    };
    (registry, invocation, counter)
}

#[test]
fn no_model_specific_exceptions_to_protected_capability_ceiling() {
    assert_eq!(
        enforce_model_ceiling(ToolEffect::Elevated),
        Err(ProtectedEffect::OperatingSystemElevation)
    );
    assert_eq!(
        enforce_model_ceiling(ToolEffect::DeviceControl),
        Err(ProtectedEffect::PhysicalDeviceControl)
    );
    assert!(enforce_model_ceiling(ToolEffect::ReadWorkspace).is_ok());
    assert!(enforce_model_ceiling(ToolEffect::WriteWorkspace).is_ok());
    assert!(enforce_model_ceiling(ToolEffect::ExecuteCode).is_ok());
}

#[tokio::test]
async fn permissive_authorizer_cannot_give_model_root_or_sprinkler_access() {
    for effect in [ToolEffect::Elevated, ToolEffect::DeviceControl] {
        let (registry, invocation, counter) = tool(effect);
        let result = registry
            .dispatch(invocation, &MisconfiguredAllowEverything)
            .await;
        assert!(matches!(result, Err(DispatchError::ProtectedEffect(_))));
        assert_eq!(counter.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn host_policy_refuses_protected_grants_before_they_are_installed() {
    let policy = PolicyEngine::new();
    for effect in [ToolEffect::Elevated, ToolEffect::DeviceControl] {
        let result = policy.grant(CapabilityGrant {
            tool_id: "lu.core.protected".into(),
            scope_id: "garden".into(),
            effect,
            expires_at: None,
        });
        assert!(matches!(result, Err(PolicyError::ProtectedEffect(_))));
        assert_eq!(policy.revision(), 0);
    }
}

#[tokio::test]
async fn a_cached_receipt_cannot_bypass_new_privilege_ceiling() {
    for effect in [ToolEffect::Elevated, ToolEffect::DeviceControl] {
        let (registry, invocation, counter) = tool(effect);
        let journal = EffectJournal::open(std::path::Path::new(":memory:")).unwrap();
        journal.prepare(&invocation).unwrap();
        journal.mark_executing(&invocation.call_id).unwrap();
        let completed: Result<ToolOutput, DispatchError> = Ok(ToolOutput {
            payload: ProtoPayload {
                type_url: "type.googleapis.com/lu.v1.Done".into(),
                bytes: Vec::new(),
            },
        });
        let receipt = wire::encode_reply(&invocation.call_id, &completed).unwrap();
        journal.mark_applied(&invocation.call_id, &receipt).unwrap();

        let dispatcher = DurableDispatcher {
            registry: &registry,
            journal: &journal,
        };
        let response = dispatcher
            .dispatch(invocation, &MisconfiguredAllowEverything)
            .await;
        assert!(matches!(response, Err(DispatchError::ProtectedEffect(_))));
        assert_eq!(counter.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn protected_failure_gets_a_nonretryable_protobuf_error() {
    let result: Result<ToolOutput, DispatchError> = Err(DispatchError::ProtectedEffect(
        ProtectedEffect::OperatingSystemElevation,
    ));
    let receipt = wire::encode_reply("call-protected", &result).unwrap();
    let message = wire::decode_reply(&receipt).unwrap();
    match message.outcome {
        Some(wire::proto::tool_reply::Outcome::Failure(error)) => {
            assert_eq!(error.code, "PRIVILEGE_BLOCKED");
            assert!(!error.retryable);
        }
        _ => panic!("expected guarded failure"),
    }
}
