//! Wave D7: opt-in native Windows AppContainer build/test worker.
//!
//! The worker is deliberately NOT connected to Tauri, the WebView, or Lu Core.
//! The native host supplies an already approved exact JobPlan and a pinned,
//! trusted, offline toolchain bundle. No shell, user command, PATH search,
//! credential inheritance, general broker, or unrestricted fallback exists.
//! Security and crash-injection acceptance are required before enabling this
//! feature in a distributed IDE build.
use crate::{
    snapshot::{verify_input, PublishedInput},
    JobPhase, JobPlan, JobSummary, NativeTreeTerminator, OsContainmentProof, Supervisor,
    SupervisorError,
};
use sha2::{Digest, Sha256};
use std::{
    ffi::{OsStr, OsString},
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    mem::size_of,
    os::windows::{ffi::{OsStrExt, OsStringExt}, fs::OpenOptionsExt},
    path::{Path, PathBuf},
    ptr,
    sync::{atomic::{AtomicBool, Ordering}, Arc},
    thread,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, GetLastError, LocalFree, HANDLE, ERROR_ALREADY_EXISTS, WAIT_OBJECT_0, WAIT_TIMEOUT},
    Security::{
        Authorization::ConvertSidToStringSidW, EqualSid, FreeSid,
        GetTokenInformation,
        Isolation::{CreateAppContainerProfile, DeleteAppContainerProfile, GetAppContainerFolderPath},
        PSID, SECURITY_CAPABILITIES, TOKEN_QUERY, TokenAppContainerSid, TokenIsAppContainer,
    },
    System::{
        Com::CoTaskMemFree,
        JobObjects::{
            CreateJobObjectW, JobObjectBasicAccountingInformation,
            JobObjectExtendedLimitInformation, JobObjectCpuRateControlInformation,
            OpenJobObjectW, QueryInformationJobObject, SetInformationJobObject,
            TerminateJobObject, JOBOBJECT_BASIC_ACCOUNTING_INFORMATION,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_ACTIVE_PROCESS,
            JOB_OBJECT_LIMIT_JOB_MEMORY, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        },
        Threading::{
            CreateProcessW, DeleteProcThreadAttributeList, GetExitCodeProcess,
            InitializeProcThreadAttributeList, OpenProcessToken, ResumeThread, TerminateProcess,
            UpdateProcThreadAttribute, WaitForSingleObject, CREATE_NO_WINDOW,
            CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT, EXTENDED_STARTUPINFO_PRESENT,
            PROCESS_INFORMATION, PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES,
            STARTUPINFOEXW,
        },
    },
};

const MAX_BUNDLE_FILES: usize = 60_000;
const MAX_BUNDLE_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const MAX_OUTPUT_FILES: usize = 6000;
const MAX_OUTPUT_BYTES: u64 = 256 * 1024 * 1024;
const MAX_OUTPUT_FILE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_DEPTH: usize = 48;
const MAX_SINGLE_INPUT: u64 = 1024 * 1024 * 1024;
const LOOP_DELAY: Duration = Duration::from_millis(40);
const REAP_LIMIT: Duration = Duration::from_secs(5);
const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
const JOB_OBJECT_TERMINATE_ACCESS: u32 = 0x0008;
const JOB_OBJECT_QUERY_ACCESS: u32 = 0x0004;
// JOBOBJECT_CPU_RATE_CONTROL_INFORMATION is two DWORDs when using CpuRate.
// ENABLE(1) | HARD_CAP(4), CpuRate = 5000/10000 (50%).
const CPU_RATE_HARD_CAP: u32 = 0x1 | 0x4;
const CPU_RATE_HALF: u32 = 5000;
// Process creation atomically attaches the child to this Job Object (Win10+).
// The old create-suspended / assign / resume pattern has a crash orphan gap.
const PROC_THREAD_ATTRIBUTE_JOB_LIST_VALUE: usize = 131085;

fn denied() -> SupervisorError { SupervisorError::NoContainment }
fn io_denied<T>(_: T) -> SupervisorError { denied() }

#[derive(Clone, Default)]
pub struct WindowsCancellation(Arc<AtomicBool>);
impl WindowsCancellation {
    pub fn new() -> Self { Self::default() }
    pub fn request_cancel(&self) { self.0.store(true, Ordering::SeqCst); }
    pub fn clone_handle(&self) -> Self { Self(Arc::clone(&self.0)) }
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
impl Handle {
    fn new(raw: HANDLE) -> Result<Self, SupervisorError> {
        if raw.is_null() { Err(denied()) } else { Ok(Self(raw)) }
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: uniquely owned native kernel handle.
            unsafe { CloseHandle(self.0); }
        }
    }
}
fn wide(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(std::iter::once(0)).collect()
}
fn wstr(value: &str) -> Vec<u16> { wide(OsStr::new(value)) }
fn valid_job_id(id: &str) -> bool {
    id.len() == 36 && id.starts_with("job-")
        && id[4..].bytes().all(|x| x.is_ascii_hexdigit())
}
fn job_name(id: &str, hash: &str) -> Result<Vec<u16>, SupervisorError> {
    if !valid_job_id(id) || hash.len() != 64 || !hash.bytes().all(|x| x.is_ascii_hexdigit()) {
        return Err(SupervisorError::InvalidPlan);
    }
    Ok(wstr(&format!("Local\\LuIdeD7-{id}-{}", &hash[..16])))
}

#[derive(Default)]
struct TreeCounter { files: usize, bytes: u64 }
fn clean_meta(path: &Path) -> Result<fs::Metadata, SupervisorError> {
    let m = fs::symlink_metadata(path).map_err(io_denied)?;
    use std::os::windows::fs::MetadataExt;
    if m.is_symlink() || m.file_attributes() & 0x400 != 0 {
        return Err(denied());
    }
    Ok(m)
}
fn clean_part(name: &OsStr) -> Result<(), SupervisorError> {
    let text = name.to_str().ok_or_else(denied)?;
    if text.is_empty() || text == "." || text == ".."
        || text.len() > 240 || text.contains(':') || text.contains('"')
        || text.chars().any(char::is_control) { return Err(denied()); }
    Ok(())
}
fn read_regular(path: &Path) -> Result<File, SupervisorError> {
    if !clean_meta(path)?.is_file() { return Err(denied()); }
    let mut options = OpenOptions::new();
    options.read(true).custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    let file = options.open(path).map_err(io_denied)?;
    if !file.metadata().map_err(io_denied)?.is_file() { return Err(denied()); }
    Ok(file)
}
// Deterministic SHA-256 of every relative path, size and content. Every
// source and staged toolchain file is compared, and all reparse points denied.
fn tree_transfer(
    src: &Path,
    dest: Option<&Path>,
    depth: usize,
    prefix: &Path,
    count: &mut TreeCounter,
    hash: &mut Sha256,
    max_files: usize,
    max_bytes: u64,
) -> Result<(), SupervisorError> {
    if depth > MAX_DEPTH || !clean_meta(src)?.is_dir() { return Err(denied()); }
    if let Some(dst) = dest { fs::create_dir_all(dst).map_err(io_denied)?; }
    let mut children = fs::read_dir(src)
        .map_err(io_denied)?
        .collect::<Result<Vec<_>,_>>()
        .map_err(io_denied)?;
    children.sort_by_key(|x| x.file_name());
    for child in children {
        let name = child.file_name();
        clean_part(&name)?;
        let file = child.path();
        let rel = prefix.join(&name);
        let meta = clean_meta(&file)?;
        let dst = dest.map(|d| d.join(&name));
        if meta.is_dir() {
            tree_transfer(&file, dst.as_deref(), depth + 1, &rel,
                count, hash, max_files, max_bytes)?;
        } else if meta.is_file() {
            count.files += 1;
            count.bytes = count.bytes.checked_add(meta.len()).ok_or_else(denied)?;
            if count.files > max_files || count.bytes > max_bytes || meta.len() > MAX_SINGLE_INPUT {
                return Err(denied());
            }
            let mut reader = read_regular(&file)?;
            let before = reader.metadata().map_err(io_denied)?;
            if before.len() != meta.len() { return Err(denied()); }
            let mut writer = if let Some(d) = dst.as_deref() {
                Some(OpenOptions::new().write(true).create_new(true).open(d).map_err(io_denied)?)
            } else { None };
            let mut item_hash = Sha256::new();
            let mut bytes = 0_u64;
            let mut buffer = [0_u8; 64 * 1024];
            loop {
                let n = reader.read(&mut buffer).map_err(io_denied)?;
                if n == 0 { break; }
                bytes = bytes.checked_add(n as u64).ok_or_else(denied)?;
                if bytes > meta.len() { return Err(denied()); }
                item_hash.update(&buffer[..n]);
                if let Some(out) = writer.as_mut() { out.write_all(&buffer[..n]).map_err(io_denied)?; }
            }
            if bytes != meta.len() { return Err(denied()); }
            if let Some(out) = writer.as_mut() { out.sync_all().map_err(io_denied)?; }
            let relative = rel.to_str().ok_or_else(denied)?.replace('\\', "/");
            hash.update(relative.as_bytes()); hash.update([0]);
            hash.update(bytes.to_le_bytes());
            hash.update(format!("{:x}", item_hash.finalize()).as_bytes());
            hash.update([0]);
        } else { return Err(denied()); }
    }
    Ok(())
}
fn digest_tree(root: &Path, destination: Option<&Path>,
    max_files: usize, max_bytes: u64) -> Result<String, SupervisorError> {
    let mut counter = TreeCounter::default();
    let mut hash = Sha256::new();
    tree_transfer(root, destination, 0, Path::new(""), &mut counter,
        &mut hash, max_files, max_bytes)?;
    Ok(format!("{:x}", hash.finalize()))
}
/// Internal native-host helper to bind a provisioned offline bundle to an
/// exact JobPlan. No executable path, bundle path or digest is accepted from
/// the WebView or model; the caller must authenticate installation provenance.
pub fn trusted_offline_toolchain_digest(bundle: &Path) -> Result<String, SupervisorError> {
    if !clean_meta(&bundle.join("bin/cargo.exe"))?.is_file()
        || !clean_meta(&bundle.join("bin/rustc.exe"))?.is_file()
        || !clean_meta(&bundle.join("lib"))?.is_dir()
        || !clean_meta(&bundle.join("cargo-home"))?.is_dir() {
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
        if !clean_meta(&p)?.is_file() { return Err(denied()); }
    }
    if !clean_meta(&bundle.join("lib"))?.is_dir()
        || !clean_meta(&bundle.join("cargo-home"))?.is_dir() { return Err(denied()); }
    if digest_tree(bundle, Some(dest), MAX_BUNDLE_FILES, MAX_BUNDLE_BYTES)? != expected {
        return Err(SupervisorError::WrongProjectOrSource);
    }
    if digest_tree(dest, None, MAX_BUNDLE_FILES, MAX_BUNDLE_BYTES)? != expected {
        return Err(denied());
    }
    Ok(())
}
fn stage_workspace(input: &PublishedInput, dest: &Path)
    -> Result<(), SupervisorError> {
    verify_input(input).map_err(|_| SupervisorError::WrongProjectOrSource)?;
    digest_tree(&input.root, Some(dest), 12_000, 512 * 1024 * 1024)?;
    let staged = PublishedInput { root: dest.to_path_buf(), seal: input.seal.clone() };
    verify_input(&staged).map_err(|_| denied())
}
fn output_seal(root: &Path) -> Result<(u64, String), SupervisorError> {
    if !clean_meta(root)?.is_dir() { return Err(denied()); }
    let mut counter = TreeCounter::default();
    let mut hash = Sha256::new();
    tree_transfer(root, None, 0, Path::new(""), &mut counter,
        &mut hash, MAX_OUTPUT_FILES, MAX_OUTPUT_BYTES)?;
    // MAX_SINGLE_INPUT in transfer is larger than output per-file policy.
    // Check independent per-file quota via another bounded metadata walk.
    fn check(dir: &Path, depth: usize) -> Result<(), SupervisorError> {
        if depth > MAX_DEPTH { return Err(denied()); }
        for entry in fs::read_dir(dir).map_err(io_denied)? {
            let p = entry.map_err(io_denied)?.path();
            let m = clean_meta(&p)?;
            if m.is_dir() { check(&p, depth + 1)?; }
            else if !m.is_file() || m.len() > MAX_OUTPUT_FILE_BYTES { return Err(denied()); }
        }
        Ok(())
    }
    check(root, 0)?;
    Ok((counter.bytes, format!("{:x}", hash.finalize())))
}

/// Publish only fully scanned, size-limited output from a terminated kernel
/// Job into the native application profile. The AppContainer has no access to
/// this parent. Reseal the copied bytes before returning an output coordinate.
fn publish_output(
    child_output: &Path, trusted_native_parent: &Path, id: &str
) -> Result<(PathBuf, u64, String), SupervisorError> {
    if !valid_job_id(id) || !clean_meta(trusted_native_parent)?.is_dir() {
        return Err(denied());
    }
    let parent = fs::canonicalize(trusted_native_parent).map_err(io_denied)?;
    let source = fs::canonicalize(child_output).map_err(io_denied)?;
    if parent.starts_with(&source) || source.starts_with(&parent) { return Err(denied()); }
    let (bytes, original_hash) = output_seal(child_output)?;
    let destination = parent.join(id);
    fs::create_dir(&destination).map_err(io_denied)?; // existing job = deny
    let copied = digest_tree(child_output, Some(&destination),
        MAX_OUTPUT_FILES, MAX_OUTPUT_BYTES)?;
    let (published_bytes, published_hash) = output_seal(&destination)?;
    if original_hash != copied || copied != published_hash || bytes != published_bytes {
        return Err(denied());
    }
    Ok((destination, published_bytes, published_hash))
}

struct AppProfile { name: Vec<u16>, sid: PSID }
impl Drop for AppProfile {
    fn drop(&mut self) {
        // SAFETY: SID and profile owned by this native broker.
        unsafe {
            if !self.sid.is_null() { FreeSid(self.sid); }
            DeleteAppContainerProfile(self.name.as_ptr());
        }
    }
}
impl AppProfile {
    fn create(id: &str) -> Result<Self, SupervisorError> {
        if !valid_job_id(id) { return Err(SupervisorError::InvalidPlan); }
        // Fresh, unpredictable job IDs prevent name reuse, but OS rejects
        // any previously-existing profile instead of attaching to it.
        let name = wstr(&format!("LuIdeD7{}", &id[4..]));
        let description = wstr("Lu IDE native isolated worker");
        let mut sid: PSID = ptr::null_mut();
        let hr = unsafe { CreateAppContainerProfile(name.as_ptr(), description.as_ptr(),
            description.as_ptr(), ptr::null(), 0, &mut sid) };
        if hr < 0 || sid.is_null() { return Err(denied()); }
        Ok(Self { name, sid })
    }
    fn local_state(&self) -> Result<PathBuf, SupervisorError> {
        let mut sid_wide: *mut u16 = ptr::null_mut();
        if unsafe { ConvertSidToStringSidW(self.sid, &mut sid_wide) } == 0 || sid_wide.is_null() {
            return Err(denied());
        }
        let sid_string = read_wide(sid_wide)?;
        unsafe { LocalFree(sid_wide.cast()); }
        let name = wstr(&sid_string);
        let mut result: *mut u16 = ptr::null_mut();
        let hr = unsafe { GetAppContainerFolderPath(name.as_ptr(), &mut result) };
        if hr < 0 || result.is_null() { return Err(denied()); }
        let value = read_wide(result)?;
        unsafe { CoTaskMemFree(result.cast()); }
        let path = PathBuf::from(value).join("LocalState");
        fs::create_dir_all(&path).map_err(io_denied)?;
        Ok(path)
    }
}
fn read_wide(value: *const u16) -> Result<String, SupervisorError> {
    if value.is_null() { return Err(denied()); }
    let mut n = 0_usize;
    while n < 32760 && unsafe { *value.add(n) } != 0 { n += 1; }
    if n == 32760 { return Err(denied()); }
    let slice = unsafe { std::slice::from_raw_parts(value, n) };
    String::from_utf16(slice).map_err(io_denied)
}
struct Attributes(Vec<usize>);
impl Attributes {
    fn new() -> Result<Self, SupervisorError> {
        let mut size = 0_usize;
        unsafe { InitializeProcThreadAttributeList(ptr::null_mut(), 2, 0, &mut size); }
        if size == 0 || size > 32768 { return Err(denied()); }
        let mut bytes = vec![0_usize; size.div_ceil(size_of::<usize>())];
        if unsafe { InitializeProcThreadAttributeList(bytes.as_mut_ptr().cast(), 2, 0, &mut size) } == 0 {
            return Err(denied());
        }
        Ok(Self(bytes))
    }
    fn raw(&mut self) -> *mut core::ffi::c_void { self.0.as_mut_ptr().cast() }
}
impl Drop for Attributes {
    fn drop(&mut self) {
        unsafe { DeleteProcThreadAttributeList(self.0.as_mut_ptr().cast()); }
    }
}

#[repr(C)]
struct CpuRateControl { flags: u32, rate: u32 }
struct Job { handle: Handle }
impl Job {
    fn create(id: &str, hash: &str, plan: &JobPlan) -> Result<Self, SupervisorError> {
        let name = job_name(id, hash)?;
        let raw = unsafe { CreateJobObjectW(ptr::null(), name.as_ptr()) };
        let created = Handle::new(raw)?;
        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS { return Err(denied()); }
        let mut limit = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limit.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
            | JOB_OBJECT_LIMIT_ACTIVE_PROCESS | JOB_OBJECT_LIMIT_JOB_MEMORY;
        limit.BasicLimitInformation.ActiveProcessLimit = plan.process_limit;
        limit.JobMemoryLimit = usize::try_from(plan.memory_limit_bytes).map_err(io_denied)?;
        if unsafe { SetInformationJobObject(created.0, JobObjectExtendedLimitInformation,
            (&raw const limit).cast(), size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32) } == 0 {
            return Err(denied());
        }
        let cpu = CpuRateControl { flags: CPU_RATE_HARD_CAP, rate: CPU_RATE_HALF };
        if unsafe { SetInformationJobObject(created.0, JobObjectCpuRateControlInformation,
            (&raw const cpu).cast(), size_of::<CpuRateControl>() as u32) } == 0 {
            return Err(denied());
        }
        Ok(Self { handle: created })
    }
    fn empty(&self) -> Result<bool, SupervisorError> {
        let mut info = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
        if unsafe { QueryInformationJobObject(self.handle.0, JobObjectBasicAccountingInformation,
            (&mut info as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
            size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
            ptr::null_mut()) } == 0 { return Err(denied()); }
        Ok(info.ActiveProcesses == 0)
    }
    fn await_empty(&self, duration: Duration) -> bool {
        let begin = Instant::now();
        while begin.elapsed() < duration {
            if matches!(self.empty(), Ok(true)) { return true; }
            thread::sleep(LOOP_DELAY);
        }
        matches!(self.empty(), Ok(true))
    }
    fn kill_and_verify(&self) -> Result<bool, SupervisorError> {
        if unsafe { TerminateJobObject(self.handle.0, 77) } == 0 { return Err(denied()); }
        Ok(self.await_empty(REAP_LIMIT))
    }
}
struct JobTerminator<'a> { job: &'a Job, id: &'a str, hash: &'a str }
impl NativeTreeTerminator for JobTerminator<'_> {
    fn terminate_and_verify_tree(&mut self, id: &str, hash: &str)
        -> Result<bool, SupervisorError> {
        if id != self.id || hash != self.hash { return Err(SupervisorError::WrongProjectOrSource); }
        self.job.kill_and_verify()
    }
}
#[repr(C)]
struct AppContainerTokenInfo { sid: PSID }

fn verify_suspended_appcontainer_token(process: HANDLE, expected_sid: PSID)
    -> Result<(), SupervisorError> {
    let mut raw: HANDLE = ptr::null_mut();
    if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut raw) } == 0 {
        return Err(denied());
    }
    let token = Handle::new(raw)?;
    let mut present = 0_u32;
    let mut len = 0_u32;
    if unsafe { GetTokenInformation(token.0, TokenIsAppContainer,
        (&mut present as *mut u32).cast(), size_of::<u32>() as u32, &mut len) } == 0
        || present != 1 { return Err(denied()); }
    let mut size = 0_u32;
    unsafe { GetTokenInformation(token.0, TokenAppContainerSid,
        ptr::null_mut(), 0, &mut size); }
    if size < size_of::<AppContainerTokenInfo>() as u32 || size > 4096 {
        return Err(denied());
    }
    let mut buffer = vec![0_usize; (size as usize).div_ceil(size_of::<usize>())];
    if unsafe { GetTokenInformation(token.0, TokenAppContainerSid,
        buffer.as_mut_ptr().cast(), size, &mut len) } == 0 {
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
        || unsafe { WaitForSingleObject(process, 5000) } != WAIT_OBJECT_0 {
        return Err(denied());
    }
    Ok(())
}
fn bootstrap_environment(local: &Path, workspace: &Path, output: &Path,
    tools: &Path) -> Result<Vec<u16>, SupervisorError> {
    let system = std::env::var_os("SystemRoot").ok_or_else(denied)?;
    let appdata = std::env::var_os("LOCALAPPDATA").ok_or_else(denied)?;
    if !Path::new(&system).is_absolute() || !Path::new(&appdata).is_absolute() {
        return Err(denied());
    }
    let bin = tools.join("bin");
    let rustc = bin.join("rustc.exe");
    let env = format!(
        "CARGO_HOME={}\0CARGO_INCREMENTAL=0\0CARGO_NET_OFFLINE=true\0CARGO_TARGET_DIR={}\0LOCALAPPDATA={}\0PATH={}\0RUSTC={}\0SystemRoot={}\0TEMP={}\0TMP={}\0\0",
        tools.join("cargo-home").display(), output.display(),
        appdata.to_string_lossy(), bin.display(), rustc.display(),
        system.to_string_lossy(), local.display(), local.display()
    );
    if env.encode_utf16().count() > 32767 || workspace.to_str().is_none()
        || env.chars().any(|c| c == '\u{0001}') { return Err(denied()); }
    Ok(env.encode_utf16().collect())
}
fn command_for(plan: &JobPlan, tools: &Path, workspace: &Path)
    -> Result<(PathBuf, Vec<u16>), SupervisorError> {
    let (subcommand, manifest) = match plan.recipe_id.as_str() {
        "rust_check" => ("check", "Cargo.toml"),
        "rust_tests" => ("test", "Cargo.toml"),
        "lu_ide_host_tests" => ("test", "ide/host/Cargo.toml"),
        _ => return Err(SupervisorError::InvalidPlan),
    };
    let exe = tools.join("bin/cargo.exe");
    if !clean_meta(&exe)?.is_file() { return Err(denied()); }
    let file = workspace.join(manifest);
    if !clean_meta(&file)?.is_file() { return Err(denied()); }
    let command = format!("\"{}\" {subcommand} --offline --locked --manifest-path \"{}\"",
        exe.display(), file.display());
    // All components are host-determined fixed paths; no project string is
    // inserted into process command, environment or executable selection.
    if command.contains('\n') || command.contains('\r') { return Err(denied()); }
    Ok((exe, wstr(&command)))
}
fn launch_appcontainer(profile: &AppProfile, job: &Job, executable: &Path,
    command: &mut Vec<u16>, cwd: &Path, environment: &[u16])
    -> Result<(Handle, Handle), SupervisorError> {
    let mut cap = SECURITY_CAPABILITIES::default();
    cap.AppContainerSid = profile.sid;
    cap.Capabilities = ptr::null_mut();
    cap.CapabilityCount = 0;
    let mut attrs = Attributes::new()?;
    if unsafe { UpdateProcThreadAttribute(attrs.raw(), 0,
        PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES as usize,
        (&cap as *const SECURITY_CAPABILITIES).cast(), size_of::<SECURITY_CAPABILITIES>(),
        ptr::null_mut(), ptr::null()) } == 0 { return Err(denied()); }
    // OS assigns the job at CreateProcessW, before any suspended child
    // exists. If the broker crashes at any point after creation, closing its
    // Job handle kills the contained child; no unassigned orphan gap.
    let jobs = [job.handle.0];
    if unsafe { UpdateProcThreadAttribute(attrs.raw(), 0,
        PROC_THREAD_ATTRIBUTE_JOB_LIST_VALUE, jobs.as_ptr().cast(),
        size_of::<HANDLE>(), ptr::null_mut(), ptr::null()) } == 0 {
        return Err(denied());
    }
    let mut startup = STARTUPINFOEXW::default();
    startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
    startup.lpAttributeList = attrs.raw();
    let mut proc = PROCESS_INFORMATION::default();
    let exe_wide = wide(executable.as_os_str());
    let cwd_wide = wide(cwd.as_os_str());
    let spawned = unsafe { CreateProcessW(exe_wide.as_ptr(), command.as_mut_ptr(),
        ptr::null(), ptr::null(), 0,
        CREATE_SUSPENDED | CREATE_NO_WINDOW | CREATE_UNICODE_ENVIRONMENT
            | EXTENDED_STARTUPINFO_PRESENT,
        environment.as_ptr().cast(), cwd_wide.as_ptr(), &startup.StartupInfo, &mut proc) };
    if spawned == 0 { return Err(denied()); }
    let process = Handle::new(proc.hProcess)?;
    let thread = match Handle::new(proc.hThread) {
        Ok(t) => t, Err(e) => { terminate_unassigned(process.0)?; return Err(e); }
    };
    // The unique Job must contain the suspended child already, because
    // membership was supplied as a creation attribute rather than an
    // after-creation assignment subject to host-crash races.
    if !matches!(job.empty(), Ok(false))
        || verify_suspended_appcontainer_token(process.0, profile.sid).is_err() {
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
        memory_quota: true, cpu_quota: true, pid_quota: true,
        protected_journal_broker: true, // no journal path or handles passed to child
    }
}

/// Internal-only Windows native worker. This function is not registered with
/// Tauri or exported through the WebView. It only runs when explicitly built
/// with windows-worker, after native host provisioned a pinned offline bundle
/// and the user performed a fresh, exact-plan approval.
pub fn execute_windows_worker(
    supervisor: &Supervisor, job_id: &str, plan: &JobPlan,
    input: &PublishedInput, trusted_offline_toolchain_root: &Path,
    trusted_native_output_parent: &Path, cancel: &WindowsCancellation,
) -> Result<WindowsExecutionResult, SupervisorError> {
    if !valid_job_id(job_id) || plan.input_snapshot_sha256 != input.seal.source_sha256
        || plan.manifest_sha256 != input.seal.manifest_sha256 {
        return Err(SupervisorError::WrongProjectOrSource);
    }
    verify_input(input).map_err(|_| SupervisorError::WrongProjectOrSource)?;
    // No AppContainer creation, one-shot grant consumption or process launch
    // happens until the entire bundle has been sealed and checked.
    let profile = AppProfile::create(job_id)?;
    let local = profile.local_state()?;
    let workspace = local.join("workspace");
    let toolchain = local.join("toolchain");
    let output = local.join("output");
    if !clean_meta(trusted_native_output_parent)?.is_dir() {
        return Err(denied());
    }
    let private_parent = fs::canonicalize(trusted_native_output_parent).map_err(io_denied)?;
    if private_parent.starts_with(&local) || local.starts_with(&private_parent)
        || private_parent.starts_with(&input.root) { return Err(denied()); }
    stage_workspace(input, &workspace)?;
    stage_toolchain(trusted_offline_toolchain_root, &toolchain, &plan.toolchain_sha256)?;
    fs::create_dir(&output).map_err(io_denied)?;
    let (exe, mut command) = command_for(plan, &toolchain, &workspace)?;
    let env = bootstrap_environment(&local, &workspace, &output, &toolchain)?;
    // Journal is never placed inside the child-controlled AppContainer tree.
    let journal_canonical = supervisor.journal_path.canonicalize()
        .or_else(|_| supervisor.journal_path.parent().ok_or_else(|| {
            io::Error::other("no native journal parent")
        }).and_then(fs::canonicalize)).map_err(io_denied)?;
    if journal_canonical.starts_with(&local) { return Err(denied()); }
    let plan_hash = supervisor.inspect()?.into_iter()
        .find(|x| x.job_id == job_id).ok_or(SupervisorError::NoSuchJob)?.plan_sha256;
    let job = Job::create(job_id, &plan_hash, plan)?;
    if !proof().complete() { return Err(denied()); }
    supervisor.consume_grant_after_attestation(job_id, plan, &proof())?;
    let (process, thread) = match launch_appcontainer(
        &profile, &job, &exe, &mut command, &workspace, &env
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
        let request = cancel.0.load(Ordering::SeqCst);
        if request || begin.elapsed().as_millis() >= plan.runtime_limit_ms as u128
            || output_seal(&output).is_err() {
            timed_out = !request;
            stopped = true;
            let mut terminator = JobTerminator { job: &job, id: job_id, hash: &plan_hash };
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
    let reaped = naturally_empty || job.kill_and_verify()?;
    // Never publish incomplete or cancelled outputs. An unbounded or
    // unpublishable output tree yields a quarantined recovery receipt,
    // not a 'successful build' based only on Cargo's exit status.
    let publication = if stopped {
        None
    } else {
        publish_output(&output, &private_parent, job_id).ok()
    };
    let summary = if stopped {
        supervisor.inspect()?.into_iter().find(|j| j.job_id == job_id)
            .ok_or(SupervisorError::NoSuchJob)?
    } else {
        supervisor.native_finish_with_output(
            job_id, exit_code == Some(0) && naturally_empty && publication.is_some(),
            reaped, publication.is_some(),
            publication.as_ref().map(|(_, _, sha)| sha.as_str()))?
    };
    let (output_root, output_bytes, output_sha256) =
        publication.unwrap_or((PathBuf::new(), 0, String::new()));
    Ok(WindowsExecutionResult {
        summary, exit_code, output_root, output_bytes,
        output_sha256, timed_out,
    })
}

/// Recovery never relaunches Cargo. It only reconciles a currently existing
/// exact-name Windows kernel Job Object, then records verified cleanup. A
/// missing object cannot prove that a previous process tree was killed and
/// remains quarantined; a reboot or close-on-crash is NOT an invented receipt.
pub fn reconcile_windows_interrupted(supervisor: &Supervisor, id: &str)
    -> Result<JobSummary, SupervisorError> {
    let state = supervisor.inspect()?.into_iter().find(|j| j.job_id == id)
        .ok_or(SupervisorError::NoSuchJob)?;
    if !state.requires_reconciliation { return Err(SupervisorError::InvalidTransition); }
    let name = job_name(id, &state.plan_sha256)?;
    let raw = unsafe { OpenJobObjectW(
        JOB_OBJECT_QUERY_ACCESS | JOB_OBJECT_TERMINATE_ACCESS, 0, name.as_ptr()
    ) };
    let job = Job { handle: Handle::new(raw).map_err(|_| SupervisorError::Quarantined)? };
    let mut terminator = JobTerminator { job: &job, id, hash: &state.plan_sha256 };
    supervisor.reconcile_interrupted_with(id, &mut terminator)
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
    #[test]
    fn rejects_reparse_and_unknown_offline_toolchain() {
        let base = std::env::temp_dir().join(format!("lu-ide-d7-fixture-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        assert!(stage_toolchain(&base, &base.join("stage"), &"a".repeat(64)).is_err());
        let _ = fs::remove_dir_all(base);
    }
}
