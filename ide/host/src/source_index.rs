//! DEV-01.6: one bounded native project-wide file-index snapshot.
//! Avoids per-directory WebView IPC; never reads file contents or executes
//! project code. The resulting snapshot is ephemeral: no stale durable cache.
use std::{collections::{BTreeSet, VecDeque}, time::{Duration, Instant}};

use serde::Serialize;

use crate::{
    browser::{descend, validate_relative, BrowserError},
    IdeWorkspaceRegistry,
};

const MAX_DIRECTORIES: usize = 128;
const MAX_FILES: usize = 4000;
const MAX_ENTRIES_PER_DIRECTORY: usize = 1000;
const MAX_TOTAL_ENTRIES: usize = 20000;
const MAX_INDEX_DURATION: Duration = Duration::from_secs(8);

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFileIndex {
    pub project_id: String,
    pub paths: Vec<String>,
    pub directories_visited: usize,
    pub entries_examined: usize,
    pub ignored_entries: usize,
    pub truncated: bool,
    pub elapsed_ms: u64,
}

fn ignored_directory(name: &str) -> bool {
    matches!(name.to_ascii_lowercase().as_str(),
        ".git" | ".lu-ide-trash" | "node_modules" | ".venv" |
        "venv" | "__pycache__" | "target" | ".next" | "dist"
    ) || name.to_ascii_lowercase().starts_with(".lu-ide-")
}

impl IdeWorkspaceRegistry {
    /// Every call checks the current native-picker-held project authority.
    /// The indexing job may finish after the browser switches projects; the
    /// frontend must reject responses whose native project ID is no longer
    /// current. Host work itself has strict time/directory/file budgets.
    pub fn index_project_files(
        &self, project_id: &str,
    ) -> Result<ProjectFileIndex, BrowserError> {
        let _root = self.source_capability(project_id)?;
        let started = Instant::now();
        let mut queue = VecDeque::from([String::new()]);
        let mut visited = BTreeSet::new();
        let mut files = Vec::new();
        let mut examined = 0;
        let mut ignored = 0;
        let mut truncated = false;

        while let Some(relative_dir) = queue.pop_front() {
            if visited.len() >= MAX_DIRECTORIES ||
                files.len() >= MAX_FILES ||
                examined >= MAX_TOTAL_ENTRIES ||
                started.elapsed() >= MAX_INDEX_DURATION {
                truncated = true;
                break;
            }
            if !visited.insert(relative_dir.clone()) { continue; }
            let parts = validate_relative(&relative_dir, true)?;
            let dir = descend(self.source_capability(project_id)?, &parts)?;
            let mut batch = Vec::new();
            let mut seen_here = 0;
            for entry in dir.entries()? {
                if seen_here >= MAX_ENTRIES_PER_DIRECTORY ||
                    examined >= MAX_TOTAL_ENTRIES ||
                    started.elapsed() >= MAX_INDEX_DURATION {
                    truncated = true;
                    break;
                }
                seen_here += 1;
                examined += 1;
                let entry = entry?;
                let name = entry.file_name();
                let Some(name) = name.to_str() else {
                    ignored += 1;
                    continue;
                };
                if name == "." || name == ".." || name.contains('\\') ||
                    name.chars().any(char::is_control) || ignored_directory(name) {
                    ignored += 1;
                    continue;
                }
                let relative_path = if relative_dir.is_empty() {
                    name.to_owned()
                } else {
                    format!("{relative_dir}/{name}")
                };
                if validate_relative(&relative_path, false).is_err() {
                    ignored += 1;
                    continue;
                }
                let file_type = entry.file_type()?;
                if file_type.is_symlink() {
                    ignored += 1;
                    continue;
                }
                if file_type.is_file() {
                    batch.push((relative_path, false));
                } else if file_type.is_dir() {
                    batch.push((relative_path, true));
                } else {
                    ignored += 1;
                }
            }
            // Deterministic ordering within the retained bounded batch, not
            // the underlying platform-dependent filesystem iteration order.
            batch.sort_by(|a, b| a.0.cmp(&b.0));
            for (path, is_directory) in batch {
                if is_directory {
                    if visited.len() + queue.len() < MAX_DIRECTORIES {
                        queue.push_back(path);
                    } else {
                        truncated = true;
                    }
                } else if files.len() < MAX_FILES {
                    files.push(path);
                } else {
                    truncated = true;
                }
            }
        }
        if !queue.is_empty() { truncated = true; }
        files.sort();
        files.dedup();
        let elapsed_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
        Ok(ProjectFileIndex {
            project_id: project_id.to_owned(), paths: files,
            directories_visited: visited.len(), entries_examined: examined,
            ignored_entries: ignored, truncated, elapsed_ms,
        })
    }
}
