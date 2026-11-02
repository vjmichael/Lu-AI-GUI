//! Alpha-support contract regressions: synthetic, not Windows process-kill proof.
use lu_core::{
    contracts::{ToolEffect, CORE_CONTRACT_VERSION},
    conversation_authority::{
        AuthorityError, AuthorizedCapability, BoundCallOrigin, ConversationAuthority,
        TaskAuthority, TrustedUserTurnVerifier,
    },
    inference_host::Cancellation,
    wire::{self, proto, WireError},
};
use prost::Message;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

struct SavedUser;
impl TrustedUserTurnVerifier for SavedUser {
    fn verify_user_turn(&self, conversation: &str, turn: &str) -> bool {
        conversation == "saved-conversation" && turn == "saved-user-turn"
    }
}
fn grant() -> TaskAuthority {
    TaskAuthority {
        conversation_id: "saved-conversation".into(),
        user_turn_id: "saved-user-turn".into(),
        task_id: "task-alpha".into(),
        execution_scope_id: "workspace-alpha".into(),
        capabilities: vec![AuthorizedCapability {
            tool_id: "lu.files.read_text".into(),
            effect: ToolEffect::ReadWorkspace,
        }],
        expires_at: None,
    }
}
fn origin() -> BoundCallOrigin {
    BoundCallOrigin {
        conversation_id: "saved-conversation".into(),
        user_turn_id: "saved-user-turn".into(),
        task_id: "task-alpha".into(),
    }
}

#[test]
fn invalid_conversation_turn_task_and_scope_ids_never_create_authority() {
    for bad in ["", " \t ", "nul\0byte", &"x".repeat(129)] {
        for dimension in 0..4 {
            let mut invalid = grant();
            match dimension {
                0 => invalid.conversation_id = bad.into(),
                1 => invalid.user_turn_id = bad.into(),
                2 => invalid.task_id = bad.into(),
                _ => invalid.execution_scope_id = bad.into(),
            }
            let ledger = ConversationAuthority::new();
            assert!(matches!(
                ledger.open_user_task(invalid, &SavedUser),
                Err(AuthorityError::InvalidIdentifier)
            ));
        }
    }
    let ledger = ConversationAuthority::new();
    let mut fabricated = grant();
    fabricated.user_turn_id = "tool-reply-cannot-authorize".into();
    assert!(matches!(
        ledger.open_user_task(fabricated, &SavedUser),
        Err(AuthorityError::UnverifiedUserTurn)
    ));
}

#[test]
fn revocation_clears_tool_catalog_and_signals_inflight_cancellation() {
    let ledger = ConversationAuthority::new();
    let token = Cancellation::default();
    let notified = Arc::new(AtomicUsize::new(0));
    let cancel_copy = token.clone();
    let observed = notified.clone();
    ledger
        .on_task_revoked(Arc::new(move |task| {
            assert_eq!(task, "task-alpha");
            observed.fetch_add(1, Ordering::SeqCst);
            cancel_copy.cancel();
        }))
        .unwrap();
    ledger.open_user_task(grant(), &SavedUser).unwrap();
    assert_eq!(ledger.permitted_tool_ids(&origin()).unwrap().len(), 1);
    assert!(!token.is_cancelled());
    ledger.revoke_task("task-alpha").unwrap();
    assert_eq!(notified.load(Ordering::SeqCst), 1);
    assert!(token.is_cancelled());
    assert!(token.clone().is_cancelled());
    assert!(ledger.permitted_tool_ids(&origin()).unwrap().is_empty());
    // Revoked task ID cannot be reused to regain an authority.
    assert!(matches!(
        ledger.open_user_task(grant(), &SavedUser),
        Err(AuthorityError::DuplicateTask)
    ));
}

fn call() -> proto::ToolCall {
    proto::ToolCall {
        contract_version: u32::from(CORE_CONTRACT_VERSION),
        call_id: "call-alpha".into(),
        tool_id: "lu.files.read_text".into(),
        workspace_id: Some("workspace-alpha".into()),
        input_type_url: "type.googleapis.com/lu.v1.LegacyJsonArguments".into(),
        input: vec![],
    }
}

#[test]
fn malformed_or_unversioned_protobuf_frames_fail_before_dispatch() {
    for bad in [vec![0xff], vec![0x08, 0x80], vec![0x12, 0x80]] {
        assert!(matches!(
            wire::decode_invocation(&bad),
            Err(WireError::Decode(_))
        ));
    }
    let mut missing = call();
    missing.call_id = String::new();
    assert_eq!(
        wire::decode_invocation(&missing.encode_to_vec()),
        Err(WireError::MissingField("call_id"))
    );
    let mut wrong = call();
    wrong.contract_version = 999;
    assert_eq!(
        wire::decode_invocation(&wrong.encode_to_vec()),
        Err(WireError::UnsupportedContractVersion(999))
    );
    let mut invalid_type = call();
    invalid_type.input_type_url = "not-an-allowed-type".into();
    assert_eq!(
        wire::decode_invocation(&invalid_type.encode_to_vec()),
        Err(WireError::InvalidTypeUrl)
    );
    assert_eq!(
        wire::decode_invocation(&vec![0u8; wire::MAX_CALL_BYTES + 1]),
        Err(WireError::Oversized)
    );
}

#[test]
fn cancellation_clone_is_host_signalled_not_a_model_permission_grant() {
    let original = Cancellation::default();
    let worker = original.clone();
    assert!(!worker.is_cancelled());
    original.cancel();
    assert!(worker.is_cancelled());
    // Cancellation alone cannot authorize any tool or open a conversation.
    assert!(matches!(
        ConversationAuthority::new().permitted_tool_ids(&origin()),
        Err(AuthorityError::UnknownTask)
    ));
}
