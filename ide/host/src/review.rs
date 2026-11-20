//! IDE-08: bounded, non-mutating, multi-hunk source review.
//!
//! A review is untrusted data, not an approval or permission to write.
//! The existing separate save endpoint rechecks the revision before replacing.
use serde::Serialize;
use similar::{ChangeTag, TextDiff};

use crate::{
    browser::{revision_for, BrowserError, MAX_TEXT_BYTES},
    IdeWorkspaceRegistry,
};

const MAX_REVIEW_LINES: usize = 240;
const CONTEXT_LINES: usize = 3;
const MAX_REVIEW_OUTPUT_BYTES: usize = 32 * 1024;
const MAX_DIFF_INPUT_LINES: usize = 8_000;
const MAX_REVIEW_HUNKS: usize = 64;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewLine {
    /// "context", "removed" or "added". Always rendered as text, never HTML.
    pub kind: &'static str,
    pub text: String,
    /// One-based line numbers, absent on the other side of an insertion/deletion.
    pub old_line: Option<usize>,
    pub new_line: Option<usize>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewHunk {
    /// Git-style display header. It is a label, not a runnable patch.
    pub header: String,
    pub old_start: usize,
    pub old_lines: usize,
    pub new_start: usize,
    pub new_lines: usize,
    pub lines: Vec<ReviewLine>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeReview {
    pub project_id: String,
    pub relative_path: String,
    pub expected_revision: String,
    pub proposed_revision: String,
    pub unchanged: bool,
    pub removed_lines: usize,
    pub added_lines: usize,
    /// False when input has too many lines to compute a complete diff safely.
    pub counts_exact: bool,
    pub truncated: bool,
    pub hunks: Vec<ReviewHunk>,
}

struct DiffSummary {
    hunks: Vec<ReviewHunk>,
    removed_lines: usize,
    added_lines: usize,
    counts_exact: bool,
    truncated: bool,
}

/// Separate change clusters correctly. The earlier R3.3 implementation
/// treated all lines between the first/last edit as replacements, even when
/// large unchanged regions separated them. Similar's line diff preserves
/// newline changes and groups operations into independent context hunks.
///
/// For very dense inputs we refuse to imply a complete preview. Users can
/// still inspect the full draft in the editor; the reviewed-save control is
/// disabled whenever `truncated` is true.
fn summarize_change(old: &str, proposed: &str) -> DiffSummary {
    let total_lines = old.lines().count().saturating_add(proposed.lines().count());
    if total_lines > MAX_DIFF_INPUT_LINES {
        return DiffSummary {
            hunks: Vec::new(),
            removed_lines: 0,
            added_lines: 0,
            counts_exact: false,
            truncated: true,
        };
    }

    let diff = TextDiff::from_lines(old, proposed);
    let mut removed_lines = 0;
    let mut added_lines = 0;
    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Delete => removed_lines += 1,
            ChangeTag::Insert => added_lines += 1,
            ChangeTag::Equal => {}
        }
    }

    let mut hunks = Vec::new();
    let mut emitted_lines: usize = 0;
    let mut emitted_bytes: usize = 0;
    let mut truncated = false;
    'group: for group in diff.grouped_ops(CONTEXT_LINES) {
        if hunks.len() >= MAX_REVIEW_HUNKS {
            truncated = true;
            break;
        }
        let first = group.first().expect("diff group cannot be empty");
        let last = group.last().expect("diff group cannot be empty");
        let old_range = first.old_range().start..last.old_range().end;
        let new_range = first.new_range().start..last.new_range().end;
        let old_lines = old_range.end - old_range.start;
        let new_lines = new_range.end - new_range.start;
        let old_start = if old_lines == 0 {
            old_range.start
        } else {
            old_range.start + 1
        };
        let new_start = if new_lines == 0 {
            new_range.start
        } else {
            new_range.start + 1
        };
        let mut hunk = ReviewHunk {
            header: format!(
                "@@ -{},{} +{},{} @@",
                old_start, old_lines, new_start, new_lines
            ),
            old_start,
            old_lines,
            new_start,
            new_lines,
            lines: Vec::new(),
        };
        for op in &group {
            for change in diff.iter_changes(op) {
                let text = change.as_str().unwrap_or("");
                if emitted_lines >= MAX_REVIEW_LINES
                    || emitted_bytes.saturating_add(text.len()) > MAX_REVIEW_OUTPUT_BYTES
                {
                    truncated = true;
                    if !hunk.lines.is_empty() {
                        hunks.push(hunk);
                    }
                    break 'group;
                }
                let kind = match change.tag() {
                    ChangeTag::Delete => "removed",
                    ChangeTag::Insert => "added",
                    ChangeTag::Equal => "context",
                };
                hunk.lines.push(ReviewLine {
                    kind,
                    text: text.to_owned(),
                    old_line: change.old_index().map(|index| index + 1),
                    new_line: change.new_index().map(|index| index + 1),
                });
                emitted_lines += 1;
                emitted_bytes += text.len();
            }
        }
        hunks.push(hunk);
    }
    DiffSummary {
        hunks,
        removed_lines,
        added_lines,
        counts_exact: true,
        truncated,
    }
}

impl IdeWorkspaceRegistry {
    /// Read-only review requires an open cataloged project, capability-relative
    /// path and exact SHA-256 revision. It never saves, launches, or authorizes.
    pub fn review_source_change(
        &self,
        project_id: &str,
        relative_path: &str,
        expected_revision: &str,
        proposed_text: &str,
    ) -> Result<ChangeReview, BrowserError> {
        if proposed_text.len() as u64 > MAX_TEXT_BYTES {
            return Err(BrowserError::TooLarge);
        }
        if proposed_text.as_bytes().contains(&0) {
            return Err(BrowserError::NotText);
        }
        if expected_revision.len() != 64
            || !expected_revision.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(BrowserError::InvalidRevision);
        }
        let original = self.preview_source_file(project_id, relative_path)?;
        if original.revision != expected_revision {
            return Err(BrowserError::Conflict);
        }
        let summary = summarize_change(&original.text, proposed_text);
        Ok(ChangeReview {
            project_id: project_id.to_owned(),
            relative_path: relative_path.to_owned(),
            expected_revision: original.revision,
            proposed_revision: revision_for(proposed_text.as_bytes()),
            unchanged: original.text == proposed_text,
            removed_lines: summary.removed_lines,
            added_lines: summary.added_lines,
            counts_exact: summary.counts_exact,
            truncated: summary.truncated,
            hunks: summary.hunks,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distinct_distant_edits_produce_two_hunks_with_exact_counts() {
        let base: Vec<_> = (0..35).map(|n| format!("line-{n}\n")).collect();
        let mut next = base.clone();
        next[3] = "CHANGED-3\n".into();
        next[29] = "CHANGED-29\n".into();
        let summary = summarize_change(&base.concat(), &next.concat());
        assert_eq!(summary.hunks.len(), 2);
        assert_eq!((summary.removed_lines, summary.added_lines), (2, 2));
        assert!(summary.counts_exact);
        assert!(!summary.truncated);
        assert!(summary.hunks[0]
            .lines
            .iter()
            .any(|line| line.kind == "removed" && line.old_line == Some(4)));
        assert!(summary.hunks[1]
            .lines
            .iter()
            .any(|line| line.kind == "added" && line.new_line == Some(30)));
        assert!(summary.hunks[0].header.starts_with("@@ -"));
    }

    #[test]
    fn narrow_edit_has_context_and_separate_add_remove_lines() {
        let summary = summarize_change("one\ntwo\nthree\nfour\n", "one\ntwo\nTHREE\nfour\n");
        assert_eq!((summary.removed_lines, summary.added_lines), (1, 1));
        assert_eq!(summary.hunks.len(), 1);
        let lines = &summary.hunks[0].lines;
        assert_eq!(
            lines.iter().filter(|line| line.kind == "removed").count(),
            1
        );
        assert_eq!(lines.iter().filter(|line| line.kind == "added").count(), 1);
        assert_eq!(
            lines.iter().filter(|line| line.kind == "context").count(),
            3
        );
    }

    #[test]
    fn giant_diff_is_truncated_not_false_complete() {
        let text = "changed line\n".repeat(2_000);
        let summary = summarize_change("", &text);
        assert_eq!((summary.removed_lines, summary.added_lines), (0, 2000));
        assert!(summary.truncated);
        assert!(summary.hunks.iter().map(|h| h.lines.len()).sum::<usize>() <= MAX_REVIEW_LINES);
    }

    #[test]
    fn too_many_input_lines_refuses_inexact_preview() {
        let text = "x\n".repeat(MAX_DIFF_INPUT_LINES + 1);
        let summary = summarize_change("", &text);
        assert!(summary.truncated);
        assert!(!summary.counts_exact);
        assert!(summary.hunks.is_empty());
    }

    #[test]
    fn newline_only_and_crlf_changes_are_visible() {
        let summary = summarize_change("hello\n", "hello");
        assert_eq!((summary.removed_lines, summary.added_lines), (1, 1));
        let summary = summarize_change("a\r\nb\r\n", "a\nb\n");
        assert_eq!((summary.removed_lines, summary.added_lines), (2, 2));
    }

    #[test]
    fn pure_insert_at_start_has_correct_empty_old_range() {
        let summary = summarize_change("first\n", "new\nfirst\n");
        assert_eq!((summary.removed_lines, summary.added_lines), (0, 1));
        assert!(summary.hunks[0]
            .lines
            .iter()
            .any(|l| l.kind == "added" && l.new_line == Some(1)));
    }
}
