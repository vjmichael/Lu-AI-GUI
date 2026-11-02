use std::{sync::mpsc, thread, time::Duration};

use lu_core::{
    AuthorizationError, AuthorizedCapability, BoundCallOrigin, ConversationAuthority, ProtoPayload,
    TaskAuthority, ToolAuthorizer, ToolDescriptor, ToolEffect, ToolInvocation,
    TrustedUserTurnVerifier,
};

struct RealUser;
impl TrustedUserTurnVerifier for RealUser {
    fn verify_user_turn(&self, conversation: &str, turn: &str) -> bool {
        conversation == "real-conversation" && turn == "real-user-turn"
    }
}

struct Policy(bool);
impl ToolAuthorizer for Policy {
    fn authorize(&self, _: &ToolInvocation, _: &ToolDescriptor) -> Result<(), AuthorizationError> {
        if self.0 {
            Ok(())
        } else {
            Err(AuthorizationError::new("host denied"))
        }
    }
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor {
        id: "publisher.worker.read".into(),
        display_name: "read".into(),
        effect: ToolEffect::ReadWorkspace,
        input_type_url: "type.googleapis.com/lu.v1.CeReadRequest".into(),
        output_type_url: "type.googleapis.com/lu.v1.CeReadResult".into(),
    }
}

fn invocation() -> ToolInvocation {
    ToolInvocation {
        call_id: "call".into(),
        tool_id: "publisher.worker.read".into(),
        workspace_id: Some("scope".into()),
        input: ProtoPayload {
            type_url: "type.googleapis.com/lu.v1.CeReadRequest".into(),
            bytes: vec![],
        },
    }
}

fn origin() -> BoundCallOrigin {
    BoundCallOrigin {
        conversation_id: "real-conversation".into(),
        user_turn_id: "real-user-turn".into(),
        task_id: "task".into(),
    }
}

fn authority() -> ConversationAuthority {
    let authority = ConversationAuthority::new();
    authority
        .open_user_task(
            TaskAuthority {
                conversation_id: "real-conversation".into(),
                user_turn_id: "real-user-turn".into(),
                task_id: "task".into(),
                execution_scope_id: "scope".into(),
                capabilities: vec![AuthorizedCapability {
                    tool_id: "publisher.worker.read".into(),
                    effect: ToolEffect::ReadWorkspace,
                }],
                expires_at: None,
            },
            &RealUser,
        )
        .unwrap();
    authority
}

#[test]
fn task_and_conversation_revocation_cannot_overtake_an_inflight_host_effect() {
    for revoke_whole_conversation in [false, true] {
        let authority = authority();
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let worker_authority = authority.clone();
        let worker = thread::spawn(move || {
            worker_authority.with_authorized_worker_effect(
                &origin(),
                &invocation(),
                &descriptor(),
                &Policy(true),
                || {
                    entered_tx.send(()).unwrap();
                    release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
                    17_u32
                },
            )
        });
        entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let (started_tx, started_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        let revoker = authority.clone();
        let revocation = thread::spawn(move || {
            started_tx.send(()).unwrap();
            let result = if revoke_whole_conversation {
                revoker.revoke_conversation("real-conversation")
            } else {
                revoker.revoke_task("task").map(|()| 1)
            };
            done_tx.send(result).unwrap();
        });
        started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        // A revocation must wait for the effect lease, even if it starts
        // after the original admission check but before the effect finishes.
        assert!(done_rx.recv_timeout(Duration::from_millis(75)).is_err());
        release_tx.send(()).unwrap();
        assert_eq!(worker.join().unwrap().unwrap(), 17);
        assert_eq!(
            done_rx
                .recv_timeout(Duration::from_secs(5))
                .unwrap()
                .unwrap(),
            1
        );
        revocation.join().unwrap();
        let mut executed = false;
        assert!(authority
            .with_authorized_worker_effect(
                &origin(),
                &invocation(),
                &descriptor(),
                &Policy(true),
                || executed = true,
            )
            .is_err());
        assert!(!executed);
    }
}

#[test]
fn forged_provenance_scope_effect_type_and_host_policy_never_enter_effect() {
    let authority = authority();
    let mut forged_origin = origin();
    forged_origin.user_turn_id = "model-invented-user".into();
    let mut wrong_scope = invocation();
    wrong_scope.workspace_id = Some("other-workspace".into());
    let mut wrong_type = invocation();
    wrong_type.input.type_url = "type.googleapis.com/lu.v1.Other".into();
    let mut wrong_effect = descriptor();
    wrong_effect.effect = ToolEffect::WriteWorkspace;
    let cases = [
        (forged_origin, invocation(), descriptor(), true),
        (origin(), wrong_scope, descriptor(), true),
        (origin(), wrong_type, descriptor(), true),
        (origin(), invocation(), wrong_effect, true),
        (origin(), invocation(), descriptor(), false),
    ];
    for (origin, invocation, descriptor, policy) in cases {
        let mut executed = false;
        assert!(authority
            .with_authorized_worker_effect(
                &origin,
                &invocation,
                &descriptor,
                &Policy(policy),
                || executed = true,
            )
            .is_err());
        assert!(!executed);
    }
}
