//! Cross-platform, no-GGUF host-route/effect-journal acceptance tests.
//! A synthetic sample DLC stands in for any optional signed extension.
//! Passing these checks is NOT permission to install or run a real worker.
use lu_core::{
    agent_tool_loop::{NativeToolTurn, ToolTurnError},
    capability_router::{ReviewedHostRouteRule, ReviewedRouteCandidate},
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
    journal::EffectJournal,
    model::{ModelToolCatalog, ModelToolCodec, ModelToolDefinition, ModelToolError},
    model_turn_journal::{ModelAuditKind, ModelTurnJournal},
    registry::NativeToolRegistry,
    supervised_tool_generation::{
        run_supervised_native_turn_journaled_observed_with_host_routing, ModelStepGenerator,
        NativeGenerationError, NativeTurnServices,
    },
};
use rusqlite::Connection;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{SystemTime, UNIX_EPOCH},
};

const INPUT: &str = "type.googleapis.com/acme.v1.SampleFractionRequest";
const OUTPUT: &str = "type.googleapis.com/acme.v1.SampleFractionResult";
const NATIVE_ID: &str = "acme.sample.fraction";
const PUBLIC_NAME: &str = "dlc_sample_fraction";
const SCOPE: &str = "thread:conversation-test";
const TASK: &str = "task-review-route";

/// This descriptor is host-reviewed test input, not installed in Lu by default.
const HOST_RULE: ReviewedHostRouteRule = ReviewedHostRouteRule {
    rule_id: "host.reviewed.sample_fraction.v1",
    public_name: PUBLIC_NAME,
    native_tool_id: NATIVE_ID,
    effect: ToolEffect::ExecuteCode,
    input_type_url: INPUT,
    argument_name: "expression",
};
fn reviewed_for_current_user(user_text: &str) -> ReviewedRouteCandidate {
    ReviewedRouteCandidate::from_host_current_user(
        user_text,
        HOST_RULE.rule_id,
        HOST_RULE.public_name,
        json!({"expression": "9/14 + 5/21"}),
    )
}

struct Verifier;
impl TrustedUserTurnVerifier for Verifier {
    fn verify_user_turn(&self, conversation_id: &str, user_turn_id: &str) -> bool {
        conversation_id == "conversation-test" && user_turn_id == "user-test"
    }
}
struct Policy;
impl ToolAuthorizer for Policy {
    fn authorize(&self, _: &ToolInvocation, _: &ToolDescriptor) -> Result<(), AuthorizationError> {
        Ok(())
    }
}
struct DenyPolicy;
impl ToolAuthorizer for DenyPolicy {
    fn authorize(&self, _: &ToolInvocation, _: &ToolDescriptor) -> Result<(), AuthorizationError> {
        Err(AuthorizationError::new("host policy denied this operation"))
    }
}
struct Codec;
impl ModelToolCodec for Codec {
    fn input_type_url(&self) -> &'static str {
        INPUT
    }
    fn output_type_url(&self) -> &'static str {
        OUTPUT
    }
    fn encode(&self, args: &Map<String, Value>) -> Result<ProtoPayload, ModelToolError> {
        if args.len() != 1 {
            return Err(ModelToolError::InvalidArguments("fields".into()));
        }
        let expr = args
            .get("expression")
            .and_then(Value::as_str)
            .ok_or_else(|| ModelToolError::InvalidArguments("expression".into()))?;
        if expr != "9/14 + 5/21" {
            return Err(ModelToolError::InvalidArguments(
                "unexpected test input".into(),
            ));
        }
        Ok(ProtoPayload {
            type_url: INPUT.into(),
            bytes: expr.as_bytes().to_vec(),
        })
    }
    fn decode(&self, output: &ProtoPayload) -> Result<Value, ModelToolError> {
        if output.type_url != OUTPUT {
            return Err(ModelToolError::InvalidResult("type".into()));
        }
        let value = std::str::from_utf8(&output.bytes)
            .map_err(|_| ModelToolError::InvalidResult("utf8".into()))?;
        Ok(json!({"exact": value}))
    }
}
struct TestTool {
    descriptor: ToolDescriptor,
    hits: Arc<AtomicUsize>,
    cancel_on_call: Option<Cancellation>,
}
impl NativeTool for TestTool {
    fn descriptor(&self) -> &ToolDescriptor {
        &self.descriptor
    }
    fn invoke(&self, invocation: ToolInvocation) -> ToolFuture<'_> {
        let hits = Arc::clone(&self.hits);
        let cancel = self.cancel_on_call.clone();
        Box::pin(async move {
            if invocation.input.bytes != b"9/14 + 5/21" {
                return Err(ToolFailure::new("INVALID_TEST_INPUT", "unexpected", false));
            }
            hits.fetch_add(1, Ordering::SeqCst);
            if let Some(cancel) = cancel {
                cancel.cancel();
            }
            Ok(ToolOutput {
                payload: ProtoPayload {
                    type_url: OUTPUT.into(),
                    bytes: b"37/42".to_vec(),
                },
            })
        })
    }
}
struct FinalModel {
    calls: usize,
    reply: &'static str,
}
impl ModelStepGenerator for FinalModel {
    fn generate_step(
        &mut self,
        prompt: &str,
        _: u32,
        _: &Cancellation,
    ) -> Result<String, NativeGenerationError> {
        self.calls += 1;
        assert!(prompt.contains("9/14 + 5/21"));
        Ok(self.reply.to_owned())
    }
}
fn fixture(
    cancel_on_call: Option<Cancellation>,
) -> (
    NativeToolRegistry,
    ModelToolCatalog,
    ConversationAuthority,
    BoundCallOrigin,
    Arc<AtomicUsize>,
) {
    let registry = NativeToolRegistry::new();
    let descriptor = ToolDescriptor {
        id: NATIVE_ID.into(),
        display_name: "reviewed rational fixture".into(),
        effect: ToolEffect::ExecuteCode,
        input_type_url: INPUT.into(),
        output_type_url: OUTPUT.into(),
    };
    let hits = Arc::new(AtomicUsize::new(0));
    registry
        .register(
            ModuleManifest {
                id: "acme.sample".into(),
                display_name: "reviewed fixture".into(),
                contract_version: CORE_CONTRACT_VERSION,
                kind: ModuleKind::CoreExtension,
                tools: vec![descriptor.clone()],
                panels: vec![],
                workflows: vec![],
            },
            vec![Arc::new(TestTool {
                descriptor,
                hits: Arc::clone(&hits),
                cancel_on_call,
            })],
        )
        .unwrap();
    let mut catalog = ModelToolCatalog::new();
    catalog
        .insert(
            &registry,
            ModelToolDefinition {
                name: PUBLIC_NAME.into(),
                native_tool_id: NATIVE_ID.into(),
                description: "reviewed rational fixture".into(),
                parameters: json!({"type":"object", "properties":{"expression":{"type":"string"}},
            "required":["expression"], "additionalProperties":false}),
                codec: Arc::new(Codec),
            },
        )
        .unwrap();
    let authority = ConversationAuthority::new();
    authority
        .open_user_task(
            TaskAuthority {
                conversation_id: "conversation-test".into(),
                user_turn_id: "user-test".into(),
                task_id: TASK.into(),
                execution_scope_id: SCOPE.into(),
                capabilities: vec![AuthorizedCapability {
                    tool_id: NATIVE_ID.into(),
                    effect: ToolEffect::ExecuteCode,
                }],
                expires_at: None,
            },
            &Verifier,
        )
        .unwrap();
    let origin = BoundCallOrigin {
        conversation_id: "conversation-test".into(),
        user_turn_id: "user-test".into(),
        task_id: TASK.into(),
    };
    (registry, catalog, authority, origin, hits)
}
fn turn(
    catalog: &ModelToolCatalog,
    authority: &ConversationAuthority,
    origin: BoundCallOrigin,
) -> NativeToolTurn {
    NativeToolTurn::new(
        catalog,
        authority,
        origin,
        SCOPE.into(),
        &BTreeSet::from([PUBLIC_NAME.to_owned()]),
    )
    .unwrap()
}
fn root(tag: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "lu-host-route-{tag}-{}-{stamp}",
        std::process::id()
    ));
    fs::create_dir(&dir).unwrap();
    dir
}
fn expected_call_id() -> String {
    let digest = Sha256::digest(TASK.as_bytes());
    let prefix = digest
        .iter()
        .take(12)
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    format!("lu-{prefix}-1")
}

#[tokio::test]
async fn reviewed_route_has_durable_receipt_and_same_call_never_reexecutes() {
    let dir = root("replay");
    let audit = ModelTurnJournal::open(&dir.join("audit.sqlite")).unwrap();
    let effects = EffectJournal::open(&dir.join("effects.sqlite")).unwrap();
    let (registry, catalog, authority, origin, hits) = fixture(None);
    for iteration in 0..2 {
        let mut active_turn = turn(&catalog, &authority, origin.clone());
        let mut model = FinalModel {
            calls: 0,
            reply: r#"{"kind":"final","text":"37/42"}"#,
        };
        let report = run_supervised_native_turn_journaled_observed_with_host_routing(
            "Compute exactly: 9/14 + 5/21",
            "Compute exactly: 9/14 + 5/21",
            Some(&reviewed_for_current_user("Compute exactly: 9/14 + 5/21")),
            &[HOST_RULE],
            &mut active_turn,
            &mut model,
            &NativeTurnServices {
                catalog: &catalog,
                registry: &registry,
                authority: &authority,
                host_policy: &Policy,
            },
            &Cancellation::default(),
            &audit,
            &effects,
            &|_| {},
        )
        .await
        .unwrap();
        assert_eq!(report.host_routed_calls, 1);
        assert_eq!(report.tool_calls, 0);
        assert_eq!(report.model_generations, 1);
        assert_eq!(model.calls, 1);
        assert_eq!(
            hits.load(Ordering::SeqCst),
            1,
            "a resumed exact call must return its receipt, never rerun"
        );
        let record = effects
            .get(&expected_call_id())
            .unwrap()
            .expect("durable receipt");
        assert_eq!(record.state, lu_core::journal::EffectState::Applied);
        assert!(record.receipt.is_some());
        let rows = audit.events_for("conversation-test", TASK).unwrap();
        assert_eq!(rows[iteration * 6].kind, ModelAuditKind::UserRequest);
        assert_eq!(
            rows[iteration * 6 + 1].kind,
            ModelAuditKind::HostRouteSelected
        );
        assert_eq!(rows[iteration * 6 + 2].kind, ModelAuditKind::ToolFeedback);
        assert_eq!(rows[iteration * 6 + 5].kind, ModelAuditKind::FinalAnswer);
    }
    drop(effects);
    drop(audit);
    fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn model_audit_failure_prevents_host_route_and_model_generation() {
    let dir = root("audit-denial");
    let audit_path = dir.join("audit.sqlite");
    let audit = ModelTurnJournal::open(&audit_path).unwrap();
    let effects = EffectJournal::open(&dir.join("effects.sqlite")).unwrap();
    let sqlite = Connection::open(&audit_path).unwrap();
    sqlite
        .execute_batch(
            "CREATE TRIGGER reject_host_selection BEFORE INSERT ON lu_model_boundary_audit
         WHEN NEW.kind='HOST_ROUTE_SELECTED'
         BEGIN SELECT RAISE(ABORT,'route audit must not proceed'); END;",
        )
        .unwrap();
    drop(sqlite);
    let (registry, catalog, authority, origin, hits) = fixture(None);
    let mut active_turn = turn(&catalog, &authority, origin);
    let mut model = FinalModel {
        calls: 0,
        reply: r#"{"kind":"final","text":"no effect"}"#,
    };
    let result = run_supervised_native_turn_journaled_observed_with_host_routing(
        "Compute exactly: 9/14 + 5/21",
        "Compute exactly: 9/14 + 5/21",
        Some(&reviewed_for_current_user("Compute exactly: 9/14 + 5/21")),
        &[HOST_RULE],
        &mut active_turn,
        &mut model,
        &NativeTurnServices {
            catalog: &catalog,
            registry: &registry,
            authority: &authority,
            host_policy: &Policy,
        },
        &Cancellation::default(),
        &audit,
        &effects,
        &|_| {},
    )
    .await;
    assert_eq!(result.unwrap_err(), NativeGenerationError::JournalFailure);
    assert_eq!(hits.load(Ordering::SeqCst), 0);
    assert_eq!(model.calls, 0);
    assert!(effects.get(&expected_call_id()).unwrap().is_none());
    drop(effects);
    drop(audit);
    fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn cancellation_after_durable_effect_does_not_generate_second_step() {
    let dir = root("cancel");
    let audit = ModelTurnJournal::open(&dir.join("audit.sqlite")).unwrap();
    let effects = EffectJournal::open(&dir.join("effects.sqlite")).unwrap();
    let cancel = Cancellation::default();
    let (registry, catalog, authority, origin, hits) = fixture(Some(cancel.clone()));
    let mut active_turn = turn(&catalog, &authority, origin);
    let mut model = FinalModel {
        calls: 0,
        reply: r#"{"kind":"final","text":"no continuation"}"#,
    };
    let result = run_supervised_native_turn_journaled_observed_with_host_routing(
        "Compute exactly: 9/14 + 5/21",
        "Compute exactly: 9/14 + 5/21",
        Some(&reviewed_for_current_user("Compute exactly: 9/14 + 5/21")),
        &[HOST_RULE],
        &mut active_turn,
        &mut model,
        &NativeTurnServices {
            catalog: &catalog,
            registry: &registry,
            authority: &authority,
            host_policy: &Policy,
        },
        &cancel,
        &audit,
        &effects,
        &|_| {},
    )
    .await;
    assert_eq!(result.unwrap_err(), NativeGenerationError::Cancelled);
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    assert_eq!(model.calls, 0);
    assert_eq!(
        effects.get(&expected_call_id()).unwrap().unwrap().state,
        lu_core::journal::EffectState::Applied
    );
    let kinds = audit
        .events_for("conversation-test", TASK)
        .unwrap()
        .into_iter()
        .map(|r| r.kind)
        .collect::<Vec<_>>();
    assert_eq!(
        kinds,
        vec![
            ModelAuditKind::UserRequest,
            ModelAuditKind::HostRouteSelected,
            ModelAuditKind::ToolFeedback,
            ModelAuditKind::GenerationFailure,
        ]
    );
    drop(effects);
    drop(audit);
    fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn unknown_model_tool_after_host_route_never_triggers_legacy_retry() {
    let dir = root("invented");
    let audit = ModelTurnJournal::open(&dir.join("audit.sqlite")).unwrap();
    let effects = EffectJournal::open(&dir.join("effects.sqlite")).unwrap();
    let (registry, catalog, authority, origin, hits) = fixture(None);
    let mut active_turn = turn(&catalog, &authority, origin);
    let mut model = FinalModel {
        calls: 0,
        reply: r#"{"kind":"tool","name":"unadvertised_call","arguments":{}}"#,
    };
    let result = run_supervised_native_turn_journaled_observed_with_host_routing(
        "Compute exactly: 9/14 + 5/21",
        "Compute exactly: 9/14 + 5/21",
        Some(&reviewed_for_current_user("Compute exactly: 9/14 + 5/21")),
        &[HOST_RULE],
        &mut active_turn,
        &mut model,
        &NativeTurnServices {
            catalog: &catalog,
            registry: &registry,
            authority: &authority,
            host_policy: &Policy,
        },
        &Cancellation::default(),
        &audit,
        &effects,
        &|_| {},
    )
    .await;
    assert!(matches!(
        result,
        Err(NativeGenerationError::ModelReply(ToolTurnError::Catalog(
            ModelToolError::UnknownTool(_)
        )))
    ));
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    assert_eq!(model.calls, 1, "no unknown-tool retry after host effect");
    assert_eq!(
        effects.get(&expected_call_id()).unwrap().unwrap().state,
        lu_core::journal::EffectState::Applied
    );
    drop(effects);
    drop(audit);
    fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn denied_host_policy_is_terminal_without_any_native_effect() {
    let dir = root("host-denied");
    let audit = ModelTurnJournal::open(&dir.join("audit.sqlite")).unwrap();
    let effects = EffectJournal::open(&dir.join("effects.sqlite")).unwrap();
    let (registry, catalog, authority, origin, hits) = fixture(None);
    let mut active_turn = turn(&catalog, &authority, origin);
    let mut model = FinalModel {
        calls: 0,
        reply: r#"{"kind":"final","text":"invalid continuation"}"#,
    };
    let result = run_supervised_native_turn_journaled_observed_with_host_routing(
        "Compute exactly: 9/14 + 5/21",
        "Compute exactly: 9/14 + 5/21",
        Some(&reviewed_for_current_user("Compute exactly: 9/14 + 5/21")),
        &[HOST_RULE],
        &mut active_turn,
        &mut model,
        &NativeTurnServices {
            catalog: &catalog,
            registry: &registry,
            authority: &authority,
            host_policy: &DenyPolicy,
        },
        &Cancellation::default(),
        &audit,
        &effects,
        &|_| {},
    )
    .await;
    assert_eq!(
        result.unwrap_err(),
        NativeGenerationError::ModelReply(ToolTurnError::DurableDispatchFailed)
    );
    assert_eq!(model.calls, 0);
    assert_eq!(hits.load(Ordering::SeqCst), 0);
    assert!(effects.get(&expected_call_id()).unwrap().is_none());
    assert_eq!(
        audit
            .events_for("conversation-test", TASK)
            .unwrap()
            .into_iter()
            .map(|e| e.kind)
            .collect::<Vec<_>>(),
        vec![
            ModelAuditKind::UserRequest,
            ModelAuditKind::HostRouteSelected,
            ModelAuditKind::Rejected
        ]
    );
    drop(effects);
    drop(audit);
    fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn effect_journal_insert_failure_is_terminal_without_execution() {
    let dir = root("effects-unavailable");
    let audit = ModelTurnJournal::open(&dir.join("audit.sqlite")).unwrap();
    let effects_path = dir.join("effects.sqlite");
    let effects = EffectJournal::open(&effects_path).unwrap();
    let sqlite = Connection::open(&effects_path).unwrap();
    sqlite
        .execute_batch(
            "CREATE TRIGGER reject_durable_prepare BEFORE INSERT ON lu_effects
         BEGIN SELECT RAISE(ABORT,'effect ledger unavailable'); END;",
        )
        .unwrap();
    drop(sqlite);
    let (registry, catalog, authority, origin, hits) = fixture(None);
    let mut active_turn = turn(&catalog, &authority, origin);
    let mut model = FinalModel {
        calls: 0,
        reply: r#"{"kind":"final","text":"invalid continuation"}"#,
    };
    let result = run_supervised_native_turn_journaled_observed_with_host_routing(
        "Compute exactly: 9/14 + 5/21",
        "Compute exactly: 9/14 + 5/21",
        Some(&reviewed_for_current_user("Compute exactly: 9/14 + 5/21")),
        &[HOST_RULE],
        &mut active_turn,
        &mut model,
        &NativeTurnServices {
            catalog: &catalog,
            registry: &registry,
            authority: &authority,
            host_policy: &Policy,
        },
        &Cancellation::default(),
        &audit,
        &effects,
        &|_| {},
    )
    .await;
    assert_eq!(
        result.unwrap_err(),
        NativeGenerationError::ModelReply(ToolTurnError::DurableDispatchFailed)
    );
    assert_eq!(model.calls, 0);
    assert_eq!(hits.load(Ordering::SeqCst), 0);
    assert!(effects.get(&expected_call_id()).unwrap().is_none());
    drop(effects);
    drop(audit);
    fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn no_default_dlc_routes_or_unreviewed_candidate_can_execute() {
    let dir = root("no-implicit-dlc");
    let audit = ModelTurnJournal::open(&dir.join("audit.sqlite")).unwrap();
    let effects = EffectJournal::open(&dir.join("effects.sqlite")).unwrap();
    let (registry, catalog, authority, origin, hits) = fixture(None);
    let mut active_turn = turn(&catalog, &authority, origin);
    let mut model = FinalModel {
        calls: 0,
        reply: r#"{"kind":"final","text":"no effect"}"#,
    };
    // The same signed sample fixture is registered and authorized, but its
    // route has NOT been separately reviewed/enrolled by the host.
    let result = run_supervised_native_turn_journaled_observed_with_host_routing(
        "Compute exactly: 9/14 + 5/21",
        "Compute exactly: 9/14 + 5/21",
        Some(&reviewed_for_current_user("Compute exactly: 9/14 + 5/21")),
        lu_core::capability_router::BUILTIN_DLC_ROUTES,
        &mut active_turn,
        &mut model,
        &NativeTurnServices {
            catalog: &catalog,
            registry: &registry,
            authority: &authority,
            host_policy: &Policy,
        },
        &Cancellation::default(),
        &audit,
        &effects,
        &|_| {},
    )
    .await;
    assert_eq!(
        result.unwrap_err(),
        NativeGenerationError::InvalidInferenceEvent
    );
    assert_eq!(model.calls, 0);
    assert_eq!(hits.load(Ordering::SeqCst), 0);
    assert!(effects.get(&expected_call_id()).unwrap().is_none());
    drop(effects);
    drop(audit);
    fs::remove_dir_all(dir).unwrap();
}
