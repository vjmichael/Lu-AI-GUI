//! FIN-04W2-A: separate restricted SCM debugger broker, AUDIT-ONLY.
//! This service does NOT launch LLDB, hold a debugger Job, modify ACL/WFP,
//! offer DAP passthrough or authorize a debugger. It verifies a real SCM
//! identity, protected OS pipe DACL and kernel-pinned client, then checks the
//! fixed-width protocol against an installer-staged REVIEW fixture. Even a
//! successful audit response is NOT a project build or execution receipt.
#![cfg(all(windows, debug_assertions, feature = "fin04w2-scm"))]

use crate::{
    fin04w2_protocol, windows_fin04w2_kernel,
    windows_service_security as security, JobPlan, SupervisorError,
};
use serde::Deserialize;
use std::{
    ffi::c_void, fs, io::Read, os::windows::fs::MetadataExt,
    path::{Path, PathBuf}, ptr,
    sync::atomic::{AtomicBool, AtomicU32, Ordering}, thread,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, GetLastError, HANDLE, ERROR_PIPE_CONNECTED, INVALID_HANDLE_VALUE},
    Storage::FileSystem::{CreateFileW, ReadFile, WriteFile, OPEN_EXISTING, PIPE_ACCESS_DUPLEX},
    System::{
        Pipes::{ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PeekNamedPipe,
            PIPE_READMODE_MESSAGE, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_MESSAGE, PIPE_WAIT},
        Services::{RegisterServiceCtrlHandlerExW, SetServiceStatus, StartServiceCtrlDispatcherW,
            SERVICE_ACCEPT_STOP, SERVICE_CONTROL_STOP, SERVICE_RUNNING, SERVICE_START_PENDING,
            SERVICE_STATUS, SERVICE_STOPPED, SERVICE_STOP_PENDING, SERVICE_TABLE_ENTRYW,
            SERVICE_WIN32_OWN_PROCESS},
    },
};
const NAME: &str = "LuIdeDebuggerBroker";
const ENDPOINT: &str = r"\\.\pipe\LuIdeDebuggerBroker.v1";
const ROOT: &str = "LuIDEDebuggerBroker";
const MAX_CONFIG: u64 = 8192;
const FIRST_PIPE_INSTANCE: u32 = 0x0008_0000;
static STOP: AtomicBool = AtomicBool::new(false);
// Fixed, non-sensitive native startup stage for the SCM service status
// report (not request output). 1=token, 2=config, 3=Job, 4=fixture,
// 5=pipe DACL template, 6=pipe creation, 7=kernel DACL, 8=ready.
static STARTUP_STAGE: AtomicU32 = AtomicU32::new(0);
static AUDIT_USED: AtomicBool = AtomicBool::new(false);
fn denied() -> SupervisorError { SupervisorError::NoContainment }
fn wide(value: &str) -> Vec<u16> { value.encode_utf16().chain([0]).collect() }
struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() && self.0 != INVALID_HANDLE_VALUE {
            unsafe { CloseHandle(self.0); }
        }
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InstalledAuditConfig {
    schema: String,
    service_name: String,
    endpoint: String,
    expected_service_sha256: String,
    expected_host_sha256: String,
    approved_host_executable: PathBuf,
    approved_host_user_sid: String,
    // Config-staged *test-only* plan, NOT read from an original
    // authenticated build/journal. Never an execution authorization.
    review_only_plan: JobPlan,
    review_only_plan_hash: String,
    review_only_nonce: String,
    // CI-only first-party fixture. This cannot be selected or influenced by
    // a named-pipe client; only the preinstalled private service.json pins it.
    #[cfg(feature = "fin04w2-contained-probe")]
    fixed_probe: FixedProbe,
}
#[cfg(feature = "fin04w2-contained-probe")]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FixedProbe {
    executable: PathBuf,
    expected_sha256: String,
    foreign_file: PathBuf,
}
fn nonce(value: &str) -> Result<[u8;16], SupervisorError> {
    if value.len() != 32 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(denied());
    }
    let mut result = [0u8;16];
    for (index, item) in result.iter_mut().enumerate() {
        *item = u8::from_str_radix(&value[index*2..index*2+2], 16)
            .map_err(|_| denied())?;
    }
    Ok(result)
}
impl InstalledAuditConfig {
    fn read() -> Result<Self, SupervisorError> {
        let root = PathBuf::from(std::env::var_os("ProgramData").ok_or_else(denied)?)
            .join(ROOT);
        let config = root.join("service.json");
        let meta = fs::symlink_metadata(&config).map_err(|_|denied())?;
        if !meta.is_file() || meta.file_attributes() & 0x400 != 0 || meta.len() > MAX_CONFIG {
            return Err(denied());
        }
        let mut bytes = Vec::new();
        fs::File::open(&config).map_err(|_|denied())?
            .take(MAX_CONFIG+1).read_to_end(&mut bytes).map_err(|_|denied())?;
        if bytes.len() as u64 > MAX_CONFIG { return Err(denied()); }
        let result: Self = serde_json::from_slice(&bytes).map_err(|_|denied())?;
        if result.schema != "lu.ide.fin04w2.audit.v1"
            || result.service_name != NAME || result.endpoint != ENDPOINT
            || !result.approved_host_executable.is_absolute()
            || !crate::validate_plan(&result.review_only_plan)
            || result.review_only_plan.recipe_id != "dap_lldb_project"
            || result.review_only_plan.effect_class != "debug_inspection_isolated"
            || result.review_only_plan_hash.len() != 64
            || !result.review_only_plan_hash.bytes().all(|b| b.is_ascii_hexdigit())
            || nonce(&result.review_only_nonce).is_err()
            || security::validate_approved_host_user_sid(
                &result.approved_host_user_sid,
            ).is_err()
        {
            return Err(denied());
        }
        #[cfg(feature = "fin04w2-contained-probe")]
        {
            let probe = &result.fixed_probe;
            if !probe.executable.is_absolute()
                || !probe.foreign_file.is_absolute()
                || probe.expected_sha256.len() != 64
                || !probe.expected_sha256.bytes().all(|b| b.is_ascii_hexdigit())
            {
                return Err(denied());
            }
            security::hash_executable(&probe.executable, &probe.expected_sha256)?;
        }
        let sid = security::sid_of_named_service(NAME)?;
        security::verify_private_evidence_root_for(&root, &sid)?;
        security::hash_executable(
            &std::env::current_exe().map_err(|_|denied())?,
            &result.expected_service_sha256,
        )?;
        security::hash_executable(
            &result.approved_host_executable, &result.expected_host_sha256,
        )?;
        Ok(result)
    }
}
unsafe extern "system" fn control(
    code: u32, _kind: u32, _data: *mut c_void, _context: *mut c_void,
) -> u32 {
    if code == SERVICE_CONTROL_STOP {
        STOP.store(true, Ordering::SeqCst);
        // Wake the single bounded synchronous listener. This does not send
        // a control message or grant any service/LLDB authority.
        let open = CreateFileW(
            wide(ENDPOINT).as_ptr(), 0, 0, ptr::null(), OPEN_EXISTING,
            0, ptr::null_mut(),
        );
        if !open.is_null() && open != INVALID_HANDLE_VALUE { CloseHandle(open); }
    }
    0
}
fn status(state: u32, controls: u32, exit: u32) -> SERVICE_STATUS {
    SERVICE_STATUS {
        dwServiceType: SERVICE_WIN32_OWN_PROCESS,
        dwCurrentState: state,
        dwControlsAccepted: controls,
        dwWin32ExitCode: if exit != 0 { 1066 } else { 0 },
        dwServiceSpecificExitCode: if exit != 0 {
            100 + STARTUP_STAGE.load(Ordering::Relaxed)
        } else { 0 },
        dwCheckPoint: 0,
        dwWaitHint: if matches!(state, SERVICE_START_PENDING | SERVICE_STOP_PENDING) { 15000 } else { 0 },
    }
}
// Fail closed even if a caller has connected: no command can execute.
// Only an authenticated *matching installer-staged review fixture* receives
// the byte 'A' ("audited, NEVER authorized"); all other inputs disconnect.
fn review_only(pipe: HANDLE, config: &InstalledAuditConfig) -> Result<(), SupervisorError> {
    let until = Instant::now() + Duration::from_secs(2);
    let mut available = 0u32;
    loop {
        if STOP.load(Ordering::SeqCst) || Instant::now() >= until { return Err(denied()); }
        if unsafe {
            PeekNamedPipe(pipe, ptr::null_mut(), 0, ptr::null_mut(),
                &mut available, ptr::null_mut())
        } == 0 { return Err(denied()); }
        if available > 92 { return Err(denied()); }
        if available >= 92 { break; }
        thread::sleep(Duration::from_millis(20));
    }
    let mut frame = [0u8;93];
    let mut got = 0u32;
    if unsafe {
        ReadFile(pipe, frame.as_mut_ptr().cast(), frame.len() as u32,
            &mut got, ptr::null_mut())
    } == 0 || got != 92 {
        return Err(denied());
    }
    let _client = security::authenticate_client_from_kernel_pipe(
        pipe, &config.approved_host_executable, &config.expected_host_sha256,
        &config.approved_host_user_sid,
    )?;
    let reviewed = fin04w2_protocol::inspect_shape_only(
        &frame[..92], &config.review_only_plan,
        &config.review_only_plan_hash, &nonce(&config.review_only_nonce)?,
    )?;
    // Shape validity does NOT confer original creator authorization or a
    // completed build receipt. Do not even acknowledge LaunchSealed/threads/
    // stack/stop as an accepted audit. Only the harmless Initialize review
    // receives 'A'; the remaining verbs all fail closed until the original
    // journal, service-issued one-use grant and PE/PDB kernel gates exist.
    if reviewed.operation != fin04w2_protocol::ReviewedOperation::Initialize {
        return Err(denied());
    }
    if AUDIT_USED.swap(true, Ordering::SeqCst) { return Err(denied()); }
    let response = [b'A']; // Not a debugger authorization or process grant.
    let mut written = 0u32;
    if unsafe {
        WriteFile(pipe, response.as_ptr().cast(), 1, &mut written, ptr::null_mut())
    } == 0 || written != 1 { return Err(denied()); }
    Ok(())
}
fn run_service(handle: windows_sys::Win32::System::Services::SERVICE_STATUS_HANDLE)
    -> Result<(), SupervisorError> {
    STARTUP_STAGE.store(1, Ordering::Relaxed);
    windows_fin04w2_kernel::inspect_current_service_principal()?;
    STARTUP_STAGE.store(2, Ordering::Relaxed);
    let config = InstalledAuditConfig::read()?;
    // Before accepting even a REVIEW-ONLY request, the genuine Windows
    // kernel must create a protected, separately named, zero-member Job
    // and confirm exact memory/process/kill-on-close limits plus its actual
    // DACL. The Job is held until the SCM service stops. It has no API to
    // enroll a debuggee; WFP and descendant tests remain mandatory.
    STARTUP_STAGE.store(3, Ordering::Relaxed);
    let _empty_audit_job = windows_fin04w2_kernel::create_bounded_empty_debugger_job()?;
    // Only the frozen CI binary can execute in this branch. There is no
    // caller action to launch it and no LLDB binary/command path. The
    // service will not report RUNNING if actual kernel Job, foreign-file or
    // outbound-socket probes do not agree. The independent admin CI step
    // must also establish actual WFP rule identity and host baseline.
    #[cfg(feature = "fin04w2-contained-probe")]
    {
        STARTUP_STAGE.store(4, Ordering::Relaxed);
        _empty_audit_job.verify_fixed_probe(
        &config.fixed_probe.executable,
        &config.fixed_probe.expected_sha256,
        &config.fixed_probe.foreign_file,
        )?;
    }
    STARTUP_STAGE.store(5, Ordering::Relaxed);
    let sid = security::sid_of_named_service(NAME)?;
    let pipe_acl = security::PipeSecurity::new_for_service(
        &config.approved_host_user_sid, &sid,
    )?;
    STOP.store(false, Ordering::SeqCst);
    AUDIT_USED.store(false, Ordering::SeqCst);
    let mut first = true;
    loop {
        STARTUP_STAGE.store(6, Ordering::Relaxed);
        let access = PIPE_ACCESS_DUPLEX | if first { FIRST_PIPE_INSTANCE } else { 0 };
        let raw = unsafe {
            CreateNamedPipeW(
                wide(ENDPOINT).as_ptr(), access,
                PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                1, 512, 512, 0, &pipe_acl.attributes,
            )
        };
        let pipe = Handle(raw);
        if raw == INVALID_HANDLE_VALUE || raw.is_null() { return Err(denied()); }
        STARTUP_STAGE.store(7, Ordering::Relaxed);
        security::verify_created_pipe_security_for(
            raw, &config.approved_host_user_sid, &sid,
        )?;
        if first {
            STARTUP_STAGE.store(8, Ordering::Relaxed);
            if unsafe {
                SetServiceStatus(handle, &status(SERVICE_RUNNING, SERVICE_ACCEPT_STOP, 0))
            } == 0 { return Err(denied()); }
            first = false;
        }
        if STOP.load(Ordering::SeqCst) { break; }
        if unsafe { ConnectNamedPipe(raw, ptr::null_mut()) } == 0
            && unsafe { GetLastError() } != ERROR_PIPE_CONNECTED {
            if STOP.load(Ordering::SeqCst) { break; }
            return Err(denied());
        }
        if STOP.load(Ordering::SeqCst) { break; }
        let _ = review_only(raw, &config);
        unsafe { DisconnectNamedPipe(raw); }
        drop(pipe);
    }
    Ok(())
}
unsafe extern "system" fn main_service(_argc: u32, _argv: *mut *mut u16) {
    let registered = RegisterServiceCtrlHandlerExW(wide(NAME).as_ptr(), Some(control), ptr::null());
    if registered.is_null() { return; }
    let _ = SetServiceStatus(registered, &status(SERVICE_START_PENDING, 0, 0));
    let outcome = std::panic::catch_unwind(|| run_service(registered));
    let ok = matches!(outcome, Ok(Ok(()))) && STOP.load(Ordering::SeqCst);
    let _ = SetServiceStatus(registered, &status(SERVICE_STOPPED, 0, if ok { 0 } else { 1 }));
}
pub fn dispatch() -> Result<(), SupervisorError> {
    let name = wide(NAME);
    let entries = [
        SERVICE_TABLE_ENTRYW {
            lpServiceName: name.as_ptr() as *mut u16,
            lpServiceProc: Some(main_service),
        },
        SERVICE_TABLE_ENTRYW { lpServiceName: ptr::null_mut(), lpServiceProc: None },
    ];
    if unsafe { StartServiceCtrlDispatcherW(entries.as_ptr()) } == 0 {
        Err(denied())
    } else {
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn interactive_ci_runner_must_not_attest_as_new_service() {
        assert!(crate::windows_fin04w2_kernel::inspect_current_service_principal().is_err());
    }
}
