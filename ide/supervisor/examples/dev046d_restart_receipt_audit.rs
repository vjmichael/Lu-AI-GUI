//! DEV-04.6D developer-only read-only Windows restart receipt audit.
//! Cannot run LLDB, send DAP or reconstruct any previously consumed consent.
#[cfg(all(windows, debug_assertions, feature = "dev046d-native-dialog"))]
fn main() {
    use lu_ide_execution_supervisor::Supervisor;
    let mut args = std::env::args_os().skip(1);
    let Some(journal) = args.next() else {
        eprintln!("DENIED: require one trusted application-profile journal path");
        std::process::exit(2);
    };
    if args.next().is_some() {
        eprintln!("DENIED: unexpected arguments");
        std::process::exit(2);
    }
    let path = std::path::PathBuf::from(journal);
    if !path.is_absolute() || path.file_name()
        != Some(std::ffi::OsStr::new("debug-fixture-receipts.jsonl"))
    {
        eprintln!("DENIED: unexpected journal filename");
        std::process::exit(2);
    }
    let supervisor = Supervisor::open(path).unwrap_or_else(|_| {
        eprintln!("DENIED: native supervisor durable journal invalid");
        std::process::exit(3);
    });
    let waiting = supervisor.native_debug_action_cleanup_jobs_after_restart()
        .unwrap_or_else(|_| {
            eprintln!("DENIED: original operator action journals corrupt or foreign");
            std::process::exit(4);
        });
    if !waiting.is_empty() {
        eprintln!("DENIED: one or more stopped Jobs still require independent OS-only cleanup; no grant restored");
        std::process::exit(5);
    }
    println!("DEV046D_NATIVE_RESTART_AUDIT=PASS;no_orphan_job_actions=true;authority_restored=false");
}
#[cfg(not(all(windows, debug_assertions, feature = "dev046d-native-dialog")))]
fn main() {
    eprintln!("DENIED: D6D audit requires explicit Windows developer-only feature");
    std::process::exit(2);
}
