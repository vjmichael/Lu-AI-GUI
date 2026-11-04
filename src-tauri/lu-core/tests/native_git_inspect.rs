//! End-to-end native Git read-only inspection against a disposable real repo.
//! No production file writes and no generic model-selected shell execution.

use lu_core::{
    agent_tool_loop::{NativeToolTurn, ToolTurnError, ToolTurnOutcome},
    contracts::{AuthorizationError, ToolAuthorizer, ToolDescriptor, ToolEffect, ToolInvocation},
    conversation_authority::{
        AuthorizedCapability, BoundCallOrigin, ConversationAuthority, TaskAuthority,
        TrustedUserTurnVerifier,
    },
    model::{ModelToolCatalog, ModelToolError},
    native_git_inspect::register_git_inspection_tools,
    registry::{DispatchError, NativeToolRegistry},
    wire::proto,
};
use prost::Message;
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

struct Verified;
impl TrustedUserTurnVerifier for Verified {
    fn verify_user_turn(&self, conversation_id: &str, turn_id: &str) -> bool {
        conversation_id == "git-chat" && turn_id == "git-turn"
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
        Err(AuthorizationError::new("Git inspection not approved"))
    }
}

static NEXT_ID: AtomicU64 = AtomicU64::new(0);
fn git_path() -> PathBuf {
    let path = PathBuf::from(
        std::env::var_os("LU_TOOLS03_TEST_GIT_PATH")
            .expect("CI must set an absolute trusted Git executable for this test"),
    );
    assert!(path.is_absolute(), "Git executable must be an absolute host path");
    path.canonicalize().unwrap()
}
fn unique_root() -> PathBuf {
    loop {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let tick = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("lu-git-{}-{tick}-{id}", std::process::id()));
        match fs::create_dir(&root) {
            Ok(()) => return root,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => panic!("cannot make fixture: {e}"),
        }
    }
}
fn run(git: &Path, root: &Path, args: &[&str]) {
    let status = Command::new(git)
        .arg("-C").arg(root).args(args)
        .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
        .status().expect("start fixture Git");
    assert!(status.success(), "fixture Git command failed: {args:?}");
}
fn fixture() -> (PathBuf, PathBuf) {
    let git = git_path();
    let root = unique_root();
    run(&git, &root, &["init", "-q"]);
    fs::write(root.join("notes.txt"), "original\n").unwrap();
    run(&git, &root, &["add", "--", "notes.txt"]);
    run(&git, &root, &[
        "-c", "user.name=Lu Test", "-c", "user.email=lu@example.invalid",
        "commit", "--no-gpg-sign", "-q", "-m", "first Lu note",
    ]);
    fs::write(root.join("notes.txt"), "updated\n").unwrap();
    fs::write(root.join("untracked.txt"), "new\n").unwrap();
    (root, git)
}
fn teardown(root: PathBuf) {
    for attempt in 0..10 {
        match fs::remove_dir_all(&root) {
            Ok(()) => return,
            Err(e) if e.raw_os_error() == Some(32) && attempt < 9 => {
                std::thread::sleep(std::time::Duration::from_millis(100));
            },
            Err(e) => panic!("cannot delete Git fixture {root:?}: {e}"),
        }
    }
}
fn register(root: &Path, git: &Path) -> (NativeToolRegistry, ModelToolCatalog) {
    let registry = NativeToolRegistry::new();
    let mut catalog = ModelToolCatalog::new();
    register_git_inspection_tools(&registry, &mut catalog, root, git).unwrap();
    (registry, catalog)
}
fn allowed() -> BTreeSet<String> {
    ["lu_git_status", "lu_git_log", "lu_git_diff"]
        .into_iter().map(String::from).collect()
}
fn scope() -> (ConversationAuthority, BoundCallOrigin) {
    let auth = ConversationAuthority::new();
    let origin = BoundCallOrigin {
        conversation_id: "git-chat".into(), user_turn_id: "git-turn".into(),
        task_id: "git-task".into(),
    };
    auth.open_user_task(TaskAuthority {
        conversation_id: origin.conversation_id.clone(),
        user_turn_id: origin.user_turn_id.clone(),
        task_id: origin.task_id.clone(),
        execution_scope_id: "git-scope".into(),
        capabilities: ["lu.git.status", "lu.git.log", "lu.git.diff"]
            .into_iter().map(|tool_id| AuthorizedCapability {
                tool_id: tool_id.into(), effect: ToolEffect::ReadWorkspace,
            }).collect(),
        expires_at: None,
    }, &Verified).unwrap();
    (auth, origin)
}

#[tokio::test]
async fn all_three_git_tools_inspect_real_repo_and_are_read_only() {
    let (root, git) = fixture();
    let before_head = fs::read(root.join(".git/HEAD")).unwrap();
    let before_index = fs::read(root.join(".git/index")).unwrap();
    let (registry, catalog) = register(&root, &git);
    assert_eq!(catalog.schemas(&allowed()).unwrap().len(), 3);
    let (auth, origin) = scope();
    let mut turn = NativeToolTurn::new_read_only(
        &catalog, &registry, &auth, origin, "git-scope".into(), &allowed(),
    ).unwrap();

    let status = turn.handle_model_reply(
        r#"{"kind":"tool","name":"lu_git_status","arguments":{"max_entries":10}}"#,
        &catalog, &registry, &auth, &Allow,
    ).await.unwrap();
    let ToolTurnOutcome::ToolResult { data: status, .. } = status else {
        panic!("status not a real native tool result");
    };
    assert_eq!(status["ok"], true);
    let entries = status["result"]["entries"].as_array().unwrap();
    assert!(entries.iter().any(|e| e["path"] == "notes.txt" && e["state"] == " M"));
    assert!(entries.iter().any(|e| e["path"] == "untracked.txt" && e["state"] == "??"));

    let log = turn.handle_model_reply(
        r#"{"kind":"tool","name":"lu_git_log","arguments":{"limit":5}}"#,
        &catalog, &registry, &auth, &Allow,
    ).await.unwrap();
    let ToolTurnOutcome::ToolResult { data: log, .. } = log else { panic!("log missing") };
    assert_eq!(log["ok"], true);
    assert_eq!(log["result"]["commits"][0]["subject"], "first Lu note");
    assert_eq!(log["result"]["commits"][0]["commit"].as_str().unwrap().len(), 40);

    let diff = turn.handle_model_reply(
        r#"{"kind":"tool","name":"lu_git_diff","arguments":{"path":"notes.txt","max_bytes":4096}}"#,
        &catalog, &registry, &auth, &Allow,
    ).await.unwrap();
    let ToolTurnOutcome::ToolResult { data: diff, .. } = diff else { panic!("diff missing") };
    assert_eq!(diff["ok"], true);
    let patch = diff["result"]["diff"].as_str().unwrap();
    assert!(patch.contains("-original") && patch.contains("+updated"));
    assert_eq!(diff["result"]["truncated"], false);
    assert_eq!(turn.step_count(), 3);
    assert_eq!(fs::read(root.join(".git/HEAD")).unwrap(), before_head);
    assert_eq!(fs::read(root.join(".git/index")).unwrap(), before_index);
    assert_eq!(fs::read_to_string(root.join("notes.txt")).unwrap(), "updated\n");
    assert_eq!(fs::read_to_string(root.join("untracked.txt")).unwrap(), "new\n");
    drop(turn); drop(auth); drop(catalog); drop(registry);
    teardown(root);
}

#[tokio::test]
async fn unsafe_model_git_arguments_and_unapproved_dispatch_are_refused() {
    let (root, git) = fixture();
    let (registry, catalog) = register(&root, &git);
    let permitted = allowed();
    for bad in [
        r#"{"path":"../secret.txt"}"#,
        r#"{"path":"/etc/passwd"}"#,
        r#"{"path":"C:/Windows/win.ini"}"#,
        r#"{"path":"--output=/tmp/writes"}"#,
        r#"{"path":".git/config"}"#,
        r#"{"path":"notes.txt","path":"untracked.txt"}"#,
        r#"{"path":"notes.txt","command":"reset --hard"}"#,
        r#"{"max_bytes":65537}"#,
        r#"{"max_bytes":0}"#,
        r#"{"max_bytes":true}"#,
    ] {
        assert!(catalog.encode_call(&permitted, "lu_git_diff", "bad", None, bad).is_err(),
            "unexpectedly accepted untrusted Git args: {bad}");
    }
    for bad in [
        r#"{"max_entries":0}"#, r#"{"max_entries":1000}"#,
        r#"{"max_entries":10,"action":"clean"}"#,
    ] {
        assert!(catalog.encode_call(&permitted, "lu_git_status", "bad", None, bad).is_err());
    }
    let call = catalog.encode_call(
        &permitted, "lu_git_log", "grant-denied", None, r#"{"limit":1}"#,
    ).unwrap();
    assert!(matches!(
        registry.dispatch(call, &Deny).await, Err(DispatchError::Denied(_))
    ));
    let (auth, origin) = scope();
    let mut turn = NativeToolTurn::new(
        &catalog, &auth, origin, "git-scope".into(), &permitted,
    ).unwrap();
    auth.revoke_task("git-task").unwrap();
    assert_eq!(
        turn.handle_model_reply(
            r#"{"kind":"tool","name":"lu_git_log","arguments":{"limit":1}}"#,
            &catalog, &registry, &auth, &Allow,
        ).await,
        Err(ToolTurnError::Revoked),
    );
    assert_eq!(turn.step_count(), 0);
    drop(turn); drop(auth); drop(catalog); drop(registry);
    teardown(root);
}

#[tokio::test]
async fn protobuf_unknown_fields_and_registry_type_spoofing_are_rejected() {
    let (root, git) = fixture();
    let (registry, catalog) = register(&root, &git);
    let allowed = allowed();
    let call = catalog.encode_call(
        &allowed, "lu_git_status", "native-1", None, "{}",
    ).unwrap();
    let mut wrong = call.clone();
    wrong.input.bytes.extend_from_slice(&[0x78, 0x01]); // unknown protobuf field 15
    assert!(matches!(registry.dispatch(wrong, &Allow).await,
        Err(DispatchError::Execution(_))));
    let mut wrong_type = call;
    wrong_type.input.type_url = "type.googleapis.com/lu.v1.ReadTextFileRequest".into();
    assert!(registry.dispatch(wrong_type, &Allow).await.is_err());
    let _ = proto::GitStatusRequest::decode(
        catalog.encode_call(&allowed, "lu_git_status", "normal", None, "{}")
            .unwrap().input.bytes.as_slice(),
    ).unwrap();
    drop(catalog); drop(registry);
    teardown(root);
}

#[test]
fn Git_root_gitdir_and_executable_are_only_host_defined() {
    let (root, git) = fixture();
    let reg = NativeToolRegistry::new();
    let mut catalog = ModelToolCatalog::new();
    assert!(register_git_inspection_tools(&reg, &mut catalog, &root.join("missing"), &git).is_err());
    let empty = unique_root();
    assert!(register_git_inspection_tools(&reg, &mut catalog, &empty, &git).is_err());
    assert!(register_git_inspection_tools(&reg, &mut catalog, &root, &root.join("notes.txt")).is_err());
    fs::write(root.join(".git/commondir"), "../elsewhere").unwrap();
    assert!(register_git_inspection_tools(&reg, &mut catalog, &root, &git).is_err());
    drop(catalog); drop(reg);
    teardown(empty);
    teardown(root);
}
