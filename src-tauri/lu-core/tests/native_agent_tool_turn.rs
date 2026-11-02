use lu_core::{
    agent_tool_loop::{NativeToolTurn, ToolTurnError, ToolTurnOutcome, MAX_TOOL_STEPS},
    contracts::{AuthorizationError, ToolAuthorizer, ToolDescriptor, ToolEffect, ToolInvocation},
    conversation_authority::{
        AuthorizedCapability, BoundCallOrigin, ConversationAuthority, TaskAuthority,
        TrustedUserTurnVerifier,
    },
    model::{ModelToolCatalog, ModelToolError},
    model_json_boundary::JsonRepairStatus,
    native_file_extra::register_inspection_tools,
    native_files::register_bundled_file_tools,
    registry::NativeToolRegistry,
};
use std::{
    collections::BTreeSet,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

struct Verified;
impl TrustedUserTurnVerifier for Verified {
    fn verify_user_turn(&self, conversation_id: &str, user_turn_id: &str) -> bool {
        conversation_id == "conversation-1" && user_turn_id == "user-turn-1"
    }
}
struct Permitted;
impl ToolAuthorizer for Permitted {
    fn authorize(&self, _: &ToolInvocation, _: &ToolDescriptor) -> Result<(), AuthorizationError> {
        Ok(())
    }
}

// Clock precision on Windows can be much lower than nanoseconds. Parallel
// tests must never share a workspace, even if system timestamps match.
static NEXT_WORKSPACE_ID: AtomicU64 = AtomicU64::new(0);

fn fresh_workspace() -> PathBuf {
    loop {
        let sequence = NEXT_WORKSPACE_ID.fetch_add(1, Ordering::Relaxed);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let folder = std::env::temp_dir().join(format!(
            "lu-native-turn-{}-{now}-{sequence}",
            std::process::id()
        ));
        match fs::create_dir(&folder) {
            Ok(()) => return folder,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => panic!("cannot create isolated Lu test workspace: {error}"),
        }
    }
}

fn remove_workspace(folder: PathBuf) {
    // Native read handles can linger briefly on Windows. Sharing violations
    // are retryable at teardown, but unexpected filesystem errors are not.
    for attempt in 0..10 {
        match fs::remove_dir_all(&folder) {
            Ok(()) => return,
            Err(error) if error.raw_os_error() == Some(32) && attempt < 9 => {
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(error) => panic!("cannot remove isolated Lu test workspace: {error}"),
        }
    }
}

#[test]
fn parallel_workspace_allocations_remain_distinct() {
    let threads: Vec<_> = (0..32)
        .map(|_| std::thread::spawn(fresh_workspace))
        .collect();
    let folders: Vec<_> = threads
        .into_iter()
        .map(|thread| thread.join().unwrap())
        .collect();
    let unique: BTreeSet<_> = folders.iter().collect();
    assert_eq!(unique.len(), folders.len());
    for folder in folders {
        remove_workspace(folder);
    }
}

fn setup() -> (
    PathBuf,
    NativeToolRegistry,
    ModelToolCatalog,
    ConversationAuthority,
    BoundCallOrigin,
    BTreeSet<String>,
) {
    let folder = fresh_workspace();
    fs::write(
        folder.join("notes.txt"),
        "first\nLu discovery marker\nlast\n",
    )
    .unwrap();
    fs::create_dir(folder.join("subdir")).unwrap();
    let registry = NativeToolRegistry::new();
    let mut catalog = register_bundled_file_tools(&registry, &folder).unwrap();
    register_inspection_tools(&registry, &mut catalog, &folder).unwrap();

    let authority = ConversationAuthority::new();
    let origin = BoundCallOrigin {
        conversation_id: "conversation-1".into(),
        user_turn_id: "user-turn-1".into(),
        task_id: "task-1".into(),
    };
    authority
        .open_user_task(
            TaskAuthority {
                conversation_id: origin.conversation_id.clone(),
                user_turn_id: origin.user_turn_id.clone(),
                task_id: origin.task_id.clone(),
                execution_scope_id: "thread:conversation-1".into(),
                capabilities: [
                    "lu.files.read_text",
                    "lu.files.list_directory",
                    "lu.inspect.stat",
                    "lu.inspect.search_text",
                ]
                .into_iter()
                .map(|tool_id| AuthorizedCapability {
                    tool_id: tool_id.into(),
                    effect: ToolEffect::ReadWorkspace,
                })
                .collect(),
                expires_at: None,
            },
            &Verified,
        )
        .unwrap();
    let allowed = BTreeSet::from([
        "lu_files_read_text".into(),
        "lu_files_list_directory".into(),
        "lu_files_stat".into(),
        "lu_files_search_text".into(),
    ]);
    (folder, registry, catalog, authority, origin, allowed)
}

#[tokio::test]
async fn plain_chat_reply_is_final_data_without_relaxing_tool_json() {
    let (folder, registry, catalog, authority, origin, allowed) = setup();
    let mut turn = NativeToolTurn::new_read_only(
        &catalog,
        &registry,
        &authority,
        origin,
        "thread:conversation-1".into(),
        &allowed,
    )
    .unwrap();

    for malformed in [
        r#"{"kind":"tool","name":"lu_files_read_text","arguments":{ "#,
        "```json\n{bad tool call}\n```",
        "<|im_start|>assistant\nhello",
        "\"quoted JSON string\"",
    ] {
        assert_eq!(
            turn.handle_model_reply(malformed, &catalog, &registry, &authority, &Permitted)
                .await,
            Err(ToolTurnError::MalformedModelReply),
        );
    }
    assert_eq!(
        turn.step_count(),
        0,
        "invalid output cannot dispatch a tool"
    );
    let reply = turn
        .handle_model_reply(
            "Hello from Lu.",
            &catalog,
            &registry,
            &authority,
            &Permitted,
        )
        .await
        .unwrap();
    assert_eq!(reply, ToolTurnOutcome::FinalText("Hello from Lu.".into()));
    assert_eq!(turn.step_count(), 0);

    drop(turn);
    drop(catalog);
    drop(registry);
    remove_workspace(folder);
}

#[tokio::test]
async fn lu_integer_chat_reply_is_display_only_without_json_tool_relaxation() {
    let (folder, registry, catalog, authority, origin, allowed) = setup();
    let mut turn = NativeToolTurn::new_read_only(
        &catalog,
        &registry,
        &authority,
        origin,
        "thread:conversation-1".into(),
        &allowed,
    )
    .unwrap();
    for invalid in ["null", "false", "true", "1.25", "-4", "\"4\"", "[4]"] {
        assert_eq!(
            turn.handle_model_reply(invalid, &catalog, &registry, &authority, &Permitted)
                .await,
            Err(ToolTurnError::MalformedModelReply),
        );
    }
    assert_eq!(turn.step_count(), 0);
    let result = turn
        .handle_model_reply(" 4 ", &catalog, &registry, &authority, &Permitted)
        .await
        .unwrap();
    assert_eq!(result, ToolTurnOutcome::FinalText("4".into()));
    assert_eq!(
        turn.step_count(),
        0,
        "numeric answer must never call a tool"
    );
    drop(turn);
    drop(catalog);
    drop(registry);
    remove_workspace(folder);
}

#[tokio::test]
async fn minimal_json_final_is_display_only_and_never_dispatches_a_tool() {
    for (payload, answer) in [
        (r#"{"text":"Hello from Lu"}"#, "Hello from Lu"),
        (r#"{"answer":"Lu is here"}"#, "Lu is here"),
        (r#"{"response":"Ready"}"#, "Ready"),
    ] {
        let (folder, registry, catalog, authority, origin, allowed) = setup();
        let mut turn = NativeToolTurn::new_read_only(
            &catalog,
            &registry,
            &authority,
            origin,
            "thread:conversation-1".into(),
            &allowed,
        )
        .unwrap();
        assert_eq!(
            turn.handle_model_reply(payload, &catalog, &registry, &authority, &Permitted)
                .await,
            Ok(ToolTurnOutcome::FinalText(answer.into())),
        );
        assert_eq!(turn.step_count(), 0, "a JSON final cannot execute a tool");
        assert_eq!(
            turn.handle_model_reply(payload, &catalog, &registry, &authority, &Permitted)
                .await,
            Err(ToolTurnError::AlreadyFinished),
        );
        drop(turn);
        drop(catalog);
        drop(registry);
        remove_workspace(folder);
    }
}

#[tokio::test]
async fn minimal_final_alias_rejects_tool_fields_unknown_kinds_and_duplicate_keys() {
    let (folder, registry, catalog, authority, origin, allowed) = setup();
    let mut turn = NativeToolTurn::new_read_only(
        &catalog,
        &registry,
        &authority,
        origin,
        "thread:conversation-1".into(),
        &allowed,
    )
    .unwrap();
    for (payload, expected) in [
        (
            r#"{"text":"hi","name":"lu_files_read_text","arguments":{"path":"notes.txt"}}"#,
            ToolTurnError::MalformedModelReply,
        ),
        (
            r#"{"kind":"tool","text":"hi"}"#,
            ToolTurnError::UnexpectedFields,
        ),
        (
            r#"{"kind":"final_answer","text":"hi"}"#,
            ToolTurnError::MalformedModelReply,
        ),
        (
            r#"{"response":{"text":"hi"}}"#,
            ToolTurnError::MalformedModelReply,
        ),
        (r#"{"answer":""}"#, ToolTurnError::MalformedModelReply),
        (
            r#"{"text":"first","text":"duplicate"}"#,
            ToolTurnError::MalformedModelReply,
        ),
    ] {
        assert_eq!(
            turn.handle_model_reply(payload, &catalog, &registry, &authority, &Permitted)
                .await,
            Err(expected),
        );
    }
    assert_eq!(turn.step_count(), 0);
    drop(turn);
    drop(catalog);
    drop(registry);
    remove_workspace(folder);
}

#[tokio::test]
async fn minimal_json_final_cannot_bypass_revocation() {
    let (folder, registry, catalog, authority, origin, allowed) = setup();
    let mut turn = NativeToolTurn::new_read_only(
        &catalog,
        &registry,
        &authority,
        origin,
        "thread:conversation-1".into(),
        &allowed,
    )
    .unwrap();
    authority.revoke_task("task-1").unwrap();
    assert_eq!(
        turn.handle_model_reply(
            r#"{"answer":"the model does not grant permission"}"#,
            &catalog,
            &registry,
            &authority,
            &Permitted,
        )
        .await,
        Err(ToolTurnError::Revoked)
    );
    assert_eq!(turn.step_count(), 0);
    drop(turn);
    drop(catalog);
    drop(registry);
    remove_workspace(folder);
}

#[tokio::test]
async fn multi_tool_turn_executes_four_typed_native_tools_then_finishes() {
    let (folder, registry, catalog, authority, origin, allowed) = setup();
    let mut turn = NativeToolTurn::new_read_only(
        &catalog,
        &registry,
        &authority,
        origin,
        "thread:conversation-1".into(),
        &allowed,
    )
    .unwrap();
    assert_eq!(turn.schemas(&catalog).unwrap().len(), 4);
    let commands = [
        r#"{"kind":"tool","name":"lu_files_list_directory","arguments":{}}"#,
        r#"{"kind":"tool","name":"lu_files_stat","arguments":{"path":"notes.txt"}}"#,
        r#"{"kind":"tool","name":"lu_files_search_text","arguments":{"query":"Lu discovery marker"}}"#,
        r#"{"kind":"tool","name":"lu_files_read_text","arguments":{"path":"notes.txt"}}"#,
    ];
    for (index, command) in commands.into_iter().enumerate() {
        let result = turn
            .handle_model_reply(command, &catalog, &registry, &authority, &Permitted)
            .await
            .unwrap();
        let ToolTurnOutcome::ToolResult { data, .. } = &result else {
            panic!("step {index} did not dispatch native tool");
        };
        assert_eq!(data["ok"], true, "step {index} failed: {data}");
        assert!(NativeToolTurn::model_feedback(&result)
            .unwrap()
            .contains("tool_result"));
        if index == 1 {
            assert_eq!(data["result"]["kind"], "file");
        }
        if index == 2 {
            assert_eq!(data["result"]["hits"][0]["line_number"], 2);
            assert_eq!(data["result"]["hits"][0]["path"], "notes.txt");
        }
    }
    assert_eq!(turn.step_count(), 4);
    let end = turn
        .handle_model_reply(
            r#"{"kind":"final","text":"I found the requested marker in notes.txt."}"#,
            &catalog,
            &registry,
            &authority,
            &Permitted,
        )
        .await
        .unwrap();
    assert_eq!(
        end,
        ToolTurnOutcome::FinalText("I found the requested marker in notes.txt.".into())
    );
    assert_eq!(
        turn.handle_model_reply(
            r#"{"kind":"final","text":"again"}"#,
            &catalog,
            &registry,
            &authority,
            &Permitted
        )
        .await,
        Err(ToolTurnError::AlreadyFinished)
    );
    drop(turn);
    drop(catalog);
    drop(registry);
    remove_workspace(folder);
}

#[tokio::test]
async fn model_cannot_expand_grants_or_inject_a_workspace() {
    let (folder, registry, catalog, authority, origin, allowed) = setup();
    let mut turn = NativeToolTurn::new(
        &catalog,
        &authority,
        origin,
        "thread:conversation-1".into(),
        &allowed,
    )
    .unwrap();
    for untrusted in [
        r#"{"kind":"tool","name":"shell","arguments":{}}"#,
        r#"{"kind":"tool","name":"lu_files_read_text","arguments":{"path":"../secret"}}"#,
        r#"{"kind":"tool","name":"lu_files_read_text","arguments":{"path":"notes.txt"},"scope":"other"}"#,
        r#"{"kind":"tool","name":"lu_files_stat","arguments":{"path":"notes.txt","extra":1}}"#,
        r#"{"kind":"tool","name":"lu_files_search_text","arguments":{"query":"a","max_files":999999}}"#,
    ] {
        assert!(
            turn.handle_model_reply(untrusted, &catalog, &registry, &authority, &Permitted,)
                .await
                .is_err(),
            "accepted {untrusted}"
        );
    }
    // Bad model outputs do not consume a call slot.
    assert_eq!(turn.step_count(), 0);
    authority.revoke_task("task-1").unwrap();
    let after = turn
        .handle_model_reply(
            r#"{"kind":"tool","name":"lu_files_read_text","arguments":{"path":"notes.txt"}}"#,
            &catalog,
            &registry,
            &authority,
            &Permitted,
        )
        .await
        .unwrap();
    let ToolTurnOutcome::ToolResult { data, .. } = after else {
        panic!("expected error result")
    };
    assert_eq!(data["ok"], false);
    assert_eq!(data["error"]["code"], "PERMISSION_DENIED");
    assert_eq!(
        turn.handle_model_reply(
            r#"{"kind":"final","text":"done"}"#,
            &catalog,
            &registry,
            &authority,
            &Permitted,
        )
        .await,
        Err(ToolTurnError::Revoked)
    );
    drop(turn);
    drop(catalog);
    drop(registry);
    remove_workspace(folder);
}

#[tokio::test]
async fn model_step_budget_is_bounded_and_authorized_names_are_selected_by_host() {
    let (folder, registry, catalog, authority, origin, allowed) = setup();
    let reduced = BTreeSet::from(["lu_files_stat".into()]);
    let mut turn = NativeToolTurn::new(
        &catalog,
        &authority,
        origin,
        "thread:conversation-1".into(),
        &reduced,
    )
    .unwrap();
    assert_eq!(turn.schemas(&catalog).unwrap().len(), 1);
    assert_eq!(
        turn.handle_model_reply(
            r#"{"kind":"tool","name":"lu_files_read_text","arguments":{"path":"notes.txt"}}"#,
            &catalog,
            &registry,
            &authority,
            &Permitted,
        )
        .await,
        Err(ToolTurnError::Catalog(ModelToolError::UnknownTool(
            "lu_files_read_text".into()
        )))
    );
    for _ in 0..MAX_TOOL_STEPS {
        let out = turn
            .handle_model_reply(
                r#"{"kind":"tool","name":"lu_files_stat","arguments":{"path":"notes.txt"}}"#,
                &catalog,
                &registry,
                &authority,
                &Permitted,
            )
            .await
            .unwrap();
        assert!(matches!(out, ToolTurnOutcome::ToolResult { .. }));
    }
    assert_eq!(
        turn.handle_model_reply(
            r#"{"kind":"tool","name":"lu_files_stat","arguments":{"path":"notes.txt"}}"#,
            &catalog,
            &registry,
            &authority,
            &Permitted,
        )
        .await,
        Err(ToolTurnError::StepLimit)
    );
    // Keep compile-time reference to the full catalog needed for larger lists.
    assert_eq!(allowed.len(), 4);
    drop(turn);
    drop(catalog);
    drop(registry);
    remove_workspace(folder);
}

#[tokio::test]
async fn text_search_caps_directory_entries_even_when_all_are_directories() {
    let (folder, registry, catalog, authority, origin, _) = setup();
    for i in 0..300 {
        fs::create_dir(folder.join(format!("folder-{i:03}"))).unwrap();
    }
    let allowed = BTreeSet::from(["lu_files_search_text".into()]);
    let mut turn = NativeToolTurn::new(
        &catalog,
        &authority,
        origin,
        "thread:conversation-1".into(),
        &allowed,
    )
    .unwrap();
    let result = turn
        .handle_model_reply(
            r#"{"kind":"tool","name":"lu_files_search_text","arguments":{"query":"nothing here"}}"#,
            &catalog,
            &registry,
            &authority,
            &Permitted,
        )
        .await
        .unwrap();
    let ToolTurnOutcome::ToolResult { data, .. } = result else {
        panic!("search was not dispatched");
    };
    assert_eq!(data["ok"], true);
    assert_eq!(data["result"]["truncated"], true);
    assert!(data["result"]["scanned_files"].as_u64().unwrap() <= 1);
    drop(turn);
    drop(catalog);
    drop(registry);
    remove_workspace(folder);
}

#[tokio::test]
async fn repaired_syntax_still_requires_schema_validation_and_authority() {
    let (folder, registry, catalog, authority, origin, _) = setup();
    let advertised = BTreeSet::from(["lu_files_read_text".into()]);
    let mut turn = NativeToolTurn::new(
        &catalog,
        &authority,
        origin,
        "thread:conversation-1".into(),
        &advertised,
    )
    .unwrap();

    // A trailing comma is punctuation only. The *real* Protobuf-native tool
    // must still be dispatched via the authorized read-only workspace.
    let result = turn
        .handle_model_reply(
            r#"{"kind":"tool","name":"lu_files_read_text","arguments":{"path":"notes.txt",},}"#,
            &catalog,
            &registry,
            &authority,
            &Permitted,
        )
        .await
        .unwrap();
    assert_eq!(
        turn.last_json_repair(),
        JsonRepairStatus::RemovedTrailingCommas
    );
    let ToolTurnOutcome::ToolResult { data, .. } = &result else {
        panic!("repair did not dispatch through native Protobuf tool");
    };
    assert_eq!(data["ok"], true);
    assert!(data["result"]["content"]
        .as_str()
        .unwrap()
        .contains("Lu discovery marker"));

    // Neither repairing a syntactically valid but disallowed tool nor
    // introducing an unknown argument can expand permissions or schemas.
    let forbidden = turn
        .handle_model_reply(
            r#"{"kind":"tool","name":"lu_files_search_text","arguments":{"query":"marker",},}"#,
            &catalog,
            &registry,
            &authority,
            &Permitted,
        )
        .await;
    assert!(matches!(
        forbidden,
        Err(ToolTurnError::Catalog(ModelToolError::UnknownTool(_)))
    ));
    let unknown_field = turn.handle_model_reply(
        r#"{"kind":"tool","name":"lu_files_read_text","arguments":{"path":"notes.txt","grant":"admin",},}"#,
        &catalog, &registry, &authority, &Permitted,
    ).await;
    assert!(matches!(
        unknown_field,
        Err(ToolTurnError::Catalog(ModelToolError::InvalidArguments(_)))
    ));
    let duplicate = turn.handle_model_reply(
        r#"{"kind":"tool","name":"lu_files_read_text","arguments":{"path":"notes.txt","path":"other.txt"}}"#,
        &catalog, &registry, &authority, &Permitted,
    ).await;
    assert_eq!(duplicate, Err(ToolTurnError::MalformedModelReply));
    assert_eq!(turn.step_count(), 1, "invalid calls must not execute");

    drop(turn);
    drop(catalog);
    drop(registry);
    remove_workspace(folder);
}
