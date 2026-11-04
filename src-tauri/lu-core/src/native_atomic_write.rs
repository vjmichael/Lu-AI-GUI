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
use std::{fs, io::Write, path::{Path, PathBuf}};

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
    let root = trusted_directory.canonicalize()
        .map_err(|_| AtomicWriteError::RootUnavailable)?;
    let target = root.join(target_name);
    let stage_name = format!(".lu-stage-{host_stage_id}.tmp");
    let stage = root.join(stage_name);
    let mut handle = fs::OpenOptions::new()
        .create_new(true).write(true).open(&stage)
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
        let _ = fs::remove_file(&stage);
        return Err(e);
    }
    if let Err(e) = fs::hard_link(&stage, &target) {
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
