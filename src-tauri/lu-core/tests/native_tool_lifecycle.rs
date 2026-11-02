//! Phase 4F: cancellation/revocation *inside* native async tool invocation,
//! and durable recovery of a started-but-unconfirmed filesystem effect.
//! Deterministic Rust fixtures do not claim an OS worker kill or Windows UI E2E.
use lu_core::{
    agent_tool_loop::{NativeToolTurn, ToolTurnError},
    contracts::{
        AuthorizationError, ModuleKind, ModuleManifest, NativeTool, ProtoPayload, ToolAuthorizer,
        ToolDescriptor, ToolEffect, ToolFailure, ToolFuture, ToolInvocation, ToolOutput,
        CORE_CONTRACT_VERSION,
    },
    conversation_authority::{
        AuthorizedCapability, BoundCallOrigin, ConversationAuthority, TaskAuthority,
        TrustedUserTurnVerifier,
    },
    inference_host::Cancellation,
    journal::{EffectJournal, EffectState, JournalError, PrepareOutcome},
    model::{ModelToolCatalog, ModelToolCodec, ModelToolDefinition, ModelToolError},
    model_turn_journal::{ModelAuditKind, ModelTurnJournal},
    registry::NativeToolRegistry,
    supervised_tool_generation::{
        run_supervised_native_turn_journaled, ModelStepGenerator, NativeGenerationError,
        NativeTurnServices,
    },
};
use prost::Message;
use serde_json::{json, Map, Value};
use std::{
    collections::BTreeSet,
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};

const REQUEST: &str = "type.googleapis.com/lu.v1.LifecycleRequest";
const REPLY: &str = "type.googleapis.com/lu.v1.LifecycleReply";
const TOOL: &str = "lu.fixture.effect";
const PUBLIC: &str = "lu_fixture_effect";

#[derive(Clone, PartialEq, Message)]
struct LifecycleRequest {
    #[prost(string, tag = "1")]
    payload: String,
}
#[derive(Clone, PartialEq, Message)]
struct LifecycleReply {
    #[prost(bool, tag = "1")]
    committed: bool,
}

struct Codec;
impl ModelToolCodec for Codec {
    fn input_type_url(&self) -> &'static str {
        REQUEST
    }
    fn output_type_url(&self) -> &'static str {
        REPLY
    }
    fn encode(&self, args: &Map<String, Value>) -> Result<ProtoPayload, ModelToolError> {
        if args.len() != 1 || args.get("payload").and_then(Value::as_str) != Some("write") {
            return Err(ModelToolError::InvalidArguments(
                "unexpected payload".into(),
            ));
        }
        Ok(ProtoPayload {
            type_url: REQUEST.into(),
            bytes: LifecycleRequest {
                payload: "write".into(),
            }
            .encode_to_vec(),
        })
    }
    fn decode(&self, output: &ProtoPayload) -> Result<Value, ModelToolError> {
        let value = LifecycleReply::decode(output.bytes.as_slice())
            .map_err(|_| ModelToolError::InvalidResult("bad protobuf".into()))?;
        Ok(json!({ "committed": value.committed }))
    }
}

struct Verified;
impl TrustedUserTurnVerifier for Verified {
    fn verify_user_turn(&self, c: &str, t: &str) -> bool {
        c == "conversation-4f" && t == "user-4f"
    }
}
struct Permitted;
impl ToolAuthorizer for Permitted {
    fn authorize(&self, _: &ToolInvocation, _: &ToolDescriptor) -> Result<(), AuthorizationError> {
        Ok(())
    }
}

#[derive(Clone)]
enum Interruption {
    Cancel(Cancellation),
    Revoke(ConversationAuthority),
}
struct FixtureTool {
    descriptor: ToolDescriptor,
    effect_journal: Arc<EffectJournal>,
    applied: Arc<AtomicUsize>,
    seen: Arc<Mutex<Option<ToolInvocation>>>,
    interruption: Interruption,
}
impl NativeTool for FixtureTool {
    fn descriptor(&self) -> &ToolDescriptor {
        &self.descriptor
    }
    fn invoke(&self, call: ToolInvocation) -> ToolFuture<'_> {
        Box::pin(async move {
            let input = LifecycleRequest::decode(call.input.bytes.as_slice())
                .map_err(|_| ToolFailure::new("INVALID_PROTO", "missing typed payload", false))?;
            if input.payload != "write" {
                return Err(ToolFailure::new(
                    "INVALID_INPUT",
                    "unexpected test input",
                    false,
                ));
            }
            let fail =
                |e: JournalError| ToolFailure::new("JOURNAL_FAILED", format!("{e:?}"), false);
            if self.effect_journal.prepare(&call).map_err(fail)? != PrepareOutcome::New {
                return Err(ToolFailure::new(
                    "REPLAY_BLOCKED",
                    "already prepared",
                    false,
                ));
            }
            self.effect_journal
                .mark_executing(&call.call_id)
                .map_err(fail)?;
            self.applied.fetch_add(1, Ordering::SeqCst);
            *self.seen.lock().unwrap() = Some(call.clone());
            // Receipt is committed BEFORE completion is returned. The effect
            // is not undone by later cancellation; never execute it twice.
            self.effect_journal
                .mark_applied(&call.call_id, b"applied")
                .map_err(fail)?;
            match &self.interruption {
                Interruption::Cancel(cancel) => cancel.cancel(),
                Interruption::Revoke(authority) => {
                    authority.revoke_task("task-4f").map_err(|_| {
                        ToolFailure::new("REVOKE_FAILED", "host revocation failed", false)
                    })?
                }
            }
            tokio::task::yield_now().await;
            Ok(ToolOutput {
                payload: ProtoPayload {
                    type_url: REPLY.into(),
                    bytes: LifecycleReply { committed: true }.encode_to_vec(),
                },
            })
        })
    }
}
struct Scripted {
    calls: usize,
}
impl ModelStepGenerator for Scripted {
    fn generate_step(
        &mut self,
        _: &str,
        _: u32,
        _: &Cancellation,
    ) -> Result<String, NativeGenerationError> {
        self.calls += 1;
        match self.calls {
            1 => Ok(
                r#"{"kind":"tool","name":"lu_fixture_effect","arguments":{"payload":"write"}}"#
                    .into(),
            ),
            2 => Ok(r#"{"kind":"final","text":"completed"}"#.into()),
            _ => panic!("unexpected extra model generation"),
        }
    }
}

fn temp() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path =
        std::env::temp_dir().join(format!("lu-tool-lifecycle-{}-{nonce}", std::process::id()));
    fs::create_dir(&path).unwrap();
    path
}

async fn exercise(interruption_kind: &str) {
    let root = temp();
    let effect_journal = Arc::new(EffectJournal::open(&root.join("effects.sqlite")).unwrap());
    let audit = ModelTurnJournal::open(&root.join("model-audit.sqlite")).unwrap();
    let authority = ConversationAuthority::new();
    let origin = BoundCallOrigin {
        conversation_id: "conversation-4f".into(),
        user_turn_id: "user-4f".into(),
        task_id: "task-4f".into(),
    };
    authority
        .open_user_task(
            TaskAuthority {
                conversation_id: origin.conversation_id.clone(),
                user_turn_id: origin.user_turn_id.clone(),
                task_id: origin.task_id.clone(),
                execution_scope_id: "workspace-4f".into(),
                capabilities: vec![AuthorizedCapability {
                    tool_id: TOOL.into(),
                    effect: ToolEffect::WriteWorkspace,
                }],
                expires_at: None,
            },
            &Verified,
        )
        .unwrap();
    let cancel = Cancellation::default();
    let interruption = if interruption_kind == "cancel" {
        Interruption::Cancel(cancel.clone())
    } else {
        Interruption::Revoke(authority.clone())
    };
    let applied = Arc::new(AtomicUsize::new(0));
    let seen = Arc::new(Mutex::new(None));
    let descriptor = ToolDescriptor {
        id: TOOL.into(),
        display_name: "Test effect".into(),
        effect: ToolEffect::WriteWorkspace,
        input_type_url: REQUEST.into(),
        output_type_url: REPLY.into(),
    };
    let tool = Arc::new(FixtureTool {
        descriptor: descriptor.clone(),
        effect_journal: Arc::clone(&effect_journal),
        applied: Arc::clone(&applied),
        seen: Arc::clone(&seen),
        interruption,
    });
    let registry = NativeToolRegistry::new();
    registry
        .register(
            ModuleManifest {
                id: "lu.fixture".into(),
                display_name: "Fixture".into(),
                kind: ModuleKind::Core,
                contract_version: CORE_CONTRACT_VERSION,
                tools: vec![descriptor],
                panels: vec![],
                workflows: vec![],
            },
            vec![tool.clone()],
        )
        .unwrap();
    let mut catalog = ModelToolCatalog::new();
    catalog
        .insert(
            &registry,
            ModelToolDefinition {
                name: PUBLIC.into(),
                native_tool_id: TOOL.into(),
                description: "Test one durable workspace effect".into(),
                parameters: json!({
                    "type":"object",
                    "properties":{"payload":{"type":"string"}},
                    "required":["payload"],"additionalProperties":false
                }),
                codec: Arc::new(Codec),
            },
        )
        .unwrap();
    // The production read-only desktop constructor must reject a genuinely
    // registered write capability even when the host grant permits it.
    assert!(matches!(
        NativeToolTurn::new_read_only(
            &catalog,
            &registry,
            &authority,
            origin.clone(),
            "workspace-4f".into(),
            &BTreeSet::from([PUBLIC.into()]),
        ),
        Err(ToolTurnError::ReadOnlyTurnViolation)
    ));
    assert_eq!(applied.load(Ordering::SeqCst), 0);
    let mut turn = NativeToolTurn::new(
        &catalog,
        &authority,
        origin,
        "workspace-4f".into(),
        &BTreeSet::from([PUBLIC.into()]),
    )
    .unwrap();
    let mut model = Scripted { calls: 0 };
    let result = run_supervised_native_turn_journaled(
        "Perform a single authorized fixture workspace effect",
        &mut turn,
        &mut model,
        &NativeTurnServices {
            catalog: &catalog,
            registry: &registry,
            authority: &authority,
            host_policy: &Permitted,
        },
        &cancel,
        &audit,
    )
    .await;
    if interruption_kind == "cancel" {
        assert_eq!(result, Err(NativeGenerationError::Cancelled));
    } else {
        assert_eq!(
            result,
            Err(NativeGenerationError::ModelReply(ToolTurnError::Revoked))
        );
    }
    assert_eq!(
        model.calls, 1,
        "no final model step after interrupted effect"
    );
    assert_eq!(
        applied.load(Ordering::SeqCst),
        1,
        "effect was not undone/repeated"
    );
    assert_eq!(turn.step_count(), 1);
    assert_eq!(
        turn.handle_model_reply(
            r#"{"kind":"tool","name":"lu_fixture_effect","arguments":{"payload":"write"}}"#,
            &catalog,
            &registry,
            &authority,
            &Permitted,
        )
        .await,
        Err(ToolTurnError::AlreadyFinished),
        "a completed/interrupted turn must not be resumed by a stale model reply",
    );
    let original = seen
        .lock()
        .unwrap()
        .clone()
        .expect("tool invocation recorded");
    assert_eq!(
        effect_journal.prepare(&original).unwrap(),
        PrepareOutcome::Existing(EffectState::Applied),
    );
    assert_eq!(
        tool.invoke(original.clone()).await.unwrap_err().code,
        "REPLAY_BLOCKED"
    );
    assert_eq!(applied.load(Ordering::SeqCst), 1);
    let events = audit.events_for("conversation-4f", "task-4f").unwrap();
    assert_eq!(events[0].kind, ModelAuditKind::UserRequest);
    assert_eq!(events[1].kind, ModelAuditKind::ModelOutput);
    assert_eq!(events[3].kind, ModelAuditKind::ToolFeedback);
    assert_eq!(events[4].kind, ModelAuditKind::GenerationFailure);
    assert!(!events.iter().any(|v| v.kind == ModelAuditKind::FinalAnswer));
    assert!(events[4].payload.contains("effect may have applied"));
    drop(turn);
    drop(registry);
    drop(tool);
    drop(catalog);
    drop(audit);
    drop(effect_journal);
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn cancellation_during_real_native_invocation_records_effect_but_never_continues() {
    exercise("cancel").await;
}
#[tokio::test]
async fn mid_call_revocation_prevents_followup_or_stale_task_replay() {
    exercise("revoke").await;
}

#[test]
fn crash_after_real_file_effect_before_receipt_is_uncertain_not_silently_replayed() {
    let root = temp();
    let data = root.join("effect.txt");
    let request = ToolInvocation {
        call_id: "write-4f".into(),
        tool_id: TOOL.into(),
        workspace_id: Some("workspace-4f".into()),
        input: ProtoPayload {
            type_url: REQUEST.into(),
            bytes: LifecycleRequest {
                payload: "write".into(),
            }
            .encode_to_vec(),
        },
    };
    {
        let journal = EffectJournal::open(&root.join("effects.sqlite")).unwrap();
        assert_eq!(journal.prepare(&request).unwrap(), PrepareOutcome::New);
        journal.mark_executing(&request.call_id).unwrap();
        fs::write(&data, "applied once").unwrap();
        // Simulate host/worker termination before any successful receipt.
    }
    let journal = EffectJournal::open(&root.join("effects.sqlite")).unwrap();
    assert_eq!(journal.reconcile_interrupted().unwrap(), 1);
    assert_eq!(journal.reconcile_interrupted().unwrap(), 0);
    assert_eq!(
        journal.prepare(&request).unwrap(),
        PrepareOutcome::Existing(EffectState::Uncertain),
    );
    assert_eq!(
        journal.mark_executing(&request.call_id),
        Err(JournalError::InvalidTransition),
    );
    assert_eq!(fs::read_to_string(&data).unwrap(), "applied once");
    drop(journal);
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn bound_durable_dispatch_replays_once_and_revocation_fences_cached_receipt() {
    struct CountedTool {
        descriptor: ToolDescriptor,
        count: Arc<AtomicUsize>,
    }
    impl NativeTool for CountedTool {
        fn descriptor(&self) -> &ToolDescriptor {
            &self.descriptor
        }
        fn invoke(&self, _: ToolInvocation) -> ToolFuture<'_> {
            Box::pin(async move {
                self.count.fetch_add(1, Ordering::SeqCst);
                Ok(ToolOutput {
                    payload: ProtoPayload {
                        type_url: REPLY.into(),
                        bytes: LifecycleReply { committed: true }.encode_to_vec(),
                    },
                })
            })
        }
    }
    let root = temp();
    let journal = EffectJournal::open(&root.join("bound-effects.sqlite")).unwrap();
    let authority = ConversationAuthority::new();
    let origin = BoundCallOrigin {
        conversation_id: "conversation-4f".into(),
        user_turn_id: "user-4f".into(),
        task_id: "task-bound-4f".into(),
    };
    authority
        .open_user_task(
            TaskAuthority {
                conversation_id: origin.conversation_id.clone(),
                user_turn_id: origin.user_turn_id.clone(),
                task_id: origin.task_id.clone(),
                execution_scope_id: "workspace-4f".into(),
                capabilities: vec![AuthorizedCapability {
                    tool_id: TOOL.into(),
                    effect: ToolEffect::WriteWorkspace,
                }],
                expires_at: None,
            },
            &Verified,
        )
        .unwrap();
    let count = Arc::new(AtomicUsize::new(0));
    let descriptor = ToolDescriptor {
        id: TOOL.into(),
        display_name: "Counted effect".into(),
        effect: ToolEffect::WriteWorkspace,
        input_type_url: REQUEST.into(),
        output_type_url: REPLY.into(),
    };
    let registry = NativeToolRegistry::new();
    registry
        .register(
            ModuleManifest {
                id: "lu.fixture".into(),
                display_name: "Fixture".into(),
                kind: ModuleKind::Core,
                contract_version: CORE_CONTRACT_VERSION,
                tools: vec![descriptor.clone()],
                panels: vec![],
                workflows: vec![],
            },
            vec![Arc::new(CountedTool {
                descriptor,
                count: count.clone(),
            })],
        )
        .unwrap();
    let call = ToolInvocation {
        call_id: "bound-write-4f".into(),
        tool_id: TOOL.into(),
        workspace_id: Some("workspace-4f".into()),
        input: ProtoPayload {
            type_url: REQUEST.into(),
            bytes: LifecycleRequest {
                payload: "write".into(),
            }
            .encode_to_vec(),
        },
    };
    let first = authority
        .dispatch_bound_durable(
            &registry,
            &journal,
            call.clone(),
            origin.clone(),
            &Permitted,
        )
        .await
        .unwrap();
    let replay = authority
        .dispatch_bound_durable(
            &registry,
            &journal,
            call.clone(),
            origin.clone(),
            &Permitted,
        )
        .await
        .unwrap();
    assert_eq!(first, replay);
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert_eq!(
        journal.get(&call.call_id).unwrap().unwrap().state,
        EffectState::Applied
    );
    authority.revoke_task(&origin.task_id).unwrap();
    assert!(
        authority
            .dispatch_bound_durable(&registry, &journal, call, origin, &Permitted)
            .await
            .is_err(),
        "revocation must fence even a stored receipt"
    );
    assert_eq!(count.load(Ordering::SeqCst), 1);
    drop(registry);
    drop(journal);
    fs::remove_dir_all(root).unwrap();
}
