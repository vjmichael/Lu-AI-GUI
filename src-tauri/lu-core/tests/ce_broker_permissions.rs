use std::{sync::Arc, time::SystemTime};

use ed25519_dalek::{Signer, SigningKey};
use lu_core::wire::proto;
use lu_core::{
    AuthorizationError, AuthorizedCapability, BoundCallOrigin, CeBrokerError, CeGrantRequest,
    CePermissionStore, CePublisherKeyEnrollment, CePublisherTrust, CeResourceBroker,
    ConversationAuthority, ProtoPayload, TaskAuthority, ToolAuthorizer, ToolDescriptor, ToolEffect,
    ToolInvocation, TrustedUserTurnVerifier, VerifiedCePackage,
};
use prost::Message;
use sha2::{Digest, Sha256};

struct Verified;
impl TrustedUserTurnVerifier for Verified {
    fn verify_user_turn(&self, _: &str, turn: &str) -> bool {
        turn == "human-turn"
    }
}
struct Allow;
impl ToolAuthorizer for Allow {
    fn authorize(&self, _: &ToolInvocation, _: &ToolDescriptor) -> Result<(), AuthorizationError> {
        Ok(())
    }
}

fn package() -> VerifiedCePackage {
    let signing = SigningKey::from_bytes(&[55u8; 32]);
    let mut trust = CePublisherTrust::new();
    trust
        .pin_key(
            "test-key",
            "Publisher",
            signing.verifying_key().to_bytes(),
            0,
            u64::MAX,
        )
        .unwrap();
    let manifest = serde_json::json!({
        "format_version":1, "id":"publisher.worker", "version":"1.0.0",
        "contract_version":1, "publisher":"Publisher",
        "artifact_sha256":format!("{:x}",Sha256::digest(b"worker")),
        "dependencies":[], "tools":["publisher.worker.read","publisher.worker.write"]
    })
    .to_string();
    let mut message = b"Lu-CE-Signed-Manifest-v1\0".to_vec();
    message.extend_from_slice(&(8u32).to_be_bytes());
    message.extend_from_slice(b"test-key");
    message.extend_from_slice(&(manifest.len() as u32).to_be_bytes());
    message.extend_from_slice(manifest.as_bytes());
    let signature = signing.sign(&message);
    let envelope = serde_json::json!({
        "format_version":1, "key_id":"test-key", "manifest_json":manifest,
        "signature_hex":signature.to_bytes().iter().map(|b|format!("{b:02x}")).collect::<String>()
    })
    .to_string();
    trust.verify_at(&envelope, b"worker", 200).unwrap()
}
fn descriptor(tool: &str, effect: ToolEffect) -> ToolDescriptor {
    let (input, output) = match effect {
        ToolEffect::ReadWorkspace => ("CeReadRequest", "CeReadResult"),
        ToolEffect::WriteWorkspace => ("CeWriteRequest", "CeWriteResult"),
        _ => panic!("unexpected test effect"),
    };
    ToolDescriptor {
        id: tool.into(),
        display_name: tool.into(),
        effect,
        input_type_url: format!("type.googleapis.com/lu.v1.{input}"),
        output_type_url: format!("type.googleapis.com/lu.v1.{output}"),
    }
}
fn call(tool: &str) -> ToolInvocation {
    let (type_url, bytes) = if tool.ends_with(".read") {
        (
            "type.googleapis.com/lu.v1.CeReadRequest",
            proto::CeReadRequest {
                relative_path: "inside".into(),
                max_bytes: 65536,
            }
            .encode_to_vec(),
        )
    } else {
        (
            "type.googleapis.com/lu.v1.CeWriteRequest",
            proto::CeWriteRequest {
                relative_path: "created".into(),
                data: b"safe".to_vec(),
            }
            .encode_to_vec(),
        )
    };
    ToolInvocation {
        call_id: "call".into(),
        tool_id: tool.into(),
        workspace_id: Some("scope".into()),
        input: ProtoPayload {
            type_url: type_url.into(),
            bytes,
        },
    }
}

#[test]
fn signed_ce_broker_requires_durable_grants_and_confines_file_access() {
    let root = std::env::temp_dir().join(format!(
        "lu_broker_{}_{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for dir in [&root, &workspace] {
            std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
    }
    std::fs::write(root.join("outside-secret"), b"hidden").unwrap();
    std::fs::write(workspace.join("inside"), b"allowed").unwrap();
    let package = package();
    let permissions = CePermissionStore::open(&root.join("permissions.db")).unwrap();
    permissions
        .enroll_publisher_key(&CePublisherKeyEnrollment {
            key_id: "test-key".into(),
            publisher: "Publisher".into(),
            public_key: SigningKey::from_bytes(&[55u8; 32])
                .verifying_key()
                .to_bytes(),
            not_before: 0,
            expires_at: i64::MAX,
        })
        .unwrap();
    let authority = ConversationAuthority::new();
    let origin = BoundCallOrigin {
        conversation_id: "conversation".into(),
        user_turn_id: "human-turn".into(),
        task_id: "task".into(),
    };
    authority
        .open_user_task(
            TaskAuthority {
                conversation_id: origin.conversation_id.clone(),
                user_turn_id: origin.user_turn_id.clone(),
                task_id: origin.task_id.clone(),
                execution_scope_id: "scope".into(),
                capabilities: vec![
                    AuthorizedCapability {
                        tool_id: "publisher.worker.read".into(),
                        effect: ToolEffect::ReadWorkspace,
                    },
                    AuthorizedCapability {
                        tool_id: "publisher.worker.write".into(),
                        effect: ToolEffect::WriteWorkspace,
                    },
                ],
                expires_at: None,
            },
            &Verified,
        )
        .unwrap();
    for (tool, effect) in [
        ("publisher.worker.read", ToolEffect::ReadWorkspace),
        ("publisher.worker.write", ToolEffect::WriteWorkspace),
    ] {
        permissions
            .approve(
                &package,
                &CeGrantRequest {
                    profile_id: "profile".into(),
                    scope_id: "scope".into(),
                    tool_id: tool.into(),
                    effect,
                    approved_user_turn_id: "human-turn".into(),
                    expires_at: i64::MAX,
                },
            )
            .unwrap();
    }
    let broker = CeResourceBroker::new(
        authority.clone(),
        Arc::new(Allow),
        permissions.clone(),
        package,
        "profile".into(),
        "scope".into(),
        &workspace,
    )
    .unwrap();
    let read = descriptor("publisher.worker.read", ToolEffect::ReadWorkspace);
    let write = descriptor("publisher.worker.write", ToolEffect::WriteWorkspace);
    assert_eq!(
        broker
            .read_file(&origin, &call(&read.id), &read, 1, "inside")
            .unwrap(),
        b"allowed"
    );
    assert_eq!(
        broker.read_file(&origin, &call(&read.id), &read, 1, "inside"),
        Err(CeBrokerError::Replay)
    );
    assert_eq!(
        broker.read_file(&origin, &call(&read.id), &read, 2, "../outside-secret"),
        Err(CeBrokerError::InvalidPath)
    );
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(root.join("outside-secret"), workspace.join("escape")).unwrap();
        assert!(broker
            .read_file(&origin, &call(&read.id), &read, 3, "escape")
            .is_err());
    }
    broker
        .write_file(&origin, &call(&write.id), &write, 4, "created", b"safe")
        .unwrap();
    assert_eq!(std::fs::read(workspace.join("created")).unwrap(), b"safe");
    permissions
        .revoke("profile", "publisher.worker", "scope", Some(&read.id))
        .unwrap();
    assert_eq!(
        broker.read_file(&origin, &call(&read.id), &read, 5, "inside"),
        Err(CeBrokerError::PermissionDenied)
    );
    authority.revoke_task("task").unwrap();
    assert_eq!(
        broker.write_file(&origin, &call(&write.id), &write, 6, "created", b"denied"),
        Err(CeBrokerError::PermissionDenied)
    );
    drop(broker);
    drop(permissions);
    std::fs::remove_dir_all(root).unwrap();
}
