//! INT01-E: bounded, nonexecuting project filename search and sorted directory pages.
//! A native registered cap-std root remains authoritative. No filesystem writes,
//! symlink traversal, shell, network, source text indexing or ambient path access.
use std::{collections::{BTreeSet, VecDeque}, time::{Duration, Instant}};

use serde::Serialize;

use crate::{
    browser::{descend, validate_relative, BrowserError, ProjectEntry},
    IdeWorkspaceRegistry,
};

const MAX_QUERY_BYTES: usize = 128;
const MAX_ENTRIES_VISITED: usize = 30_000;
const MAX_DIRECTORIES_VISITED: usize = 3_000;
const MAX_DEPTH: usize = 24;
const MAX_RESULTS: usize = 60;
const PAGE_SIZE: usize = 120;
// Retain the original DEV-01 eight-second budget without reverting to its obsolete indexer.
const MAX_SCAN_DURATION: Duration = Duration::from_secs(8);

pub(crate) fn generated(name: &str) -> bool {
    // Case-insensitive like the original DEV-01 index. The native file
    // transactions module stores reversible trash in .lu-ide-trash; neither
    // that nor other private .lu-ide-* directories belong in Quick Open.
    let lower = name.to_ascii_lowercase();
    lower.starts_with(".lu-ide-")
        || matches!(
            lower.as_str(),
            ".git" | ".hg" | ".svn" | "node_modules" | "target" | "dist" | "build"
                | ".venv" | "venv" | "__pycache__" | ".next" | ".cache"
                | "site-packages" | "vendor" | ".idea" | ".turbo"
        )
}

fn scan_time_exhausted(started: Instant) -> bool {
    started.elapsed() >= MAX_SCAN_DURATION
}

fn rank(path: &str, query: &str) -> Option<u8> {
    let file = path.rsplit('/').next()?.to_lowercase();
    let full = path.to_lowercase();
    if file == query {
        Some(0)
    } else if file.starts_with(query) {
        Some(1)
    } else if file.contains(query) {
        Some(2)
    } else if full.contains(query) {
        Some(3)
    } else {
        None
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickOpenMatch {
    pub relative_path: String,
    pub size: u64,
    pub score: u8,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickOpenResult {
    pub schema_version: u32,
    pub project_id: String,
    pub query: String,
    pub matches: Vec<QuickOpenMatch>,
    pub entries_examined: usize,
    pub directories_examined: usize,
    pub truncated: bool,
    pub read_only: bool,
    pub source_files_read: bool,
    pub worktree_modified: bool,
    pub process_started: bool,
}

/// A single sorted page. The cursor is only a filename, not an authority or
/// durable point-in-time snapshot: users must refresh when the folder changes.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryPage {
    pub schema_version: u32,
    pub project_id: String,
    pub relative_dir: String,
    pub entries: Vec<ProjectEntry>,
    pub next_after: Option<String>,
    pub truncated: bool,
    pub scan_limited: bool,
    pub entries_examined: usize,
    pub read_only: bool,
}

impl IdeWorkspaceRegistry {
    /// Search filenames only, without reading their contents. A bounded BFS
    /// follows only real directories within the registered native capability.
    pub fn quick_open_source_paths(
        &self,
        project_id: &str,
        query: &str,
    ) -> Result<QuickOpenResult, BrowserError> {
        let query = query.trim().to_lowercase();
        if query.is_empty()
            || query.len() > MAX_QUERY_BYTES
            || query.chars().any(char::is_control)
            || query.contains('\\')
        {
            return Err(BrowserError::InvalidSearch);
        }
        let root = self.source_capability(project_id)?;
        let started = Instant::now();
        let mut pending = VecDeque::new();
        pending.push_back((Vec::<String>::new(), 0usize));
        let mut examined = 0usize;
        let mut dirs = 0usize;
        let mut limited = false;
        let mut candidates: Vec<(u8, String, u64)> = Vec::new();
        'walk: while let Some((parts, depth)) = pending.pop_front() {
            if dirs >= MAX_DIRECTORIES_VISITED || scan_time_exhausted(started) {
                limited = true;
                break;
            }
            dirs += 1;
            let dir = match descend(root.try_clone()?, &parts) {
                Ok(d) => d,
                Err(_) => {
                    limited = true;
                    continue;
                }
            };
            // Stable order inside each enumerated directory makes ties
            // deterministic. Directory metadata is never used as a path grant.
            let mut names = Vec::new();
            for item in dir.entries()? {
                if examined >= MAX_ENTRIES_VISITED || scan_time_exhausted(started) {
                    limited = true;
                    break 'walk;
                }
                examined += 1;
                let item = item?;
                let name = item.file_name();
                let Some(name) = name.to_str() else { continue };
                if generated(name) || validate_relative(name, false).is_err() {
                    continue;
                }
                names.push(name.to_owned());
            }
            names.sort();
            for name in names {
                if scan_time_exhausted(started) {
                    limited = true;
                    break 'walk;
                }
                let mut next = parts.clone();
                next.push(name);
                let path = next.join("/");
                if validate_relative(&path, false).is_err() {
                    continue;
                }
                let Some(leaf) = next.last() else { continue };
                let meta = match dir.symlink_metadata(leaf) {
                    Ok(m) => m,
                    Err(_) => {
                        limited = true;
                        continue;
                    }
                };
                if meta.file_type().is_symlink() {
                    continue;
                }
                if meta.is_dir() {
                    if depth + 1 < MAX_DEPTH {
                        pending.push_back((next, depth + 1));
                    } else {
                        limited = true;
                    }
                } else if meta.is_file() {
                    if let Some(score) = rank(&path, &query) {
                        candidates.push((score, path, meta.len()));
                        if candidates.len() > MAX_RESULTS * 2 {
                            candidates.sort_by(|a, b| {
                                (a.0, &a.1).cmp(&(b.0, &b.1))
                            });
                            candidates.truncate(MAX_RESULTS);
                            limited = true;
                        }
                    }
                }
            }
        }
        candidates.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
        if candidates.len() > MAX_RESULTS {
            candidates.truncate(MAX_RESULTS);
            limited = true;
        }
        Ok(QuickOpenResult {
            schema_version: 1,
            project_id: project_id.to_owned(),
            query,
            matches: candidates
                .into_iter()
                .map(|(score, relative_path, size)| QuickOpenMatch {
                    relative_path,
                    size,
                    score,
                })
                .collect(),
            entries_examined: examined,
            directories_examined: dirs,
            truncated: limited,
            read_only: true,
            source_files_read: false,
            worktree_modified: false,
            process_started: false,
        })
    }

    /// Sorted bounded directory pages. Re-enumerating each page instead of
    /// caching every filename keeps large-folder memory bounded.
    pub fn list_source_directory_page(
        &self,
        project_id: &str,
        relative_dir: &str,
        after: Option<&str>,
    ) -> Result<DirectoryPage, BrowserError> {
        let components = validate_relative(relative_dir, true)?;
        if let Some(cursor) = after {
            if cursor.is_empty()
                || cursor.len() > 255
                || validate_relative(cursor, false).is_err()
                || cursor.contains('/')
            {
                return Err(BrowserError::InvalidPath);
            }
        }
        let root = self.source_capability(project_id)?;
        let folder = descend(root, &components)?;
        let started = Instant::now();
        let mut top = BTreeSet::new();
        let mut examined = 0usize;
        let mut candidates = 0usize;
        let mut scan_limited = false;
        for item in folder.entries()? {
            if examined >= MAX_ENTRIES_VISITED || scan_time_exhausted(started) {
                scan_limited = true;
                break;
            }
            examined += 1;
            let item = item?;
            let name = item.file_name();
            let Some(name) = name.to_str() else { continue };
            if validate_relative(name, false).is_err() || name.contains('/') {
                continue;
            }
            if after.is_some_and(|cursor| name <= cursor) {
                continue;
            }
            candidates += 1;
            top.insert(name.to_owned());
            if top.len() > PAGE_SIZE {
                if let Some(last) = top.iter().next_back().cloned() {
                    top.remove(&last);
                }
            }
        }
        let mut entries = Vec::new();
        for name in top {
            let relative_path = if relative_dir.is_empty() {
                name.clone()
            } else {
                format!("{relative_dir}/{name}")
            };
            if validate_relative(&relative_path, false).is_err() {
                continue;
            }
            let meta = match folder.symlink_metadata(&name) {
                Ok(m) => m,
                Err(_) => {
                    scan_limited = true;
                    continue;
                }
            };
            let kind = if meta.file_type().is_symlink() {
                "symlink"
            } else if meta.is_dir() {
                "directory"
            } else if meta.is_file() {
                "file"
            } else {
                "other"
            };
            entries.push(ProjectEntry {
                name,
                relative_path,
                kind,
                size: if kind == "file" { meta.len() } else { 0 },
            });
        }
        let has_more = candidates > PAGE_SIZE;
        let next_after = if !scan_limited && has_more {
            entries.last().map(|entry| entry.name.clone())
        } else {
            None
        };
        Ok(DirectoryPage {
            schema_version: 1,
            project_id: project_id.to_owned(),
            relative_dir: relative_dir.to_owned(),
            entries,
            next_after,
            truncated: has_more || scan_limited,
            scan_limited,
            entries_examined: examined,
            read_only: true,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{generated, scan_time_exhausted};
    use std::time::{Duration, Instant};

    #[test]
    fn historical_index_private_and_casefolded_ignored_names_stay_hidden() {
        for name in [".lu-ide-trash", ".Lu-IDE-Recovery", ".GIT", "NODE_MODULES",
                     "Target", "vendor", ".VENV"] {
            assert!(generated(name), "{name}");
        }
        assert!(!generated("source.rs"));
    }

    #[test]
    fn historical_eight_second_budget_fails_closed_without_sleeping() {
        let stale = Instant::now().checked_sub(Duration::from_secs(9)).unwrap();
        assert!(scan_time_exhausted(stale));
        assert!(!scan_time_exhausted(Instant::now()));
    }
}
