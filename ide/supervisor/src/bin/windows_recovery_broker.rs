//! Installed, separately supervised Windows recovery-keeper process.
//! No desktop IPC, project commands, scripts, shell, or model requests.
#[cfg(all(windows, feature = "windows-worker"))]
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(reason) = lu_ide_execution_supervisor::windows_broker::guard_process(&args) {
        eprintln!("DENIED: Windows recovery broker: {}", reason.code());
        std::process::exit(1);
    }
}

#[cfg(not(all(windows, feature = "windows-worker")))]
fn main() {
    eprintln!("DENIED: Windows native recovery worker feature unavailable");
    std::process::exit(2);
}
