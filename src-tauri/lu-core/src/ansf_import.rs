//! Non-destructive, streaming Jan desktop JSONL -> ANSF import adapter.
//!
//! The input is a trusted-host-selected Jan `messages.jsonl` snapshot. This
//! adapter NEVER opens arbitrary input paths, deletes or modifies Jan files,
//! executes message contents, or changes model context. A single original JSON
//! record is stored verbatim (excluding its JSONL record separator) as the
//! immutable ANSF entry `content`. A short derived summary is only for search.
//!
//! Existing Jan supports editing messages in place; an edited record with the
//! same message ID never rewrites the ANSF original. Ordinary import reports
//! `ChangedSource`. The explicit verified-newer-snapshot mode appends a new
//! immutable entry linked to that original; the timestamp represents when Lu
//! observed the imported snapshot, not necessarily the user's edit time.
//! All writes are transactional *per entry*: after a power loss the
//! operator may repeat the import, and identical message IDs are deduplicated.
//! Import is not yet integrated into the Jan desktop or its history UI.
//!
//! Hosts MUST enforce ownership and conversation ID provenance. The records
//! themselves are untrusted data, not grants for the assistant to act.

use std::io::BufRead;

use serde_json::Value;

use crate::{
    ansf::{AnsfError, AnsfStore, EntryRole},
    ansf_revisions::RevisionOrigin,
};

const MAX_LINE_BYTES: usize = 1024 * 1024;
const MAX_SHORT_SUMMARY_CHARS: usize = 180;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportError {
    Store(AnsfError),
    Io(String),
    InvalidRecord { line: u64, reason: String },
    OversizedRecord { line: u64 },
    ChangedSource { line: u64, message_id: String },
}

impl From<AnsfError> for ImportError {
    fn from(value: AnsfError) -> Self {
        Self::Store(value)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportStats {
    pub source_lines: u64,
    pub added: u64,
    pub unchanged: u64,
    pub revised: u64,
    pub blank_lines: u64,
}

/// Ordinary imports refuse unexpected source mutations. This stronger mode
/// must only be selected by a trusted desktop host that has confirmed this is
/// a *newer* Jan snapshot, not an old backup or stale sync echo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JanImportPolicy {
    RejectChangedSource,
    AppendConfirmedNewerSnapshot,
}

fn next_bounded_line<R: BufRead>(
    reader: &mut R,
    line_number: u64,
) -> Result<Option<Vec<u8>>, ImportError> {
    let mut line = Vec::new();
    loop {
        let (count, has_newline) = {
            let bytes = reader
                .fill_buf()
                .map_err(|error| ImportError::Io(error.to_string()))?;
            if bytes.is_empty() {
                return Ok((!line.is_empty()).then_some(line));
            }
            let count = bytes
                .iter()
                .position(|b| *b == b'\n')
                .map(|index| index + 1)
                .unwrap_or(bytes.len());
            if line.len().saturating_add(count) > MAX_LINE_BYTES + 2 {
                return Err(ImportError::OversizedRecord { line: line_number });
            }
            line.extend_from_slice(&bytes[..count]);
            (count, bytes[count - 1] == b'\n')
        };
        reader.consume(count);
        if has_newline {
            return Ok(Some(line));
        }
    }
}

fn message_text(payload: &Value) -> String {
    let mut collected = Vec::new();
    if let Some(text) = payload.get("content").and_then(Value::as_str) {
        collected.push(text);
    }
    for field in ["content", "parts"] {
        if let Some(parts) = payload.get(field).and_then(Value::as_array) {
            for item in parts {
                if let Some(text) = item.get("text").and_then(Value::as_str) {
                    collected.push(text);
                } else if let Some(text) = item
                    .get("text")
                    .and_then(|text| text.get("value"))
                    .and_then(Value::as_str)
                {
                    collected.push(text);
                }
            }
        }
    }
    let prefix = collected
        .into_iter()
        .flat_map(|s| s.split_whitespace())
        .take(160)
        .collect::<Vec<_>>()
        .join(" ");
    prefix.chars().take(MAX_SHORT_SUMMARY_CHARS).collect()
}

fn parse(
    json: &str,
    line: u64,
    expected_session: &str,
) -> Result<(String, EntryRole, String, &'static str), ImportError> {
    let invalid = |reason: &str| ImportError::InvalidRecord {
        line,
        reason: reason.to_owned(),
    };
    let payload: Value =
        serde_json::from_str(json).map_err(|_| invalid("malformed JSON object"))?;
    if !payload.is_object() {
        return Err(invalid("message must be a JSON object"));
    }
    if let Some(actual_session) = payload.get("thread_id") {
        if actual_session.as_str() != Some(expected_session) {
            return Err(invalid("message belongs to a different conversation"));
        }
    }
    let id = payload
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("missing message id"))?;
    if id.trim().is_empty() || id.len() > 128 || id.contains('\0') {
        return Err(invalid("invalid message id"));
    }
    let role = payload
        .get("role")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("missing role"))?;
    let (role, label) = match role {
        "user" => (EntryRole::User, "user"),
        "assistant" => (EntryRole::Assistant, "assistant"),
        "system" => (EntryRole::System, "system"),
        "tool" | "tool_result" => (EntryRole::ToolResult, "tool_result"),
        "tool_call" => (EntryRole::ToolCall, "tool_call"),
        _ => return Err(invalid("unrecognized role; no record silently dropped")),
    };
    let text = message_text(&payload);
    let summary = if text.is_empty() {
        format!("{label}: [non-text or structured content]")
    } else {
        format!("{label}: {text}")
    };
    Ok((id.to_owned(), role, summary, label))
}

/// Import an existing Jan stream without loading the whole chat into memory.
///
/// Caller must create the ANSF session first. If no chapter is currently open,
/// a single explicit `Imported Jan history` chapter is created. Later
/// reclassification can add topical metadata without changing original
/// message bytes. Import is safe to rerun after partial progress.
pub fn import_jan_jsonl<R: BufRead>(
    store: &AnsfStore,
    session_id: &str,
    input: R,
) -> Result<ImportStats, ImportError> {
    import_jan_jsonl_with_policy(
        store,
        session_id,
        input,
        JanImportPolicy::RejectChangedSource,
    )
}

/// Explicit opt-in revision synchronization. Before using
/// AppendConfirmedNewerSnapshot, the host must guarantee snapshot chronology.
/// Raw JSONL metadata alone is NOT proof that an edited row is newer.
pub fn import_jan_jsonl_with_policy<R: BufRead>(
    store: &AnsfStore,
    session_id: &str,
    mut input: R,
    policy: JanImportPolicy,
) -> Result<ImportStats, ImportError> {
    if session_id.trim().is_empty() || session_id.len() > 128 {
        return Err(ImportError::InvalidRecord {
            line: 0,
            reason: "invalid conversation id".into(),
        });
    }
    let chapters = store.chapters(session_id)?;
    if !chapters.iter().any(|chapter| !chapter.closed) {
        store.start_chapter(session_id, "Imported Jan history")?;
    }
    let mut stats = ImportStats::default();
    loop {
        let line_num = stats.source_lines + 1;
        let Some(mut record) = next_bounded_line(&mut input, line_num)? else {
            return Ok(stats);
        };
        stats.source_lines += 1;
        if record.last() == Some(&b'\n') {
            record.pop();
        }
        if record.last() == Some(&b'\r') {
            record.pop();
        }
        if record.len() > MAX_LINE_BYTES {
            return Err(ImportError::OversizedRecord { line: line_num });
        }
        let source = std::str::from_utf8(&record).map_err(|_| ImportError::InvalidRecord {
            line: line_num,
            reason: "invalid UTF-8".into(),
        })?;
        if source.trim().is_empty() {
            stats.blank_lines += 1;
            continue;
        }
        let (message_id, role, summary, role_label) = parse(source, line_num, session_id)?;
        if let Some(current) = store.current_message(session_id, &message_id)? {
            if current.entry.role != role_label {
                return Err(ImportError::ChangedSource {
                    line: line_num,
                    message_id,
                });
            }
            if current.entry.content == source {
                stats.unchanged += 1;
                continue;
            }
            match policy {
                JanImportPolicy::RejectChangedSource => {
                    return Err(ImportError::ChangedSource {
                        line: line_num,
                        message_id,
                    });
                }
                JanImportPolicy::AppendConfirmedNewerSnapshot => {
                    // Keep original and all intervening versions. This note
                    // must never falsely claim an authenticated user edit.
                    store.append_edited_entry(
                        session_id,
                        &message_id,
                        &summary,
                        source,
                        RevisionOrigin::ImportObserved,
                    )?;
                    stats.revised += 1;
                    continue;
                }
            }
        }
        store.append_entry(session_id, &message_id, role, &summary, source)?;
        stats.added += 1;
    }
}
