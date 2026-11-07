//! WO4 fixed trusted systemd delegated-cgroup preparation helper.
//!
//! Intended only as an installer-pinned ExecStartPre for the exact
//! lu-ide-delegated.service systemd unit with Delegate=cpu memory pids and
//! DelegateSubgroup=lu-ide-host. No user-provided paths, process IDs,
//! commands, shell, sudo, or fallback cgroup namespace are accepted.
//! This helper does NOT install the unit or execute any project code.
#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("DENIED: WO4 delegated cgroup prepare requires Linux");
    std::process::exit(2);
}

#[cfg(target_os = "linux")]
mod prepare {
    use std::{
        env, fs, io,
        path::{Path, PathBuf},
    };

    fn denied(why: &'static str) -> io::Error {
        io::Error::new(io::ErrorKind::PermissionDenied, why)
    }
    fn valid_controllers(text: &str) -> bool {
        ["cpu", "memory", "pids"]
            .iter()
            .all(|controller| text.split_whitespace().any(|item| item == *controller))
    }
    fn kernel_location() -> io::Result<PathBuf> {
        let data = fs::read_to_string("/proc/self/cgroup")?;
        let mut lines = data.lines().filter_map(|line| line.strip_prefix("0::"));
        let rel = lines.next().ok_or_else(|| denied("no unified cgroup-v2 identity"))?;
        if lines.next().is_some()
            || !rel.starts_with('/')
            || rel.split('/').any(|component| component == "." || component == "..")
        {
            return Err(denied("untrusted cgroup location"));
        }
        let root = fs::canonicalize("/sys/fs/cgroup")?;
        let current = fs::canonicalize(root.join(rel.trim_start_matches('/')))?;
        if !current.starts_with(&root) || current == root {
            return Err(denied("cgroup identity escaped kernel mount"));
        }
        Ok(current)
    }
    fn root_and_mode(current: &Path) -> io::Result<(PathBuf, bool)> {
        let name = current.file_name().and_then(|s| s.to_str());
        let root = match name {
            Some("lu-ide-delegated.service") => current,
            Some(".control" | "lu-ide-host") => {
                current.parent().ok_or_else(|| denied("no service cgroup parent"))?
            }
            _ => return Err(denied("not an exact delegated systemd service process")),
        };
        if root.file_name().and_then(|s| s.to_str()) != Some("lu-ide-delegated.service") {
            return Err(denied("unexpected delegated unit name"));
        }
        Ok((root.to_path_buf(), current == root))
    }
    fn exactly_self_at_root(root: &Path) -> io::Result<bool> {
        let pids = fs::read_to_string(root.join("cgroup.procs"))?;
        let self_pid = std::process::id();
        let mut observed = pids.lines().filter(|line| !line.trim().is_empty());
        Ok(observed.next().and_then(|pid| pid.parse::<u32>().ok()) == Some(self_pid)
            && observed.next().is_none())
    }
    fn require_empty_internal_processes(root: &Path) -> io::Result<()> {
        if fs::read_to_string(root.join("cgroup.procs"))?.trim().is_empty() {
            Ok(())
        } else {
            Err(denied("service root still contains internal processes"))
        }
    }
    fn prepare_exact_delegation() -> io::Result<()> {
        if env::args_os().len() != 1 {
            return Err(denied("trusted prepare accepts no options"));
        }
        let current = kernel_location()?;
        let (unit, was_at_unit_root) = root_and_mode(&current)?;
        if fs::read_to_string(unit.join("cgroup.type"))?.trim() != "domain" {
            return Err(denied("delegation must be a domain cgroup"));
        }
        if !valid_controllers(&fs::read_to_string(unit.join("cgroup.controllers"))?) {
            return Err(denied("systemd did not delegate CPU/memory/PID controllers"));
        }
        let host = unit.join("lu-ide-host");
        match fs::create_dir(&host) {
            Ok(()) => {}
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
                let meta = fs::symlink_metadata(&host)?;
                if !meta.is_dir() || meta.file_type().is_symlink() {
                    return Err(denied("host leaf identity unexpectedly changed"));
                }
            }
            Err(err) => return Err(err),
        }
        if was_at_unit_root {
            // The only permitted migration is this single trusted preparation
            // process, from the exact service unit root to its fixed child.
            // Never move an arbitrary PID, pre-existing task or other user.
            if !exactly_self_at_root(&unit)?
                || !fs::read_to_string(host.join("cgroup.procs"))?.trim().is_empty()
            {
                return Err(denied("cannot migrate extra service processes"));
            }
            fs::write(host.join("cgroup.procs"), std::process::id().to_string())?;
            if kernel_location()? != fs::canonicalize(&host)? {
                return Err(denied("kernel did not attach fixed host process"));
            }
        } else if current.file_name().and_then(|s| s.to_str()) == Some("lu-ide-host") {
            // The process is already inside the manager-assigned DelegateSubgroup.
            if current != fs::canonicalize(&host)? {
                return Err(denied("host leaf mismatch"));
            }
        }
        // cgroup-v2 prohibits enabling domain controllers in a populated
        // nonroot cgroup. Systemd owns the unit boundary and delegated it;
        // only the exact unit's subtree_control is modified.
        require_empty_internal_processes(&unit)?;
        let enabled = fs::read_to_string(unit.join("cgroup.subtree_control"))?;
        let need: Vec<String> = ["cpu", "memory", "pids"]
            .iter()
            .filter(|c| !enabled.split_whitespace().any(|e| e == **c))
            .map(|c| format!("+{c}"))
            .collect();
        if !need.is_empty() {
            fs::write(unit.join("cgroup.subtree_control"), need.join(" "))?;
        }
        let observed = fs::read_to_string(unit.join("cgroup.subtree_control"))?;
        if !valid_controllers(&observed) {
            return Err(denied("kernel refused required subtree controller activation"));
        }
        require_empty_internal_processes(&unit)?;
        if !host.join("cgroup.procs").is_file() {
            return Err(denied("missing fixed host leaf kernel procs file"));
        }
        println!("PASS: trusted systemd Lu IDE service delegated CPU/memory/PID controllers to empty parent");
        println!("SCOPE: trusted cgroup preparation only, not a worker or crash-recovery acceptance");
        Ok(())
    }

    pub fn run() -> io::Result<()> {
        prepare_exact_delegation()
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn exact_service_identity_only() {
            let service = Path::new("/sys/fs/cgroup/system.slice/lu-ide-delegated.service");
            assert_eq!(root_and_mode(service).unwrap(), (service.to_path_buf(), true));
            let host = service.join("lu-ide-host");
            assert_eq!(root_and_mode(&host).unwrap(), (service.to_path_buf(), false));
            assert!(root_and_mode(Path::new("/tmp/lu-ide-host")).is_err());
            assert!(root_and_mode(Path::new("/sys/fs/cgroup/user.slice/foreign.service")).is_err());
        }
        #[test]
        fn all_controllers_are_required() {
            assert!(valid_controllers("io cpu memory pids"));
            assert!(!valid_controllers("cpu memory"));
            assert!(!valid_controllers("+cpu +memory +pids"));
        }
    }
}

#[cfg(target_os = "linux")]
fn main() {
    if let Err(err) = prepare::run() {
        eprintln!("DENIED: trusted WO4 systemd controller preparation: {err}");
        std::process::exit(2);
    }
}
