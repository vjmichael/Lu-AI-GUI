//! Real OS probe for IDE Task 2. No project program can be selected.
use std::{fs, process::ExitCode, time::Duration};
use lu_ide_linux_sandbox::{run_probe, Probe};

fn main() -> ExitCode {
    let dir = std::env::temp_dir().join(format!("lu-ide-probe-{}", std::process::id()));
    if let Err(e) = fs::create_dir_all(&dir) {
        eprintln!("ISOLATION_UNAVAILABLE: cannot prepare fixture: {e}");
        return ExitCode::FAILURE;
    }
    let cases = [
        Probe::NamespaceSetup,
        Probe::SecretIsolation,
        Probe::ReadOnlyWorkspace,
        Probe::NetworkIsolation,
    ];
    let mut passed = 0;
    for case in cases {
        match run_probe(&dir, case, Duration::from_secs(5)) {
            Ok(result) => {
                println!("ISOLATION_PROBE_PASS: {:?} exit={}", result.probe, result.exit_code);
                passed += 1;
            }
            Err(error) => {
                eprintln!("ISOLATION_PROBE_FAIL: {case:?}: {error:?}");
                // No direct execution fallback. Continue only to report results.
            }
        }
    }
    let _ = fs::remove_dir_all(dir);
    if passed == cases.len() {
        println!("ISOLATION_PROBES: {passed}/{} passed", cases.len());
        ExitCode::SUCCESS
    } else {
        eprintln!("ISOLATION_PROBES: {passed}/{} passed; do not enable workers", cases.len());
        ExitCode::FAILURE
    }
}
