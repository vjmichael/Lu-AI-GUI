//! Host-owned CE approval boundary. This is NOT a webview/Tauri command or
//! an approval UI. A trusted desktop adapter must verify real human consent
//! using its own one-use gesture/event records and exact immutable prompt.
//! Neither an LLM nor a package can implement an approved source.

use std::time::Instant;

use crate::{
    ce_permissions::{CeGrantRequest, CePermissionError, CePermissionStore},
    ce_signing::VerifiedCePackage,
    contracts::ToolEffect,
    conversation_authority::TrustedUserTurnVerifier,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CeApprovalPrompt {
    pub conversation_id: String,
    pub user_turn_id: String,
    pub profile_id: String,
    pub scope_id: String,
    pub package_id: String,
    pub artifact_digest: String,
    pub signer_key_id: String,
    pub publisher: String,
    pub tool_id: String,
    pub effect: ToolEffect,
    pub expires_at: i64,
}

/// Implement ONLY in Lu's trusted desktop host backed by its authenticated,
/// one-use UI approval store. The adapter must atomically consume a user
/// confirmation matching EVERY field of the displayed prompt; model output,
/// untrusted webview requests and manifest text do not create approvals.
pub trait TrustedCeApprovalSource: Send + Sync {
    /// Consume exactly once and return the host-owned monotonic expiry.
    fn consume_exact_confirmation(&self, prompt: &CeApprovalPrompt) -> Option<Instant>;
}

pub struct CeApprovalGate<'a> {
    pub permissions: &'a CePermissionStore,
    pub turns: &'a dyn TrustedUserTurnVerifier,
    pub confirmations: &'a dyn TrustedCeApprovalSource,
}

impl CeApprovalGate<'_> {
    /// Host-only entry point. A real USER turn and a separately captured
    /// exact UI decision are both required before a durable grant is written.
    pub fn approve_confirmed(
        &self,
        conversation_id: &str,
        package: &VerifiedCePackage,
        grant: &CeGrantRequest,
    ) -> Result<(), CePermissionError> {
        if conversation_id.trim().is_empty()
            || conversation_id.len() > 128
            || !self
                .turns
                .verify_user_turn(conversation_id, &grant.approved_user_turn_id)
        {
            return Err(CePermissionError::Denied);
        }
        let prompt = CeApprovalPrompt {
            conversation_id: conversation_id.into(),
            user_turn_id: grant.approved_user_turn_id.clone(),
            profile_id: grant.profile_id.clone(),
            scope_id: grant.scope_id.clone(),
            package_id: package.manifest().id.clone(),
            artifact_digest: package.artifact_sha256().into(),
            signer_key_id: package.key_id().into(),
            publisher: package.manifest().publisher.clone(),
            tool_id: grant.tool_id.clone(),
            effect: grant.effect,
            expires_at: grant.expires_at,
        };
        let deadline = self
            .confirmations
            .consume_exact_confirmation(&prompt)
            .ok_or(CePermissionError::Denied)?;
        self.permissions
            .approve_with_deadline(package, grant, deadline)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ce_permissions::CePublisherKeyEnrollment, ce_signing::CePublisherTrust};
    use ed25519_dalek::{Signer, SigningKey};
    use sha2::{Digest, Sha256};
    use std::{
        sync::{mpsc, Mutex},
        thread,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    struct Turn;
    impl TrustedUserTurnVerifier for Turn {
        fn verify_user_turn(&self, conversation: &str, turn: &str) -> bool {
            conversation == "conversation" && turn == "real-human-turn"
        }
    }

    #[derive(Default)]
    struct OneUseUi {
        approvals: Mutex<Vec<(CeApprovalPrompt, Instant)>>,
    }
    impl TrustedCeApprovalSource for OneUseUi {
        fn consume_exact_confirmation(&self, prompt: &CeApprovalPrompt) -> Option<Instant> {
            let mut approvals = self.approvals.lock().ok()?;
            let position = approvals.iter().position(|(record, _)| record == prompt)?;
            Some(approvals.remove(position).1)
        }
    }

    struct SignaledUi {
        approval: Mutex<Option<(CeApprovalPrompt, Instant)>>,
        signal: mpsc::Sender<()>,
    }
    impl TrustedCeApprovalSource for SignaledUi {
        fn consume_exact_confirmation(&self, prompt: &CeApprovalPrompt) -> Option<Instant> {
            let (expected, deadline) = self.approval.lock().ok()?.take()?;
            if &expected != prompt {
                return None;
            }
            let _ = self.signal.send(());
            Some(deadline)
        }
    }

    #[test]
    fn approval_requires_verified_turn_and_exact_one_use_host_confirmation() {
        let root = std::env::temp_dir().join(format!(
            "lu_approval_{}_{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        let key = SigningKey::from_bytes(&[76u8; 32]);
        let permissions = CePermissionStore::open(&root.join("grants.db")).unwrap();
        permissions
            .enroll_publisher_key(&CePublisherKeyEnrollment {
                key_id: "key".into(),
                publisher: "Publisher".into(),
                public_key: key.verifying_key().to_bytes(),
                not_before: 0,
                expires_at: i64::MAX,
            })
            .unwrap();
        let manifest = serde_json::json!({
            "format_version":1,"id":"publisher.extension","version":"1.0.0",
            "contract_version":1,"publisher":"Publisher",
            "artifact_sha256":format!("{:x}",Sha256::digest(b"worker")),
            "dependencies":[],"tools":["publisher.extension.read"]
        })
        .to_string();
        let mut message = b"Lu-CE-Signed-Manifest-v1\0".to_vec();
        message.extend_from_slice(&(3u32).to_be_bytes());
        message.extend_from_slice(b"key");
        message.extend_from_slice(&(manifest.len() as u32).to_be_bytes());
        message.extend_from_slice(manifest.as_bytes());
        let signature = key.sign(&message);
        let envelope = serde_json::json!({
            "format_version":1,"key_id":"key","manifest_json":manifest,
            "signature_hex":signature.to_bytes().iter()
                .map(|b|format!("{b:02x}")).collect::<String>()
        })
        .to_string();
        let package = permissions
            .verify_signed_package(&envelope, b"worker")
            .unwrap();
        let grant = CeGrantRequest {
            profile_id: "profile".into(),
            scope_id: "scope".into(),
            tool_id: "publisher.extension.read".into(),
            effect: ToolEffect::ReadWorkspace,
            approved_user_turn_id: "real-human-turn".into(),
            expires_at: i64::MAX,
        };
        let decisions = OneUseUi::default();
        let gate = CeApprovalGate {
            permissions: &permissions,
            turns: &Turn,
            confirmations: &decisions,
        };
        assert_eq!(
            gate.approve_confirmed("conversation", &package, &grant),
            Err(CePermissionError::Denied)
        );
        let prompt = CeApprovalPrompt {
            conversation_id: "conversation".into(),
            user_turn_id: "real-human-turn".into(),
            profile_id: "profile".into(),
            scope_id: "scope".into(),
            package_id: "publisher.extension".into(),
            artifact_digest: package.artifact_sha256().into(),
            signer_key_id: "key".into(),
            publisher: "Publisher".into(),
            tool_id: grant.tool_id.clone(),
            effect: grant.effect,
            expires_at: grant.expires_at,
        };
        decisions
            .approvals
            .lock()
            .unwrap()
            .push((prompt.clone(), Instant::now() + Duration::from_secs(120)));
        assert_eq!(
            gate.approve_confirmed("conversation-fake", &package, &grant),
            Err(CePermissionError::Denied)
        );
        gate.approve_confirmed("conversation", &package, &grant)
            .unwrap();
        assert_eq!(
            gate.approve_confirmed("conversation", &package, &grant),
            Err(CePermissionError::Denied)
        );
        // A native Yes is consumed while another SQLite connection holds
        // the writer lease. It must not commit after its prompt TTL.
        let queued_grant = CeGrantRequest {
            scope_id: "queued-scope".into(),
            ..grant.clone()
        };
        let queued_prompt = CeApprovalPrompt {
            scope_id: "queued-scope".into(),
            ..prompt.clone()
        };
        let blocker = rusqlite::Connection::open(root.join("grants.db")).unwrap();
        blocker.execute_batch("BEGIN IMMEDIATE").unwrap();
        let (consumed_tx, consumed_rx) = mpsc::channel();
        let decision = SignaledUi {
            approval: Mutex::new(Some((
                queued_prompt,
                Instant::now() + Duration::from_millis(250),
            ))),
            signal: consumed_tx,
        };
        let queued_permissions = permissions.clone();
        let queued_package = package.clone();
        let attempt = thread::spawn(move || {
            let gate = CeApprovalGate {
                permissions: &queued_permissions,
                turns: &Turn,
                confirmations: &decision,
            };
            gate.approve_confirmed("conversation", &queued_package, &queued_grant)
        });
        consumed_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        thread::sleep(Duration::from_millis(350));
        blocker.execute_batch("COMMIT").unwrap();
        assert_eq!(attempt.join().unwrap(), Err(CePermissionError::Denied));
        assert_eq!(
            permissions.require(
                &package,
                "profile",
                "queued-scope",
                "publisher.extension.read",
                ToolEffect::ReadWorkspace
            ),
            Err(CePermissionError::Denied)
        );
        drop(blocker);
        drop(permissions);
        std::fs::remove_dir_all(root).unwrap();
        // Ensure ordinary signing verifier remains independent of consent.
        let mut bare = CePublisherTrust::new();
        bare.pin_key(
            "key",
            "Publisher",
            key.verifying_key().to_bytes(),
            0,
            u64::MAX,
        )
        .unwrap();
        assert!(bare.verify(&envelope, b"worker").is_ok());
    }
}
