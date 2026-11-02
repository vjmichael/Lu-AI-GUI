use lu_core::{
    AuthorizedCapability, ConversationAuthority, TaskAuthority, ToolEffect, TrustedUserTurnVerifier,
};
use std::sync::{Arc, Mutex};

struct Verified;
impl TrustedUserTurnVerifier for Verified {
    fn verify_user_turn(&self, _: &str, turn: &str) -> bool {
        turn == "user"
    }
}
#[test]
fn task_and_conversation_revocations_are_synchronous_and_idempotent() {
    let authority = ConversationAuthority::new();
    for id in ["task-1", "task-2"] {
        authority
            .open_user_task(
                TaskAuthority {
                    conversation_id: "conversation".into(),
                    user_turn_id: "user".into(),
                    task_id: id.into(),
                    execution_scope_id: "scope".into(),
                    capabilities: vec![AuthorizedCapability {
                        tool_id: "acme.ce.run".into(),
                        effect: ToolEffect::ReadWorkspace,
                    }],
                    expires_at: None,
                },
                &Verified,
            )
            .unwrap();
    }
    let observed = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&observed);
    authority
        .on_task_revoked(Arc::new(move |task| {
            sink.lock().unwrap().push(task.to_string())
        }))
        .unwrap();
    authority.revoke_task("task-1").unwrap();
    assert_eq!(observed.lock().unwrap().as_slice(), ["task-1"]);
    assert_eq!(authority.revoke_conversation("conversation").unwrap(), 1);
    assert_eq!(observed.lock().unwrap().as_slice(), ["task-1", "task-2"]);
    assert_eq!(authority.revoke_conversation("conversation").unwrap(), 0);
}
