//! Safe, deterministic export artifact naming and provenance metadata.
//! No file is written. A future host renderer must use create_new/atomic
//! creation and verify the source digest before exporting PDF/DOCX/CSV.
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    Pdf,
    Docx,
    Csv,
}
impl ExportFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Docx => "docx",
            Self::Csv => "csv",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportRequest {
    pub title: String,
    pub format: ExportFormat,
    pub conversation_id: String,
    pub message_ids: Vec<String>,
    pub source_sha256: String,
    /// Must come from the host clock, not model output.
    pub generated_at_unix_secs: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportProvenance {
    pub conversation_id: String,
    pub message_ids: Vec<String>,
    pub source_sha256: String,
    pub generated_at_unix_secs: u64,
    pub format: ExportFormat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportPlan {
    pub file_name: String,
    pub provenance: ExportProvenance,
    /// The writer must never overwrite an existing user's artifact.
    pub create_new: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportError {
    InvalidTitle,
    UnsafeTitle,
    InvalidConversation,
    InvalidMessages,
    InvalidDigest,
    InvalidTimestamp,
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_' || byte == b':'
        })
}

fn safe_stem(title: &str) -> Result<String, ExportError> {
    if title.trim().is_empty() || title.len() > 256 {
        return Err(ExportError::InvalidTitle);
    }
    if title.contains("..")
        || title
            .chars()
            .any(|c| c.is_control() || matches!(c, '/' | '\\' | ':'))
    {
        return Err(ExportError::UnsafeTitle);
    }
    let mut stem = String::new();
    for character in title.chars() {
        let ch = character.to_ascii_lowercase();
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            stem.push(ch);
        } else if !stem.ends_with('-') && !stem.is_empty() {
            stem.push('-');
        }
        if stem.len() >= 60 {
            break;
        }
    }
    let stem = stem.trim_matches(|c: char| matches!(c, '-' | '_' | '.' | ' '));
    if stem.is_empty() {
        return Err(ExportError::InvalidTitle);
    }
    let upper = stem.to_ascii_uppercase();
    if ["CON", "PRN", "AUX", "NUL"].contains(&upper.as_str())
        || (upper.len() == 4
            && (upper.starts_with("COM") || upper.starts_with("LPT"))
            && matches!(upper.as_bytes()[3], b'1'..=b'9'))
    {
        return Ok(format!("lu-{stem}"));
    }
    Ok(stem.to_string())
}

/// This is a planning/validation boundary; it never accepts a destination
/// directory or raw path from a model, and never writes an artifact.
pub fn plan_export(request: &ExportRequest) -> Result<ExportPlan, ExportError> {
    let stem = safe_stem(&request.title)?;
    if !valid_id(&request.conversation_id) {
        return Err(ExportError::InvalidConversation);
    }
    if request.message_ids.is_empty()
        || request.message_ids.len() > 64
        || !request.message_ids.iter().all(|id| valid_id(id))
        || request.message_ids.iter().collect::<BTreeSet<_>>().len() != request.message_ids.len()
    {
        return Err(ExportError::InvalidMessages);
    }
    if request.source_sha256.len() != 64
        || !request
            .source_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(ExportError::InvalidDigest);
    }
    if request.generated_at_unix_secs == 0 {
        return Err(ExportError::InvalidTimestamp);
    }
    let digest = request.source_sha256.to_ascii_lowercase();
    Ok(ExportPlan {
        file_name: format!(
            "{stem}-{}-{}.{}",
            request.generated_at_unix_secs,
            &digest[..16],
            request.format.extension()
        ),
        provenance: ExportProvenance {
            conversation_id: request.conversation_id.clone(),
            message_ids: request.message_ids.clone(),
            source_sha256: digest,
            generated_at_unix_secs: request.generated_at_unix_secs,
            format: request.format,
        },
        create_new: true,
    })
}
