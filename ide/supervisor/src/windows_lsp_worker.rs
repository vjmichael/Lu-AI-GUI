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
        && std::env::var_os("LU_IDE_E2_NATIVE_TRACE").as_deref() == Some(OsStr::new("1"))
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
            &["--log=error", "--background-index=false", "-j=1"],
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
        #[cfg(feature = "dap-transport")]
        "dap_lldb_fixed" => "lldb-dap.exe",
        #[cfg(feature = "dap-transport")]
        "dap_lldb_project" => "lldb-dap.exe",
        _ => return Err(SupervisorError::InvalidPlan),
    };
    if !clean_meta(root)?.is_dir()
        || !clean_meta(&root.join("bin"))?.is_dir()
        || !clean_meta(&root.join("bin").join(plan_server))?.is_file()
    {
        return Err(denied());
    }
    #[cfg(feature = "dap-transport")]
    if matches!(recipe_id, "dap_lldb_fixed" | "dap_lldb_project")
        && !clean_meta(&root.join("debuggee/wo6-selftest.exe"))?.is_file()
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
    if !Path::new(&system).is_absolute() || !local.is_absolute() || !tools.is_absolute() {
        return Err(denied());
    }
    // Never forward the interactive host's LOCALAPPDATA or profile directory
    // to an AppContainer child. The sealed profile's LocalState is the only
    // writable user-data/cache/temp root intentionally exposed to this LSP.
    let env = format!(
        "APPDATA={0}\0HOME={0}\0LOCALAPPDATA={0}\0PATH={1};{2}\0SystemRoot={3}\0TEMP={0}\0TMP={0}\0USERPROFILE={0}\0\0",
        local.display(),
        tools.join("bin").display(),
        Path::new(&system).join("System32").display(),
        system.to_string_lossy()
    );
    if env.encode_utf16().count() > 32767 {
        return Err(denied());
    }
    Ok(env.encode_utf16().collect())
}
#[cfg(test)]
mod native_stderr_classification_tests {
    use super::*;
    #[test]
    fn native_exit_statuses_are_classified_without_paths_or_rights_changes() {
        assert_eq!(classify_native_exit(0xc0000135), "dependency_dll_missing");
        assert_eq!(
            classify_native_exit(0xc0000142),
            "dll_initialization_failed"
        );
        assert_eq!(classify_native_exit(0xc0000022), "kernel_access_denied");
        assert_eq!(classify_native_exit(0xc000007b), "invalid_binary_format");
        assert_eq!(classify_native_exit(1), "other_nonzero");
        assert_eq!(classify_native_exit(0), "zero");
    }
    #[test]
    fn diagnostic_classification_never_exposes_untrusted_message_bytes() {
        assert_eq!(
            classify_debug_stderr(b"error: unknown command line argument"),
            "invalid_option"
        );
        assert_eq!(
            classify_debug_stderr(b"Access is denied: C:\\secret.txt"),
            "permission_denied"
        );
        assert_eq!(
            classify_debug_stderr(b"error: custom user source path"),
            "reported_error"
        );
        assert_eq!(
            classify_debug_stderr(b"secret user document content"),
            "other_nonempty"
        );
        assert_eq!(
            classify_debug_denial_context(b"failed to open stdout: Permission denied"),
            "stdout"
        );
        assert_eq!(
            classify_debug_denial_context(b"failed to create temporary cache: Permission denied"),
            "cache_or_temp"
        );
    }
}

#[cfg(test)]
mod native_environment_tests {
    use super::*;
    #[test]
    fn native_lsp_profile_does_not_inherit_host_localappdata() {
        let local = Path::new(r"C:\LuIdeSandbox\LocalState");
        let tools = Path::new(r"C:\LuIdeSandbox\LanguageServer");
        let env = min_environment(local, tools).unwrap();
        let decoded = String::from_utf16(&env).unwrap();
        for key in [
            "APPDATA",
            "HOME",
            "LOCALAPPDATA",
            "TEMP",
            "TMP",
            "USERPROFILE",
        ] {
            assert!(
                decoded.contains(&format!("{key}={}\0", local.display())),
                "LSP profile environment did not use private sandbox"
            );
        }
        assert!(decoded.contains(&format!("PATH={};", tools.join("bin").display())));
        assert!(decoded.ends_with("\0\0"));
    }
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
// Fixed, nonidentifying Windows NTSTATUS classification for native CI only.
// Loader errors explain why a verified AppContainer may exit before any DAP/LSP
// bytes; none of these statuses authorize a weaker execution path.
fn classify_native_exit(code: u32) -> &'static str {
    match code {
        0 => "zero",
        0xc0000135 => "dependency_dll_missing",
        0xc0000142 => "dll_initialization_failed",
        0xc0000022 => "kernel_access_denied",
        0xc000007b => "invalid_binary_format",
        _ => "other_nonzero",
    }
}
// Safe fixed-purpose native fault evidence. The numeric process exit status
// is never used to grant access or declare containment; debug acceptance only.
fn native_lsp_exit_code(process: HANDLE) {
    if cfg!(debug_assertions)
        && std::env::var_os("LU_IDE_E2_NATIVE_TRACE").as_deref() == Some(OsStr::new("1"))
    {
        let mut code = 0u32;
        if unsafe { GetExitCodeProcess(process, &mut code) } != 0 {
            eprintln!("PROTECTED_LSP_NATIVE_EXIT_CODE={code}");
            eprintln!(
                "PROTECTED_LSP_NATIVE_EXIT_CLASS={}",
                classify_native_exit(code)
            );
        }
    }
}
// Debug-only stderr witness. The production worker still gives clangd the
// inherited NUL handle. Native acceptance captures at most 2048 bytes, emits
// ONLY a small whitelisted classification (never source text, paths or raw
// stderr), and cannot compile this capture into a release build.
fn native_lsp_debug_enabled() -> bool {
    cfg!(debug_assertions)
        && std::env::var_os("LU_IDE_E2_NATIVE_TRACE").as_deref() == Some(OsStr::new("1"))
}
fn classify_debug_stderr(bytes: &[u8]) -> &'static str {
    if bytes.is_empty() {
        return "empty";
    }
    let value = String::from_utf8_lossy(bytes).to_ascii_lowercase();
    if value.contains("unknown command line argument") || value.contains("unknown argument") {
        "invalid_option"
    } else if value.contains("permission denied")
        || value.contains("access is denied")
        || value.contains("access denied")
    {
        "permission_denied"
    } else if value.contains("no such file") || value.contains("not found") {
        "missing_file"
    } else if value.contains("invalid argument") || value.contains("invalid handle") {
        "invalid_handle_or_argument"
    } else if value.contains("error:") || value.contains("fatal:") {
        "reported_error"
    } else {
        "other_nonempty"
    }
}
// A second fixed enum helps separate filesystem from stdio/loader denial.
// It never emits raw stderr, arbitrary filenames or the denied absolute path.
// All classifications are evidence hints, NOT security authorization.
fn classify_debug_denial_context(bytes: &[u8]) -> &'static str {
    let text = String::from_utf8_lossy(bytes).to_ascii_lowercase();
    if text.contains("stdin") || text.contains("standard input") {
        "stdin"
    } else if text.contains("stdout") || text.contains("standard output") {
        "stdout"
    } else if text.contains("stderr") || text.contains("standard error") {
        "stderr"
    } else if text.contains("pipe") {
        "pipe"
    } else if text.contains("config") || text.contains(".clangd") {
        "config"
    } else if text.contains("cache") || text.contains("temp") {
        "cache_or_temp"
    } else if text.contains("resource") || text.contains("library") || text.contains("dll") {
        "resources_or_dependency"
    } else if text.contains("index") {
        "index"
    } else if text.contains("socket") || text.contains("network") {
        "network"
    } else if text.contains("workspace") || text.contains("main.c") {
        "workspace"
    } else if text.contains("open") || text.contains("read") {
        "open_or_read"
    } else if text.contains("create") || text.contains("write") {
        "create_or_write"
    } else if text.contains("path") || text.contains("directory") || text.contains("file") {
        "filesystem"
    } else {
        "unspecified"
    }
}

fn inspect_debug_stderr(handle: Option<&Handle>) {
    if !native_lsp_debug_enabled() {
        return;
    }
    let Some(handle) = handle else {
        eprintln!("PROTECTED_LSP_NATIVE_STDERR_CLASS=no_debug_pipe");
        return;
    };
    let mut available = 0u32;
    if unsafe {
        PeekNamedPipe(
            handle.0,
            ptr::null_mut(),
            0,
            ptr::null_mut(),
            &mut available,
            ptr::null_mut(),
        )
    } == 0
    {
        eprintln!("PROTECTED_LSP_NATIVE_STDERR_CLASS=pipe_unavailable");
        return;
    }
    if available == 0 {
        eprintln!("PROTECTED_LSP_NATIVE_STDERR_CLASS=empty");
        return;
    }
    let mut output = [0u8; 2048];
    let limit = available.min(output.len() as u32);
    let mut read = 0u32;
    if unsafe {
        ReadFile(
            handle.0,
            output.as_mut_ptr(),
            limit,
            &mut read,
            ptr::null_mut(),
        )
    } == 0
    {
        eprintln!("PROTECTED_LSP_NATIVE_STDERR_CLASS=read_failed");
        return;
    }
    eprintln!(
        "PROTECTED_LSP_NATIVE_STDERR_CLASS={}",
        classify_debug_stderr(&output[..read as usize])
    );
    eprintln!(
        "PROTECTED_LSP_NATIVE_DENIAL_CONTEXT={}",
        classify_debug_denial_context(&output[..read as usize])
    );
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
) -> Result<(Handle, Handle, Handle, Handle, Option<Handle>), SupervisorError> {
    let (child_stdin, host_stdin) = inheritable_pipe()?;
    let (host_stdout, child_stdout) = inheritable_pipe()?;
    uninheritable(host_stdin.0)?;
    uninheritable(host_stdout.0)?;
    // Only a CI/debug acceptance session uses an inherited diagnostic pipe.
    // The fixed OS handle allowlist remains exactly three child handles.
    // In release the child continues to inherit the NUL stderr handle.
    let (stderr, debug_stderr) = if native_lsp_debug_enabled() {
        let (host_read, child_write) = inheritable_pipe()?;
        uninheritable(host_read.0)?;
        (child_write, Some(host_read))
    } else {
        (nul_stderr()?, None)
    };
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
    Ok((process, thread, host_stdin, host_stdout, debug_stderr))
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
                        native_lsp_trace("writer_native_write_denied");
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
    debug_stderr: Option<Handle>,
    released: bool,
}
// Process and handle ownership are exclusive; no native HANDLE is shared
// concurrently and each pipe read/write occurs on one serialized worker actor.
unsafe impl Send for WindowsLspProcess {}
impl RestrictedLspProcess for WindowsLspProcess {
    #[cfg(feature = "dap-transport")]
    fn observe_windows_job_module(
        &mut self, expected_binary: &Path, expected_sha256: &str, pc: u64,
    ) -> Result<crate::windows_kernel_pc::WindowsLoadedModuleWitness, SupervisorError> {
        if !self.lease.alive()
            || unsafe { WaitForSingleObject(self.process.0, 0) } != WAIT_TIMEOUT
        {
            return Err(SupervisorError::NoContainment);
        }
        crate::windows_kernel_pc::observe_job_module(
            self.job.handle.0, expected_binary, expected_sha256, pc,
        )
    }
    fn trace_native_fault(&mut self, stage: &'static str) {
        native_lsp_trace(stage);
        native_lsp_exit_code(self.process.0);
        inspect_debug_stderr(self.debug_stderr.as_ref());
    }
    fn write_bounded(&mut self, bytes: &[u8]) -> io::Result<()> {
        if bytes.is_empty() || bytes.len() > MAX_LSP_STDIO_WRITE {
            native_lsp_trace("outbound_frame_denied");
            return Err(io::ErrorKind::InvalidInput.into());
        }
        if !self.lease.alive() {
            native_lsp_trace("scm_lease_lost_before_write");
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        match unsafe { WaitForSingleObject(self.process.0, 0) } {
            WAIT_TIMEOUT => {}
            WAIT_OBJECT_0 => {
                native_lsp_trace("child_exited_before_write");
                native_lsp_exit_code(self.process.0);
                inspect_debug_stderr(self.debug_stderr.as_ref());
                return Err(io::ErrorKind::BrokenPipe.into());
            }
            _ => {
                native_lsp_trace("process_wait_denied_before_write");
                return Err(io::ErrorKind::BrokenPipe.into());
            }
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
                native_lsp_trace("scm_lease_lost_before_read");
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
                native_lsp_trace("stdout_peek_failed");
                native_lsp_exit_code(self.process.0);
                inspect_debug_stderr(self.debug_stderr.as_ref());
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
                    native_lsp_trace("stdout_read_failed");
                    native_lsp_exit_code(self.process.0);
                    return Err(io::Error::last_os_error());
                }
                data.truncate(read as usize);
                return Ok(data);
            }
            if unsafe { WaitForSingleObject(self.process.0, 0) } == WAIT_OBJECT_0 {
                native_lsp_trace("child_exited_before_read");
                native_lsp_exit_code(self.process.0);
                inspect_debug_stderr(self.debug_stderr.as_ref());
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
    // Clangd and the Windows runtime can open the NUL device by name after
    // AppContainer launch, just as pinned Cargo/rustc does. Obtain an exact
    // package-SID NUL lease from the independently restricted SCM service
    // while the Job is empty. The service journals and verifies the grant,
    // then revokes it before the final durable release ACK. This is NOT an
    // interactive/user ACL edit, blanket AppContainer grant or launch bypass.
    // The LSP adapter is compiled only with protected-recovery-service.
    native_lsp_trace("scm_exact_nul_lease_requested");
    lease.grant_null()?;
    native_lsp_trace("scm_exact_nul_lease_fsynced");
    if let Some(target) = project_target {
        target.verify()?;
    }
    supervisor.consume_grant_after_attestation(job_id, plan, &proof())?;
    native_lsp_trace("grant_consumed");
    let (process, child_thread, stdin_write, stdout_read, debug_stderr) =
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
        debug_stderr,
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

#[cfg(feature = "dap-transport")]
pub fn start_windows_dap(
    supervisor: Arc<Supervisor>,
    job_id: &str,
    plan: &JobPlan,
    current_native_source_head: &str,
    input: &PublishedInput,
    trusted_release_bundle: &Path,
    trusted_service: &TrustedBroker,
) -> Result<WindowsDapSession, SupervisorError> {
    start_windows_dap_inner(
        supervisor,
        job_id,
        plan,
        current_native_source_head,
        input,
        trusted_release_bundle,
        trusted_service,
        None,
    )
}
#[cfg(feature = "dap-transport")]
pub fn start_windows_project_dap(
    supervisor: Arc<Supervisor>,
    job_id: &str,
    plan: &JobPlan,
    current_native_source_head: &str,
    input: &PublishedInput,
    trusted_release_bundle: &Path,
    trusted_service: &TrustedBroker,
    project_target: &crate::project_debug_target::SealedProjectDebugTarget,
) -> Result<WindowsDapSession, SupervisorError> {
    start_windows_dap_inner(
        supervisor,
        job_id,
        plan,
        current_native_source_head,
        input,
        trusted_release_bundle,
        trusted_service,
        Some(project_target),
    )
}
#[cfg(feature = "dap-transport")]
fn start_windows_dap_inner(
    supervisor: Arc<Supervisor>,
    job_id: &str,
    plan: &JobPlan,
    current_native_source_head: &str,
    input: &PublishedInput,
    trusted_release_bundle: &Path,
    trusted_service: &TrustedBroker,
    project_target: Option<&crate::project_debug_target::SealedProjectDebugTarget>,
) -> Result<WindowsDapSession, SupervisorError> {
    if plan.effect_class != "debug_inspection_isolated" {
        return Err(SupervisorError::InvalidPlan);
    }
    match (plan.recipe_id.as_str(), project_target) {
        ("dap_lldb_fixed", None) if plan.debug_target_binding_sha256.is_none() => {}
        ("dap_lldb_project", Some(target))
            if plan.debug_target_binding_sha256.as_deref()
                == Some(target.binding_sha256.as_str())
                && target.lineage.project_id == plan.project_id
                && target.lineage.source_head == plan.source_head
                && target.lineage.source_snapshot_sha256 == plan.input_snapshot_sha256
                && target.lineage.manifest_sha256 == plan.manifest_sha256 =>
        {
            target.verify()?;
        }
        _ => return Err(SupervisorError::InvalidPlan),
    }
    let executable_basename = "lldb-dap.exe";
    let arguments: &[&str] = &[];
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
    let project_debuggee = if let Some(target) = project_target {
        // Stage the previously independently audited binary/PDB into the
        // new AppContainer LocalState. No host output path is reachable
        // from the child, and no user-selected executable is trusted.
        target.verify()?;
        let name = std::ffi::OsStr::new("debug-target");
        publish_into_native_parent(
            &target.root,
            &local,
            name,
            8,
            128 * 1024 * 1024,
            64 * 1024 * 1024,
        )?;
        let stage_root = local.join("debug-target");
        let witness = crate::wo6_output_evidence::audit_published_output(&stage_root)?;
        let binary =
            crate::debug_binary_witness::audit_debug_binary_candidate(&stage_root, &witness)?
                .ok_or_else(denied)?;
        let symbols = crate::debug_symbol_verification::verify_build_debug_symbols(
            &stage_root,
            &witness,
            &binary,
        )?
        .ok_or_else(denied)?;
        if binary.binary_sha256 != target.binary.binary_sha256
            || symbols.symbols_sha256 != target.symbols.symbols_sha256
            || symbols.symbols_relative_path != target.symbols.symbols_relative_path
        {
            return Err(SupervisorError::WrongProjectOrSource);
        }
        Some(
            stage_root
                .join(target.confined_relative_binary())
                .to_string_lossy()
                .into_owned(),
        )
    } else {
        None
    };
    let staged_input = PublishedInput {
        root: workspace.clone(),
        seal: input.seal.clone(),
    };
    verify_input(&staged_input).map_err(|_| SupervisorError::WrongProjectOrSource)?;
    let staged_document = workspace.join(&input.seal.manifest_relative_path);
    if !clean_meta(&staged_document)?.is_file() {
        return Err(SupervisorError::WrongProjectOrSource);
    }
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
    // Clangd and the Windows runtime can open the NUL device by name after
    // AppContainer launch, just as pinned Cargo/rustc does. Obtain an exact
    // package-SID NUL lease from the independently restricted SCM service
    // while the Job is empty. The service journals and verifies the grant,
    // then revokes it before the final durable release ACK. This is NOT an
    // interactive/user ACL edit, blanket AppContainer grant or launch bypass.
    // The LSP adapter is compiled only with protected-recovery-service.
    native_lsp_trace("scm_exact_nul_lease_requested");
    lease.grant_null()?;
    native_lsp_trace("scm_exact_nul_lease_fsynced");
    supervisor.consume_grant_after_attestation(job_id, plan, &proof())?;
    native_lsp_trace("grant_consumed");
    let (process, child_thread, stdin_write, stdout_read, debug_stderr) =
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
        debug_stderr,
        released: false,
    };
    let gateway = crate::dap_transport::DapGateway::attach_native(
        Arc::clone(&supervisor),
        job_id,
        &plan_hash,
        &proof(),
        Box::new(actor),
    )?;
    native_lsp_trace("gateway_attached");
    Ok(WindowsDapSession {
        gateway,
        project_debuggee,
        native_job_id: job_id.to_owned(),
        native_plan_sha256: plan_hash,
        source_input: PublishedInput {
            root: input.root.clone(),
            seal: input.seal.clone(),
        },
        fixed_debuggee: bundle
            .join("debuggee/wo6-selftest.exe")
            .to_string_lossy()
            .into_owned(),
    })
}

#[cfg(feature = "dap-transport")]
pub struct WindowsDapSession {
    gateway: crate::dap_transport::DapGateway,
    fixed_debuggee: String,
    project_debuggee: Option<String>,
    native_job_id: String,
    native_plan_sha256: String,
    source_input: PublishedInput,
}
#[cfg(feature = "dap-transport")]
impl WindowsDapSession {
    pub fn initialize(&mut self) -> Result<(), SupervisorError> {
        self.gateway.initialize()
    }
    pub fn launch_fixed_fixture(&mut self) -> Result<(), SupervisorError> {
        self.gateway.launch_fixed_fixture(&self.fixed_debuggee)
    }
    pub fn launch_project_target(&mut self) -> Result<(), SupervisorError> {
        let target = self
            .project_debuggee
            .as_deref()
            .ok_or(SupervisorError::InvalidPlan)?;
        self.gateway.launch_verified_project(target)
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
    /// This does NOT validate a module/PC. Native offline symbol verification
    /// must independently map the returned instruction address.
    pub fn source_candidate(
        &self,
    ) -> Option<lu_ide_workspace::dap_source_lines::SourceLineCandidate> {
        self.gateway.source_candidate()
    }
    /// Observes only the OS loaded PE image in the REAL protected Job. A
    /// claimed DAP PC inside that module does NOT prove that a thread stopped
    /// there or that the PE/PDB maps the PC to the claimed source line.
    /// No renderer route and no debugger permission is minted.
    pub fn verify_project_loaded_module(
        &mut self,
        supervisor: &Supervisor,
        job_id: &str,
        plan: &JobPlan,
        target: &crate::project_debug_target::SealedProjectDebugTarget,
    ) -> Result<crate::windows_kernel_pc::WindowsProtectedModuleLineage, SupervisorError> {
        use crate::windows_kernel_pc::WindowsProtectedModuleLineage;
        if self.native_job_id != job_id
            || plan.recipe_id != "dap_lldb_project"
            || plan.debug_target_binding_sha256.as_deref()
                != Some(target.binding_sha256.as_str())
            || target.lineage.project_id != plan.project_id
            || target.lineage.source_head != plan.source_head
            || target.lineage.source_snapshot_sha256 != plan.input_snapshot_sha256
            || target.lineage.manifest_sha256 != plan.manifest_sha256
            || self.source_input.seal.source_sha256 != plan.input_snapshot_sha256
            || self.source_input.seal.manifest_sha256 != plan.manifest_sha256
        {
            return Err(SupervisorError::WrongProjectOrSource);
        }
        target.verify()?;
        verify_input(&self.source_input)
            .map_err(|_| SupervisorError::WrongProjectOrSource)?;
        let pending = supervisor.inspect()?.into_iter()
            .find(|job| job.job_id == job_id)
            .ok_or(SupervisorError::NoSuchJob)?;
        if pending.project_id != plan.project_id
            || pending.plan_sha256 != self.native_plan_sha256
            || pending.phase != JobPhase::Running
            || pending.requires_reconciliation {
            return Err(SupervisorError::InvalidTransition);
        }
        let debug = supervisor.receipt_history(job_id)?;
        if !debug.iter().any(|r| r.body.phase == JobPhase::Authorized
                && r.body.plan_sha256 == self.native_plan_sha256)
            || debug.last().is_none_or(|r| r.body.phase != JobPhase::Running
                || r.body.plan_sha256 != self.native_plan_sha256) {
            return Err(SupervisorError::InvalidTransition);
        }
        let build = supervisor.receipt_history(&target.lineage.build_job_id)?;
        if build.last().is_none_or(|r| {
            r.body.phase != JobPhase::Completed
                || r.entry_hash != target.lineage.terminal_receipt_sha256
                || r.body.plan_sha256 != target.lineage.build_plan_sha256
        }) {
            return Err(SupervisorError::InvalidTransition);
        }
        let source_claim = self.gateway.source_candidate()
            .ok_or(SupervisorError::NoContainment)?;
        let pc = source_claim.instruction_address
            .ok_or(SupervisorError::NoContainment)?;
        if source_claim.line == 0 { return Err(SupervisorError::NoContainment); }
        let stage = self.project_debuggee.as_deref()
            .ok_or(SupervisorError::InvalidPlan)?;
        let module = self.gateway.observe_windows_project_module(
            Path::new(stage), &target.binary.binary_sha256, pc,
        )?;
        if module.binary_sha256 != target.binary.binary_sha256
            || !module.job_member_verified
            || !module.process_exe_path_verified
            || !module.os_loaded_pe_image_verified
            || module.os_thread_context_ip_verified
            || module.independent_pdb_line_verified {
            return Err(SupervisorError::NoContainment);
        }
        target.verify()?;
        verify_input(&self.source_input)
            .map_err(|_| SupervisorError::WrongProjectOrSource)?;
        if !supervisor.inspect()?.into_iter().any(|job| {
            job.job_id == job_id && job.plan_sha256 == self.native_plan_sha256
                && job.phase == JobPhase::Running && !job.requires_reconciliation
        }) {
            return Err(SupervisorError::InvalidTransition);
        }
        Ok(WindowsProtectedModuleLineage {
            module,
            debugger_job_id: job_id.to_owned(),
            debugger_plan_sha256: self.native_plan_sha256.clone(),
            debug_running_receipt_sha256: debug.last().unwrap().entry_hash.clone(),
            build_job_id: target.lineage.build_job_id.clone(),
            build_terminal_receipt_sha256: target.lineage.terminal_receipt_sha256.clone(),
            source_snapshot_sha256: target.lineage.source_snapshot_sha256.clone(),
            target_binding_sha256: target.binding_sha256.clone(),
            dap_claimed_source_line: source_claim.line,
            source_snapshot_integrity_verified: true,
            source_line_mapping_verified: false,
            stopped_thread_os_verified: false,
            installed_app_accepted: false,
        })
    }
    pub fn close(self) -> Result<(), SupervisorError> {
        self.gateway.close()
    }
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
