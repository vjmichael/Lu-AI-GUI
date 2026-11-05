//! WRITE-01E: immutable, byte-exact edit proposals.
//!
//! This module intentionally has **no filesystem write API**. The trusted host
//! must supply the selected file bytes and the authorization source; a model
//! can propose replacement text but cannot approve or publish it.
//! Publication, backup, journaling and rollback require a separate audited
//! commit path. Do not treat proposal verification as an effect grant.

use sha2::{Digest, Sha256};
use std::fmt::Write;

pub const MAX_EDIT_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditError {
    InvalidPath,
    InvalidText,
    TooLarge,
    NoChanges,
    ApprovalMismatch,
    Conflict,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditProposal {
    /// Host-selected, workspace-relative path (not a filesystem authority).
    pub relative_path: String,
    pub source_sha256: String,
    pub replacement_sha256: String,
    /// Fingerprint commits the path, old hash, and proposed exact new bytes.
    pub approval_fingerprint: String,
    pub original: String,
    pub replacement: String,
}

fn digest(data: &[u8]) -> String {
    let hash = Sha256::digest(data);
    let mut result = String::with_capacity(64);
    for b in hash {
        write!(&mut result, "{b:02x}").expect("writing to String");
    }
    result
}

fn validate_text(bytes: &[u8]) -> Result<&str, EditError> {
    if bytes.len() > MAX_EDIT_BYTES {
        return Err(EditError::TooLarge);
    }
    if bytes.contains(&0) {
        return Err(EditError::InvalidText);
    }
    std::str::from_utf8(bytes).map_err(|_| EditError::InvalidText)
}

fn validate_path(path: &str) -> Result<(), EditError> {
    if path.is_empty()
        || path.len() > 4096
        || path.starts_with('/')
        || path.contains(['\\', ':', '\0'])
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(EditError::InvalidPath);
    }
    Ok(())
}

fn fingerprint(path: &str, old: &str, new: &str) -> String {
    // Length-prefix the fields to avoid concatenation ambiguity.
    let mut hash = Sha256::new();
    for field in [path.as_bytes(), old.as_bytes(), new.as_bytes()] {
        hash.update((field.len() as u64).to_le_bytes());
        hash.update(field);
    }
    let bytes = hash.finalize();
    digest(&bytes)
}

/// Create a preview of **all** old and new text, not a truncated or
/// model-asserted diff. A UI may render a line diff from these exact strings.
pub fn prepare_text_edit(
    relative_path: &str,
    current_bytes: &[u8],
    replacement_bytes: &[u8],
) -> Result<EditProposal, EditError> {
    validate_path(relative_path)?;
    let before = validate_text(current_bytes)?;
    let after = validate_text(replacement_bytes)?;
    if before == after {
        return Err(EditError::NoChanges);
    }
    let source_sha256 = digest(current_bytes);
    let replacement_sha256 = digest(replacement_bytes);
    let approval_fingerprint =
        fingerprint(relative_path, &source_sha256, &replacement_sha256);
    Ok(EditProposal {
        relative_path: relative_path.into(),
        source_sha256,
        replacement_sha256,
        approval_fingerprint,
        original: before.into(),
        replacement: after.into(),
    })
}

/// Verifies the exact proposal and a trusted UI's approved fingerprint
/// against a fresh read of the target. Must be called again inside the
/// eventual exclusive commit critical section; it is not a locking API.
pub fn verify_text_edit(
    plan: &EditProposal,
    approved_fingerprint: &str,
    latest_bytes: &[u8],
) -> Result<(), EditError> {
    validate_path(&plan.relative_path)?;
    validate_text(latest_bytes)?;
    validate_text(plan.original.as_bytes())?;
    validate_text(plan.replacement.as_bytes())?;
    if plan.original == plan.replacement {
        return Err(EditError::NoChanges);
    }
    if digest(plan.original.as_bytes()) != plan.source_sha256
        || digest(plan.replacement.as_bytes()) != plan.replacement_sha256
        || fingerprint(
            &plan.relative_path,
            &plan.source_sha256,
            &plan.replacement_sha256,
        ) != plan.approval_fingerprint
        || approved_fingerprint != plan.approval_fingerprint
    {
        return Err(EditError::ApprovalMismatch);
    }
    if digest(latest_bytes) != plan.source_sha256 {
        return Err(EditError::Conflict);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_approval_and_concurrent_change_detection() {
        let plan = prepare_text_edit("report/notes.md", b"hello\n", b"hello world\n").unwrap();
        assert_eq!(plan.original, "hello\n");
        assert_eq!(plan.replacement, "hello world\n");
        assert!(verify_text_edit(&plan, &plan.approval_fingerprint, b"hello\n").is_ok());
        assert_eq!(
            verify_text_edit(&plan, &plan.approval_fingerprint, b"other\n"),
            Err(EditError::Conflict)
        );
        assert_eq!(
            verify_text_edit(&plan, "model-said-yes", b"hello\n"),
            Err(EditError::ApprovalMismatch)
        );
    }

    #[test]
    fn tampered_proposal_never_verifies() {
        let mut plan = prepare_text_edit("a.json", b"{}", b"{\"ok\":true}").unwrap();
        let approved = plan.approval_fingerprint.clone();
        plan.replacement.push(' ');
        assert_eq!(
            verify_text_edit(&plan, &approved, b"{}"),
            Err(EditError::ApprovalMismatch)
        );
    }

    #[test]
    fn reject_traversal_binary_and_noop() {
        for p in ["../secret", "/root", "a//b", "a/./b", "C:/x", "a\\b"] {
            assert_eq!(prepare_text_edit(p, b"a", b"b"), Err(EditError::InvalidPath));
        }
        assert_eq!(
            prepare_text_edit("x.txt", b"a\0b", b"x"),
            Err(EditError::InvalidText)
        );
        assert_eq!(
            prepare_text_edit("x.txt", b"a", b"a"),
            Err(EditError::NoChanges)
        );
    }

    #[test]
    fn file_format_is_not_guessed_from_extension() {
        // This gate applies to verified UTF-8 bytes, not just .txt files.
        let plan = prepare_text_edit("index.html", b"<h1>A</h1>", b"<h1>B</h1>").unwrap();
        assert!(verify_text_edit(&plan, &plan.approval_fingerprint, b"<h1>A</h1>").is_ok());
    }
}
