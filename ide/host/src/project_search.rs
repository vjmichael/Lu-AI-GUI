//! B3: bounded project-wide saved-text search. No filesystem writes, shell
//! execution, indexing daemon, Lu bridge, or implicit network access.
use std::{collections::VecDeque, io::Read};

use serde::Serialize;

use crate::{
    browser::{descend, revision_for, validate_relative, BrowserError},
    IdeWorkspaceRegistry,
};

const MAX_QUERY_BYTES: usize = 128;
const MAX_FILES: usize = 320;
const MAX_ENTRIES: usize = 3000;
const MAX_ENTRIES_PER_DIR: usize = 400;
const MAX_DEPTH: usize = 12;
const MAX_FILE_BYTES: u64 = 256 * 1024;
const MAX_TOTAL_BYTES: u64 = 16 * 1024 * 1024;
const MAX_MATCHES: usize = 240;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSearchMatch {
    pub relative_path: String,
    pub revision: String,
    pub line: usize,
    pub column_utf16: usize,
    pub match_length_utf16: usize,
    pub from_byte: u64,
    pub to_byte: u64,
    pub preview: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSearchResult {
    pub schema_version: u32,
    pub project_id: String,
    pub query: String,
    pub case_sensitive: bool,
    pub scope: &'static str,
    pub files_scanned: usize,
    pub entries_seen: usize,
    pub bytes_scanned: u64,
    pub skipped_files: usize,
    pub matches: Vec<ProjectSearchMatch>,
    pub truncated: bool,
}

/// Literal saved-text match offsets in ORIGINAL UTF-8 bytes, not positions in
/// lowercased text. The standard-library Unicode lowercase transform can
/// expand characters; a valid hit must start AND end at original char bounds.
/// No new dependency is admitted into the already-reviewed Tauri Cargo.lock.
fn literal_offsets(line: &str, query: &str, case_sensitive: bool) -> Vec<(usize, usize)> {
    if case_sensitive {
        return line
            .match_indices(query)
            .map(|(start, found)| (start, start + found.len()))
            .collect();
    }
    let normalized_query = query.to_lowercase();
    let mut folded = String::with_capacity(line.len());
    let mut boundaries = Vec::with_capacity(line.len() + 1);
    boundaries.push((0usize, 0usize));
    for (offset, ch) in line.char_indices() {
        for lower in ch.to_lowercase() {
            folded.push(lower);
        }
        boundaries.push((folded.len(), offset + ch.len_utf8()));
    }
    folded
        .match_indices(&normalized_query)
        .filter_map(|(start, found)| {
            let end = start + found.len();
            let first = boundaries.binary_search_by_key(&start, |pair| pair.0).ok()?;
            let last = boundaries.binary_search_by_key(&end, |pair| pair.0).ok()?;
            Some((boundaries[first].1, boundaries[last].1))
        })
        .collect()
}

fn excluded(name: &str) -> bool {
    matches!(
        name,
        ".git"
            | ".hg"
            | ".svn"
            | "node_modules"
            | "target"
            | "dist"
            | "build"
            | ".venv"
            | "venv"
            | "__pycache__"
            | ".next"
            | ".cache"
            | "site-packages"
            | "vendor"
            | ".idea"
            | ".turbo"
    )
}

fn excerpt(line: &str, start: usize, end: usize) -> String {
    let char_start = line[..start].chars().count();
    let char_end = line[..end].chars().count();
    let from = char_start.saturating_sub(45);
    let to = (char_end + 85).min(line.chars().count());
    let mut text: String = line.chars().skip(from).take(to - from).collect();
    if from > 0 {
        text.insert_str(0, "…");
    }
    if to < line.chars().count() {
        text.push('…');
    }
    text
}

impl IdeWorkspaceRegistry {
    /// Reads only registered, currently open project roots. The result is a
    /// snapshot of *saved disk bytes*, never unsaved CodeMirror buffers.
    /// Entries are sorted for deterministic traversal and every discovered
    /// file carries the SHA-256 of the bytes actually inspected.
    pub fn find_in_project(
        &self,
        project_id: &str,
        query: &str,
        case_sensitive: bool,
    ) -> Result<ProjectSearchResult, BrowserError> {
        if query.trim().is_empty()
            || query.len() > MAX_QUERY_BYTES
            || query.chars().any(char::is_control)
        {
            return Err(BrowserError::InvalidSearch);
        }
        let root = self.source_capability(project_id)?;
        let mut result = ProjectSearchResult {
            schema_version: 1,
            project_id: project_id.to_owned(),
            query: query.to_owned(),
            case_sensitive,
            scope: "saved_utf8_text_excluding_generated_directories",
            files_scanned: 0,
            entries_seen: 0,
            bytes_scanned: 0,
            skipped_files: 0,
            matches: Vec::new(),
            truncated: false,
        };
        let mut directories = VecDeque::new();
        directories.push_back((Vec::<String>::new(), 0usize));
        'walk: while let Some((parts, depth)) = directories.pop_front() {
            let folder = match descend(root.try_clone()?, &parts) {
                Ok(folder) => folder,
                Err(_) => {
                    result.truncated = true;
                    continue;
                }
            };
            let mut names = Vec::new();
            for item in folder.entries()? {
                let entry = item?;
                result.entries_seen += 1;
                if result.entries_seen > MAX_ENTRIES || names.len() >= MAX_ENTRIES_PER_DIR {
                    result.truncated = true;
                    break 'walk;
                }
                let name = entry.file_name();
                let Some(name) = name.to_str() else { continue };
                if excluded(name) || validate_relative(name, false).is_err() {
                    continue;
                }
                names.push(name.to_owned());
            }
            names.sort();
            for name in names {
                let relative_path = parts
                    .iter()
                    .chain(std::iter::once(&name))
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .join("/");
                if validate_relative(&relative_path, false).is_err() {
                    result.truncated = true;
                    continue;
                }
                let meta = match folder.symlink_metadata(&name) {
                    Ok(meta) => meta,
                    Err(_) => {
                        result.truncated = true;
                        continue;
                    }
                };
                if meta.file_type().is_symlink() {
                    continue;
                }
                if meta.is_dir() {
                    if depth >= MAX_DEPTH {
                        result.truncated = true;
                    } else {
                        let mut child = parts.clone();
                        child.push(name);
                        directories.push_back((child, depth + 1));
                    }
                    continue;
                }
                if !meta.is_file() {
                    continue;
                }
                if meta.len() > MAX_FILE_BYTES {
                    result.skipped_files += 1;
                    continue;
                }
                if result.files_scanned >= MAX_FILES
                    || result.bytes_scanned.saturating_add(meta.len()) > MAX_TOTAL_BYTES
                {
                    result.truncated = true;
                    break 'walk;
                }
                let file = match folder.open(&name) {
                    Ok(f) => f,
                    Err(_) => {
                        result.skipped_files += 1;
                        continue;
                    }
                };
                let actual = match file.metadata() {
                    Ok(m) if m.is_file() && m.len() <= MAX_FILE_BYTES => m,
                    _ => {
                        result.skipped_files += 1;
                        continue;
                    }
                };
                let mut bytes = Vec::with_capacity(actual.len() as usize);
                if file
                    .take(MAX_FILE_BYTES + 1)
                    .read_to_end(&mut bytes)
                    .is_err()
                    || bytes.len() as u64 > MAX_FILE_BYTES
                {
                    result.skipped_files += 1;
                    continue;
                }
                result.files_scanned += 1;
                result.bytes_scanned += bytes.len() as u64;
                if bytes.contains(&0) {
                    result.skipped_files += 1;
                    continue;
                }
                let Ok(text) = std::str::from_utf8(&bytes) else {
                    result.skipped_files += 1;
                    continue;
                };
                let revision = revision_for(&bytes);
                let mut line_start_byte = 0usize;
                for (index, raw) in text.split_inclusive('\n').enumerate() {
                    let line = raw.strip_suffix('\n').unwrap_or(raw);
                    let line = line.strip_suffix('\r').unwrap_or(line);
                    for (found_start, found_end) in literal_offsets(line, query, case_sensitive) {
                        result.matches.push(ProjectSearchMatch {
                            relative_path: relative_path.clone(),
                            revision: revision.clone(),
                            line: index + 1,
                            column_utf16: line[..found_start].encode_utf16().count() + 1,
                            match_length_utf16: line[found_start..found_end]
                                .encode_utf16()
                                .count(),
                            from_byte: (line_start_byte + found_start) as u64,
                            to_byte: (line_start_byte + found_end) as u64,
                            preview: excerpt(line, found_start, found_end),
                        });
                        if result.matches.len() >= MAX_MATCHES {
                            result.truncated = true;
                            break 'walk;
                        }
                    }
                    line_start_byte += raw.len();
                }
            }
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unicode_lowercase_keeps_original_byte_offsets_and_char_boundaries() {
        let text = "α😀NEEDLE İ I ẞ";
        assert_eq!(literal_offsets(text, "needle", false), vec![(6, 12)]);
        assert_eq!(literal_offsets(text, "needle", true), Vec::new());
        // Unicode lowercase expands U+0130 into i + combining dot; neither
        // an inner expansion nor a partial source scalar is a valid hit.
        assert_eq!(literal_offsets("İ", "i", false), Vec::new());
        assert_eq!(literal_offsets("İ", "İ", false), vec![(0, 2)]);
        assert_eq!(literal_offsets("ẞ", "ß", false), vec![(0, 3)]);
    }

    #[test]
    fn excerpt_keeps_the_matched_text_visible() {
        let line = format!("{}TARGET{}", "a".repeat(160), "b".repeat(160));
        let view = excerpt(&line, 160, 166);
        assert!(view.contains("TARGET"));
        assert!(view.len() < line.len());
    }
}
