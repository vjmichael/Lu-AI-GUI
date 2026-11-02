use lu_core::{
    contracts::{
        AuthorizationError, ModuleKind, ModuleManifest, NativeTool, ProtoPayload, ToolAuthorizer,
        ToolDescriptor, ToolEffect, ToolFailure, ToolFuture, ToolInvocation, ToolOutput,
        CORE_CONTRACT_VERSION,
    },
    durable::DurableDispatcher,
    journal::{EffectJournal, EffectState},
    registry::{DispatchError, NativeToolRegistry},
};
use std::{
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{SystemTime, UNIX_EPOCH},
};

const INPUT: &str = "type.googleapis.com/lu.v1.Operation";
const OUTPUT: &str = "type.googleapis.com/lu.v1.Done";

struct Effect {
    desc: ToolDescriptor,
    count: Arc<AtomicUsize>,
    fail: bool,
}
impl NativeTool for Effect {
    fn descriptor(&self) -> &ToolDescriptor {
        &self.desc
    }
    fn invoke(&self, _: ToolInvocation) -> ToolFuture<'_> {
        self.count.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move {
            if self.fail {
                Err(ToolFailure::new(
                    "PARTIAL_EFFECT",
                    "could have changed files",
                    false,
                ))
            } else {
                Ok(ToolOutput {
                    payload: ProtoPayload {
                        type_url: OUTPUT.to_string(),
                        bytes: b"completed".to_vec(),
                    },
                })
            }
        })
    }
}
struct Allow;
impl ToolAuthorizer for Allow {
    fn authorize(&self, _: &ToolInvocation, _: &ToolDescriptor) -> Result<(), AuthorizationError> {
        Ok(())
    }
}
fn setup(fail: bool) -> (NativeToolRegistry, Arc<AtomicUsize>) {
    let registry = NativeToolRegistry::new();
    let desc = ToolDescriptor {
        id: "lu.core.effect".into(),
        display_name: "Effect".into(),
        effect: ToolEffect::WriteWorkspace,
        input_type_url: INPUT.into(),
        output_type_url: OUTPUT.into(),
    };
    let count = Arc::new(AtomicUsize::new(0));
    registry
        .register(
            ModuleManifest {
                id: "lu.core".into(),
                display_name: "Core".into(),
                contract_version: CORE_CONTRACT_VERSION,
                kind: ModuleKind::Core,
                tools: vec![desc.clone()],
                panels: vec![],
                workflows: vec![],
            },
            vec![Arc::new(Effect {
                desc,
                count: Arc::clone(&count),
                fail,
            })],
        )
        .unwrap();
    (registry, count)
}
fn call() -> ToolInvocation {
    ToolInvocation {
        call_id: "effect-1".into(),
        tool_id: "lu.core.effect".into(),
        workspace_id: None,
        input: ProtoPayload {
            type_url: INPUT.into(),
            bytes: vec![1, 2],
        },
    }
}
fn db() -> PathBuf {
    // A timestamp alone is not a reliable per-test identity when tests run
    // concurrently on VMs with coarse or virtualized wall-clock resolution.
    static NEXT_DB: AtomicUsize = AtomicUsize::new(0);
    let sequence = NEXT_DB.fetch_add(1, Ordering::Relaxed);
    let n = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "lu-dispatch-{}-{n}-{sequence}.sqlite",
        std::process::id()
    ))
}

#[tokio::test]
async fn second_invocation_uses_durable_receipt_instead_of_repeating_effect() {
    let (registry, count) = setup(false);
    let path = db();
    let journal = EffectJournal::open(&path).unwrap();
    let dispatcher = DurableDispatcher {
        registry: &registry,
        journal: &journal,
    };
    let first = dispatcher.dispatch(call(), &Allow).await.unwrap();
    let second = dispatcher.dispatch(call(), &Allow).await.unwrap();
    assert_eq!(first, second);
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert_eq!(
        journal.get("effect-1").unwrap().unwrap().state,
        EffectState::Applied
    );
    drop(journal);
    let _ = fs::remove_file(path);
}

#[tokio::test]
async fn partial_effect_errors_become_uncertain_and_block_auto_replay() {
    let (registry, count) = setup(true);
    let path = db();
    let journal = EffectJournal::open(&path).unwrap();
    let dispatcher = DurableDispatcher {
        registry: &registry,
        journal: &journal,
    };
    assert!(matches!(
        dispatcher.dispatch(call(), &Allow).await,
        Err(DispatchError::Execution(_))
    ));
    assert!(
        matches!(dispatcher.dispatch(call(), &Allow).await, Err(DispatchError::Execution(ToolFailure { code, .. })) if code == "EFFECT_REPLAY_BLOCKED")
    );
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert_eq!(
        journal.get("effect-1").unwrap().unwrap().state,
        EffectState::Uncertain
    );
    drop(journal);
    let _ = fs::remove_file(path);
}
