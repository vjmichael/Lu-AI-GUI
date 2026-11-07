//! E2B Linux native supervised long-lived LSP process.
//! Runs only compiled-in language engines through root-owned Bubblewrap with
//! delegated cgroup-v2 CPU/memory/PID controls and no network namespace.
//! Does not invoke shell, project scripts, package managers or PATH programs.
//! The native host owns exact installed provenance and a fresh D3 grant.
use super::*;
use crate::{
    lsp_transport::{LspGateway, RestrictedLspProcess},
    snapshot::PublishedInput,
};
use lu_ide_workspace::{
    lsp::DiagnosticSource,
    lsp_session::{LspEvent, LspPhase, Position},
};
use std::{
    collections::BTreeMap,
    fs::File,
    io,
    os::fd::{AsRawFd, RawFd},
    process::{Child, ChildStdin, ChildStdout},
    sync::Arc,
};
const MAX_LSP_BUNDLE_FILES: usize = 1000;
const MAX_LSP_BUNDLE_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_LSP_FILE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_LSP_WRITE: usize = 512 * 1024 + 256;

fn fixed_server(
    plan: &JobPlan,
) -> Result<(&'static str, &'static str, &'static [&'static str]), SupervisorError> {
    if plan.effect_class != "language_analysis_isolated" {
        return Err(denied());
    }
    match plan.recipe_id.as_str() {
        "lsp_rust_analyzer" => Ok(("rust-analyzer", "rust-analyzer", &[])),
        "lsp_clangd" => Ok((
            "clangd",
            "clangd",
            &["--log=error", "--background-index=false"],
        )),
        "lsp_ruff" => Ok(("ruff", "ruff", &["server"])),
        _ => Err(SupervisorError::InvalidPlan),
    }
}
fn audit_bundle(root: &Path) -> Result<String, SupervisorError> {
    let meta = fs::symlink_metadata(root).map_err(|_| denied())?;
    if !meta.is_dir()
        || meta.file_type().is_symlink()
        || meta.uid() != 0
        || meta.mode() & 0o022 != 0
    {
        return Err(denied());
    }
    let canonical = fs::canonicalize(root).map_err(|_| denied())?;
    if canonical != root {
        return Err(denied());
    }
    let mut entries: BTreeMap<String, PathBuf> = BTreeMap::new();
    fn walk(
        base: &Path,
        relative: &Path,
        entries: &mut BTreeMap<String, PathBuf>,
        total: &mut u64,
    ) -> Result<(), SupervisorError> {
        if relative.components().count() > 18 {
            return Err(denied());
        }
        for item in fs::read_dir(base.join(relative)).map_err(|_| denied())? {
            let item = item.map_err(|_| denied())?;
            let name = item.file_name().to_str().ok_or_else(denied)?.to_owned();
            if name.is_empty()
                || name.len() > 200
                || !name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b'+'))
            {
                return Err(denied());
            }
            let path = relative.join(name);
            let full = base.join(&path);
            let meta = fs::symlink_metadata(&full).map_err(|_| denied())?;
            if meta.is_symlink() || meta.uid() != 0 || meta.mode() & 0o022 != 0 {
                return Err(denied());
            }
            if meta.is_dir() {
                walk(base, &path, entries, total)?;
            } else if meta.is_file() {
                if meta.len() > MAX_LSP_FILE_BYTES {
                    return Err(denied());
                }
                *total = total.checked_add(meta.len()).ok_or_else(denied)?;
                if *total > MAX_LSP_BUNDLE_BYTES || entries.len() >= MAX_LSP_BUNDLE_FILES {
                    return Err(denied());
                }
                let key = path.to_str().ok_or_else(denied)?.replace('\\', "/");
                entries.insert(key, full);
            } else {
                return Err(denied());
            }
        }
        Ok(())
    }
    let mut total = 0u64;
    walk(root, Path::new(""), &mut entries, &mut total)?;
    let mut overall = Sha256::new();
    for (relative, path) in entries {
        let mut reader = File::open(path).map_err(|_| denied())?;
        let mut digest = Sha256::new();
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let n = reader.read(&mut buffer).map_err(|_| denied())?;
            if n == 0 {
                break;
            }
            digest.update(&buffer[..n]);
        }
        overall.update(relative.as_bytes());
        overall.update([0]);
        overall.update(format!("{:x}", digest.finalize()).as_bytes());
        overall.update([0]);
    }
    Ok(format!("{:x}", overall.finalize()))
}
/// Only a root-owned, non-writable, release-verified tree is usable.
/// The reviewed expected digest is supplied by trusted installer metadata.
pub fn native_lsp_bundle_digest(root: &Path, recipe: &str) -> Result<String, SupervisorError> {
    let filename = match recipe {
        "lsp_rust_analyzer" => "rust-analyzer",
        "lsp_clangd" => "clangd",
        "lsp_ruff" => "ruff",
        _ => return Err(SupervisorError::InvalidPlan),
    };
    let bin = root.join("bin").join(filename);
    ensure_root_owned_tool(&bin)?;
    audit_bundle(root)
}
fn set_nonblocking(fd: RawFd) -> Result<(), SupervisorError> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(denied());
    }
    Ok(())
}
fn poll_fd(fd: RawFd, flags: i16, timeout_ms: i32) -> io::Result<bool> {
    let mut item = libc::pollfd {
        fd,
        events: flags,
        revents: 0,
    };
    let result = unsafe { libc::poll(&mut item, 1, timeout_ms) };
    if result < 0 {
        return Err(io::Error::last_os_error());
    }
    if result == 0 {
        return Ok(false);
    }
    if item.revents & (libc::POLLNVAL | libc::POLLERR) != 0 {
        return Err(io::ErrorKind::BrokenPipe.into());
    }
    Ok((item.revents & flags) != 0)
}
struct LinuxLspProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: ChildStdout,
    group: Cgroup,
    released: bool,
}
// Owns nonblocking stdio and the only native child owner, with cgroup as
// kernel authority. No data path to the WebView and no ambient project mount.
impl RestrictedLspProcess for LinuxLspProcess {
    fn write_bounded(&mut self, bytes: &[u8]) -> io::Result<()> {
        if bytes.is_empty() || bytes.len() > MAX_LSP_WRITE {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        if self.child.try_wait()?.is_some() {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        let fd = self.stdin.as_raw_fd();
        let start = Instant::now();
        let mut sent = 0usize;
        while sent < bytes.len() {
            if start.elapsed() >= Duration::from_secs(3) {
                return Err(io::ErrorKind::TimedOut.into());
            }
            if !poll_fd(fd, libc::POLLOUT, 150)? {
                continue;
            }
            let count = (bytes.len() - sent).min(8192);
            let n = unsafe { libc::write(fd, bytes[sent..].as_ptr().cast(), count) };
            if n < 0 {
                let error = io::Error::last_os_error();
                if error.kind() == io::ErrorKind::WouldBlock
                    || error.kind() == io::ErrorKind::Interrupted
                {
                    continue;
                }
                return Err(error);
            }
            if n == 0 {
                return Err(io::ErrorKind::WriteZero.into());
            }
            sent += n as usize;
        }
        Ok(())
    }
    fn read_bounded(&mut self, limit: usize, timeout: Duration) -> io::Result<Vec<u8>> {
        if limit == 0 || limit > 16 * 1024 {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let timeout = i32::try_from(timeout.as_millis().min(1000)).unwrap_or(1000);
        let fd = self.stdout.as_raw_fd();
        if !poll_fd(fd, libc::POLLIN | libc::POLLHUP, timeout)? {
            if self.child.try_wait()?.is_some() {
                return Err(io::ErrorKind::BrokenPipe.into());
            }
            return Ok(Vec::new());
        }
        let mut data = vec![0u8; limit];
        let n = unsafe { libc::read(fd, data.as_mut_ptr().cast(), limit) };
        if n < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::WouldBlock {
                return Ok(Vec::new());
            }
            return Err(error);
        }
        if n == 0 {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        data.truncate(n as usize);
        Ok(data)
    }
    fn terminate_tree_and_verify(&mut self) -> Result<bool, SupervisorError> {
        let empty = self.group.kill_and_verify()?;
        if empty {
            let _ = self.child.wait();
        }
        Ok(empty)
    }
    fn release_after_verified_terminal(&mut self) -> Result<(), SupervisorError> {
        if !self.group.zero_members() {
            return Err(denied());
        }
        self.released = true;
        Ok(())
    }
}
impl Drop for LinuxLspProcess {
    fn drop(&mut self) {
        if !self.released {
            let _ = self.group.kill_and_verify();
        }
    }
}
pub fn start_linux_lsp(
    supervisor: Arc<Supervisor>,
    job_id: &str,
    plan: &JobPlan,
    current_native_head: &str,
    input: &PublishedInput,
    release_root: &Path,
) -> Result<LinuxLspSession, SupervisorError> {
    let (server_id, executable, args) = fixed_server(plan)?;
    if plan.source_head != current_native_head
        || plan.input_snapshot_sha256 != input.seal.source_sha256
        || plan.manifest_sha256 != input.seal.manifest_sha256
    {
        return Err(SupervisorError::WrongProjectOrSource);
    }
    verify_input(input).map_err(|_| SupervisorError::WrongProjectOrSource)?;
    let relative = Path::new(&input.seal.manifest_relative_path);
    if !input.root.join(relative).is_file() {
        return Err(SupervisorError::WrongProjectOrSource);
    }
    let native_uri =
        crate::lsp_transport::trusted_staged_file_uri(&Path::new("/workspace").join(relative))?;
    if native_lsp_bundle_digest(release_root, &plan.recipe_id)? != plan.toolchain_sha256 {
        return Err(SupervisorError::WrongProjectOrSource);
    }
    ensure_root_owned_tool(Path::new(BWRAP))?;
    let group = make_cgroup(job_id, plan)?;
    let proc_file = OpenOptions::new()
        .write(true)
        .open(group.dir.join("cgroup.procs"))
        .map_err(|_| denied())?;
    let descriptor = proc_file.as_raw_fd();
    let native_executable = format!("/ls/bin/{executable}");
    let mut command = Command::new(BWRAP);
    command
        .env_clear()
        .arg("--die-with-parent")
        .arg("--new-session")
        .arg("--unshare-all")
        .arg("--clearenv")
        .arg("--ro-bind")
        .arg("/usr")
        .arg("/usr")
        .arg("--ro-bind-try")
        .arg("/bin")
        .arg("/bin")
        .arg("--ro-bind-try")
        .arg("/lib")
        .arg("/lib")
        .arg("--ro-bind-try")
        .arg("/lib64")
        .arg("/lib64")
        .arg("--ro-bind")
        .arg(&input.root)
        .arg("/workspace")
        .arg("--ro-bind")
        .arg(release_root)
        .arg("/ls")
        .arg("--tmpfs")
        .arg("/tmp")
        .arg("--proc")
        .arg("/proc")
        .arg("--dev")
        .arg("/dev")
        .arg("--setenv")
        .arg("HOME")
        .arg("/tmp")
        .arg("--setenv")
        .arg("PATH")
        .arg("/ls/bin:/usr/bin:/bin")
        .arg("--setenv")
        .arg("LANG")
        .arg("C")
        .arg("--chdir")
        .arg("/workspace")
        .arg("--")
        .arg(&native_executable)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    unsafe {
        command.pre_exec(move || attach_pid_in_pre_exec(descriptor));
    }
    let proof = proof_for(&group);
    if !proof.complete() {
        return Err(denied());
    }
    supervisor.consume_grant_after_attestation(job_id, plan, &proof)?;
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(_) => {
            if group.zero_members() {
                supervisor.native_deny_before_start(job_id, true)?;
            } else {
                let _ = supervisor.native_quarantine_after_error(job_id);
            }
            return Err(denied());
        }
    };
    drop(proc_file);
    let stdin = child.stdin.take().ok_or_else(denied)?;
    let stdout = child.stdout.take().ok_or_else(denied)?;
    if let Err(e) =
        set_nonblocking(stdin.as_raw_fd()).and_then(|_| set_nonblocking(stdout.as_raw_fd()))
    {
        let _ = group.kill_and_verify();
        let _ = supervisor.native_quarantine_after_error(job_id);
        return Err(e);
    }
    if let Err(e) = supervisor.native_mark_running(job_id) {
        let _ = group.kill_and_verify();
        let _ = supervisor.native_quarantine_after_error(job_id);
        return Err(e);
    }
    let process = LinuxLspProcess {
        child,
        stdin,
        stdout,
        group,
        released: false,
    };
    let plan_hash = supervisor
        .inspect()?
        .into_iter()
        .find(|x| x.job_id == job_id)
        .ok_or(SupervisorError::NoSuchJob)?
        .plan_sha256;
    let gateway = LspGateway::attach_native(
        Arc::clone(&supervisor),
        job_id,
        &plan_hash,
        &proof,
        Box::new(process),
        server_id,
    )?;
    Ok(LinuxLspSession {
        gateway,
        native_uri,
    })
}
pub struct LinuxLspSession {
    gateway: LspGateway,
    native_uri: String,
}
impl LinuxLspSession {
    pub fn native_document_uri(&self) -> &str {
        &self.native_uri
    }
    pub fn initialize(&mut self) -> Result<(), SupervisorError> {
        self.gateway.initialize()
    }
    pub fn open(
        &mut self,
        source: DiagnosticSource,
        uri: &str,
        text: &str,
    ) -> Result<(), SupervisorError> {
        self.gateway.open(source, uri, text)
    }
    pub fn changed(
        &mut self,
        current: &DiagnosticSource,
        next: DiagnosticSource,
        text: &str,
    ) -> Result<(), SupervisorError> {
        self.gateway.changed(current, next, text)
    }
    pub fn query(
        &mut self,
        kind: &str,
        source: &DiagnosticSource,
        pos: Position,
    ) -> Result<(), SupervisorError> {
        self.gateway.query(kind, source, pos)
    }
    pub fn poll(&mut self) -> Result<Vec<LspEvent>, SupervisorError> {
        self.gateway.poll()
    }
    pub fn close(self) -> Result<(), SupervisorError> {
        self.gateway.close()
    }
    pub fn is_ready(&self) -> bool {
        self.gateway.phase() == LspPhase::Ready
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_release_bundle_is_not_a_user_writable_project_binary() {
        let unavailable = Path::new("/tmp/nonexistent-unapproved-lsp-release");
        assert!(native_lsp_bundle_digest(unavailable, "lsp_clangd").is_err());
        assert!(native_lsp_bundle_digest(unavailable, "arbitrary_shell").is_err());
    }
}
