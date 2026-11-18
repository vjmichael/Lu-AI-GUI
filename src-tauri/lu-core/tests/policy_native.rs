use lu_core::{
    contracts::{ToolAuthorizer, ToolDescriptor, ToolEffect},
    native_files::register_bundled_file_tools,
    policy::{CapabilityGrant, PolicyEngine, PolicyError},
    registry::{DispatchError, NativeToolRegistry},
};
use std::{
    collections::BTreeSet,
    fs,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

fn grant(tool_id: &str, scope_id: &str, effect: ToolEffect) -> CapabilityGrant {
    CapabilityGrant {
        tool_id: tool_id.into(),
        effect,
        scope_id: scope_id.into(),
        expires_at: None,
    }
}

#[tokio::test]
async fn a_session_must_be_explicitly_granted_a_native_file_capability() {
    let dir = std::env::temp_dir().join(format!(
        "lu-policy-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("doc.txt"), "private workspace text").unwrap();

    let registry = NativeToolRegistry::new();
    let catalog = register_bundled_file_tools(&registry, &dir).unwrap();
    let allowed = BTreeSet::from(["lu_files_read_text".to_string()]);
    let call = catalog
        .encode_call(
            &allowed,
            "lu_files_read_text",
            "call-a",
            Some("chat-a"),
            r#"{"path":"doc.txt"}"#,
        )
        .unwrap();
    let policy = PolicyEngine::new();
    assert!(matches!(
        registry.dispatch(call.clone(), &policy).await,
        Err(DispatchError::Denied(_))
    ));

    assert_eq!(
        policy
            .grant(grant(
                "lu.files.read_text",
                "chat-a",
                ToolEffect::ReadWorkspace
            ))
            .unwrap(),
        1
    );
    assert!(registry.dispatch(call.clone(), &policy).await.is_ok());
    let mut other = call.clone();
    other.workspace_id = Some("chat-b".into());
    assert!(matches!(
        registry.dispatch(other, &policy).await,
        Err(DispatchError::Denied(_))
    ));
    assert_eq!(policy.revoke("lu.files.read_text", "chat-a").unwrap(), 2);
    assert!(matches!(
        registry.dispatch(call, &policy).await,
        Err(DispatchError::Denied(_))
    ));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn exact_effect_mismatch_and_expired_grants_fail_closed() {
    let policy = PolicyEngine::new();
    let effect = ToolEffect::ReadWorkspace;
    let d = ToolDescriptor {
        id: "lu.files.read_text".into(),
        display_name: "Read".into(),
        effect,
        input_type_url: "type.googleapis.com/lu.v1.ReadTextFileRequest".into(),
        output_type_url: "type.googleapis.com/lu.v1.ReadTextFileResult".into(),
    };
    let invoke = lu_core::ToolInvocation {
        call_id: "call".into(),
        tool_id: d.id.clone(),
        workspace_id: Some("session-1".into()),
        input: lu_core::ProtoPayload {
            type_url: d.input_type_url.clone(),
            bytes: vec![],
        },
    };
    assert!(policy
        .grant(CapabilityGrant {
            expires_at: Some(SystemTime::now() - Duration::from_secs(10)),
            ..grant(&d.id, "session-1", effect)
        })
        .is_ok());
    assert!(policy.authorize(&invoke, &d).is_err());

    policy
        .grant(grant(&d.id, "session-1", ToolEffect::NetworkWrite))
        .unwrap();
    assert!(policy.authorize(&invoke, &d).is_err());
    policy.grant(grant(&d.id, "session-1", effect)).unwrap();
    assert!(policy.authorize(&invoke, &d).is_ok());
    assert!(matches!(
        policy.grant(grant(&d.id, "", effect)),
        Err(PolicyError::InvalidGrant)
    ));
}

#[test]
fn scope_revocation_is_immediate_and_bumps_revision() {
    let policy = PolicyEngine::new();
    policy
        .grant(grant(
            "lu.files.read_text",
            "session-a",
            ToolEffect::ReadWorkspace,
        ))
        .unwrap();
    policy
        .grant(grant(
            "lu.files.list_directory",
            "session-a",
            ToolEffect::ReadWorkspace,
        ))
        .unwrap();
    policy
        .grant(grant(
            "lu.files.read_text",
            "session-b",
            ToolEffect::ReadWorkspace,
        ))
        .unwrap();
    assert_eq!(policy.revoke_scope("session-a").unwrap(), 4);
    assert_eq!(policy.revision(), 4);
    assert!(matches!(
        policy.grant(grant("", "session-c", ToolEffect::ReadWorkspace)),
        Err(PolicyError::InvalidGrant)
    ));
}

#[tokio::test]
async fn host_accessibility_does_not_mint_task_authority() {
    // This native tool has a real OS-accessible host-selected root.
    // Neither filesystem accessibility nor registry discovery grants a model
    // authority to read it. The host selects a root for this specific tool,
    // not an application-wide operating-environment sandbox.
    let root = std::env::temp_dir().join(format!(
        "lu-access-not-authority-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("host-file.txt"), "approved test data").unwrap();
    assert_eq!(fs::read_to_string(root.join("host-file.txt")).unwrap(), "approved test data");

    let registry = NativeToolRegistry::new();
    let catalog = register_bundled_file_tools(&registry, &root).unwrap();
    let advertised = BTreeSet::from(["lu_files_read_text".to_owned()]);
    let request = catalog.encode_call(
        &advertised,
        "lu_files_read_text",
        "read-host-file",
        Some("authorized-turn"),
        r#"{"path":"host-file.txt"}"#,
    ).unwrap();
    let policy = PolicyEngine::new();
    // OS allows access, but the authenticated task has no authority yet.
    assert!(matches!(
        registry.dispatch(request.clone(), &policy).await,
        Err(DispatchError::Denied(_))
    ));
    policy.grant(grant(
        "lu.files.read_text",
        "authorized-turn",
        ToolEffect::ReadWorkspace,
    )).unwrap();
    assert!(registry.dispatch(request.clone(), &policy).await.is_ok());

    // A distinct task cannot ride another task's approved OS capability.
    let mut unrelated = request.clone();
    unrelated.workspace_id = Some("unrelated-turn".into());
    assert!(matches!(
        registry.dispatch(unrelated, &policy).await,
        Err(DispatchError::Denied(_))
    ));
    policy.revoke_scope("authorized-turn").unwrap();
    assert!(matches!(
        registry.dispatch(request, &policy).await,
        Err(DispatchError::Denied(_))
    ));
    // Native file tools hold an opened directory capability. Explicitly
    // release both the model catalog and tool registry before deleting the
    // fixture: Windows correctly refuses removal of still-open directories.
    drop(catalog);
    drop(registry);
    fs::remove_dir_all(root).unwrap();
}
