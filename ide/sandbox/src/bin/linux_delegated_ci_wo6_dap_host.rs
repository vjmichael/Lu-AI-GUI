//! Trusted WO6 CI systemd ExecStart host (not a project/desktop subprocess
//! interface). The exact systemd unit first places this host in lu-ide-host.
//! Then it enables the delegated parent controllers with an existing fixed
//! helper and runs ONLY the compiled native LLDB-DAP acceptance fixture.
//! No arbitrary command, project data, PID, path or shell is accepted.
#[cfg(not(target_os = "linux"))]
fn main() {
    std::process::exit(2);
}
#[cfg(target_os = "linux")]
fn main() {
    if let Err(reason) = run() {
        eprintln!("DENIED: fixed WO6 delegated DAP acceptance: {reason}");
        std::process::exit(2);
    }
}
#[cfg(target_os = "linux")]
fn run() -> Result<(), &'static str> {
    use std::{env, fs, path::Path, process::Command};
    let mut args = env::args_os().skip(1);
    let mode = args.next();
    if args.next().is_some()
        || mode.as_deref().is_some_and(|arg| {
            arg != std::ffi::OsStr::new("--cancel") && arg != std::ffi::OsStr::new("--crash-verify")
        })
    {
        return Err("only fixed normal/cancel/crash native acceptance modes supported");
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let helper = root.join("target/debug/linux_cgroup_prepare");
    let fixture = root.join("../supervisor/target/debug/examples/wo6_native_dap_acceptance");
    let bundle = Path::new("/opt/lu-ide-wo6-dap-test");
    for binary in [
        &helper,
        &fixture,
        &bundle.join("bin/lldb-dap"),
        &bundle.join("debuggee/wo6-selftest"),
    ] {
        let meta = fs::symlink_metadata(binary).map_err(|_| "fixed executable missing")?;
        if !meta.is_file() || meta.file_type().is_symlink() {
            return Err("fixed executable not a regular file");
        }
    }
    let helper_status = Command::new(&helper)
        .env_clear()
        .status()
        .map_err(|_| "delegated cgroup controller helper failed to spawn")?;
    if !helper_status.success() {
        return Err("genuine kernel delegation helper rejected unit");
    }
    let mut command = Command::new(&fixture);
    command
        .env_clear()
        .env("LU_IDE_WO6_DAP_BUNDLE", bundle)
        .env("HOME", "/tmp")
        .env("LANG", "C")
        .env("LU_IDE_E2_NATIVE_TRACE", "1");
    if mode.as_deref() == Some(std::ffi::OsStr::new("--crash-verify")) {
        // Two independent OS processes: first exits without running Drop,
        // then a fresh verifier reopens durable receipts and reads the REAL
        // kernel cgroup/events. No project-supplied mode, path or debugger.
        use std::os::unix::fs::PermissionsExt;
        let unique = format!(
            "lu-ide-p03-crash-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|_| "invalid native witness clock")?
                .as_nanos()
        );
        let witness = env::temp_dir().join(unique);
        fs::create_dir(&witness).map_err(|_| "trusted crash witness root cannot be created")?;
        fs::set_permissions(&witness, fs::Permissions::from_mode(0o700))
            .map_err(|_| "trusted crash witness root mode failed")?;
        let status = command
            .env("LU_IDE_P03_CRASH_WITNESS_ROOT", &witness)
            .arg("--crash-after-initialize")
            .status()
            .map_err(|_| "fixed crash fixture spawn failed")?;
        if status.code() != Some(73) {
            return Err("native debugger did not reach crash-after-initialize witness");
        }
        let resumed = Command::new(&fixture)
            .env_clear()
            .env("LU_IDE_P03_CRASH_WITNESS_ROOT", &witness)
            .arg("--verify-crash-replay")
            .status()
            .map_err(|_| "independent crash replay verifier failed to spawn")?;
        if !resumed.success() {
            return Err("kernel cgroup or durable replay did not quarantine host crash");
        }
        println!("PASS: actual native debugger supervisor crash/restart evidence");
        return Ok(());
    }
    if mode.is_some() {
        command.arg("--cancel");
    }
    let outcome = command
        .status()
        .map_err(|_| "fixed DAP fixture spawn failed")?;
    if !outcome.success() {
        return Err("real native DAP OS acceptance failed");
    }
    println!(
        "PASS: genuine delegated LLDB-DAP {} acceptance",
        if mode.is_some() {
            "cancellation"
        } else {
            "normal"
        }
    );
    Ok(())
}
