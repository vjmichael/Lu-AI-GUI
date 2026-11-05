//! Real OS probe for IDE Task 2. No project program can be selected.
use lu_ide_linux_sandbox::{run_probe, Probe, SandboxError};
use std::{fs, process::ExitCode, time::Duration};

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
                println!(
                    "ISOLATION_PROBE_PASS: {:?} exit={}",
                    result.probe, result.exit_code
                );
                passed += 1;
            }
            Err(error) => {
                eprintln!("ISOLATION_PROBE_FAIL: {case:?}: {error:?}");
                // No direct execution fallback. Continue only to report results.
            }
        }
    }
    // A long-running fixture must be killed and reaped by the host deadline.
    // This does not by itself prove all descendant cleanup or cgroup quotas.
    match run_probe(&dir, Probe::TimeoutContainment, Duration::from_millis(250)) {
        Err(SandboxError::TimedOut) => {
            println!("ISOLATION_PROBE_PASS: TimeoutContainment");
            passed += 1;
        }
        outcome => eprintln!("ISOLATION_PROBE_FAIL: TimeoutContainment: {outcome:?}"),
    }
    let _ = fs::remove_dir_all(dir);
    let total = cases.len() + 1;
    if passed == total {
        println!("ISOLATION_PROBES: {passed}/{total} passed");
        ExitCode::SUCCESS
    } else {
        eprintln!("ISOLATION_PROBES: {passed}/{total} passed; do not enable workers");
        ExitCode::FAILURE
    }
}
