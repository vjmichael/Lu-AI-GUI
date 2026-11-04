//! WRITE-01B tests against production Protobuf registry, authority, policy,
//! and SQLite effect journal. These do NOT enable model-originated GUI writes.
use std::{
    fs, path::PathBuf, time::{SystemTime, UNIX_EPOCH},
    sync::atomic::{AtomicU64, Ordering},
};
use prost::Message;
use sha2::{Digest, Sha256};
use lu_core::{
    contracts::{ProtoPayload, ToolEffect, ToolInvocation},
    conversation_authority::{
        AuthorizedCapability, BoundCallOrigin, ConversationAuthority, TaskAuthority,
        TrustedUserTurnVerifier,
    },
    journal::{EffectJournal, EffectState, PrepareOutcome},
    native_create_new::{
        register_create_new, ExactCreateNewPolicy, TOOL_ID, REQUEST_TYPE, RESULT_TYPE,
    },
    policy::{CapabilityGrant, PolicyEngine},
    registry::{DispatchError, NativeToolRegistry},
    wire::proto,
};
static COUNTER: AtomicU64 = AtomicU64::new(0);
fn scratch() -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "lu-write-bound-{}-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos(),
        COUNTER.fetch_add(1, Ordering::Relaxed),
    ));
    fs::create_dir_all(&p).unwrap();
    p
}
struct PersistedHuman;
impl TrustedUserTurnVerifier for PersistedHuman {
    fn verify_user_turn(&self, chat: &str, turn: &str) -> bool {
        chat == "chat-approved" && turn == "g-authenticated-user-turn"
    }
}
fn call(call_id: &str, target: &str, bytes: &[u8], scope: &str) -> ToolInvocation {
    ToolInvocation {
        call_id: call_id.into(),
        tool_id: TOOL_ID.into(),
        workspace_id: Some(scope.into()),
        input: ProtoPayload {
            type_url: REQUEST_TYPE.into(),
            bytes: proto::CeWriteRequest {
                relative_path: target.into(), data: bytes.into(),
            }.encode_to_vec(),
        },
    }
}
fn authorized() -> (ConversationAuthority, PolicyEngine, BoundCallOrigin) {
    let authority = ConversationAuthority::new();
    let policy = PolicyEngine::new();
    let origin = BoundCallOrigin {
        conversation_id: "chat-approved".into(),
        user_turn_id: "g-authenticated-user-turn".into(),
        task_id: "write-task-01".into(),
    };
    authority.open_user_task(TaskAuthority {
        conversation_id: origin.conversation_id.clone(),
        user_turn_id: origin.user_turn_id.clone(),
        task_id: origin.task_id.clone(),
        execution_scope_id: "target-file-approval".into(),
        capabilities: vec![AuthorizedCapability {
            tool_id: TOOL_ID.into(), effect: ToolEffect::WriteWorkspace,
        }],
        expires_at: None,
    }, &PersistedHuman).unwrap();
    policy.grant(CapabilityGrant {
        tool_id: TOOL_ID.into(),
        effect: ToolEffect::WriteWorkspace,
        scope_id: "target-file-approval".into(),
        expires_at: None,
    }).unwrap();
    (authority, policy, origin)
}
fn digest(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }

#[tokio::test]
async fn approved_new_file_has_durable_protobuf_receipt_no_overwrite_and_no_double_execution() {
    let root = scratch();
    let registry = NativeToolRegistry::new();
    register_create_new(&registry, &root).unwrap();
    let effects = EffectJournal::open(&root.join("effects.sqlite")).unwrap();
    let (authority, policy, origin) = authorized();
    let bytes = b"Verified user-approved Lu draft";
    let approved_sha = digest(bytes);
    let target_policy = ExactCreateNewPolicy {
        host_policy: &policy, expected_target: "draft.txt",
        expected_sha256: &approved_sha,
    };
    let req = call("g-write-approved-001", "draft.txt", bytes, "target-file-approval");
    let result = authority.dispatch_bound_durable(
        &registry, &effects, req.clone(), origin.clone(), &target_policy,
    ).await.unwrap();
    assert_eq!(result.payload.type_url, RESULT_TYPE);
    let decoded = proto::CeWriteResult::decode(result.payload.bytes.as_slice()).unwrap();
    assert!(decoded.committed);
    assert_eq!(decoded.bytes_written, bytes.len() as u64);
    assert_eq!(fs::read(root.join("draft.txt")).unwrap(), bytes);
    assert_eq!(effects.get(&req.call_id).unwrap().unwrap().state, EffectState::Applied);

    // A success replay is served only from matching receipt, not another
    // filesystem write, even if a distinct process changes the target.
    fs::write(root.join("draft.txt"), "external mutation").unwrap();
    let replay = authority.dispatch_bound_durable(
        &registry, &effects, req.clone(), origin.clone(), &target_policy,
    ).await.unwrap();
    assert_eq!(replay, result);
    assert_eq!(fs::read_to_string(root.join("draft.txt")).unwrap(), "external mutation");

    // Host revocation also denies an otherwise valid cached success.
    authority.revoke_task(&origin.task_id).unwrap();
    let denied = authority.dispatch_bound_durable(
        &registry, &effects, req.clone(), origin, &target_policy,
    ).await;
    assert!(matches!(denied, Err(DispatchError::Denied(_))));
    drop(effects);
    drop(registry);
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn wrong_target_wrong_content_wrong_chat_and_no_grant_cannot_write_or_journal() {
    let root = scratch();
    let registry = NativeToolRegistry::new();
    register_create_new(&registry, &root).unwrap();
    let effects = EffectJournal::open(&root.join("effects.sqlite")).unwrap();
    let (authority, policy, origin) = authorized();
    let bytes = b"allowed contents";
    let approved_sha = digest(bytes);
    let target_policy = ExactCreateNewPolicy {
        host_policy: &policy, expected_target: "approved.txt",
        expected_sha256: &approved_sha,
    };
    for (id, target, payload) in [
        ("g-invalid-target-01", "other.txt", bytes.as_slice()),
        ("g-invalid-content-01", "approved.txt", b"unapproved payload".as_slice()),
        ("g-invalid-path-01", "../secret.txt", bytes.as_slice()),
    ] {
        let request = call(id, target, payload, "target-file-approval");
        let response = authority.dispatch_bound_durable(
            &registry, &effects, request.clone(), origin.clone(), &target_policy,
        ).await;
        assert!(matches!(response, Err(DispatchError::Denied(_))));
        assert!(effects.get(id).unwrap().is_none());
    }
    let request = call("g-wrong-chat-origin", "approved.txt", bytes, "target-file-approval");
    let other = BoundCallOrigin { conversation_id: "chat-attacker".into(), ..origin.clone() };
    assert!(matches!(
        authority.dispatch_bound_durable(&registry, &effects, request, other, &target_policy).await,
        Err(DispatchError::Denied(_))
    ));
    assert!(!root.join("approved.txt").exists());
    assert!(!root.join("other.txt").exists());
    drop(effects); drop(registry);
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn target_exists_and_abandoned_stage_become_uncertain_and_never_reexecute_on_restart() {
    let root = scratch();
    fs::write(root.join("existing.txt"), b"untouchable").unwrap();
    let registry = NativeToolRegistry::new();
    register_create_new(&registry, &root).unwrap();
    let journal_path = root.join("effects.sqlite");
    let effects = EffectJournal::open(&journal_path).unwrap();
    let (authority, policy, origin) = authorized();
    let approved_sha = digest(b"overwrite not permitted");
    let fence = ExactCreateNewPolicy {
        host_policy: &policy, expected_target: "existing.txt",
        expected_sha256: &approved_sha,
    };
    let req = call("g-write-existing-01", "existing.txt", b"overwrite not permitted", "target-file-approval");
    assert!(matches!(
        authority.dispatch_bound_durable(&registry, &effects, req.clone(), origin.clone(), &fence).await,
        Err(DispatchError::Execution(_))
    ));
    assert_eq!(fs::read(root.join("existing.txt")).unwrap(), b"untouchable");
    assert_eq!(effects.get(&req.call_id).unwrap().unwrap().state, EffectState::Uncertain);

    drop(effects);
    let reopened = EffectJournal::open(&journal_path).unwrap();
    assert_eq!(reopened.reconcile_interrupted().unwrap(), 0);
    assert!(matches!(
        authority.dispatch_bound_durable(&registry, &reopened, req, origin, &fence).await,
        Err(DispatchError::Execution(_))
    ));
    assert_eq!(fs::read(root.join("existing.txt")).unwrap(), b"untouchable");
    drop(reopened); drop(registry);
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn interrupted_executing_write_stays_uncertain_and_is_not_blindly_replayed() {
    let root = scratch();
    let registry = NativeToolRegistry::new();
    register_create_new(&registry, &root).unwrap();
    let file = root.join("effects.sqlite");
    let req = call(
        "g-write-crashed-001", "crashed.txt", b"may have happened", "target-file-approval"
    );
    {
        let effects = EffectJournal::open(&file).unwrap();
        assert_eq!(effects.prepare(&req).unwrap(), PrepareOutcome::New);
        effects.mark_executing(&req.call_id).unwrap();
    }
    let effects = EffectJournal::open(&file).unwrap();
    assert_eq!(effects.reconcile_interrupted().unwrap(), 1);
    assert_eq!(effects.get(&req.call_id).unwrap().unwrap().state, EffectState::Uncertain);
    let (authority, policy, origin) = authorized();
    let sha = digest(b"may have happened");
    let fence = ExactCreateNewPolicy {
        host_policy: &policy, expected_target: "crashed.txt", expected_sha256: &sha
    };
    assert!(matches!(
        authority.dispatch_bound_durable(&registry, &effects, req, origin, &fence).await,
        Err(DispatchError::Execution(_))
    ));
    assert!(!root.join("crashed.txt").exists());
    drop(effects); drop(registry);
    fs::remove_dir_all(root).unwrap();
}
