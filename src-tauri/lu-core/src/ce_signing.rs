//! Host-pinned Ed25519 verification of a single-file Lu CE artifact.
//! The signed message includes the exact manifest bytes and its SHA-256
//! artifact digest. A package's claimed publisher is never a trust root.
//!
//! This module authenticates bytes, not execution safety. A verified artifact
//! must still pass OS sandbox and host-grant checks before it can run.

use std::{
    collections::BTreeMap,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use ed25519_dalek::{Signature, VerifyingKey};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::ce_packages::{CePackageManifest, MAX_CE_ARCHIVE_BYTES, MAX_CE_MANIFEST_BYTES};

const DOMAIN: &[u8] = b"Lu-CE-Signed-Manifest-v1\0";
const MAX_ENVELOPE_BYTES: usize = MAX_CE_MANIFEST_BYTES + 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignatureError {
    InvalidEnvelope,
    InvalidKey,
    UnknownKey,
    KeyRevoked,
    KeyExpired,
    PublisherMismatch,
    InvalidSignature,
    InvalidManifest,
    ArtifactMismatch,
    ArtifactTooLarge,
}

fn valid_key_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.bytes().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'.' | b'-' | b'_')
        })
}

fn hex_decode<const N: usize>(s: &str) -> Option<[u8; N]> {
    if s.len() != N * 2 {
        return None;
    }
    let mut out = [0u8; N];
    for (index, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&s[index * 2..index * 2 + 2], 16).ok()?;
    }
    Some(out)
}

fn only(object: &Map<String, Value>, fields: &[&str]) -> bool {
    object.len() == fields.len() && fields.iter().all(|field| object.contains_key(*field))
}

fn message(key_id: &str, manifest_json: &str) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(DOMAIN.len() + 8 + key_id.len() + manifest_json.len());
    bytes.extend_from_slice(DOMAIN);
    bytes.extend_from_slice(&(key_id.len() as u32).to_be_bytes());
    bytes.extend_from_slice(key_id.as_bytes());
    bytes.extend_from_slice(&(manifest_json.len() as u32).to_be_bytes());
    bytes.extend_from_slice(manifest_json.as_bytes());
    bytes
}

#[derive(Clone)]
struct PublisherKey {
    publisher: String,
    public_key: VerifyingKey,
    not_before: u64,
    expires_at: u64,
    revoked: bool,
}

/// Trusted-host-only publisher pin set. Keys must come from an authenticated
/// host configuration, NEVER a package archive, model, or manifest publisher.
#[derive(Default)]
pub struct CePublisherTrust {
    keys: BTreeMap<String, PublisherKey>,
}

impl CePublisherTrust {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn pin_key(
        &mut self,
        key_id: &str,
        publisher: &str,
        public_key: [u8; 32],
        not_before: u64,
        expires_at: u64,
    ) -> Result<(), SignatureError> {
        if !valid_key_id(key_id)
            || publisher.trim().is_empty()
            || publisher.len() > 128
            || not_before >= expires_at
            || self.keys.contains_key(key_id)
        {
            return Err(SignatureError::InvalidKey);
        }
        let public_key =
            VerifyingKey::from_bytes(&public_key).map_err(|_| SignatureError::InvalidKey)?;
        self.keys.insert(
            key_id.to_owned(),
            PublisherKey {
                publisher: publisher.to_owned(),
                public_key,
                not_before,
                expires_at,
                revoked: false,
            },
        );
        Ok(())
    }

    pub fn revoke_key(&mut self, key_id: &str) -> Result<(), SignatureError> {
        let key = self
            .keys
            .get_mut(key_id)
            .ok_or(SignatureError::UnknownKey)?;
        key.revoked = true;
        Ok(())
    }

    pub fn verify_at(
        &self,
        envelope_json: &str,
        artifact: &[u8],
        now: u64,
    ) -> Result<VerifiedCePackage, SignatureError> {
        if envelope_json.len() > MAX_ENVELOPE_BYTES {
            return Err(SignatureError::InvalidEnvelope);
        }
        if artifact.is_empty() || artifact.len() > MAX_CE_ARCHIVE_BYTES {
            return Err(SignatureError::ArtifactTooLarge);
        }
        let value: Value =
            serde_json::from_str(envelope_json).map_err(|_| SignatureError::InvalidEnvelope)?;
        let object = value.as_object().ok_or(SignatureError::InvalidEnvelope)?;
        if !only(
            object,
            &["format_version", "key_id", "manifest_json", "signature_hex"],
        ) || object.get("format_version").and_then(Value::as_u64) != Some(1)
        {
            return Err(SignatureError::InvalidEnvelope);
        }
        let key_id = object
            .get("key_id")
            .and_then(Value::as_str)
            .filter(|id| valid_key_id(id))
            .ok_or(SignatureError::InvalidEnvelope)?;
        let manifest_json = object
            .get("manifest_json")
            .and_then(Value::as_str)
            .filter(|s| s.len() <= MAX_CE_MANIFEST_BYTES)
            .ok_or(SignatureError::InvalidEnvelope)?;
        let signature = object
            .get("signature_hex")
            .and_then(Value::as_str)
            .and_then(hex_decode::<64>)
            .ok_or(SignatureError::InvalidEnvelope)?;
        let key = self.keys.get(key_id).ok_or(SignatureError::UnknownKey)?;
        if key.revoked {
            return Err(SignatureError::KeyRevoked);
        }
        if now < key.not_before || now >= key.expires_at {
            return Err(SignatureError::KeyExpired);
        }
        key.public_key
            .verify_strict(
                &message(key_id, manifest_json),
                &Signature::from_bytes(&signature),
            )
            .map_err(|_| SignatureError::InvalidSignature)?;
        let manifest =
            CePackageManifest::parse(manifest_json).map_err(|_| SignatureError::InvalidManifest)?;
        if manifest.publisher != key.publisher {
            return Err(SignatureError::PublisherMismatch);
        }
        let digest = format!("{:x}", Sha256::digest(artifact));
        if digest != manifest.artifact_sha256 {
            return Err(SignatureError::ArtifactMismatch);
        }
        Ok(VerifiedCePackage {
            manifest,
            key_id: key_id.to_owned(),
            signer_public_key: key.public_key.to_bytes(),
            bytes: Arc::from(artifact),
        })
    }

    pub fn verify(
        &self,
        envelope_json: &str,
        artifact: &[u8],
    ) -> Result<VerifiedCePackage, SignatureError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| SignatureError::KeyExpired)?
            .as_secs();
        self.verify_at(envelope_json, artifact, now)
    }
}

/// A successful verification is an immutable host-issued object. It does not
/// contain any user approval or authority to execute.
#[derive(Clone)]
pub struct VerifiedCePackage {
    manifest: CePackageManifest,
    key_id: String,
    signer_public_key: [u8; 32],
    bytes: Arc<[u8]>,
}

impl VerifiedCePackage {
    pub fn manifest(&self) -> &CePackageManifest {
        &self.manifest
    }
    pub fn key_id(&self) -> &str {
        &self.key_id
    }
    /// Public key of the exact signer that verified this artifact, not an ID claim.
    pub fn signer_public_key(&self) -> &[u8; 32] {
        &self.signer_public_key
    }
    pub fn artifact(&self) -> &[u8] {
        &self.bytes
    }
    pub fn artifact_sha256(&self) -> &str {
        &self.manifest.artifact_sha256
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    fn signed(artifact: &[u8]) -> (CePublisherTrust, String) {
        let signing = SigningKey::from_bytes(&[42u8; 32]);
        let mut trust = CePublisherTrust::new();
        trust
            .pin_key(
                "publisher-key",
                "Acme",
                signing.verifying_key().to_bytes(),
                100,
                1000,
            )
            .unwrap();
        let manifest = serde_json::json!({
            "format_version": 1,
            "id": "acme.hello",
            "version": "1.0.0",
            "contract_version": 1,
            "publisher": "Acme",
            "artifact_sha256": format!("{:x}", Sha256::digest(artifact)),
            "dependencies": [],
            "tools": ["acme.hello.run"]
        })
        .to_string();
        let signature = signing.sign(&message("publisher-key", &manifest));
        let envelope = serde_json::json!({
            "format_version": 1,
            "key_id": "publisher-key",
            "manifest_json": manifest,
            "signature_hex": signature.to_bytes().iter().map(|b| format!("{b:02x}")).collect::<String>()
        }).to_string();
        (trust, envelope)
    }

    #[test]
    fn signed_artifact_is_bound_to_publisher_key_digest_and_exact_manifest() {
        let (mut trust, envelope) = signed(b"real worker bytes");
        let verified = trust
            .verify_at(&envelope, b"real worker bytes", 200)
            .unwrap();
        assert_eq!(verified.manifest().id, "acme.hello");
        assert_eq!(verified.key_id(), "publisher-key");
        assert_eq!(
            trust.verify_at(&envelope, b"tampered", 200).err(),
            Some(SignatureError::ArtifactMismatch)
        );
        assert_eq!(
            trust.verify_at(&envelope, b"real worker bytes", 1000).err(),
            Some(SignatureError::KeyExpired)
        );
        trust.revoke_key("publisher-key").unwrap();
        assert_eq!(
            trust.verify_at(&envelope, b"real worker bytes", 200).err(),
            Some(SignatureError::KeyRevoked)
        );
    }

    #[test]
    fn unknown_keys_and_tampered_manifests_are_rejected() {
        let (trust, envelope) = signed(b"worker");
        let mut value: Value = serde_json::from_str(&envelope).unwrap();
        value["manifest_json"] = Value::String(
            value["manifest_json"]
                .as_str()
                .unwrap()
                .replace("Acme", "Impostor"),
        );
        assert_eq!(
            trust.verify_at(&value.to_string(), b"worker", 200).err(),
            Some(SignatureError::InvalidSignature)
        );
        value["key_id"] = Value::String("attacker-key".into());
        assert_eq!(
            trust.verify_at(&value.to_string(), b"worker", 200).err(),
            Some(SignatureError::UnknownKey)
        );
    }
}
