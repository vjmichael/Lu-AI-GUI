//! AUTH-01: real desktop ChatStore user-turn verifier drives Lu Core dispatch.
//! A host-readable file is NOT an authorization grant.
#[path = "../../../lu-desktop/src-tauri/src/chat_store.rs"]
mod chat_store;

use chat_store::ChatStore;
use lu_core::{
    contracts::ToolEffect,
    conversation_authority::{AuthorizedCapability, BoundCallOrigin, ConversationAuthority, TaskAuthority},
    native_files::register_bundled_file_tools,
    policy::{CapabilityGrant, PolicyEngine},
    registry::{DispatchError, NativeToolRegistry},
};
use std::{collections::BTreeSet, fs, time::{SystemTime, UNIX_EPOCH}};

fn temp_root() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "lu-real-chat-authority-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
    ))
}

#[tokio::test]
async fn real_persisted_user_turn_requires_separate_host_approval_before_native_read() {
    let root = temp_root();
    fs::create_dir_all(&root).unwrap();
    let chats = ChatStore::open(&root.join("chats")).unwrap();
    let chat = chats.create_chat().unwrap();
    let other_chat = chats.create_chat().unwrap();
    let turn_id = format!("g-{}", "a".repeat(32));
    chats.append_user_with_turn_id(&chat.chat_id, &turn_id, "Read the approved report").unwrap();
    let outside_chats = root.join("host-documents");
    fs::create_dir_all(&outside_chats).unwrap();
    fs::write(outside_chats.join("report.txt"), "actual host file").unwrap();
    let registry = NativeToolRegistry::new();
    let catalog = register_bundled_file_tools(&registry, &outside_chats).unwrap();
    let allowed = BTreeSet::from(["lu_files_read_text".to_string()]);
    let scope = format!("host-task:{}", chat.chat_id);
    let request = catalog.encode_call(
        &allowed, "lu_files_read_text", "read-1", Some(&scope),
        r#"{"path":"report.txt"}"#,
    ).unwrap();
    let origin = BoundCallOrigin {
        conversation_id: chat.chat_id.clone(),
        user_turn_id: turn_id.clone(),
        task_id: "authorized-task-1".into(),
    };
    let authority = ConversationAuthority::new();
    let policy = PolicyEngine::new();

    // Neither a real file, nor a real user message, nor advertised tools grants action.
    assert!(matches!(
        authority.dispatch_bound(&registry, request.clone(), origin.clone(), &policy).await,
        Err(DispatchError::Denied(_))
    ));
    let task = TaskAuthority {
        conversation_id: chat.chat_id.clone(),
        user_turn_id: turn_id.clone(),
        task_id: origin.task_id.clone(),
        execution_scope_id: scope.clone(),
        capabilities: vec![AuthorizedCapability {
            tool_id: "lu.files.read_text".into(),
            effect: ToolEffect::ReadWorkspace,
        }],
        expires_at: None,
    };
    assert!(authority.open_user_task(TaskAuthority {
        conversation_id: other_chat.chat_id,
        ..task.clone()
    }, &chats).is_err());
    authority.open_user_task(task, &chats).unwrap();
    // A verified origin without a trusted host-side effect grant remains denied.
    assert!(matches!(
        authority.dispatch_bound(&registry, request.clone(), origin.clone(), &policy).await,
        Err(DispatchError::Denied(_))
    ));
    policy.grant(CapabilityGrant {
        tool_id: "lu.files.read_text".into(),
        effect: ToolEffect::ReadWorkspace,
        scope_id: scope.clone(),
        expires_at: None,
    }).unwrap();
    let result = authority.dispatch_bound(&registry, request.clone(), origin.clone(), &policy).await;
    assert!(result.is_ok(), "approved native read failed: {result:?}");
    let mut wrong_scope = request.clone();
    wrong_scope.workspace_id = Some("different-scope".into());
    assert!(matches!(
        authority.dispatch_bound(&registry, wrong_scope, origin.clone(), &policy).await,
        Err(DispatchError::Denied(_))
    ));
    authority.revoke_task(&origin.task_id).unwrap();
    assert!(matches!(
        authority.dispatch_bound(&registry, request, origin, &policy).await,
        Err(DispatchError::Denied(_))
    ));
    drop(registry);
    drop(catalog);
    drop(chats);
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn imported_legacy_user_message_cannot_mint_tool_authority() {
    let root = temp_root();
    fs::create_dir_all(&root).unwrap();
    let chats = ChatStore::open(&root).unwrap();
    let chat = chats.create_chat().unwrap();
    chats.append_user(&chat.chat_id, "Legacy User event with no trusted ID").unwrap();
    let authority = ConversationAuthority::new();
    let result = authority.open_user_task(TaskAuthority {
        conversation_id: chat.chat_id,
        user_turn_id: format!("g-{}", "b".repeat(32)),
        task_id: "forged-task".into(),
        execution_scope_id: "scope".into(),
        capabilities: vec![AuthorizedCapability {
            tool_id: "lu.files.read_text".into(),
            effect: ToolEffect::ReadWorkspace,
        }],
        expires_at: None,
    }, &chats);
    assert!(result.is_err());
    drop(chats);
    fs::remove_dir_all(root).unwrap();
}
