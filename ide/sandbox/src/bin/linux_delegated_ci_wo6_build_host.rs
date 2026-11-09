//! Fixed WO6 CI Cargo build/test host for real systemd delegated cgroup.
//! No arbitrary command, source, path, PID, shell or root worker accepted.
#[cfg(not(target_os = "linux"))]
fn main() {
    std::process::exit(2);
}
#[cfg(target_os = "linux")]
fn main() {
    if let Err(reason) = run() {
        eprintln!("DENIED: genuine WO6 delegated Cargo fixture: {reason}");
        std::process::exit(2);
    }
}
#[cfg(target_os = "linux")]
fn run() -> Result<(), &'static str> {
    use std::{env, fs, path::Path, process::Command};
    let mut args = env::args_os().skip(1);
    let first = args.next().ok_or("missing fixed acceptance mode")?;
    if args.next().is_some()
        || !matches!(
            first.to_str(),
            Some("--check" | "--test" | "--build" | "--project-debug" | "--cancel")
        )
    {
        return Err("only fixed check, test, build, project-debug and cancel modes");
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let helper = root.join("target/debug/linux_cgroup_prepare");
    let fixture =
        root.join("../supervisor/target/debug/examples/wo6_linux_native_build_acceptance");
    for executable in [&helper, &fixture] {
        let meta = fs::symlink_metadata(executable).map_err(|_| "fixed native binary missing")?;
        if !meta.is_file() || meta.file_type().is_symlink() {
            return Err("fixed native binary is not a regular file");
        }
    }
    let result = Command::new(&helper)
        .env_clear()
        .status()
        .map_err(|_| "delegated controller helper could not spawn")?;
    if !result.success() {
        return Err("real kernel cgroup delegation denied");
    }
    let result = Command::new(&fixture)
        .arg(first)
        .env_clear()
        .env("HOME", "/tmp")
        .env("LANG", "C")
        .status()
        .map_err(|_| "native Cargo fixed fixture spawn failed")?;
    if !result.success() {
        return Err("native contained Cargo fixture OS check denied");
    }
    println!("PASS: real delegated Linux Cargo WO6 acceptance");
    Ok(())
}
