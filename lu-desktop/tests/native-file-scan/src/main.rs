// Link real platform guard code, and compile the SAME shared Linux opened
// identity module as production; no mock identity oracle or Tauri runtime.
extern crate self as lu_core;
#[cfg(windows)]
pub mod windows_file_guard{
    pub use lu_win_worker_job::file_guard::inspect_open_file;
}
#[cfg(target_os="linux")]
#[path = "../../../../src-tauri/lu-core/src/reviewed_scan_identity.rs"]
pub mod native_background_scheduler;

// Exercise the exact desktop scanner module in a real OS child process.
// The official Tauri executable remains separately gated on packaged engines.
#[path = "../../../src-tauri/src/file_scan_worker.rs"]
mod file_scan_worker;

fn main() {
    std::process::exit(file_scan_worker::run_file_scan_worker());
}
