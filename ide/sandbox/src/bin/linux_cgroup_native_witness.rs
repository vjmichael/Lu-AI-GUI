//! WO4 independent cgroup-v2 native execution witness.
//!
//! This acceptance-only program executes its own two fixed sleep modes.
//! It does not accept project paths, scripts, executable names, target PIDs,
//! user-supplied cgroup paths, or privileged setup options.
//! A missing systemd-delegated Linux cgroup fails closed (never "simulates"
//! a successful worker admission).
#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("DENIED: WO4 kernel cgroup witness requires native Linux");
    std::process::exit(2);
}

#[cfg(target_os = "linux")]
mod native {
    use std::{
        env, fs,
        fs::OpenOptions,
        io::{self, BufRead, BufReader, Write},
        os::{fd::AsRawFd, raw::c_void, unix::process::CommandExt},
        path::{Path, PathBuf},
        process::{Command, Stdio},
        sync::mpsc,
        thread,
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };

    unsafe extern "C" {
        fn write(fd: i32, buf: *const c_void, count: usize) -> isize;
    }

    fn denied(reason: &'static str) -> io::Error {
        io::Error::new(io::ErrorKind::PermissionDenied, reason)
    }
    fn exact_controllers(contents: &str) -> bool {
        ["cpu", "memory", "pids"]
            .iter()
            .all(|required| contents.split_whitespace().any(|item| item == *required))
    }
    fn populated(contents: &str) -> io::Result<bool> {
        let mut seen = None;
        for line in contents.lines() {
            if let Some(value) = line.strip_prefix("populated ") {
                if seen.is_some() || (value != "0" && value != "1") {
                    return Err(denied("duplicate or malformed populated evidence"));
                }
                seen = Some(value == "1");
            }
        }
        seen.ok_or_else(|| denied("kernel populated evidence missing"))
    }
    fn delegated_parent() -> io::Result<PathBuf> {
        let data = fs::read_to_string("/proc/self/cgroup")?;
        let mut unified = data.lines().filter_map(|line| line.strip_prefix("0::"));
        let relative = unified
            .next()
            .ok_or_else(|| denied("no cgroup v2 identity"))?;
        if unified.next().is_some()
            || !relative.starts_with('/')
            || relative
                .split('/')
                .any(|component| component == "." || component == "..")
        {
            return Err(denied("invalid cgroup v2 identity"));
        }
        let root = fs::canonicalize("/sys/fs/cgroup")?;
        let host = fs::canonicalize(root.join(relative.trim_start_matches('/')))?;
        let parent = host.parent().ok_or_else(|| denied("no delegated parent"))?;
        if host.file_name().and_then(|v| v.to_str()) != Some("lu-ide-host")
            || !matches!(
                parent.file_name().and_then(|v| v.to_str()),
                Some("lu-ide-delegated" | "lu-ide-delegated.service")
            )
            || parent == root
            || !parent.starts_with(&root)
        {
            return Err(denied("no exact Lu IDE delegated host-leaf identity"));
        }
        if !exact_controllers(&fs::read_to_string(parent.join("cgroup.controllers"))?)
            || !exact_controllers(&fs::read_to_string(parent.join("cgroup.subtree_control"))?)
            || !fs::read_to_string(parent.join("cgroup.procs"))?
                .trim()
                .is_empty()
            || fs::read_to_string(parent.join("cgroup.type"))?.trim() != "domain"
        {
            return Err(denied(
                "cgroup controllers not delegated to empty domain parent",
            ));
        }
        Ok(parent.to_path_buf())
    }

    struct WitnessGroup(PathBuf);
    impl WitnessGroup {
        fn empty(&self) -> io::Result<bool> {
            Ok(!populated(&fs::read_to_string(
                self.0.join("cgroup.events"),
            )?)?)
        }
        fn kill(&self) -> io::Result<()> {
            fs::write(self.0.join("cgroup.kill"), "1")
        }
        fn await_empty(&self) -> io::Result<bool> {
            let start = Instant::now();
            while start.elapsed() < Duration::from_secs(8) {
                if self.empty()? {
                    return Ok(true);
                }
                thread::sleep(Duration::from_millis(40));
            }
            self.empty()
        }
    }
    impl Drop for WitnessGroup {
        fn drop(&mut self) {
            // No false success on failed cleanup. The original kernel
            // cgroup remains as independently inspectable evidence.
            if !self.empty().unwrap_or(false) {
                let _ = self.kill();
                let _ = self.await_empty();
            }
            if self.empty().unwrap_or(false) {
                let _ = fs::remove_dir(&self.0);
            }
        }
    }

    fn create_group(parent: &Path) -> io::Result<WitnessGroup> {
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| denied("invalid monotonic wall-clock epoch"))?
            .as_nanos();
        let group =
            WitnessGroup(parent.join(format!("lu-ide-witness-{}-{id}", std::process::id())));
        fs::create_dir(&group.0)?;
        if !group.empty()? {
            return Err(denied("new kernel cgroup is already populated"));
        }
        let quotas = [
            ("cpu.max", "50000 100000"),
            ("memory.max", "134217728"),
            ("pids.max", "8"),
        ];
        for (name, required) in quotas {
            fs::write(group.0.join(name), required)?;
            if fs::read_to_string(group.0.join(name))?.trim() != required {
                return Err(denied("kernel did not retain exact worker quota"));
            }
        }
        if !group.0.join("cgroup.kill").is_file() || !group.0.join("cgroup.procs").is_file() {
            return Err(denied("missing kernel cgroup cleanup control"));
        }
        Ok(group)
    }

    fn is_in_witness_group() -> io::Result<()> {
        let data = fs::read_to_string("/proc/self/cgroup")?;
        let relative = data
            .lines()
            .find_map(|line| line.strip_prefix("0::"))
            .ok_or_else(|| denied("fixture lacks kernel cgroup membership"))?;
        if !relative
            .split('/')
            .last()
            .unwrap_or("")
            .starts_with("lu-ide-witness-")
        {
            return Err(denied("fixed child invoked outside private witness cgroup"));
        }
        Ok(())
    }
    fn fixed_child(args: &[String]) -> io::Result<bool> {
        match args {
            [mode] if mode == "--fixed-grandchild" => {
                is_in_witness_group()?;
                thread::sleep(Duration::from_secs(120));
                Ok(true)
            }
            [mode] if mode == "--fixed-parent" => {
                is_in_witness_group()?;
                let grandchild = Command::new(env::current_exe()?)
                    .arg("--fixed-grandchild")
                    .env_clear()
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()?;
                println!("READY {}", grandchild.id());
                io::stdout().flush()?;
                thread::sleep(Duration::from_secs(120));
                Ok(true)
            }
            [] => Ok(false),
            _ => Err(denied("unsupported native witness mode")),
        }
    }

    fn require_exact_member(group: &WitnessGroup, pid: u32) -> io::Result<()> {
        let observed = fs::read_to_string(format!("/proc/{pid}/cgroup"))?;
        let relative = observed
            .lines()
            .find_map(|line| line.strip_prefix("0::"))
            .ok_or_else(|| denied("missing kernel process membership"))?;
        let current =
            fs::canonicalize(Path::new("/sys/fs/cgroup").join(relative.trim_start_matches('/')))?;
        if current != group.0 {
            return Err(denied("child escaped exact kernel cgroup"));
        }
        if !fs::read_to_string(group.0.join("cgroup.procs"))?
            .lines()
            .any(|line| line.parse::<u32>().ok() == Some(pid))
        {
            return Err(denied("child missing from cgroup.procs"));
        }
        Ok(())
    }

    fn run_native() -> io::Result<()> {
        let parent = delegated_parent()?;
        let group = create_group(&parent)?;
        let proc_file = OpenOptions::new()
            .write(true)
            .open(group.0.join("cgroup.procs"))?;
        let fd = proc_file.as_raw_fd();
        let mut cmd = Command::new(env::current_exe()?);
        cmd.arg("--fixed-parent")
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        // cgroup.procs accepts "0" to attach the *writing* process. Do this
        // after fork, before exec, using one async-signal-safe syscall and a
        // preopened fd. A failure aborts Command::spawn (no uncontained exec).
        unsafe {
            cmd.pre_exec(move || {
                let value = b"0";
                if write(fd, value.as_ptr().cast(), value.len()) == 1 {
                    Ok(())
                } else {
                    Err(io::Error::last_os_error())
                }
            });
        }
        let mut child = cmd.spawn()?;
        drop(proc_file);
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| denied("no fixed native witness pipe"))?;
        let (send, recv) = mpsc::channel();
        let _reader = thread::spawn(move || {
            let mut line = String::new();
            let result = BufReader::new(stdout).read_line(&mut line).map(|_| line);
            let _ = send.send(result);
        });
        let line = recv
            .recv_timeout(Duration::from_secs(10))
            .map_err(|_| denied("child-grandchild kernel witness deadline"))??;
        let grandchild: u32 = line
            .strip_prefix("READY ")
            .ok_or_else(|| denied("invalid fixed descendant witness"))?
            .trim()
            .parse()
            .map_err(|_| denied("invalid descendant PID evidence"))?;
        if grandchild == 0 || grandchild == child.id() || child.try_wait()?.is_some() {
            return Err(denied("fixed descendant terminated before kernel witness"));
        }
        require_exact_member(&group, child.id())?;
        require_exact_member(&group, grandchild)?;
        if group.empty()? {
            return Err(denied("kernel reports live worker group empty"));
        }
        // Kernel cgroup.kill acts on the entire descendant tree and handles
        // concurrent forks. Reap the direct child, then independently require
        // the recursive populated=0 kernel observation.
        group.kill()?;
        let empty = group.await_empty()?;
        let _ = child.wait()?;
        if !empty || !group.empty()? {
            return Err(denied("native subtree survived kernel cgroup.kill"));
        }
        println!("PASS: fixed child and grandchild kernel membership pre-exec -> cgroup.kill -> populated 0");
        println!("SCOPE: cgroup OS witness only; bwrap and native Cargo/recovery remain independent gates");
        drop(group);
        Ok(())
    }

    pub fn run() -> io::Result<()> {
        let args: Vec<String> = env::args().skip(1).collect();
        if fixed_child(&args)? {
            return Ok(());
        }
        run_native()
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn exact_kernel_events_are_required() {
            assert_eq!(populated("populated 0\nfrozen 0\n").unwrap(), false);
            assert_eq!(populated("populated 1\nfrozen 0\n").unwrap(), true);
            assert!(populated("populated 0\npopulated 0\n").is_err());
            assert!(populated("populated 2\n").is_err());
            assert!(populated("frozen 0\n").is_err());
        }
        #[test]
        fn advertised_controllers_must_all_be_present() {
            assert!(exact_controllers("cpu memory pids io"));
            assert!(!exact_controllers("cpu memory io"));
            assert!(!exact_controllers("+cpu +memory +pids"));
        }
        #[test]
        fn fixture_modes_are_fixed_and_no_dynamic_commands_exist() {
            assert!(fixed_child(&["--arbitrary-executable".to_owned()]).is_err());
        }
    }
}

#[cfg(target_os = "linux")]
fn main() {
    if let Err(error) = native::run() {
        eprintln!("DENIED: native WO4 cgroup witness: {error}");
        std::process::exit(2);
    }
}
