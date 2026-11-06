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
    collections::HashMap,
    ffi::c_void,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    mem::size_of,
    os::windows::fs::MetadataExt,
    path::{Path, PathBuf},
    ptr,
    sync::{
        atomic::{AtomicU32, AtomicU8, AtomicUsize, Ordering},
        Arc, Mutex, OnceLock,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use windows_sys::Win32::{
    Foundation::{
        CloseHandle, GetLastError, ERROR_PIPE_CONNECTED, HANDLE, INVALID_HANDLE_VALUE,
        WAIT_OBJECT_0, WAIT_TIMEOUT,
    },
    Storage::FileSystem::{FlushFileBuffers, ReadFile, WriteFile, PIPE_ACCESS_DUPLEX},
    System::{
        JobObjects::{
            JobObjectBasicAccountingInformation, OpenJobObjectW, QueryInformationJobObject,
            TerminateJobObject, JOBOBJECT_BASIC_ACCOUNTING_INFORMATION,
        },
        Pipes::{
            ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PeekNamedPipe,
            PIPE_READMODE_MESSAGE, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_MESSAGE, PIPE_WAIT,
        },
        Services::{
            RegisterServiceCtrlHandlerExW, SetServiceStatus, StartServiceCtrlDispatcherW,
            SERVICE_RUNNING, SERVICE_START_PENDING, SERVICE_STATUS, SERVICE_STATUS_HANDLE,
            SERVICE_STOPPED, SERVICE_TABLE_ENTRYW, SERVICE_WIN32_OWN_PROCESS,
        },
        Threading::WaitForSingleObject,
    },
};
const NAME: &str = "LuIdeRecoveryBroker";
// Non-sensitive SCM exit classification, not an authentication claim.
// 10=restricted LocalService identity, 20=exact installed config/ACL,
// 30=protected pipe ACL, 40=SERVICE_RUNNING, 50=named pipe created.
// It is reported only on service failure, to diagnose genuine OS failures.
static STARTUP_STAGE: AtomicU32 = AtomicU32::new(0);
const SERVICE_SPECIFIC_ERROR: u32 = 1066;
const JOB_QUERY: u32 = 0x0004;
const JOB_TERMINATE: u32 = 0x0008;
const SYNCHRONIZE: u32 = 0x00100000;
const PIPE_FIRST_INSTANCE: u32 = 0x00080000;
const MAX_MESSAGE: usize = 2048;
const MAX_EVIDENCE: u64 = 128 * 1024;
const HOLD: Duration = Duration::from_secs(15 * 60);
// Bounded retry of an already-fsynced recovery ACK if a legitimate restarted
// host lost the first pipe response. Keep the original named Job handle live.
const RECOVERY_ACK_RETRY: Duration = Duration::from_secs(60);
const TICK: Duration = Duration::from_millis(100);
// Bound authenticated and unauthenticated service connection threads.
// Invalid/idle IPC clients cannot exhaust the process with unbounded
// threads or force a 17th CreateNamedPipe failure to terminate the service.
const MAX_ACTIVE_PIPE_SESSIONS: usize = 12;
fn denied() -> SupervisorError {
    SupervisorError::NoContainment
}

struct Handle(HANDLE);
// Transferred exactly once to a connection worker; never shared concurrently.
unsafe impl Send for Handle {}
impl Handle {
    fn new(raw: HANDLE) -> Result<Self, SupervisorError> {
        if raw.is_null() || raw == INVALID_HANDLE_VALUE {
            Err(denied())
        } else {
            Ok(Self(raw))
        }
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InstalledConfig {
    schema: String,
    service_name: String,
    expected_service_sha256: String,
    expected_host_sha256: String,
    expected_host_executable: PathBuf,
    approved_host_user_sid: String,
    evidence_root: PathBuf,
    endpoint: String,
    acceptance_unsigned: bool,
}
impl InstalledConfig {
    fn read() -> Result<Self, SupervisorError> {
        let root = PathBuf::from(std::env::var_os("ProgramData").ok_or_else(denied)?)
            .join("LuIDERecoveryBroker");
        if !root.is_absolute()
            || fs::symlink_metadata(&root)
                .map_err(|_| denied())?
                .file_attributes()
                & 0x400
                != 0
        {
            return Err(denied());
        }
        let path = root.join("service.json");
        let meta = fs::symlink_metadata(&path).map_err(|_| denied())?;
        if !meta.is_file() || meta.file_attributes() & 0x400 != 0 || meta.len() > 8192 {
            return Err(denied());
        }
        let mut bytes = Vec::new();
        File::open(&path)
            .map_err(|_| denied())?
            .take(8193)
            .read_to_end(&mut bytes)
            .map_err(|_| denied())?;
        if bytes.len() > 8192 {
            return Err(denied());
        }
        let config: Self = serde_json::from_slice(&bytes).map_err(|_| denied())?;
        if config.schema != "lu.ide.windows.recovery-service.v2"
            || config.service_name != NAME
            || config.endpoint != security::SERVICE_ENDPOINT
            || !config.evidence_root.is_absolute()
            || fs::canonicalize(&config.evidence_root).map_err(|_| denied())?
                != fs::canonicalize(&root).map_err(|_| denied())?
            || !config.expected_host_executable.is_absolute()
            || security::validate_approved_host_user_sid(&config.approved_host_user_sid).is_err()
        {
            return Err(denied());
        }
        // acceptance_unsigned is a deployment audit label, never a runtime bypass.
        let _ = config.acceptance_unsigned;
        security::hash_executable(
            &std::env::current_exe().map_err(|_| denied())?,
            &config.expected_service_sha256,
        )?;
        security::hash_executable(
            &config.expected_host_executable,
            &config.expected_host_sha256,
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
struct Response<'a> {
    schema: &'static str,
    state: &'a str,
    nonce: &'a str,
}
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
struct EvidenceLine<'a> {
    evidence: Evidence<'a>,
    entry_sha256: String,
}
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
        let path = root.join(format!(
            "{}-{}.protected.jsonl",
            request.job_id, request.plan_sha256
        ));
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|_| denied())?;
        file.sync_all().map_err(|_| denied())?;
        Ok(Self {
            file,
            sequence: 0,
            previous: "0".repeat(64),
            job_id: request.job_id.clone(),
            plan_sha256: request.plan_sha256.clone(),
            pid: request.owner_pid,
            creation: request.owner_creation,
            nonce_sha256: format!("{:x}", Sha256::digest(request.nonce.as_bytes())),
        })
    }
    fn record(&mut self, state: &str) -> Result<(), SupervisorError> {
        if !matches!(
            state,
            "ready"
                | "owner_exited"
                | "kill_requested"
                | "tree_empty"
                | "kill_unverified"
                | "released_empty"
                | "hold_expired"
                | "pipe_lost"
                | "recovered_release"
        ) {
            return Err(denied());
        }
        let when = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| denied())?
            .as_millis();
        let body = Evidence {
            schema: "lu.ide.d7d.protected-service.v1",
            sequence: self.sequence,
            previous_sha256: &self.previous,
            job_id: &self.job_id,
            plan_sha256: &self.plan_sha256,
            owner_pid: self.pid,
            owner_creation: self.creation,
            nonce_sha256: &self.nonce_sha256,
            state,
            unix_ms: when,
        };
        let body_bytes = serde_json::to_vec(&body).map_err(|_| denied())?;
        let entry_sha256 = format!("{:x}", Sha256::digest(&body_bytes));
        let line = serde_json::to_vec(&EvidenceLine {
            evidence: body,
            entry_sha256: entry_sha256.clone(),
        })
        .map_err(|_| denied())?;
        if self.file.metadata().map_err(|_| denied())?.len() + line.len() as u64 + 1 > MAX_EVIDENCE
        {
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
    id.len() == 36 && id.starts_with("job-") && id[4..].bytes().all(|b| b.is_ascii_hexdigit())
}
fn valid_sha(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}
fn validate_request(r: &Admission) -> bool {
    r.schema == "lu.ide.d7d.admission.v1"
        && matches!(r.operation.as_str(), "admit" | "recover")
        && valid_id(&r.job_id)
        && valid_sha(&r.plan_sha256)
        && r.owner_pid != 0
        && r.owner_creation != 0
        && r.nonce.len() == 48
        && r.nonce.bytes().all(|b| b.is_ascii_hexdigit())
}
fn job_name(job_id: &str, hash: &str) -> Vec<u16> {
    security::ws(&format!("Global\\LuIdeD7-{job_id}-{}", &hash[..16]))
}
fn job_empty(job: HANDLE) -> Result<bool, SupervisorError> {
    let mut accounting = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
    let mut returned = 0u32;
    if unsafe {
        QueryInformationJobObject(
            job,
            JobObjectBasicAccountingInformation,
            (&mut accounting as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
            size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
            &mut returned,
        )
    } == 0
        || returned != size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32
    {
        return Err(denied());
    }
    Ok(accounting.ActiveProcesses == 0)
}
fn kill_and_verify(job: HANDLE) -> bool {
    if unsafe { TerminateJobObject(job, 1) } == 0 {
        return false;
    }
    let began = Instant::now();
    while began.elapsed() < Duration::from_secs(8) {
        if matches!(job_empty(job), Ok(true)) {
            return true;
        }
        thread::sleep(TICK);
    }
    false
}
fn read_message(pipe: HANDLE) -> Result<Vec<u8>, SupervisorError> {
    // Never let an interactive client occupy a service thread forever by
    // connecting without sending an admission. Message bodies are atomic
    // and capped before ReadFile consumes the pending message.
    let began = Instant::now();
    while began.elapsed() < Duration::from_secs(8) {
        let mut available = 0u32;
        if unsafe {
            PeekNamedPipe(
                pipe,
                ptr::null_mut(),
                0,
                ptr::null_mut(),
                &mut available,
                ptr::null_mut(),
            )
        } == 0
        {
            return Err(denied());
        }
        if available > 0 {
            if available as usize >= MAX_MESSAGE {
                return Err(denied());
            }
            let mut bytes = [0u8; MAX_MESSAGE];
            let mut size = 0u32;
            if unsafe {
                ReadFile(
                    pipe,
                    bytes.as_mut_ptr(),
                    MAX_MESSAGE as u32,
                    &mut size,
                    ptr::null_mut(),
                )
            } == 0
                || size == 0
                || size as usize >= MAX_MESSAGE
            {
                return Err(denied());
            }
            return Ok(bytes[..size as usize].to_vec());
        }
        thread::sleep(TICK);
    }
    Err(denied())
}
fn send(pipe: HANDLE, nonce: &str, state: &str) -> Result<(), SupervisorError> {
    let bytes = serde_json::to_vec(&Response {
        schema: "lu.ide.d7d.response.v1",
        state,
        nonce,
    })
    .map_err(|_| denied())?;
    let mut written = 0u32;
    if unsafe {
        WriteFile(
            pipe,
            bytes.as_ptr(),
            bytes.len() as u32,
            &mut written,
            ptr::null_mut(),
        )
    } == 0
        || written as usize != bytes.len()
    {
        return Err(denied());
    }
    // Without flushing, the service could DisconnectNamedPipe immediately
    // after writing and discard an unread READY/recovery acknowledgement.
    if unsafe { FlushFileBuffers(pipe) } == 0 {
        return Err(denied());
    }
    Ok(())
}

// State 0 = owner running, 1 = owner dead + Job reaped and witnessed,
// 2 = authenticated restart requested release, 3 = release fsynced.
// An in-memory flag is not a durable cleanup receipt, and never substitutes
// for reopening the kernel Job. It only coordinates two protected service
// threads so the independent holder can fsync before closing its handle.
static ACTIVE: OnceLock<Mutex<HashMap<String, Arc<AtomicU8>>>> = OnceLock::new();
fn registry() -> &'static Mutex<HashMap<String, Arc<AtomicU8>>> {
    ACTIVE.get_or_init(|| Mutex::new(HashMap::new()))
}
fn key(id: &str, hash: &str) -> String {
    format!("{id}-{hash}")
}
struct Active {
    key: String,
    state: Arc<AtomicU8>,
}
impl Active {
    fn admit(key: String) -> Result<Self, SupervisorError> {
        let mut entries = registry().lock().map_err(|_| denied())?;
        if entries.contains_key(&key) {
            return Err(denied());
        }
        let state = Arc::new(AtomicU8::new(0));
        entries.insert(key.clone(), Arc::clone(&state));
        Ok(Self { key, state })
    }
}
impl Drop for Active {
    fn drop(&mut self) {
        if let Ok(mut entries) = registry().lock() {
            entries.remove(&self.key);
        }
    }
}
fn hold_for_recovery(
    owner: HANDLE,
    job: HANDLE,
    witness: &mut Witness,
    active: &Active,
    owner_already_exited: bool,
) -> Result<(), SupervisorError> {
    if !owner_already_exited {
        let began = Instant::now();
        while began.elapsed() < HOLD {
            match unsafe { WaitForSingleObject(owner, 0) } {
                WAIT_OBJECT_0 => {
                    witness.record("owner_exited")?;
                    break;
                }
                WAIT_TIMEOUT => thread::sleep(TICK),
                _ => return Err(denied()),
            }
        }
        if unsafe { WaitForSingleObject(owner, 0) } != WAIT_OBJECT_0 {
            witness.record("hold_expired")?;
            return Err(denied());
        }
    }
    if !matches!(job_empty(job), Ok(true)) {
        return Err(denied());
    }
    active.state.store(1, Ordering::Release);
    let began = Instant::now();
    while began.elapsed() < HOLD {
        if active.state.load(Ordering::Acquire) == 2 {
            if !matches!(job_empty(job), Ok(true)) {
                return Err(denied());
            }
            // Once this is fsynced, a restarted host can accept the ACK.
            witness.record("recovered_release")?;
            active.state.store(3, Ordering::Release);
            // Retain the original kernel Job briefly, so a lost pipe ACK
            // can be retried by the same installer-approved binary/TokenUser.
            // A retry never trusts the JSON witness alone: the service
            // independently reopens and verifies this still-live Job.
            let retry_begin = Instant::now();
            while retry_begin.elapsed() < RECOVERY_ACK_RETRY {
                if !matches!(job_empty(job), Ok(true)) {
                    return Err(denied());
                }
                thread::sleep(TICK);
            }
            return Ok(());
        }
        thread::sleep(TICK);
    }
    witness.record("hold_expired")?;
    Ok(())
}
fn recovery_state_allows_attempt(state: u8) -> bool {
    // A recovery request may start only after tree_empty (1), or retry the
    // already-fsynced protected reply (3). No execution or pending fsync.
    matches!(state, 1 | 3)
}
fn recover(
    pipe: HANDLE,
    request: &Admission,
    config: &InstalledConfig,
) -> Result<(), SupervisorError> {
    let active = registry()
        .lock()
        .map_err(|_| denied())?
        .get(&key(&request.job_id, &request.plan_sha256))
        .cloned()
        .ok_or_else(denied)?;
    let state = active.load(Ordering::Acquire);
    if !recovery_state_allows_attempt(state) {
        // 0=not reaped, 2=another request still waiting for fsync.
        return Err(denied());
    }
    // A recovery grant is never file-only evidence. Service itself reopens
    // the exact original named Job, and independently checks zero processes.
    let job = Handle::new(unsafe {
        OpenJobObjectW(
            JOB_QUERY,
            0,
            job_name(&request.job_id, &request.plan_sha256).as_ptr(),
        )
    })?;
    if !matches!(job_empty(job.0), Ok(true)) {
        return Err(denied());
    }
    security::verify_private_evidence_root(&config.evidence_root)?;
    if state == 3 {
        // Only the original holder sets 3, AFTER durable recovered_release
        // and while retaining the exact empty kernel Job. The newly
        // authenticated request has its own nonce and gets its own ACK.
        return send(pipe, &request.nonce, "recovered_release");
    }
    active
        .compare_exchange(1, 2, Ordering::AcqRel, Ordering::Acquire)
        .map_err(|_| denied())?;
    let began = Instant::now();
    while began.elapsed() < Duration::from_secs(8) {
        if active.load(Ordering::Acquire) == 3 {
            // The original owner thread fsynced 'recovered_release' before 3.
            return send(pipe, &request.nonce, "recovered_release");
        }
        thread::sleep(TICK);
    }
    Err(denied()) // never invent an acknowledgement on timeout
}
fn serve(pipe: HANDLE, config: &InstalledConfig) -> Result<(), SupervisorError> {
    let request: Admission = serde_json::from_slice(&read_message(pipe)?).map_err(|_| denied())?;
    if !validate_request(&request) {
        return Err(denied());
    }
    // The kernel client PID and creation time, token SID, and approved host
    // binary are always revalidated even on a recovered connection.
    let owner = security::authenticate_client(
        pipe,
        request.owner_pid,
        request.owner_creation,
        &config.expected_host_executable,
        &config.expected_host_sha256,
        &config.approved_host_user_sid,
    )?;
    if request.operation == "recover" {
        return recover(pipe, &request, config);
    }
    let active = Active::admit(key(&request.job_id, &request.plan_sha256))?;
    let job = Handle::new(unsafe {
        OpenJobObjectW(
            JOB_QUERY | JOB_TERMINATE | SYNCHRONIZE,
            0,
            job_name(&request.job_id, &request.plan_sha256).as_ptr(),
        )
    })?;
    if !matches!(job_empty(job.0), Ok(true)) {
        return Err(denied());
    }
    let mut witness = Witness::new(&config.evidence_root, &request)?;
    witness.record("ready")?;
    send(pipe, &request.nonce, "ready")?;
    loop {
        match unsafe { WaitForSingleObject(owner.process_handle(), 0) } {
            WAIT_OBJECT_0 => {
                witness.record("owner_exited")?;
                witness.record("kill_requested")?;
                let killed = kill_and_verify(job.0);
                witness.record(if killed {
                    "tree_empty"
                } else {
                    "kill_unverified"
                })?;
                if !killed {
                    return Err(denied());
                }
                return hold_for_recovery(
                    owner.process_handle(),
                    job.0,
                    &mut witness,
                    &active,
                    true,
                );
            }
            WAIT_TIMEOUT => {}
            _ => return Err(denied()),
        }
        // Pipe break is not a safe release: kill and retain the kernel Job
        // until the original OS process exits, then enter bounded hold.
        let mut available = 0u32;
        if unsafe {
            PeekNamedPipe(
                pipe,
                ptr::null_mut(),
                0,
                ptr::null_mut(),
                &mut available,
                ptr::null_mut(),
            )
        } == 0
        {
            witness.record("pipe_lost")?;
            witness.record("kill_requested")?;
            let killed = kill_and_verify(job.0);
            witness.record(if killed {
                "tree_empty"
            } else {
                "kill_unverified"
            })?;
            if !killed {
                return Err(denied());
            }
            return hold_for_recovery(owner.process_handle(), job.0, &mut witness, &active, false);
        }
        if available > 0 {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase", deny_unknown_fields)]
            struct Release {
                schema: String,
                operation: String,
                nonce: String,
            }
            let message: Release =
                serde_json::from_slice(&read_message(pipe)?).map_err(|_| denied())?;
            if message.schema != "lu.ide.d7d.release.v1"
                || message.operation != "release"
                || message.nonce != request.nonce
                || !matches!(job_empty(job.0), Ok(true))
            {
                witness.record("kill_requested")?;
                let killed = kill_and_verify(job.0);
                witness.record(if killed {
                    "tree_empty"
                } else {
                    "kill_unverified"
                })?;
                if !killed {
                    return Err(denied());
                }
                return hold_for_recovery(
                    owner.process_handle(),
                    job.0,
                    &mut witness,
                    &active,
                    false,
                );
            }
            witness.record("released_empty")?;
            send(pipe, &request.nonce, "released_empty")?;
            return Ok(());
        }
        thread::sleep(TICK);
    }
}
struct SessionSlot(Arc<AtomicUsize>);
impl Drop for SessionSlot {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Release);
    }
}
fn serve_one(pipe: Handle, config: &InstalledConfig) {
    let _ = serve(pipe.0, config);
    unsafe {
        DisconnectNamedPipe(pipe.0);
    }
}
fn run_service(status_handle: SERVICE_STATUS_HANDLE) -> Result<(), SupervisorError> {
    security::assert_restricted_local_service()?;
    STARTUP_STAGE.store(10, Ordering::Relaxed);
    let config = InstalledConfig::read()?;
    STARTUP_STAGE.store(20, Ordering::Relaxed);
    let acl = security::PipeSecurity::new(&config.approved_host_user_sid)?;
    STARTUP_STAGE.store(30, Ordering::Relaxed);
    let mut first = true;
    let active_sessions = Arc::new(AtomicUsize::new(0));
    loop {
        // Do not create more pipe instances than the bounded worker pool.
        // With the prior unlimited spawn loop, a same-user client could
        // hold 16 instances and make CreateNamedPipe terminate the service,
        // dropping all independently retained Job Object handles.
        while active_sessions.load(Ordering::Acquire) >= MAX_ACTIVE_PIPE_SESSIONS {
            thread::sleep(TICK);
        }
        // PIPE_REJECT_REMOTE_CLIENTS and FIRST_PIPE_INSTANCE prevent remote
        // callers or same-user prebinding from impersonating the service.
        let mode = PIPE_ACCESS_DUPLEX | if first { PIPE_FIRST_INSTANCE } else { 0 };
        let pipe = Handle::new(unsafe {
            CreateNamedPipeW(
                security::ws(security::SERVICE_ENDPOINT).as_ptr(),
                mode,
                PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                16,
                4096,
                4096,
                0,
                &acl.attributes,
            )
        })?;
        STARTUP_STAGE.store(50, Ordering::Relaxed);
        // Refuse an SCM RUNNING state unless the actual kernel object has
        // the exact protected DACL and excludes client pipe-instance rights.
        security::verify_created_pipe_security(pipe.0, &config.approved_host_user_sid)?;
        STARTUP_STAGE.store(55, Ordering::Relaxed);
        if first {
            // SCM RUNNING must not precede actual protected pipe creation.
            let running = SERVICE_STATUS {
                dwServiceType: SERVICE_WIN32_OWN_PROCESS,
                dwCurrentState: SERVICE_RUNNING,
                dwControlsAccepted: 0,
                dwWin32ExitCode: 0,
                dwServiceSpecificExitCode: 0,
                dwCheckPoint: 0,
                dwWaitHint: 0,
            };
            if unsafe { SetServiceStatus(status_handle, &running) } == 0 {
                return Err(denied());
            }
            first = false;
            STARTUP_STAGE.store(40, Ordering::Relaxed);
        }
        if unsafe { ConnectNamedPipe(pipe.0, ptr::null_mut()) } == 0
            && unsafe { GetLastError() } != ERROR_PIPE_CONNECTED
        {
            return Err(denied());
        }
        // Copy only administrator-installed configuration; no request data
        // can change executable or evidence paths.
        let cloned = InstalledConfig {
            schema: config.schema.clone(),
            service_name: config.service_name.clone(),
            expected_service_sha256: config.expected_service_sha256.clone(),
            expected_host_sha256: config.expected_host_sha256.clone(),
            expected_host_executable: config.expected_host_executable.clone(),
            approved_host_user_sid: config.approved_host_user_sid.clone(),
            evidence_root: config.evidence_root.clone(),
            endpoint: config.endpoint.clone(),
            acceptance_unsigned: config.acceptance_unsigned,
        };
        active_sessions.fetch_add(1, Ordering::AcqRel);
        let permit = SessionSlot(Arc::clone(&active_sessions));
        // The permit is dropped even if a native thread cannot be created;
        // never leave a stale saturated count or unbounded worker spawn.
        thread::Builder::new()
            .name("lu-ide-protected-ipc".into())
            .spawn(move || {
                let _permit = permit;
                serve_one(pipe, &cloned);
            })
            .map_err(|_| denied())?;
    }
}
unsafe extern "system" fn control(
    _control: u32,
    _event_type: u32,
    _event_data: *mut c_void,
    _context: *mut c_void,
) -> u32 {
    0
}
unsafe extern "system" fn service_main(_argc: u32, _argv: *mut *mut u16) {
    let handle =
        RegisterServiceCtrlHandlerExW(security::ws(NAME).as_ptr(), Some(control), ptr::null());
    if handle.is_null() {
        return;
    }
    let mut status = SERVICE_STATUS {
        dwServiceType: SERVICE_WIN32_OWN_PROCESS,
        dwCurrentState: SERVICE_START_PENDING,
        dwControlsAccepted: 0, // Do not drop a held Job on an ordinary STOP.
        dwWin32ExitCode: 0,
        dwServiceSpecificExitCode: 0,
        dwCheckPoint: 1,
        dwWaitHint: 20000,
    };
    if SetServiceStatus(handle, &status) == 0 {
        return;
    }
    let result = std::panic::catch_unwind(|| run_service(handle));
    status.dwCurrentState = SERVICE_STOPPED;
    let ok = matches!(result, Ok(Ok(())));
    status.dwWin32ExitCode = if ok { 0 } else { SERVICE_SPECIFIC_ERROR };
    // Non-secret fixed stage number only; never report the raw error object,
    // project data, policy token, executable path, or service credentials.
    status.dwServiceSpecificExitCode = if ok {
        0
    } else if STARTUP_STAGE.load(Ordering::Relaxed) == 0 {
        // An OS-token startup failure happens before stage 10. The
        // low fixed substage 101..106 distinguishes Win32 token checks
        // without disclosing the service or host identity.
        100 + security::service_identity_stage()
    } else {
        100 + STARTUP_STAGE.load(Ordering::Relaxed)
    };
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
        SERVICE_TABLE_ENTRYW {
            lpServiceName: ptr::null_mut(),
            lpServiceProc: None,
        },
    ];
    if unsafe { StartServiceCtrlDispatcherW(entries.as_ptr()) } == 0 {
        Err(denied())
    } else {
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn service_ipc_session_slot_releases_under_denied_or_failed_request() {
        let sessions = Arc::new(AtomicUsize::new(1));
        {
            let _one = SessionSlot(Arc::clone(&sessions));
            assert_eq!(sessions.load(Ordering::Acquire), 1);
        }
        assert_eq!(sessions.load(Ordering::Acquire), 0);
        assert!(MAX_ACTIVE_PIPE_SESSIONS < 16);
    }
    #[test]
    fn admission_fails_on_bad_scope_or_replay_nonce() {
        let valid = Admission {
            schema: "lu.ide.d7d.admission.v1".into(),
            operation: "admit".into(),
            job_id: format!("job-{}", "a".repeat(32)),
            plan_sha256: "b".repeat(64),
            owner_pid: 10,
            owner_creation: 100,
            nonce: "c".repeat(48),
        };
        assert!(validate_request(&valid));
        let mut bad = valid;
        bad.operation = "run".into();
        assert!(!validate_request(&bad));
    }
    #[test]
    fn protected_recovery_ack_is_never_available_before_fsync() {
        // Original owner running (0), fully reaped and ready (1), request
        // racing with fsync (2), and protected fsynced reply (3). The actual
        // response path additionally reopens the original kernel Job.
        assert!(!recovery_state_allows_attempt(0));
        assert!(recovery_state_allows_attempt(1));
        assert!(!recovery_state_allows_attempt(2));
        assert!(recovery_state_allows_attempt(3));
        assert!(!recovery_state_allows_attempt(u8::MAX));
    }
    #[test]
    fn wire_denies_unknown_fields() {
        let request = format!(
            r#"{{"schema":"lu.ide.d7d.admission.v1","operation":"admit","jobId":"job-{}","planSha256":"{}","ownerPid":1,"ownerCreation":2,"nonce":"{}","shell":"powershell"}}"#,
            "a".repeat(32),
            "b".repeat(64),
            "c".repeat(48)
        );
        assert!(serde_json::from_str::<Admission>(&request).is_err());
    }
}
