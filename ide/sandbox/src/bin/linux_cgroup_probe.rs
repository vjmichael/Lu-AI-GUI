//! Wave D4-L1: Linux cgroup-v2 delegation/quota admission probe.
//! No child process is launched. No arbitrary paths are accepted from users.
#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("Linux cgroup-v2 probe requires Linux; no fallback");
    std::process::exit(2);
}

#[cfg(target_os = "linux")]
mod probe {
    use std::{
        fs, io,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    const ROOT: &str = "/sys/fs/cgroup";

    fn current_cgroup() -> io::Result<PathBuf> {
        let proc = fs::read_to_string("/proc/self/cgroup")?;
        let line = proc
            .lines()
            .find(|line| line.starts_with("0::"))
            .ok_or_else(|| io::Error::other("unified cgroup-v2 not mounted"))?;
        let path = line.strip_prefix("0::").unwrap();
        if !path.starts_with('/') || path.split('/').any(|part| part == "..") {
            return Err(io::Error::other("invalid current cgroup identity"));
        }
        let mount = fs::canonicalize(ROOT)?;
        let relative = path.trim_start_matches('/');
        let current = fs::canonicalize(mount.join(relative))?;
        if !current.starts_with(&mount) {
            return Err(io::Error::other("current cgroup escaped mount"));
        }
        // The trusted host must be in a separate leaf; the empty delegated
        // parent is the only place where worker subgroups may be created.
        if current.file_name().and_then(|v| v.to_str()) != Some("lu-ide-host") {
            return Err(io::Error::other("no dedicated Lu IDE host leaf"));
        }
        let parent = current
            .parent()
            .ok_or_else(|| io::Error::other("missing delegated parent"))?;
        if !matches!(
            parent.file_name().and_then(|v| v.to_str()),
            Some("lu-ide-delegated" | "lu-ide-delegated.service")
        ) || !parent.starts_with(&mount)
            || parent == mount
        {
            return Err(io::Error::other("unexpected delegated parent identity"));
        }
        Ok(parent.to_path_buf())
    }

    struct TempChild(PathBuf);
    impl Drop for TempChild {
        fn drop(&mut self) {
            // A newly created, never-populated cgroup can be removed safely.
            let _ = fs::remove_dir(&self.0);
        }
    }

    fn require_controllers(parent: &Path) -> io::Result<()> {
        if fs::read_to_string(parent.join("cgroup.type"))?.trim() != "domain" {
            return Err(io::Error::other("delegation must be a domain cgroup"));
        }
        let available = fs::read_to_string(parent.join("cgroup.controllers"))?;
        let enabled = fs::read_to_string(parent.join("cgroup.subtree_control"))?;
        for required in ["cpu", "memory", "pids"] {
            if !available.split_whitespace().any(|c| c == required)
                || !enabled.split_whitespace().any(|c| c == required)
            {
                return Err(io::Error::other(format!(
                    "delegated CPU/memory/PID controller not activated: {required}"
                )));
            }
        }
        if !fs::read_to_string(parent.join("cgroup.procs"))?
            .trim()
            .is_empty()
        {
            return Err(io::Error::other(
                "delegated parent contains internal processes",
            ));
        }
        Ok(())
    }

    pub fn run() -> io::Result<bool> {
        let parent = match current_cgroup() {
            Ok(value) => value,
            Err(error) => {
                eprintln!("DENIED: no verified cgroup-v2 mount: {error}");
                return Ok(false);
            }
        };
        if let Err(error) = require_controllers(&parent) {
            eprintln!("DENIED: missing delegated quota controllers: {error}");
            return Ok(false);
        }
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let child = parent.join(format!("lu-ide-quota-probe-{}-{id}", std::process::id()));
        if let Err(error) = fs::create_dir(&child) {
            eprintln!("DENIED: cgroup-v2 delegated child creation unavailable: {error}");
            return Ok(false);
        }
        let _guard = TempChild(child.clone());
        if !fs::read_to_string(child.join("cgroup.events"))?
            .lines()
            .any(|line| line.trim() == "populated 0")
        {
            return Err(io::Error::other("new worker cgroup unexpectedly populated"));
        }
        // Configuring these limits in an EMPTY cgroup never runs code or
        // changes the limits of the parent or any other user process.
        let settings = [
            ("cpu.max", "50000 100000"),
            ("memory.max", "67108864"),
            ("pids.max", "4"),
        ];
        for (file, value) in settings {
            if let Err(error) = fs::write(child.join(file), value) {
                eprintln!("DENIED: cannot enforce {file}: {error}");
                return Ok(false);
            }
            let observed = fs::read_to_string(child.join(file))?;
            if observed.trim() != value {
                eprintln!("DENIED: kernel failed to retain {file} setting");
                return Ok(false);
            }
        }
        for name in ["cgroup.procs", "cgroup.kill"] {
            if !child.join(name).is_file() {
                eprintln!("DENIED: required kernel file {name} missing");
                return Ok(false);
            }
        }
        println!("PASS: dedicated empty Lu IDE cgroup-v2 parent delegated CPU/memory/PID quotas to a fresh worker child");
        println!("UNVERIFIED: atomic pre-exec placement, subtree kill, bwrap linkage, filesystem/network isolation, recovery");
        Ok(true)
    }
}
#[cfg(target_os = "linux")]
fn main() {
    match probe::run() {
        Ok(true) => {}
        Ok(false) => {
            // Not an acceptance signal: the standalone probe intentionally
            // reports fail-closed denial without attempting any worker spawn.
            println!("CONTROLLED DENIAL: production worker admission remains disabled");
        }
        Err(error) => {
            eprintln!("DENIED: cgroup admission error: {error}");
            std::process::exit(1);
        }
    }
}
