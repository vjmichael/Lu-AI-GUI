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
use std::{
    ffi::OsStr,
    io,
    sync::{
        mpsc::{self, Receiver, SyncSender},
        Arc,
    },
    thread::JoinHandle,
};
use windows_sys::Win32::{
    Foundation::{SetHandleInformation, INVALID_HANDLE_VALUE},
    Security::SECURITY_ATTRIBUTES,
    Storage::FileSystem::{CreateFileW, ReadFile, WriteFile, OPEN_EXISTING},
    System::{
        Pipes::{CreatePipe, PeekNamedPipe},
        Threading::STARTF_USESTDHANDLES,
    },
};
// CI/debug-only fixed stage identifiers. Never print tokens, paths, SIDs,
// source text or application-level LSP messages. No release logging fallback.
fn native_lsp_trace(stage: &'static str) {
    if cfg!(debug_assertions)
        && std::env::var_os("LU_IDE_E2_NATIVE_TRACE").as_deref()
            == Some(OsStr::new("1"))
    {
        eprintln!("PROTECTED_LSP_ADMISSION_STAGE={stage}");
    }
}

const MAX_LSP_BUNDLE_FILES: usize = 500;
const MAX_LSP_BUNDLE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_LSP_FILE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_LSP_STDIO_WRITE: usize = 512 * 1024 + 256;
const HANDLE_FLAG_INHERIT_VALUE: u32 = 1;
const PROC_THREAD_ATTRIBUTE_HANDLE_LIST_VALUE: usize = 0x20002;
const NUL_GENERIC_WRITE: u32 = 0x4000_0000;
const FILE_SHARE_READ_VALUE: u32 = 1;
const FILE_SHARE_WRITE_VALUE: u32 = 2;

fn fixed_server(
    plan: &JobPlan,
) -> Result<(&'static str, &'static str, &'static [&'static str]), SupervisorError> {
    if plan.effect_class != "language_analysis_isolated" {
        return Err(denied());
    }
    match plan.recipe_id.as_str() {
        "lsp_rust_analyzer" => Ok(("rust-analyzer", "rust-analyzer.exe", &[])),
        "lsp_clangd" => Ok((
            "clangd",
            "clangd.exe",
            &["--log=error", "--background-index=false"],
        )),
        "lsp_ruff" => Ok(("ruff", "ruff.exe", &["server"])),
        _ => Err(SupervisorError::InvalidPlan),
    }
}
/// The native host must compare this exact tree hash against an independently
/// reviewed installer pin; measuring a user-chosen folder is not approval.
pub fn native_lsp_bundle_digest(root: &Path, recipe_id: &str) -> Result<String, SupervisorError> {
    let plan_server = match recipe_id {
        "lsp_rust_analyzer" => "rust-analyzer.exe",
        "lsp_clangd" => "clangd.exe",
        "lsp_ruff" => "ruff.exe",
        _ => return Err(SupervisorError::InvalidPlan),
    };
    if !clean_meta(root)?.is_dir()
        || !clean_meta(&root.join("bin"))?.is_dir()
        || !clean_meta(&root.join("bin").join(plan_server))?.is_file()
    {
        return Err(denied());
    }
    digest_tree(root, None, MAX_LSP_BUNDLE_FILES, MAX_LSP_BUNDLE_BYTES)
}
fn stage_bundle(
    root: &Path,
    local: &Path,
    recipe_id: &str,
    expected: &str,
) -> Result<(), SupervisorError> {
    if native_lsp_bundle_digest(root, recipe_id)? != expected {
        return Err(SupervisorError::WrongProjectOrSource);
    }
    let parent = local.parent().ok_or_else(denied)?;
    let name = local.file_name().ok_or_else(denied)?;
    let (_, hash) = publish_into_native_parent(
        root,
        parent,
        name,
        MAX_LSP_BUNDLE_FILES,
        MAX_LSP_BUNDLE_BYTES,
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
    let env = format!(
        "LOCALAPPDATA={}\0PATH={};{}\0SystemRoot={}\0TEMP={}\0TMP={}\0\0",
        appdata.to_string_lossy(),
        tools.join("bin").display(),
        Path::new(&system).join("System32").display(),
        system.to_string_lossy(),
        local.display(),
        local.display()
    );
    if env.encode_utf16().count() > 32767 {
        return Err(denied());
    }
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
    } else {
        Ok(())
    }
}
fn nul_stderr() -> Result<Handle, SupervisorError> {
    let mut attrs = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: ptr::null_mut(),
        bInheritHandle: 1,
    };
    let raw = unsafe {
        CreateFileW(
            wide(OsStr::new("NUL")).as_ptr(),
            NUL_GENERIC_WRITE,
            FILE_SHARE_READ_VALUE | FILE_SHARE_WRITE_VALUE,
            &mut attrs,
            OPEN_EXISTING,
            0,
            ptr::null_mut(),
        )
    };
    if raw == INVALID_HANDLE_VALUE || raw.is_null() {
        return Err(denied());
    }
    Handle::new(raw)
}
fn spawn_contained_stdio(
    profile: &AppProfile,
    job: &Job,
    executable: &Path,
    command: &mut Vec<u16>,
    cwd: &Path,
    env: &[u16],
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
    if unsafe {
        UpdateProcThreadAttribute(
            attributes.raw(),
            0,
            PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES as usize,
            (&capabilities as *const SECURITY_CAPABILITIES).cast(),
            size_of::<SECURITY_CAPABILITIES>(),
            ptr::null_mut(),
            ptr::null(),
        )
    } == 0
    {
        return Err(denied());
    }
    let jobs = [job.handle.0];
    if unsafe {
        UpdateProcThreadAttribute(
            attributes.raw(),
            0,
            PROC_THREAD_ATTRIBUTE_JOB_LIST_VALUE,
            jobs.as_ptr().cast(),
            size_of::<HANDLE>(),
            ptr::null_mut(),
            ptr::null(),
        )
    } == 0
    {
        return Err(denied());
    }
    let child_handles = [child_stdin.0, child_stdout.0, stderr.0];
    if unsafe {
        UpdateProcThreadAttribute(
            attributes.raw(),
            0,
            PROC_THREAD_ATTRIBUTE_HANDLE_LIST_VALUE,
            child_handles.as_ptr().cast(),
            size_of::<[HANDLE; 3]>(),
            ptr::null_mut(),
            ptr::null(),
        )
    } == 0
    {
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
    if unsafe {
        CreateProcessW(
            exe_wide.as_ptr(),
            command.as_mut_ptr(),
            ptr::null(),
            ptr::null(),
            1,
            CREATE_NO_WINDOW
                | CREATE_SUSPENDED
                | CREATE_UNICODE_ENVIRONMENT
                | EXTENDED_STARTUPINFO_PRESENT,
            env.as_ptr().cast(),
            cwd_wide.as_ptr(),
            &startup.StartupInfo,
            &mut child,
        )
    } == 0
    {
        return Err(denied());
    }
    let process = Handle::new(child.hProcess)?;
    let thread = Handle::new(child.hThread)?;
    // Child already belongs to Job on return from CreateProcess.
    if !matches!(job.empty(), Ok(false))
        || verify_suspended_appcontainer_token(process.0, profile.sid).is_err()
    {
        if !matches!(job.kill_and_verify(), Ok(true)) {
            return Err(SupervisorError::Quarantined);
        }
        return Err(denied());
    }
    // child_stdin/stdout/stderr handles close here in the trusted host;
    // parent holds only stdin-write and stdout-read.
    Ok((process, thread, host_stdin, host_stdout))
}

struct BoundedWrite {
    bytes: Vec<u8>,
    ack: mpsc::Sender<io::Result<()>>,
}
/// The child may stop reading at any moment. Never call synchronous WriteFile
/// on the UI/worker actor itself; a dedicated bounded-channel writer owns the
/// only host write handle. If it stalls, return within three seconds and kill
/// the underlying Job Object before any further request is accepted.
// A kernel pipe handle is process-wide, but the single writer thread
// receives exclusive ownership. Do not make generic HANDLE wrappers Send.
struct ThreadOwnedStdin(Handle);
// SAFETY: the owned handle moves once to a single writer thread; it is
// never concurrently accessed elsewhere, and its Drop closes that handle.
unsafe impl Send for ThreadOwnedStdin {}
impl ThreadOwnedStdin {
    fn raw(&self) -> HANDLE {
        self.0 .0
    }
}

fn start_writer(
    stdin_write: Handle,
) -> Result<(SyncSender<BoundedWrite>, JoinHandle<()>), SupervisorError> {
    let (tx, rx): (SyncSender<BoundedWrite>, Receiver<BoundedWrite>) = mpsc::sync_channel(1);
    let write_pipe = ThreadOwnedStdin(stdin_write);
    let thread = thread::Builder::new()
        .name("lu-e2-lsp-stdin".into())
        .spawn(move || {
            for request in rx {
                let mut result = Ok(());
                for part in request.bytes.chunks(4096) {
                    let mut written = 0u32;
                    if unsafe {
                        WriteFile(
                            write_pipe.raw(),
                            part.as_ptr(),
                            part.len() as u32,
                            &mut written,
                            ptr::null_mut(),
                        )
                    } == 0
                        || written as usize != part.len()
                    {
                        result = Err(io::Error::last_os_error());
                        break;
                    }
                }
                let failed = result.is_err();
                let _ = request.ack.send(result);
                if failed {
                    break;
                }
            }
        })
        .map_err(|_| denied())?;
    Ok((tx, thread))
}

struct WindowsLspProcess {
    _profile: AppProfile,
    // Lease must drop BEFORE the borrowed Job HANDLE it uses in its Drop.
    lease: BrokerLease,
    job: Job,
    process: Handle,
    _thread: Handle,
    write_queue: SyncSender<BoundedWrite>,
    writer_thread: Option<JoinHandle<()>>,
    stdout_read: Handle,
    released: bool,
}
// Process and handle ownership are exclusive; no native HANDLE is shared
// concurrently and each pipe read/write occurs on one serialized worker actor.
unsafe impl Send for WindowsLspProcess {}
impl RestrictedLspProcess for WindowsLspProcess {
    fn write_bounded(&mut self, bytes: &[u8]) -> io::Result<()> {
        if bytes.is_empty()
            || bytes.len() > MAX_LSP_STDIO_WRITE
            || !self.lease.alive()
            || unsafe { WaitForSingleObject(self.process.0, 0) } != WAIT_TIMEOUT
        {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "untrusted LSP stopped",
            ));
        }
        let (tx, rx) = mpsc::channel();
        self.write_queue
            .try_send(BoundedWrite {
                bytes: bytes.to_vec(),
                ack: tx,
            })
            .map_err(|_| io::ErrorKind::WouldBlock)?;
        rx.recv_timeout(Duration::from_secs(3))
            .map_err(|_| io::ErrorKind::TimedOut)?
    }
    fn read_bounded(&mut self, limit: usize, timeout: Duration) -> io::Result<Vec<u8>> {
        if limit == 0 || limit > 16 * 1024 {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let began = Instant::now();
        while began.elapsed() < timeout {
            if !self.lease.alive() {
                return Err(io::ErrorKind::BrokenPipe.into());
            }
            let mut available = 0u32;
            if unsafe {
                PeekNamedPipe(
                    self.stdout_read.0,
                    ptr::null_mut(),
                    0,
                    ptr::null_mut(),
                    &mut available,
                    ptr::null_mut(),
                )
            } == 0
            {
                return Err(io::Error::last_os_error());
            }
            if available > 0 {
                let mut data = vec![0u8; available.min(limit as u32) as usize];
                let mut read = 0u32;
                if unsafe {
                    ReadFile(
                        self.stdout_read.0,
                        data.as_mut_ptr(),
                        data.len() as u32,
                        &mut read,
                        ptr::null_mut(),
                    )
                } == 0
                {
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
        if !self.job.empty()? || !self.lease.alive() {
            return Err(denied());
        }
        self.lease.release_empty()?;
        self.released = true;
        Ok(())
    }
}
impl Drop for WindowsLspProcess {
    fn drop(&mut self) {
        if !self.released {
            let _ = self.job.kill_and_verify();
        }
        // Dropping the queue closes the writer's input channel; the Job was
        // already killed (if necessary) so any blocked WriteFile unblocks.
        // Never join an OS thread indefinitely from an IDE shutdown path.
        self.writer_thread.take();
    }
}

/// Caller owns: (a) registered project and exact native source snapshot,
/// (b) human one-use job grant, (c) fixed approved installed bundle digest,
/// (d) restricted D7D service deployment. NO user-supplied executable path.
pub fn start_windows_lsp(
    supervisor: Arc<Supervisor>,
    job_id: &str,
    plan: &JobPlan,
    current_native_source_head: &str,
    input: &PublishedInput,
    trusted_release_bundle: &Path,
    trusted_service: &TrustedBroker,
) -> Result<WindowsLspSession, SupervisorError> {
    let (server_id, executable_basename, arguments) = fixed_server(plan)?;
    if !valid_job_id(job_id)
        || plan.source_head != current_native_source_head
        || plan.input_snapshot_sha256 != input.seal.source_sha256
        || plan.manifest_sha256 != input.seal.manifest_sha256
    {
        return Err(SupervisorError::WrongProjectOrSource);
    }
    verify_input(input).map_err(|_| SupervisorError::WrongProjectOrSource)?;
    if native_lsp_bundle_digest(trusted_release_bundle, &plan.recipe_id)? != plan.toolchain_sha256 {
        return Err(SupervisorError::WrongProjectOrSource);
    }
    native_lsp_trace("profile_create_start");
    let profile = AppProfile::create(job_id)?;
    native_lsp_trace("profile_created");
    let local = profile.local_state()?;
    let workspace = local.join("workspace");
    let bundle = local.join("language-server");
    stage_workspace(input, &workspace)?;
    native_lsp_trace("workspace_staged");
    native_lsp_trace("bundle_stage_start");
    stage_bundle(
        trusted_release_bundle,
        &bundle,
        &plan.recipe_id,
        &plan.toolchain_sha256,
    )?;
    native_lsp_trace("bundle_staged");
    let staged_input = PublishedInput {
        root: workspace.clone(),
        seal: input.seal.clone(),
    };
    verify_input(&staged_input).map_err(|_| SupervisorError::WrongProjectOrSource)?;
    let staged_document = workspace.join(&input.seal.manifest_relative_path);
    if !clean_meta(&staged_document)?.is_file() {
        return Err(SupervisorError::WrongProjectOrSource);
    }
    let native_uri = crate::lsp_transport::trusted_staged_file_uri(&staged_document)?;
    let executable = bundle.join("bin").join(executable_basename);
    if !clean_meta(&executable)?.is_file() {
        return Err(denied());
    }
    let env = min_environment(&local, &bundle)?;
    let command_text = format!(
        "\"{}\"{}",
        executable.display(),
        arguments
            .iter()
            .map(|arg| format!(" {arg}"))
            .collect::<String>()
    );
    if command_text.chars().any(char::is_control) || command_text.len() > 4096 {
        return Err(denied());
    }
    let mut command = wstr(&command_text);
    let plan_hash = supervisor
        .inspect()?
        .into_iter()
        .find(|x| x.job_id == job_id)
        .ok_or(SupervisorError::NoSuchJob)?
        .plan_sha256;
    let job = Job::create(job_id, &plan_hash, plan)?;
    native_lsp_trace("job_created_empty");
    if !proof().complete() {
        return Err(denied());
    }
    native_lsp_trace("scm_lease_start");
    let mut lease = BrokerLease::begin(job.handle.0, job_id, &plan_hash, trusted_service)?;
    native_lsp_trace("scm_lease_fsynced");
    supervisor.consume_grant_after_attestation(job_id, plan, &proof())?;
    native_lsp_trace("grant_consumed");
    let (process, child_thread, stdin_write, stdout_read) =
        match spawn_contained_stdio(&profile, &job, &executable, &mut command, &workspace, &env) {
            Ok(value) => {
                native_lsp_trace("suspended_child_attested");
                value
            }
            Err(_) => {
                native_lsp_trace("suspended_launch_denied");
                if matches!(job.empty(), Ok(true)) {
                    supervisor.native_deny_before_start(job_id, true)?;
                    return Err(denied());
                }
                native_lsp_trace("suspended_launch_job_not_empty");
                let _ = supervisor.native_quarantine_after_error(job_id);
                return Err(SupervisorError::Quarantined);
            }
        };
    if !lease.alive() {
        native_lsp_trace("scm_lease_lost_before_resume");
        let _ = job.kill_and_verify();
        let _ = supervisor.native_quarantine_after_error(job_id);
        return Err(SupervisorError::Quarantined);
    }
    if let Err(reason) = supervisor.native_mark_running(job_id) {
        native_lsp_trace("running_receipt_denied");
        let _ = job.kill_and_verify();
        let _ = supervisor.native_quarantine_after_error(job_id);
        return Err(reason);
    }
    native_lsp_trace("running_receipt_fsynced");
    if unsafe { ResumeThread(child_thread.0) } == u32::MAX {
        native_lsp_trace("resume_denied");
        let _ = job.kill_and_verify();
        let _ = supervisor.native_quarantine_after_error(job_id);
        return Err(SupervisorError::Quarantined);
    }
    // Pipe writer is separate from the supervisor event actor. A malicious
    // server cannot indefinitely block the desktop via synchronous WriteFile.
    native_lsp_trace("child_resumed");
    let (write_queue, writer_thread) = match start_writer(stdin_write) {
        Ok(value) => value,
        Err(reason) => {
            let _ = job.kill_and_verify();
            let _ = supervisor.native_quarantine_after_error(job_id);
            return Err(reason);
        }
    };
    native_lsp_trace("writer_ready");
    let actor = WindowsLspProcess {
        _profile: profile,
        lease,
        job,
        process,
        _thread: child_thread,
        write_queue,
        writer_thread: Some(writer_thread),
        stdout_read,
        released: false,
    };
    let gateway = LspGateway::attach_native(
        Arc::clone(&supervisor),
        job_id,
        &plan_hash,
        &proof(),
        Box::new(actor),
        server_id,
    )?;
    native_lsp_trace("gateway_attached");
    Ok(WindowsLspSession {
        gateway,
        native_uri,
    })
}

pub struct WindowsLspSession {
    gateway: LspGateway,
    native_uri: String,
}
impl WindowsLspSession {
    pub fn native_document_uri(&self) -> &str {
        &self.native_uri
    }
    pub fn initialize(&mut self) -> Result<(), SupervisorError> {
        self.gateway.initialize()
    }
    pub fn open(
        &mut self,
        source: DiagnosticSource,
        trusted_uri: &str,
        text: &str,
    ) -> Result<(), SupervisorError> {
        self.gateway.open(source, trusted_uri, text)
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
