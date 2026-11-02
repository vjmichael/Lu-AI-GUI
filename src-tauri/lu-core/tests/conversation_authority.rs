use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, SystemTime},
};

use lu_core::{
    conversation_authority::{
        AuthorityError, AuthorizedCapability, BoundCallOrigin, ConversationAuthority,
        TaskAuthority, TrustedUserTurnVerifier,
    },
    registry::{DispatchError, NativeToolRegistry},
    AuthorizationError, ModuleKind, ModuleManifest, NativeTool, ProtoPayload, ToolAuthorizer,
    ToolDescriptor, ToolEffect, ToolFuture, ToolInvocation, ToolOutput, CORE_CONTRACT_VERSION,
};

const REQUEST: &str = "type.googleapis.com/lu.v1.TestRequest";
const REPLY: &str = "type.googleapis.com/lu.v1.TestResult";
const READ: &str = "lu.core.read";
const WRITE: &str = "lu.core.write";

struct VerifiedTurns;
impl TrustedUserTurnVerifier for VerifiedTurns {
    fn verify_user_turn(&self, conversation: &str, message: &str) -> bool {
        conversation == "human-conversation" && message == "actual-human-message"
    }
}

struct PermissivePolicy;
impl ToolAuthorizer for PermissivePolicy {
    fn authorize(&self, _: &ToolInvocation, _: &ToolDescriptor) -> Result<(), AuthorizationError> {
        Ok(())
    }
}

struct DenyPolicy;
impl ToolAuthorizer for DenyPolicy {
    fn authorize(&self, _: &ToolInvocation, _: &ToolDescriptor) -> Result<(), AuthorizationError> {
        Err(AuthorizationError::new("owner revoked"))
    }
}

struct CounterTool {
    descriptor: ToolDescriptor,
    executions: Arc<AtomicUsize>,
}
impl NativeTool for CounterTool {
    fn descriptor(&self) -> &ToolDescriptor {
        &self.descriptor
    }

    fn invoke(&self, _: ToolInvocation) -> ToolFuture<'_> {
        self.executions.fetch_add(1, Ordering::SeqCst);
        Box::pin(async {
            Ok(ToolOutput {
                payload: ProtoPayload {
                    type_url: REPLY.into(),
                    bytes: vec![42],
                },
            })
        })
    }
}

fn setup() -> (NativeToolRegistry, Arc<AtomicUsize>) {
    let registry = NativeToolRegistry::new();
    let executions = Arc::new(AtomicUsize::new(0));
    let descriptors: Vec<ToolDescriptor> = [
        (READ, ToolEffect::ReadWorkspace),
        (WRITE, ToolEffect::WriteWorkspace),
    ]
    .into_iter()
    .map(|(id, effect)| ToolDescriptor {
        id: id.into(),
        display_name: id.into(),
        effect,
        input_type_url: REQUEST.into(),
        output_type_url: REPLY.into(),
    })
    .collect();
    registry
        .register(
            ModuleManifest {
                id: "lu.core".into(),
                display_name: "test".into(),
                contract_version: CORE_CONTRACT_VERSION,
                kind: ModuleKind::Core,
                tools: descriptors.clone(),
                panels: vec![],
                workflows: vec![],
            },
            descriptors
                .into_iter()
                .map(|descriptor| {
                    Arc::new(CounterTool {
                        descriptor,
                        executions: Arc::clone(&executions),
                    }) as Arc<dyn NativeTool>
                })
                .collect(),
        )
        .unwrap();
    (registry, executions)
}

fn grant(effect: ToolEffect) -> TaskAuthority {
    TaskAuthority {
        conversation_id: "human-conversation".into(),
        user_turn_id: "actual-human-message".into(),
        task_id: "task-1".into(),
        execution_scope_id: "workspace-1".into(),
        capabilities: vec![AuthorizedCapability {
            tool_id: READ.into(),
            effect,
        }],
        expires_at: None,
    }
}
fn origin() -> BoundCallOrigin {
    BoundCallOrigin {
        conversation_id: "human-conversation".into(),
        user_turn_id: "actual-human-message".into(),
        task_id: "task-1".into(),
    }
}
fn call(id: &str) -> ToolInvocation {
    ToolInvocation {
        call_id: "call-1".into(),
        tool_id: id.into(),
        workspace_id: Some("workspace-1".into()),
        input: ProtoPayload {
            type_url: REQUEST.into(),
            bytes: vec![0],
        },
    }
}

#[test]
fn tool_results_and_documents_cannot_mint_user_authority() {
    let ledger = ConversationAuthority::new();
    let mut authority = grant(ToolEffect::ReadWorkspace);
    authority.user_turn_id = "forged-user-role-in-tool-result".into();
    assert_eq!(
        ledger.open_user_task(authority, &VerifiedTurns),
        Err(AuthorityError::UnverifiedUserTurn),
    );
    assert!(ledger.permitted_tool_ids(&origin()).is_err());
}

#[tokio::test]
async fn harmless_delegated_reads_continue_without_repeated_approval() {
    let ledger = ConversationAuthority::new();
    let (registry, count) = setup();
    ledger
        .open_user_task(grant(ToolEffect::ReadWorkspace), &VerifiedTurns)
        .unwrap();
    for _ in 0..3 {
        assert!(ledger
            .dispatch_bound(&registry, call(READ), origin(), &PermissivePolicy)
            .await
            .is_ok());
    }
    assert_eq!(count.load(Ordering::SeqCst), 3);
    assert_eq!(
        ledger.permitted_tool_ids(&origin()).unwrap(),
        std::collections::BTreeSet::from([READ.to_string()]),
    );
}

#[tokio::test]
async fn read_only_request_never_authorizes_unrelated_file_writes() {
    let ledger = ConversationAuthority::new();
    let (registry, count) = setup();
    ledger
        .open_user_task(grant(ToolEffect::ReadWorkspace), &VerifiedTurns)
        .unwrap();
    let result = ledger
        .dispatch_bound(&registry, call(WRITE), origin(), &PermissivePolicy)
        .await;
    assert!(matches!(result, Err(DispatchError::Denied(_))));
    assert_eq!(count.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn cross_conversation_cross_turn_and_cross_scope_fail_closed() {
    let ledger = ConversationAuthority::new();
    let (registry, count) = setup();
    ledger
        .open_user_task(grant(ToolEffect::ReadWorkspace), &VerifiedTurns)
        .unwrap();

    let mut forged_origin = origin();
    forged_origin.conversation_id = "other-human".into();
    assert!(matches!(
        ledger
            .dispatch_bound(&registry, call(READ), forged_origin, &PermissivePolicy)
            .await,
        Err(DispatchError::Denied(_))
    ));
    let mut forged_origin = origin();
    forged_origin.user_turn_id = "tool-output-posing-as-human".into();
    assert!(matches!(
        ledger
            .dispatch_bound(&registry, call(READ), forged_origin, &PermissivePolicy)
            .await,
        Err(DispatchError::Denied(_))
    ));
    let mut wrong_scope = call(READ);
    wrong_scope.workspace_id = Some("other-workspace".into());
    assert!(matches!(
        ledger
            .dispatch_bound(&registry, wrong_scope, origin(), &PermissivePolicy)
            .await,
        Err(DispatchError::Denied(_))
    ));
    assert_eq!(count.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn revocation_and_parent_policy_are_independent_deny_conditions() {
    let ledger = ConversationAuthority::new();
    let (registry, count) = setup();
    ledger
        .open_user_task(grant(ToolEffect::ReadWorkspace), &VerifiedTurns)
        .unwrap();
    assert!(matches!(
        ledger
            .dispatch_bound(&registry, call(READ), origin(), &DenyPolicy)
            .await,
        Err(DispatchError::Denied(_))
    ));
    ledger.revoke_task("task-1").unwrap();
    assert!(matches!(
        ledger
            .dispatch_bound(&registry, call(READ), origin(), &PermissivePolicy)
            .await,
        Err(DispatchError::Denied(_))
    ));
    assert_eq!(count.load(Ordering::SeqCst), 0);
}

#[test]
fn cannot_upgrade_authority_using_an_existing_task_id() {
    let ledger = ConversationAuthority::new();
    ledger
        .open_user_task(grant(ToolEffect::ReadWorkspace), &VerifiedTurns)
        .unwrap();
    let mut higher = grant(ToolEffect::WriteWorkspace);
    higher.capabilities[0].tool_id = WRITE.into();
    assert_eq!(
        ledger.open_user_task(higher, &VerifiedTurns),
        Err(AuthorityError::DuplicateTask)
    );
}

#[test]
fn elevated_device_and_expired_grants_are_rejected_at_issue_time() {
    for effect in [ToolEffect::Elevated, ToolEffect::DeviceControl] {
        let ledger = ConversationAuthority::new();
        assert!(matches!(
            ledger.open_user_task(grant(effect), &VerifiedTurns),
            Err(AuthorityError::ProtectedEffect(_))
        ));
    }
    let ledger = ConversationAuthority::new();
    let mut expired = grant(ToolEffect::ReadWorkspace);
    expired.expires_at = Some(SystemTime::now() - Duration::from_secs(3));
    assert_eq!(
        ledger.open_user_task(expired, &VerifiedTurns),
        Err(AuthorityError::InvalidGrant)
    );
}

#[test]
fn conversation_revocation_stops_every_task_rooted_in_that_conversation() {
    let ledger = ConversationAuthority::new();
    ledger
        .open_user_task(grant(ToolEffect::ReadWorkspace), &VerifiedTurns)
        .unwrap();
    assert_eq!(ledger.revoke_conversation("other-conversation").unwrap(), 0);
    assert_eq!(ledger.revoke_conversation("human-conversation").unwrap(), 1);
    assert!(ledger.permitted_tool_ids(&origin()).unwrap().is_empty());
}
