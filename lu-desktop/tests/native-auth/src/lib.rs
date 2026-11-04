//! AUTH-01: real desktop ChatStore user-turn verifier drives Lu Core dispatch.
//! A host-readable file is NOT an authorization grant.
#[path = "../../../src-tauri/src/chat_store.rs"]
mod chat_store;
#[path = "../../../src-tauri/src/native_host_read.rs"]
mod native_host_read;
#[path = "../../../src-tauri/src/read_proposal.rs"]
mod read_proposal;

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

#[test]
fn real_desktop_user_turn_opens_context_read_without_transfer_of_authority() {
    use lu_core::continuity::{ContinuityError, ContinuityStore, TaskCheckpoint};

    let root = temp_root();
    fs::create_dir_all(&root).unwrap();
    let chats = ChatStore::open(&root.join("chats")).unwrap();
    let active = chats.create_chat().unwrap();
    let unrelated = chats.create_chat().unwrap();
    let turn_id = format!("g-{}", "c".repeat(32));
    chats.append_user_with_turn_id(&active.chat_id, &turn_id, "Continue Lu v1.0").unwrap();
    let continuity = ContinuityStore::open(&root.join("continuity.sqlite")).unwrap();
    continuity.append(&TaskCheckpoint {
        checkpoint_id: "checkpoint-first".into(),
        project_id: "lu-v1".into(),
        task_id: "context-01".into(),
        conversation_id: active.chat_id.clone(),
        source_entry_ids: Vec::new(),
        completed_work: "Recovered design and tests".into(),
        next_action: "Review next gate".into(),
        blockers: Vec::new(),
    }).unwrap();

    let found = continuity.latest_for_verified_user_turn(
        "lu-v1", "context-01", &active.chat_id, &turn_id, &chats
    ).unwrap().unwrap();
    assert_eq!(found.next_action, "Review next gate");
    assert_eq!(found.conversation_id, active.chat_id);

    assert_eq!(
        continuity.latest_for_verified_user_turn(
            "lu-v1", "context-01", &unrelated.chat_id, &turn_id, &chats,
        ).unwrap_err(),
        ContinuityError::InvalidInput
    );
    assert_eq!(
        continuity.latest_for_verified_user_turn(
            "lu-v1", "context-01", &active.chat_id, "untrusted-quoted-turn", &chats,
        ).unwrap_err(),
        ContinuityError::InvalidInput
    );

    // Lookup creates no capability: the tool authority store remains empty.
    let empty_authority = ConversationAuthority::new();
    assert!(empty_authority.revoke_task("context-01").is_err());
    drop(continuity);
    drop(chats);
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn user_selected_text_is_read_through_host_authorized_native_tool_and_journaled() {
    let root = temp_root();
    fs::create_dir_all(&root).unwrap();
    let chats = ChatStore::open(&root.join("chats")).unwrap();
    let chat = chats.create_chat().unwrap();
    let other_chat = chats.create_chat().unwrap();
    let files = root.join("selected-host-location");
    fs::create_dir_all(&files).unwrap();
    fs::write(files.join("readme.md"), "# Native Lu inspection\n").unwrap();

    let turn = format!("g-{}", "e".repeat(32));
    let result = native_host_read::inspect_selected_file(
        &chats, &chat.chat_id, &turn, &files.join("readme.md")
    ).await.unwrap();
    assert_eq!(result.file_name, "readme.md");
    assert_eq!(result.text, "# Native Lu inspection\n");
    assert!(!result.truncated);
    let history = chats.history(&chat.chat_id).unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].role, "user");
    assert_eq!(history[1].role, "assistant");
    assert_eq!(history[1].text, result.text);
    assert!(!history[1].interrupted);
    assert!(chats.history(&other_chat.chat_id).unwrap().is_empty());
    assert!(native_host_read::inspect_selected_file(
        &chats, &chat.chat_id, &turn, &files.join("readme.md")
    ).await.is_err(), "a used turn must not be admitted twice");

    drop(chats);
    let reopened = ChatStore::open(&root.join("chats")).unwrap();
    assert_eq!(reopened.history(&chat.chat_id).unwrap()[1].text, "# Native Lu inspection\n");
    drop(reopened);
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn selected_native_read_refuses_link_or_nonfile_and_oversized_text() {
    let root = temp_root();
    fs::create_dir_all(&root).unwrap();
    let chats = ChatStore::open(&root.join("chats")).unwrap();
    let chat = chats.create_chat().unwrap();
    let turn = format!("g-{}", "f".repeat(32));
    assert!(native_host_read::inspect_selected_file(
        &chats, &chat.chat_id, &turn, &root
    ).await.is_err());
    assert!(chats.history(&chat.chat_id).unwrap().is_empty());
    fs::write(root.join("large.txt"), "x".repeat(70000)).unwrap();
    let result = native_host_read::inspect_selected_file(
        &chats, &chat.chat_id, &turn, &root.join("large.txt")
    ).await.unwrap();
    assert!(result.truncated);
    assert_eq!(result.text.len(), 64 * 1024);
    drop(chats);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn untrusted_read_proposal_requires_exact_native_file_selection_once() {
    use read_proposal::{ProposalError, ReadApprovalQueue};
    let root = temp_root();
    fs::create_dir_all(&root).unwrap();
    let target = root.join("note.txt");
    let other = root.join("other.txt");
    fs::write(&target, "approved file").unwrap();
    fs::write(&other, "not approved").unwrap();
    let queue = ReadApprovalQueue::default();
    let json = r#"{"kind":"propose_read_file","file_name":"note.txt","purpose":"Summarize this note","max_bytes":4096}"#;
    let staged = queue.stage("p-1", "chat-1", "g-1", json).unwrap();
    assert_eq!(staged.file_name, "note.txt");
    assert_eq!(queue.stage("p-1", "chat-1", "g-1", json), Err(ProposalError::Duplicate));
    assert_eq!(
        queue.approve_selected("p-1", "other-chat", "g-1", &target),
        Err(ProposalError::WrongConversation)
    );
    assert_eq!(
        queue.approve_selected("p-1", "chat-1", "g-1", &target),
        Err(ProposalError::Unknown), "denial must consume the proposal"
    );
    queue.stage("p-2", "chat-1", "g-2", json).unwrap();
    assert_eq!(
        queue.approve_selected("p-2", "chat-1", "g-2", &other),
        Err(ProposalError::TargetMismatch)
    );
    queue.stage("p-3", "chat-1", "g-3", json).unwrap();
    assert_eq!(
        queue.approve_selected("p-3", "chat-1", "other-origin", &target),
        Err(ProposalError::WrongOrigin)
    );
    queue.stage("p-4", "chat-1", "g-4", json).unwrap();
    assert_eq!(queue.approve_selected("p-4", "chat-1", "g-4", &target).unwrap(), staged);
    assert_eq!(queue.approve_selected("p-4", "chat-1", "g-4", &target), Err(ProposalError::Unknown));
    drop(queue);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn proposal_parser_rejects_malformed_json_paths_overbroad_reads_and_mutations() {
    use read_proposal::{ProposalError, ReadApprovalQueue};
    let queue = ReadApprovalQueue::default();
    for (i, raw) in [
        r#"{"kind":"propose_read_file","file_name":"../secret","purpose":"Read","max_bytes":4096}"#,
        r#"{"kind":"propose_read_file","file_name":"note.txt","purpose":"Read","max_bytes":65537}"#,
        r#"{"kind":"propose_read_file","file_name":"note.txt","purpose":"Read","max_bytes":0}"#,
        r#"{"kind":"propose_write_file","file_name":"note.txt","purpose":"Read","max_bytes":4096}"#,
        r#"{"kind":"propose_read_file","file_name":"note.txt","purpose":"Read","max_bytes":4096,"grant":"all"}"#,
        r#"{"kind":"propose_read_file","file_name":"note.txt","purpose":"","max_bytes":4096}"#,
        r#"{"kind":"propose_read_file","file_name":"note.txt","purpose":"Read","max_bytes":4096,"file_name":"other.txt"}"#,
        r#"{"kind":"propose_read_file","file_name":"C:\\secret.txt","purpose":"Read","max_bytes":4096}"#,
        "{ malformed json",
    ].iter().enumerate() {
        assert_eq!(
            queue.stage(&format!("p-{i}"), "chat-1", "g-1", raw),
            Err(ProposalError::Invalid),
            "unsafe proposal admitted: {raw}"
        );
    }
    let good = r#"{"kind":"propose_read_file","file_name":"note.txt","purpose":"Read","max_bytes":1024}"#;
    queue.stage("p-ok", "chat-1", "g-1", good).unwrap();
    assert_eq!(queue.clear_conversation("chat-1").unwrap(), 1);
    assert_eq!(queue.dismiss("p-ok"), Err(ProposalError::Unknown));
}

#[tokio::test]
async fn completed_model_proposal_can_only_execute_with_matching_os_selected_file_and_limit() {
    use read_proposal::{ProposalError, ReadApprovalQueue};
    let root = temp_root();
    fs::create_dir_all(&root).unwrap();
    let files = root.join("host-files");
    fs::create_dir_all(&files).unwrap();
    let target = files.join("notes.txt");
    fs::write(&target, "z".repeat(8192)).unwrap();
    let chats = ChatStore::open(&root.join("chats")).unwrap();
    let chat = chats.create_chat().unwrap();
    let other = chats.create_chat().unwrap();
    let raw = r#"{"kind":"propose_read_file","file_name":"notes.txt","purpose":"Review first 128 bytes","max_bytes":128}"#;
    let pending = ReadApprovalQueue::default();
    pending.stage("p-bound", &chat.chat_id, "model-output-1", raw).unwrap();
    assert_eq!(
        pending.origin_for_conversation("p-bound", &other.chat_id),
        Err(ProposalError::WrongConversation)
    );
    assert_eq!(
        pending.origin_for_conversation("p-bound", &chat.chat_id).unwrap(),
        "model-output-1"
    );
    let request = pending.approve_selected(
        "p-bound", &chat.chat_id, "model-output-1", &target
    ).unwrap();
    assert_eq!(request.max_bytes, 128);
    let result = native_host_read::inspect_selected_file_limited(
        &chats, &chat.chat_id, &format!("g-{}", "1".repeat(32)),
        &target, u64::from(request.max_bytes),
    ).await.unwrap();
    assert!(result.truncated);
    assert_eq!(result.text.len(), 128);
    assert!(chats.history(&other.chat_id).unwrap().is_empty());
    assert_eq!(pending.origin_for_conversation("p-bound", &chat.chat_id), Err(ProposalError::Unknown));
    assert!(native_host_read::inspect_selected_file_limited(
        &chats, &chat.chat_id, &format!("g-{}", "2".repeat(32)),
        &target, 65537,
    ).await.is_err(), "native tool cannot exceed the maximum allowed inspection budget");
    drop(chats);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn model_proposal_rejection_preserves_other_chats_and_is_idempotent() {
    use read_proposal::{ProposalError, ReadApprovalQueue};
    let queue = ReadApprovalQueue::default();
    let raw = r#"{"kind":"propose_read_file","file_name":"notes.txt","purpose":"Read","max_bytes":4096}"#;
    queue.stage("p-a", "chat-a", "generation-a", raw).unwrap();
    queue.stage("p-b", "chat-b", "generation-b", raw).unwrap();
    assert_eq!(queue.dismiss_for_conversation("p-a", "chat-b"), Err(ProposalError::WrongConversation));
    queue.dismiss_for_conversation("p-a", "chat-a").unwrap();
    assert_eq!(queue.dismiss_for_conversation("p-a", "chat-a"), Err(ProposalError::Unknown));
    assert_eq!(queue.origin_for_conversation("p-b", "chat-b").unwrap(), "generation-b");
    assert_eq!(queue.clear_conversation("chat-b").unwrap(), 1);
}

#[tokio::test]
async fn durable_native_read_has_applied_receipt_and_survives_chat_restart() {
    use lu_core::journal::{EffectJournal, EffectState};
    use lu_core::read_approval_ledger::{ReadApprovalLedger, ReadDecision};

    let root = temp_root();
    fs::create_dir_all(&root).unwrap();
    let files = root.join("external-files");
    fs::create_dir_all(&files).unwrap();
    fs::write(files.join("note.txt"), "persisted verified text").unwrap();
    let chats = ChatStore::open(&root.join("chats")).unwrap();
    let chat = chats.create_chat().unwrap();
    let turn = format!("g-{}", "3".repeat(32));
    let effects = EffectJournal::open(&root.join("effects.sqlite")).unwrap();
    let approvals = ReadApprovalLedger::open(&root.join("approvals.sqlite")).unwrap();
    approvals.stage("p-1", &chat.chat_id, "model-generation-1", "note.txt", 24).unwrap();
    approvals.decide(
        "p-1", &chat.chat_id, "model-generation-1", "note.txt",
        24, ReadDecision::Approved,
    ).unwrap();
    approvals.link_effect("p-1", &chat.chat_id, "model-generation-1", &turn).unwrap();
    let read = native_host_read::inspect_selected_file_with_receipt(
        &chats, &chat.chat_id, &turn, &files.join("note.txt"), 24, Some(&effects)
    ).await.unwrap();
    assert_eq!(read.text, "persisted verified text");
    assert_eq!(effects.get(&turn).unwrap().unwrap().state, EffectState::Applied);
    assert!(effects.get(&turn).unwrap().unwrap().receipt.is_some());

    drop(approvals);
    drop(effects);
    drop(chats);
    let approvals = ReadApprovalLedger::open(&root.join("approvals.sqlite")).unwrap();
    let effects = EffectJournal::open(&root.join("effects.sqlite")).unwrap();
    let chats = ChatStore::open(&root.join("chats")).unwrap();
    let decision = approvals.get("p-1").unwrap().unwrap();
    assert_eq!(decision.decision, ReadDecision::Approved);
    assert_eq!(decision.effect_call_id.as_deref(), Some(turn.as_str()));
    assert_eq!(effects.get(&turn).unwrap().unwrap().state, EffectState::Applied);
    assert_eq!(chats.history(&chat.chat_id).unwrap()[1].text, "persisted verified text");
    assert_eq!(effects.reconcile_interrupted().unwrap(), 0);
    drop(approvals); drop(effects); drop(chats);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn pending_read_approval_is_interrupted_after_restart_never_automatically_granted() {
    use lu_core::read_approval_ledger::{ReadApprovalLedger, ReadDecision, ReadLedgerError};
    let root = temp_root();
    fs::create_dir_all(&root).unwrap();
    let db = ReadApprovalLedger::open(&root.join("approvals.sqlite")).unwrap();
    db.stage("p-orphan", "chat-a", "generation-a", "notes.txt", 4096).unwrap();
    drop(db);
    // Called only in an isolated test after the simulated old owner is gone.
    let reopened = ReadApprovalLedger::open(&root.join("approvals.sqlite")).unwrap();
    assert_eq!(reopened.reconcile_pending().unwrap(), 1);
    assert_eq!(reopened.get("p-orphan").unwrap().unwrap().decision, ReadDecision::Interrupted);
    assert_eq!(reopened.decide(
        "p-orphan", "chat-a", "generation-a", "notes.txt", 4096,
        ReadDecision::Approved,
    ), Err(ReadLedgerError::Transition));
    drop(reopened);
    fs::remove_dir_all(root).unwrap();
}
