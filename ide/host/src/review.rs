//! IDE-08 foundation: a bounded, read-only source-change review.
//!
//! Both human drafts and future Lu-generated proposals are untrusted text.
//! Reviewing a proposal never writes a file, starts a worker, or grants
//! authority. An independent human action must authorize any later save.
//! The existing save path revalidates the source revision before replacement.
use serde::Serialize;

use crate::{
    browser::{revision_for, BrowserError, MAX_TEXT_BYTES},
    IdeWorkspaceRegistry,
};

const MAX_REVIEW_LINES: usize = 240;
const CONTEXT_LINES: usize = 3;
const MAX_REVIEW_OUTPUT_BYTES: usize = 32 * 1024;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewLine {
    /// "context", "removed" or "added". Always rendered as plain text.
    pub kind: &'static str,
    pub text: String,
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
    pub truncated: bool,
    pub lines: Vec<ReviewLine>,
}

fn lines(text: &str) -> Vec<&str> {
    // split_inclusive retains line ending differences; the UI renders each
    // item as an escaped string and can show them in a monospace row.
    text.split_inclusive('\n').collect()
}

/// Limited context-first change review. A bounded preview is not a complete
/// diff for a file with multiple distant edits; `truncated` tells the user
/// when the full changed region cannot be displayed. A later diff engine
/// can replace this without changing the no-effects contract.
fn summarize_change(
    old: &str,
    proposed: &str,
) -> (Vec<ReviewLine>, usize, usize, bool) {
    let old_lines = lines(old);
    let new_lines = lines(proposed);
    let mut common_start = 0;
    while common_start < old_lines.len()
        && common_start < new_lines.len()
        && old_lines[common_start] == new_lines[common_start]
    {
        common_start += 1;
    }
    let mut common_end = 0;
    while common_end < old_lines.len().saturating_sub(common_start)
        && common_end < new_lines.len().saturating_sub(common_start)
        && old_lines[old_lines.len() - common_end - 1]
            == new_lines[new_lines.len() - common_end - 1]
    {
        common_end += 1;
    }
    let removed = &old_lines[common_start..old_lines.len() - common_end];
    let added = &new_lines[common_start..new_lines.len() - common_end];
    let mut candidates: Vec<(&'static str, &str)> = Vec::new();
    for value in &old_lines[common_start.saturating_sub(CONTEXT_LINES)..common_start] {
        candidates.push(("context", value));
    }
    for value in removed {
        candidates.push(("removed", value));
    }
    for value in added {
        candidates.push(("added", value));
    }
    let suffix_start = new_lines.len() - common_end;
    for value in &new_lines[suffix_start..(suffix_start + common_end.min(CONTEXT_LINES))] {
        candidates.push(("context", value));
    }
    let mut result = Vec::new();
    let mut rendered_bytes = 0;
    let mut truncated = false;
    for (kind, text) in candidates {
        if result.len() >= MAX_REVIEW_LINES || rendered_bytes + text.len() > MAX_REVIEW_OUTPUT_BYTES {
            truncated = true;
            break;
        }
        result.push(ReviewLine {
            kind,
            text: text.to_owned(),
        });
        rendered_bytes += text.len();
    }
    (result, removed.len(), added.len(), truncated)
}

impl IdeWorkspaceRegistry {
    /// Return a small, non-executing change review for a registered open
    /// project. Reject a stale base so that a proposed change never silently
    /// refers to different disk content.
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
        if expected_revision.len() != 64 || !expected_revision.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(BrowserError::InvalidRevision);
        }
        let original = self.preview_source_file(project_id, relative_path)?;
        if original.revision != expected_revision {
            return Err(BrowserError::Conflict);
        }
        let (lines, removed_lines, added_lines, truncated) =
            summarize_change(&original.text, proposed_text);
        Ok(ChangeReview {
            project_id: project_id.to_owned(),
            relative_path: relative_path.to_owned(),
            expected_revision: original.revision,
            proposed_revision: revision_for(proposed_text.as_bytes()),
            unchanged: original.text == proposed_text,
            removed_lines,
            added_lines,
            truncated,
            lines,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_and_changed_region_are_separated() {
        let (lines, removed, added, truncated) =
            summarize_change("one\ntwo\nthree\nfour\n", "one\ntwo\nTHREE\nfour\n");
        assert_eq!((removed, added), (1, 1));
        assert!(!truncated);
        assert_eq!(lines.iter().filter(|line| line.kind == "removed").count(), 1);
        assert_eq!(lines.iter().filter(|line| line.kind == "added").count(), 1);
        assert_eq!(lines.iter().filter(|line| line.kind == "context").count(), 3);
    }

    #[test]
    fn giant_change_never_returns_unbounded_review() {
        let text = "changed line\n".repeat(2000);
        let (lines, removed, added, truncated) = summarize_change("", &text);
        assert_eq!(removed, 0);
        assert_eq!(added, 2000);
        assert!(truncated);
        assert!(lines.len() <= MAX_REVIEW_LINES);
    }

    #[test]
    fn newline_only_change_is_visible() {
        let (lines, removed, added, _) = summarize_change("hello\n", "hello");
        assert_eq!((removed, added), (1, 1));
        assert_eq!(lines.len(), 2);
    }
}
