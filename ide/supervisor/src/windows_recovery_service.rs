//! D7D: Windows SCM-only recovery service. No Tauri or project-command entrypoint.
//!
//! This service is a second trust principal, not a child of the desktop process.
//! A successful authenticated admission pins a kernel Job handle BEFORE the
//! suspended AppContainer child resumes. Service crashes do not certify cleanup.
//! Missing named Jobs and missing signed evidence always remain quarantined.
use crate::{windows_service_security as security, SupervisorError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    ffi::c_void,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    mem::size_of,
    os::windows::fs::MetadataExt,
    path::{Path, PathBuf},
    ptr,
    sync::{Mutex, OnceLock},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, GetLastError, HANDLE, ERROR_PIPE_CONNECTED, INVALID_HANDLE_VALUE,
                 WAIT_OBJECT_0, WAIT_TIMEOUT},
    Storage::FileSystem::{ReadFile, WriteFile},
    System::{
        JobObjects::{OpenJobObjectW, QueryInformationJobObject, TerminateJobObject,
            JobObjectBasicAccountingInformation, JOBOBJECT_BASIC_ACCOUNTING_INFORMATION},
        Pipes::{ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PeekNamedPipe,
            PIPE_ACCESS_DUPLEX, PIPE_READMODE_MESSAGE, PIPE_TYPE_MESSAGE, PIPE_WAIT,
            PIPE_REJECT_REMOTE_CLIENTS},
        Services::{RegisterServiceCtrlHandlerExW, SetServiceStatus,
            StartServiceCtrlDispatcherW, SERVICE_RUNNING, SERVICE_START_PENDING,
            SERVICE_STOPPED, SERVICE_STATUS, SERVICE_TABLE_ENTRYW, SERVICE_WIN32_OWN_PROCESS},
        Threading::WaitForSingleObject,
    },
};
const NAME: &str = "LuIdeRecoveryBroker";
const JOB_QUERY: u32 = 0x0004;
const JOB_TERMINATE: u32 = 0x0008;
const SYNCHRONIZE: u32 = 0x00100000;
const PIPE_FIRST_INSTANCE: u32 = 0x00080000;
const MAX_MESSAGE: usize = 2048;
const MAX_EVIDENCE: u64 = 128 * 1024;
const HOLD: Duration = Duration::from_secs(15 * 60);
const TICK: Duration = Duration::from_millis(100);
fn denied() -> SupervisorError { SupervisorError::NoContainment }

struct Handle(HANDLE);
impl Handle {
    fn new(raw: HANDLE) -> Result<Self, SupervisorError> {
        if raw.is_null() || raw == INVALID_HANDLE_VALUE { Err(denied()) } else { Ok(Self(raw)) }
    }
}
impl Drop for Handle {
    fn drop(&mut self) { unsafe { CloseHandle(self.0); } }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InstalledConfig {
    schema: String,
    service_name: String,
    expected_service_sha256: String,
    expected_host_sha256: String,
    expected_host_executable: PathBuf,
    evidence_root: PathBuf,
    endpoint: String,
    acceptance_unsigned: bool,
}
impl InstalledConfig {
    fn read() -> Result<Self, SupervisorError> {
        let root = PathBuf::from(std::env::var_os("ProgramData").ok_or_else(denied)?)
            .join("LuIDE").join("RecoveryBroker");
        if !root.is_absolute() || fs::symlink_metadata(&root).map_err(|_| denied())?
            .file_attributes() & 0x400 != 0 { return Err(denied()); }
        let path = root.join("service.json");
        let meta = fs::symlink_metadata(&path).map_err(|_| denied())?;
        if !meta.is_file() || meta.file_attributes() & 0x400 != 0 || meta.len() > 8192 {
            return Err(denied());
        }
        let mut bytes = Vec::new();
        File::open(&path).map_err(|_| denied())?.take(8193)
            .read_to_end(&mut bytes).map_err(|_| denied())?;
        if bytes.len() > 8192 { return Err(denied()); }
        let config: Self = serde_json::from_slice(&bytes).map_err(|_| denied())?;
        if config.schema != "lu.ide.windows.recovery-service.v1"
            || config.service_name != NAME
            || config.endpoint != security::SERVICE_ENDPOINT
            || !config.evidence_root.is_absolute()
            || fs::canonicalize(&config.evidence_root).map_err(|_| denied())?
                != fs::canonicalize(&root).map_err(|_| denied())?
            || !config.expected_host_executable.is_absolute() {
            return Err(denied());
        }
        // acceptance_unsigned is a deployment audit label, never a runtime bypass.
        let _ = config.acceptance_unsigned;
        security::hash_executable(
            &std::env::current_exe().map_err(|_| denied())?,
            &config.expected_service_sha256,
        )?;
        security::hash_executable(
            &config.expected_host_executable, &config.expected_host_sha256,
        )?;
        security::verify_private_evidence_root(&root)?;
        Ok(config)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Admission {
    schema: String,
    operation: String,
    job_id: String,
    plan_sha256: String,
    owner_pid: u32,
    owner_creation: u64,
    nonce: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Response<'a> { schema: &'static str, state: &'a str, nonce: &'a str }
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Evidence<'a> {
    schema: &'static str,
    sequence: u64,
    previous_sha256: &'a str,
    job_id: &'a str,
    plan_sha256: &'a str,
    owner_pid: u32,
    owner_creation: u64,
    nonce_sha256: &'a str,
    state: &'a str,
    unix_ms: u128,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EvidenceLine<'a> { evidence: Evidence<'a>, entry_sha256: String }
struct Witness {
    file: File,
    sequence: u64,
    previous: String,
    job_id: String,
    plan_sha256: String,
    pid: u32,
    creation: u64,
    nonce_sha256: String,
}
impl Witness {
    fn new(root: &Path, request: &Admission) -> Result<Self, SupervisorError> {
        let path = root.join(format!("{}-{}.protected.jsonl",
            request.job_id, request.plan_sha256));
        let file = OpenOptions::new().write(true).create_new(true)
            .open(path).map_err(|_| denied())?;
        file.sync_all().map_err(|_| denied())?;
        Ok(Self {
            file, sequence: 0, previous: "0".repeat(64),
            job_id: request.job_id.clone(), plan_sha256: request.plan_sha256.clone(),
            pid: request.owner_pid, creation: request.owner_creation,
            nonce_sha256: format!("{:x}", Sha256::digest(request.nonce.as_bytes())),
        })
    }
    fn record(&mut self, state: &str) -> Result<(), SupervisorError> {
        if !matches!(state, "ready" | "owner_exited" | "kill_requested"
            | "tree_empty" | "kill_unverified" | "released_empty" | "hold_expired") {
            return Err(denied());
        }
        let when = SystemTime::now().duration_since(UNIX_EPOCH)
            .map_err(|_| denied())?.as_millis();
        let body = Evidence {
            schema: "lu.ide.d7d.protected-service.v1", sequence: self.sequence,
            previous_sha256: &self.previous, job_id: &self.job_id,
            plan_sha256: &self.plan_sha256, owner_pid: self.pid,
            owner_creation: self.creation, nonce_sha256: &self.nonce_sha256,
            state, unix_ms: when,
        };
        let body_bytes = serde_json::to_vec(&body).map_err(|_| denied())?;
        let entry_sha256 = format!("{:x}", Sha256::digest(&body_bytes));
        let line = serde_json::to_vec(&EvidenceLine { evidence: body, entry_sha256: entry_sha256.clone() })
            .map_err(|_| denied())?;
        if self.file.metadata().map_err(|_| denied())?.len() + line.len() as u64 + 1 > MAX_EVIDENCE {
            return Err(denied());
        }
        self.file.write_all(&line).map_err(|_| denied())?;
        self.file.write_all(b"\n").map_err(|_| denied())?;
        self.file.sync_all().map_err(|_| denied())?;
        self.previous = entry_sha256;
        self.sequence += 1;
        Ok(())
    }
}
fn valid_id(id: &str) -> bool {
    id.len() == 36 && id.starts_with("job-")
        && id[4..].bytes().all(|b| b.is_ascii_hexdigit())
}
fn valid_sha(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}
fn validate_request(r: &Admission) -> bool {
    r.schema == "lu.ide.d7d.admission.v1" && r.operation == "admit"
        && valid_id(&r.job_id) && valid_sha(&r.plan_sha256)
        && r.owner_pid != 0 && r.owner_creation != 0
        && r.nonce.len() == 48 && r.nonce.bytes().all(|b| b.is_ascii_hexdigit())
}
fn job_name(job_id: &str, hash: &str) -> Vec<u16> {
    security::ws(&format!("Global\\LuIdeD7-{job_id}-{}", &hash[..16]))
}
fn job_empty(job: HANDLE) -> Result<bool, SupervisorError> {
    let mut accounting = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
    let mut returned = 0u32;
    if unsafe { QueryInformationJobObject(
        job, JobObjectBasicAccountingInformation,
        (&mut accounting as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
        size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32, &mut returned,
    ) } == 0 || returned != size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32 {
        return Err(denied());
    }
    Ok(accounting.ActiveProcesses == 0)
}
fn kill_and_verify(job: HANDLE) -> bool {
    if unsafe { TerminateJobObject(job, 1) } == 0 { return false; }
    let began = Instant::now();
    while began.elapsed() < Duration::from_secs(8) {
        if matches!(job_empty(job), Ok(true)) { return true; }
        thread::sleep(TICK);
    }
    false
}
fn read_message(pipe: HANDLE) -> Result<Vec<u8>, SupervisorError> {
    let mut bytes = [0u8; MAX_MESSAGE];
    let mut size = 0u32;
    if unsafe { ReadFile(pipe, bytes.as_mut_ptr(), MAX_MESSAGE as u32,
        &mut size, ptr::null_mut()) } == 0 || size == 0
        || size as usize >= MAX_MESSAGE {
        return Err(denied());
    }
    Ok(bytes[..size as usize].to_vec())
}
fn send(pipe: HANDLE, nonce: &str, state: &str) -> Result<(), SupervisorError> {
    let bytes = serde_json::to_vec(&Response {
        schema: "lu.ide.d7d.response.v1", state, nonce,
    }).map_err(|_| denied())?;
    let mut written = 0u32;
    if unsafe { WriteFile(pipe, bytes.as_ptr(), bytes.len() as u32,
        &mut written, ptr::null_mut()) } == 0 || written as usize != bytes.len() {
        return Err(denied());
    }
    Ok(())
}
static ACTIVE: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
struct Active(String);
impl Active {
    fn admit(id: String) -> Result<Self, SupervisorError> {
        let mut items = ACTIVE.get_or_init(|| Mutex::new(HashSet::new()))
            .lock().map_err(|_| denied())?;
        if !items.insert(id.clone()) { return Err(denied()); }
        Ok(Self(id))
    }
}
impl Drop for Active {
    fn drop(&mut self) {
        if let Ok(mut items) = ACTIVE.get_or_init(|| Mutex::new(HashSet::new())).lock() {
            items.remove(&self.0);
        }
    }
}
fn serve(pipe: HANDLE, config: &InstalledConfig) -> Result<(), SupervisorError> {
    let request: Admission = serde_json::from_slice(&read_message(pipe)?).map_err(|_| denied())?;
    if !validate_request(&request) { return Err(denied()); }
    // This keeps a live kernel handle to the exact client PID and creation time
    // for the full admission, including after the original process exits.
    let owner = security::authenticate_client(
        pipe, request.owner_pid, request.owner_creation,
        &config.expected_host_executable, &config.expected_host_sha256,
    )?;
    let _active = Active::admit(format!("{}-{}", request.job_id, request.plan_sha256))?;
    let job = Handle::new(unsafe { OpenJobObjectW(
        JOB_QUERY | JOB_TERMINATE | SYNCHRONIZE, 0,
        job_name(&request.job_id, &request.plan_sha256).as_ptr(),
    ) })?;
    if !matches!(job_empty(job.0), Ok(true)) { return Err(denied()); }
    let mut witness = Witness::new(&config.evidence_root, &request)?;
    witness.record("ready")?;
    send(pipe, &request.nonce, "ready")?;
    loop {
        match unsafe { WaitForSingleObject(owner.process_handle(), 0) } {
            WAIT_OBJECT_0 => {
                witness.record("owner_exited")?;
                witness.record("kill_requested")?;
                let killed = kill_and_verify(job.0);
                witness.record(if killed { "tree_empty" } else { "kill_unverified" })?;
                if !killed { return Err(denied()); }
                // Keep original job alive long enough for an independently
                // restarted supervisor to inspect it. A JSON line cannot
                // replace that kernel observation.
                let began = Instant::now();
                while began.elapsed() < HOLD { thread::sleep(Duration::from_secs(1)); }
                witness.record("hold_expired")?;
                return Ok(());
            }
            WAIT_TIMEOUT => {}
            _ => return Err(denied()),
        }
        // A broken connection while the original host still lives is a loss
        // of trusted witness: revoke execution and do not claim success.
        let mut available = 0u32;
        if unsafe { PeekNamedPipe(pipe, ptr::null_mut(), 0, ptr::null_mut(),
            &mut available, ptr::null_mut()) } == 0 {
            witness.record("kill_requested")?;
            let killed = kill_and_verify(job.0);
            witness.record(if killed { "tree_empty" } else { "kill_unverified" })?;
            return Err(denied());
        }
        if available > 0 {
            #[derive(Deserialize)]
            #[serde(rename_all="camelCase", deny_unknown_fields)]
            struct Release { schema: String, operation: String, nonce: String }
            let message: Release = serde_json::from_slice(&read_message(pipe)?)
                .map_err(|_| denied())?;
            if message.schema != "lu.ide.d7d.release.v1"
                || message.operation != "release"
                || message.nonce != request.nonce
                || !matches!(job_empty(job.0), Ok(true)) {
                witness.record("kill_requested")?;
                let _ = kill_and_verify(job.0);
                return Err(denied());
            }
            witness.record("released_empty")?;
            send(pipe, &request.nonce, "released_empty")?;
            return Ok(());
        }
        thread::sleep(TICK);
    }
}
fn serve_one(pipe: Handle, config: &InstalledConfig) {
    let _ = serve(pipe.0, config);
    unsafe { DisconnectNamedPipe(pipe.0); }
}
fn run_service() -> Result<(), SupervisorError> {
    security::assert_restricted_local_service()?;
    let config = InstalledConfig::read()?;
    let acl = security::PipeSecurity::new()?;
    let mut first = true;
    loop {
        // PIPE_REJECT_REMOTE_CLIENTS and FIRST_PIPE_INSTANCE prevent remote
        // callers or same-user prebinding from impersonating the service.
        let mode = PIPE_ACCESS_DUPLEX | if first { PIPE_FIRST_INSTANCE } else { 0 };
        let pipe = Handle::new(unsafe { CreateNamedPipeW(
            security::ws(security::SERVICE_ENDPOINT).as_ptr(),
            mode,
            PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            16, 4096, 4096, 0, &acl.attributes,
        ) })?;
        first = false;
        if unsafe { ConnectNamedPipe(pipe.0, ptr::null_mut()) } == 0
            && unsafe { GetLastError() } != ERROR_PIPE_CONNECTED {
            return Err(denied());
        }
        // Copy only administrator-installed configuration; no request data
        // can change executable or evidence paths.
        let cloned = InstalledConfig {
            schema: config.schema.clone(), service_name: config.service_name.clone(),
            expected_service_sha256: config.expected_service_sha256.clone(),
            expected_host_sha256: config.expected_host_sha256.clone(),
            expected_host_executable: config.expected_host_executable.clone(),
            evidence_root: config.evidence_root.clone(), endpoint: config.endpoint.clone(),
            acceptance_unsigned: config.acceptance_unsigned,
        };
        thread::spawn(move || serve_one(pipe, &cloned));
    }
}
unsafe extern "system" fn control(
    _control: u32, _event_type: u32, _event_data: *mut c_void,
    _context: *mut c_void,
) -> u32 { 0 }
unsafe extern "system" fn service_main(_argc: u32, _argv: *mut *mut u16) {
    let handle = RegisterServiceCtrlHandlerExW(security::ws(NAME).as_ptr(),
        Some(control), ptr::null());
    if handle == 0 { return; }
    let mut status = SERVICE_STATUS {
        dwServiceType: SERVICE_WIN32_OWN_PROCESS,
        dwCurrentState: SERVICE_START_PENDING,
        dwControlsAccepted: 0, // Do not drop a held Job on an ordinary STOP.
        dwWin32ExitCode: 0, dwServiceSpecificExitCode: 0,
        dwCheckPoint: 1, dwWaitHint: 20000,
    };
    if SetServiceStatus(handle, &status) == 0 { return; }
    let result = std::panic::catch_unwind(run_service);
    status.dwCurrentState = SERVICE_STOPPED;
    status.dwWin32ExitCode = if matches!(result, Ok(Ok(()))) { 0 } else { 1 };
    status.dwCheckPoint = 0;
    status.dwWaitHint = 0;
    let _ = SetServiceStatus(handle, &status);
}
pub fn dispatch() -> Result<(), SupervisorError> {
    // An invocation from a desktop user is rejected by SCM dispatcher and
    // cannot act as an independent service identity.
    let name = security::ws(NAME);
    let entries = [
        SERVICE_TABLE_ENTRYW {
            lpServiceName: name.as_ptr() as *mut u16,
            lpServiceProc: Some(service_main),
        },
        SERVICE_TABLE_ENTRYW { lpServiceName: ptr::null_mut(), lpServiceProc: None },
    ];
    if unsafe { StartServiceCtrlDispatcherW(entries.as_ptr()) } == 0 {
        Err(denied())
    } else { Ok(()) }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn admission_fails_on_bad_scope_or_replay_nonce() {
        let valid = Admission {
            schema: "lu.ide.d7d.admission.v1".into(), operation: "admit".into(),
            job_id: format!("job-{}", "a".repeat(32)),
            plan_sha256: "b".repeat(64), owner_pid: 10, owner_creation: 100,
            nonce: "c".repeat(48),
        };
        assert!(validate_request(&valid));
        let mut bad = valid;
        bad.operation = "run".into();
        assert!(!validate_request(&bad));
    }
    #[test]
    fn wire_denies_unknown_fields() {
        let request = format!(
            r#"{{"schema":"lu.ide.d7d.admission.v1","operation":"admit","jobId":"job-{}","planSha256":"{}","ownerPid":1,"ownerCreation":2,"nonce":"{}","shell":"powershell"}}"#,
            "a".repeat(32), "b".repeat(64), "c".repeat(48)
        );
        assert!(serde_json::from_str::<Admission>(&request).is_err());
    }
}
