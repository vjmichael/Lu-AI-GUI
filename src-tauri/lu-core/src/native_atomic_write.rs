//! First-phase host-side, *create-new-only* atomic file primitive.
//!
//! This module is NOT a model tool and does not grant any action authority.
//! The trusted host must already have an authenticated, target-specific user
//! grant and must dispatch it via DurableDispatcher/EffectJournal.
//! No existing target is replaced. The host provides a private, prevalidated
//! directory and a unique, unpredictable staging ID.
//!
//! A synced staging file is published by hard-linking it to the new target;
//! hard_link refuses to overwrite existing files on supported filesystems.
//! Failure to remove the staging alias after publication is an UNCERTAIN
//! effect and must be reconciled, never automatically retried.
use sha2::{Digest, Sha256};
use cap_std::{ambient_authority, fs::{Dir, OpenOptions}};
use std::{fs, io::Write, path::Path};

pub const MAX_ATOMIC_WRITE_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AtomicWriteError {
    InvalidTarget,
    InvalidStage,
    Oversized,
    RootUnavailable,
    ExistingTarget,
    StageCollision,
    Io(String),
    PublishedNeedsReconciliation,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteReceipt {
    pub target: String,
    pub bytes_written: usize,
    pub sha256: String,
}

fn simple_filename(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && name != "." && name != ".."
        && !name.starts_with(".lu-stage-")
        && !name.chars().any(|c| matches!(c, '\\' | '/' | ':' | '\0'))
        && !Path::new(name).is_absolute()
}
fn valid_stage_id(id: &str) -> bool {
    (16..=128).contains(&id.len())
        && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Create an entirely NEW target; never overwrite an existing path.
///
/// The caller guarantees that `trusted_directory` is a host-selected
/// directory and not substituted by model text, retrieved content or the
/// WebView. An adversarial parent-directory rename race requires a later
/// handle-relative publish implementation before enabling arbitrary writable
/// user directories.
pub fn create_new_atomic(
    trusted_directory: &Path,
    target_name: &str,
    host_stage_id: &str,
    content: &[u8],
) -> Result<WriteReceipt, AtomicWriteError> {
    if !simple_filename(target_name) { return Err(AtomicWriteError::InvalidTarget); }
    if !valid_stage_id(host_stage_id) { return Err(AtomicWriteError::InvalidStage); }
    if content.len() > MAX_ATOMIC_WRITE_BYTES { return Err(AtomicWriteError::Oversized); }
    let metadata = fs::symlink_metadata(trusted_directory)
        .map_err(|_| AtomicWriteError::RootUnavailable)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(AtomicWriteError::RootUnavailable);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err(AtomicWriteError::RootUnavailable);
        }
    }
    // Convert the trusted selected path into an OS directory handle.
    // The underlying directory remains the same object even if an external
    // actor subsequently renames its pathname.
    let root = Dir::open_ambient_dir(trusted_directory, ambient_authority())
        .map_err(|_| AtomicWriteError::RootUnavailable)?;
    create_new_atomic_in_dir(&root, target_name, host_stage_id, content)
}

/// Handle-relative publication: staging, sync, linking and cleanup are all
/// relative to the SAME opened directory. This is not a general Lu sandbox;
/// it is an exact-target host capability for one approved action.
pub fn create_new_atomic_in_dir(
    root: &Dir,
    target_name: &str,
    host_stage_id: &str,
    content: &[u8],
) -> Result<WriteReceipt, AtomicWriteError> {
    if !simple_filename(target_name) { return Err(AtomicWriteError::InvalidTarget); }
    if !valid_stage_id(host_stage_id) { return Err(AtomicWriteError::InvalidStage); }
    if content.len() > MAX_ATOMIC_WRITE_BYTES { return Err(AtomicWriteError::Oversized); }
    let stage_name = format!(".lu-stage-{host_stage_id}.tmp");
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    let mut handle = root.open_with(&stage_name, &options)
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                AtomicWriteError::StageCollision
            } else { AtomicWriteError::Io(e.to_string()) }
        })?;
    let write_result = (|| -> Result<(), AtomicWriteError> {
        handle.write_all(content).map_err(|e| AtomicWriteError::Io(e.to_string()))?;
        handle.sync_all().map_err(|e| AtomicWriteError::Io(e.to_string()))?;
        Ok(())
    })();
    drop(handle);
    if let Err(e) = write_result {
        let _ = root.remove_file(&stage_name);
        return Err(e);
    }
    if let Err(e) = root.hard_link(&stage_name, root, target_name) {
        let _ = fs::remove_file(&stage);
        if e.kind() == std::io::ErrorKind::AlreadyExists {
            return Err(AtomicWriteError::ExistingTarget);
        }
        return Err(AtomicWriteError::Io(e.to_string()));
    }
    // Publish happened. If cleanup fails the file may be visible along two
    // names; require a durable UNCERTAIN receipt and human reconciliation.
    fs::remove_file(&stage)
        .map_err(|_| AtomicWriteError::PublishedNeedsReconciliation)?;
    let sha256 = format!("{:x}", Sha256::digest(content));
    Ok(WriteReceipt {
        target: target_name.to_owned(),
        bytes_written: content.len(),
        sha256,
    })
}
