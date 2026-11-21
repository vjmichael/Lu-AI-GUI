//! Service Control Manager entrypoint only. Running as a desktop child is denied.
#[cfg(all(windows, feature = "protected-recovery-service"))]
fn main() {
    if !std::env::args_os().skip(1).collect::<Vec<_>>().is_empty() {
        eprintln!("DENIED: service accepts no command-line or desktop instructions");
        std::process::exit(2);
    }
    if let Err(reason) = lu_ide_execution_supervisor::windows_recovery_service::dispatch() {
        eprintln!("DENIED: service bootstrap: {}", reason.code());
        std::process::exit(1);
    }
}
#[cfg(not(all(windows, feature = "protected-recovery-service")))]
fn main() {
    eprintln!("DENIED: native Windows service unavailable");
    std::process::exit(2);
}
