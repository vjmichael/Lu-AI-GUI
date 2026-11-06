//! D7C independent Windows Job Object recovery broker.
//!
//! A separate native process holds a named kernel Job handle from BEFORE the
//! AppContainer child resumes. It observes an exact opened supervisor process
//! handle, not an easily reused PID, and writes its own fsynced evidence.
//! On parent crash it durably records kill intent, terminates the Job tree,
//! proves ActiveProcesses=0, and HOLDS the Job name for a bounded recovery
//! interval so a newly started supervisor can independently verify it.
//!
//! Security boundary: the executable digest MUST come from a trusted native
//! installation manifest, and receipts MUST reside in an OS-protected broker
//! profile under a separate principal. Same-user JSON/hash chains alone are
//! not authenticated recovery proof; never clear quarantine from a file alone.
use crate::SupervisorError;
use rand::{rngs::OsRng, RngCore};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    ffi::OsStr,
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Read, Write},
    mem::size_of,
    os::windows::{
        ffi::OsStrExt,
        fs::{MetadataExt, OpenOptionsExt},
        process::CommandExt,
    },
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    ptr,
    sync::mpsc,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use windows_sys::Win32::{
    Foundation::{
        CloseHandle, GetLastError, FILETIME, HANDLE, ERROR_ALREADY_EXISTS,
        WAIT_OBJECT_0, WAIT_TIMEOUT,
    },
    System::{
        JobObjects::{
            OpenJobObjectW, QueryInformationJobObject, TerminateJobObject,
            JobObjectBasicAccountingInformation, JOBOBJECT_BASIC_ACCOUNTING_INFORMATION,
        },
        Threading::{
            CreateEventW, GetCurrentProcess, GetProcessTimes, OpenEventW,
            OpenProcess, ResetEvent, SetEvent, WaitForSingleObject,
        },
    },
};

const JOB_QUERY: u32 = 0x0004;
const JOB_TERMINATE: u32 = 0x0008;
const EVENT_MODIFY_STATE: u32 = 0x0002;
const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
const SYNCHRONIZE: u32 = 0x00100000;
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
const FILE_SHARE_READ: u32 = 0x0000_0001;
const READY_LIMIT: Duration = Duration::from_secs(8);
const KILL_LIMIT: Duration = Duration::from_secs(8);
const RECOVERY_HOLD: Duration = Duration::from_secs(15 * 60);
const TICK: Duration = Duration::from_millis(100);
const WITNESS_LIMIT: u64 = 128 * 1024;

fn denied() -> SupervisorError { SupervisorError::NoContainment }
fn wide(s: &OsStr) -> Vec<u16> { s.encode_wide().chain([0]).collect() }
fn os_wide(s: &str) -> Vec<u16> { wide(OsStr::new(s)) }
fn valid_id(id: &str) -> bool {
    id.len() == 36 && id.starts_with("job-")
        && id[4..].bytes().all(|b| b.is_ascii_hexdigit())
}
fn valid_hash(hash: &str) -> bool {
    hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit())
}
fn job_name(id: &str, hash: &str) -> Result<Vec<u16>, SupervisorError> {
    if !valid_id(id) || !valid_hash(hash) { return Err(denied()); }
    Ok(os_wide(&format!("Local\\LuIdeD7-{id}-{}", &hash[..16])))
}
fn release_name(id: &str, hash: &str) -> Result<Vec<u16>, SupervisorError> {
    if !valid_id(id) || !valid_hash(hash) { return Err(denied()); }
    Ok(os_wide(&format!("Local\\LuIdeD7BrokerRelease-{id}-{}", &hash[..16])))
}
struct Handle(HANDLE);
impl Handle {
    fn new(raw: HANDLE) -> Result<Self, SupervisorError> {
        if raw.is_null() { Err(denied()) } else { Ok(Self(raw)) }
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() { unsafe { CloseHandle(self.0); } }
    }
}
fn filetime_value(value: FILETIME) -> u64 {
    ((value.dwHighDateTime as u64) << 32) | value.dwLowDateTime as u64
}
fn process_started_at(process: HANDLE) -> Result<u64, SupervisorError> {
    let (mut created, mut exited, mut kernel, mut user) =
        (FILETIME::default(), FILETIME::default(), FILETIME::default(), FILETIME::default());
    if unsafe { GetProcessTimes(process, &mut created, &mut exited, &mut kernel, &mut user) } == 0 {
        return Err(denied());
    }
    Ok(filetime_value(created))
}
pub(crate) fn current_owner_creation() -> Result<u64, SupervisorError> {
    process_started_at(unsafe { GetCurrentProcess() })
}
fn job_empty(handle: HANDLE) -> Result<bool, SupervisorError> {
    let mut status = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
    if unsafe {
        QueryInformationJobObject(handle, JobObjectBasicAccountingInformation,
            (&mut status as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
            size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
            ptr::null_mut())
    } == 0 { return Err(denied()); }
    Ok(status.ActiveProcesses == 0)
}
fn await_empty(handle: HANDLE) -> bool {
    let start = Instant::now();
    loop {
        if matches!(job_empty(handle), Ok(true)) { return true; }
        if start.elapsed() >= KILL_LIMIT { return false; }
        thread::sleep(TICK);
    }
}
fn terminate_and_check(handle: HANDLE) -> bool {
    unsafe { TerminateJobObject(handle, 98) } != 0 && await_empty(handle)
}
fn witness_file(id: &str, hash: &str, root: &Path) -> Result<PathBuf, SupervisorError> {
    if !valid_id(id) || !valid_hash(hash) || !root.is_absolute() {
        return Err(denied());
    }
    let meta = fs::symlink_metadata(root).map_err(|_| denied())?;
    if !meta.is_dir() || meta.file_attributes() & 0x400 != 0 {
        return Err(denied());
    }
    Ok(root.join(format!("broker-{id}-{}.jsonl", &hash[..16])))
}
fn hash_text(value: &[u8]) -> String { format!("{:x}", Sha256::digest(value)) }
#[derive(Serialize)]
struct Body<'a> {
    schema: &'static str,
    seq: u64,
    id: &'a str,
    plan_sha256: &'a str,
    owner_pid: u32,
    owner_creation: u64,
    nonce_sha256: &'a str,
    state: &'a str,
    unix_ms: u128,
}
#[derive(Serialize)]
struct Line<'a> {
    body: Body<'a>,
    prior: &'a str,
    entry_sha256: String,
}
struct Journal {
    file: File,
    next: u64,
    previous: String,
    nonce_digest: String,
    id: String,
    hash: String,
    pid: u32,
    creation: u64,
}
impl Journal {
    fn create(root: &Path, id: &str, hash: &str, pid: u32,
        creation: u64, nonce: &str) -> Result<Self, SupervisorError> {
        let path = witness_file(id, hash, root)?;
        let file = OpenOptions::new().write(true).create_new(true)
            .open(path).map_err(|_| denied())?;
        file.sync_all().map_err(|_| denied())?;
        Ok(Self {
            file, next: 0, previous: "0".repeat(64),
            nonce_digest: hash_text(nonce.as_bytes()),
            id: id.to_owned(), hash: hash.to_owned(), pid, creation,
        })
    }
    fn record(&mut self, state: &str) -> Result<(), SupervisorError> {
        if !matches!(state,
            "ready" | "owner_exited" | "kill_requested" | "tree_empty" |
            "kill_unverified" | "released_empty" | "recovery_hold_expired")
        { return Err(denied()); }
        let ms = SystemTime::now().duration_since(UNIX_EPOCH)
            .map_err(|_| denied())?.as_millis();
        let body = Body {
            schema: "lu.ide.d7c.independent-broker.v1",
            seq: self.next,
            id: &self.id, plan_sha256: &self.hash,
            owner_pid: self.pid, owner_creation: self.creation,
            nonce_sha256: &self.nonce_digest, state, unix_ms: ms,
        };
        let encoded = serde_json::to_vec(&body).map_err(|_| denied())?;
        let mut digest = Sha256::new();
        digest.update(self.previous.as_bytes());
        digest.update(&encoded);
        let entry_hash = format!("{:x}", digest.finalize());
        let line = Line {
            body, prior: &self.previous, entry_sha256: entry_hash.clone(),
        };
        let bytes = serde_json::to_vec(&line).map_err(|_| denied())?;
        if self.file.metadata().map_err(|_| denied())?.len()
            .saturating_add(bytes.len() as u64 + 1) > WITNESS_LIMIT {
            return Err(denied());
        }
        self.file.write_all(&bytes).map_err(|_| denied())?;
        self.file.write_all(b"\n").map_err(|_| denied())?;
        self.file.sync_all().map_err(|_| denied())?;
        self.next += 1;
        self.previous = entry_hash;
        Ok(())
    }
}

/// Native host-owned config; MUST be installed from reviewed source and be
/// inaccessible to project code. Not a frontend or LLM-provided path.
pub struct TrustedBroker {
    pub executable: PathBuf,
    pub expected_sha256: String,
    pub private_witness_directory: PathBuf,
}
pub struct BrokerLease {
    child: Child,
    job: HANDLE,
    release_name: Vec<u16>,
    released: bool,
    _pinned_binary: File,
}
impl BrokerLease {
    /// Begin before any authorized worker is resumed. Does not consume a
    /// grant itself; it only holds the kernel Job in an independent process.
    pub(crate) fn begin(
        job: HANDLE, id: &str, plan_hash: &str, config: &TrustedBroker,
    ) -> Result<Self, SupervisorError> {
        if !valid_id(id) || !valid_hash(plan_hash) ||
            !config.executable.is_absolute() || !valid_hash(&config.expected_sha256) ||
            !config.private_witness_directory.is_absolute() ||
            !matches!(job_empty(job), Ok(true)) {
            return Err(denied());
        }
        // Holding a readable file handle without FILE_SHARE_WRITE or
        // FILE_SHARE_DELETE pins the trusted binary against path replacement
        // or in-place changes between the digest and CreateProcess.
        let meta = fs::symlink_metadata(&config.executable).map_err(|_| denied())?;
        if !meta.is_file() || meta.file_attributes() & 0x400 != 0 ||
            meta.len() > 64 * 1024 * 1024 {
            return Err(denied());
        }
        let mut options = OpenOptions::new();
        options.read(true).share_mode(FILE_SHARE_READ)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
        let mut binary = options.open(&config.executable).map_err(|_| denied())?;
        let mut hasher = Sha256::new();
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let n = binary.read(&mut buffer).map_err(|_| denied())?;
            if n == 0 { break; }
            hasher.update(&buffer[..n]);
        }
        if format!("{:x}", hasher.finalize()) != config.expected_sha256 {
            return Err(denied());
        }
        let owner = std::process::id();
        let created = current_owner_creation()?;
        let mut random = [0_u8; 24];
        OsRng.fill_bytes(&mut random);
        let nonce = random.iter().map(|b| format!("{b:02x}")).collect::<String>();
        let mut command = Command::new(&config.executable);
        command.arg("--guard").arg(id).arg(plan_hash)
            .arg(owner.to_string()).arg(created.to_string())
            .arg(&config.private_witness_directory).arg(&nonce)
            .env_clear()
            .env("SystemRoot", std::env::var_os("SystemRoot").ok_or_else(denied)?)
            .env("LOCALAPPDATA", std::env::var_os("LOCALAPPDATA").ok_or_else(denied)?)
            .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW);
        let mut child = command.spawn().map_err(|_| denied())?;
        let stdout = match child.stdout.take() {
            Some(stdout) => stdout,
            None => {
                let _ = child.kill(); let _ = child.wait();
                return Err(denied());
            }
        };
        let (send, receive) = mpsc::channel();
        thread::spawn(move || {
            let mut line = String::new();
            let read = BufReader::new(stdout).read_line(&mut line);
            let _ = send.send((read, line));
        });
        let ready = receive.recv_timeout(READY_LIMIT)
            .ok().and_then(|(result, line)|
                result.ok().map(|_| line == format!("READY {nonce}\n")))
            .unwrap_or(false);
        if !ready || !matches!(child.try_wait(), Ok(None)) {
            let _ = child.kill(); let _ = child.wait();
            return Err(denied());
        }
        Ok(Self {
            child, job, release_name: release_name(id, plan_hash)?,
            released: false, _pinned_binary: binary,
        })
    }
    /// Must be checked before resume and throughout execution. If the broker
    /// exits early, the host must cancel the Job and quarantine its receipt.
    pub(crate) fn alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }
    /// Only a verified empty Job and durable terminal supervisor receipt
    /// may gracefully release the independently-held kernel Job handle.
    pub(crate) fn release_empty(&mut self) -> Result<(), SupervisorError> {
        if !matches!(job_empty(self.job), Ok(true)) { return Err(denied()); }
        if !self.alive() { return Err(denied()); }
        self.signal_release()?;
        self.released = true;
        Ok(())
    }
    fn signal_release(&self) -> Result<(), SupervisorError> {
        let event = Handle::new(unsafe {
            OpenEventW(EVENT_MODIFY_STATE, 0, self.release_name.as_ptr())
        })?;
        if unsafe { SetEvent(event.0) } == 0 { return Err(denied()); }
        Ok(())
    }
}
impl Drop for BrokerLease {
    fn drop(&mut self) {
        if !self.released {
            // Any early return with a live Job must kill it. The broker also
            // observes owner death independently if this process crashes.
            let _ = terminate_and_check(self.job);
            if matches!(job_empty(self.job), Ok(true)) {
                let _ = self.signal_release();
            }
        }
        // No Child::kill on normal release: it would destroy the sole
        // independent process before it can fsync its own final observation.
    }
}

/// Used only by the dedicated, fixed-mode broker binary. It never executes
/// source, Cargo, model commands, installer code, or untrusted worker input.
pub fn guard_process(args: &[String]) -> Result<(), SupervisorError> {
    if args.len() != 7 || args[0] != "--guard" { return Err(denied()); }
    let (id, hash) = (&args[1], &args[2]);
    let owner: u32 = args[3].parse().map_err(|_| denied())?;
    let owner_creation: u64 = args[4].parse().map_err(|_| denied())?;
    let root = Path::new(&args[5]);
    let nonce = &args[6];
    if !valid_id(id) || !valid_hash(hash) || owner == 0 || owner_creation == 0 ||
        nonce.len() != 48 || !nonce.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(denied());
    }
    let owner = Handle::new(unsafe {
        OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZE, 0, owner)
    })?;
    if process_started_at(owner.0)? != owner_creation ||
        unsafe { WaitForSingleObject(owner.0, 0) } != WAIT_TIMEOUT {
        return Err(denied());
    }
    let job = Handle::new(unsafe {
        OpenJobObjectW(JOB_QUERY | JOB_TERMINATE, 0, job_name(id, hash)?.as_ptr())
    })?;
    if !matches!(job_empty(job.0), Ok(true)) { return Err(denied()); }
    let release = Handle::new(unsafe {
        CreateEventW(ptr::null(), 1, 0, release_name(id, hash)?.as_ptr())
    })?;
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS { return Err(denied()); }
    let mut receipts = Journal::create(root, id, hash, std::process::id(), owner_creation, nonce)?;
    receipts.record("ready")?;
    // This byte sequence goes through an anonymous stdout pipe which only
    // the process that spawned this fixed executable can read.
    println!("READY {nonce}");
    std::io::stdout().flush().map_err(|_| denied())?;
    loop {
        let owner_wait = unsafe { WaitForSingleObject(owner.0, 0) };
        if owner_wait == WAIT_OBJECT_0 {
            receipts.record("owner_exited")?;
            receipts.record("kill_requested")?;
            let killed = terminate_and_check(job.0);
            receipts.record(if killed { "tree_empty" } else { "kill_unverified" })?;
            if !killed { return Err(denied()); }
            // A restart can OPEN THE STILL-LIVE NAMED JOB and independently
            // reconcile it. Do not accept the JSON alone as proof. This
            // bounded lease is deliberately not an unbounded background task.
            let began = Instant::now();
            while began.elapsed() < RECOVERY_HOLD { thread::sleep(TICK); }
            receipts.record("recovery_hold_expired")?;
            return Ok(());
        }
        if owner_wait != WAIT_TIMEOUT { return Err(denied()); }
        if unsafe { WaitForSingleObject(release.0, 0) } == WAIT_OBJECT_0 {
            if matches!(job_empty(job.0), Ok(true)) {
                receipts.record("released_empty")?;
                return Ok(());
            }
            // Never accept a forged "release" while any worker is alive.
            unsafe { ResetEvent(release.0); }
        }
        thread::sleep(TICK);
    }
}
