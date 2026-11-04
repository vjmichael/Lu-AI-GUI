//! Integration: real production authority/registry/protobuf/durable journal.
//! The test verifier stands in for the desktop persisted-user-turn verifier;
//! this alone is NOT production GUI approval or Windows crash acceptance.
use std::{fs, path::PathBuf, sync::{Arc, atomic::{AtomicU64, Ordering}},
          time::{SystemTime, UNIX_EPOCH}};
use cap_std::{ambient_authority, fs::Dir};
use lu_core::{
    contracts::{ProtoPayload, ToolEffect, ToolInvocation},
    conversation_authority::{AuthorizedCapability, BoundCallOrigin,
        ConversationAuthority, TaskAuthority, TrustedUserTurnVerifier},
    edit_safety::{bind_reviewed_edit, bind_reviewed_edit_with_host_identity, plan_text_edit},
    journal::{EffectJournal, EffectState},
    native_edit_existing::{register_edit_tools_pinned, observe_edit,
        EditObservation, ExactEditPolicy, ExactRollbackPolicy,
        EDIT_TOOL_ID, EDIT_REQUEST_TYPE, EDIT_RESULT_TYPE,
        ROLLBACK_TOOL_ID, ROLLBACK_REQUEST_TYPE, ROLLBACK_RESULT_TYPE},
    policy::{CapabilityGrant, PolicyEngine},
    registry::{DispatchError, NativeToolRegistry},
    wire::proto,
};
use prost::Message;
use sha2::{Digest, Sha256};
static NEXT: AtomicU64 = AtomicU64::new(0);
fn scratch() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "lu-edit-bound-{}-{}-{}", std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos(),
        NEXT.fetch_add(1, Ordering::Relaxed)));
    fs::create_dir_all(&path).unwrap();
    path
}
fn sha(b: &[u8]) -> String { format!("{:x}", Sha256::digest(b)) }
fn register_test_selected(
    registry:&NativeToolRegistry,dir:Arc<Dir>,file_name:&str
) {
    #[cfg(windows)]
    {
        let file=dir.open(file_name).unwrap().into_std();
        let id=lu_core::windows_file_guard::inspect_open_file(&file).unwrap();
        lu_core::native_edit_existing::register_edit_tools_pinned_selected(
            registry,dir,file_name,id
        ).unwrap();
    }
    #[cfg(not(windows))]
    {
        let _=file_name;
        register_edit_tools_pinned(registry,dir).unwrap();
    }
}
fn review_test_selected(
    plan:&lu_core::edit_safety::TextEditPlan,dir:&Dir,
    old:&[u8],new:&[u8]
)->lu_core::edit_safety::ReviewedEdit{
    #[cfg(windows)]
    {
        let file=dir.open("notes.json").unwrap().into_std();
        let id=lu_core::windows_file_guard::inspect_open_file(&file).unwrap();
        bind_reviewed_edit_with_host_identity(plan,"notes.json",old,new,Some(&id.token())).unwrap()
    }
    #[cfg(not(windows))]
    {
        let _=dir;
        bind_reviewed_edit(plan,"notes.json",old,new).unwrap()
    }
}

struct PersistedHuman;
impl TrustedUserTurnVerifier for PersistedHuman {
    fn verify_user_turn(&self, chat: &str, user_turn: &str) -> bool {
        chat == "approved-chat" &&
            matches!(user_turn, "approved-edit-turn" | "approved-rollback-turn")
    }
}
fn open_task(authority: &ConversationAuthority, policy: &PolicyEngine,
             user_turn: &str, task: &str, tool: &str) -> BoundCallOrigin {
    let origin = BoundCallOrigin {
        conversation_id: "approved-chat".into(),
        user_turn_id: user_turn.into(),
        task_id: task.into(),
    };
    authority.open_user_task(TaskAuthority {
        conversation_id: origin.conversation_id.clone(),
        user_turn_id: origin.user_turn_id.clone(),
        task_id: origin.task_id.clone(),
        execution_scope_id: task.into(),
        capabilities: vec![AuthorizedCapability {
            tool_id: tool.into(), effect: ToolEffect::WriteWorkspace
        }],
        expires_at: None,
    }, &PersistedHuman).unwrap();
    policy.grant(CapabilityGrant {
        tool_id: tool.into(), effect: ToolEffect::WriteWorkspace,
        scope_id: task.into(), expires_at: None,
    }).unwrap();
    origin
}
fn edit_call(id: &str, target: &str, original: &str,
             replacement: &[u8], reviewed: &str) -> ToolInvocation {
    ToolInvocation {
        call_id: id.into(), tool_id: EDIT_TOOL_ID.into(),
        workspace_id: Some("scope-edit-01".into()),
        input: ProtoPayload {
            type_url: EDIT_REQUEST_TYPE.into(),
            bytes: proto::EditExistingRequest {
                relative_path: target.into(),
                expected_original_sha256: original.into(),
                replacement: replacement.into(),
                reviewed_approval_sha256: reviewed.into(),
            }.encode_to_vec(),
        },
    }
}
fn rollback_call(id: &str, original_edit: &str, current: &str, original: &str)
    -> ToolInvocation {
    ToolInvocation {
        call_id: id.into(), tool_id: ROLLBACK_TOOL_ID.into(),
        workspace_id: Some("scope-rollback-01".into()),
        input: ProtoPayload {
            type_url: ROLLBACK_REQUEST_TYPE.into(),
            bytes: proto::EditRollbackRequest {
                relative_path: "notes.json".into(),
                original_edit_id: original_edit.into(),
                expected_current_sha256: current.into(),
                expected_restored_sha256: original.into(),
            }.encode_to_vec(),
        },
    }
}

#[tokio::test]
async fn one_approved_edit_durable_receipt_replay_revocation_and_newly_approved_rollback() {
    let folder = scratch();
    let old = br#"{"a":1}"#;
    let new = br#"{"a":2}"#;
    fs::write(folder.join("notes.json"), old).unwrap();
    let root = Arc::new(Dir::open_ambient_dir(&folder, ambient_authority()).unwrap());
    let registry = NativeToolRegistry::new();
    register_test_selected(&registry, Arc::clone(&root), "notes.json");
    let journal = EffectJournal::open(&folder.join("effects.sqlite")).unwrap();
    let authority = ConversationAuthority::new();
    let policy = PolicyEngine::new();
    let origin = open_task(&authority, &policy, "approved-edit-turn", "scope-edit-01", EDIT_TOOL_ID);
    let plan = plan_text_edit("notes.json", old, &sha(old), new).unwrap();
    let review = review_test_selected(&plan,&root,old,new);
    let fence = ExactEditPolicy {
        host_policy: &policy, plan: &plan, reviewed: &review,
        approved_original: old, approved_replacement: new,
    };
    let call = edit_call("g-edited-exact-file-0001", "notes.json", &sha(old),
                         new, &review.approval_sha256);
    let response = authority.dispatch_bound_durable(
        &registry, &journal, call.clone(), origin.clone(), &fence
    ).await.unwrap();
    assert_eq!(response.payload.type_url, EDIT_RESULT_TYPE);
    let result = proto::EditExistingResult::decode(response.payload.bytes.as_slice()).unwrap();
    assert_eq!(result.original_sha256, sha(old));
    assert_eq!(result.replacement_sha256, sha(new));
    assert_eq!(result.backup_id, call.call_id);
    assert!(result.committed);
    assert_eq!(fs::read(folder.join("notes.json")).unwrap(), new);
    assert_eq!(fs::read(folder.join(format!(".lu-edit-{}.orig", call.call_id))).unwrap(), old);
    assert_eq!(observe_edit(&root, &call.call_id),
        EditObservation::ReplacementAndBackupVerified);
    assert_eq!(journal.get(&call.call_id).unwrap().unwrap().state, EffectState::Applied);
    assert_eq!(authority.dispatch_bound_durable(
        &registry, &journal, call.clone(), origin.clone(), &fence).await.unwrap(), response);

    // A journaled success cannot be replayed after host task revocation.
    authority.revoke_task(&origin.task_id).unwrap();
    assert!(matches!(
        authority.dispatch_bound_durable(&registry, &journal, call.clone(), origin, &fence).await,
        Err(DispatchError::Denied(_))
    ));
    let rollback_registry=NativeToolRegistry::new();
    register_test_selected(&rollback_registry,Arc::clone(&root),"notes.json");
    let rollback_origin = open_task(&authority, &policy,
        "approved-rollback-turn", "scope-rollback-01", ROLLBACK_TOOL_ID);
    let rollback_fence = ExactRollbackPolicy {
        host_policy: &policy, expected_target: "notes.json",
        expected_original_edit_id: &call.call_id,
        expected_current_sha256: &sha(new),
        expected_restored_sha256: &sha(old),
    };
    let rollback = rollback_call("g-explicit-rollback-00001", &call.call_id, &sha(new), &sha(old));
    let restored = authority.dispatch_bound_durable(
        &rollback_registry, &journal, rollback.clone(), rollback_origin, &rollback_fence
    ).await.unwrap();
    assert_eq!(restored.payload.type_url, ROLLBACK_RESULT_TYPE);
    assert!(proto::EditRollbackResult::decode(restored.payload.bytes.as_slice()).unwrap().committed);
    assert_eq!(fs::read(folder.join("notes.json")).unwrap(), old);
    assert_eq!(journal.get(&rollback.call_id).unwrap().unwrap().state, EffectState::Applied);
    drop(root); drop(registry); drop(journal);
    fs::remove_dir_all(folder).unwrap();
}

#[tokio::test]
async fn rejected_arguments_have_no_effect_and_stale_source_is_uncertain_not_retried() {
    let folder = scratch();
    let old = br#"{"value":"first"}"#;
    let new = br#"{"value":"second"}"#;
    fs::write(folder.join("notes.json"), old).unwrap();
    let root = Arc::new(Dir::open_ambient_dir(&folder, ambient_authority()).unwrap());
    let registry = NativeToolRegistry::new();
    register_edit_tools_pinned(&registry, Arc::clone(&root)).unwrap();
    let journal_path = folder.join("effects.sqlite");
    let journal = EffectJournal::open(&journal_path).unwrap();
    let authority = ConversationAuthority::new();
    let policy = PolicyEngine::new();
    let origin = open_task(&authority, &policy, "approved-edit-turn", "scope-edit-01", EDIT_TOOL_ID);
    let plan = plan_text_edit("notes.json", old, &sha(old), new).unwrap();
    let review = bind_reviewed_edit(&plan, "notes.json", old, new).unwrap();
    let fence = ExactEditPolicy {
        host_policy: &policy, plan: &plan, reviewed: &review,
        approved_original: old, approved_replacement: new,
    };
    for (id, name, payload, approved) in [
        ("g-wrong-file-edit-00001", "other.json", new.as_slice(), review.approval_sha256.as_str()),
        ("g-wrong-bytes-edit-0001", "notes.json", b"corrupted".as_slice(), review.approval_sha256.as_str()),
        ("g-faked-approval-00001", "notes.json", new.as_slice(), "from-model"),
    ] {
        let call = edit_call(id, name, &sha(old), payload, approved);
        assert!(matches!(authority.dispatch_bound_durable(
            &registry, &journal, call.clone(), origin.clone(), &fence).await,
            Err(DispatchError::Denied(_))));
        assert!(journal.get(id).unwrap().is_none());
    }
    fs::write(folder.join("notes.json"), b"changed outside Lu").unwrap();
    let call = edit_call("g-external-edit-conflict-01", "notes.json", &sha(old),
                         new, &review.approval_sha256);
    assert!(authority.dispatch_bound_durable(
        &registry, &journal, call.clone(), origin.clone(), &fence).await.is_err());
    assert_eq!(journal.get(&call.call_id).unwrap().unwrap().state, EffectState::Uncertain);
    assert_eq!(observe_edit(&root, &call.call_id), EditObservation::NoManifest);
    assert!(authority.dispatch_bound_durable(
        &registry, &journal, call.clone(), origin.clone(), &fence).await.is_err());
    assert_eq!(fs::read(folder.join("notes.json")).unwrap(), b"changed outside Lu");
    drop(journal);
    let reopened = EffectJournal::open(&journal_path).unwrap();
    assert_eq!(reopened.get(&call.call_id).unwrap().unwrap().state, EffectState::Uncertain);
    drop(reopened); drop(root); drop(registry);
    fs::remove_dir_all(folder).unwrap();
}
