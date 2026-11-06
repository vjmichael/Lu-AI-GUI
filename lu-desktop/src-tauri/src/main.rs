#![cfg_attr(windows, windows_subsystem = "windows")]
fn main() {
    // Trusted internal worker mode starts before Tauri, GGUF, or model state.
    if std::env::args_os().nth(1).as_deref()
        == Some(std::ffi::OsStr::new("--lu-file-scan-worker"))
    {
        std::process::exit(lu_desktop::run_file_scan_worker());
    }
    if matches!(std::env::args_os().nth(1).as_deref(),
        Some(s) if s==std::ffi::OsStr::new("--lu-archive-extract-worker")||
            s==std::ffi::OsStr::new("--lu-archive-preview-worker"))
    {
        std::process::exit(lu_desktop::run_archive_process_worker());
    }
    lu_desktop::run();
}
