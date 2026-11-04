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
