//! Wave D7: opt-in native Windows AppContainer build/test worker.
//!
//! The worker is deliberately NOT connected to Tauri, the WebView, or Lu Core.
//! The native host supplies an already approved exact JobPlan and a pinned,
//! trusted, offline toolchain bundle. No shell, user command, PATH search,
//! credential inheritance, general broker, or unrestricted fallback exists.
//! Security and crash-injection acceptance are required before enabling this
//! feature in a distributed IDE build.
#[cfg(not(feature = "protected-recovery-service"))]
use crate::windows_broker::BrokerLease;
#[cfg(feature = "protected-recovery-service")]
use crate::windows_service_client::ServiceLease as BrokerLease;
use crate::{
    snapshot::{verify_input, PublishedInput},
    windows_broker::TrustedBroker,
    windows_cap_tree::{audit_tree, digest_tree, publish_into_native_parent},
    JobPhase, JobPlan, JobSummary, NativeTreeTerminator, OsContainmentProof, Supervisor,
    SupervisorError,
};
use std::{
    ffi::OsStr,
    fs::{self},
    io,
    mem::size_of,
    os::windows::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
    ptr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{
        CloseHandle, GetLastError, LocalFree, ERROR_ALREADY_EXISTS, HANDLE, INVALID_HANDLE_VALUE,
        WAIT_OBJECT_0, WAIT_TIMEOUT,
    },
    Security::{
        Authorization::ConvertSidToStringSidW,
        EqualSid, FreeSid, GetTokenInformation,
        Isolation::{
            CreateAppContainerProfile, DeleteAppContainerProfile, GetAppContainerFolderPath,
        },
        TokenAppContainerSid, TokenIsAppContainer, PSID, SECURITY_ATTRIBUTES,
        SECURITY_CAPABILITIES, TOKEN_QUERY,
    },
    Storage::FileSystem::{CreateFileW, CREATE_NEW, OPEN_EXISTING},
    System::{
        Com::CoTaskMemFree,
        JobObjects::{
            CreateJobObjectW, JobObjectBasicAccountingInformation,
            JobObjectCpuRateControlInformation, JobObjectExtendedLimitInformation, OpenJobObjectW,
            QueryInformationJobObject, SetInformationJobObject, TerminateJobObject,
            JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JOB_OBJECT_LIMIT_ACTIVE_PROCESS, JOB_OBJECT_LIMIT_JOB_MEMORY,
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        },
        Threading::{
            CreateProcessW, DeleteProcThreadAttributeList, GetExitCodeProcess,
            InitializeProcThreadAttributeList, OpenProcessToken, ResumeThread, TerminateProcess,
            UpdateProcThreadAttribute, WaitForSingleObject, CREATE_NO_WINDOW, CREATE_SUSPENDED,
            CREATE_UNICODE_ENVIRONMENT, EXTENDED_STARTUPINFO_PRESENT, PROCESS_INFORMATION,
            PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES, STARTF_USESTDHANDLES, STARTUPINFOEXW,
        },
    },
};

#[cfg(all(feature = "lsp-transport", feature = "protected-recovery-service"))]
#[path = "windows_lsp_worker.rs"]
pub mod lsp_native;
const MAX_BUNDLE_FILES: usize = 60_000;
const MAX_BUNDLE_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const MAX_OUTPUT_FILES: usize = 6000;
const MAX_OUTPUT_BYTES: u64 = 256 * 1024 * 1024;
const MAX_OUTPUT_FILE_BYTES: u64 = 64 * 1024 * 1024;
const LOOP_DELAY: Duration = Duration::from_millis(40);
const REAP_LIMIT: Duration = Duration::from_secs(5);
const JOB_OBJECT_TERMINATE_ACCESS: u32 = 0x0008;
const JOB_OBJECT_QUERY_ACCESS: u32 = 0x0004;
const PROC_THREAD_ATTRIBUTE_HANDLE_LIST_VALUE: usize = 0x0002_0002;
const GENERIC_READ_ACCESS: u32 = 0x8000_0000;
const GENERIC_WRITE_ACCESS: u32 = 0x4000_0000;
const FILE_SHARE_READ_VALUE: u32 = 1;
const FILE_SHARE_WRITE_VALUE: u32 = 2;
const FILE_ATTRIBUTE_NORMAL_VALUE: u32 = 0x80;
// JOBOBJECT_CPU_RATE_CONTROL_INFORMATION is two DWORDs when using CpuRate.
// ENABLE(1) | HARD_CAP(4), CpuRate = 5000/10000 (50%).
const CPU_RATE_HARD_CAP: u32 = 0x1 | 0x4;
const CPU_RATE_HALF: u32 = 5000;
// Process creation atomically attaches the child to this Job Object (Win10+).
// The old create-suspended / assign / resume pattern has a crash orphan gap.
const PROC_THREAD_ATTRIBUTE_JOB_LIST_VALUE: usize = 131085;

fn denied() -> SupervisorError {
    SupervisorError::NoContainment
}
fn io_denied<T>(_: T) -> SupervisorError {
    denied()
}

#[derive(Clone, Default)]
pub struct WindowsCancellation(Arc<AtomicBool>);
impl WindowsCancellation {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn request_cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
    pub fn clone_handle(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

#[derive(Debug)]
pub struct WindowsExecutionResult {
    pub summary: JobSummary,
    pub exit_code: Option<u32>,
    pub output_root: PathBuf,
    pub output_bytes: u64,
    pub output_sha256: String,
    pub timed_out: bool,
}

struct Handle(HANDLE);
// A HANDLE is an owned kernel value. Moving it between trusted native worker
// threads does not share ownership or grant additional access. E2 pipe
// ownership moves once into a bounded writer thread.
#[cfg(feature = "lsp-transport")]
unsafe impl Send for Handle {}
impl Handle {
    fn new(raw: HANDLE) -> Result<Self, SupervisorError> {
        if raw.is_null() {
            Err(denied())
        } else {
            Ok(Self(raw))
        }
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: uniquely owned native kernel handle.
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}
fn wide(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(std::iter::once(0)).collect()
}
fn wstr(value: &str) -> Vec<u16> {
    wide(OsStr::new(value))
}
fn valid_job_id(id: &str) -> bool {
    id.len() == 36 && id.starts_with("job-") && id[4..].bytes().all(|x| x.is_ascii_hexdigit())
}
fn job_name(id: &str, hash: &str) -> Result<Vec<u16>, SupervisorError> {
    if !valid_job_id(id) || hash.len() != 64 || !hash.bytes().all(|x| x.is_ascii_hexdigit()) {
        return Err(SupervisorError::InvalidPlan);
    }
    #[cfg(feature = "protected-recovery-service")]
    let prefix = "Global";
    #[cfg(not(feature = "protected-recovery-service"))]
    let prefix = "Local";
    Ok(wstr(&format!("{prefix}\\LuIdeD7-{id}-{}", &hash[..16])))
}

fn clean_meta(path: &Path) -> Result<fs::Metadata, SupervisorError> {
    let m = fs::symlink_metadata(path).map_err(io_denied)?;
    use std::os::windows::fs::MetadataExt;
    if m.is_symlink() || m.file_attributes() & 0x400 != 0 {
        return Err(denied());
    }
    Ok(m)
}
/// Internal native-host helper to bind a provisioned offline bundle to an
/// exact JobPlan. No executable path, bundle path or digest is accepted from
/// the WebView or model; the caller must authenticate installation provenance.
pub fn trusted_offline_toolchain_digest(bundle: &Path) -> Result<String, SupervisorError> {
    if !clean_meta(&bundle.join("bin/cargo.exe"))?.is_file()
        || !clean_meta(&bundle.join("bin/rustc.exe"))?.is_file()
        || !clean_meta(&bundle.join("lib"))?.is_dir()
        || !clean_meta(&bundle.join("cargo-home"))?.is_dir()
    {
        return Err(denied());
    }
    digest_tree(bundle, None, MAX_BUNDLE_FILES, MAX_BUNDLE_BYTES)
}

fn stage_toolchain(bundle: &Path, dest: &Path, expected: &str) -> Result<(), SupervisorError> {
    if !expected.bytes().all(|x| x.is_ascii_hexdigit()) || expected.len() != 64 {
        return Err(SupervisorError::InvalidPlan);
    }
    // The native host chooses a pinned offline bundle root. The WebView
    // cannot provide paths, hashes, commands, flags or executables.
    for executable in ["bin/cargo.exe", "bin/rustc.exe"] {
        let p = bundle.join(executable);
        if !clean_meta(&p)?.is_file() {
            return Err(denied());
        }
    }
    if !clean_meta(&bundle.join("lib"))?.is_dir()
        || !clean_meta(&bundle.join("cargo-home"))?.is_dir()
    {
        return Err(denied());
    }
    let parent = dest.parent().ok_or_else(denied)?;
    let name = dest.file_name().ok_or_else(denied)?;
    let (_, copied_hash) = publish_into_native_parent(
        bundle,
        parent,
        name,
        MAX_BUNDLE_FILES,
        MAX_BUNDLE_BYTES,
        1024 * 1024 * 1024,
    )?;
    if copied_hash != expected {
        return Err(SupervisorError::WrongProjectOrSource);
    }
    Ok(())
}
fn stage_workspace(input: &PublishedInput, dest: &Path) -> Result<(), SupervisorError> {
    verify_input(input).map_err(|_| SupervisorError::WrongProjectOrSource)?;
    let parent = dest.parent().ok_or_else(denied)?;
    let name = dest.file_name().ok_or_else(denied)?;
    let (_, staged_hash) = publish_into_native_parent(
        &input.root,
        parent,
        name,
        12_000,
        512 * 1024 * 1024,
        64 * 1024 * 1024,
    )?;
    if staged_hash != input.seal.source_sha256 {
        return Err(SupervisorError::WrongProjectOrSource);
    }
    let staged = PublishedInput {
        root: dest.to_path_buf(),
        seal: input.seal.clone(),
    };
    verify_input(&staged).map_err(|_| denied())
}
fn output_seal(root: &Path) -> Result<(u64, String), SupervisorError> {
    audit_tree(
        root,
        None,
        MAX_OUTPUT_FILES,
        MAX_OUTPUT_BYTES,
        MAX_OUTPUT_FILE_BYTES,
    )
}

/// Publish only fully scanned, size-limited output from a terminated kernel
/// Job into the native application profile. The AppContainer has no access to
/// this parent. Reseal the copied bytes before returning an output coordinate.
fn publish_output(
    child_output: &Path,
    trusted_native_parent: &Path,
    id: &str,
) -> Result<(PathBuf, u64, String), SupervisorError> {
    if !valid_job_id(id) || !clean_meta(trusted_native_parent)?.is_dir() {
        return Err(denied());
    }
    let parent = fs::canonicalize(trusted_native_parent).map_err(io_denied)?;
    let source = fs::canonicalize(child_output).map_err(io_denied)?;
    if parent.starts_with(&source) || source.starts_with(&parent) {
        return Err(denied());
    }
    let (bytes, original_hash) = output_seal(child_output)?;
    // The native parent and newly created job directory are pinned by
    // cap_std handles until all bytes have been copied and re-sealed.
    let (published_bytes, published_hash) = publish_into_native_parent(
        child_output,
        &parent,
        OsStr::new(id),
        MAX_OUTPUT_FILES,
        MAX_OUTPUT_BYTES,
        MAX_OUTPUT_FILE_BYTES,
    )?;
    if original_hash != published_hash || bytes != published_bytes {
        return Err(denied());
    }
    Ok((parent.join(id), published_bytes, published_hash))
}

struct AppProfile {
    name: Vec<u16>,
    sid: PSID,
}
impl Drop for AppProfile {
    fn drop(&mut self) {
        // SAFETY: SID and profile owned by this native broker.
        unsafe {
            if !self.sid.is_null() {
                FreeSid(self.sid);
            }
            DeleteAppContainerProfile(self.name.as_ptr());
        }
    }
}
impl AppProfile {
    fn create(id: &str) -> Result<Self, SupervisorError> {
        if !valid_job_id(id) {
            return Err(SupervisorError::InvalidPlan);
        }
        // Fresh, unpredictable job IDs prevent name reuse, but OS rejects
        // any previously-existing profile instead of attaching to it.
        let name = wstr(&format!("LuIdeD7{}", &id[4..]));
        let description = wstr("Lu IDE native isolated worker");
        let mut sid: PSID = ptr::null_mut();
        let hr = unsafe {
            CreateAppContainerProfile(
                name.as_ptr(),
                description.as_ptr(),
                description.as_ptr(),
                ptr::null(),
                0,
                &mut sid,
            )
        };
        if hr < 0 || sid.is_null() {
            return Err(denied());
        }
        Ok(Self { name, sid })
    }
    fn local_state(&self) -> Result<PathBuf, SupervisorError> {
        let mut sid_wide: *mut u16 = ptr::null_mut();
        if unsafe { ConvertSidToStringSidW(self.sid, &mut sid_wide) } == 0 || sid_wide.is_null() {
            return Err(denied());
        }
        let sid_string = read_wide(sid_wide)?;
        unsafe {
            LocalFree(sid_wide.cast());
        }
        let name = wstr(&sid_string);
        let mut result: *mut u16 = ptr::null_mut();
        let hr = unsafe { GetAppContainerFolderPath(name.as_ptr(), &mut result) };
        if hr < 0 || result.is_null() {
            return Err(denied());
        }
        let value = read_wide(result)?;
        unsafe {
            CoTaskMemFree(result.cast());
        }
        let path = PathBuf::from(value).join("LocalState");
        fs::create_dir_all(&path).map_err(io_denied)?;
        Ok(path)
    }
}
fn read_wide(value: *const u16) -> Result<String, SupervisorError> {
    if value.is_null() {
        return Err(denied());
    }
    let mut n = 0_usize;
    while n < 32760 && unsafe { *value.add(n) } != 0 {
        n += 1;
    }
    if n == 32760 {
        return Err(denied());
    }
    let slice = unsafe { std::slice::from_raw_parts(value, n) };
    String::from_utf16(slice).map_err(io_denied)
}
struct Attributes(Vec<usize>);
impl Attributes {
    fn new() -> Result<Self, SupervisorError> {
        // Two explicitly scoped stdio handles, no ambient inheritance.
        Self::with_count(3)
    }
    /// E2 requires a THIRD attribute: the explicit inherited stdio HANDLE_LIST.
    /// The original worker retains exactly two attributes and zero inherited
    /// handles. Do not widen the default Cargo build worker's OS rights.
    #[cfg(feature = "lsp-transport")]
    fn with_count(count: u32) -> Result<Self, SupervisorError> {
        if !(2..=3).contains(&count) {
            return Err(denied());
        }
        let mut size = 0_usize;
        unsafe {
            InitializeProcThreadAttributeList(ptr::null_mut(), count, 0, &mut size);
        }
        if size == 0 || size > 32768 {
            return Err(denied());
        }
        let mut bytes = vec![0_usize; size.div_ceil(size_of::<usize>())];
        if unsafe {
            InitializeProcThreadAttributeList(bytes.as_mut_ptr().cast(), count, 0, &mut size)
        } == 0
        {
            return Err(denied());
        }
        Ok(Self(bytes))
    }
    #[cfg(not(feature = "lsp-transport"))]
    fn with_count(count: u32) -> Result<Self, SupervisorError> {
        if count != 3 {
            return Err(denied());
        }
        let mut size = 0_usize;
        unsafe {
            InitializeProcThreadAttributeList(ptr::null_mut(), count, 0, &mut size);
        }
        if size == 0 || size > 32768 {
            return Err(denied());
        }
        let mut bytes = vec![0_usize; size.div_ceil(size_of::<usize>())];
        if unsafe {
            InitializeProcThreadAttributeList(bytes.as_mut_ptr().cast(), count, 0, &mut size)
        } == 0
        {
            return Err(denied());
        }
        Ok(Self(bytes))
    }
    fn raw(&mut self) -> *mut core::ffi::c_void {
        self.0.as_mut_ptr().cast()
    }
}
impl Drop for Attributes {
    fn drop(&mut self) {
        unsafe {
            DeleteProcThreadAttributeList(self.0.as_mut_ptr().cast());
        }
    }
}

#[repr(C)]
struct CpuRateControl {
    flags: u32,
    rate: u32,
}
struct Job {
    handle: Handle,
}
impl Job {
    fn create(id: &str, hash: &str, plan: &JobPlan) -> Result<Self, SupervisorError> {
        let name = job_name(id, hash)?;
        #[cfg(feature = "protected-recovery-service")]
        let raw = {
            let acl = crate::windows_service_security::JobSecurity::new()?;
            unsafe { CreateJobObjectW(&acl.attributes, name.as_ptr()) }
        };
        // Preserve D7C's local Job fixture without needing a service SID
        // to be installed. No implicit protected-service downgrade exists
        // when the dedicated D7D production feature is selected.
        #[cfg(not(feature = "protected-recovery-service"))]
        let raw = unsafe { CreateJobObjectW(ptr::null(), name.as_ptr()) };
        let created = Handle::new(raw)?;
        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            return Err(denied());
        }
        let mut limit = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limit.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
            | JOB_OBJECT_LIMIT_ACTIVE_PROCESS
            | JOB_OBJECT_LIMIT_JOB_MEMORY;
        limit.BasicLimitInformation.ActiveProcessLimit = plan.process_limit;
        limit.JobMemoryLimit = usize::try_from(plan.memory_limit_bytes).map_err(io_denied)?;
        if unsafe {
            SetInformationJobObject(
                created.0,
                JobObjectExtendedLimitInformation,
                (&raw const limit).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        } == 0
        {
            return Err(denied());
        }
        let cpu = CpuRateControl {
            flags: CPU_RATE_HARD_CAP,
            rate: CPU_RATE_HALF,
        };
        if unsafe {
            SetInformationJobObject(
                created.0,
                JobObjectCpuRateControlInformation,
                (&raw const cpu).cast(),
                size_of::<CpuRateControl>() as u32,
            )
        } == 0
        {
            return Err(denied());
        }
        Ok(Self { handle: created })
    }
    fn empty(&self) -> Result<bool, SupervisorError> {
        let mut info = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
        if unsafe {
            QueryInformationJobObject(
                self.handle.0,
                JobObjectBasicAccountingInformation,
                (&mut info as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
                size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                ptr::null_mut(),
            )
        } == 0
        {
            return Err(denied());
        }
        Ok(info.ActiveProcesses == 0)
    }
    fn await_empty(&self, duration: Duration) -> bool {
        let begin = Instant::now();
        while begin.elapsed() < duration {
            if matches!(self.empty(), Ok(true)) {
                return true;
            }
            thread::sleep(LOOP_DELAY);
        }
        matches!(self.empty(), Ok(true))
    }
    fn kill_and_verify(&self) -> Result<bool, SupervisorError> {
        if unsafe { TerminateJobObject(self.handle.0, 77) } == 0 {
            return Err(denied());
        }
        Ok(self.await_empty(REAP_LIMIT))
    }
}
struct JobTerminator<'a> {
    job: &'a Job,
    id: &'a str,
    hash: &'a str,
}
impl NativeTreeTerminator for JobTerminator<'_> {
    fn terminate_and_verify_tree(&mut self, id: &str, hash: &str) -> Result<bool, SupervisorError> {
        if id != self.id || hash != self.hash {
            return Err(SupervisorError::WrongProjectOrSource);
        }
        self.job.kill_and_verify()
    }
}
#[repr(C)]
struct AppContainerTokenInfo {
    sid: PSID,
}

fn verify_suspended_appcontainer_token(
    process: HANDLE,
    expected_sid: PSID,
) -> Result<(), SupervisorError> {
    let mut raw: HANDLE = ptr::null_mut();
    if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut raw) } == 0 {
        return Err(denied());
    }
    let token = Handle::new(raw)?;
    let mut present = 0_u32;
    let mut len = 0_u32;
    if unsafe {
        GetTokenInformation(
            token.0,
            TokenIsAppContainer,
            (&mut present as *mut u32).cast(),
            size_of::<u32>() as u32,
            &mut len,
        )
    } == 0
        || present != 1
    {
        return Err(denied());
    }
    let mut size = 0_u32;
    unsafe {
        GetTokenInformation(token.0, TokenAppContainerSid, ptr::null_mut(), 0, &mut size);
    }
    if size < size_of::<AppContainerTokenInfo>() as u32 || size > 4096 {
        return Err(denied());
    }
    let mut buffer = vec![0_usize; (size as usize).div_ceil(size_of::<usize>())];
    if unsafe {
        GetTokenInformation(
            token.0,
            TokenAppContainerSid,
            buffer.as_mut_ptr().cast(),
            size,
            &mut len,
        )
    } == 0
    {
        return Err(denied());
    }
    let found = unsafe { (*(buffer.as_ptr() as *const AppContainerTokenInfo)).sid };
    if found.is_null() || unsafe { EqualSid(found, expected_sid) } == 0 {
        return Err(denied());
    }
    Ok(())
}

fn terminate_unassigned(process: HANDLE) -> Result<(), SupervisorError> {
    if unsafe { TerminateProcess(process, 103) } == 0
        || unsafe { WaitForSingleObject(process, 5000) } != WAIT_OBJECT_0
    {
        return Err(denied());
    }
    Ok(())
}
fn bootstrap_environment(
    local: &Path,
    workspace: &Path,
    output: &Path,
    tools: &Path,
) -> Result<Vec<u16>, SupervisorError> {
    let system = std::env::var_os("SystemRoot").ok_or_else(denied)?;
    let appdata = std::env::var_os("LOCALAPPDATA").ok_or_else(denied)?;
    if !Path::new(&system).is_absolute() || !Path::new(&appdata).is_absolute() {
        return Err(denied());
    }
    let bin = tools.join("bin");
    let sdk_bin = tools.join("sdk-bin");
    let rustc = bin.join("rustc.exe");
    let vc_lib = tools.join("msvc/lib/x64");
    let vc_include = tools.join("msvc/include");
    let sdk_lib_um = tools.join("windows-sdk/lib/um/x64");
    let sdk_lib_ucrt = tools.join("windows-sdk/lib/ucrt/x64");
    let sdk_include_um = tools.join("windows-sdk/include/um");
    let sdk_include_ucrt = tools.join("windows-sdk/include/ucrt");
    let sdk_include_shared = tools.join("windows-sdk/include/shared");
    for required in [
        &rustc,
        &bin.join("cargo.exe"),
        &bin.join("link.exe"),
        &sdk_bin.join("rc.exe"),
        &sdk_bin.join("mt.exe"),
        &vc_lib,
        &vc_include,
        &sdk_lib_um,
        &sdk_lib_ucrt,
        &sdk_include_um,
        &sdk_include_ucrt,
        &sdk_include_shared,
    ] {
        let meta = clean_meta(required)?;
        if !meta.is_dir() && !meta.is_file() {
            return Err(denied());
        }
    }
    // Source, target, compiler, linker, VC libs, UCRT, headers and SDK
    // tools must all resolve inside the *same content-sealed bundle*.
    // Host PATH/LIB/INCLUDE and VS toolchain variables are not inherited.
    let sdk_root = tools.join("windows-sdk");
    let vc_root = tools.join("msvc");
    let os_system32 = Path::new(&system).join("System32");
    let env = format!(
        "CARGO_HOME={}\0CARGO_INCREMENTAL=0\0CARGO_NET_OFFLINE=true\0CARGO_TARGET_DIR={}\0INCLUDE={};{};{};{}\0LIB={};{};{}\0LOCALAPPDATA={}\0PATH={};{};{}\0RUSTC={}\0SystemRoot={}\0TEMP={}\0TMP={}\0VCToolsInstallDir={}\0WindowsSdkDir={}\0\0",
        tools.join("cargo-home").display(), output.display(),
        vc_include.display(), sdk_include_um.display(),
        sdk_include_ucrt.display(), sdk_include_shared.display(),
        vc_lib.display(), sdk_lib_um.display(), sdk_lib_ucrt.display(),
        appdata.to_string_lossy(), bin.display(), sdk_bin.display(),
        os_system32.display(), rustc.display(), system.to_string_lossy(),
        local.display(), local.display(), vc_root.display(), sdk_root.display()
    );
    if env.encode_utf16().count() > 32767
        || workspace.to_str().is_none()
        || env.chars().any(|c| c == '\u{0001}')
    {
        return Err(denied());
    }
    Ok(env.encode_utf16().collect())
}
fn command_for(
    plan: &JobPlan,
    tools: &Path,
    workspace: &Path,
) -> Result<(PathBuf, Vec<u16>), SupervisorError> {
    let (subcommand, manifest) = match plan.recipe_id.as_str() {
        "rust_check" => ("check", "Cargo.toml"),
        "rust_tests" => ("test", "Cargo.toml"),
        "lu_ide_host_tests" => ("test", "ide/host/Cargo.toml"),
        _ => return Err(SupervisorError::InvalidPlan),
    };
    let exe = tools.join("bin/cargo.exe");
    if !clean_meta(&exe)?.is_file() {
        return Err(denied());
    }
    let file = workspace.join(manifest);
    if !clean_meta(&file)?.is_file() {
        return Err(denied());
    }
    let command = format!(
        "\"{}\" {subcommand} --offline --locked --manifest-path \"{}\"",
        exe.display(),
        file.display()
    );
    // All components are host-determined fixed paths; no project string is
    // inserted into process command, environment or executable selection.
    if command.contains('\n') || command.contains('\r') {
        return Err(denied());
    }
    Ok((exe, wstr(&command)))
}
/// Fixed NUL stdin and one write-only diagnostic log, located strictly
/// inside the worker's already-contained output. Kernel HANDLE_LIST blocks
/// every unrelated native host handle, including journal and credentials.
fn isolated_cargo_stdio(output: &Path) -> Result<(Handle, Handle), SupervisorError> {
    if !clean_meta(output)?.is_dir() {
        return Err(denied());
    }
    let mut sec = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: ptr::null_mut(),
        bInheritHandle: 1,
    };
    let nul = unsafe {
        CreateFileW(
            wstr("NUL").as_ptr(),
            GENERIC_READ_ACCESS,
            FILE_SHARE_READ_VALUE | FILE_SHARE_WRITE_VALUE,
            &mut sec,
            OPEN_EXISTING,
            0,
            ptr::null_mut(),
        )
    };
    if nul.is_null() || nul == INVALID_HANDLE_VALUE {
        return Err(denied());
    }
    let nul = Handle::new(nul)?;
    let file = output.join("cargo-diagnostics.log");
    let diagnostics = unsafe {
        CreateFileW(
            wide(file.as_os_str()).as_ptr(),
            GENERIC_WRITE_ACCESS,
            0,
            &mut sec,
            CREATE_NEW,
            FILE_ATTRIBUTE_NORMAL_VALUE,
            ptr::null_mut(),
        )
    };
    if diagnostics.is_null() || diagnostics == INVALID_HANDLE_VALUE {
        return Err(denied());
    }
    Ok((nul, Handle::new(diagnostics)?))
}

fn launch_appcontainer(
    profile: &AppProfile,
    job: &Job,
    executable: &Path,
    command: &mut Vec<u16>,
    cwd: &Path,
    environment: &[u16],
    output: &Path,
) -> Result<(Handle, Handle), SupervisorError> {
    let mut cap = SECURITY_CAPABILITIES::default();
    cap.AppContainerSid = profile.sid;
    cap.Capabilities = ptr::null_mut();
    cap.CapabilityCount = 0;
    let mut attrs = Attributes::new()?;
    if unsafe {
        UpdateProcThreadAttribute(
            attrs.raw(),
            0,
            PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES as usize,
            (&cap as *const SECURITY_CAPABILITIES).cast(),
            size_of::<SECURITY_CAPABILITIES>(),
            ptr::null_mut(),
            ptr::null(),
        )
    } == 0
    {
        return Err(denied());
    }
    // OS assigns the job at CreateProcessW, before any suspended child
    // exists. If the broker crashes at any point after creation, closing its
    // Job handle kills the contained child; no unassigned orphan gap.
    let jobs = [job.handle.0];
    if unsafe {
        UpdateProcThreadAttribute(
            attrs.raw(),
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
    let (stdin_nul, diagnostic_log) = isolated_cargo_stdio(output)?;
    let inherited = [stdin_nul.0, diagnostic_log.0];
    if unsafe {
        UpdateProcThreadAttribute(
            attrs.raw(),
            0,
            PROC_THREAD_ATTRIBUTE_HANDLE_LIST_VALUE,
            inherited.as_ptr().cast(),
            size_of::<[HANDLE; 2]>(),
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
    startup.StartupInfo.hStdInput = stdin_nul.0;
    startup.StartupInfo.hStdOutput = diagnostic_log.0;
    startup.StartupInfo.hStdError = diagnostic_log.0;
    startup.lpAttributeList = attrs.raw();
    let mut proc = PROCESS_INFORMATION::default();
    let exe_wide = wide(executable.as_os_str());
    let cwd_wide = wide(cwd.as_os_str());
    let spawned = unsafe {
        CreateProcessW(
            exe_wide.as_ptr(),
            command.as_mut_ptr(),
            ptr::null(),
            ptr::null(),
            1, // inherit only the two kernel-allowlisted stdio handles
            CREATE_SUSPENDED
                | CREATE_NO_WINDOW
                | CREATE_UNICODE_ENVIRONMENT
                | EXTENDED_STARTUPINFO_PRESENT,
            environment.as_ptr().cast(),
            cwd_wide.as_ptr(),
            &startup.StartupInfo,
            &mut proc,
        )
    };
    if spawned == 0 {
        return Err(denied());
    }
    let process = Handle::new(proc.hProcess)?;
    let thread = match Handle::new(proc.hThread) {
        Ok(t) => t,
        Err(e) => {
            terminate_unassigned(process.0)?;
            return Err(e);
        }
    };
    // The unique Job must contain the suspended child already, because
    // membership was supplied as a creation attribute rather than an
    // after-creation assignment subject to host-crash races.
    if !matches!(job.empty(), Ok(false))
        || verify_suspended_appcontainer_token(process.0, profile.sid).is_err()
    {
        // Already in the Job Object at process creation; a verified Job kill
        // applies even if the token differs from the expected profile SID.
        if !matches!(job.kill_and_verify(), Ok(true)) {
            return Err(SupervisorError::Quarantined);
        }
        return Err(denied());
    }
    Ok((process, thread))
}
fn proof() -> OsContainmentProof {
    OsContainmentProof {
        platform: "windows".into(),
        restricted_identity: true, // OS AppContainer SID in SECURITY_CAPABILITIES
        filesystem_access_limited: true, // profile-owned local storage, no host handles
        network_access_limited: true, // zero declared AppContainer capabilities
        child_contained_before_resume: true, // atomic PROC_THREAD_ATTRIBUTE_JOB_LIST
        process_tree_kill_verified: true, // native query + terminate (not just close)
        memory_quota: true,
        cpu_quota: true,
        pid_quota: true,
        protected_journal_broker: true, // no journal path or handles passed to child
    }
}

/// CI/debug-only progress labels. These carry no source contents, paths,
/// tokens, SIDs, credentials or process IDs. The environment flag changes
/// logging only; it never affects OS admissions or authorization decisions.
fn acceptance_trace(stage: &'static str) {
    if cfg!(debug_assertions)
        && std::env::var_os("LU_IDE_NATIVE_ACCEPTANCE_TRACE").as_deref() == Some(OsStr::new("1"))
    {
        eprintln!("NATIVE_ACCEPTANCE_STAGE={stage}");
    }
}

/// Internal-only Windows native worker. This function is not registered with
/// Tauri or exported through the WebView. It only runs when explicitly built
/// with windows-worker, after native host provisioned a pinned offline bundle
/// and the user performed a fresh, exact-plan approval.
pub fn execute_windows_worker(
    supervisor: &Supervisor,
    job_id: &str,
    plan: &JobPlan,
    current_host_observed_head: &str,
    input: &PublishedInput,
    trusted_offline_toolchain_root: &Path,
    trusted_native_output_parent: &Path,
    trusted_broker: &TrustedBroker,
    cancel: &WindowsCancellation,
) -> Result<WindowsExecutionResult, SupervisorError> {
    // Existing entrypoint preserves the original rule: callers must have
    // obtained an unexpired human grant before invoking worker execution.
    // It MUST fail closed if costly staging consumes the 60-second grant.
    execute_windows_worker_after_staging_approval(
        supervisor,
        job_id,
        plan,
        current_host_observed_head,
        input,
        trusted_offline_toolchain_root,
        trusted_native_output_parent,
        trusted_broker,
        cancel,
        || Ok(()),
    )
}

/// Production host may provide a synchronous native approval callback after
/// sealed input/toolchain preflight and before any child is created, for a
/// genuinely fresh on-screen approval. The acceptance fixture injects an
/// isolated test-only approval; this is not a WebView or Lu Core capability.
/// An untrusted model/tool cannot supply this callback.
pub fn execute_windows_worker_after_staging_approval<F>(
    supervisor: &Supervisor,
    job_id: &str,
    plan: &JobPlan,
    current_host_observed_head: &str,
    input: &PublishedInput,
    trusted_offline_toolchain_root: &Path,
    trusted_native_output_parent: &Path,
    trusted_broker: &TrustedBroker,
    cancel: &WindowsCancellation,
    approve_after_staging: F,
) -> Result<WindowsExecutionResult, SupervisorError>
where
    F: FnOnce() -> Result<(), SupervisorError>,
{
    if !valid_job_id(job_id)
        || plan.source_head != current_host_observed_head
        || plan.input_snapshot_sha256 != input.seal.source_sha256
        || plan.manifest_sha256 != input.seal.manifest_sha256
    {
        return Err(SupervisorError::WrongProjectOrSource);
    }
    verify_input(input).map_err(|_| SupervisorError::WrongProjectOrSource)?;
    acceptance_trace("input_seal_verified");
    // No AppContainer creation, one-shot grant consumption or process launch
    // happens until the entire bundle has been sealed and checked.
    let profile = AppProfile::create(job_id)?;
    acceptance_trace("appcontainer_profile_created");
    let local = profile.local_state()?;
    let workspace = local.join("workspace");
    let toolchain = local.join("toolchain");
    let output = local.join("output");
    if !clean_meta(trusted_native_output_parent)?.is_dir() {
        return Err(denied());
    }
    let private_parent = fs::canonicalize(trusted_native_output_parent).map_err(io_denied)?;
    if private_parent.starts_with(&local)
        || local.starts_with(&private_parent)
        || private_parent.starts_with(&input.root)
    {
        return Err(denied());
    }
    stage_workspace(input, &workspace)?;
    acceptance_trace("workspace_staged");
    stage_toolchain(
        trusted_offline_toolchain_root,
        &toolchain,
        &plan.toolchain_sha256,
    )?;
    acceptance_trace("toolchain_staged");
    fs::create_dir(&output).map_err(io_denied)?;
    let (exe, mut command) = command_for(plan, &toolchain, &workspace)?;
    let env = bootstrap_environment(&local, &workspace, &output, &toolchain)?;
    // Journal is never placed inside the child-controlled AppContainer tree.
    let journal_canonical = supervisor
        .journal_path
        .canonicalize()
        .or_else(|_| {
            supervisor
                .journal_path
                .parent()
                .ok_or_else(|| io::Error::other("no native journal parent"))
                .and_then(fs::canonicalize)
        })
        .map_err(io_denied)?;
    if journal_canonical.starts_with(&local) {
        return Err(denied());
    }
    // D7C: revalidate the exact bytes to be executed immediately before
    // consuming one-shot authority. This is deliberately performed against
    // the AppContainer-owned staged copy, not the original project path.
    // The native profile still requires OS ACL hardening against hostile
    // same-account write access before production acceptance.
    let final_input = PublishedInput {
        root: workspace.clone(),
        seal: input.seal.clone(),
    };
    verify_input(&final_input).map_err(|_| SupervisorError::WrongProjectOrSource)?;
    if trusted_offline_toolchain_digest(&toolchain)? != plan.toolchain_sha256 {
        return Err(SupervisorError::WrongProjectOrSource);
    }
    acceptance_trace("staged_digests_verified");
    // Source identity and the exact native execution bundle are now sealed.
    // Only NOW request/consume fresh one-use authority. No untrusted program
    // has been started, and the 60-second grant TTL remains unchanged.
    approve_after_staging()?;
    acceptance_trace("fresh_grant_issued_after_staging");
    let plan_hash = supervisor
        .inspect()?
        .into_iter()
        .find(|x| x.job_id == job_id)
        .ok_or(SupervisorError::NoSuchJob)?
        .plan_sha256;
    let job = Job::create(job_id, &plan_hash, plan)?;
    acceptance_trace("kernel_job_created");
    if !proof().complete() {
        return Err(denied());
    }
    // Independent kernel Job holder MUST acknowledge readiness and fsync
    // its own witness before the one-shot grant can be consumed.
    let mut broker = BrokerLease::begin(job.handle.0, job_id, &plan_hash, trusted_broker)?;
    acceptance_trace("protected_broker_fsync_ready");
    supervisor.consume_grant_after_attestation(job_id, plan, &proof())?;
    acceptance_trace("one_use_grant_consumed");
    let (process, thread) = match launch_appcontainer(
        &profile,
        &job,
        &exe,
        &mut command,
        &workspace,
        &env,
        &output,
    ) {
        Ok(value) => value,
        Err(reason) => {
            // An OS failure that cannot prove the suspended process was
            // reaped is quarantined immediately. Do not falsely certify
            // "no child executed" or permit a second job in this session.
            if reason == SupervisorError::Quarantined {
                let _ = supervisor.native_quarantine_after_error(job_id);
            } else {
                supervisor.native_deny_before_start(job_id, true)?;
            }
            return Err(reason);
        }
    };
    // The separate broker must remain live across the suspended child
    // admission. A dead broker cannot be waved through as containment.
    acceptance_trace("suspended_appcontainer_attested");
    if !broker.alive() {
        let _ = job.kill_and_verify();
        let _ = supervisor.native_quarantine_after_error(job_id);
        return Err(SupervisorError::Quarantined);
    }
    // Record the contained process BEFORE ResumeThread, so a host crash in
    // this window is quarantined rather than falsely accepted as success.
    if let Err(reason) = supervisor.native_mark_running(job_id) {
        let _ = job.kill_and_verify();
        let _ = supervisor.native_quarantine_after_error(job_id);
        return Err(reason);
    }
    if unsafe { ResumeThread(thread.0) } == u32::MAX {
        let reaped = matches!(job.kill_and_verify(), Ok(true));
        if !reaped {
            let _ = supervisor.native_quarantine_after_error(job_id);
            return Err(SupervisorError::Quarantined);
        }
        let bounded = output_seal(&output).is_ok();
        let _ = supervisor.native_finish(job_id, false, reaped, bounded);
        return Err(denied());
    }
    let begin = Instant::now();
    let mut timed_out = false;
    let mut stopped = false;
    let exit_code = loop {
        let waited = unsafe { WaitForSingleObject(process.0, LOOP_DELAY.as_millis() as u32) };
        if waited == WAIT_OBJECT_0 {
            let mut code = 1_u32;
            if unsafe { GetExitCodeProcess(process.0, &mut code) } == 0 {
                let _ = supervisor.native_quarantine_after_error(job_id);
                return Err(denied());
            }
            break Some(code);
        }
        if waited != WAIT_TIMEOUT {
            let _ = supervisor.native_quarantine_after_error(job_id);
            return Err(denied());
        }
        if !broker.alive() {
            let _ = job.kill_and_verify();
            let _ = supervisor.native_quarantine_after_error(job_id);
            return Err(SupervisorError::Quarantined);
        }
        let request = cancel.0.load(Ordering::SeqCst);
        if request
            || begin.elapsed().as_millis() >= plan.runtime_limit_ms as u128
            || output_seal(&output).is_err()
        {
            timed_out = !request;
            stopped = true;
            let mut terminator = JobTerminator {
                job: &job,
                id: job_id,
                hash: &plan_hash,
            };
            if let Err(reason) = supervisor.cancel_running_with(job_id, &mut terminator) {
                let _ = supervisor.native_quarantine_after_error(job_id);
                return Err(reason);
            }
            if unsafe { WaitForSingleObject(process.0, 5000) } != WAIT_OBJECT_0 {
                return Err(SupervisorError::Quarantined);
            }
            break None;
        }
    };
    // All descendants must exit or be killed, including after the Cargo
    // parent process exits. Job Object handle remains open through receipt.
    let naturally_empty = job.await_empty(REAP_LIMIT);
    let reaped = if naturally_empty {
        true
    } else {
        match job.kill_and_verify() {
            Ok(value) => value,
            Err(reason) => {
                let _ = supervisor.native_quarantine_after_error(job_id);
                return Err(reason);
            }
        }
    };
    if !reaped {
        let _ = supervisor.native_quarantine_after_error(job_id);
        return Err(SupervisorError::Quarantined);
    }
    // Never publish incomplete or cancelled outputs. An unbounded or
    // unpublishable output tree yields a quarantined recovery receipt,
    // not a 'successful build' based only on Cargo's exit status.
    let publication = if stopped {
        None
    } else {
        publish_output(&output, &private_parent, job_id).ok()
    };
    let summary = if stopped {
        supervisor
            .inspect()?
            .into_iter()
            .find(|j| j.job_id == job_id)
            .ok_or(SupervisorError::NoSuchJob)?
    } else {
        supervisor.native_finish_with_output(
            job_id,
            exit_code == Some(0) && naturally_empty && publication.is_some(),
            reaped,
            publication.is_some(),
            publication.as_ref().map(|(_, _, sha)| sha.as_str()),
        )?
    };
    // Only after the supervisor terminal receipt is durable and the
    // kernel Job is verified empty may the recovery keeper be released.
    broker.release_empty()?;
    let (output_root, output_bytes, output_sha256) =
        publication.unwrap_or((PathBuf::new(), 0, String::new()));
    Ok(WindowsExecutionResult {
        summary,
        exit_code,
        output_root,
        output_bytes,
        output_sha256,
        timed_out,
    })
}

/// Recovery never relaunches Cargo. It only reconciles a currently existing
/// exact-name Windows kernel Job Object, then records verified cleanup. A
/// missing object cannot prove that a previous process tree was killed and
/// remains quarantined; a reboot or close-on-crash is NOT an invented receipt.
#[cfg(not(feature = "protected-recovery-service"))]
pub fn reconcile_windows_interrupted(
    supervisor: &Supervisor,
    id: &str,
) -> Result<JobSummary, SupervisorError> {
    let state = supervisor
        .inspect()?
        .into_iter()
        .find(|j| j.job_id == id)
        .ok_or(SupervisorError::NoSuchJob)?;
    if !state.requires_reconciliation {
        return Err(SupervisorError::InvalidTransition);
    }
    let name = job_name(id, &state.plan_sha256)?;
    let raw = unsafe {
        OpenJobObjectW(
            JOB_OBJECT_QUERY_ACCESS | JOB_OBJECT_TERMINATE_ACCESS,
            0,
            name.as_ptr(),
        )
    };
    let job = Job {
        handle: Handle::new(raw).map_err(|_| SupervisorError::Quarantined)?,
    };
    let mut terminator = JobTerminator {
        job: &job,
        id,
        hash: &state.plan_sha256,
    };
    let summary = supervisor.reconcile_interrupted_with(id, &mut terminator)?;
    // The OS process tree was independently reaped and the supervisor
    // recovery receipt is durable. Only now hint that the keeper may
    // relinquish its named handle; failure is not proof of an unsafe tree
    // and simply leaves the broker to expire its bounded recovery hold.
    if summary.phase == JobPhase::Cancelled && !summary.requires_reconciliation {
        #[cfg(not(feature = "protected-recovery-service"))]
        let _ = crate::windows_broker::release_after_verified_recovery(id, &state.plan_sha256);
        // Protected service retains its own kernel witness until bounded
        // hold expiry; it does not honor same-user named release Events.
    }
    Ok(summary)
}

/// D7D two-phase restart reconciliation. The restarted host retains the
/// original kernel Job handle across the service's fsynced release ACK.
/// The supervisor's in-memory AND replayed quarantine remain active until
/// that ACK and a second native Job-tree verification both succeed.
#[cfg(feature = "protected-recovery-service")]
pub fn reconcile_windows_interrupted_via_service(
    supervisor: &Supervisor,
    id: &str,
    trusted_service: &TrustedBroker,
) -> Result<JobSummary, SupervisorError> {
    let state = supervisor
        .inspect()?
        .into_iter()
        .find(|j| j.job_id == id)
        .ok_or(SupervisorError::NoSuchJob)?;
    if !state.requires_reconciliation {
        return Err(SupervisorError::InvalidTransition);
    }
    let name = job_name(id, &state.plan_sha256)?;
    let raw = unsafe {
        OpenJobObjectW(
            JOB_OBJECT_QUERY_ACCESS | JOB_OBJECT_TERMINATE_ACCESS,
            0,
            name.as_ptr(),
        )
    };
    let job = Job {
        handle: Handle::new(raw).map_err(|_| SupervisorError::Quarantined)?,
    };
    // A missing named Job, a stale witness or an unverified tree cannot
    // become a terminal cleanup receipt.
    if !matches!(job.empty(), Ok(true)) {
        if !matches!(job.kill_and_verify(), Ok(true)) {
            return Err(SupervisorError::Quarantined);
        }
    }
    supervisor.mark_protected_recovery_pending(id)?;
    crate::windows_service_client::release_after_verified_recovery(
        id,
        &state.plan_sha256,
        trusted_service,
    )
    .map_err(|_| SupervisorError::Quarantined)?;
    let mut terminator = JobTerminator {
        job: &job,
        id,
        hash: &state.plan_sha256,
    };
    let summary = supervisor.reconcile_interrupted_with(id, &mut terminator)?;
    if summary.phase != JobPhase::Cancelled || summary.requires_reconciliation {
        return Err(SupervisorError::Quarantined);
    }
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_job_names_and_fixed_cpu_rate() {
        assert!(job_name(&format!("job-{}", "a".repeat(32)), &"b".repeat(64)).is_ok());
        assert!(job_name("user-command", &"b".repeat(64)).is_err());
        assert_eq!(CPU_RATE_HARD_CAP, 5);
        assert_eq!(CPU_RATE_HALF, 5000);
    }
    fn test_tree() -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("lu-ide-d7b-hostile-{}-{stamp}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn hostile_hardlink_cannot_copy_external_secret() {
        let root = test_tree();
        let source = root.join("workspace");
        fs::create_dir(&source).unwrap();
        let private = root.join("outside-secret.txt");
        fs::write(&private, b"PRIVATE SECRET NOT A WORKSPACE FILE").unwrap();
        fs::hard_link(&private, source.join("alias.txt")).unwrap();
        let staged = root.join("staged");
        assert!(
            digest_tree(&source, Some(&staged), 100, 100_000).is_err(),
            "hardlinked outside secrets must be denied by kernel handle count"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn hostile_junction_and_reparse_parent_are_rejected() {
        let root = test_tree();
        let workspace = root.join("workspace");
        let outside = root.join("outside");
        fs::create_dir_all(&workspace).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("host-secret.txt"), b"HOST_ONLY").unwrap();
        std::os::windows::fs::symlink_dir(&outside, workspace.join("junction"))
            .expect("native Windows test host must permit symlink creation");
        assert!(
            digest_tree(&workspace, Some(&root.join("staged")), 100, 100_000).is_err(),
            "directory reparse must never be traversed"
        );
        assert!(
            digest_tree(&workspace.join("junction"), None, 100, 100_000).is_err(),
            "reparse root must fail before enumeration"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn staged_content_change_and_output_hardlink_are_denied() {
        let root = test_tree();
        let source = root.join("trusted");
        let dest = root.join("stage");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("main.rs"), b"fn main() {}").unwrap();
        let seal = digest_tree(&source, Some(&dest), 100, 100_000).unwrap();
        assert_eq!(digest_tree(&dest, None, 100, 100_000).unwrap(), seal);
        fs::write(dest.join("main.rs"), b"fn main() { panic!() }").unwrap();
        assert_ne!(digest_tree(&dest, None, 100, 100_000).unwrap(), seal);
        let output = root.join("output");
        fs::create_dir_all(&output).unwrap();
        let outside = root.join("outside.dat");
        fs::write(&outside, b"host data").unwrap();
        fs::hard_link(&outside, output.join("host-hardlink")).unwrap();
        assert!(output_seal(&output).is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn adversarial_parent_swap_cannot_pass_reparse_validation() {
        let root = test_tree();
        let source = root.join("source");
        let outside = root.join("outside");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(source.join("Cargo.toml"), b"package metadata").unwrap();
        fs::write(outside.join("Cargo.toml"), b"OUTSIDE").unwrap();
        let before = digest_tree(&source, None, 100, 100_000).unwrap();
        fs::rename(&source, root.join("source-moved")).unwrap();
        std::os::windows::fs::symlink_dir(&outside, &source)
            .expect("native Windows test host must permit symlink creation");
        assert!(digest_tree(&source, None, 100, 100_000).is_err());
        fs::remove_dir(&source).unwrap();
        fs::rename(root.join("source-moved"), &source).unwrap();
        assert_eq!(digest_tree(&source, None, 100, 100_000).unwrap(), before);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn pinned_bundle_digest_detects_mutation_and_rejects_reuse() {
        let root = test_tree();
        let src = root.join("bundle");
        fs::create_dir_all(src.join("bin")).unwrap();
        fs::create_dir_all(src.join("lib")).unwrap();
        fs::create_dir_all(src.join("cargo-home")).unwrap();
        fs::write(src.join("bin/cargo.exe"), b"known-pinned-cargo").unwrap();
        fs::write(src.join("bin/rustc.exe"), b"known-pinned-rustc").unwrap();
        let expected = trusted_offline_toolchain_digest(&src).unwrap();
        let first = root.join("first");
        stage_toolchain(&src, &first, &expected).unwrap();
        assert_eq!(digest_tree(&first, None, 100, 100_000).unwrap(), expected);
        fs::write(src.join("bin/cargo.exe"), b"unexpected-cargo-update").unwrap();
        assert_eq!(
            stage_toolchain(&src, &root.join("second"), &expected).unwrap_err(),
            SupervisorError::WrongProjectOrSource
        );
        let _ = fs::remove_dir_all(root);
    }

    // The original same-user kernel-Job recovery entrypoint is deliberately
    // absent in protected-service builds, where release requires the SCM ACK.
    #[cfg(not(feature = "protected-recovery-service"))]
    #[test]
    fn quarantined_restart_cannot_invent_cleanup_for_missing_job() {
        let root = test_tree();
        let journal = root.join("receipts.jsonl");
        let supervisor = Supervisor::open(journal.clone()).unwrap();
        let plan = JobPlan {
            project_id: format!("ide-{}", "a".repeat(32)),
            recipe_id: "rust_check".into(),
            source_head: "b".repeat(40),
            input_snapshot_sha256: "c".repeat(64),
            manifest_sha256: "d".repeat(64),
            toolchain_sha256: "e".repeat(64),
            runtime_limit_ms: 3000,
            memory_limit_bytes: 128 * 1024 * 1024,
            process_limit: 3,
            log_limit_bytes: 65536,
            effect_class: "test_build_isolated".into(),
        };
        let prepared = supervisor.prepare(plan.clone()).unwrap();
        supervisor
            .record_human_approval(
                &prepared.job_id,
                &prepared.plan_sha256,
                &plan.source_head,
                true,
            )
            .unwrap();
        supervisor
            .consume_grant_after_attestation(&prepared.job_id, &plan, &proof())
            .unwrap();
        supervisor.native_mark_running(&prepared.job_id).unwrap();
        drop(supervisor);
        let restarted = Supervisor::open(journal).unwrap();
        assert!(restarted.inspect().unwrap()[0].requires_reconciliation);
        // No kernel Job was actually created, so restart may never infer a
        // cancelled/clean outcome from a missing named Job Object.
        assert_eq!(
            reconcile_windows_interrupted(&restarted, &prepared.job_id).unwrap_err(),
            SupervisorError::Quarantined
        );
        let after = restarted.inspect().unwrap();
        assert!(after[0].requires_reconciliation);
        assert_ne!(after[0].phase, JobPhase::Cancelled);
        drop(restarted);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_reparse_and_unknown_offline_toolchain() {
        let base = std::env::temp_dir().join(format!("lu-ide-d7-fixture-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        assert!(stage_toolchain(&base, &base.join("stage"), &"a".repeat(64)).is_err());
        let _ = fs::remove_dir_all(base);
    }
}
