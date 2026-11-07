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
    process::{Child, ChildStderr, ChildStdin, ChildStdout},
    sync::Arc,
};
// Fixed CI/debug-only source-stage identifiers; never record paths, PIDs,
// source text, cgroup names, credentials or untrusted LSP protocol contents.
fn native_linux_lsp_trace(stage: &'static str) {
    if cfg!(debug_assertions)
        && std::env::var_os("LU_IDE_E2_NATIVE_TRACE").as_deref() == Some(std::ffi::OsStr::new("1"))
    {
        eprintln!("PROTECTED_LINUX_LSP_STAGE={stage}");
    }
}

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
            &["--log=error", "--background-index=false", "-j=1"],
        )),
        "lsp_ruff" => Ok(("ruff", "ruff", &["server"])),
        _ => Err(SupervisorError::InvalidPlan),
    }
}
fn audit_bundle(root: &Path) -> Result<String, SupervisorError> {
    let meta = fs::symlink_metadata(root).map_err(|_| denied())?;
    native_linux_lsp_trace("bundle_root_metadata_read");
    // Fixed nonidentifying debug-only classification of native admission.
    if !meta.is_dir() {
        native_linux_lsp_trace("bundle_root_not_directory");
        return Err(denied());
    }
    if meta.file_type().is_symlink() {
        native_linux_lsp_trace("bundle_root_symlink_denied");
        return Err(denied());
    }
    if meta.uid() != 0 {
        native_linux_lsp_trace("bundle_root_not_root_owned");
        return Err(denied());
    }
    if meta.mode() & 0o022 != 0 {
        native_linux_lsp_trace("bundle_root_group_or_world_writable");
        return Err(denied());
    }
    native_linux_lsp_trace("bundle_root_owner_mode_verified");
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
    native_linux_lsp_trace("bundle_recursive_audit_verified");
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
    native_linux_lsp_trace("bundle_recipe_probe_begin");
    let filename = match recipe {
        "lsp_rust_analyzer" => "rust-analyzer",
        "lsp_clangd" => "clangd",
        "lsp_ruff" => "ruff",
        #[cfg(feature = "dap-transport")]
        "dap_lldb_fixed" => "lldb-dap",
        _ => return Err(SupervisorError::InvalidPlan),
    };
    let bin = root.join("bin").join(filename);
    ensure_root_owned_tool(&bin)?;
    #[cfg(feature = "dap-transport")]
    if recipe == "dap_lldb_fixed" {
        ensure_root_owned_tool(&root.join("debuggee/wo6-selftest"))?;
    }
    native_linux_lsp_trace("root_owned_clangd_binary_verified");
    let result = audit_bundle(root)?;
    native_linux_lsp_trace("root_owned_bundle_tree_verified");
    Ok(result)
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
// Only fixed error categories may be logged, and only for test-only native
// debugger diagnostics with an explicit debug build+trace environment toggle.
// No stderr payload, project path, user text, SID, or process ID is emitted.
fn classify_dap_child_failure(bytes: &[u8]) -> &'static str {
    let lower = String::from_utf8_lossy(bytes).to_ascii_lowercase();
    if lower.contains("error while loading shared libraries")
        || lower.contains("cannot open shared object file")
        || lower.contains("library not loaded")
    {
        "dynamic_library_missing"
    } else if lower.contains("permission denied") || lower.contains("operation not permitted") {
        "os_permission_denied"
    } else if lower.contains("no such file or directory") {
        "required_file_missing"
    } else if lower.contains("failed to create") || lower.contains("read-only file system") {
        "runtime_write_denied"
    } else if lower.contains("ptrace") || lower.contains("debugger") {
        "ptrace_or_debugger_reported"
    } else if lower.contains("cannot find") || lower.contains("not found") {
        "dependency_missing"
    } else if lower.contains("error") || lower.contains("fatal") {
        "other_reported_error"
    } else if bytes.is_empty() {
        "empty"
    } else {
        "other_nonempty"
    }
}
struct LinuxLspProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: ChildStdout,
    debug_stderr: Option<ChildStderr>,
    group: Cgroup,
    released: bool,
}
// Report only a boolean/unknown kernel resource-exhaustion witness, never
// process identity or raw cgroup file contents. Read while the Job is alive
// and before fail-closed cgroup.kill; missing controls remain unavailable.
fn kernel_limit_hit(path: &Path, key: &str) -> &'static str {
    match std::fs::read_to_string(path) {
        Ok(data) => {
            for line in data.lines() {
                if let Some(number) = line.strip_prefix(key).and_then(|v| v.strip_prefix(' ')) {
                    return match number.trim().parse::<u64>() {
                        Ok(0) => "no",
                        Ok(_) => "yes",
                        Err(_) => "unavailable",
                    };
                }
            }
            "unavailable"
        }
        Err(_) => "unavailable",
    }
}
impl LinuxLspProcess {
    fn trace_dap_native_failure(&mut self, stage: &'static str) {
        if !cfg!(debug_assertions)
            || std::env::var_os("LU_IDE_E2_NATIVE_TRACE").as_deref()
                != Some(std::ffi::OsStr::new("1"))
            || self.debug_stderr.is_none()
        {
            return;
        }
        eprintln!("PROTECTED_LINUX_DAP_OR_LSP_CHILD_FAILURE_STAGE={stage}");
        let state = match self.child.try_wait() {
            Ok(Some(status)) if status.success() => "exited_zero",
            Ok(Some(_)) => "exited_nonzero",
            Ok(None) => "running",
            Err(_) => "status_unavailable",
        };
        eprintln!("PROTECTED_LINUX_DAP_CHILD_STATE={state}");
        eprintln!(
            "PROTECTED_LINUX_DAP_PIDS_MAX_HIT={}",
            kernel_limit_hit(&self.group.dir.join("pids.events"), "max")
        );
        eprintln!(
            "PROTECTED_LINUX_DAP_MEMORY_OOM_HIT={}",
            kernel_limit_hit(&self.group.dir.join("memory.events"), "oom_kill")
        );
        if let Some(stderr) = &mut self.debug_stderr {
            let fd = stderr.as_raw_fd();
            let readable = poll_fd(fd, libc::POLLIN | libc::POLLHUP, 0).unwrap_or(false);
            if readable {
                let mut bytes = [0u8; 2048];
                let amount = unsafe { libc::read(fd, bytes.as_mut_ptr().cast(), bytes.len()) };
                if amount > 0 {
                    eprintln!(
                        "PROTECTED_LINUX_DAP_STDERR_CLASS={}",
                        classify_dap_child_failure(&bytes[..amount as usize])
                    );
                } else {
                    eprintln!("PROTECTED_LINUX_DAP_STDERR_CLASS=empty_or_unavailable");
                }
            } else {
                eprintln!("PROTECTED_LINUX_DAP_STDERR_CLASS=pipe_not_ready");
            }
        }
    }
}
// Owns nonblocking stdio and the only native child owner, with cgroup as
// kernel authority. No data path to the WebView and no ambient project mount.
impl RestrictedLspProcess for LinuxLspProcess {
    fn trace_native_fault(&mut self, stage: &'static str) {
        self.trace_dap_native_failure(stage);
    }
    fn write_bounded(&mut self, bytes: &[u8]) -> io::Result<()> {
        if bytes.is_empty() || bytes.len() > MAX_LSP_WRITE {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        if self.child.try_wait()?.is_some() {
            self.trace_dap_native_failure("write_after_child_exit");
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
                self.trace_dap_native_failure("stdout_timeout_child_exited");
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
            self.trace_dap_native_failure("stdout_eof");
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
    native_linux_lsp_trace("begin_recipe_check");
    let (server_id, executable, args) = fixed_server(plan)?;
    if plan.source_head != current_native_head
        || plan.input_snapshot_sha256 != input.seal.source_sha256
        || plan.manifest_sha256 != input.seal.manifest_sha256
    {
        return Err(SupervisorError::WrongProjectOrSource);
    }
    verify_input(input).map_err(|_| SupervisorError::WrongProjectOrSource)?;
    native_linux_lsp_trace("source_snapshot_verified");
    let relative = Path::new(&input.seal.manifest_relative_path);
    if !input.root.join(relative).is_file() {
        return Err(SupervisorError::WrongProjectOrSource);
    }
    let native_uri =
        crate::lsp_transport::trusted_staged_file_uri(&Path::new("/workspace").join(relative))?;
    native_linux_lsp_trace("bundle_digest_start");
    if native_lsp_bundle_digest(release_root, &plan.recipe_id)? != plan.toolchain_sha256 {
        return Err(SupervisorError::WrongProjectOrSource);
    }
    native_linux_lsp_trace("bundle_digest_verified");
    ensure_root_owned_tool(Path::new(BWRAP))?;
    native_linux_lsp_trace("root_owned_bwrap_verified");
    let group = make_cgroup(job_id, plan)?;
    native_linux_lsp_trace("kernel_worker_cgroup_created");
    let proc_file = OpenOptions::new()
        .write(true)
        .open(group.dir.join("cgroup.procs"))
        .map_err(|_| denied())?;
    native_linux_lsp_trace("kernel_cgroup_procs_handle_open");
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
        // The ordinary released worker discards stderr. The independent
        // debug-only native acceptance fixture may collect up to 2048 bytes
        // into an OS pipe, then emit ONLY fixed categories after failure.
        .stderr(
            if cfg!(debug_assertions)
                && std::env::var_os("LU_IDE_E2_NATIVE_TRACE").as_deref()
                    == Some(std::ffi::OsStr::new("1"))
            {
                Stdio::piped()
            } else {
                Stdio::null()
            },
        );
    unsafe {
        command.pre_exec(move || attach_pid_in_pre_exec(descriptor));
    }
    let proof = proof_for(&group);
    if !proof.complete() {
        return Err(denied());
    }
    supervisor.consume_grant_after_attestation(job_id, plan, &proof)?;
    native_linux_lsp_trace("grant_consumed");
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
    native_linux_lsp_trace("bwrap_child_spawned");
    // Identical OS kernel membership gate to fixed native Cargo: the LSP
    // cannot advance to Running on cgroup configuration alone.
    if require_kernel_child_membership(&group, child.id()).is_err() {
        let _ = group.kill_and_verify();
        let _ = supervisor.native_quarantine_after_error(job_id);
        return Err(denied());
    }
    native_linux_lsp_trace("exact_kernel_pid_membership_verified");
    let stdin = child.stdin.take().ok_or_else(denied)?;
    let stdout = child.stdout.take().ok_or_else(denied)?;
    if let Err(e) =
        set_nonblocking(stdin.as_raw_fd()).and_then(|_| set_nonblocking(stdout.as_raw_fd()))
    {
        let _ = group.kill_and_verify();
        let _ = supervisor.native_quarantine_after_error(job_id);
        return Err(e);
    }
    native_linux_lsp_trace("bounded_stdio_ready");
    if let Err(e) = supervisor.native_mark_running(job_id) {
        let _ = group.kill_and_verify();
        let _ = supervisor.native_quarantine_after_error(job_id);
        return Err(e);
    }
    native_linux_lsp_trace("running_receipt_fsynced");
    let debug_stderr = child.stderr.take();
    let process = LinuxLspProcess {
        child,
        stdin,
        stdout,
        debug_stderr,
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
    native_linux_lsp_trace("gateway_attached");
    Ok(LinuxLspSession {
        gateway,
        native_uri,
    })
}
#[cfg(feature = "dap-transport")]
pub fn start_linux_dap(
    supervisor: Arc<Supervisor>,
    job_id: &str,
    plan: &JobPlan,
    current_native_head: &str,
    input: &PublishedInput,
    release_root: &Path,
) -> Result<LinuxDapSession, SupervisorError> {
    native_linux_lsp_trace("begin_recipe_check");
    if plan.recipe_id != "dap_lldb_fixed" || plan.effect_class != "debug_inspection_isolated" {
        return Err(SupervisorError::InvalidPlan);
    }
    let executable = "lldb-dap";
    let args: &[&str] = &[];
    if plan.source_head != current_native_head
        || plan.input_snapshot_sha256 != input.seal.source_sha256
        || plan.manifest_sha256 != input.seal.manifest_sha256
    {
        return Err(SupervisorError::WrongProjectOrSource);
    }
    verify_input(input).map_err(|_| SupervisorError::WrongProjectOrSource)?;
    native_linux_lsp_trace("source_snapshot_verified");
    let relative = Path::new(&input.seal.manifest_relative_path);
    if !input.root.join(relative).is_file() {
        return Err(SupervisorError::WrongProjectOrSource);
    }
    native_linux_lsp_trace("bundle_digest_start");
    if native_lsp_bundle_digest(release_root, &plan.recipe_id)? != plan.toolchain_sha256 {
        return Err(SupervisorError::WrongProjectOrSource);
    }
    native_linux_lsp_trace("bundle_digest_verified");
    ensure_root_owned_tool(Path::new(BWRAP))?;
    native_linux_lsp_trace("root_owned_bwrap_verified");
    let group = make_cgroup(job_id, plan)?;
    native_linux_lsp_trace("kernel_worker_cgroup_created");
    let proc_file = OpenOptions::new()
        .write(true)
        .open(group.dir.join("cgroup.procs"))
        .map_err(|_| denied())?;
    native_linux_lsp_trace("kernel_cgroup_procs_handle_open");
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
        .stderr(
            if cfg!(debug_assertions)
                && std::env::var_os("LU_IDE_E2_NATIVE_TRACE").as_deref()
                    == Some(std::ffi::OsStr::new("1"))
            {
                Stdio::piped()
            } else {
                Stdio::null()
            },
        );
    unsafe {
        command.pre_exec(move || attach_pid_in_pre_exec(descriptor));
    }
    let proof = proof_for(&group);
    if !proof.complete() {
        return Err(denied());
    }
    supervisor.consume_grant_after_attestation(job_id, plan, &proof)?;
    native_linux_lsp_trace("grant_consumed");
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
    native_linux_lsp_trace("bwrap_child_spawned");
    // Identical OS kernel membership gate to fixed native Cargo: the LSP
    // cannot advance to Running on cgroup configuration alone.
    if require_kernel_child_membership(&group, child.id()).is_err() {
        let _ = group.kill_and_verify();
        let _ = supervisor.native_quarantine_after_error(job_id);
        return Err(denied());
    }
    native_linux_lsp_trace("exact_kernel_pid_membership_verified");
    let stdin = child.stdin.take().ok_or_else(denied)?;
    let stdout = child.stdout.take().ok_or_else(denied)?;
    if let Err(e) =
        set_nonblocking(stdin.as_raw_fd()).and_then(|_| set_nonblocking(stdout.as_raw_fd()))
    {
        let _ = group.kill_and_verify();
        let _ = supervisor.native_quarantine_after_error(job_id);
        return Err(e);
    }
    native_linux_lsp_trace("bounded_stdio_ready");
    if let Err(e) = supervisor.native_mark_running(job_id) {
        let _ = group.kill_and_verify();
        let _ = supervisor.native_quarantine_after_error(job_id);
        return Err(e);
    }
    native_linux_lsp_trace("running_receipt_fsynced");
    let debug_stderr = child.stderr.take();
    let process = LinuxLspProcess {
        child,
        stdin,
        stdout,
        debug_stderr,
        group,
        released: false,
    };
    let plan_hash = supervisor
        .inspect()?
        .into_iter()
        .find(|x| x.job_id == job_id)
        .ok_or(SupervisorError::NoSuchJob)?
        .plan_sha256;
    let gateway = crate::dap_transport::DapGateway::attach_native(
        Arc::clone(&supervisor),
        job_id,
        &plan_hash,
        &proof,
        Box::new(process),
    )?;
    native_linux_lsp_trace("gateway_attached");
    Ok(LinuxDapSession { gateway })
}
#[cfg(feature = "dap-transport")]
pub struct LinuxDapSession {
    gateway: crate::dap_transport::DapGateway,
}
#[cfg(feature = "dap-transport")]
impl LinuxDapSession {
    pub fn initialize(&mut self) -> Result<(), SupervisorError> {
        self.gateway.initialize()
    }
    pub fn launch_fixed_fixture(&mut self) -> Result<(), SupervisorError> {
        self.gateway
            .launch_fixed_fixture("/ls/debuggee/wo6-selftest")
    }
    pub fn phase(&self) -> lu_ide_workspace::dap_session::DapPhase {
        self.gateway.phase()
    }
    pub fn poll(
        &mut self,
    ) -> Result<Vec<lu_ide_workspace::dap_session::DapEvent>, SupervisorError> {
        self.gateway.poll()
    }
    pub fn configuration_needed(&self) -> bool {
        self.gateway.configuration_needed()
    }
    pub fn configuration_done(&mut self) -> Result<(), SupervisorError> {
        self.gateway.configuration_done()
    }
    pub fn threads(&mut self) -> Result<(), SupervisorError> {
        self.gateway.threads()
    }
    pub fn stack_trace(&mut self, thread_id: u64) -> Result<(), SupervisorError> {
        self.gateway.stack_trace(thread_id)
    }
    pub fn close(self) -> Result<(), SupervisorError> {
        self.gateway.close()
    }
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
    fn kernel_quota_witness_emits_only_bounded_boolean_or_unavailable() {
        let file =
            std::env::temp_dir().join(format!("wo6-kernel-limit-{:032x}", rand::random::<u128>()));
        std::fs::write(&file, "max 0\nother 12\noom_kill 1\n").unwrap();
        assert_eq!(kernel_limit_hit(&file, "max"), "no");
        assert_eq!(kernel_limit_hit(&file, "oom_kill"), "yes");
        assert_eq!(kernel_limit_hit(&file, "none"), "unavailable");
        std::fs::remove_file(&file).unwrap();
        assert_eq!(kernel_limit_hit(&file, "max"), "unavailable");
    }
    #[test]
    fn native_dap_failure_diagnostics_emit_only_fixed_categories() {
        assert_eq!(
            classify_dap_child_failure(
                b"/private/secret: error while loading shared libraries: libmissing.so"
            ),
            "dynamic_library_missing"
        );
        assert_eq!(
            classify_dap_child_failure(b"/private/secret: Permission denied"),
            "os_permission_denied"
        );
        assert_eq!(
            classify_dap_child_failure(b"child critical: no such file or directory"),
            "required_file_missing"
        );
        assert_eq!(classify_dap_child_failure(b""), "empty");
        assert_eq!(
            classify_dap_child_failure(b"secret arbitrary user text"),
            "other_nonempty"
        );
    }
    #[test]
    fn native_release_bundle_is_not_a_user_writable_project_binary() {
        let unavailable = Path::new("/tmp/nonexistent-unapproved-lsp-release");
        assert!(native_lsp_bundle_digest(unavailable, "lsp_clangd").is_err());
        assert!(native_lsp_bundle_digest(unavailable, "arbitrary_shell").is_err());
    }
}
