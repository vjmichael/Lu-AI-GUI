//! Independent P03 CI execution of the original WO4 native kernel witness.
//! A fixed systemd unit places THIS native program into its delegated host
//! leaf before the compiled helper enables controllers. Only the fixed first-
//! party witness binary can execute; no project, path, PID or command input.
#[cfg(not(target_os = "linux"))]
fn main() {
    std::process::exit(2);
}

#[cfg(target_os = "linux")]
fn main() {
    if let Err(reason) = run() {
        eprintln!("DENIED: independent WO4 native delegated kernel witness: {reason}");
        std::process::exit(2);
    }
}

#[cfg(target_os = "linux")]
fn run() -> Result<(), &'static str> {
    use std::{env, fs, path::Path, process::Command};
    if env::args_os().len() != 1 {
        return Err("fixed witness accepts no command arguments");
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let helper = root.join("target/debug/linux_cgroup_prepare");
    let witness = root.join("target/debug/linux_cgroup_native_witness");
    for fixed in [&helper, &witness] {
        let meta = fs::symlink_metadata(fixed)
            .map_err(|_| "fixed acceptance executable missing")?;
        if !meta.is_file() || meta.file_type().is_symlink() {
            return Err("fixed acceptance executable changed identity");
        }
    }
    if !Command::new(&helper)
        .env_clear()
        .status()
        .map_err(|_| "trusted delegated cgroup controller helper could not start")?
        .success()
    {
        return Err("kernel refused exact controller delegation");
    }
    if !Command::new(&witness)
        .env_clear()
        .status()
        .map_err(|_| "native child/grandchild witness could not start")?
        .success()
    {
        return Err("real kernel child/grandchild membership or cleanup denied");
    }
    println!("PASS: independent kernel child and grandchild cgroup-v2 admission, quotas, kill and populated 0");
    Ok(())
}
