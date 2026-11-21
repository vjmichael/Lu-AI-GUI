//! FIN-04W2 CI-only SCM entrypoint. Never a desktop debugger runner.
#[cfg(all(windows, debug_assertions, feature = "fin04w2-scm"))]
fn main() {
    if std::env::args_os().nth(1).is_some() {
        eprintln!("DENIED: debugger broker accepts no user commands");
        std::process::exit(2);
    }
    if lu_ide_execution_supervisor::windows_fin04w2_service::dispatch().is_err() {
        eprintln!("DENIED: debugger broker requires SCM");
        std::process::exit(2);
    }
}
#[cfg(not(all(windows, debug_assertions, feature = "fin04w2-scm")))]
fn main() {
    eprintln!("DENIED: FIN-04W2 debugger service unavailable");
    std::process::exit(2);
}
