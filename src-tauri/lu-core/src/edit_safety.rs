//! WRITE-01E foundation: deterministic preflight for editing EXISTING text.
//! This code does NOT publish changes. Every future replace/patch tool must
//! require a plan with an exact baseline hash, independently checked target
//! syntax, full user diff review, and durable reversible backup custody.
//! Models are never permitted to retry edits automatically on failure.
use sha2::{Digest, Sha256};
use crate::write_format::{identify_text_format, FormatError, TextFormat};

pub const MAX_EDIT_DOCUMENT_BYTES: usize = 1024 * 1024;
const MAX_DIFF_PREVIEW_CHARS: usize = 2048;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditRisk { Localized, Extensive }
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditPreflightError {
    OriginalHashMismatch,
    InvalidOriginalUtf8,
    InvalidReplacementUtf8,
    InvalidFormat(FormatError),
    Oversized,
    Unchanged,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEditPlan {
    pub filename: String,
    pub expected_original_sha256: String,
    pub replacement_sha256: String,
    pub original_bytes: usize,
    pub replacement_bytes: usize,
    pub old_changed_bytes: usize,
    pub new_changed_bytes: usize,
    pub original_excerpt: String,
    pub replacement_excerpt: String,
    pub format: TextFormat,
    pub risk: EditRisk,
    pub requires_explicit_full_diff_review: bool,
    pub automatic_repair_attempts_allowed: u8,
}
fn digest(data: &[u8])->String {format!("{:x}",Sha256::digest(data))}

/// Pure, deterministic admission: checks a snapshotted baseline and the
/// replacement content without touching disk. The actual commit path must
/// recheck the baseline *again* using the already-opened file handle,
/// preserve an original backup, then atomically publish or refuse.
pub fn plan_text_edit(
    filename: &str,
    original: &[u8],
    expected_original_sha256: &str,
    replacement: &[u8],
) -> Result<TextEditPlan,EditPreflightError> {
    if original.len()>MAX_EDIT_DOCUMENT_BYTES || replacement.len()>MAX_EDIT_DOCUMENT_BYTES {
        return Err(EditPreflightError::Oversized);
    }
    if digest(original) != expected_original_sha256 {
        return Err(EditPreflightError::OriginalHashMismatch);
    }
    let old=std::str::from_utf8(original).map_err(|_|EditPreflightError::InvalidOriginalUtf8)?;
    let new=std::str::from_utf8(replacement).map_err(|_|EditPreflightError::InvalidReplacementUtf8)?;
    let format=identify_text_format(filename,new).map_err(EditPreflightError::InvalidFormat)?;
    if old==new {return Err(EditPreflightError::Unchanged);}
    let mut prefix=0usize;
    for (a,b) in old.chars().zip(new.chars()) {
        if a!=b {break;}
        prefix+=a.len_utf8();
    }
    let mut suffix_old=0usize;
    let mut suffix_new=0usize;
    for (a,b) in old[prefix..].chars().rev().zip(new[prefix..].chars().rev()) {
        if a!=b {break;}
        suffix_old+=a.len_utf8();
        suffix_new+=b.len_utf8();
    }
    let old_changed=&old[prefix..old.len()-suffix_old];
    let new_changed=&new[prefix..new.len()-suffix_new];
    let delta=old_changed.len().max(new_changed.len());
    let extensive=delta>4096 || delta.saturating_mul(2)>original.len().max(1);
    Ok(TextEditPlan{
        filename:filename.to_owned(),
        expected_original_sha256:expected_original_sha256.into(),
        replacement_sha256:digest(replacement),
        original_bytes:original.len(),
        replacement_bytes:replacement.len(),
        old_changed_bytes:old_changed.len(),
        new_changed_bytes:new_changed.len(),
        original_excerpt:old_changed.chars().take(MAX_DIFF_PREVIEW_CHARS).collect(),
        replacement_excerpt:new_changed.chars().take(MAX_DIFF_PREVIEW_CHARS).collect(),
        format,
        risk:if extensive {EditRisk::Extensive} else {EditRisk::Localized},
        requires_explicit_full_diff_review:true,
        automatic_repair_attempts_allowed:0,
    })
}


/// Host-bound approval for the precise target and bytes. This value is only
/// evidence that a review matched; it does not create a tool grant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewedEdit {
    pub host_relative_path: String,
    pub approval_sha256: String,
    /// Native host selected-file identity when Windows requires binding.
    /// Never sourced from a model, WebView or arbitrary user-written JSON.
    pub host_file_identity: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewError {
    InvalidHostPath,
    ChangedSource,
    InvalidPlan(EditPreflightError),
    MismatchedPlan,
    MismatchedApproval,
}

fn valid_host_path(path: &str) -> bool {
    !path.is_empty() && path.len() <= 4096 && !path.starts_with('/')
        && !path.chars().any(|c| c == '\\' || c == ':' || c.is_control())
        && path.split('/').all(|part| !part.is_empty() && part != "." && part != "..")
}

/// Serialize values with length boundaries and a domain separator. The full
/// diff has to be rendered by the host from the exact bytes supplied here;
/// preview excerpts in TextEditPlan are NEVER sufficient for approval.
fn reviewed_digest(path: &str, old: &[u8], new: &[u8], file_identity:Option<&str>) -> String {
    let mut hash = Sha256::new();
    // Preserve exact historical v1 hashes for non-Windows approved edits.
    hash.update(if file_identity.is_some() {
        b"LU-WRITE-01E-REVIEW-NATIVE-ID-v2".as_slice()
    } else {
        b"LU-WRITE-01E-REVIEW-v1".as_slice()
    });
    for field in [path.as_bytes(), old, new] {
        hash.update((field.len() as u64).to_le_bytes());
        hash.update(field);
    }
    if let Some(id)=file_identity {
        hash.update((id.len() as u64).to_le_bytes());
        hash.update(id.as_bytes());
    }
    format!("{:x}", hash.finalize())
}

/// To be called ONLY by the trusted desktop host after independently
/// rendering the entire original and replacement and obtaining user approval.
/// A model-supplied fingerprint is never an approval source.
pub fn bind_reviewed_edit(
    plan: &TextEditPlan,
    host_relative_path: &str,
    original: &[u8],
    replacement: &[u8],
) -> Result<ReviewedEdit, ReviewError> {
    bind_reviewed_edit_with_host_identity(plan,host_relative_path,original,replacement,None)
}

/// The trusted Windows picker supplies a 48-character lowercase native ID
/// token. Its identity is included in the irreversible reviewed fingerprint
/// that the existing SQLite approval ledger persists. This is still NOT a
/// permission grant and is verified again by the native effect executor.
pub fn bind_reviewed_edit_with_host_identity(
    plan: &TextEditPlan,
    host_relative_path: &str,
    original: &[u8],
    replacement: &[u8],
    host_file_identity:Option<&str>,
) -> Result<ReviewedEdit, ReviewError> {
    if !valid_host_path(host_relative_path)
        || host_relative_path.rsplit('/').next() != Some(plan.filename.as_str()) {
        return Err(ReviewError::InvalidHostPath);
    }
    if host_file_identity.is_some_and(|id|
        id.len()!=48 || !id.bytes().all(|b|
            b.is_ascii_digit() || (b'a'..=b'f').contains(&b))) {
        return Err(ReviewError::InvalidHostPath);
    }
    let expected = plan_text_edit(&plan.filename, original, &plan.expected_original_sha256, replacement)
        .map_err(ReviewError::InvalidPlan)?;
    if &expected != plan {
        return Err(ReviewError::MismatchedPlan);
    }
    Ok(ReviewedEdit {
        host_relative_path: host_relative_path.into(),
        approval_sha256: reviewed_digest(host_relative_path, original, replacement,host_file_identity),
        host_file_identity:host_file_identity.map(str::to_owned),
    })
}

/// Fresh-read check immediately before a future exclusive, journaled commit.
/// This is a pure check and not a filesystem lock or an effect authorization.
pub fn verify_reviewed_edit(
    plan: &TextEditPlan,
    proof: &ReviewedEdit,
    trusted_host_path: &str,
    latest_original: &[u8],
    replacement: &[u8],
    trusted_approved_sha256: &str,
) -> Result<(), ReviewError> {
    if !valid_host_path(trusted_host_path)
        || proof.host_relative_path != trusted_host_path
        || trusted_host_path.rsplit('/').next() != Some(plan.filename.as_str()) {
        return Err(ReviewError::InvalidHostPath);
    }
    if digest(latest_original) != plan.expected_original_sha256 {
        return Err(ReviewError::ChangedSource);
    }
    let regenerated = plan_text_edit(&plan.filename, latest_original, &plan.expected_original_sha256, replacement)
        .map_err(ReviewError::InvalidPlan)?;
    if regenerated != *plan {
        return Err(ReviewError::MismatchedPlan);
    }
    let check = reviewed_digest(trusted_host_path, latest_original, replacement,
        proof.host_file_identity.as_deref());
    if check != proof.approval_sha256 || check != trusted_approved_sha256 {
        return Err(ReviewError::MismatchedApproval);
    }
    Ok(())
}

#[cfg(test)]
mod reviewed_edit_tests {
    use super::*;

    #[test]
    fn reviewed_bytes_are_exact_and_source_is_not_silently_overwritten() {
        let old = b"{\"enabled\":false}\n";
        let new = b"{\"enabled\":true}\n";
        let plan = plan_text_edit("preferences.json", old, &digest(old), new).unwrap();
        let proof = bind_reviewed_edit(&plan, "Documents/preferences.json", old, new).unwrap();
        assert!(verify_reviewed_edit(&plan, &proof, "Documents/preferences.json", old, new, &proof.approval_sha256).is_ok());
        assert_eq!(verify_reviewed_edit(&plan, &proof, "Documents/preferences.json", b"{\"enabled\":false} \n", new, &proof.approval_sha256), Err(ReviewError::ChangedSource));
        assert_eq!(verify_reviewed_edit(&plan, &proof, "Documents/preferences.json", old, b"{\"enabled\":null}\n", &proof.approval_sha256), Err(ReviewError::MismatchedPlan));
    }

    #[test]
    fn approval_cannot_be_reused_for_another_target_or_mutated_plan() {
        let plan = plan_text_edit("x.md", b"a", &digest(b"a"), b"b").unwrap();
        let proof = bind_reviewed_edit(&plan, "work/x.md", b"a", b"b").unwrap();
        assert_eq!(verify_reviewed_edit(&plan, &proof, "other/x.md", b"a", b"b", &proof.approval_sha256), Err(ReviewError::InvalidHostPath));
        assert_eq!(verify_reviewed_edit(&plan, &proof, "work/x.md", b"a", b"b", "model-approved"), Err(ReviewError::MismatchedApproval));
        let mut tampered = plan.clone();
        tampered.original_excerpt.push('!');
        assert_eq!(verify_reviewed_edit(&tampered, &proof, "work/x.md", b"a", b"b", &proof.approval_sha256), Err(ReviewError::MismatchedPlan));
        assert_eq!(bind_reviewed_edit(&plan, "../x.md", b"a", b"b"), Err(ReviewError::InvalidHostPath));
    }

    #[test]
    fn host_captured_native_file_identity_is_part_of_the_approval_digest() {
        let old=b"draft";let new=b"revised";
        let plan=plan_text_edit("notes.md",old,&digest(old),new).unwrap();
        let first="0123456789abcdef0123456789abcdef0123456789abcdef";
        let second="0123456789abcdef0123456789abcdef0123456789abcdee";
        let a=bind_reviewed_edit_with_host_identity(&plan,"notes.md",old,new,Some(first)).unwrap();
        let b=bind_reviewed_edit_with_host_identity(&plan,"notes.md",old,new,Some(second)).unwrap();
        let legacy=bind_reviewed_edit(&plan,"notes.md",old,new).unwrap();
        assert_ne!(a.approval_sha256,b.approval_sha256);
        assert_ne!(a.approval_sha256,legacy.approval_sha256);
        assert_eq!(a.host_file_identity.as_deref(),Some(first));
        assert!(verify_reviewed_edit(&plan,&a,"notes.md",old,new,&a.approval_sha256).is_ok());
        assert_eq!(verify_reviewed_edit(&plan,&a,"notes.md",old,new,&b.approval_sha256),
            Err(ReviewError::MismatchedApproval));
        assert_eq!(bind_reviewed_edit_with_host_identity(&plan,"notes.md",old,new,
            Some("model-approved")),Err(ReviewError::InvalidHostPath));
    }

    #[test]
    fn invalid_json_and_changed_binary_are_never_approved() {
        let original = b"{\"ok\":true}";
        let plan = plan_text_edit("x.json", original, &digest(original), b"{\"ok\":false}").unwrap();
        assert_eq!(bind_reviewed_edit(&plan, "x.json", original, b"{bad"), Err(ReviewError::InvalidPlan(EditPreflightError::InvalidFormat(FormatError::InvalidJson))));
    }
}
