use lu_core::{
    AuthorizationError, DispatchError, ModuleKind, ModuleManifest, NativeTool, NativeToolRegistry,
    PanelContribution, ProtoPayload, RegistryError, ToolAuthorizer, ToolDescriptor, ToolEffect,
    ToolFuture, ToolInvocation, ToolOutput, WorkflowContribution, CORE_CONTRACT_VERSION,
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

const INPUT: &str = "type.googleapis.com/lu.tools.ReadRequest";
const OUTPUT: &str = "type.googleapis.com/lu.tools.ReadResult";

struct EchoTool {
    descriptor: ToolDescriptor,
    invocations: Arc<AtomicUsize>,
}
impl NativeTool for EchoTool {
    fn descriptor(&self) -> &ToolDescriptor {
        &self.descriptor
    }
    fn invoke(&self, invocation: ToolInvocation) -> ToolFuture<'_> {
        self.invocations.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move {
            Ok(ToolOutput {
                payload: ProtoPayload {
                    type_url: OUTPUT.to_string(),
                    bytes: invocation.input.bytes,
                },
            })
        })
    }
}
struct Allow;
impl ToolAuthorizer for Allow {
    fn authorize(&self, _: &ToolInvocation, _: &ToolDescriptor) -> Result<(), AuthorizationError> {
        Ok(())
    }
}
struct Deny;
impl ToolAuthorizer for Deny {
    fn authorize(&self, _: &ToolInvocation, _: &ToolDescriptor) -> Result<(), AuthorizationError> {
        Err(AuthorizationError::new("approval required"))
    }
}
fn descriptor(id: &str) -> ToolDescriptor {
    ToolDescriptor {
        id: id.into(),
        display_name: "Read file".into(),
        effect: ToolEffect::ReadWorkspace,
        input_type_url: INPUT.into(),
        output_type_url: OUTPUT.into(),
    }
}
fn manifest(id: &str, kind: ModuleKind, tools: Vec<ToolDescriptor>) -> ModuleManifest {
    ModuleManifest {
        id: id.into(),
        display_name: id.into(),
        contract_version: CORE_CONTRACT_VERSION,
        kind,
        tools,
        panels: vec![],
        workflows: vec![],
    }
}
fn tool(d: ToolDescriptor, counter: &Arc<AtomicUsize>) -> Arc<dyn NativeTool> {
    Arc::new(EchoTool {
        descriptor: d,
        invocations: Arc::clone(counter),
    })
}
fn call(id: &str) -> ToolInvocation {
    ToolInvocation {
        call_id: "call-001".into(),
        tool_id: id.into(),
        workspace_id: None,
        input: ProtoPayload {
            type_url: INPUT.into(),
            bytes: vec![1, 2, 3],
        },
    }
}

#[test]
fn registration_is_atomic_when_implementation_is_missing_or_mismatched() {
    let reg = NativeToolRegistry::new();
    let counter = Arc::new(AtomicUsize::new(0));
    let d = descriptor("lu.core.files_read");
    assert!(matches!(
        reg.register(
            manifest("lu.core", ModuleKind::Core, vec![d.clone()]),
            vec![]
        ),
        Err(RegistryError::InvalidDescriptor(_))
    ));
    assert!(reg.tools().unwrap().is_empty());
    assert!(matches!(
        reg.register(
            manifest("lu.core", ModuleKind::Core, vec![d]),
            vec![tool(descriptor("lu.core.another"), &counter)]
        ),
        Err(RegistryError::DescriptorMismatch(_))
    ));
    assert!(reg.modules().unwrap().is_empty());
}

#[test]
fn duplicate_registration_is_rejected_without_replacing_existing_tools() {
    let reg = NativeToolRegistry::new();
    let counter = Arc::new(AtomicUsize::new(0));
    let id = "lu.core.files_read";
    reg.register(
        manifest("lu.core", ModuleKind::Core, vec![descriptor(id)]),
        vec![tool(descriptor(id), &counter)],
    )
    .unwrap();
    assert_eq!(reg.tools().unwrap().len(), 1);
    assert!(matches!(
        reg.register(
            manifest("lu.core", ModuleKind::Core, vec![descriptor(id)]),
            vec![tool(descriptor(id), &counter)]
        ),
        Err(RegistryError::DuplicateModule(_))
    ));
    assert_eq!(reg.tools().unwrap().len(), 1);
    assert!(matches!(
        reg.unregister_workspace("lu.core"),
        Err(RegistryError::CoreModuleProtected(_))
    ));
}

#[tokio::test]
async fn native_dispatch_checks_authorization_and_type_without_mcp() {
    let reg = NativeToolRegistry::new();
    let count = Arc::new(AtomicUsize::new(0));
    let id = "lu.core.files_read";
    reg.register(
        manifest("lu.core", ModuleKind::Core, vec![descriptor(id)]),
        vec![tool(descriptor(id), &count)],
    )
    .unwrap();
    assert_eq!(
        reg.dispatch(call(id), &Deny).await,
        Err(DispatchError::Denied("approval required".into()))
    );
    assert_eq!(count.load(Ordering::SeqCst), 0);
    let mut invalid = call(id);
    invalid.input.type_url = "type.googleapis.com/lu.tools.Other".into();
    assert!(matches!(
        reg.dispatch(invalid, &Allow).await,
        Err(DispatchError::InputTypeMismatch { .. })
    ));
    assert_eq!(count.load(Ordering::SeqCst), 0);
    let outcome = reg.dispatch(call(id), &Allow).await.unwrap();
    assert_eq!(outcome.payload.bytes, vec![1, 2, 3]);
    assert_eq!(count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn workspaces_require_scope_can_unload_and_cannot_unload_core() {
    let reg = NativeToolRegistry::new();
    let count = Arc::new(AtomicUsize::new(0));
    let id = "acme.nvp.render";
    let mut m = manifest("acme.nvp", ModuleKind::Workspace, vec![descriptor(id)]);
    m.panels.push(PanelContribution {
        id: "acme.nvp.timeline".into(),
        title: "Timeline".into(),
    });
    m.workflows.push(WorkflowContribution {
        id: "acme.nvp.export".into(),
        title: "Export".into(),
    });
    reg.register(m, vec![tool(descriptor(id), &count)]).unwrap();
    assert!(matches!(
        reg.dispatch(call(id), &Allow).await,
        Err(DispatchError::WorkspaceRequired(_))
    ));
    let mut c = call(id);
    c.workspace_id = Some("session-nvp".into());
    assert!(reg.dispatch(c, &Allow).await.is_ok());
    reg.unregister_workspace("acme.nvp").unwrap();
    assert!(matches!(
        reg.dispatch(call(id), &Allow).await,
        Err(DispatchError::UnknownTool(_))
    ));
}

#[test]
fn invalid_names_and_wire_type_urls_fail_closed() {
    let reg = NativeToolRegistry::new();
    let counter = Arc::new(AtomicUsize::new(0));
    let mut d = descriptor("lu.core.files_read");
    d.input_type_url = "application/json".into();
    assert!(matches!(
        reg.register(
            manifest("lu.core", ModuleKind::Core, vec![d.clone()]),
            vec![tool(d, &counter)]
        ),
        Err(RegistryError::InvalidDescriptor(_))
    ));
    assert!(reg.modules().unwrap().is_empty());
    let d = descriptor("foreign.tool.read");
    assert!(matches!(
        reg.register(
            manifest("lu.core", ModuleKind::Core, vec![d.clone()]),
            vec![tool(d, &counter)]
        ),
        Err(RegistryError::InvalidIdentifier(_))
    ));
}

#[tokio::test]
async fn registry_clones_share_state_and_unregistration_blocks_new_dispatches() {
    let reg = NativeToolRegistry::new();
    let observer = reg.clone();
    let count = Arc::new(AtomicUsize::new(0));
    let id = "acme.research.lookup";
    reg.register(
        manifest("acme.research", ModuleKind::Workspace, vec![descriptor(id)]),
        vec![tool(descriptor(id), &count)],
    )
    .unwrap();
    assert_eq!(observer.tools().unwrap().len(), 1);
    observer.unregister_workspace("acme.research").unwrap();
    assert_eq!(reg.tools().unwrap().len(), 0);
}
