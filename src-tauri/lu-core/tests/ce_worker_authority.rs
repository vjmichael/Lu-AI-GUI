use lu_core::{
    AuthorizationError, AuthorizedCapability, BoundCallOrigin, ConversationAuthority, ProtoPayload,
    TaskAuthority, ToolAuthorizer, ToolDescriptor, ToolEffect, ToolInvocation,
    TrustedUserTurnVerifier,
};

const TOOL: &str = "lu.ce.worker.probe";
const INPUT: &str = "type.googleapis.com/lu.v1.CeProbeRequest";
const OUTPUT: &str = "type.googleapis.com/lu.v1.CeProbeResult";

struct Verified;
impl TrustedUserTurnVerifier for Verified {
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
fn origin() -> BoundCallOrigin {
    BoundCallOrigin {
        conversation_id: "real-conversation".into(),
        user_turn_id: "real-user-turn".into(),
        task_id: "task-1".into(),
    }
}
fn descriptor() -> ToolDescriptor {
    ToolDescriptor {
        id: TOOL.into(),
        display_name: "CE worker diagnostic".into(),
        effect: ToolEffect::ReadWorkspace,
        input_type_url: INPUT.into(),
        output_type_url: OUTPUT.into(),
    }
}
fn invocation() -> ToolInvocation {
    ToolInvocation {
        call_id: "call-1".into(),
        tool_id: TOOL.into(),
        workspace_id: Some("workspace-1".into()),
        input: ProtoPayload {
            type_url: INPUT.into(),
            bytes: vec![],
        },
    }
}
fn authority() -> ConversationAuthority {
    let authority = ConversationAuthority::new();
    authority
        .open_user_task(
            TaskAuthority {
                conversation_id: "real-conversation".into(),
                user_turn_id: "real-user-turn".into(),
                task_id: "task-1".into(),
                execution_scope_id: "workspace-1".into(),
                capabilities: vec![AuthorizedCapability {
                    tool_id: TOOL.into(),
                    effect: ToolEffect::ReadWorkspace,
                }],
                expires_at: None,
            },
            &Verified,
        )
        .unwrap();
    authority
}

#[test]
fn worker_admission_requires_exact_origin_scope_tool_effect_and_host_policy() {
    let authority = authority();
    assert!(authority
        .authorize_worker_call(&origin(), &invocation(), &descriptor(), &Policy(true))
        .is_ok());
    assert!(authority
        .authorize_worker_call(&origin(), &invocation(), &descriptor(), &Policy(false))
        .is_err());

    let mut forged = origin();
    forged.user_turn_id = "model-said-user".into();
    assert!(authority
        .authorize_worker_call(&forged, &invocation(), &descriptor(), &Policy(true))
        .is_err());

    let mut wrong_scope = invocation();
    wrong_scope.workspace_id = Some("another-workspace".into());
    assert!(authority
        .authorize_worker_call(&origin(), &wrong_scope, &descriptor(), &Policy(true))
        .is_err());

    let mut wrong_type = invocation();
    wrong_type.input.type_url = "type.googleapis.com/lu.v1.Other".into();
    assert!(authority
        .authorize_worker_call(&origin(), &wrong_type, &descriptor(), &Policy(true))
        .is_err());

    let mut elevated = descriptor();
    elevated.effect = ToolEffect::Elevated;
    assert!(authority
        .authorize_worker_call(&origin(), &invocation(), &elevated, &Policy(true))
        .is_err());

    authority.revoke_task("task-1").unwrap();
    assert!(authority
        .authorize_worker_call(&origin(), &invocation(), &descriptor(), &Policy(true))
        .is_err());
}
