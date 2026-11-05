//! TOOLS-04: real ZIPs through native registry and actual authorized turn.
use std::{
    collections::BTreeSet, fs, io::Write, path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
use lu_core::{
    agent_tool_loop::{NativeToolTurn, ToolTurnOutcome},
    contracts::{AuthorizationError, ToolAuthorizer, ToolDescriptor, ToolEffect, ToolInvocation},
    conversation_authority::{
        AuthorizedCapability, BoundCallOrigin, ConversationAuthority, TaskAuthority,
        TrustedUserTurnVerifier,
    },
    model::ModelToolCatalog,
    native_archive_inspect::register_archive_inspection_tool,
    registry::{DispatchError, NativeToolRegistry},
};
use zip::{ZipWriter, write::SimpleFileOptions, CompressionMethod};

static COUNTER: AtomicU64 = AtomicU64::new(0);
fn sandbox() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let p = std::env::temp_dir().join(format!("lu-archive-{}-{ts}-{n}", std::process::id()));
    fs::create_dir(&p).unwrap(); p
}
fn write_zip(root: &Path, name: &str, entries: &[(&str, &str)]) {
    let file = fs::File::create(root.join(name)).unwrap();
    let mut z = ZipWriter::new(file);
    for (name, data) in entries {
        z.start_file(*name, SimpleFileOptions::default()
            .compression_method(CompressionMethod::Stored)).unwrap();
        z.write_all(data.as_bytes()).unwrap();
    }
    z.finish().unwrap();
}
fn register(root:&Path) -> (NativeToolRegistry, ModelToolCatalog) {
    let registry = NativeToolRegistry::new();
    let mut catalog = ModelToolCatalog::new();
    register_archive_inspection_tool(&registry, &mut catalog, root).unwrap();
    (registry, catalog)
}
struct UserVerifier;
impl TrustedUserTurnVerifier for UserVerifier {
    fn verify_user_turn(&self,c:&str,t:&str)->bool {c=="archive-chat"&&t=="archive-user"}
}
struct Allow;
impl ToolAuthorizer for Allow {
    fn authorize(&self,_:&ToolInvocation,_:&ToolDescriptor)->Result<(),AuthorizationError>{Ok(())}
}
struct Deny;
impl ToolAuthorizer for Deny {
    fn authorize(&self,_:&ToolInvocation,_:&ToolDescriptor)->Result<(),AuthorizationError>{
        Err(AuthorizationError::new("read not approved"))
    }
}
fn scope() -> (ConversationAuthority,BoundCallOrigin,BTreeSet<String>) {
    let auth=ConversationAuthority::new();
    let origin=BoundCallOrigin {
        conversation_id:"archive-chat".into(),
        user_turn_id:"archive-user".into(),
        task_id:"archive-task".into(),
    };
    auth.open_user_task(TaskAuthority {
        conversation_id:origin.conversation_id.clone(),
        user_turn_id:origin.user_turn_id.clone(),
        task_id:origin.task_id.clone(),
        execution_scope_id:"archive-scope".into(),
        capabilities:vec![AuthorizedCapability {
            tool_id:"lu.archive.list".into(),effect:ToolEffect::ReadWorkspace,
        }],
        expires_at:None,
    },&UserVerifier).unwrap();
    let tools=BTreeSet::from(["lu_archive_list".into()]);
    (auth,origin,tools)
}
async fn inspect_one(root:&Path,path:&str,max:usize)->serde_json::Value {
    let (reg, catalog)=register(root);
    let (auth, origin,tools)=scope();
    let mut turn=NativeToolTurn::new_read_only(
        &catalog,&reg,&auth,origin,"archive-scope".into(),&tools
    ).unwrap();
    let prompt=format!(r#"{{"kind":"tool","name":"lu_archive_list","arguments":{{"path":"{path}","max_entries":{max}}}}}"#);
    let out=turn.handle_model_reply(&prompt,&catalog,&reg,&auth,&Allow).await.unwrap();
    let ToolTurnOutcome::ToolResult{data,..}=out else {panic!("missing typed tool result")};
    data
}
#[tokio::test]
async fn real_zip_metadata_only_with_strict_output_budget() {
    let root=sandbox();
    write_zip(&root,"payload.zip",&[
        ("a.txt","alpha"),("folder/b.txt","beta"),("c.json",r#"{"x":1}"#),
    ]);
    let before=fs::read(root.join("payload.zip")).unwrap();
    let full=inspect_one(&root,"payload.zip",10).await;
    assert_eq!(full["ok"],true);
    assert_eq!(full["result"]["total_entries"],3);
    assert_eq!(full["result"]["truncated"],false);
    assert_eq!(full["result"]["entries"][0]["path"],"a.txt");
    assert_eq!(full["result"]["entries"][1]["path"],"folder/b.txt");
    assert_eq!(full["result"]["entries"][1]["declared_uncompressed_bytes"],4);
    assert_eq!(full["result"]["entries"][1]["compressed_bytes"],4);
    let partial=inspect_one(&root,"payload.zip",1).await;
    assert_eq!(partial["ok"],true);
    assert_eq!(partial["result"]["entries"].as_array().unwrap().len(),1);
    assert_eq!(partial["result"]["truncated"],true);
    assert_eq!(partial["result"]["total_entries"],3);
    assert_eq!(fs::read(root.join("payload.zip")).unwrap(),before);
    assert_eq!(fs::read_dir(&root).unwrap().count(),1,"never extract");
    fs::remove_dir_all(root).unwrap();
}
#[tokio::test]
async fn malicious_archive_members_fail_closed_without_extraction() {
    let root=sandbox();
    write_zip(&root,"traversal.zip",&[("../escape.txt","x")]);
    // Construct a valid ZIP first, then mutate *equal-length* local+central
    // header names to make a malicious duplicate. ZipWriter may itself refuse
    // duplicate names (as a safe writer should), so this fixture must not
    // depend on writer support for deliberately malformed directory entries.
    write_zip(&root,"duplicates.zip",&[("same.txt","x"),("sane.txt","y")]);
    let dup = root.join("duplicates.zip");
    let mut bytes = fs::read(&dup).unwrap();
    let mut replaced=0;
    for i in 0..bytes.len().saturating_sub(7) {
        if &bytes[i..i+8] == b"sane.txt" {
            bytes[i..i+8].copy_from_slice(b"same.txt");
            replaced+=1;
        }
    }
    assert_eq!(replaced,2,"expected local and central duplicate-name rewrite");
    fs::write(&dup,bytes).unwrap();
    write_zip(&root,"windows-alias.zip",&[("Readme.txt","x"),("README.TXT","y")]);
    write_zip(&root,"evil-slash.zip",&[("folder\\other.txt","x")]);
    let mut link_zip = ZipWriter::new(fs::File::create(root.join("symlink.zip")).unwrap());
    link_zip.add_symlink("shortcut.txt","../../outside.txt",
        SimpleFileOptions::default().compression_method(CompressionMethod::Stored)).unwrap();
    link_zip.finish().unwrap();
    for name in ["traversal.zip","duplicates.zip","windows-alias.zip","evil-slash.zip","symlink.zip"] {
        let got=inspect_one(&root,name,10).await;
        assert_eq!(got["ok"],false,"must refuse unsafe ZIP {name}: {got}");
        assert_eq!(got["error"]["retryable"],false);
    }
    assert!(!root.join("escape.txt").exists());
    assert_eq!(fs::read_dir(&root).unwrap().count(),5);
    fs::remove_dir_all(root).unwrap();
}
#[tokio::test]
async fn invalid_frames_scope_denial_and_missing_zip_are_rejected() {
    let root=sandbox();
    fs::write(root.join("invalid.zip"),b"not a zip").unwrap();
    let bad_zip=inspect_one(&root,"invalid.zip",2).await;
    assert_eq!(bad_zip["ok"],false);
    let (reg,cat)=register(&root);
    let (auth,origin,tools)=scope();
    assert_eq!(cat.schemas(&tools).unwrap().len(),1);
    for args in [
        r#"{"path":"../outside.zip"}"#,
        r#"{"path":"/etc/secret.zip"}"#,
        r#"{"path":"C:\\private.zip"}"#,
        r#"{"path":"bad.txt"}"#,
        r#"{"path":"invalid.zip","max_entries":0}"#,
        r#"{"path":"invalid.zip","max_entries":257}"#,
        r#"{"path":"invalid.zip","max_entries":5,"command":"unzip"}"#,
        r#"{"path":"invalid.zip","path":"secret.zip"}"#,
        r#"{"path":"invalid.zip","max_entries":true}"#,
    ] {
        assert!(cat.encode_call(&tools,"lu_archive_list","bad",None,args).is_err(),
            "accepted unauthorized/ambiguous archive args: {args}");
    }
    let req=cat.encode_call(&tools,"lu_archive_list","call-1",None,
        r#"{"path":"invalid.zip"}"#).unwrap();
    assert!(matches!(reg.dispatch(req.clone(),&Deny).await,Err(DispatchError::Denied(_))));
    let mut forged=req;
    forged.input.bytes.extend_from_slice(&[0x78,0x01]); // unknown Protobuf field
    assert!(reg.dispatch(forged,&Allow).await.is_err());
    let mut turn=NativeToolTurn::new(
        &cat,&auth,origin,"archive-scope".into(),&tools
    ).unwrap();
    auth.revoke_task("archive-task").unwrap();
    let denied=turn.handle_model_reply(
        r#"{"kind":"tool","name":"lu_archive_list","arguments":{"path":"invalid.zip"}}"#,
        &cat,&reg,&auth,&Allow
    ).await.expect("revoked archive scope produces a paired typed tool denial");
    let ToolTurnOutcome::ToolResult { data, .. } = denied else {
        panic!("revoked archive tool returned a non-tool answer");
    };
    assert_eq!(data["ok"],false);
    assert_eq!(data["error"]["code"],"PERMISSION_DENIED");
    assert_eq!(data["error"]["retryable"],false);
    assert_eq!(turn.step_count(),1);
    // The registered archive inspector retains a pinned ZIP File. Linux
    // permits unlinking an open file, while Windows does not. Release all
    // native handles before removing this synthetic temporary repository.
    drop(turn);
    drop(auth);
    drop(cat);
    drop(reg);
    fs::remove_dir_all(root).unwrap();
}
