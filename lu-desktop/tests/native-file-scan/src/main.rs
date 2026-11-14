// Exercise the exact desktop scanner module in a real OS child process.
// The official Tauri executable remains separately gated on packaged engines.
#[path = "../../../src-tauri/src/file_scan_worker.rs"]
mod file_scan_worker;

fn main() {
    std::process::exit(file_scan_worker::run_file_scan_worker());
}
