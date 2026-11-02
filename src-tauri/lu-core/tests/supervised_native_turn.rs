//! The next integration gate: *supervisor-generated* model text, Protobuf
//! native dispatch and a second supervisor generation using the tool result.
//! Backend is deterministic for CI. This does not certify a real GGUF model.

use lu_core::{
    agent_tool_loop::{NativeToolTurn, ToolDispatchCheckpoint},
    contracts::{AuthorizationError, ToolAuthorizer, ToolDescriptor, ToolEffect, ToolInvocation},
    conversation_authority::{
        AuthorizedCapability, BoundCallOrigin, ConversationAuthority, TaskAuthority,
        TrustedUserTurnVerifier,
    },
    inference_host::{Cancellation, EngineError, GenerationRequest, InferenceBackend, TokenChunk},
    inference_supervisor::TrustedInferenceSupervisor,
    model_admission::{AdmittedModel, ModelAdmissionManager},
    model_turn_journal::{ModelAuditKind, ModelTurnJournal},
    native_file_extra::register_inspection_tools,
    native_files::register_bundled_file_tools,
    registry::NativeToolRegistry,
    supervised_tool_generation::{
        run_supervised_native_turn, run_supervised_native_turn_journaled,
        run_supervised_native_turn_journaled_observed, ModelStepGenerator, NativeGenerationError,
        NativeTurnRecord, NativeTurnServices, SupervisorModelStep,
    },
};
use std::{
    collections::BTreeSet,
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

struct Verified;
impl TrustedUserTurnVerifier for Verified {
    fn verify_user_turn(&self, conversation_id: &str, turn_id: &str) -> bool {
        conversation_id == "conversation-test" && turn_id == "user-test"
    }
}
struct Permitted;
impl ToolAuthorizer for Permitted {
    fn authorize(&self, _: &ToolInvocation, _: &ToolDescriptor) -> Result<(), AuthorizationError> {
        Ok(())
    }
}

/// A deterministic stand-in used ONLY to prove that the production inference
/// supervisor, wire decoding, real native registry and model continuation
/// execute in correct order. It is NOT a downloaded GGUF.
struct StreamFixture {
    prompts: Arc<Mutex<Vec<String>>>,
}
impl InferenceBackend for StreamFixture {
    fn load(&mut self, _: &AdmittedModel) -> Result<(), String> {
        Ok(())
    }
    fn unload(&mut self) {}
    fn generate(
        &mut self,
        request: &GenerationRequest,
        cancel: &Cancellation,
        emit: &mut dyn FnMut(TokenChunk) -> Result<(), EngineError>,
    ) -> Result<(), String> {
        if cancel.is_cancelled() {
            return Err("cancelled".into());
        }
        self.prompts.lock().unwrap().push(request.prompt.clone());
        let response = if request.prompt.contains("\"kind\":\"tool_result\"")
            && request.prompt.contains("Lu evidence marker")
        {
            r#"{"kind":"final","text":"I found Lu evidence marker in notes.txt."}"#
        } else {
            r#"{"kind":"tool","name":"lu_files_read_text","arguments":{"path":"notes.txt"}}"#
        };
        // Exercise split token/event boundaries, not an already assembled call.
        let split = response.len() / 2;
        for (index, part) in [&response[..split], &response[split..]]
            .into_iter()
            .enumerate()
        {
            emit(TokenChunk {
                text: part.into(),
                index: index as u32,
            })
            .map_err(|e| format!("{e:?}"))?;
        }
        Ok(())
    }
}

// Prevent parallel Windows tests from sharing a temp directory when the
// system clock reports identical timestamps for separate threads.
static NEXT_SUPERVISED_WORKSPACE_ID: AtomicU64 = AtomicU64::new(0);

fn sandbox() -> PathBuf {
    let dir = loop {
        let sequence = NEXT_SUPERVISED_WORKSPACE_ID.fetch_add(1, Ordering::Relaxed);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let candidate = std::env::temp_dir().join(format!(
            "lu-supervised-turn-{}-{now}-{sequence}",
            std::process::id()
        ));
        match fs::create_dir(&candidate) {
            Ok(()) => break candidate,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => panic!("cannot create isolated Lu supervisor test workspace: {error}"),
        }
    };
    fs::write(dir.join("notes.txt"), "Lu evidence marker\n").unwrap();
    let mut gguf = Vec::new();
    gguf.extend_from_slice(b"GGUF");
    gguf.extend_from_slice(&3_u32.to_le_bytes());
    gguf.extend_from_slice(&1_u64.to_le_bytes());
    gguf.extend_from_slice(&1_u64.to_le_bytes());
    fs::write(dir.join("test.gguf"), gguf).unwrap();
    dir
}

/// Windows can retain a short-lived handle after native tool shutdown.
/// Retry only the known sharing-violation condition; all other errors fail.
fn remove_workspace(root: PathBuf) {
    for attempt in 0..10 {
        match fs::remove_dir_all(&root) {
            Ok(()) => return,
            Err(error) if error.raw_os_error() == Some(32) && attempt < 9 => {
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(error) => panic!("cannot clean disposable Lu test workspace: {error}"),
        }
    }
}

fn tools_and_authority(
    root: &std::path::Path,
) -> (
    NativeToolRegistry,
    lu_core::model::ModelToolCatalog,
    ConversationAuthority,
    BoundCallOrigin,
) {
    let registry = NativeToolRegistry::new();
    let mut catalog = register_bundled_file_tools(&registry, root).unwrap();
    register_inspection_tools(&registry, &mut catalog, root).unwrap();
    let authority = ConversationAuthority::new();
    let origin = BoundCallOrigin {
        conversation_id: "conversation-test".into(),
        user_turn_id: "user-test".into(),
        task_id: "task-test".into(),
    };
    authority
        .open_user_task(
            TaskAuthority {
                conversation_id: origin.conversation_id.clone(),
                user_turn_id: origin.user_turn_id.clone(),
                task_id: origin.task_id.clone(),
                execution_scope_id: "thread:conversation-test".into(),
                capabilities: [
                    "lu.files.read_text",
                    "lu.files.list_directory",
                    "lu.inspect.stat",
                    "lu.inspect.search_text",
                ]
                .into_iter()
                .map(|name| AuthorizedCapability {
                    tool_id: name.into(),
                    effect: ToolEffect::ReadWorkspace,
                })
                .collect(),
                expires_at: None,
            },
            &Verified,
        )
        .unwrap();
    (registry, catalog, authority, origin)
}

#[tokio::test]
async fn model_output_from_supervisor_dispatches_real_protobuf_tool_and_continues() {
    let root = sandbox();
    let (registry, catalog, authority, origin) = tools_and_authority(&root);
    let allowed: BTreeSet<String> = [
        "lu_files_read_text",
        "lu_files_list_directory",
        "lu_files_stat",
        "lu_files_search_text",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    let mut turn = NativeToolTurn::new(
        &catalog,
        &authority,
        origin,
        "thread:conversation-test".into(),
        &allowed,
    )
    .unwrap();
    let model = ModelAdmissionManager::new();
    let mut model = model;
    let admitted = model.admit(&root, "test.gguf").unwrap().clone();
    let prompts = Arc::new(Mutex::new(Vec::<String>::new()));
    let observed = Arc::clone(&prompts);
    let supervisor = TrustedInferenceSupervisor::new(
        "test-backend".into(),
        512,
        vec![("model-test".into(), admitted)],
        move || {
            Ok(StreamFixture {
                prompts: Arc::clone(&observed),
            })
        },
    )
    .unwrap();
    supervisor
        .load("model-test", "s-test".into(), "load-test")
        .unwrap();
    let mut inference =
        SupervisorModelStep::new(&supervisor, "s-test".into(), "turn-test".into()).unwrap();
    let journal_path = root.join("audit.sqlite3");
    let journal = ModelTurnJournal::open(&journal_path).unwrap();
    let checkpoints = Arc::new(Mutex::new(Vec::new()));
    let observed_checkpoints = Arc::clone(&checkpoints);
    let observer = move |stage| {
        observed_checkpoints.lock().unwrap().push(stage);
    };
    let result = run_supervised_native_turn_journaled_observed(
        "Read notes.txt and report the marker.",
        &mut turn,
        &mut inference,
        &NativeTurnServices {
            catalog: &catalog,
            registry: &registry,
            authority: &authority,
            host_policy: &Permitted,
        },
        &Cancellation::default(),
        &journal,
        &observer,
    )
    .await
    .unwrap();
    assert_eq!(
        *checkpoints.lock().unwrap(),
        vec![
            ToolDispatchCheckpoint::Entered,
            ToolDispatchCheckpoint::Returned,
        ]
    );
    assert_eq!(result.tool_calls, 1);
    assert_eq!(result.model_generations, 2);
    assert!(result.answer.contains("Lu evidence marker"));
    drop(journal);
    let reloaded = ModelTurnJournal::open(&journal_path).unwrap();
    let evidence = reloaded
        .events_for("conversation-test", "task-test")
        .unwrap();
    assert_eq!(evidence.len(), 7);
    assert_eq!(evidence[0].kind, ModelAuditKind::UserRequest);
    assert_eq!(evidence[1].kind, ModelAuditKind::ModelOutput);
    assert_eq!(evidence[2].kind, ModelAuditKind::ParsedCandidate);
    assert_eq!(evidence[3].kind, ModelAuditKind::ToolFeedback);
    assert_eq!(evidence[6].kind, ModelAuditKind::FinalAnswer);
    drop(reloaded);
    assert!(matches!(
        &result.records[0],
        NativeTurnRecord::ModelOutput(_)
    ));
    assert!(matches!(
        &result.records[1],
        NativeTurnRecord::ToolFeedback(_)
    ));
    assert!(matches!(
        &result.records[3],
        NativeTurnRecord::FinalAnswer(_)
    ));
    let observed = prompts.lock().unwrap();
    assert_eq!(observed.len(), 2);
    assert!(!observed[0].contains("\"kind\":\"tool_result\""));
    assert!(observed[1].contains("\"kind\":\"tool_result\""));
    assert!(observed[1].contains("Lu evidence marker"));
    drop(observed);
    assert!(supervisor.status().unwrap().generation_id.is_none());
    supervisor.unload("s-test", "unload-test").unwrap();
    drop(inference);
    drop(turn);
    drop(registry);
    drop(catalog);
    drop(authority);
    remove_workspace(root);
}

#[tokio::test]
async fn unavailable_audit_journal_blocks_model_generation_and_tool_dispatch() {
    let root = sandbox();
    let (registry, catalog, authority, origin) = tools_and_authority(&root);
    let allowed = BTreeSet::from(["lu_files_read_text".into()]);
    let mut turn = NativeToolTurn::new(
        &catalog,
        &authority,
        origin,
        "thread:conversation-test".into(),
        &allowed,
    )
    .unwrap();
    let db = root.join("blocked-audit.sqlite3");
    let journal = ModelTurnJournal::open(&db).unwrap();
    // Force deterministic persistence failure, not an incidental file lock.
    let connection = rusqlite::Connection::open(&db).unwrap();
    connection
        .execute_batch(
            "CREATE TRIGGER lu_test_reject_audit BEFORE INSERT
             ON lu_model_boundary_audit
             BEGIN SELECT RAISE(ABORT, 'audit storage is unavailable'); END;",
        )
        .unwrap();
    struct MustNotGenerate;
    impl ModelStepGenerator for MustNotGenerate {
        fn generate_step(
            &mut self,
            _: &str,
            _: u32,
            _: &Cancellation,
        ) -> Result<String, NativeGenerationError> {
            panic!("model generation must not run without durable audit");
        }
    }
    assert_eq!(
        run_supervised_native_turn_journaled(
            "Read notes.txt",
            &mut turn,
            &mut MustNotGenerate,
            &NativeTurnServices {
                catalog: &catalog,
                registry: &registry,
                authority: &authority,
                host_policy: &Permitted,
            },
            &Cancellation::default(),
            &journal,
        )
        .await,
        Err(NativeGenerationError::JournalFailure),
    );
    assert_eq!(turn.step_count(), 0);
    drop(connection);
    drop(journal);
    drop(turn);
    drop(catalog);
    drop(registry);
    drop(authority);
    remove_workspace(root);
}

#[tokio::test]
async fn cancellation_before_generation_prevents_tool_execution() {
    let root = sandbox();
    let (registry, catalog, authority, origin) = tools_and_authority(&root);
    let allowed = BTreeSet::from(["lu_files_read_text".into()]);
    let mut turn = NativeToolTurn::new(
        &catalog,
        &authority,
        origin,
        "thread:conversation-test".into(),
        &allowed,
    )
    .unwrap();
    struct MustNotRun;
    impl ModelStepGenerator for MustNotRun {
        fn generate_step(
            &mut self,
            _: &str,
            _: u32,
            _: &Cancellation,
        ) -> Result<String, NativeGenerationError> {
            panic!("generation must be cancelled before model call")
        }
    }
    let cancelled = Cancellation::default();
    cancelled.cancel();
    assert_eq!(
        run_supervised_native_turn(
            "Read notes.txt",
            &mut turn,
            &mut MustNotRun,
            &NativeTurnServices {
                catalog: &catalog,
                registry: &registry,
                authority: &authority,
                host_policy: &Permitted
            },
            &cancelled
        )
        .await,
        Err(NativeGenerationError::Cancelled),
    );
    assert_eq!(turn.step_count(), 0);
    drop(turn);
    drop(registry);
    drop(catalog);
    drop(authority);
    remove_workspace(root);
}

#[tokio::test]
async fn model_cannot_provide_its_own_host_session_identifier() {
    let root = sandbox();
    let (registry, catalog, authority, origin) = tools_and_authority(&root);
    let allowed = BTreeSet::from(["lu_files_read_text".into()]);
    let mut turn = NativeToolTurn::new(
        &catalog,
        &authority,
        origin,
        "thread:conversation-test".into(),
        &allowed,
    )
    .unwrap();
    let journal_path = root.join("rejected-audit.sqlite3");
    let journal = ModelTurnJournal::open(&journal_path).unwrap();
    struct MaliciousReply;
    impl ModelStepGenerator for MaliciousReply {
        fn generate_step(
            &mut self,
            _: &str,
            _: u32,
            _: &Cancellation,
        ) -> Result<String, NativeGenerationError> {
            Ok(r#"{"kind":"tool","name":"lu_files_read_text","arguments":{"path":"notes.txt"},"session":"other"}"#.into())
        }
    }
    assert_eq!(
        run_supervised_native_turn_journaled(
            "Read notes.txt",
            &mut turn,
            &mut MaliciousReply,
            &NativeTurnServices {
                catalog: &catalog,
                registry: &registry,
                authority: &authority,
                host_policy: &Permitted
            },
            &Cancellation::default(),
            &journal,
        )
        .await,
        Err(NativeGenerationError::ModelReplyStructure(
            "model_tool_fields_rejected",
        )),
    );
    assert_eq!(turn.step_count(), 0);
    drop(journal);
    let persisted = ModelTurnJournal::open(&journal_path).unwrap();
    let evidence = persisted
        .events_for("conversation-test", "task-test")
        .unwrap();
    assert_eq!(evidence.len(), 4);
    assert_eq!(evidence[1].kind, ModelAuditKind::ModelOutput);
    assert_eq!(evidence[3].kind, ModelAuditKind::Rejected);
    assert!(evidence[3].payload.contains("UnexpectedFields"));
    drop(persisted);
    drop(turn);
    drop(registry);
    drop(catalog);
    drop(authority);
    remove_workspace(root);
}

#[tokio::test]
async fn invented_tool_is_rejected_then_corrected_without_native_dispatch() {
    let root = sandbox();
    let (registry, catalog, authority, origin) = tools_and_authority(&root);
    let allowed = BTreeSet::from(["lu_files_read_text".to_owned()]);
    let mut turn = NativeToolTurn::new(
        &catalog,
        &authority,
        origin,
        "thread:conversation-test".into(),
        &allowed,
    )
    .unwrap();

    struct UnknownThenFinal {
        prompts: Vec<String>,
    }
    impl ModelStepGenerator for UnknownThenFinal {
        fn generate_step(
            &mut self,
            prompt: &str,
            _: u32,
            _: &Cancellation,
        ) -> Result<String, NativeGenerationError> {
            self.prompts.push(prompt.to_owned());
            if self.prompts.len() == 1 {
                Ok(r#"{"kind":"tool","name":"lu_invented_tool","arguments":{}}"#.into())
            } else {
                Ok(r#"{"kind":"final","text":"Hello from Lu."}"#.into())
            }
        }
    }

    let mut model = UnknownThenFinal {
        prompts: Vec::new(),
    };
    let journal = ModelTurnJournal::open(&root.join("unknown-tool.sqlite3")).unwrap();
    let report = run_supervised_native_turn_journaled(
        "Say hello.",
        &mut turn,
        &mut model,
        &NativeTurnServices {
            catalog: &catalog,
            registry: &registry,
            authority: &authority,
            host_policy: &Permitted,
        },
        &Cancellation::default(),
        &journal,
    )
    .await
    .unwrap();
    assert_eq!(report.answer, "Hello from Lu.");
    assert_eq!(report.model_generations, 2);
    assert_eq!(report.tool_calls, 0);
    assert_eq!(turn.step_count(), 0);
    assert_eq!(model.prompts.len(), 2);
    assert!(model.prompts[1].contains("NOT executed"));
    assert!(!model.prompts[1].contains("lu_invented_tool"));
    let audit = journal
        .events_for("conversation-test", "task-test")
        .unwrap();
    assert_eq!(audit.len(), 7);
    assert_eq!(audit[3].kind, ModelAuditKind::Rejected);
    assert_eq!(audit[6].kind, ModelAuditKind::FinalAnswer);
    assert!(!audit.iter().any(|r| r.kind == ModelAuditKind::ToolFeedback));
    drop(journal);
    drop(turn);
    drop(catalog);
    drop(registry);
    drop(authority);
    remove_workspace(root);
}

#[tokio::test]
async fn invented_tool_twice_fails_closed_without_native_dispatch() {
    let root = sandbox();
    let (registry, catalog, authority, origin) = tools_and_authority(&root);
    let allowed = BTreeSet::from(["lu_files_read_text".to_owned()]);
    let mut turn = NativeToolTurn::new(
        &catalog,
        &authority,
        origin,
        "thread:conversation-test".into(),
        &allowed,
    )
    .unwrap();

    struct AlwaysInvent {
        calls: usize,
    }
    impl ModelStepGenerator for AlwaysInvent {
        fn generate_step(
            &mut self,
            _: &str,
            _: u32,
            _: &Cancellation,
        ) -> Result<String, NativeGenerationError> {
            self.calls += 1;
            Ok(r#"{"kind":"tool","name":"lu_invented_tool","arguments":{}}"#.into())
        }
    }
    let mut model = AlwaysInvent { calls: 0 };
    let journal = ModelTurnJournal::open(&root.join("reject-twice.sqlite3")).unwrap();
    let outcome = run_supervised_native_turn_journaled(
        "Say hello.",
        &mut turn,
        &mut model,
        &NativeTurnServices {
            catalog: &catalog,
            registry: &registry,
            authority: &authority,
            host_policy: &Permitted,
        },
        &Cancellation::default(),
        &journal,
    )
    .await;
    assert!(matches!(
        outcome,
        Err(NativeGenerationError::ModelReply(
            lu_core::agent_tool_loop::ToolTurnError::Catalog(
                lu_core::model::ModelToolError::UnknownTool(_)
            )
        ))
    ));
    assert_eq!(model.calls, 2);
    assert_eq!(turn.step_count(), 0);
    let audit = journal
        .events_for("conversation-test", "task-test")
        .unwrap();
    assert_eq!(
        audit
            .iter()
            .filter(|row| row.kind == ModelAuditKind::Rejected)
            .count(),
        2
    );
    assert!(!audit.iter().any(|r| r.kind == ModelAuditKind::ToolFeedback));
    drop(journal);
    drop(turn);
    drop(catalog);
    drop(registry);
    drop(authority);
    remove_workspace(root);
}
