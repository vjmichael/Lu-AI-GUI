//! Safe bridge to Lu's isolated Windows Win32 file-handle FFI.
//! lu-core forbids unsafe code; Win32 ABI lives in lu-win-worker-job.
#![cfg(windows)]
pub use lu_win_worker_job::file_guard::{inspect_open_file, WindowsFileIdentity};

/// Safe, handle-only security/time prepublication checks.
pub use lu_win_worker_job::metadata_guard::{
    SecurityMetadataSnapshot,snapshot_metadata,check_source_metadata,prepare_stage_metadata,
    no_write_lease
};


/** Check already-opened capability handles instead of path-only metadata. */
pub fn reject_opened_file_reparse(file:&cap_std::fs::File)->std::io::Result<()>{
    use std::{io,os::windows::fs::MetadataExt};
    let opened=file.try_clone()?.into_std();
    let m=opened.metadata()?;
    if !m.is_file() || m.file_attributes() & 0x0000_0400 != 0 {
        return Err(io::Error::new(io::ErrorKind::PermissionDenied,
            "Windows reparse point or nonregular file"))
    }
    Ok(())
}
pub fn reject_opened_directory_reparse(dir:&cap_std::fs::Dir)->std::io::Result<()>{
    use std::{io,os::windows::fs::MetadataExt};
    let opened=dir.try_clone()?.into_std_file();
    let m=opened.metadata()?;
    if !m.is_dir() || m.file_attributes() & 0x0000_0400 != 0 {
        return Err(io::Error::new(io::ErrorKind::PermissionDenied,
            "Windows reparse point or nondirectory"))
    }
    Ok(())
}
