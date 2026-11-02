//! Opt-in real GGUF acceptance gate. Unlike supervised_native_turn, no
//! deterministic/mock model replies are supplied. A checksum-verified
//! llama-server must generate the actual tool JSON through Lu's persistent
//! inference backend, then consume the real Protobuf file-tool result.
//!
//! Execute with:
//!   LU_REAL_GGUF_PATH=<approved .gguf>
//!   LU_REAL_GGUF_SHA256=<64 hex chars>
//!   LU_REAL_LLAMA_SERVER_PATH=<trusted llama-server executable>
//!   LU_REAL_LLAMA_SERVER_SHA256=<64 hex chars>
//!   LU_REAL_LLAMA_API_KEY=<trusted, random 64 hex chars>
//! cargo test --manifest-path src-tauri/lu-core/Cargo.toml --test real_gguf_tool_json -- --ignored --nocapture
//!
//! No assumption that a specific model or quantization will follow ChatML
//! JSON instructions. Invalid output fails acceptance, rather than being
//! replaced with a canned tool call or silently repaired.

use lu_core::{
    agent_tool_loop::NativeToolTurn,
    contracts::{AuthorizationError, ToolAuthorizer, ToolDescriptor, ToolEffect, ToolInvocation},
    conversation_authority::{
        AuthorizedCapability, BoundCallOrigin, ConversationAuthority, TaskAuthority,
        TrustedUserTurnVerifier,
    },
    inference_host::{Cancellation, EnginePhase},
    inference_supervisor::TrustedInferenceSupervisor,
    model_admission::ModelAdmissionManager,
    model_json_boundary::parse_model_boundary,
    model_turn_journal::{ModelAuditKind, ModelTurnJournal},
    native_files::register_bundled_file_tools,
    persistent_llama_backend::{PersistentEngineConfig, PersistentLlamaBackend},
    registry::NativeToolRegistry,
    supervised_tool_generation::{
        run_supervised_native_turn_journaled, NativeTurnRecord, NativeTurnServices,
        SupervisorModelStep,
    },
};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    env, fs,
    io::{Read, Result as IoResult},
    num::NonZeroUsize,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

struct TrustedUserTurn;
impl TrustedUserTurnVerifier for TrustedUserTurn {
    fn verify_user_turn(&self, conversation_id: &str, user_turn_id: &str) -> bool {
        conversation_id == "lu-real-gguf-test" && user_turn_id == "test-user-turn"
    }
}
struct ReadOnlyPolicy;
impl ToolAuthorizer for ReadOnlyPolicy {
    fn authorize(&self, _: &ToolInvocation, d: &ToolDescriptor) -> Result<(), AuthorizationError> {
        if d.id == "lu.files.read_text" && d.effect == ToolEffect::ReadWorkspace {
            Ok(())
        } else {
            Err(AuthorizationError::new(
                "test does not authorize this effect",
            ))
        }
    }
}

fn required_env(key: &str) -> String {
    env::var(key).unwrap_or_else(|_| panic!("real GGUF test requires {key}"))
}

fn read_digest(value: &str) -> [u8; 32] {
    assert_eq!(value.len(), 64, "expected a SHA-256 hexadecimal digest");
    let mut bytes = [0_u8; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
            .expect("invalid SHA-256 hex byte");
    }
    assert!(
        bytes.iter().any(|byte| *byte != 0),
        "empty fingerprint is forbidden"
    );
    bytes
}

fn calculate_sha256(path: &Path) -> IoResult<[u8; 32]> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finalize().into())
}

struct EphemeralWorkspace(PathBuf);
impl Drop for EphemeralWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[tokio::test]
#[ignore = "requires a verified real GGUF model and a trusted Windows/Linux llama-server"]
async fn gguf_model_generates_tool_json_reads_file_and_continues() {
    let model = PathBuf::from(required_env("LU_REAL_GGUF_PATH"))
        .canonicalize()
        .expect("GGUF model path invalid");
    let engine = PathBuf::from(required_env("LU_REAL_LLAMA_SERVER_PATH"))
        .canonicalize()
        .expect("llama-server path invalid");
    let expected_model = read_digest(&required_env("LU_REAL_GGUF_SHA256"));
    let expected_engine = read_digest(&required_env("LU_REAL_LLAMA_SERVER_SHA256"));
    let api_key = required_env("LU_REAL_LLAMA_API_KEY");
    assert_eq!(
        api_key.len(),
        64,
        "trusted test host must mint a 256-bit key"
    );
    assert!(api_key.bytes().all(|b| b.is_ascii_hexdigit()));
    assert_eq!(
        calculate_sha256(&model).expect("failed to hash real model"),
        expected_model,
        "GGUF fingerprint mismatch"
    );
    assert_eq!(
        calculate_sha256(&engine).expect("failed to hash real engine"),
        expected_engine,
        "llama-server fingerprint mismatch"
    );
    assert!(
        fs::metadata(&model).unwrap().len() > 1_000_000,
        "tiny synthetic GGUF fixtures are forbidden"
    );

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let workspace = EphemeralWorkspace(
        env::temp_dir().join(format!("lu-gguf-tool-json-{}-{nonce}", std::process::id())),
    );
    fs::create_dir(&workspace.0).expect("create isolated workspace");
    let marker = format!("LU_FILE_PROOF_{nonce}");
    fs::write(workspace.0.join("notes.txt"), format!("{marker}\n"))
        .expect("write tool-only evidence");

    let registry = NativeToolRegistry::new();
    let catalog =
        register_bundled_file_tools(&registry, &workspace.0).expect("register real Protobuf tools");
    let authority = ConversationAuthority::new();
    let origin = BoundCallOrigin {
        conversation_id: "lu-real-gguf-test".into(),
        user_turn_id: "test-user-turn".into(),
        task_id: "test-host-task".into(),
    };
    authority
        .open_user_task(
            TaskAuthority {
                conversation_id: origin.conversation_id.clone(),
                user_turn_id: origin.user_turn_id.clone(),
                task_id: origin.task_id.clone(),
                execution_scope_id: "thread:lu-real-gguf-test".into(),
                capabilities: vec![AuthorizedCapability {
                    tool_id: "lu.files.read_text".into(),
                    effect: ToolEffect::ReadWorkspace,
                }],
                expires_at: None,
            },
            &TrustedUserTurn,
        )
        .expect("host must grant the exact read-only capability");
    let approved = BTreeSet::from(["lu_files_read_text".into()]);
    let mut turn = NativeToolTurn::new(
        &catalog,
        &authority,
        origin,
        "thread:lu-real-gguf-test".into(),
        &approved,
    )
    .expect("host must advertise only the approved tool");

    let mut admission = ModelAdmissionManager::new();
    let model_dir = model.parent().expect("GGUF must have a parent directory");
    let filename = model
        .file_name()
        .and_then(|n| n.to_str())
        .expect("GGUF filename is not UTF-8");
    let admitted = admission
        .admit(model_dir, filename)
        .expect("real GGUF header admission failed")
        .clone();
    let supervisor = TrustedInferenceSupervisor::new(
        "llama-cpu".into(),
        2048,
        vec![("verified-model".into(), admitted)],
        move || {
            PersistentLlamaBackend::new(PersistentEngineConfig {
                executable: engine.clone(),
                expected_sha256: expected_engine,
                threads: NonZeroUsize::new(2).expect("nonzero threads"),
                context_tokens: 2048,
                startup_timeout: Duration::from_secs(120),
                request_timeout: Duration::from_secs(120),
                api_key: api_key.clone(),
            })
        },
    )
    .expect("trusted GGUF supervisor setup failed");
    supervisor
        .load(
            "verified-model",
            "session-real-test".into(),
            "load-real-test",
        )
        .expect("pinned real GGUF engine failed to load");
    let status = supervisor.status().expect("engine state missing");
    assert_eq!(status.phase, EnginePhase::Ready);
    assert!(status.pid.is_some(), "no real llama-server worker PID");

    let mut live = SupervisorModelStep::new(
        &supervisor,
        "session-real-test".into(),
        "turn-real-test".into(),
    )
    .expect("invalid trusted inference session");
    // The evidence marker is NOT in this prompt. It is available only through
    // the authorized native tool. The model must independently emit the JSON.
    let journal_path = workspace.0.join("model-audit.sqlite3");
    let journal = ModelTurnJournal::open(&journal_path).expect("durable Lu boundary journal");
    let outcome = run_supervised_native_turn_journaled(
        "Read notes.txt using the advertised lu_files_read_text tool. \
         First respond with ONLY a JSON tool request, not Python, Markdown or prose. \
         Do not invent or guess the file content. Once the tool result is provided, \
         respond with ONLY a JSON final response containing the exact file marker.",
        &mut turn,
        &mut live,
        &NativeTurnServices {
            catalog: &catalog,
            registry: &registry,
            authority: &authority,
            host_policy: &ReadOnlyPolicy,
        },
        &Cancellation::default(),
        &journal,
    )
    .await;
    // Preserve reproducible, bounded diagnostics of actual generation.
    // Debug formatting escapes controls and avoids unbounded CI output.
    if outcome.is_err() {
        if let Ok(events) = journal.events_for("lu-real-gguf-test", "test-host-task") {
            for (index, event) in events
                .iter()
                .filter(|event| event.kind == ModelAuditKind::ModelOutput)
                .enumerate()
            {
                let preview: String = event.payload.chars().take(1024).collect();
                let digest = Sha256::digest(event.payload.as_bytes());
                eprintln!(
                    "LU_REAL_GGUF_MODEL_FRAME index={index} bytes={} sha256={digest:x} escaped_preview={preview:?}",
                    event.payload.len()
                );
            }
        }
    }
    let report = outcome.expect("real GGUF failed to issue valid JSON, invoke tool, and continue");
    if report.model_generations != 2 || report.tool_calls != 1 {
        eprintln!(
            "LU_REAL_GGUF_DIAG generations={} tool_calls={} answer_preview={:?}",
            report.model_generations,
            report.tool_calls,
            report.answer.chars().take(500).collect::<String>()
        );
        for (index, record) in report.records.iter().enumerate() {
            eprintln!(
                "LU_REAL_GGUF_DIAG record[{index}]={:?}",
                format!("{record:?}").chars().take(700).collect::<String>()
            );
        }
    }
    assert_eq!(
        report.model_generations, 2,
        "expected one tool call then a final answer"
    );
    assert_eq!(
        report.tool_calls, 1,
        "model did not request exactly one tool"
    );
    assert_eq!(
        report.records.len(),
        4,
        "missing model/tool/provenance turn records"
    );

    let NativeTurnRecord::ModelOutput(raw) = &report.records[0] else {
        panic!("first model turn was not real inference output");
    };
    let parsed = parse_model_boundary(raw).expect("real model JSON rejected at Lu boundary");
    assert_eq!(parsed.object["kind"], "tool");
    assert_eq!(parsed.object["name"], "lu_files_read_text");
    assert_eq!(parsed.object["arguments"]["path"], "notes.txt");

    let NativeTurnRecord::ToolFeedback(reply) = &report.records[1] else {
        panic!("real model call did not enter Lu native Protobuf dispatch");
    };
    assert_eq!(reply["name"], "lu_files_read_text");
    assert_eq!(reply["response"]["ok"], true, "native tool did not succeed");
    assert_eq!(
        reply["response"]["result"]["content"],
        format!("{marker}\n")
    );
    assert!(
        report.answer.contains(&marker),
        "real GGUF continuation failed to use native tool evidence"
    );
    assert!(matches!(
        &report.records[3],
        NativeTurnRecord::FinalAnswer(_)
    ));
    drop(journal);
    let restored = ModelTurnJournal::open(&journal_path).expect("reopen durable boundary journal");
    let events = restored
        .events_for("lu-real-gguf-test", "test-host-task")
        .unwrap();
    assert_eq!(
        events.len(),
        7,
        "incomplete durable model/tool/final journal"
    );
    assert_eq!(events[0].kind, ModelAuditKind::UserRequest);
    assert_eq!(events[1].kind, ModelAuditKind::ModelOutput);
    assert_eq!(events[3].kind, ModelAuditKind::ToolFeedback);
    assert_eq!(events[6].kind, ModelAuditKind::FinalAnswer);
    assert!(events[6].payload.contains(&marker));
    drop(restored);
    assert_eq!(
        supervisor.status().unwrap().phase,
        EnginePhase::Ready,
        "model worker did not recover to Ready after tool continuation"
    );
    supervisor
        .unload("session-real-test", "unload-real-test")
        .expect("real GGUF worker did not unload cleanly");

    println!(
        "LU_REAL_GGUF_NATIVE_TOOL_JSON_PASS generations={} tools={} worker_recovered=true",
        report.model_generations, report.tool_calls
    );
}
