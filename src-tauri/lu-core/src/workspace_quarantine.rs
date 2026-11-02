//! Untrusted workspace/DLC intake ("Sandbox Hell").
//!
//! External archives are DATA, not loadable modules. This staging service may
//! inspect an archive's identity and digest, but NEVER extracts, imports,
//! executes, links, registers tools, or modifies the active WorkspaceHost.
//!
//! Important limitations:
//! * Quarantine is NOT an OS sandbox or a malware scanner.
//! * SHA-256 gives integrity/identity, not publisher authenticity or safety.
//! * Human review cannot bless arbitrary native code into Lu's own process.
//! * Future promotion requires separately implemented publisher verification,
//!   archive validation, non-admin OS worker isolation, scoped egress, policy,
//!   conversation-bound approval, worker cancellation and audit trails.
//!
//! Until those exist, external packages CANNOT leave quarantine to execute.

use std::{
    collections::BTreeMap,
    sync::{Arc, RwLock},
};

use sha2::{Digest, Sha256};

/// Keep untrusted, potentially hostile archive bytes bounded. No decompression
/// happens here (so there is no archive traversal or zip-bomb attack surface).
pub const MAX_STAGED_ARCHIVE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntakeStatus {
    Quarantined,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuarantineError {
    InvalidPackageId,
    ArchiveEmpty,
    ArchiveTooLarge,
    DuplicatePackageId,
    UnknownPackageId,
    AlreadyRejected,
    DigestMismatch,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntakeReceipt {
    /// Untrusted claimed package identifier, not a verified publisher.
    pub package_id: String,
    pub sha256: String,
    pub byte_length: usize,
    pub status: IntakeStatus,
    pub reason: Option<String>,
}

#[derive(Clone, Default)]
pub struct WorkspaceQuarantine {
    entries: Arc<RwLock<BTreeMap<String, IntakeReceipt>>>,
}

fn valid_package_id(s: &str) -> bool {
    // Use a restricted, human-readable namespace, never an archive path.
    !s.is_empty()
        && s.len() <= 128
        && s.contains('.')
        && s.split('.').all(|part| {
            !part.is_empty()
                && part.len() <= 64
                && part.bytes().enumerate().all(|(index, byte)| {
                    byte.is_ascii_lowercase()
                        || (index > 0 && (byte.is_ascii_digit() || byte == b'-' || byte == b'_'))
                })
        })
}

impl WorkspaceQuarantine {
    pub fn new() -> Self {
        Self::default()
    }

    /// The complete untrusted archive is accepted as inert bytes.
    ///
    /// IMPORTANT: This in-memory receipt is not a persistent artifact store.
    /// Production intake must first store archive bytes in a restricted,
    /// non-executable quarantine directory and persist its digest/receipt.
    pub fn stage(
        &self,
        claimed_id: &str,
        archive: &[u8],
    ) -> Result<IntakeReceipt, QuarantineError> {
        if !valid_package_id(claimed_id) {
            return Err(QuarantineError::InvalidPackageId);
        }
        if archive.is_empty() {
            return Err(QuarantineError::ArchiveEmpty);
        }
        if archive.len() > MAX_STAGED_ARCHIVE_BYTES {
            return Err(QuarantineError::ArchiveTooLarge);
        }
        let receipt = IntakeReceipt {
            package_id: claimed_id.to_owned(),
            sha256: format!("{:x}", Sha256::digest(archive)),
            byte_length: archive.len(),
            status: IntakeStatus::Quarantined,
            reason: None,
        };
        let mut entries = self
            .entries
            .write()
            .map_err(|_| QuarantineError::Unavailable)?;
        if entries.contains_key(claimed_id) {
            // Never let a new malicious archive silently replace an existing
            // trusted-looking identity or override a recorded rejection.
            return Err(QuarantineError::DuplicatePackageId);
        }
        entries.insert(claimed_id.to_owned(), receipt.clone());
        Ok(receipt)
    }

    /// Reject an exact staged digest. A corrupted/mismatched artifact may not
    /// be treated as the previously reviewed package. Rejection is terminal.
    /// No callable `approve` or `execute` path exists in this stage.
    pub fn reject(
        &self,
        package_id: &str,
        expected_sha256: &str,
        reason: &str,
    ) -> Result<IntakeReceipt, QuarantineError> {
        let mut entries = self
            .entries
            .write()
            .map_err(|_| QuarantineError::Unavailable)?;
        let receipt = entries
            .get_mut(package_id)
            .ok_or(QuarantineError::UnknownPackageId)?;
        if receipt.sha256 != expected_sha256 {
            return Err(QuarantineError::DigestMismatch);
        }
        if receipt.status == IntakeStatus::Rejected {
            return Err(QuarantineError::AlreadyRejected);
        }
        receipt.status = IntakeStatus::Rejected;
        receipt.reason = Some(reason.to_owned());
        Ok(receipt.clone())
    }

    pub fn get(&self, package_id: &str) -> Result<Option<IntakeReceipt>, QuarantineError> {
        let entries = self
            .entries
            .read()
            .map_err(|_| QuarantineError::Unavailable)?;
        Ok(entries.get(package_id).cloned())
    }

    pub fn list(&self) -> Result<Vec<IntakeReceipt>, QuarantineError> {
        let entries = self
            .entries
            .read()
            .map_err(|_| QuarantineError::Unavailable)?;
        Ok(entries.values().cloned().collect())
    }
}
