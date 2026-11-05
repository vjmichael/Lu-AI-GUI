//! Read-only source browser for IDE-02. No project code is executed.
//!
//! File resolution is relative to a capability directory. This is important:
//! a JSON path check alone cannot protect against symlink/TOCTOU escapes.
use std::{
    io::{self, Read},
    path::{Component, Path},
};

use cap_std::{ambient_authority, fs::Dir};
use serde::Serialize;

use crate::{safe_root, wire, IdeWorkspaceRegistry};

const MAX_ENTRY_COUNT: usize = 400;
const MAX_TEXT_BYTES: u64 = 512 * 1024;
const MAX_RELATIVE_BYTES: usize = 1024;
const MAX_DEPTH: usize = 32;

#[derive(Debug)]
pub enum BrowserError {
    InvalidProject,
    NotOpen,
    InvalidPath,
    NotDirectory,
    NotFile,
    NotText,
    TooLarge,
    Io(io::Error),
    HostUnavailable,
}
impl BrowserError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidProject => "PROJECT_NOT_FOUND",
            Self::NotOpen => "PROJECT_NOT_OPEN",
            Self::InvalidPath => "INVALID_RELATIVE_PATH",
            Self::NotDirectory => "NOT_A_DIRECTORY",
            Self::NotFile => "NOT_A_FILE",
            Self::NotText => "NOT_UTF8_TEXT",
            Self::TooLarge => "FILE_TOO_LARGE",
            Self::Io(_) => "BROWSE_UNAVAILABLE",
            Self::HostUnavailable => "HOST_UNAVAILABLE",
        }
    }
}
impl From<io::Error> for BrowserError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectEntry {
    pub name: String,
    pub relative_path: String,
    pub kind: &'static str,
    pub size: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryListing {
    pub project_id: String,
    pub relative_dir: String,
    pub entries: Vec<ProjectEntry>,
    pub truncated: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextPreview {
    pub project_id: String,
    pub relative_path: String,
    pub text: String,
    pub byte_length: u64,
}

fn validate_relative(path: &str, allow_root: bool) -> Result<Vec<String>, BrowserError> {
    if path.len() > MAX_RELATIVE_BYTES || path.contains('\\') || path.chars().any(char::is_control)
    {
        return Err(BrowserError::InvalidPath);
    }
    if path.is_empty() && allow_root {
        return Ok(Vec::new());
    }
    if path.is_empty() || path.starts_with('/') || path.ends_with('/') || path.contains("//") {
        return Err(BrowserError::InvalidPath);
    }
    let mut parts = Vec::new();
    for item in path.split('/') {
        if item.is_empty() || item == "." || item == ".." || item.contains(':') {
            return Err(BrowserError::InvalidPath);
        }
        if Path::new(item).components().count() != 1
            || !matches!(
                Path::new(item).components().next(),
                Some(Component::Normal(_))
            )
        {
            return Err(BrowserError::InvalidPath);
        }
        parts.push(item.to_string());
    }
    if parts.len() > MAX_DEPTH {
        return Err(BrowserError::InvalidPath);
    }
    Ok(parts)
}

fn descend(mut dir: Dir, parts: &[String]) -> Result<Dir, BrowserError> {
    for component in parts {
        let meta = dir.symlink_metadata(component)?;
        if !meta.is_dir() || meta.file_type().is_symlink() {
            return Err(BrowserError::NotDirectory);
        }
        // cap-std does not allow following a link beyond the held directory.
        dir = dir.open_dir(component)?;
    }
    Ok(dir)
}

impl IdeWorkspaceRegistry {
    /// Create an ephemeral capability from the host-only catalog. The identity
    /// of the picked root is revalidated on each call. All subsequent lookup
    /// happens within the cap-std Dir (never ambient absolute path joins).
    fn source_capability(&self, project_id: &str) -> Result<Dir, BrowserError> {
        if !wire::valid_project_id(project_id) {
            return Err(BrowserError::InvalidProject);
        }
        let records = self
            .records
            .lock()
            .map_err(|_| BrowserError::HostUnavailable)?;
        let active = self
            .active
            .lock()
            .map_err(|_| BrowserError::HostUnavailable)?;
        if !active.contains(project_id) {
            return Err(BrowserError::NotOpen);
        }
        let record = records
            .get(project_id)
            .ok_or(BrowserError::InvalidProject)?;
        let root = safe_root(&record.root).map_err(|_| BrowserError::InvalidProject)?;
        if root != record.root {
            return Err(BrowserError::InvalidProject);
        }
        let dir = Dir::open_ambient_dir(&root, ambient_authority())?;
        Ok(dir)
    }

    /// List one directory only (no recursive scan, no executable workers).
    /// Huge folders are capped and callers are told that results are partial.
    pub fn list_source_directory(
        &self,
        project_id: &str,
        relative_dir: &str,
    ) -> Result<DirectoryListing, BrowserError> {
        let parts = validate_relative(relative_dir, true)?;
        let base = self.source_capability(project_id)?;
        let folder = descend(base, &parts)?;
        let mut entries = Vec::new();
        let mut truncated = false;
        for entry in folder.entries()? {
            let entry = entry?;
            let os_name = entry.file_name();
            let Some(name) = os_name.to_str() else {
                continue;
            };
            if name == "."
                || name == ".."
                || name.contains('\\')
                || name.chars().any(char::is_control)
            {
                continue;
            }
            let ty = entry.file_type()?;
            let kind = if ty.is_symlink() {
                "symlink"
            } else if ty.is_dir() {
                "directory"
            } else if ty.is_file() {
                "file"
            } else {
                "other"
            };
            // Entries beyond this limit are never sent to the UI.
            if entries.len() == MAX_ENTRY_COUNT {
                truncated = true;
                break;
            }
            let size = if kind == "file" {
                entry.metadata()?.len()
            } else {
                0
            };
            let relative_path = if relative_dir.is_empty() {
                name.to_owned()
            } else {
                format!("{relative_dir}/{name}")
            };
            if validate_relative(&relative_path, false).is_err() {
                continue;
            }
            entries.push(ProjectEntry {
                name: name.to_string(),
                relative_path,
                kind,
                size,
            });
        }
        entries.sort_by(|a, b| {
            let rank = |e: &ProjectEntry| if e.kind == "directory" { 0 } else { 1 };
            rank(a)
                .cmp(&rank(b))
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        Ok(DirectoryListing {
            project_id: project_id.into(),
            relative_dir: relative_dir.into(),
            entries,
            truncated,
        })
    }

    /// Read a small UTF-8 text file only. Binary/oversized files are rejected.
    /// Importantly no file content is ever evaluated, parsed as a command, or
    /// sent to Lu without a separate future explicit handoff.
    pub fn preview_source_file(
        &self,
        project_id: &str,
        relative_path: &str,
    ) -> Result<TextPreview, BrowserError> {
        let mut parts = validate_relative(relative_path, false)?;
        let leaf = parts.pop().ok_or(BrowserError::InvalidPath)?;
        let base = self.source_capability(project_id)?;
        let parent = descend(base, &parts)?;
        let metadata = parent.symlink_metadata(&leaf)?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(BrowserError::NotFile);
        }
        if metadata.len() > MAX_TEXT_BYTES {
            return Err(BrowserError::TooLarge);
        }
        let mut file = parent.open(&leaf)?;
        let actual = file.metadata()?;
        if !actual.is_file() {
            return Err(BrowserError::NotFile);
        }
        if actual.len() > MAX_TEXT_BYTES {
            return Err(BrowserError::TooLarge);
        }
        let mut bytes = Vec::with_capacity(actual.len() as usize);
        file.take(MAX_TEXT_BYTES + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_TEXT_BYTES {
            return Err(BrowserError::TooLarge);
        }
        if bytes.iter().any(|b| *b == 0) {
            return Err(BrowserError::NotText);
        }
        let text = String::from_utf8(bytes).map_err(|_| BrowserError::NotText)?;
        Ok(TextPreview {
            project_id: project_id.into(),
            relative_path: relative_path.into(),
            byte_length: text.len() as u64,
            text,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn path_validation_blocks_traversal_and_ambient_paths() {
        for path in [
            "../secret",
            "a/../../secret",
            "/etc/passwd",
            "C:/Windows",
            "a\\b",
            "./file",
            "a//b",
            "a/",
            "a/..",
            "x:\\etc",
            "file\0bad",
        ] {
            assert!(validate_relative(path, false).is_err(), "{path:?}");
        }
        assert!(validate_relative("", true).unwrap().is_empty());
        assert_eq!(
            validate_relative("src/main.rs", false).unwrap(),
            ["src", "main.rs"]
        );
    }
}
