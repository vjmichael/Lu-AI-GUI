//! Fixed, trusted WO5 Linux CI startup host for systemd delegation.
//!
//! systemd first places this binary in DelegateSubgroup=lu-ide-host. Only
//! AFTER placement may the fixed preparation subprocess enable parent domain
//! controllers. ExecStartPre must NOT enable them while systemd still needs
//! to attach ExecStart to the unit: the kernel can reject that attach EBUSY.
//!
//! This binary runs exactly one compiled-in acceptance fixture (normal or
//! --cancel). It accepts NO project/user executable, path, shell, command,
//! token, PID, cgroup name, mount instruction or privilege escalation.
#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("DENIED: fixed delegated WO5 fixture is Linux-only");
    std::process::exit(2);
}

#[cfg(target_os = "linux")]
fn main() {
    if let Err(err) = run() {
        eprintln!("DENIED: trusted WO5 delegated Linux host: {err}");
        std::process::exit(2);
    }
}

#[cfg(target_os = "linux")]
fn run() -> Result<(), &'static str> {
    use std::{env, fs, path::Path, process::Command};
    let mut args = env::args_os().skip(1);
    let mode = args.next();
    if args.next().is_some()
        || mode
            .as_deref()
            .is_some_and(|x| x != std::ffi::OsStr::new("--cancel"))
    {
        return Err("only normal or fixed cancellation acceptance is supported");
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let prep = root.join("target/debug/linux_cgroup_prepare");
    let fixture = root.join("../supervisor/target/debug/examples/e2_native_lsp_acceptance");
    for executable in [&prep, &fixture] {
        let meta = fs::symlink_metadata(executable)
            .map_err(|_| "fixed, compiled-in native executable missing")?;
        if !meta.is_file() || meta.file_type().is_symlink() {
            return Err("fixed native executable is not a regular file");
        }
    }
    let bundle = Path::new("/opt/lu-ide-e2-test");
    if !bundle.join("bin/clangd").is_file() {
        return Err("fixed native clangd acceptance bundle missing");
    }
    // The caller cannot supply a command: the helper itself verifies its
    // exact systemd cgroup identity before modifying only that parent's
    // subtree_control. There is no chmod, sudo or root cgroup escape.
    let status = Command::new(&prep)
        .env_clear()
        .status()
        .map_err(|_| "trusted delegated controller helper could not start")?;
    if !status.success() {
        return Err("kernel delegated controller preparation denied");
    }
    // Do not inherit runner credentials or arbitrary host/project variables.
    let mut fixture_cmd = Command::new(&fixture);
    fixture_cmd
        .env_clear()
        .env("LU_IDE_E2_PINNED_LSP_BUNDLE", bundle)
        .env("LU_IDE_E2_NATIVE_TRACE", "1")
        .env("HOME", "/tmp")
        .env("LANG", "C")
        .current_dir(root.parent().ok_or("fixture root unavailable")?);
    if mode.is_some() {
        fixture_cmd.arg("--cancel");
    }
    let status = fixture_cmd
        .status()
        .map_err(|_| "fixed native LSP fixture could not start")?;
    if !status.success() {
        return Err("genuine delegated LSP native acceptance failed");
    }
    println!(
        "PASS: native Linux delegated clangd {} and durable receipt",
        if mode.is_some() { "cancellation" } else { "normal execution" }
    );
    Ok(())
}
