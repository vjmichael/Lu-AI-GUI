//! Safe bridge to Lu's isolated Windows Win32 file-handle FFI.
//! lu-core forbids unsafe code; Win32 ABI lives in lu-win-worker-job.
#![cfg(windows)]
pub use lu_win_worker_job::file_guard::{inspect_open_file, WindowsFileIdentity};

/// Safe, handle-only security/time prepublication checks.
pub use lu_win_worker_job::metadata_guard::{
    SecurityMetadataSnapshot,snapshot_metadata,check_source_metadata,prepare_stage_metadata
};
