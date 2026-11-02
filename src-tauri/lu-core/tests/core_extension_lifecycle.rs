use std::sync::Arc;

use lu_core::{
    AuthorizationError, CoreExtensionError, CoreExtensionHost, DispatchError, ModuleKind,
    ModuleManifest, NativeTool, NativeToolRegistry, ProtoPayload, RegistryError, ToolAuthorizer,
    ToolDescriptor, ToolEffect, ToolFuture, ToolInvocation, ToolOutput, CORE_CONTRACT_VERSION,
};

const INPUT: &str = "type.googleapis.com/lu.v1.ReadTextFileRequest";
const OUTPUT: &str = "type.googleapis.com/lu.v1.ReadTextFileResult";

struct Echo(ToolDescriptor);
impl NativeTool for Echo {
    fn descriptor(&self) -> &ToolDescriptor {
        &self.0
    }

    fn invoke(&self, invocation: ToolInvocation) -> ToolFuture<'_> {
        Box::pin(async move {
            Ok(ToolOutput {
                payload: ProtoPayload {
                    type_url: OUTPUT.into(),
                    bytes: invocation.input.bytes,
                },
            })
        })
    }
}

struct Deny;
impl ToolAuthorizer for Deny {
    fn authorize(&self, _: &ToolInvocation, _: &ToolDescriptor) -> Result<(), AuthorizationError> {
        Err(AuthorizationError::new("host did not grant this tool"))
    }
}

fn descriptor(id: &str) -> ToolDescriptor {
    ToolDescriptor {
        id: id.into(),
        display_name: "Read".into(),
        effect: ToolEffect::ReadWorkspace,
        input_type_url: INPUT.into(),
        output_type_url: OUTPUT.into(),
    }
}

fn manifest(id: &str, kind: ModuleKind, tool_id: &str) -> ModuleManifest {
    ModuleManifest {
        id: id.into(),
        display_name: id.into(),
        contract_version: CORE_CONTRACT_VERSION,
        kind,
        tools: vec![descriptor(tool_id)],
        panels: vec![],
        workflows: vec![],
    }
}

fn tool(id: &str) -> Arc<dyn NativeTool> {
    Arc::new(Echo(descriptor(id)))
}

#[tokio::test]
async fn extension_is_optional_and_never_bypasses_host_authorization() {
    let registry = NativeToolRegistry::new();
    let host = CoreExtensionHost::new(registry.clone());
    let core_id = "lu.core.read";
    let ce_id = "lu.terminal.run";
    registry
        .register(
            manifest("lu.core", ModuleKind::Core, core_id),
            vec![tool(core_id)],
        )
        .unwrap();
    host.register_trusted(
        manifest("lu.terminal", ModuleKind::CoreExtension, ce_id),
        vec![tool(ce_id)],
    )
    .unwrap();
    assert_eq!(host.extensions().unwrap().len(), 1);
    assert!(matches!(
        registry.unregister_workspace("lu.terminal"),
        Err(RegistryError::NotWorkspaceModule(_))
    ));
    let invocation = ToolInvocation {
        call_id: "call-1".into(),
        tool_id: ce_id.into(),
        workspace_id: None,
        input: ProtoPayload {
            type_url: INPUT.into(),
            bytes: vec![1, 2, 3],
        },
    };
    assert_eq!(
        registry.dispatch(invocation, &Deny).await,
        Err(DispatchError::Denied("host did not grant this tool".into()))
    );
    host.unregister_trusted("lu.terminal").unwrap();
    assert!(registry.tools().unwrap().iter().all(|t| t.id != ce_id));
    assert!(registry.tools().unwrap().iter().any(|t| t.id == core_id));
    assert!(host.extensions().unwrap().is_empty());
}

#[test]
fn extension_host_rejects_core_and_workspace_modules() {
    let registry = NativeToolRegistry::new();
    let host = CoreExtensionHost::new(registry.clone());
    for (id, kind, tool_id) in [
        ("lu.core", ModuleKind::Core, "lu.core.read"),
        ("acme.film", ModuleKind::Workspace, "acme.film.render"),
    ] {
        assert_eq!(
            host.register_trusted(manifest(id, kind, tool_id), vec![tool(tool_id)]),
            Err(CoreExtensionError::NotCoreExtensionModule)
        );
    }
    assert!(registry.modules().unwrap().is_empty());
    registry
        .register(
            manifest("acme.film", ModuleKind::Workspace, "acme.film.render"),
            vec![tool("acme.film.render")],
        )
        .unwrap();
    assert!(matches!(
        host.unregister_trusted("acme.film"),
        Err(CoreExtensionError::Registry(
            RegistryError::NotCoreExtensionModule(_)
        ))
    ));
    assert_eq!(registry.modules().unwrap().len(), 1);
}
