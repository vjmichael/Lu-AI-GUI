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
    env: &mut Vec<u16>,
    owner_event_channel: bool,
) -> Result<
    (
        Handle,
        Handle,
        Handle,
        Handle,
        Option<Handle>,
        Option<(Handle, [u8; 16])>,
    ),
    SupervisorError,
> {
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
    // The fourth handle is *only* a test-only reviewed LLDB event channel.
    // Normal LSP, ordinary DAP and all release processes inherit exactly 3.
    let event_pipe = if owner_event_channel
        && cfg!(all(debug_assertions, feature = "dev045d5-ci-event-owner"))
    {
        let (host_read, child_write) = inheritable_pipe()?;
        uninheritable(host_read.0)?;
        let nonce = rand::random::<u128>().to_le_bytes();
        let nonce_hex: String = nonce.iter().map(|v| format!("{v:02x}")).collect();
        let extra = format!(
            "LU_IDE_OWNER_PIPE_HANDLE={:x}\0LU_IDE_OWNER_NONCE={}\0\0",
            child_write.0 as usize, nonce_hex,
        );
        if env.len() < 2 || env[env.len() - 1] != 0 || env[env.len() - 2] != 0 {
            return Err(denied());
        }
        env.pop();
        env.extend(extra.encode_utf16());
        if env.len() > 32767 {
            return Err(denied());
        }
        Some((host_read, child_write, nonce))
    } else {
        None
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
    let mut child_handles = vec![child_stdin.0, child_stdout.0, stderr.0];
    if let Some((_, event_write, _)) = event_pipe.as_ref() {
        child_handles.push(event_write.0);
    }
    if !matches!(child_handles.len(), 3 | 4) || (child_handles.len() == 4 && !owner_event_channel) {
        return Err(denied());
    }
    if unsafe {
        UpdateProcThreadAttribute(
            attributes.raw(),
            0,
            PROC_THREAD_ATTRIBUTE_HANDLE_LIST_VALUE,
            child_handles.as_ptr().cast(),
            child_handles.len() * size_of::<HANDLE>(),
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
    // restricts inheritance to 3 normal or 4 explicit CI-only pipe ends.
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
    // Only the native host retains the owner-event read end. The extra
    // write end closes here and cannot leak through normal launch paths.
    let owner_read = event_pipe.map(|(host_read, _write, nonce)| (host_read, nonce));
    Ok((
        process,
        thread,
        host_stdin,
        host_stdout,
        debug_stderr,
        owner_read,
    ))
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
    #[cfg(all(debug_assertions, feature = "dev045d5-ci-event-owner"))]
    event_sideband: Option<crate::windows_event_owner::OwnerSideband>,
    released: bool,
}
// Process and handle ownership are exclusive; no native HANDLE is shared
// concurrently and each pipe read/write occurs on one serialized worker actor.
unsafe impl Send for WindowsLspProcess {}
impl RestrictedLspProcess for WindowsLspProcess {
    #[cfg(all(debug_assertions, feature = "dev045d5-ci-event-owner"))]
    fn wait_owner_continued_for_ci(&mut self, seq: u64) -> Result<(), SupervisorError> {
        if !self.lease.alive() {
            return Err(SupervisorError::NoContainment);
        }
        self.event_sideband
            .as_mut()
            .ok_or(SupervisorError::NoContainment)?
            .wait_exact_native_continue(seq)
    }
    #[cfg(all(debug_assertions, feature = "dev045d5-ci-event-owner"))]
    fn crash_owned_lldb_for_ci(&mut self) -> Result<(), SupervisorError> {
        if !self.lease.alive() || unsafe { WaitForSingleObject(self.process.0, 0) } != WAIT_TIMEOUT
        {
            return Err(SupervisorError::NoContainment);
        }
        if unsafe { TerminateProcess(self.process.0, 127) } == 0
            || unsafe { WaitForSingleObject(self.process.0, 3_000) } != WAIT_OBJECT_0
        {
            return Err(SupervisorError::NoContainment);
        }
        // A surviving debuggee must NOT retain our private attestation
        // writer. Native kernel pipe EOF is mandatory after LLDB dies;
        // timeout means an inherited handle leaked or a worker survived.
        self.event_sideband
            .as_mut()
            .ok_or(SupervisorError::NoContainment)?
            .expect_pipe_eof_after_lldb_crash()?;
        Ok(())
    }
    #[cfg(all(debug_assertions, feature = "dev045d5-ci-event-owner"))]
    fn verify_lldb_kernel_event_owner(
        &mut self,
        pid: u32,
        tid: u32,
        rip: u64,
    ) -> Result<crate::windows_event_owner::OwnerEvent, SupervisorError> {
        if !self.lease.alive()
            || unsafe { WaitForSingleObject(self.process.0, 0) } != WAIT_TIMEOUT
            || !crate::windows_kernel_pc::job_has_member(self.job.handle.0, pid)?
        {
            return Err(SupervisorError::NoContainment);
        }
        let event = self
            .event_sideband
            .as_mut()
            .ok_or(SupervisorError::NoContainment)?
            .current(pid, tid, rip)?;
        crate::windows_kernel_pc::verify_lldb_event_owner_thread(self.process.0, event.owner_tid)?;
        Ok(event)
    }
    #[cfg(feature = "dap-transport")]
    fn corroborate_windows_debug_port(
        &mut self,
        module: &crate::windows_kernel_pc::WindowsLoadedModuleWitness,
    ) -> Result<crate::windows_kernel_pc::WindowsDebugPortCorroboration, SupervisorError> {
        if !self.lease.alive() || unsafe { WaitForSingleObject(self.process.0, 0) } != WAIT_TIMEOUT
        {
            return Err(SupervisorError::NoContainment);
        }
        crate::windows_kernel_pc::verify_debug_port_and_owned_parent(
            self.job.handle.0,
            self.process.0,
            module,
        )
    }
    #[cfg(feature = "dap-transport")]
    fn sample_windows_job_thread_rip(
        &mut self,
        module: &crate::windows_kernel_pc::WindowsLoadedModuleWitness,
    ) -> Result<crate::windows_kernel_pc::WindowsThreadRipSample, SupervisorError> {
        if !self.lease.alive() || unsafe { WaitForSingleObject(self.process.0, 0) } != WAIT_TIMEOUT
        {
            return Err(SupervisorError::NoContainment);
        }
        crate::windows_kernel_pc::sample_job_thread_rip(self.job.handle.0, module)
    }
    #[cfg(feature = "dap-transport")]
    fn observe_windows_job_module(
        &mut self,
        expected_binary: &Path,
        expected_sha256: &str,
        pc: u64,
    ) -> Result<crate::windows_kernel_pc::WindowsLoadedModuleWitness, SupervisorError> {
        if !self.lease.alive() || unsafe { WaitForSingleObject(self.process.0, 0) } != WAIT_TIMEOUT
        {
            return Err(SupervisorError::NoContainment);
        }
        crate::windows_kernel_pc::observe_job_module(
            self.job.handle.0,
            expected_binary,
            expected_sha256,
            pc,
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
    let mut env = min_environment(&local, &bundle)?;
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
    // This is the ordinary LSP worker; it has no project-debug target.
    // The distinct start_windows_dap_inner validates its optional sealed
    // project target before launching LLDB. Never borrow debugger authority.
    supervisor.consume_grant_after_attestation(job_id, plan, &proof())?;
    native_lsp_trace("grant_consumed");
    let (process, child_thread, stdin_write, stdout_read, debug_stderr, _event_pipe) =
        match spawn_contained_stdio(
            &profile,
            &job,
            &executable,
            &mut command,
            &workspace,
            &mut env,
            false,
        ) {
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
        #[cfg(all(debug_assertions, feature = "dev045d5-ci-event-owner"))]
        event_sideband: None,
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
    let mut env = min_environment(&local, &bundle)?;
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
    let (process, child_thread, stdin_write, stdout_read, debug_stderr, owner_pipe) =
        match spawn_contained_stdio(
            &profile,
            &job,
            &executable,
            &mut command,
            &workspace,
            &mut env,
            plan.recipe_id == "dap_lldb_project"
                && cfg!(all(debug_assertions, feature = "dev045d5-ci-event-owner")),
        ) {
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
    #[cfg(all(debug_assertions, feature = "dev045d5-ci-event-owner"))]
    let event_sideband = if let Some((reader, nonce)) = owner_pipe {
        let raw = reader.0 as usize;
        // The bounded native reader exclusively owns this kernel handle
        // from now on; it closes it on EOF, invalid wire or queue pressure.
        std::mem::forget(reader);
        match crate::windows_event_owner::start_owner_pipe_reader(raw, nonce) {
            Ok(reader) => Some(reader),
            Err(_) => {
                let _ = job.kill_and_verify();
                let _ = supervisor.native_quarantine_after_error(job_id);
                return Err(SupervisorError::Quarantined);
            }
        }
    } else {
        None
    };
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
        #[cfg(all(debug_assertions, feature = "dev045d5-ci-event-owner"))]
        event_sideband,
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
        #[cfg(all(debug_assertions, feature = "dev045d4c-ci-oracle"))]
        private_reviewed_bundle: bundle.clone(),
        fixed_debuggee: bundle
            .join("debuggee/wo6-selftest.exe")
            .to_string_lossy()
            .into_owned(),
    })
}

/// Native CI receipt; no Serialize and no public desktop route.
#[cfg(all(
    feature = "dap-transport",
    debug_assertions,
    feature = "dev045d4c-ci-oracle"
))]
#[derive(Debug)]
pub struct WindowsProjectStopPdbEvidence {
    pub debugger_job_id: String,
    pub debugger_plan_sha256: String,
    pub debug_running_receipt_sha256: String,
    pub build_job_id: String,
    pub build_terminal_receipt_sha256: String,
    pub source_snapshot_sha256: String,
    pub target_binding_sha256: String,
    pub source_line: u32,
    pub source_file_sha256: String,
    pub observed_rip: u64,
    pub loaded_pe_base: u64,
    pub os_owned_thread_context_matches_lldb: bool,
    pub independent_pdb_source_line_verified: bool,
    pub kernel_debug_port_present: bool,
    pub debuggee_parent_is_native_lldb: bool,
    pub lldb_debug_event_owner_attested: bool,
    pub debugger_commands_authorized: bool,
    pub installed_app_accepted: bool,
}
/// Native-only evidence from the *actual* pinned LLVM debugger owner
/// event loop, a separate private inherited pipe, the Job OS witness,
/// original PE/PDB source line and durable one-use receipts. This is not a
/// new permission to set a breakpoint, step, evaluate or inspect variables.
#[cfg(all(debug_assertions, feature = "dev045d5-ci-event-owner"))]
#[derive(Debug, Clone)]
pub struct WindowsOwnerEventAccepted {
    pub debugger_job_id: String,
    pub debugger_plan_sha256: String,
    pub build_terminal_receipt_sha256: String,
    pub debug_running_receipt_sha256: String,
    pub native_owner_receipt_sha256: String,
    pub source_snapshot_sha256: String,
    pub owner_sequence: u64,
    pub debuggee_pid: u32,
    pub debuggee_thread_id: u32,
    pub lldb_owner_thread_id: u32,
    pub stopped_rip: u64,
    pub verified_source_line: u32,
    pub lldb_event_loop_owner_attested: bool,
    pub protected_pe_pdb_verified: bool,
    pub durable_receipt_verified: bool,
    pub debugger_commands_authorized: bool,
    pub installed_app_accepted: bool,
}
#[cfg(feature = "dap-transport")]
pub struct WindowsDapSession {
    gateway: crate::dap_transport::DapGateway,
    fixed_debuggee: String,
    project_debuggee: Option<String>,
    native_job_id: String,
    native_plan_sha256: String,
    source_input: PublishedInput,
    #[cfg(all(debug_assertions, feature = "dev045d4c-ci-oracle"))]
    private_reviewed_bundle: PathBuf,
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
            || plan.debug_target_binding_sha256.as_deref() != Some(target.binding_sha256.as_str())
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
        verify_input(&self.source_input).map_err(|_| SupervisorError::WrongProjectOrSource)?;
        let pending = supervisor
            .inspect()?
            .into_iter()
            .find(|job| job.job_id == job_id)
            .ok_or(SupervisorError::NoSuchJob)?;
        if pending.project_id != plan.project_id
            || pending.plan_sha256 != self.native_plan_sha256
            || pending.phase != JobPhase::Running
            || pending.requires_reconciliation
        {
            return Err(SupervisorError::InvalidTransition);
        }
        let debug = supervisor.receipt_history(job_id)?;
        if !debug.iter().any(|r| {
            r.body.phase == JobPhase::Authorized && r.body.plan_sha256 == self.native_plan_sha256
        }) || debug.last().is_none_or(|r| {
            r.body.phase != JobPhase::Running || r.body.plan_sha256 != self.native_plan_sha256
        }) {
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
        let source_claim = self
            .gateway
            .source_candidate()
            .ok_or(SupervisorError::NoContainment)?;
        let pc = source_claim
            .instruction_address
            .ok_or(SupervisorError::NoContainment)?;
        if source_claim.line == 0 {
            return Err(SupervisorError::NoContainment);
        }
        let stage = self
            .project_debuggee
            .as_deref()
            .ok_or(SupervisorError::InvalidPlan)?;
        let module = self.gateway.observe_windows_project_module(
            Path::new(stage),
            &target.binary.binary_sha256,
            pc,
        )?;
        if module.binary_sha256 != target.binary.binary_sha256
            || !module.job_member_verified
            || !module.process_exe_path_verified
            || !module.os_loaded_pe_image_verified
            || module.os_thread_context_ip_verified
            || module.independent_pdb_line_verified
        {
            return Err(SupervisorError::NoContainment);
        }
        target.verify()?;
        verify_input(&self.source_input).map_err(|_| SupervisorError::WrongProjectOrSource)?;
        if !supervisor.inspect()?.into_iter().any(|job| {
            job.job_id == job_id
                && job.plan_sha256 == self.native_plan_sha256
                && job.phase == JobPhase::Running
                && !job.requires_reconciliation
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
    /// Independent GetThreadContext samples are useful but not equivalent
    /// to a Windows DEBUG_EVENT owned by LLDB. Both stronger gates remain
    /// false; this never authorizes a DAP mutation or debugger command.
    pub fn sample_stopped_project_thread_for_ci(
        &mut self,
        supervisor: &Supervisor,
        job_id: &str,
        plan: &JobPlan,
        target: &crate::project_debug_target::SealedProjectDebugTarget,
    ) -> Result<crate::windows_kernel_pc::WindowsThreadRipSample, SupervisorError> {
        let witness = self.verify_project_loaded_module(supervisor, job_id, plan, target)?;
        if witness.source_line_mapping_verified
            || witness.stopped_thread_os_verified
            || witness.module.os_thread_context_ip_verified
            || witness.module.independent_pdb_line_verified
            || witness.installed_app_accepted
        {
            return Err(SupervisorError::NoContainment);
        }
        self.gateway
            .sample_windows_stopped_thread_rip(&witness.module)
    }
    /// DEV-04.5D4C: the real previously one-use-authorized project LLDB
    /// DAP stop must match an OS Job-owned x64 RIP and independently decoded
    /// PE/PDB line. Test-only and never a breakpoint/step/variable right.
    #[cfg(all(debug_assertions, feature = "dev045d4c-ci-oracle"))]
    pub fn verify_project_os_thread_pdb_receipts_for_ci(
        &mut self,
        supervisor: &Supervisor,
        job_id: &str,
        plan: &JobPlan,
        target: &crate::project_debug_target::SealedProjectDebugTarget,
    ) -> Result<WindowsProjectStopPdbEvidence, SupervisorError> {
        use crate::windows_stopped_line_oracle::independently_verify_pdb_line_for_ci;
        let native = self.verify_project_loaded_module(supervisor, job_id, plan, target)?;
        let origin = self
            .gateway
            .verify_windows_native_debug_port(&native.module)?;
        if !origin.kernel_reports_debugger_attached
            || !origin.debuggee_parent_is_owned_lldb
            || !origin.job_membership_verified
            || origin.actual_debug_event_owner_attested
            || origin.debugger_control_authorized
        {
            return Err(SupervisorError::NoContainment);
        }
        let thread = self
            .gateway
            .sample_windows_stopped_thread_rip(&native.module)?;
        let frame = self
            .gateway
            .source_candidate()
            .ok_or(SupervisorError::NoContainment)?;
        if self.gateway.phase() != lu_ide_workspace::dap_session::DapPhase::Stopped
            || frame.instruction_address != Some(thread.observed_rip)
            || !thread.two_matching_samples
            || !thread.job_membership_rechecked
            || thread.kernel_debug_event_independently_verified
            || thread.debugger_control_authorized
        {
            return Err(SupervisorError::NoContainment);
        }
        let pdb = independently_verify_pdb_line_for_ci(
            &self.private_reviewed_bundle,
            &plan.toolchain_sha256,
            target,
            &self.source_input.root,
            &native.module,
            &thread,
            &frame,
        )?;
        // The native symbolizer is a separate CI worker. A stop can become
        // stale while it runs. Query the REAL Job-owned kernel thread again
        // and consume pending DAP input before accepting the receipt.
        let after_origin = self
            .gateway
            .verify_windows_native_debug_port(&native.module)?;
        if after_origin.debugger_pid != origin.debugger_pid
            || after_origin.debuggee_pid != origin.debuggee_pid
            || !after_origin.kernel_reports_debugger_attached
            || !after_origin.debuggee_parent_is_owned_lldb
            || after_origin.actual_debug_event_owner_attested
        {
            return Err(SupervisorError::NoContainment);
        }
        let second = self
            .gateway
            .sample_windows_stopped_thread_rip(&native.module)?;
        if second.process_id != thread.process_id
            || second.thread_id != thread.thread_id
            || second.observed_rip != thread.observed_rip
            || !second.two_matching_samples
            || !second.job_membership_rechecked
        {
            return Err(SupervisorError::NoContainment);
        }
        if !self.gateway.poll()?.is_empty() {
            return Err(SupervisorError::NoContainment);
        }
        let current = self
            .gateway
            .source_candidate()
            .ok_or(SupervisorError::NoContainment)?;
        if self.gateway.phase() != lu_ide_workspace::dap_session::DapPhase::Stopped
            || current != frame
            || !pdb.independent_pe_pdb_source_line_verified
            || !pdb.os_thread_rip_matches_lldb_stack
            || !pdb.os_loaded_pe_mapping_verified
            || pdb.lldb_owned_kernel_debug_event_attested
            || pdb.debugger_commands_authorized
            || pdb.installed_app_accepted
        {
            return Err(SupervisorError::NoContainment);
        }
        // Re-open BOTH original fsynced journal chains after the external
        // symbol lookup. No old authorization can be reused.
        target.verify()?;
        verify_input(&self.source_input).map_err(|_| SupervisorError::WrongProjectOrSource)?;
        let live = supervisor
            .inspect()?
            .into_iter()
            .find(|x| x.job_id == job_id)
            .ok_or(SupervisorError::NoSuchJob)?;
        if live.phase != JobPhase::Running
            || live.requires_reconciliation
            || live.plan_sha256 != self.native_plan_sha256
            || live.project_id != plan.project_id
            || native.debugger_job_id != job_id
            || native.debugger_plan_sha256 != self.native_plan_sha256
            || native.build_terminal_receipt_sha256 != target.lineage.terminal_receipt_sha256
            || native.target_binding_sha256 != target.binding_sha256
            || native.source_snapshot_sha256 != plan.input_snapshot_sha256
            || pdb.binary_sha256 != target.binary.binary_sha256
            || pdb.pdb_sha256 != target.symbols.symbols_sha256
            || pdb.oracle_bundle_sha256 != plan.toolchain_sha256
        {
            return Err(SupervisorError::InvalidTransition);
        }
        let journal = supervisor.receipt_history(job_id)?;
        if journal.last().is_none_or(|r| {
            r.body.phase != JobPhase::Running
                || r.body.plan_sha256 != self.native_plan_sha256
                || r.entry_hash != native.debug_running_receipt_sha256
        }) || !journal.iter().any(|r| {
            r.body.phase == JobPhase::Authorized && r.body.plan_sha256 == self.native_plan_sha256
        }) {
            return Err(SupervisorError::InvalidTransition);
        }
        let build = supervisor.receipt_history(&native.build_job_id)?;
        if build.last().is_none_or(|r| {
            r.body.phase != JobPhase::Completed
                || r.entry_hash != native.build_terminal_receipt_sha256
                || r.body.plan_sha256 != target.lineage.build_plan_sha256
        }) {
            return Err(SupervisorError::InvalidTransition);
        }
        Ok(WindowsProjectStopPdbEvidence {
            debugger_job_id: job_id.into(),
            debugger_plan_sha256: self.native_plan_sha256.clone(),
            debug_running_receipt_sha256: native.debug_running_receipt_sha256,
            build_job_id: native.build_job_id,
            build_terminal_receipt_sha256: native.build_terminal_receipt_sha256,
            source_snapshot_sha256: native.source_snapshot_sha256,
            target_binding_sha256: native.target_binding_sha256,
            source_line: pdb.line,
            source_file_sha256: pdb.source_sha256,
            observed_rip: thread.observed_rip,
            loaded_pe_base: native.module.loaded_base,
            os_owned_thread_context_matches_lldb: true,
            independent_pdb_source_line_verified: true,
            kernel_debug_port_present: origin.kernel_reports_debugger_attached,
            debuggee_parent_is_native_lldb: origin.debuggee_parent_is_owned_lldb,
            // Only LLDB itself owns the kernel DEBUG_EVENT. Without an
            // independently authenticated owner callback this remains false.
            lldb_debug_event_owner_attested: false,
            debugger_commands_authorized: false,
            installed_app_accepted: false,
        })
    }
    /// D5 native acceptance: the exact Windows kernel event, received and
    /// still owned on the original debugger thread, is proven independently
    /// of the LLDB-DAP JSON and bound to both fsynced supervisor journals.
    #[cfg(all(debug_assertions, feature = "dev045d5-ci-event-owner"))]
    pub fn verify_real_lldb_event_owner_for_ci(
        &mut self,
        supervisor: &Supervisor,
        job_id: &str,
        plan: &JobPlan,
        target: &crate::project_debug_target::SealedProjectDebugTarget,
    ) -> Result<WindowsOwnerEventAccepted, SupervisorError> {
        use crate::windows_event_owner::persist_native_owner_receipt;
        // First independently verify the actual protected Cargo->LLDB
        // Job-owned OS RIP, compiler-generated PE/PDB line and source bytes.
        let line =
            self.verify_project_os_thread_pdb_receipts_for_ci(supervisor, job_id, plan, target)?;
        if !line.os_owned_thread_context_matches_lldb
            || !line.independent_pdb_source_line_verified
            || !line.kernel_debug_port_present
            || !line.debuggee_parent_is_native_lldb
            || line.lldb_debug_event_owner_attested
            || line.debugger_commands_authorized
            || line.installed_app_accepted
        {
            return Err(SupervisorError::NoContainment);
        }
        // The DAP frame isn't allowed to specify a PID or event owner.
        // Recover actual Job-owned process and unique OS thread separately.
        let target_module = self.verify_project_loaded_module(supervisor, job_id, plan, target)?;
        let sample = self
            .gateway
            .sample_windows_stopped_thread_rip(&target_module.module)?;
        if !sample.two_matching_samples
            || !sample.job_membership_rechecked
            || sample.observed_rip != line.observed_rip
        {
            return Err(SupervisorError::NoContainment);
        }
        let owner = self.gateway.verify_lldb_kernel_event_owner(
            sample.process_id,
            sample.thread_id,
            sample.observed_rip,
        )?;
        if owner.kind != 1
            || !matches!(owner.exception_code, 0x8000_0003 | 0x8000_0004)
            || owner.first_chance != 1
            || owner.pid != sample.process_id
            || owner.tid != sample.thread_id
            || owner.rip != sample.observed_rip
            || owner.exception_address < target_module.module.loaded_base
            || owner.exception_address
                >= target_module
                    .module
                    .loaded_base
                    .checked_add(target_module.module.loaded_size)
                    .ok_or(SupervisorError::NoContainment)?
        {
            return Err(SupervisorError::NoContainment);
        }
        let root = supervisor.native_owner_receipt_root()?;
        let hash = persist_native_owner_receipt(
            &root,
            job_id,
            &self.native_plan_sha256,
            &line.debug_running_receipt_sha256,
            &owner,
        )?;
        // An event CONTINUED while the receipt was being synced invalidates
        // the stop. The native event loop sends CONTINUED on the SAME pipe
        // before the job can be marked ready for any future debugger action.
        let still = self.gateway.verify_lldb_kernel_event_owner(
            sample.process_id,
            sample.thread_id,
            sample.observed_rip,
        )?;
        if still != owner
            || !self.gateway.poll()?.is_empty()
            || self.gateway.phase() != lu_ide_workspace::dap_session::DapPhase::Stopped
            || self
                .gateway
                .source_candidate()
                .and_then(|f| f.instruction_address)
                != Some(sample.observed_rip)
        {
            return Err(SupervisorError::NoContainment);
        }
        target.verify()?;
        verify_input(&self.source_input).map_err(|_| SupervisorError::WrongProjectOrSource)?;
        let debug = supervisor.receipt_history(job_id)?;
        if debug.last().is_none_or(|r| {
            r.body.phase != JobPhase::Running
                || r.body.plan_sha256 != self.native_plan_sha256
                || r.entry_hash != line.debug_running_receipt_sha256
        }) || !debug.iter().any(|r| {
            r.body.phase == JobPhase::Authorized && r.body.plan_sha256 == self.native_plan_sha256
        }) {
            return Err(SupervisorError::InvalidTransition);
        }
        let build = supervisor.receipt_history(&target.lineage.build_job_id)?;
        if build.last().is_none_or(|r| {
            r.body.phase != JobPhase::Completed
                || r.entry_hash != line.build_terminal_receipt_sha256
                || r.body.plan_sha256 != target.lineage.build_plan_sha256
        }) {
            return Err(SupervisorError::InvalidTransition);
        }
        Ok(WindowsOwnerEventAccepted {
            debugger_job_id: job_id.to_owned(),
            debugger_plan_sha256: self.native_plan_sha256.clone(),
            build_terminal_receipt_sha256: line.build_terminal_receipt_sha256,
            debug_running_receipt_sha256: line.debug_running_receipt_sha256,
            native_owner_receipt_sha256: hash,
            source_snapshot_sha256: line.source_snapshot_sha256,
            owner_sequence: owner.seq,
            debuggee_pid: owner.pid,
            debuggee_thread_id: owner.tid,
            lldb_owner_thread_id: owner.owner_tid,
            stopped_rip: owner.rip,
            verified_source_line: line.source_line,
            lldb_event_loop_owner_attested: true,
            protected_pe_pdb_verified: true,
            durable_receipt_verified: true,
            debugger_commands_authorized: false,
            installed_app_accepted: false,
        })
    }
    /// DEV-04.6A native CI integration: review proposals are derived from
    /// the actual D5 kernel-owner receipt and must remain non-executable.
    /// Only this existing native worker can see the authenticated sideband.
    #[cfg(all(debug_assertions, feature = "dev045d5-ci-event-owner"))]
    pub fn verify_bounded_debug_reviews_for_ci(
        &mut self,
        supervisor: &Supervisor,
        owner: &WindowsOwnerEventAccepted,
        snapshot_sha256: &str,
    ) -> Result<(String, String, String, String), SupervisorError> {
        use crate::debug_action_receipts::{
            ApprovalDenial, DebugActionApproval, NativeConsentMarker,
        };
        use crate::debug_command_review::{
            review_from_verified_native_owner_for_ci, DebugReviewDenial, DebugReviewIntent,
            StepKind,
        };
        if self.gateway.phase() != lu_ide_workspace::dap_session::DapPhase::Stopped
            || self
                .gateway
                .source_candidate()
                .and_then(|frame| frame.instruction_address)
                != Some(owner.stopped_rip)
        {
            return Err(SupervisorError::NoContainment);
        }
        let live_event = self.gateway.verify_lldb_kernel_event_owner(
            owner.debuggee_pid,
            owner.debuggee_thread_id,
            owner.stopped_rip,
        )?;
        if live_event.seq != owner.owner_sequence {
            return Err(SupervisorError::NoContainment);
        }
        let mut review = review_from_verified_native_owner_for_ci(owner, snapshot_sha256)
            .map_err(|_| SupervisorError::NoContainment)?;
        let breakpoint = review
            .preview(
                DebugReviewIntent::Breakpoint {
                    source: "src/main.rs".into(),
                    line: owner.verified_source_line,
                },
                owner.owner_sequence,
            )
            .map_err(|_| SupervisorError::NoContainment)?;
        if breakpoint.executable_authority
            || breakpoint.installed_app_accepted
            || review.has_executable_authority()
        {
            return Err(SupervisorError::NoContainment);
        }
        review
            .decline(&breakpoint.preview_sha256)
            .map_err(|_| SupervisorError::NoContainment)?;
        let step = review
            .preview(
                DebugReviewIntent::Step {
                    kind: StepKind::Over,
                    thread_id: owner.debuggee_thread_id,
                },
                owner.owner_sequence,
            )
            .map_err(|_| SupervisorError::NoContainment)?;
        if step.executable_authority || step.preview_sha256 == breakpoint.preview_sha256 {
            return Err(SupervisorError::NoContainment);
        }
        // Only the disposable native CI harness synthesizes an operator
        // gesture here. The persisted receipt explicitly says CI_ONLY;
        // it is NOT a claim that installed native human consent exists.
        let binding = review
            .pending_native_binding(&step.preview_sha256, owner.owner_sequence)
            .map_err(|_| SupervisorError::NoContainment)?;
        let root = supervisor.native_owner_receipt_root()?;
        let now = crate::now_ms()?;
        let mut action = DebugActionApproval::prepare_native(&root, binding, step.clone(), now)
            .map_err(|_| SupervisorError::JournalUnavailable)?;
        let native = NativeConsentMarker::for_native_ci_fixture(
            &step.preview_sha256,
            action.challenge_sha256(),
            owner.owner_sequence,
            now,
            true,
        );
        action
            .decide(native, owner.owner_sequence, now)
            .map_err(|_| SupervisorError::InvalidApproval)?;
        let current = self.gateway.verify_lldb_kernel_event_owner(
            owner.debuggee_pid,
            owner.debuggee_thread_id,
            owner.stopped_rip,
        )?;
        if current.seq != owner.owner_sequence {
            return Err(SupervisorError::NoContainment);
        }
        let consumed = action
            .consume_for_receipt_test(&step.preview_sha256, current.seq, now)
            .map_err(|_| SupervisorError::GrantConsumed)?;
        if consumed.execution_authorized
            || !consumed.ci_fixture_only
            || consumed.owner_sequence != owner.owner_sequence
            || consumed.consumed_receipt_sha256 == consumed.approved_receipt_sha256
            || action.consume_for_receipt_test(&step.preview_sha256, current.seq, now)
                != Err(ApprovalDenial::GrantConsumed)
        {
            return Err(SupervisorError::InvalidTransition);
        }
        // Until D6C supplies an independently verified *actual* operation
        // result, the committed outcome MUST remain uncertain. Never
        // invent a debugger command, DAP reply or success witness.
        action
            .record_uncertain_result(now)
            .map_err(|_| SupervisorError::JournalUnavailable)?;
        let audit = action
            .audit_reference("ResultUncertain")
            .map_err(|_| SupervisorError::JournalCorrupt)?;
        review
            .decline(&step.preview_sha256)
            .map_err(|_| SupervisorError::NoContainment)?;
        if review.preview(
            DebugReviewIntent::InspectVariables { frame_id: 1 },
            owner.owner_sequence,
        ) != Err(DebugReviewDenial::UnverifiedFrame)
        {
            return Err(SupervisorError::NoContainment);
        }
        // A later or continued event invalidates the entire review session;
        // do not restore control from an older persisted receipt.
        if review.preview(
            DebugReviewIntent::Step {
                kind: StepKind::Into,
                thread_id: owner.debuggee_thread_id,
            },
            owner.owner_sequence.saturating_add(1),
        ) != Err(DebugReviewDenial::StaleOrRevoked)
            || review.has_executable_authority()
        {
            return Err(SupervisorError::NoContainment);
        }
        Ok((
            audit.action_id_sha256,
            audit.preview_sha256,
            audit.last_receipt_sha256,
            audit.final_phase,
        ))
    }
    /// DEV-04.6C: actual *disposable Windows CI* one-step native execution.
    /// Never exposed to the desktop, model or release; no original debugger
    /// launch approval is reused. The fresh D6B action is consumed and
    /// fsynced BEFORE a single bounded DAP step is transmitted.
    #[cfg(all(debug_assertions, feature = "dev046c-ci-control"))]
    pub fn execute_one_protected_ci_step(
        &mut self,
        supervisor: &Supervisor,
        job_id: &str,
        plan: &JobPlan,
        target: &crate::project_debug_target::SealedProjectDebugTarget,
        original: &WindowsOwnerEventAccepted,
    ) -> Result<(WindowsOwnerEventAccepted, (String, String, String, String)), SupervisorError>
    {
        self.execute_one_protected_step_inner(
            supervisor, job_id, plan, target, original, None, None,
        )
    }

    /// D6D real native UI bridge for a *single* verified Windows LLDB stop.
    /// The callback is owned by the desktop Rust host (Tauri OS-native dialog),
    /// not the WebView, model, adapter, or historic consent. The original
    /// one-use claim must be fsynced before sending any bounded DAP action.
    #[cfg(all(debug_assertions, feature = "dev046d-native-dialog"))]
    pub fn execute_one_protected_native_step(
        &mut self,
        supervisor: &Supervisor,
        job_id: &str,
        plan: &JobPlan,
        target: &crate::project_debug_target::SealedProjectDebugTarget,
        original: &WindowsOwnerEventAccepted,
        approve: &mut dyn FnMut(&str) -> bool,
        cancelled: &std::sync::atomic::AtomicBool,
    ) -> Result<(WindowsOwnerEventAccepted, (String, String, String, String)), SupervisorError>
    {
        self.execute_one_protected_step_inner(
            supervisor,
            job_id,
            plan,
            target,
            original,
            Some(approve),
            Some(cancelled),
        )
    }

    #[cfg(all(debug_assertions, feature = "dev046c-ci-control"))]
    fn execute_one_protected_step_inner(
        &mut self,
        supervisor: &Supervisor,
        job_id: &str,
        plan: &JobPlan,
        target: &crate::project_debug_target::SealedProjectDebugTarget,
        original: &WindowsOwnerEventAccepted,
        approval: Option<&mut dyn FnMut(&str) -> bool>,
        cancelled: Option<&std::sync::atomic::AtomicBool>,
    ) -> Result<(WindowsOwnerEventAccepted, (String, String, String, String)), SupervisorError>
    {
        use crate::{
            debug_action_receipts::{DebugActionApproval, NativeConsentMarker},
            debug_command_review::{
                review_from_verified_native_owner_for_ci, DebugReviewIntent, StepKind,
            },
            debug_control_dispatch::{
                native_control_stop_from_owner, verify_independent_step_result, CiControlSlot,
                NativeStepAck,
            },
        };
        use lu_ide_workspace::dap_session::{CiStepKind, DapEvent, DapPhase};
        if self.gateway.phase() != DapPhase::Stopped
            || original.debugger_job_id != job_id
            || original.debugger_plan_sha256 != self.native_plan_sha256
            || original.source_snapshot_sha256 != self.source_input.seal.source_sha256
            || !original.lldb_event_loop_owner_attested
            || original.debugger_commands_authorized
            || original.installed_app_accepted
        {
            return Err(SupervisorError::NoContainment);
        }
        // Independent read-only job, source and original owner checks
        // immediately before minting a *new* D6C action challenge.
        target.verify()?;
        verify_input(&self.source_input).map_err(|_| SupervisorError::WrongProjectOrSource)?;
        let running = supervisor
            .inspect()?
            .into_iter()
            .find(|x| x.job_id == job_id)
            .ok_or(SupervisorError::NoSuchJob)?;
        if running.phase != JobPhase::Running
            || running.requires_reconciliation
            || running.plan_sha256 != self.native_plan_sha256
        {
            return Err(SupervisorError::InvalidTransition);
        }
        let current = self.gateway.verify_lldb_kernel_event_owner(
            original.debuggee_pid,
            original.debuggee_thread_id,
            original.stopped_rip,
        )?;
        if current.seq != original.owner_sequence {
            return Err(SupervisorError::NoContainment);
        }
        let mut review = review_from_verified_native_owner_for_ci(
            original,
            &self.source_input.seal.source_sha256,
        )
        .map_err(|_| SupervisorError::NoContainment)?;
        let preview = review
            .preview(
                DebugReviewIntent::Step {
                    kind: StepKind::Over,
                    thread_id: original.debuggee_thread_id,
                },
                original.owner_sequence,
            )
            .map_err(|_| SupervisorError::NoContainment)?;
        let binding = review
            .pending_native_binding(&preview.preview_sha256, original.owner_sequence)
            .map_err(|_| SupervisorError::NoContainment)?;
        let mut action = DebugActionApproval::prepare_native(
            &supervisor.native_owner_receipt_root()?,
            binding,
            preview.clone(),
            crate::now_ms()?,
        )
        .map_err(|_| SupervisorError::JournalUnavailable)?;
        let challenge = action.challenge_sha256().to_owned();
        let marker = if let Some(ask_native_operator) = approval {
            #[cfg(all(debug_assertions, feature = "dev046d-native-dialog"))]
            {
                let message = format!(
                    "Approve ONE native LLDB Step Over?\n\n\
                    Protected Job: {job_id}\n\
                    Original Cargo source: src/main.rs:{}\n\
                    Verified OS thread: {}\n\
                    Native owner event: {}\n\
                    Source snapshot: {}\n\
                    One-use review: {}\n\
                    One-use challenge: {}\n\n\
                    No evaluate, memory writes, terminal, arbitrary attach or reusable approval.\n\
                    This operation will be consumed before a debugger command is sent.",
                    original.verified_source_line,
                    original.debuggee_thread_id,
                    original.owner_sequence,
                    original.source_snapshot_sha256,
                    preview.preview_sha256,
                    challenge,
                );
                let confirmed = ask_native_operator(&message);
                // The modal may have blocked while the OS stop continued,
                // the source changed or the host was cancelled. Re-prove
                // the same kernel owner, Job/source and immutable target.
                if self.gateway.phase() != DapPhase::Stopped
                    || self
                        .gateway
                        .verify_lldb_kernel_event_owner(
                            original.debuggee_pid,
                            original.debuggee_thread_id,
                            original.stopped_rip,
                        )
                        .map(|event| event.seq)
                        .ok()
                        != Some(original.owner_sequence)
                    || target.verify().is_err()
                    || verify_input(&self.source_input).is_err()
                {
                    let _ = action.revoke(crate::now_ms()?);
                    return Err(SupervisorError::NoContainment);
                }
                NativeConsentMarker::for_trusted_modal(
                    &preview.preview_sha256,
                    &challenge,
                    original.owner_sequence,
                    crate::now_ms()?,
                    confirmed,
                )
            }
            #[cfg(not(all(debug_assertions, feature = "dev046d-native-dialog")))]
            {
                let _ = ask_native_operator;
                return Err(SupervisorError::InvalidApproval);
            }
        } else {
            NativeConsentMarker::for_native_ci_fixture(
                &preview.preview_sha256,
                &challenge,
                original.owner_sequence,
                crate::now_ms()?,
                true,
            )
        };
        if cancelled.is_some_and(|flag| flag.load(std::sync::atomic::Ordering::SeqCst)) {
            let _ = action.revoke(crate::now_ms()?);
            return Err(SupervisorError::Quarantined);
        }
        // The modal can remain open while a separate actor requests
        // cancellation, loses SCM custody or starts OS cleanup.
        let live_after_dialog = supervisor
            .inspect()?
            .into_iter()
            .find(|job| job.job_id == job_id)
            .ok_or(SupervisorError::NoSuchJob)?;
        if live_after_dialog.phase != JobPhase::Running
            || live_after_dialog.requires_reconciliation
            || live_after_dialog.plan_sha256 != self.native_plan_sha256
        {
            return Err(SupervisorError::Quarantined);
        }
        action
            .decide(marker, original.owner_sequence, crate::now_ms()?)
            .map_err(|_| SupervisorError::InvalidApproval)?;
        if cancelled.is_some_and(|flag| flag.load(std::sync::atomic::Ordering::SeqCst)) {
            let _ = action.revoke(crate::now_ms()?);
            return Err(SupervisorError::Quarantined);
        }
        let claim = action
            .consume_for_receipt_test(
                &preview.preview_sha256,
                original.owner_sequence,
                crate::now_ms()?,
            )
            .map_err(|_| SupervisorError::GrantConsumed)?;
        let mut slot = CiControlSlot::admit(
            claim,
            native_control_stop_from_owner(original).map_err(|_| SupervisorError::NoContainment)?,
        )
        .map_err(|_| SupervisorError::NoContainment)?;
        let dispatched = slot
            .take_step()
            .map_err(|_| SupervisorError::GrantConsumed)?;
        if slot.has_unconsumed_slot() {
            return Err(SupervisorError::InvalidTransition);
        }
        let ci_kind = match dispatched.kind() {
            StepKind::Over => CiStepKind::Over,
            StepKind::Into => CiStepKind::Into,
            StepKind::Out => CiStepKind::Out,
        };
        // Re-check the exact one-use owner event AFTER durable consumption
        // and immediately BEFORE the only executable native command.
        // A revoked/advanced stop cannot spend the old human action slot.
        let at_send = self.gateway.verify_lldb_kernel_event_owner(
            original.debuggee_pid,
            original.debuggee_thread_id,
            original.stopped_rip,
        )?;
        if at_send.seq != original.owner_sequence {
            return Err(SupervisorError::NoContainment);
        }
        // The consumed claim is now held only in 'dispatched', which is
        // non-clonable and never returned to a caller/renderer. A failed
        // send leaves the journal at Consumed or ResultUncertain; NO retry.
        if cancelled.is_some_and(|flag| flag.load(std::sync::atomic::Ordering::SeqCst)) {
            return Err(SupervisorError::Quarantined);
        }
        self.gateway
            .ci_send_step(dispatched.thread_id() as u64, ci_kind)?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(12);
        let mut saw_stop = false;
        let mut requested_threads = false;
        let mut requested_stack = false;
        let mut stack_received = false;
        let mut acknowledged = None;
        while std::time::Instant::now() < deadline && !(stack_received && acknowledged.is_some()) {
            if cancelled.is_some_and(|flag| flag.load(std::sync::atomic::Ordering::SeqCst)) {
                return Err(SupervisorError::Quarantined);
            }
            let events = self.gateway.poll()?;
            if acknowledged.is_none() {
                acknowledged = self.gateway.ci_take_observation();
            }
            for event in events {
                match event {
                    DapEvent::Stopped { thread_id }
                        if thread_id == u64::from(dispatched.thread_id()) && !saw_stop =>
                    {
                        saw_stop = true;
                        self.gateway.threads()?;
                        requested_threads = true;
                    }
                    DapEvent::ThreadsReceived if requested_threads && !requested_stack => {
                        self.gateway.stack_trace(dispatched.thread_id() as u64)?;
                        requested_stack = true;
                    }
                    DapEvent::StackTraceReceived if requested_stack => {
                        stack_received = true;
                    }
                    DapEvent::Terminated | DapEvent::Disconnected => {
                        return Err(SupervisorError::NoContainment);
                    }
                    _ => {}
                }
            }
            if !(stack_received && acknowledged.is_some()) {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }
        let observed = acknowledged.ok_or(SupervisorError::NoContainment)?;
        if !stack_received
            || !saw_stop
            || !observed.response_acknowledged
            || !observed.new_stop_observed
            || !observed.is_step
            || observed.thread_id != u64::from(dispatched.thread_id())
            || self.gateway.phase() != DapPhase::Stopped
        {
            return Err(SupervisorError::NoContainment);
        }
        // New compiler source and OS RIP must be independently verified by
        // EXACTLY the old D5 protected Job/PE/PDB/owner-receipt mechanism.
        // The adapter's positive ACK is not a successful step outcome.
        let after = self.verify_real_lldb_event_owner_for_ci(supervisor, job_id, plan, target)?;
        let new_stop =
            native_control_stop_from_owner(&after).map_err(|_| SupervisorError::NoContainment)?;
        let proof = verify_independent_step_result(
            &dispatched,
            &new_stop,
            NativeStepAck {
                correlated_response: observed.response_acknowledged,
                new_dap_stop: observed.new_stop_observed,
                thread_id: observed.thread_id as u32,
            },
        )
        .map_err(|_| SupervisorError::NoContainment)?;
        action
            .record_independent_ci_step_result(&proof, crate::now_ms()?)
            .map_err(|_| SupervisorError::JournalUnavailable)?;
        let audit = action
            .audit_reference("ResultVerified")
            .map_err(|_| SupervisorError::JournalCorrupt)?;
        review.invalidate();
        Ok((
            after,
            (
                audit.action_id_sha256,
                audit.preview_sha256,
                audit.last_receipt_sha256,
                audit.final_phase,
            ),
        ))
    }

    /// A breakpoint may be transmitted only to the same independently
    /// reviewed source location in disposable CI; a DAP verified=true
    /// response is NEVER sufficient to mark installation successful.
    #[cfg(all(debug_assertions, feature = "dev046c-ci-control"))]
    pub fn attempt_one_protected_ci_breakpoint(
        &mut self,
        supervisor: &Supervisor,
        job_id: &str,
        plan: &JobPlan,
        target: &crate::project_debug_target::SealedProjectDebugTarget,
        original: &WindowsOwnerEventAccepted,
    ) -> Result<(String, String, String, String), SupervisorError> {
        self.attempt_one_protected_breakpoint_inner(
            supervisor, job_id, plan, target, original, None, None,
        )
    }

    #[cfg(all(debug_assertions, feature = "dev046d-native-dialog"))]
    pub fn attempt_one_protected_native_breakpoint(
        &mut self,
        supervisor: &Supervisor,
        job_id: &str,
        plan: &JobPlan,
        target: &crate::project_debug_target::SealedProjectDebugTarget,
        original: &WindowsOwnerEventAccepted,
        approve: &mut dyn FnMut(&str) -> bool,
        cancelled: &std::sync::atomic::AtomicBool,
    ) -> Result<(String, String, String, String), SupervisorError> {
        self.attempt_one_protected_breakpoint_inner(
            supervisor,
            job_id,
            plan,
            target,
            original,
            Some(approve),
            Some(cancelled),
        )
    }

    #[cfg(all(debug_assertions, feature = "dev046c-ci-control"))]
    fn attempt_one_protected_breakpoint_inner(
        &mut self,
        supervisor: &Supervisor,
        job_id: &str,
        plan: &JobPlan,
        target: &crate::project_debug_target::SealedProjectDebugTarget,
        original: &WindowsOwnerEventAccepted,
        approval: Option<&mut dyn FnMut(&str) -> bool>,
        cancelled: Option<&std::sync::atomic::AtomicBool>,
    ) -> Result<(String, String, String, String), SupervisorError> {
        use crate::{
            debug_action_receipts::{DebugActionApproval, NativeConsentMarker},
            debug_command_review::{review_from_verified_native_owner_for_ci, DebugReviewIntent},
            debug_control_dispatch::{native_control_stop_from_owner, CiControlSlot},
        };
        use lu_ide_workspace::dap_session::DapPhase;
        if self.gateway.phase() != DapPhase::Stopped
            || original.debugger_job_id != job_id
            || original.debugger_job_id != self.native_job_id
            || original.debugger_plan_sha256 != self.native_plan_sha256
            || original.source_snapshot_sha256 != self.source_input.seal.source_sha256
            || !original.lldb_event_loop_owner_attested
            || original.debugger_commands_authorized
            || original.installed_app_accepted
        {
            return Err(SupervisorError::NoContainment);
        }
        target.verify()?;
        verify_input(&self.source_input).map_err(|_| SupervisorError::WrongProjectOrSource)?;
        let live = supervisor
            .inspect()?
            .into_iter()
            .find(|r| r.job_id == job_id)
            .ok_or(SupervisorError::NoSuchJob)?;
        if live.phase != JobPhase::Running
            || live.requires_reconciliation
            || live.plan_sha256 != self.native_plan_sha256
            || plan.input_snapshot_sha256 != self.source_input.seal.source_sha256
        {
            return Err(SupervisorError::InvalidTransition);
        }
        let current = self.gateway.verify_lldb_kernel_event_owner(
            original.debuggee_pid,
            original.debuggee_thread_id,
            original.stopped_rip,
        )?;
        if current.seq != original.owner_sequence {
            return Err(SupervisorError::NoContainment);
        }
        let mut review = review_from_verified_native_owner_for_ci(
            original,
            &self.source_input.seal.source_sha256,
        )
        .map_err(|_| SupervisorError::NoContainment)?;
        let preview = review
            .preview(
                DebugReviewIntent::Breakpoint {
                    source: "src/main.rs".into(),
                    line: original.verified_source_line,
                },
                original.owner_sequence,
            )
            .map_err(|_| SupervisorError::NoContainment)?;
        let binding = review
            .pending_native_binding(&preview.preview_sha256, original.owner_sequence)
            .map_err(|_| SupervisorError::NoContainment)?;
        let now = crate::now_ms()?;
        let mut action = DebugActionApproval::prepare_native(
            &supervisor.native_owner_receipt_root()?,
            binding,
            preview.clone(),
            now,
        )
        .map_err(|_| SupervisorError::JournalUnavailable)?;
        let challenge = action.challenge_sha256().to_owned();
        let marker = if let Some(ask_native_operator) = approval {
            #[cfg(all(debug_assertions, feature = "dev046d-native-dialog"))]
            {
                let message = format!(
                    "Approve ONE native LLDB source breakpoint?\n\n\
                    Protected Job: {job_id}\n\
                    Verified original source: src/main.rs:{}\n\
                    Verified OS thread: {}\n\
                    Native owner event: {}\n\
                    Source snapshot: {}\n\
                    One-use review: {}\n\
                    One-use challenge: {}\n\n\
                    LLDB breakpoint ACK is NOT an independently verified hit.\n\
                    The result will remain uncertain. No condition, shell or evaluate.",
                    original.verified_source_line,
                    original.debuggee_thread_id,
                    original.owner_sequence,
                    original.source_snapshot_sha256,
                    preview.preview_sha256,
                    challenge,
                );
                let confirmed = ask_native_operator(&message);
                if self.gateway.phase() != DapPhase::Stopped
                    || self
                        .gateway
                        .verify_lldb_kernel_event_owner(
                            original.debuggee_pid,
                            original.debuggee_thread_id,
                            original.stopped_rip,
                        )
                        .map(|event| event.seq)
                        .ok()
                        != Some(original.owner_sequence)
                    || target.verify().is_err()
                    || verify_input(&self.source_input).is_err()
                {
                    let _ = action.revoke(crate::now_ms()?);
                    return Err(SupervisorError::NoContainment);
                }
                NativeConsentMarker::for_trusted_modal(
                    &preview.preview_sha256,
                    &challenge,
                    original.owner_sequence,
                    crate::now_ms()?,
                    confirmed,
                )
            }
            #[cfg(not(all(debug_assertions, feature = "dev046d-native-dialog")))]
            {
                let _ = ask_native_operator;
                return Err(SupervisorError::InvalidApproval);
            }
        } else {
            NativeConsentMarker::for_native_ci_fixture(
                &preview.preview_sha256,
                &challenge,
                original.owner_sequence,
                now,
                true,
            )
        };
        if cancelled.is_some_and(|flag| flag.load(std::sync::atomic::Ordering::SeqCst)) {
            let _ = action.revoke(crate::now_ms()?);
            return Err(SupervisorError::Quarantined);
        }
        // The modal can remain open while a separate actor requests
        // cancellation, loses SCM custody or starts OS cleanup.
        let live_after_dialog = supervisor
            .inspect()?
            .into_iter()
            .find(|job| job.job_id == job_id)
            .ok_or(SupervisorError::NoSuchJob)?;
        if live_after_dialog.phase != JobPhase::Running
            || live_after_dialog.requires_reconciliation
            || live_after_dialog.plan_sha256 != self.native_plan_sha256
        {
            return Err(SupervisorError::Quarantined);
        }
        action
            .decide(marker, original.owner_sequence, crate::now_ms()?)
            .map_err(|_| SupervisorError::InvalidApproval)?;
        if cancelled.is_some_and(|flag| flag.load(std::sync::atomic::Ordering::SeqCst)) {
            let _ = action.revoke(crate::now_ms()?);
            return Err(SupervisorError::Quarantined);
        }
        let claim = action
            .consume_for_receipt_test(
                &preview.preview_sha256,
                original.owner_sequence,
                crate::now_ms()?,
            )
            .map_err(|_| SupervisorError::GrantConsumed)?;
        let mut slot = CiControlSlot::admit(
            claim,
            native_control_stop_from_owner(original).map_err(|_| SupervisorError::NoContainment)?,
        )
        .map_err(|_| SupervisorError::NoContainment)?;
        let submitted = slot
            .take_breakpoint()
            .map_err(|_| SupervisorError::GrantConsumed)?;
        if submitted.independently_installed()
            || slot.has_unconsumed_slot()
            || submitted.source_line() != original.verified_source_line
        {
            return Err(SupervisorError::NoContainment);
        }
        let at_send = self.gateway.verify_lldb_kernel_event_owner(
            original.debuggee_pid,
            original.debuggee_thread_id,
            original.stopped_rip,
        )?;
        if at_send.seq != original.owner_sequence {
            return Err(SupervisorError::NoContainment);
        }
        if cancelled.is_some_and(|flag| flag.load(std::sync::atomic::Ordering::SeqCst)) {
            return Err(SupervisorError::Quarantined);
        }
        self.gateway.ci_send_one_line_breakpoint()?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let mut ack = None;
        while std::time::Instant::now() < deadline && ack.is_none() {
            if cancelled.is_some_and(|flag| flag.load(std::sync::atomic::Ordering::SeqCst)) {
                return Err(SupervisorError::Quarantined);
            }
            let _ = self.gateway.poll()?;
            ack = self.gateway.ci_take_observation();
            if ack.is_none() {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }
        let ack = ack.ok_or(SupervisorError::NoContainment)?;
        if !ack.response_acknowledged || ack.is_step || self.gateway.phase() != DapPhase::Stopped {
            return Err(SupervisorError::NoContainment);
        }
        // Recheck still-owned stop, sealed original PE/PDB/source and Job
        // after LLDB's ACK; this still cannot assert an installed breakpoint.
        target.verify()?;
        verify_input(&self.source_input).map_err(|_| SupervisorError::WrongProjectOrSource)?;
        let owner_after = self.gateway.verify_lldb_kernel_event_owner(
            original.debuggee_pid,
            original.debuggee_thread_id,
            original.stopped_rip,
        )?;
        if owner_after.seq != original.owner_sequence {
            return Err(SupervisorError::NoContainment);
        }
        // Independent breakpoint *hit* or process memory proof does not yet
        // exist, so record uncertainty even when LLDB acknowledged the write.
        action
            .record_uncertain_result(crate::now_ms()?)
            .map_err(|_| SupervisorError::JournalUnavailable)?;
        let audit = action
            .audit_reference("ResultUncertain")
            .map_err(|_| SupervisorError::JournalCorrupt)?;
        review.invalidate();
        Ok((
            audit.action_id_sha256,
            audit.preview_sha256,
            audit.last_receipt_sha256,
            audit.final_phase,
        ))
    }
    #[cfg(all(debug_assertions, feature = "dev045d5-ci-event-owner"))]
    pub fn crash_real_lldb_after_owner_receipt_for_ci(&mut self) -> Result<(), SupervisorError> {
        self.gateway.crash_protected_lldb_for_ci()
    }
    #[cfg(all(debug_assertions, feature = "dev045d5-ci-event-owner"))]
    pub fn close_with_real_owner_continue_for_ci(
        self,
        sequence: u64,
    ) -> Result<(), SupervisorError> {
        self.gateway
            .close_with_actual_owner_continue_for_ci(sequence)
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
