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
    let quota_probe = root.join("target/debug/linux_cgroup_probe");
    for fixed in [&helper, &witness, &quota_probe] {
        let meta =
            fs::symlink_metadata(fixed).map_err(|_| "fixed acceptance executable missing")?;
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
    // The standalone D4 probe deliberately exits zero on controlled denial.
    // Require its exact positive observation, not just a successful status.
    let quota = Command::new(&quota_probe)
        .env_clear()
        .output()
        .map_err(|_| "original D4 quota probe could not start")?;
    const POSITIVE: &str = "PASS: dedicated empty Lu IDE cgroup-v2 parent delegated CPU/memory/PID quotas to a fresh worker child";
    if !quota.status.success()
        || !quota.stderr.is_empty()
        || !std::str::from_utf8(&quota.stdout).is_ok_and(|s| {
            s.lines().filter(|line| *line == POSITIVE).count() == 1
                && s.lines()
                    .all(|line| line == POSITIVE || line.starts_with("UNVERIFIED: "))
        })
    {
        return Err("original standalone quota readback not positively verified");
    }
    println!("PASS: original D4 standalone exact quota readback and empty child cleanup");
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
