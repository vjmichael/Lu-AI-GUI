//! R3: explicit human-initiated source saves. No build/execution commands.
//!
//! This is a bounded, capability-relative, compare-before-replace operation.
//! It is NOT a guarantee of conflict-free writes against an uncooperative
//! external writer racing between the final check and the atomic rename.
use std::io::{Read, Write};

use cap_std::fs::{Dir, OpenOptions};

use crate::{
    browser::{descend, revision_for, validate_relative, BrowserError, TextPreview, MAX_TEXT_BYTES},
    fresh_project_id, IdeWorkspaceRegistry,
};

fn read_existing(parent: &Dir, leaf: &str) -> Result<Vec<u8>, BrowserError> {
    let meta = parent.symlink_metadata(leaf)?;
    if meta.file_type().is_symlink() || !meta.is_file() {
        return Err(BrowserError::NotFile);
    }
    if meta.len() > MAX_TEXT_BYTES {
        return Err(BrowserError::TooLarge);
    }
    let mut bytes = Vec::with_capacity(meta.len() as usize);
    parent.open(leaf)?.take(MAX_TEXT_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_TEXT_BYTES {
        return Err(BrowserError::TooLarge);
    }
    if bytes.contains(&0) || std::str::from_utf8(&bytes).is_err() {
        return Err(BrowserError::NotText);
    }
    Ok(bytes)
}

fn valid_revision(revision: &str) -> bool {
    revision.len() == 64
        && revision
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

impl IdeWorkspaceRegistry {
    /// Save a text file selected by a human. The caller must send the exact
    /// SHA-256 revision returned with the original preview.
    ///
    /// Write/flush/sync happen to a randomized temporary sibling. Only after
    /// the base revision is rechecked do we rename it over the original. The
    /// project's root and all intermediate directory lookups are cap-scoped.
    pub fn save_source_file(
        &self,
        project_id: &str,
        relative_path: &str,
        expected_revision: &str,
        text: &str,
    ) -> Result<TextPreview, BrowserError> {
        if !valid_revision(expected_revision) {
            return Err(BrowserError::InvalidRevision);
        }
        let payload = text.as_bytes();
        if payload.len() as u64 > MAX_TEXT_BYTES {
            return Err(BrowserError::TooLarge);
        }
        if payload.contains(&0) {
            return Err(BrowserError::NotText);
        }

        // Serialize saves in this standalone process. Other programs may edit
        // the same project; protect against those edits by comparing revisions.
        let _save = self.save_lock.lock().map_err(|_| BrowserError::HostUnavailable)?;
        let mut parts = validate_relative(relative_path, false)?;
        let leaf = parts.pop().ok_or(BrowserError::InvalidPath)?;
        let base = self.source_capability(project_id)?;
        let parent = descend(base, &parts)?;

        let original = read_existing(&parent, &leaf)?;
        let base_revision = revision_for(&original);
        if base_revision != expected_revision {
            return Err(BrowserError::Conflict);
        }
        if original == payload {
            return Ok(TextPreview {
                project_id: project_id.to_owned(),
                relative_path: relative_path.to_owned(),
                text: text.to_owned(),
                byte_length: payload.len() as u64,
                revision: base_revision,
            });
        }

        let original_permissions = parent.symlink_metadata(&leaf)?.permissions();
        let temp = format!(".lu-ide-{}.tmp", fresh_project_id());
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        let result = (|| -> Result<(), BrowserError> {
            // cap-std directory handles may be O_PATH on Linux. They cannot
            // be fsynced directly (EBADF). Open a separate read-only directory
            // fd before replacement and verify it is syncable.
            #[cfg(unix)]
            let durable_dir = parent.open(".")?;
            #[cfg(unix)]
            durable_dir.sync_all()?;
            let mut file = parent.open_with(&temp, &options)?;
            file.write_all(payload)?;
            // Modify permissions through the already-open file descriptor,
            // never by resolving the temporary filename a second time.
            // Unix mode bits are preserved; complete Windows ACL fidelity
            // remains a separate release gate.
            file.set_permissions(original_permissions)?;
            file.sync_all()?;
            drop(file);

            // Revalidate after potentially lengthy disk IO. This is still
            // optimistic concurrency, not cross-process exclusive locking.
            let current = read_existing(&parent, &leaf)?;
            if revision_for(&current) != expected_revision {
                return Err(BrowserError::Conflict);
            }
            parent.rename(&temp, &parent, &leaf)?;
            #[cfg(unix)]
            durable_dir.sync_all()?;
            Ok(())
        })();
        if result.is_err() {
            // Clean up only our randomized sibling; never delete the original.
            let _ = parent.remove_file(&temp);
        }
        result?;
        Ok(TextPreview {
            project_id: project_id.to_owned(),
            relative_path: relative_path.to_owned(),
            text: text.to_owned(),
            byte_length: payload.len() as u64,
            revision: revision_for(payload),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn revision_format_is_restricted() {
        assert!(valid_revision(&"0".repeat(64)));
        assert!(!valid_revision(&"A".repeat(64)));
        assert!(!valid_revision("../../bad"));
        assert!(!valid_revision(&"f".repeat(63)));
    }
}
