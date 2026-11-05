//! IDE-ARCH-02 / Wave A1: revision-checked, bounded source-span identity.
//! All filesystem access goes through the existing capability-safe text preview.
//! This module does not execute, write, grant permissions or connect to Lu.
use serde::Serialize;

use crate::{browser::BrowserError, IdeWorkspaceRegistry};

const MAX_SPAN_LINES: usize = 120;
const MAX_SPAN_BYTES: usize = 32 * 1024;

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SourceLine {
    /// One-based line index. Blank lines and trailing final empty line count.
    pub line_number: usize,
    pub text: String,
    /// UTF-8 byte offsets in the exact file revision; end is exclusive.
    pub start_byte: usize,
    pub end_byte: usize,
    /// "lf", "crlf" or "none" (the last non-terminated line).
    pub newline: &'static str,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceSpan {
    pub schema_version: u32,
    pub project_id: String,
    pub relative_path: String,
    pub revision: String,
    pub first_line: usize,
    pub last_line: usize,
    pub total_lines: usize,
    pub lines: Vec<SourceLine>,
}
#[derive(Debug, PartialEq, Eq)]
pub enum SpanError {
    InvalidRange,
    TooLarge,
}
impl SpanError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidRange => "INVALID_SOURCE_RANGE",
            Self::TooLarge => "SOURCE_SPAN_TOO_LARGE",
        }
    }
}
#[derive(Debug)]
pub enum SourceSpanError {
    Browser(BrowserError),
    Span(SpanError),
}
impl SourceSpanError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Browser(err) => err.code(),
            Self::Span(err) => err.code(),
        }
    }
}
impl From<BrowserError> for SourceSpanError {
    fn from(value: BrowserError) -> Self { Self::Browser(value) }
}

fn extract_lines(text: &str, first: usize, last: usize) -> Result<(usize, Vec<SourceLine>), SpanError> {
    if first == 0 || last < first || last - first >= MAX_SPAN_LINES {
        return Err(SpanError::InvalidRange);
    }
    let mut start = 0;
    let mut number = 1;
    let mut lines = Vec::new();
    let mut selected_bytes = 0;
    // split_inclusive handles LF, CRLF and a trailing empty line separately.
    for chunk in text.split_inclusive('\n') {
        let has_lf = chunk.ends_with('\n');
        let has_crlf = chunk.ends_with("\r\n");
        let stripped = if has_crlf { &chunk[..chunk.len()-2] }
            else if has_lf { &chunk[..chunk.len()-1] } else { chunk };
        if number >= first && number <= last {
            selected_bytes += stripped.len();
            if selected_bytes > MAX_SPAN_BYTES { return Err(SpanError::TooLarge); }
            lines.push(SourceLine {
                line_number: number, text: stripped.to_string(),
                start_byte: start, end_byte: start + stripped.len(),
                newline: if has_crlf { "crlf" } else if has_lf { "lf" } else { "none" },
            });
        }
        start += chunk.len();
        number += 1;
    }
    // Empty files contain line 1; a file ending in a newline has a final empty line.
    if text.is_empty() || text.ends_with('\n') {
        if number >= first && number <= last {
            lines.push(SourceLine { line_number: number, text: String::new(),
                start_byte: text.len(), end_byte: text.len(), newline: "none" });
        }
        number += 1;
    }
    let total = number - 1;
    if last > total { return Err(SpanError::InvalidRange); }
    Ok((total, lines))
}
impl IdeWorkspaceRegistry {
    /// Read only the requested lines from the currently saved disk revision.
    /// The expected SHA-256 is required to prevent citing a changed file.
    /// Unsaved CodeMirror buffers are distinct and not represented here.
    pub fn read_source_span(&self, project_id: &str, relative_path: &str,
        expected_revision: &str, first_line: usize, last_line: usize
    ) -> Result<SourceSpan, SourceSpanError> {
        if expected_revision.len() != 64 ||
            !expected_revision.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(SourceSpanError::Browser(BrowserError::InvalidRevision));
        }
        let preview = self.preview_source_file(project_id, relative_path)?;
        if preview.revision != expected_revision {
            return Err(SourceSpanError::Browser(BrowserError::Conflict));
        }
        let (total_lines, lines) = extract_lines(&preview.text, first_line, last_line)
            .map_err(SourceSpanError::Span)?;
        Ok(SourceSpan { schema_version: 1, project_id: preview.project_id,
            relative_path: preview.relative_path, revision: preview.revision,
            first_line, last_line, total_lines, lines })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tracks_crlf_lf_blank_and_unicode_byte_offsets() {
        let input = "α\r\n\nβ\nlast";
        let (total, lines) = extract_lines(input, 1, 4).unwrap();
        assert_eq!(total, 4);
        assert_eq!(lines[0].text, "α");
        assert_eq!((lines[0].start_byte, lines[0].end_byte), (0, 2));
        assert_eq!(lines[0].newline, "crlf");
        assert_eq!(lines[1].text, "");
        assert_eq!((lines[1].start_byte, lines[1].end_byte), (4, 4));
        assert_eq!(lines[2].text, "β");
        assert_eq!((lines[2].start_byte, lines[2].end_byte), (5, 7));
        assert_eq!(lines[3].text, "last");
        assert_eq!(lines[3].newline, "none");
    }
    #[test]
    fn trailing_blank_line_and_empty_file_are_addressable() {
        let (count, lines) = extract_lines("a\n", 2, 2).unwrap();
        assert_eq!(count, 2);
        assert_eq!(lines[0].start_byte, 2);
        assert_eq!(lines[0].text, "");
        let (count, lines) = extract_lines("", 1, 1).unwrap();
        assert_eq!(count, 1);
        assert_eq!(lines[0].newline, "none");
    }
    #[test]
    fn rejects_overlong_or_out_of_bounds_requests() {
        assert_eq!(extract_lines("a", 0, 1).unwrap_err(), SpanError::InvalidRange);
        assert_eq!(extract_lines("a", 1, 2).unwrap_err(), SpanError::InvalidRange);
        assert_eq!(extract_lines("a", 1, 121).unwrap_err(), SpanError::InvalidRange);
        assert_eq!(extract_lines(&"x".repeat(MAX_SPAN_BYTES + 1), 1, 1).unwrap_err(), SpanError::TooLarge);
    }
}
