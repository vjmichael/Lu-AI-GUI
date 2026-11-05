//! IDE Task 2 — Linux isolation admission and *fixed diagnostic probes*.
//!
//! This crate does not offer an arbitrary command execution API. Even a trusted
//! project cannot run build commands through this interface. A later worker
//! API requires independent scope, cancellation and resource review.
//!
//! Bubblewrap is an isolation toolkit, not itself a security policy. This
//! module denies execution unless the OS sandbox can be constructed.
#![forbid(unsafe_code)]
use std::{
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

#[derive(Debug)]
pub enum SandboxError {
    Unsupported,
    UnsafeWorkspace,
    LauncherUnavailable,
    LauncherUntrusted,
    Spawn(io::Error),
    TimedOut,
    IsolatedCommandFailed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Probe {
    /// Exercise namespace creation and the minimal runtime mounts.
    NamespaceSetup,
    /// Assert host home and privileged sockets cannot be seen.
    SecretIsolation,
    /// Assert the project input is mounted read-only.
    ReadOnlyWorkspace,
    /// Assert an isolated process cannot connect to an external address.
    NetworkIsolation,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ProbeResult {
    pub probe: Probe,
    pub exit_code: i32,
}

fn safe_workspace(workspace: &Path) -> Result<PathBuf, SandboxError> {
    let metadata = fs::symlink_metadata(workspace).map_err(|_| SandboxError::UnsafeWorkspace)?;
    if !metadata.file_type().is_dir() {
        return Err(SandboxError::UnsafeWorkspace);
    }
    let canonical = fs::canonicalize(workspace).map_err(|_| SandboxError::UnsafeWorkspace)?;
    // Never allow a test to expose all of the filesystem as the workspace.
    if canonical.parent().is_none() || canonical == Path::new("/usr") {
        return Err(SandboxError::UnsafeWorkspace);
    }
    Ok(canonical)
}

#[cfg(target_os = "linux")]
fn trusted_bwrap() -> Result<PathBuf, SandboxError> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    const BWRAP: &str = "/usr/bin/bwrap";
    let meta = fs::symlink_metadata(BWRAP).map_err(|_| SandboxError::LauncherUnavailable)?;
    if !meta.file_type().is_file()
        || meta.uid() != 0
        || meta.permissions().mode() & (0o4000 | 0o0022) != 0
    {
        return Err(SandboxError::LauncherUntrusted);
    }
    let canonical = fs::canonicalize(BWRAP).map_err(|_| SandboxError::LauncherUntrusted)?;
    if !canonical.starts_with("/usr/") {
        return Err(SandboxError::LauncherUntrusted);
    }
    Ok(canonical)
}

fn fixed_probe_args(probe: Probe) -> Vec<OsString> {
    // All programs and their arguments are fixed, not model/user input.
    let values: &[&str] = match probe {
        Probe::NamespaceSetup => &["/usr/bin/true"],
        Probe::SecretIsolation => &[
            "/bin/sh",
            "-c",
            "test ! -e /home/runner/.ssh && test ! -e /root/.ssh && test ! -e /run/user && test ! -e /var/run/docker.sock",
        ],
        Probe::ReadOnlyWorkspace => &[
            "/bin/sh",
            "-c",
            "! (touch /workspace/lu-ide-sandbox-escape-marker 2>/dev/null)",
        ],
        Probe::NetworkIsolation => &[
            "/usr/bin/python3",
            "-c",
            "import socket,sys; s=socket.socket(); s.settimeout(0.5); sys.exit(0 if s.connect_ex(('1.1.1.1',53)) != 0 else 1)",
        ],
    };
    values.iter().map(OsString::from).collect()
}

fn mount_system_path(args: &mut Vec<OsString>, path: &str) {
    // Keep the basic Linux runtime visible for trusted *fixed probes* only.
    // Canonicalizing the source collapses /bin and /lib symlinks to /usr.
    if let Ok(real) = fs::canonicalize(path) {
        if real.is_dir() {
            args.extend(["--ro-bind".into(), real.into_os_string(), path.into()]);
        }
    }
}

fn arguments(workspace: &Path, probe: Probe) -> Result<Vec<OsString>, SandboxError> {
    let root = safe_workspace(workspace)?;
    let mut args: Vec<OsString> = [
        "--unshare-all",
        "--die-with-parent",
        "--new-session",
        "--clearenv",
        "--proc",
        "/proc",
        "--dev",
        "/dev",
        "--tmpfs",
        "/tmp",
        "--dir",
        "/home",
        "--ro-bind",
        "/usr",
        "/usr",
    ]
    .into_iter()
    .map(OsString::from)
    .collect();
    for path in ["/bin", "/lib", "/lib64", "/sbin"] {
        mount_system_path(&mut args, path);
    }
    args.extend([
        "--ro-bind".into(),
        root.into_os_string(),
        "/workspace".into(),
        "--chdir".into(),
        "/workspace".into(),
        "--setenv".into(),
        "HOME".into(),
        "/home".into(),
        "--setenv".into(),
        "PATH".into(),
        "/usr/bin:/bin".into(),
        "--setenv".into(),
        "LANG".into(),
        "C".into(),
        "--".into(),
    ]);
    args.extend(fixed_probe_args(probe));
    Ok(args)
}

/// Runs only predefined, bounded probes. On an unsupported host this returns
/// an error, never a normal or reduced-privilege execution fallback.
pub fn run_probe(
    workspace: &Path,
    probe: Probe,
    deadline: Duration,
) -> Result<ProbeResult, SandboxError> {
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (workspace, probe, deadline);
        return Err(SandboxError::Unsupported);
    }
    #[cfg(target_os = "linux")]
    {
        let launcher = trusted_bwrap()?;
        if deadline.is_zero() || deadline > Duration::from_secs(15) {
            return Err(SandboxError::TimedOut);
        }
        let args = arguments(workspace, probe)?;
        let mut child = Command::new(launcher)
            .args(args)
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(SandboxError::Spawn)?;
        let started = Instant::now();
        loop {
            match child.try_wait().map_err(SandboxError::Spawn)? {
                Some(status) if status.success() => {
                    return Ok(ProbeResult {
                        probe,
                        exit_code: status.code().unwrap_or(0),
                    });
                }
                Some(_) => return Err(SandboxError::IsolatedCommandFailed),
                None if started.elapsed() < deadline => thread::sleep(Duration::from_millis(25)),
                None => {
                    child.kill().map_err(SandboxError::Spawn)?;
                    child.wait().map_err(SandboxError::Spawn)?;
                    return Err(SandboxError::TimedOut);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dangerous_roots_and_missing_paths_are_rejected() {
        assert!(matches!(
            safe_workspace(Path::new("/")),
            Err(SandboxError::UnsafeWorkspace)
        ));
        assert!(safe_workspace(Path::new("/some/path/that/does/not/exist")).is_err());
    }

    #[test]
    fn plan_has_no_host_home_mount_and_denies_network() {
        let scratch = std::env::temp_dir().join(format!("lu-ide-sbx-{}", std::process::id()));
        fs::create_dir_all(&scratch).unwrap();
        let args = arguments(&scratch, Probe::NamespaceSetup).unwrap();
        let normalized: Vec<String> = args
            .iter()
            .map(|x| x.to_string_lossy().to_string())
            .collect();
        assert!(normalized.contains(&"--unshare-all".to_owned()));
        assert!(normalized.contains(&"--new-session".to_owned()));
        assert!(normalized.contains(&"--die-with-parent".to_owned()));
        assert!(normalized.contains(&"--clearenv".to_owned()));
        assert!(normalized.contains(&"--ro-bind".to_owned()));
        assert!(!normalized
            .iter()
            .any(|x| x == "--bind" || x == "--share-net"));
        assert!(!normalized
            .iter()
            .any(|x| x == "/root" || x.starts_with("/home/")));
        assert!(normalized.iter().any(|x| x == "/workspace"));
        fs::remove_dir_all(scratch).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn symlink_directory_cannot_become_an_isolation_root() {
        use std::os::unix::fs::symlink;
        let base = std::env::temp_dir().join(format!("lu-ide-link-{}", std::process::id()));
        fs::create_dir_all(&base).unwrap();
        let real = base.join("real");
        fs::create_dir_all(&real).unwrap();
        let link = base.join("link");
        symlink(&real, &link).unwrap();
        assert!(matches!(
            safe_workspace(&link),
            Err(SandboxError::UnsafeWorkspace)
        ));
        fs::remove_dir_all(base).unwrap();
    }
}
