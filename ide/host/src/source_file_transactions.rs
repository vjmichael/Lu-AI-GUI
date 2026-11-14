//! DEV-01.3: project-capability file move and recoverable trash.
//! No std::fs path-rename: cap-std rename can silently REPLACE an existing
//! destination. A hard-link is exclusive, followed by a verified unlink.
//! These are file-only operations, always on the same registered project
//! filesystem. On a crash during transfer BOTH hard links may remain; the
//! operation never deletes the only surviving source of bytes.
use std::io::{self, Read, Write};

use cap_std::fs::{Dir, OpenOptions};
use serde::{Deserialize, Serialize};

use crate::{
    browser::{descend, revision_for, validate_relative, BrowserError, MAX_TEXT_BYTES},
    fresh_project_id, source_create::portable_components, IdeWorkspaceRegistry,
};

const TRASH: &str = ".lu-ide-trash";
const MAX_TRASH_ITEMS: usize = 128;
const MAX_RECORD_BYTES: u64 = 4096;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct TrashRecord {
    version: u32,
    token: String,
    original_relative_path: String,
    revision: String,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileOperationReceipt {
    pub project_id: String,
    pub original_relative_path: String,
    pub current_relative_path: Option<String>,
    pub revision: String,
    pub trash_token: Option<String>,
    pub operation: &'static str,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrashedFile {
    pub token: String,
    pub original_relative_path: String,
    pub revision: String,
    pub payload_present: bool,
    pub original_present: bool,
}

fn checked_path(path: &str) -> Result<(Vec<String>, String), BrowserError> {
    let mut parts = validate_relative(path, false)?;
    portable_components(&parts)?;
    let name = parts.pop().ok_or(BrowserError::InvalidPath)?;
    Ok((parts, name))
}
fn checked_revision(revision: &str) -> bool {
    revision.len() == 64 && revision.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
fn token_valid(token: &str) -> bool {
    token.len() == 36 && token.starts_with("ide-") &&
        token[4..].bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
fn read_verified(parent: &Dir, name: &str, expected: &str) -> Result<(), BrowserError> {
    if !checked_revision(expected) {
        return Err(BrowserError::InvalidRevision);
    }
    let meta = parent.symlink_metadata(name)?;
    if !meta.is_file() || meta.file_type().is_symlink() {
        return Err(BrowserError::NotFile);
    }
    if meta.len() > MAX_TEXT_BYTES { return Err(BrowserError::TooLarge); }
    let mut bytes = Vec::with_capacity(meta.len() as usize);
    parent.open(name)?.take(MAX_TEXT_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_TEXT_BYTES { return Err(BrowserError::TooLarge); }
    if revision_for(&bytes) != expected { return Err(BrowserError::Conflict); }
    Ok(())
}
fn reject_case_alias(dest: &Dir, name: &str) -> Result<(), BrowserError> {
    let folded = name.to_lowercase();
    for (count, item) in dest.entries()?.enumerate() {
        if count >= 4096 { return Err(BrowserError::TooLarge); }
        if item?.file_name().to_string_lossy().to_lowercase() == folded {
            return Err(BrowserError::AlreadyExists);
        }
    }
    Ok(())
}
fn sync_dir(dir: &Dir) -> Result<(), BrowserError> {
    // cap-std may use O_PATH fd on Linux; open a syncable read-only ".".
    #[cfg(unix)] { dir.open(".")?.sync_all()?; }
    #[cfg(not(unix))] { let _ = dir; }
    Ok(())
}
fn exclusive_link_then_unlink(
    source: &Dir, source_name: &str,
    dest: &Dir, dest_name: &str, expected: &str,
) -> Result<(), BrowserError> {
    read_verified(source, source_name, expected)?;
    reject_case_alias(dest, dest_name)?;
    // hard_link fails if the destination already exists, even under a race.
    source.hard_link(source_name, dest, dest_name)
        .map_err(|error| if error.kind() == io::ErrorKind::AlreadyExists {
            BrowserError::AlreadyExists
        } else { BrowserError::Io(error) })?;
    sync_dir(dest)?;
    // Reject modified content or changed source names before unlinking. The
    // newly-linked name is left in place if later source deletion fails,
    // so loss of the sole surviving copy is not a recovery mode.
    if let Err(problem) = read_verified(source, source_name, expected)
        .and_then(|_| read_verified(dest, dest_name, expected)) {
        return Err(problem);
    }
    source.remove_file(source_name)?;
    sync_dir(source)?;
    Ok(())
}
fn open_trash(base: &Dir, create: bool) -> Result<Option<Dir>, BrowserError> {
    match base.symlink_metadata(TRASH) {
        Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() =>
            Ok(Some(base.open_dir(TRASH)?)),
        Ok(_) => Err(BrowserError::NotDirectory),
        Err(error) if error.kind() == io::ErrorKind::NotFound && !create => Ok(None),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            match base.create_dir(TRASH) {
                Ok(()) => (),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => (),
                Err(e) => return Err(BrowserError::Io(e)),
            };
            let meta = base.symlink_metadata(TRASH)?;
            if !meta.is_dir() || meta.file_type().is_symlink() {
                return Err(BrowserError::NotDirectory);
            }
            sync_dir(base)?;
            Ok(Some(base.open_dir(TRASH)?))
        },
        Err(error) => Err(BrowserError::Io(error)),
    }
}
fn record_name(token: &str) -> String { format!("{token}.json") }
fn payload_name(token: &str) -> String { format!("{token}.payload") }
fn read_record(trash: &Dir, token: &str) -> Result<TrashRecord, BrowserError> {
    if !token_valid(token) { return Err(BrowserError::InvalidPath); }
    let name = record_name(token);
    let meta = trash.symlink_metadata(&name)?;
    if !meta.is_file() || meta.file_type().is_symlink() ||
        meta.len() > MAX_RECORD_BYTES { return Err(BrowserError::InvalidPath); }
    let mut raw = Vec::new();
    trash.open(&name)?.take(MAX_RECORD_BYTES + 1).read_to_end(&mut raw)?;
    if raw.len() as u64 > MAX_RECORD_BYTES { return Err(BrowserError::InvalidPath); }
    let record: TrashRecord = serde_json::from_slice(&raw)
        .map_err(|_| BrowserError::InvalidPath)?;
    if record.version != 1 || record.token != token ||
        !checked_revision(&record.revision) {
        return Err(BrowserError::InvalidPath);
    }
    checked_path(&record.original_relative_path)?;
    Ok(record)
}
impl IdeWorkspaceRegistry {
    /// Human-only file rename or move. Destination directory must already
    /// exist, be inside the same picker-registered project, and cannot clobber.
    /// Directories are deliberately NOT supported by the hard-link transport.
    pub fn move_source_file(
        &self, project_id: &str, source_path: &str,
        destination_path: &str, expected_revision: &str,
    ) -> Result<FileOperationReceipt, BrowserError> {
        let (from_parts, from_name) = checked_path(source_path)?;
        let (to_parts, to_name) = checked_path(destination_path)?;
        if source_path == destination_path { return Err(BrowserError::AlreadyExists); }
        let _guard = self.save_lock.lock().map_err(|_| BrowserError::HostUnavailable)?;
        let base = self.source_capability(project_id)?;
        let source_dir = descend(base, &from_parts)?;
        let target_dir = descend(self.source_capability(project_id)?, &to_parts)?;
        exclusive_link_then_unlink(&source_dir, &from_name, &target_dir, &to_name, expected_revision)?;
        Ok(FileOperationReceipt {
            project_id: project_id.to_owned(),
            original_relative_path: source_path.to_owned(),
            current_relative_path: Some(destination_path.to_owned()),
            revision: expected_revision.to_owned(),
            trash_token: None, operation: "move",
        })
    }
    /// Append a persisted native trash record BEFORE moving the file. An
    /// interrupted unlink can leave both links but never intentionally loses
    /// the payload. A corrupt or inconsistent receipt is denied, not ignored.
    pub fn trash_source_file(
        &self, project_id: &str, source_path: &str, expected_revision: &str,
    ) -> Result<FileOperationReceipt, BrowserError> {
        let (parts, source_name) = checked_path(source_path)?;
        let _guard = self.save_lock.lock().map_err(|_| BrowserError::HostUnavailable)?;
        let base = self.source_capability(project_id)?;
        let source_dir = descend(self.source_capability(project_id)?, &parts)?;
        read_verified(&source_dir, &source_name, expected_revision)?;
        let trash = open_trash(&base, true)?.ok_or(BrowserError::HostUnavailable)?;
        let token = fresh_project_id();
        let record = TrashRecord {
            version: 1, token: token.clone(),
            original_relative_path: source_path.to_owned(),
            revision: expected_revision.to_owned(),
        };
        let bytes = serde_json::to_vec(&record).map_err(|_| BrowserError::InvalidPath)?;
        if bytes.len() as u64 > MAX_RECORD_BYTES { return Err(BrowserError::TooLarge); }
        let name = record_name(&token);
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        let mut file = trash.open_with(&name, &options)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        sync_dir(&trash)?;
        // If transfer fails the JSON record is retained for explicit human
        // recovery/inspection, never silently erased or counted as success.
        exclusive_link_then_unlink(
            &source_dir, &source_name, &trash, &payload_name(&token), expected_revision,
        )?;
        Ok(FileOperationReceipt {
            project_id: project_id.to_owned(),
            original_relative_path: source_path.to_owned(),
            current_relative_path: None,
            revision: expected_revision.to_owned(),
            trash_token: Some(token), operation: "trash",
        })
    }
    pub fn list_trashed_source_files(
        &self, project_id: &str,
    ) -> Result<Vec<TrashedFile>, BrowserError> {
        let base = self.source_capability(project_id)?;
        let Some(trash) = open_trash(&base, false)? else { return Ok(Vec::new()) };
        let mut found = Vec::new();
        let mut visited = 0;
        for entry in trash.entries()? {
            visited += 1;
            if visited > MAX_TRASH_ITEMS * 2 + 8 { return Err(BrowserError::TooLarge); }
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let Some(token) = name.strip_suffix(".json") else { continue };
            if !token_valid(token) { return Err(BrowserError::InvalidPath); }
            let record = read_record(&trash, token)?;
            let payload_present = trash.symlink_metadata(payload_name(token))
                .map(|m| m.is_file() && !m.file_type().is_symlink()).unwrap_or(false);
            let (parent_parts, source_name) = checked_path(&record.original_relative_path)?;
            let original_present = descend(self.source_capability(project_id)?, &parent_parts)
                .and_then(|parent| parent.symlink_metadata(&source_name).map_err(BrowserError::Io))
                .is_ok();
            found.push(TrashedFile {
                token: token.to_owned(), original_relative_path: record.original_relative_path,
                revision: record.revision, payload_present, original_present,
            });
            if found.len() > MAX_TRASH_ITEMS { return Err(BrowserError::TooLarge); }
        }
        found.sort_by(|a, b| a.original_relative_path.cmp(&b.original_relative_path));
        Ok(found)
    }
    /// Restore only to the original path, through the original project cap.
    /// No overwrite: any existing path (even same bytes) remains untouched.
    /// On an interrupted restore a second link may remain for manual review.
    pub fn restore_source_file(
        &self, project_id: &str, token: &str,
    ) -> Result<FileOperationReceipt, BrowserError> {
        if !token_valid(token) { return Err(BrowserError::InvalidPath); }
        let _guard = self.save_lock.lock().map_err(|_| BrowserError::HostUnavailable)?;
        let base = self.source_capability(project_id)?;
        let trash = open_trash(&base, false)?.ok_or(BrowserError::NotFile)?;
        let record = read_record(&trash, token)?;
        let (parts, name) = checked_path(&record.original_relative_path)?;
        let original_dir = descend(self.source_capability(project_id)?, &parts)?;
        exclusive_link_then_unlink(
            &trash, &payload_name(token), &original_dir, &name, &record.revision,
        )?;
        // Source now has an actual content-verified native inode. Removing the
        // manifest last is safe; a crash before this step leaves a harmless
        // record with no payload, explicitly reported by the listing.
        trash.remove_file(record_name(token))?;
        sync_dir(&trash)?;
        Ok(FileOperationReceipt {
            project_id: project_id.to_owned(),
            original_relative_path: record.original_relative_path.clone(),
            current_relative_path: Some(record.original_relative_path),
            revision: record.revision,
            trash_token: Some(token.to_owned()), operation: "restore",
        })
    }
}
