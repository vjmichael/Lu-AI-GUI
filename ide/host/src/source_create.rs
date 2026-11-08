//! DEV-01.2 human-initiated, capability-scoped *exclusive* creation.
//! No ambient project paths, recursive parent creation, overwrite, execution,
//! or automatic installation. The source root comes only from the OS picker.
//! External cross-process writers remain outside this standalone registry's lock.
use std::io::{self, Write};

use cap_std::fs::OpenOptions;
use serde::Serialize;

use crate::{
    browser::{descend, revision_for, validate_relative, BrowserError},
    IdeWorkspaceRegistry,
};

const MAX_SIBLING_SCAN: usize = 4096;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceCreateReceipt {
    pub project_id: String,
    pub relative_path: String,
    pub kind: &'static str,
    pub revision: Option<String>,
    pub created: bool,
}

fn source_component_allowed(component: &str) -> bool {
    // Refuse Git internals and IDE's private temporary names even if a folder
    // is user-controlled. Protect Windows portablility and DOS device aliases.
    let lower = component.to_ascii_lowercase();
    if lower == ".git" || lower == ".lu-ide" || lower.starts_with(".lu-ide-")
        || component.ends_with(' ') || component.ends_with('.')
        || component.chars().any(|c| matches!(c, '<' | '>' | '"' | '|' | '?' | '*'))
    {
        return false;
    }
    let stem = lower.split('.').next().unwrap_or("");
    if matches!(stem, "con" | "prn" | "aux" | "nul") {
        return false;
    }
    if stem.len() == 4 && (stem.starts_with("com") || stem.starts_with("lpt"))
        && matches!(stem.as_bytes()[3], b'1'..=b'9')
    {
        return false;
    }
    true
}

pub(crate) fn portable_components(parts: &[String]) -> Result<(), BrowserError> {
    if parts.iter().all(|part| source_component_allowed(part)) {
        Ok(())
    } else {
        Err(BrowserError::InvalidPath)
    }
}

impl IdeWorkspaceRegistry {
    /// Create one empty UTF-8 file or one directory inside the selected
    /// project. The parent must already exist and must not be a symlink.
    ///
    /// Existing targets are never overwritten: native create_new/mkdir
    /// exclusively decides the result (not a check-then-write).
    pub fn create_source_entry(
        &self,
        project_id: &str,
        relative_dir: &str,
        name: &str,
        kind: &str,
    ) -> Result<SourceCreateReceipt, BrowserError> {
        if kind != "file" && kind != "directory" {
            return Err(BrowserError::InvalidPath);
        }
        let parent_parts = validate_relative(relative_dir, true)?;
        let name_parts = validate_relative(name, false)?;
        if name_parts.len() != 1 {
            return Err(BrowserError::InvalidPath);
        }
        portable_components(&parent_parts)?;
        portable_components(&name_parts)?;
        // Recheck the complete path's length/depth through the native validator.
        let relative_path = if relative_dir.is_empty() {
            name.to_owned()
        } else {
            format!("{relative_dir}/{name}")
        };
        validate_relative(&relative_path, false)?;

        // Serialize IDE-origin filesystem mutations just like explicit Save.
        let _guard = self.save_lock.lock().map_err(|_| BrowserError::HostUnavailable)?;
        let base = self.source_capability(project_id)?;
        let parent = descend(base, &parent_parts)?;

        // Reject portable case-only aliases even on Linux filesystems that
        // would otherwise allow both variants in a single directory.
        let folded = name.to_lowercase();
        let mut count = 0;
        for item in parent.entries()? {
            let item = item?;
            count += 1;
            if count > MAX_SIBLING_SCAN {
                return Err(BrowserError::TooLarge);
            }
            if item.file_name().to_string_lossy().to_lowercase() == folded {
                return Err(BrowserError::AlreadyExists);
            }
        }

        #[cfg(unix)]
        let durable_parent = parent.open(".")?;
        #[cfg(unix)]
        durable_parent.sync_all()?;
        if kind == "file" {
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            let mut file = parent.open_with(name, &options)
                .map_err(|error| match error.kind() {
                    io::ErrorKind::AlreadyExists => BrowserError::AlreadyExists,
                    _ => BrowserError::Io(error),
                })?;
            file.flush()?;
            file.sync_all()?;
        } else {
            parent.create_dir(name).map_err(|error| match error.kind() {
                io::ErrorKind::AlreadyExists => BrowserError::AlreadyExists,
                _ => BrowserError::Io(error),
            })?;
        }
        #[cfg(unix)]
        durable_parent.sync_all()?;
        Ok(SourceCreateReceipt {
            project_id: project_id.to_owned(),
            relative_path,
            kind: if kind == "file" { "file" } else { "directory" },
            revision: (kind == "file").then(|| revision_for(&[])),
            created: true,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disallows_private_and_windows_alias_names() {
        for name in [".git", ".GIT", ".lu-ide-trash", "con", "CON.txt", "NUL",
                     "com1.rs", "LPT9", "space ", "dot.", "star*", "pipe|"] {
            assert!(!source_component_allowed(name), "{name:?}");
        }
        for name in ["main.rs", ".gitignore", "src", "foo bar", "hello-世界.py"] {
            assert!(source_component_allowed(name), "{name:?}");
        }
    }
}
