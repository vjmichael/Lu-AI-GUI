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
