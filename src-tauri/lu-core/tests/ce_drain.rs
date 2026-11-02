//! Regression tests for CE admission closure and in-flight unload safety.
//! This is NOT a subprocess sandbox test.
use std::sync::{Arc, Mutex};

use lu_core::{
    AuthorizationError, CoreExtensionError, CoreExtensionHost, DispatchError, ModuleKind,
    ModuleManifest, NativeTool, NativeToolRegistry, ProtoPayload, RegistryError, ToolAuthorizer,
    ToolDescriptor, ToolEffect, ToolFuture, ToolInvocation, ToolOutput, CORE_CONTRACT_VERSION,
};
use tokio::sync::oneshot;

const INPUT: &str = "type.googleapis.com/lu.v1.ReadTextFileRequest";
const OUTPUT: &str = "type.googleapis.com/lu.v1.ReadTextFileResult";
const MODULE: &str = "lu.reader";
const TOOL: &str = "lu.reader.read";

struct Allow;
impl ToolAuthorizer for Allow {
    fn authorize(&self, _: &ToolInvocation, _: &ToolDescriptor) -> Result<(), AuthorizationError> {
        Ok(())
    }
}

struct PausedTool {
    descriptor: ToolDescriptor,
    started: Mutex<Option<oneshot::Sender<()>>>,
    resume: Mutex<Option<oneshot::Receiver<()>>>,
}

impl NativeTool for PausedTool {
    fn descriptor(&self) -> &ToolDescriptor {
        &self.descriptor
    }

    fn invoke(&self, invocation: ToolInvocation) -> ToolFuture<'_> {
        let started = self.started.lock().unwrap().take();
        let resume = self.resume.lock().unwrap().take().unwrap();
        Box::pin(async move {
            if let Some(started) = started {
                let _ = started.send(());
            }
            let _ = resume.await;
            Ok(ToolOutput {
                payload: ProtoPayload {
                    type_url: OUTPUT.into(),
                    bytes: invocation.input.bytes,
                },
            })
        })
    }
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor {
        id: TOOL.into(),
        display_name: "Read".into(),
        effect: ToolEffect::ReadWorkspace,
        input_type_url: INPUT.into(),
        output_type_url: OUTPUT.into(),
    }
}

fn call() -> ToolInvocation {
    ToolInvocation {
        call_id: "call-a".into(),
        tool_id: TOOL.into(),
        workspace_id: None,
        input: ProtoPayload {
            type_url: INPUT.into(),
            bytes: vec![1, 2, 3],
        },
    }
}

fn setup() -> (
    NativeToolRegistry,
    CoreExtensionHost,
    oneshot::Receiver<()>,
    oneshot::Sender<()>,
) {
    let registry = NativeToolRegistry::new();
    let host = CoreExtensionHost::new(registry.clone());
    let (started_tx, started_rx) = oneshot::channel();
    let (resume_tx, resume_rx) = oneshot::channel();
    host.register_trusted(
        ModuleManifest {
            id: MODULE.into(),
            display_name: "Reader".into(),
            contract_version: CORE_CONTRACT_VERSION,
            kind: ModuleKind::CoreExtension,
            tools: vec![descriptor()],
            panels: vec![],
            workflows: vec![],
        },
        vec![Arc::new(PausedTool {
            descriptor: descriptor(),
            started: Mutex::new(Some(started_tx)),
            resume: Mutex::new(Some(resume_rx)),
        })],
    )
    .unwrap();
    (registry, host, started_rx, resume_tx)
}

#[tokio::test]
async fn busy_unload_closes_admission_and_retries_after_completion() {
    let (registry, host, started, resume) = setup();
    let running_registry = registry.clone();
    let running = tokio::spawn(async move { running_registry.dispatch(call(), &Allow).await });
    started.await.unwrap();

    assert_eq!(
        host.unregister_trusted(MODULE),
        Err(CoreExtensionError::Registry(RegistryError::CallsInFlight {
            module_id: MODULE.into(),
            active_calls: 1,
        }))
    );
    assert_eq!(
        registry.dispatch(call(), &Allow).await,
        Err(DispatchError::ModuleDraining(MODULE.into()))
    );
    assert_eq!(host.extensions().unwrap().len(), 1);
    assert!(registry.tools().unwrap().is_empty());

    resume.send(()).unwrap();
    assert_eq!(running.await.unwrap().unwrap().payload.bytes, vec![1, 2, 3]);
    host.unregister_trusted(MODULE).unwrap();
    assert!(host.extensions().unwrap().is_empty());
    assert!(matches!(
        registry.dispatch(call(), &Allow).await,
        Err(DispatchError::UnknownTool(_))
    ));
}

#[tokio::test]
async fn cancelled_future_releases_lease_and_allows_safe_unload() {
    let (registry, host, started, _resume) = setup();
    let running_registry = registry.clone();
    let running = tokio::spawn(async move { running_registry.dispatch(call(), &Allow).await });
    started.await.unwrap();
    assert!(matches!(
        host.unregister_trusted(MODULE),
        Err(CoreExtensionError::Registry(
            RegistryError::CallsInFlight { .. }
        ))
    ));
    running.abort();
    assert!(running.await.is_err());
    host.unregister_trusted(MODULE).unwrap();
    assert!(registry.tools().unwrap().is_empty());
}

#[tokio::test]
async fn validation_failure_does_not_leave_a_live_execution_lease() {
    struct Deny;
    impl ToolAuthorizer for Deny {
        fn authorize(
            &self,
            _: &ToolInvocation,
            _: &ToolDescriptor,
        ) -> Result<(), AuthorizationError> {
            Err(AuthorizationError::new("denied"))
        }
    }
    let (registry, host, _started, _resume) = setup();
    assert_eq!(
        registry.dispatch(call(), &Deny).await,
        Err(DispatchError::Denied("denied".into()))
    );
    host.unregister_trusted(MODULE).unwrap();
}
