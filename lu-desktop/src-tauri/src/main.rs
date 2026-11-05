#![cfg_attr(windows, windows_subsystem = "windows")]
fn main() {
    // Trusted internal worker mode starts before Tauri, GGUF, or model state.
    if std::env::args_os().nth(1).as_deref()
        == Some(std::ffi::OsStr::new("--lu-file-scan-worker"))
    {
        std::process::exit(lu_desktop::run_file_scan_worker());
    }
    lu_desktop::run();
}
