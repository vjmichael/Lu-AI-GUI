use std::{
    collections::BTreeSet,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use lu_core::{
    contracts::{AuthorizationError, ProtoPayload, ToolAuthorizer, ToolDescriptor, ToolInvocation},
    model::{ModelToolError, MAX_MODEL_TOOL_ARGS_BYTES},
    native_files::register_bundled_file_tools,
    registry::{DispatchError, NativeToolRegistry},
    wire::{self, proto},
};
use prost::Message;

struct Authorized;
impl ToolAuthorizer for Authorized {
    fn authorize(&self, _: &ToolInvocation, _: &ToolDescriptor) -> Result<(), AuthorizationError> {
        Ok(())
    }
}
struct Denied;
impl ToolAuthorizer for Denied {
    fn authorize(&self, _: &ToolInvocation, _: &ToolDescriptor) -> Result<(), AuthorizationError> {
        Err(AuthorizationError::new("file access disabled"))
    }
}
// Windows clocks need not advance between parallel fixture creation calls.
// A monotonic per-process ordinal makes each test root distinct even when
// timestamp and PID are identical. This counter grants no tool authority.
static NEXT_FIXTURE_ID: AtomicU64 = AtomicU64::new(0);

fn fixture() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let ordinal = NEXT_FIXTURE_ID.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "lu-model-files-{}-{nonce}-{ordinal}",
        std::process::id()
    ));
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("hello.txt"), "Hello Lu from a native tool").unwrap();
    fs::create_dir(root.join("subdir")).unwrap();
    root
}

#[test]
fn parallel_native_file_fixtures_do_not_reuse_a_windows_temp_root() {
    // Simulate the Windows CI failure class: many tests within one process
    // can observe the same coarse system clock tick.
    let tasks: Vec<_> = (0..48).map(|_| std::thread::spawn(fixture)).collect();
    let directories: Vec<_> = tasks
        .into_iter()
        .map(|task| task.join().expect("fixture thread"))
        .collect();
    let unique: BTreeSet<_> = directories.iter().collect();
    assert_eq!(
        unique.len(),
        directories.len(),
        "parallel fixtures collided"
    );
    for directory in directories {
        assert_eq!(
            fs::read_to_string(directory.join("hello.txt")).unwrap(),
            "Hello Lu from a native tool"
        );
        assert!(directory.join("subdir").is_dir());
        fs::remove_dir_all(directory).unwrap();
    }
}

#[tokio::test]
async fn model_json_is_strict_then_dispatched_as_typed_protobuf_without_mcp() {
    let root = fixture();
    let registry = NativeToolRegistry::new();
    let catalog = register_bundled_file_tools(&registry, &root).unwrap();
    let allowed = BTreeSet::from([
        "lu_files_read_text".to_string(),
        "lu_files_list_directory".to_string(),
    ]);
    let schemas = catalog.schemas(&allowed).unwrap();
    assert_eq!(schemas.len(), 2);
    assert_eq!(
        schemas[0]["function"]["parameters"]["additionalProperties"],
        false
    );

    let call = catalog
        .encode_call(
            &allowed,
            "lu_files_read_text",
            "file-1",
            None,
            r#"{"path":"hello.txt","max_bytes":512}"#,
        )
        .unwrap();
    let frame = wire::encode_invocation(&call).unwrap();
    let decoded = wire::decode_invocation(&frame).unwrap();
    assert_eq!(decoded.tool_id, "lu.files.read_text");
    assert!(matches!(
        registry.dispatch(decoded.clone(), &Denied).await,
        Err(DispatchError::Denied(_))
    ));
    let output = registry.dispatch(decoded, &Authorized).await.unwrap();
    let reply = proto::ReadTextFileResult::decode(output.payload.bytes.as_slice()).unwrap();
    assert_eq!(reply.content, "Hello Lu from a native tool");
    assert!(!reply.truncated);
    let json = catalog
        .decode_result(&allowed, "lu_files_read_text", &Ok(output))
        .unwrap();
    assert_eq!(json["ok"], true);
    assert_eq!(json["result"]["content"], "Hello Lu from a native tool");
    let permission_error = catalog
        .decode_result(
            &allowed,
            "lu_files_read_text",
            &Err(DispatchError::Denied("internal permission detail".into())),
        )
        .unwrap();
    assert_eq!(permission_error["ok"], false);
    assert_eq!(permission_error["error"]["code"], "PERMISSION_DENIED");
    assert_eq!(
        permission_error["error"]["message"],
        "tool action was not authorized"
    );
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn directory_listing_works_and_respects_limits() {
    let root = fixture();
    let registry = NativeToolRegistry::new();
    let catalog = register_bundled_file_tools(&registry, &root).unwrap();
    let allowed = BTreeSet::from(["lu_files_list_directory".to_string()]);
    let call = catalog
        .encode_call(
            &allowed,
            "lu_files_list_directory",
            "list-1",
            None,
            r#"{"limit":20}"#,
        )
        .unwrap();
    let output = registry.dispatch(call, &Authorized).await.unwrap();
    let reply = proto::ListDirectoryResult::decode(output.payload.bytes.as_slice()).unwrap();
    assert_eq!(reply.entries.len(), 2);
    assert_eq!(reply.entries[0].name, "hello.txt");
    assert_eq!(reply.entries[1].name, "subdir");
    assert_eq!(reply.entries[1].kind, "directory");
    let json = catalog
        .decode_result(&allowed, "lu_files_list_directory", &Ok(output))
        .unwrap();
    assert_eq!(json["ok"], true);
    assert_eq!(json["result"]["entries"][1]["kind"], "directory");
    let wrong = catalog.decode_result(
        &allowed,
        "lu_files_list_directory",
        &Ok(lu_core::contracts::ToolOutput {
            payload: ProtoPayload {
                type_url: "type.googleapis.com/lu.v1.ReadTextFileResult".into(),
                bytes: Vec::new(),
            },
        }),
    );
    assert!(matches!(wrong, Err(ModelToolError::InvalidResult(_))));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn malformed_oversized_and_unadvertised_calls_are_rejected_without_dispatch() {
    let root = fixture();
    let registry = NativeToolRegistry::new();
    let catalog = register_bundled_file_tools(&registry, &root).unwrap();
    let read = BTreeSet::from(["lu_files_read_text".to_string()]);
    for input in [
        r#"{"path":"hello.txt","extra":true}"#,
        r#"{"path":42}"#,
        r#"{"path":"../secrets.txt"}"#,
        r#"{"path":"C:\\Windows\\system.ini"}"#,
        r#"{"path":"hello.txt","max_bytes":0}"#,
        r#"{"path":"hello.txt","max_bytes":true}"#,
        r#"{"path":"hello.txt","max_bytes":99999999}"#,
        r#"{"path":"hello.txt""#,
        "null",
        "[]",
    ] {
        assert!(
            matches!(
                catalog.encode_call(&read, "lu_files_read_text", "bad", None, input),
                Err(ModelToolError::InvalidArguments(_))
            ),
            "should reject: {input}",
        );
    }
    assert!(matches!(
        catalog.encode_call(&read, "lu_files_list_directory", "bad", None, "{}"),
        Err(ModelToolError::UnknownTool(_))
    ));
    assert!(matches!(
        catalog.encode_call(
            &read,
            "lu_files_read_text",
            "large",
            None,
            &" ".repeat(MAX_MODEL_TOOL_ARGS_BYTES + 1)
        ),
        Err(ModelToolError::OversizedArguments)
    ));
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn rejects_symlink_escape_from_authorized_root() {
    let root = fixture();
    let registry = NativeToolRegistry::new();
    let _catalog = register_bundled_file_tools(&registry, &root).unwrap();
    #[cfg(unix)]
    let allowed = BTreeSet::from(["lu_files_read_text".to_string()]);
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("/etc/passwd", root.join("outlink")).unwrap();
        let call = _catalog
            .encode_call(
                &allowed,
                "lu_files_read_text",
                "symlink-1",
                None,
                r#"{"path":"outlink"}"#,
            )
            .unwrap();
        let result = registry.dispatch(call, &Authorized).await;
        assert!(matches!(result, Err(DispatchError::Execution(_))));
    }
    // Keep an explicit test of invalid protobuf bytes bypassing the model.
    let bad = ToolInvocation {
        call_id: "bypass".into(),
        tool_id: "lu.files.read_text".into(),
        workspace_id: None,
        input: ProtoPayload {
            type_url: "type.googleapis.com/lu.v1.ReadTextFileRequest".into(),
            bytes: vec![0xff],
        },
    };
    assert!(matches!(
        registry.dispatch(bad, &Authorized).await,
        Err(DispatchError::Execution(_))
    ));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn tool_schema_budget_and_registered_implementation_are_enforced() {
    let root = fixture();
    let registry = NativeToolRegistry::new();
    let catalog = register_bundled_file_tools(&registry, &root).unwrap();
    let allowed = BTreeSet::from(["not_registered".to_string()]);
    assert!(matches!(
        catalog.schemas(&allowed),
        Err(ModelToolError::UnknownTool(_))
    ));
    assert!(matches!(
        register_bundled_file_tools(&registry, &root),
        Err(lu_core::native_files::FileBundleError::Registry(_))
    ));
    let _ = fs::remove_dir_all(root);
}

#[cfg(windows)]
#[tokio::test]
async fn windows_model_file_tool_cannot_read_sibling_or_parent_workspace() {
    // Both workspaces are genuine filesystem paths. The hostile model must
    // not be able to select the other scope or climb out through Windows
    // path forms, even when the underlying file actually exists.
    let container = fixture();
    let authorized = container.join("authorized");
    let private = container.join("private");
    fs::create_dir(&authorized).unwrap();
    fs::create_dir(&private).unwrap();
    fs::write(authorized.join("visible.txt"), "visible workspace data").unwrap();
    fs::write(private.join("secret.txt"), "secret sibling workspace").unwrap();
    let registry = NativeToolRegistry::new();
    let catalog = register_bundled_file_tools(&registry, &authorized).unwrap();
    let allowed = BTreeSet::from(["lu_files_read_text".to_string()]);
    let valid = catalog
        .encode_call(
            &allowed,
            "lu_files_read_text",
            "in-root",
            Some("thread:authorized"),
            r#"{"path":"visible.txt"}"#,
        )
        .unwrap();
    let good = registry.dispatch(valid, &Authorized).await.unwrap();
    let reply = proto::ReadTextFileResult::decode(good.payload.bytes.as_slice()).unwrap();
    assert_eq!(reply.content, "visible workspace data");

    for attempted in [
        "../private/secret.txt",
        "nested/../../private/secret.txt",
        "/private/secret.txt",
        "C:/Windows/win.ini",
        "C:\\\\Windows\\\\win.ini",
        "//server/share/file.txt",
        "private\\\\secret.txt",
    ] {
        let encoded = serde_json::json!({"path": attempted}).to_string();
        assert!(
            matches!(
                catalog.encode_call(
                    &allowed,
                    "lu_files_read_text",
                    "escape",
                    Some("thread:authorized"),
                    &encoded
                ),
                Err(ModelToolError::InvalidArguments(_))
            ),
            "Windows path escape accepted: {attempted:?}"
        );
    }
    let permitted_but_absent = catalog
        .encode_call(
            &allowed,
            "lu_files_read_text",
            "sibling-name",
            Some("thread:authorized"),
            r#"{"path":"secret.txt"}"#,
        )
        .unwrap();
    assert!(matches!(
        registry.dispatch(permitted_but_absent, &Authorized).await,
        Err(DispatchError::Execution(_))
    ));
    assert_eq!(
        fs::read_to_string(private.join("secret.txt")).unwrap(),
        "secret sibling workspace"
    );
    // On Windows the capability directory handle remains open while the
    // registry is alive. Release it before recursively removing this fixture.
    drop(catalog);
    drop(registry);
    fs::remove_dir_all(container).unwrap();
}
