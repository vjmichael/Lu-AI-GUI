//! E2B Windows: independently supervised, long-lived local LSP subprocess.
//! This is a CHILD of windows_worker to reuse its exclusive AppContainer,
//! handle-rooted workspace staging, Job quotas and D7D protected SCM lease.
//! Enabled ONLY by lsp-transport + protected-recovery-service. Not registered
//! with Tauri/WebView; caller must independently authenticate the selected
//! native project and approved release-pinned executable.
use super::*;
use crate::{
    lsp_transport::{LspGateway, RestrictedLspProcess},
    snapshot::PublishedInput,
};
use lu_ide_workspace::{
    lsp::DiagnosticSource,
    lsp_session::{LspEvent, LspPhase, Position},
};
use std::{ffi::OsStr, io, sync::Arc};
use windows_sys::Win32::{
    Foundation::{INVALID_HANDLE_VALUE, SetHandleInformation},
    Security::SECURITY_ATTRIBUTES,
    Storage::FileSystem::{CreateFileW, ReadFile, WriteFile, OPEN_EXISTING},
    System::{
        Pipes::{CreatePipe, PeekNamedPipe},
        Threading::STARTF_USESTDHANDLES,
    },
};
const MAX_LSP_BUNDLE_FILES: usize = 500;
const MAX_LSP_BUNDLE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_LSP_FILE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_LSP_STDIO_WRITE: usize = 512 * 1024 + 256;
const HANDLE_FLAG_INHERIT_VALUE: u32 = 1;
const PROC_THREAD_ATTRIBUTE_HANDLE_LIST_VALUE: usize = 0x20002;
const NUL_GENERIC_WRITE: u32 = 0x4000_0000;
const FILE_SHARE_READ_VALUE: u32 = 1;
const FILE_SHARE_WRITE_VALUE: u32 = 2;

fn fixed_server(plan: &JobPlan) -> Result<(&'static str, &'static str, &'static [&'static str]), SupervisorError> {
    if plan.effect_class != "language_analysis_isolated" { return Err(denied()); }
    match plan.recipe_id.as_str() {
        "lsp_rust_analyzer" => Ok(("rust-analyzer", "rust-analyzer.exe", &[])),
        "lsp_clangd" => Ok(("clangd", "clangd.exe", &["--log=error", "--background-index=false"])),
        "lsp_ruff" => Ok(("ruff", "ruff.exe", &["server"])),
        _ => Err(SupervisorError::InvalidPlan),
    }
}
/// The native host must compare this exact tree hash against an independently
/// reviewed installer pin; measuring a user-chosen folder is not approval.
pub fn native_lsp_bundle_digest(root: &Path, recipe_id: &str)
    -> Result<String, SupervisorError> {
    let plan_server = match recipe_id {
        "lsp_rust_analyzer" => "rust-analyzer.exe",
        "lsp_clangd" => "clangd.exe",
        "lsp_ruff" => "ruff.exe",
        _ => return Err(SupervisorError::InvalidPlan),
    };
    if !clean_meta(root)?.is_dir() ||
        !clean_meta(&root.join("bin"))?.is_dir() ||
        !clean_meta(&root.join("bin").join(plan_server))?.is_file() {
        return Err(denied());
    }
    digest_tree(root, None, MAX_LSP_BUNDLE_FILES, MAX_LSP_BUNDLE_BYTES)
}
fn stage_bundle(root: &Path, local: &Path, recipe_id: &str, expected: &str)
    -> Result<(), SupervisorError> {
    if native_lsp_bundle_digest(root, recipe_id)? != expected {
        return Err(SupervisorError::WrongProjectOrSource);
    }
    let parent = local.parent().ok_or_else(denied)?;
    let name = local.file_name().ok_or_else(denied)?;
    let (_, hash) = publish_into_native_parent(
        root, parent, name, MAX_LSP_BUNDLE_FILES, MAX_LSP_BUNDLE_BYTES,
        MAX_LSP_FILE_BYTES,
    )?;
    if hash != expected || native_lsp_bundle_digest(local, recipe_id)? != expected {
        return Err(SupervisorError::WrongProjectOrSource);
    }
    Ok(())
}
fn min_environment(local: &Path, tools: &Path) -> Result<Vec<u16>, SupervisorError> {
    let system = std::env::var_os("SystemRoot").ok_or_else(denied)?;
    let appdata = std::env::var_os("LOCALAPPDATA").ok_or_else(denied)?;
    if !Path::new(&system).is_absolute() || !Path::new(&appdata).is_absolute() {
        return Err(denied());
    }
    let env = format!("LOCALAPPDATA={}\0PATH={};{}\0SystemRoot={}\0TEMP={}\0TMP={}\0\0",
        appdata.to_string_lossy(),
        tools.join("bin").display(), Path::new(&system).join("System32").display(),
        system.to_string_lossy(), local.display(), local.display());
    if env.encode_utf16().count() > 32767 { return Err(denied()); }
    Ok(env.encode_utf16().collect())
}
fn inheritable_pipe() -> Result<(Handle, Handle), SupervisorError> {
    let mut security = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: ptr::null_mut(),
        bInheritHandle: 1,
    };
    let mut read: HANDLE = ptr::null_mut();
    let mut write: HANDLE = ptr::null_mut();
    if unsafe { CreatePipe(&mut read, &mut write, &mut security, 0) } == 0 {
        return Err(denied());
    }
    let reader = Handle::new(read)?;
    let writer = Handle::new(write)?;
    Ok((reader, writer))
}
fn uninheritable(raw: HANDLE) -> Result<(), SupervisorError> {
    if unsafe { SetHandleInformation(raw, HANDLE_FLAG_INHERIT_VALUE, 0) } == 0 {
        Err(denied())
    } else { Ok(()) }
}
fn nul_stderr() -> Result<Handle, SupervisorError> {
    let mut attrs = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: ptr::null_mut(),
        bInheritHandle: 1,
    };
    let raw = unsafe { CreateFileW(
        wide(OsStr::new("NUL")).as_ptr(), NUL_GENERIC_WRITE,
        FILE_SHARE_READ_VALUE | FILE_SHARE_WRITE_VALUE, &mut attrs,
        OPEN_EXISTING, 0, ptr::null_mut(),
    ) };
    if raw == INVALID_HANDLE_VALUE || raw.is_null() { return Err(denied()); }
    Handle::new(raw)
}
fn spawn_contained_stdio(
    profile: &AppProfile, job: &Job, executable: &Path,
    command: &mut Vec<u16>, cwd: &Path, env: &[u16],
) -> Result<(Handle, Handle, Handle, Handle), SupervisorError> {
    let (child_stdin, host_stdin) = inheritable_pipe()?;
    let (host_stdout, child_stdout) = inheritable_pipe()?;
    uninheritable(host_stdin.0)?;
    uninheritable(host_stdout.0)?;
    let stderr = nul_stderr()?;
    let mut capabilities = SECURITY_CAPABILITIES::default();
    capabilities.AppContainerSid = profile.sid;
    capabilities.Capabilities = ptr::null_mut();
    capabilities.CapabilityCount = 0;
    let mut attributes = Attributes::with_count(3)?;
    if unsafe { UpdateProcThreadAttribute(attributes.raw(), 0,
        PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES as usize,
        (&capabilities as *const SECURITY_CAPABILITIES).cast(),
        size_of::<SECURITY_CAPABILITIES>(), ptr::null_mut(), ptr::null()) } == 0 {
        return Err(denied());
    }
    let jobs = [job.handle.0];
    if unsafe { UpdateProcThreadAttribute(attributes.raw(), 0,
        PROC_THREAD_ATTRIBUTE_JOB_LIST_VALUE, jobs.as_ptr().cast(),
        size_of::<HANDLE>(), ptr::null_mut(), ptr::null()) } == 0 {
        return Err(denied());
    }
    let child_handles = [child_stdin.0, child_stdout.0, stderr.0];
    if unsafe { UpdateProcThreadAttribute(attributes.raw(), 0,
        PROC_THREAD_ATTRIBUTE_HANDLE_LIST_VALUE, child_handles.as_ptr().cast(),
        size_of::<[HANDLE;3]>(), ptr::null_mut(), ptr::null()) } == 0 {
        return Err(denied());
    }
    let mut startup = STARTUPINFOEXW::default();
    startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
    startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    startup.StartupInfo.hStdInput = child_stdin.0;
    startup.StartupInfo.hStdOutput = child_stdout.0;
    startup.StartupInfo.hStdError = stderr.0;
    startup.lpAttributeList = attributes.raw();
    let exe_wide = wide(executable.as_os_str());
    let cwd_wide = wide(cwd.as_os_str());
    let mut child = PROCESS_INFORMATION::default();
    // InheritHandles=TRUE is safe here ONLY because the kernel handle list
    // restricts inheritance to the three explicitly selected pipe ends.
    if unsafe { CreateProcessW(
        exe_wide.as_ptr(), command.as_mut_ptr(),
        ptr::null(), ptr::null(), 1,
        CREATE_NO_WINDOW | CREATE_SUSPENDED | CREATE_UNICODE_ENVIRONMENT |
            EXTENDED_STARTUPINFO_PRESENT,
        env.as_ptr().cast(), cwd_wide.as_ptr(),
        &startup.StartupInfo, &mut child,
    ) } == 0 { return Err(denied()); }
    let process = Handle::new(child.hProcess)?;
    let thread = Handle::new(child.hThread)?;
    // Child already belongs to Job on return from CreateProcess.
    if !matches!(job.empty(), Ok(false)) ||
        verify_suspended_appcontainer_token(process.0, profile.sid).is_err() {
        if !matches!(job.kill_and_verify(), Ok(true)) {
            return Err(SupervisorError::Quarantined);
        }
        return Err(denied());
    }
    // child_stdin/stdout/stderr handles close here in the trusted host;
    // parent holds only stdin-write and stdout-read.
    Ok((process, thread, host_stdin, host_stdout))
}

struct WindowsLspProcess {
    _profile: AppProfile,
    job: Job,
    lease: BrokerLease,
    process: Handle,
    _thread: Handle,
    stdin_write: Handle,
    stdout_read: Handle,
    released: bool,
}
// Process and handle ownership are exclusive; no native HANDLE is shared
// concurrently and each pipe read/write occurs on one serialized worker actor.
unsafe impl Send for WindowsLspProcess {}
impl RestrictedLspProcess for WindowsLspProcess {
    fn write_bounded(&mut self, bytes: &[u8]) -> io::Result<()> {
        if bytes.is_empty() || bytes.len() > MAX_LSP_STDIO_WRITE || !self.lease.alive()
            || unsafe { WaitForSingleObject(self.process.0, 0) } != WAIT_TIMEOUT {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "untrusted LSP stopped"));
        }
        // Limit individual synchronous writes to reduce pipe pressure. The
        // supervisor can terminate the OS Job if the writer stalls; avoid
        // feeding a growing, unbounded queue to a slow/malicious process.
        for part in bytes.chunks(4096) {
            let mut written = 0u32;
            if unsafe { WriteFile(self.stdin_write.0, part.as_ptr(),
                part.len() as u32, &mut written, ptr::null_mut()) } == 0
                || written as usize != part.len() {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(())
    }
    fn read_bounded(&mut self, limit: usize, timeout: Duration) -> io::Result<Vec<u8>> {
        if limit == 0 || limit > 16 * 1024 { return Err(io::ErrorKind::InvalidInput.into()); }
        let began = Instant::now();
        while began.elapsed() < timeout {
            if !self.lease.alive() { return Err(io::ErrorKind::BrokenPipe.into()); }
            let mut available = 0u32;
            if unsafe { PeekNamedPipe(self.stdout_read.0, ptr::null_mut(), 0,
                ptr::null_mut(), &mut available, ptr::null_mut()) } == 0 {
                return Err(io::Error::last_os_error());
            }
            if available > 0 {
                let mut data = vec![0u8; available.min(limit as u32) as usize];
                let mut read = 0u32;
                if unsafe { ReadFile(self.stdout_read.0, data.as_mut_ptr(),
                    data.len() as u32, &mut read, ptr::null_mut()) } == 0 {
                    return Err(io::Error::last_os_error());
                }
                data.truncate(read as usize);
                return Ok(data);
            }
            if unsafe { WaitForSingleObject(self.process.0, 0) } == WAIT_OBJECT_0 {
                return Err(io::ErrorKind::BrokenPipe.into());
            }
            thread::sleep(Duration::from_millis(20));
        }
        Ok(Vec::new()) // timeout is not a protocol success, just no bytes
    }
    fn terminate_tree_and_verify(&mut self) -> Result<bool, SupervisorError> {
        self.job.kill_and_verify()
    }
    fn release_after_verified_terminal(&mut self) -> Result<(), SupervisorError> {
        if !self.job.empty()? || !self.lease.alive() { return Err(denied()); }
        self.lease.release_empty()?;
        self.released = true;
        Ok(())
    }
}
impl Drop for WindowsLspProcess {
    fn drop(&mut self) {
        if !self.released { let _ = self.job.kill_and_verify(); }
    }
}

/// Caller owns: (a) registered project and exact native source snapshot,
/// (b) human one-use job grant, (c) fixed approved installed bundle digest,
/// (d) restricted D7D service deployment. NO user-supplied executable path.
pub(crate) fn start_windows_lsp(
    supervisor: Arc<Supervisor>, job_id: &str, plan: &JobPlan,
    current_native_source_head: &str, input: &PublishedInput,
    trusted_release_bundle: &Path, trusted_service: &TrustedBroker,
) -> Result<WindowsLspSession, SupervisorError> {
    let (server_id, executable_basename, arguments) = fixed_server(plan)?;
    if !valid_job_id(job_id) || plan.source_head != current_native_source_head
        || plan.input_snapshot_sha256 != input.seal.source_sha256
        || plan.manifest_sha256 != input.seal.manifest_sha256 {
        return Err(SupervisorError::WrongProjectOrSource);
    }
    verify_input(input).map_err(|_| SupervisorError::WrongProjectOrSource)?;
    if native_lsp_bundle_digest(trusted_release_bundle, &plan.recipe_id)? != plan.toolchain_sha256 {
        return Err(SupervisorError::WrongProjectOrSource);
    }
    let profile = AppProfile::create(job_id)?;
    let local = profile.local_state()?;
    let workspace = local.join("workspace");
    let bundle = local.join("language-server");
    stage_workspace(input, &workspace)?;
    stage_bundle(trusted_release_bundle, &bundle, &plan.recipe_id, &plan.toolchain_sha256)?;
    let staged_input = PublishedInput {root:workspace.clone(),seal:input.seal.clone()};
    verify_input(&staged_input).map_err(|_| SupervisorError::WrongProjectOrSource)?;
    let executable = bundle.join("bin").join(executable_basename);
    if !clean_meta(&executable)?.is_file() { return Err(denied()); }
    let env = min_environment(&local, &bundle)?;
    let command_text = format!("\"{}\"{}", executable.display(),
        arguments.iter().map(|arg| format!(" {arg}")).collect::<String>());
    if command_text.chars().any(char::is_control) || command_text.len() > 4096 {
        return Err(denied());
    }
    let mut command = wstr(&command_text);
    let plan_hash = supervisor.inspect()?.into_iter().find(|x| x.job_id == job_id)
        .ok_or(SupervisorError::NoSuchJob)?.plan_sha256;
    let job = Job::create(job_id, &plan_hash, plan)?;
    if !proof().complete() { return Err(denied()); }
    let mut lease = BrokerLease::begin(job.handle.0, job_id, &plan_hash, trusted_service)?;
    supervisor.consume_grant_after_attestation(job_id, plan, &proof())?;
    let (process, child_thread, stdin_write, stdout_read) = match spawn_contained_stdio(
        &profile, &job, &executable, &mut command, &workspace, &env
    ) {
        Ok(value) => value,
        Err(_) => {
            if matches!(job.empty(), Ok(true)) {
                supervisor.native_deny_before_start(job_id, true)?;
                return Err(denied());
            }
            let _ = supervisor.native_quarantine_after_error(job_id);
            return Err(SupervisorError::Quarantined);
        }
    };
    if !lease.alive() {
        let _ = job.kill_and_verify();
        let _ = supervisor.native_quarantine_after_error(job_id);
        return Err(SupervisorError::Quarantined);
    }
    if let Err(reason) = supervisor.native_mark_running(job_id) {
        let _ = job.kill_and_verify();
        let _ = supervisor.native_quarantine_after_error(job_id);
        return Err(reason);
    }
    if unsafe { ResumeThread(child_thread.0) } == u32::MAX {
        let _ = job.kill_and_verify();
        let _ = supervisor.native_quarantine_after_error(job_id);
        return Err(SupervisorError::Quarantined);
    }
    let actor = WindowsLspProcess {
        _profile: profile, job, lease, process, _thread: child_thread,
        stdin_write, stdout_read, released: false,
    };
    let gateway = LspGateway::attach_native(
        Arc::clone(&supervisor), job_id, &plan_hash, &proof(),
        Box::new(actor), server_id,
    )?;
    Ok(WindowsLspSession { gateway })
}

pub(crate) struct WindowsLspSession { gateway: LspGateway }
impl WindowsLspSession {
    pub fn initialize(&mut self) -> Result<(), SupervisorError> {
        self.gateway.initialize()
    }
    pub fn open(&mut self, source: DiagnosticSource, trusted_uri: &str, text: &str)
        -> Result<(), SupervisorError> {
        self.gateway.open(source, trusted_uri, text)
    }
    pub fn changed(&mut self, current: &DiagnosticSource, next: DiagnosticSource, text: &str)
        -> Result<(), SupervisorError> {
        self.gateway.changed(current, next, text)
    }
    pub fn query(&mut self, kind: &str, source: &DiagnosticSource, pos: Position)
        -> Result<(), SupervisorError> {
        self.gateway.query(kind, source, pos)
    }
    pub fn poll(&mut self) -> Result<Vec<LspEvent>, SupervisorError> {
        self.gateway.poll()
    }
    pub fn close(self) -> Result<(), SupervisorError> { self.gateway.close() }
    pub fn is_ready(&self) -> bool { self.gateway.phase() == LspPhase::Ready }
}
