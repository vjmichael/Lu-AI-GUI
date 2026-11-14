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
#[cfg(feature="fin04w2-c2")]
use crate::fin04w2_c2::{self,C2JournalGate};
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
    // An administrator-pinned original supervisor receipt path and native
    // Job ID, never supplied by a pipe client. Only W2-C has these fields.
    #[cfg(feature="fin04w2-c2")]
    original_supervisor_journal: PathBuf,
    #[cfg(feature="fin04w2-c2")]
    original_job_id: String,
    // The split profile must bind the C2 debugger admission to a genuinely
    // distinct, independently installed SCM target service. Only an
    // administrator-controlled service.json can supply this image pin.
    #[cfg(feature="fin04w2-split-native")]
    approved_target_service_executable: PathBuf,
    #[cfg(feature="fin04w2-split-native")]
    expected_target_service_sha256: String,
    // Both SCM services independently pin the same original target bytes.
    // The target service checks its own protected config and seal/Job;
    // neither peer trusts caller-provided PE, PDB or a DAP-request PID.
    #[cfg(feature="fin04w2-split-native")]
    approved_target_image: PathBuf,
    #[cfg(feature="fin04w2-split-native")]
    expected_target_image_sha256: String,
    #[cfg(feature="fin04w2-split-native")]
    expected_target_pdb_sha256: String,
    // Only the debugger SCM service may prepare this pinned LLDB bundle.
    #[cfg(feature="fin04w2-split-native")]
    approved_lldb_dap_executable: PathBuf,
    #[cfg(feature="fin04w2-split-native")]
    expected_lldb_dap_sha256: String,
    #[cfg(feature="fin04w2-split-native")]
    expected_liblldb_sha256: String,
    #[cfg(feature="fin04w2-split-native")]
    expected_python310_sha256: String,
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
        if !(result.schema == "lu.ide.fin04w2.audit.v1"
            || (cfg!(feature="fin04w2-c2")
                && result.schema == "lu.ide.fin04w2.c2.v1"))
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
        #[cfg(feature = "fin04w2-c2")]
        {
            if result.schema!="lu.ide.fin04w2.c2.v1"
                || !result.original_supervisor_journal.is_absolute()
                || result.original_job_id.len()!=36
                || !result.original_job_id.starts_with("job-")
                || !result.original_job_id[4..].bytes().all(|b|b.is_ascii_hexdigit())
                || result.review_only_plan_hash != crate::fingerprint(&result.review_only_plan)?
            {return Err(denied());}
            security::verify_original_approval_journal_acl(
                &result.original_supervisor_journal,
                &result.approved_host_user_sid,
            )?;
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
        #[cfg(feature="fin04w2-split-native")]
        {
            if !result.approved_target_service_executable.is_absolute()
                || result.expected_target_service_sha256.len()!=64
                || !result.expected_target_service_sha256.bytes().all(|b|b.is_ascii_hexdigit())
            {return Err(denied());}
            security::hash_executable(
                &result.approved_target_service_executable,
                &result.expected_target_service_sha256,
            )?;
            if !result.approved_target_image.is_absolute()
                ||!result.approved_lldb_dap_executable.is_absolute(){return Err(denied());}
            for sha in [
                &result.expected_target_image_sha256,
                &result.expected_target_pdb_sha256,
                &result.expected_lldb_dap_sha256,
                &result.expected_liblldb_sha256,
                &result.expected_python310_sha256,
            ]{
                if sha.len()!=64||!sha.bytes().all(|b|b.is_ascii_hexdigit()){
                    return Err(denied());
                }
            }
            security::hash_executable(
                &result.approved_target_image,
                &result.expected_target_image_sha256,
            )?;
            security::hash_executable(
                &result.approved_lldb_dap_executable,
                &result.expected_lldb_dap_sha256,
            )?;
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
// C2 is the only executable route, with a fixed 74-byte request and
// one first-party benign fixture. W2-A's audit-only 92-byte grammar stays
// separate. Neither route admits LLDB, custom command or caller-selected PID.
#[cfg(feature="fin04w2-c2")]
fn signal(pipe:HANDLE,byte:u8)->Result<(),SupervisorError>{
    let mut written=0u32;
    if unsafe{
        WriteFile(pipe,(&byte as *const u8).cast(),1,&mut written,ptr::null_mut())
    }==0||written!=1{return Err(denied());}
    Ok(())
}
// Windows DisconnectNamedPipe can discard a message the peer has not
// yet read. A final F/J reply must remain readable until the authenticated
// host closes its pipe; no additional request or authority is accepted.
#[cfg(feature="fin04w2-c2")]
fn deliver_terminal_reply(pipe:HANDLE,reply:u8)->Result<(),SupervisorError>{
    if reply!=b'F'&&reply!=b'J'{return Err(denied());}
    signal(pipe,reply)?;
    let until=Instant::now()+Duration::from_secs(8);
    loop{
        if STOP.load(Ordering::SeqCst)||Instant::now()>=until{
            return Err(denied());
        }
        let mut available=0u32;
        // A broken pipe here is the expected host-close acknowledgement.
        // Unexpected client bytes are NEVER interpreted as new commands.
        if unsafe{
            PeekNamedPipe(pipe,ptr::null_mut(),0,ptr::null_mut(),
                &mut available,ptr::null_mut())
        }==0{return Ok(());}
        if available!=0{return Err(denied());}
        thread::sleep(Duration::from_millis(20));
    }
}
#[cfg(feature="fin04w2-c2")]
fn expect_signal(pipe:HANDLE,expected:u8)->Result<(),SupervisorError>{
    let until=Instant::now()+Duration::from_secs(8);
    loop{
        if STOP.load(Ordering::SeqCst)||Instant::now()>=until{
            return Err(denied());
        }
        let mut available=0u32;
        if unsafe{
            PeekNamedPipe(pipe,ptr::null_mut(),0,ptr::null_mut(),
                &mut available,ptr::null_mut())
        }==0||available>1{return Err(denied());}
        if available==1{break;}
        thread::sleep(Duration::from_millis(20));
    }
    let mut read=0u32;
    let mut byte=0u8;
    if unsafe{
        ReadFile(pipe,(&mut byte as *mut u8).cast(),1,
            &mut read,ptr::null_mut())
    }==0||read!=1||byte!=expected{
        return Err(denied());
    }
    Ok(())
}
#[cfg(feature="fin04w2-c2")]
fn review_only(pipe:HANDLE,config:&InstalledAuditConfig)
    ->Result<(),SupervisorError>{
    // The independent SCM guardian has already passed real kernel token,
    // protected Job and private service-config readback at startup.
    use rand::{rngs::OsRng,RngCore};
    let mut nonce=[0u8;16];
    OsRng.fill_bytes(&mut nonce);
    let mut challenge=[0u8;17];
    challenge[0]=b'C';challenge[1..].copy_from_slice(&nonce);
    let mut wrote=0u32;
    if unsafe{
        WriteFile(pipe,challenge.as_ptr().cast(),17,
            &mut wrote,ptr::null_mut())
    }==0||wrote!=17{return Err(denied());}
    let until=Instant::now()+Duration::from_secs(2);
    loop{
        if STOP.load(Ordering::SeqCst)||Instant::now()>=until{
            return Err(denied());
        }
        let mut available=0u32;
        if unsafe{
            PeekNamedPipe(pipe,ptr::null_mut(),0,ptr::null_mut(),
                &mut available,ptr::null_mut())
        }==0||available>fin04w2_c2::WIRE_BYTES as u32{
            return Err(denied());
        }
        if available==fin04w2_c2::WIRE_BYTES as u32{break;}
        thread::sleep(Duration::from_millis(20));
    }
    let mut packet=[0u8;fin04w2_c2::WIRE_BYTES];
    let mut count=0u32;
    if unsafe{
        ReadFile(pipe,packet.as_mut_ptr().cast(),
            packet.len() as u32,&mut count,ptr::null_mut())
    }==0||count!=packet.len() as u32{return Err(denied());}
    security::authenticate_client_from_kernel_pipe(
        pipe,&config.approved_host_executable,
        &config.expected_host_sha256,&config.approved_host_user_sid,
    )?;
    // The C2 one-use admission can never be upgraded to split-debugger
    // authority merely because the old single-service probe succeeds.
    // In the opt-in split profile require TWO live, differently named,
    // separately restricted SCM processes with exact pinned image hashes.
    // Hold their process handles throughout the bounded C2 transaction.
    // This is service presence only: a target-side one-use IPC, target Job
    // membership and stopped RIP still require independent evidence.
    #[cfg(feature="fin04w2-split-native")]
    let scm_pair=crate::windows_fin04w2_scm_pair::inspect_independent_service_pair(
        &std::env::current_exe().map_err(|_|denied())?,
        &config.expected_service_sha256,
        &config.approved_target_service_executable,
        &config.expected_target_service_sha256,
    )?;
    fin04w2_c2::verify_fixed_request(
        &packet,&config.original_job_id,&config.review_only_plan,&nonce,
    )?;
    security::verify_original_approval_journal_acl(
        &config.original_supervisor_journal,
        &config.approved_host_user_sid,
    )?;
    fin04w2_c2::verify_original_path(
        &config.original_supervisor_journal,&config.original_job_id,
        &config.review_only_plan,C2JournalGate::Authorized,
    )?;
    #[cfg(feature="fin04w2-split-native")]
    let original=crate::fin04w2_authority::verify_original_authorized_file(
        &config.original_supervisor_journal,&config.original_job_id,
        &config.review_only_plan,&config.review_only_plan_hash,
    )?;
    let root=std::env::var_os("ProgramData").ok_or_else(denied)
        .map(PathBuf::from)?.join(ROOT);
    let sid=security::sid_of_named_service(NAME)?;
    security::verify_private_evidence_root_for(&root,&sid)?;
    // Independent per-SCM fsynced reservations precede any suspended
    // process creation. In split mode bind BOTH service reservations to the
    // same immutable original human approval-entry and session, not merely
    // to a plan/job string. Original unsplit C2 receipt is unchanged.
    #[cfg(feature="fin04w2-split-native")]
    crate::fin04w2_authority::reserve_fresh_original_authorization(
        &root,&original,
    )?;
    #[cfg(not(feature="fin04w2-split-native"))]
    fin04w2_c2::reserve_once(
        &root,&config.original_job_id,&config.review_only_plan,
    )?;
    // The independent debugger SCM Job must be held while the exact
    // target broker prepares a separately Job-custodied suspended AppContainer.
    // Both services reserve the SAME original human-approved Job before
    // either executable can run. This is intentionally a NON-RUNNABLE
    // two-process source gate until independent LLDB stopped-RIP acceptance.
    let job=windows_fin04w2_kernel::create_bounded_empty_debugger_job()?;
    #[cfg(feature="fin04w2-split-native")]
    {
        scm_pair.reobserve()?;
        let mut target=crate::windows_fin04w2_target_ipc::prepare_original_target(
            &scm_pair,&config.original_job_id,&config.review_only_plan,
            &config.expected_target_image_sha256,
            &config.expected_target_pdb_sha256,
            &original.approved_entry_sha256,&original.session_id,
        )?;
        let debugger=crate::windows_fin04w2_split_launcher::prepare_external_debugger(
            &job,&config.approved_lldb_dap_executable,
            &config.expected_lldb_dap_sha256,
            &config.expected_liblldb_sha256,
            &config.expected_python310_sha256,
        )?;
        if debugger.process().is_null()||target.observed_target_pid()==0{
            return Err(denied());
        }
        target.reobserve(&scm_pair)?;
        scm_pair.reobserve()?;
        // The real executable is the *sealed* image in the target service's
        // immutable per-job stage, NOT the source-build PE path.
        let target_name=config.approved_target_image.file_name().ok_or_else(denied)?;
        let staged_target=std::env::var_os("ProgramData").ok_or_else(denied)
            .map(PathBuf::from)?.join("LuIDEDebugTargetBroker")
            .join("sealed-debug-targets").join(&config.original_job_id)
            .join(target_name);
        security::hash_executable(&staged_target,&config.expected_target_image_sha256)?;
        crate::windows_fin04w2_wfp::require_exact_outbound_blocks(
            &config.approved_lldb_dap_executable,&staged_target,
        )?;
        // ONLY the original native host's one-use grant can consume the
        // Authorized state and fsync RUNNING; this S is a suspended custody
        // acknowledgment, not authorization by the debugger SCM itself.
        signal(pipe,b'S')?;
        expect_signal(pipe,b'G')?;
        fin04w2_c2::verify_original_path(
            &config.original_supervisor_journal,&config.original_job_id,
            &config.review_only_plan,C2JournalGate::Running,
        )?;
        crate::fin04w2_authority::verify_running_same_original_authorization(
            &config.original_supervisor_journal,&config.original_job_id,
            &config.review_only_plan,&config.review_only_plan_hash,&original,
        )?;
        crate::fin04w2_authority::verify_reserved_original_authorization(
            &root,&original,
        )?;
        scm_pair.reobserve()?;
        crate::windows_fin04w2_wfp::require_exact_outbound_blocks(
            &config.approved_lldb_dap_executable,&staged_target,
        )?;
        target.grant_original_running(&scm_pair)?;
        crate::windows_fin04w2_wfp::require_exact_outbound_blocks(
            &config.approved_lldb_dap_executable,&staged_target,
        )?;
        scm_pair.reobserve()?;
        debugger.resume_after_original_running()?;
        // Genuine bounded native LLDB-DAP initialize/attach/stopped and
        // stackTrace; unlike C2's benign fixture, this is a real debugger.
        // DAP's thread/IP/line are UNTRUSTED candidates until the separate
        // target SCM compares exact Job, OS-loaded PE and x64 thread RIP.
        let stop=crate::windows_fin04w2_lldb_dap::attach_and_observe_stop(
            &debugger,target.observed_target_pid(),
        )?;
        let os_thread=target.corroborate_stopped_rip(&scm_pair,stop.observed_pc)?;
        if os_thread!=stop.thread_id||stop.source_line==0
            ||stop.source_path.is_empty(){return Err(denied());}
        // A late WFP rule or PE/PDB/journal mutation must invalidate even
        // an already observed DAP stop. These are OS readbacks, NOT a
        // substitute for effective hostile-socket or source-line proof.
        crate::windows_fin04w2_wfp::require_exact_outbound_blocks(
            &config.approved_lldb_dap_executable,&staged_target,
        )?;
        security::hash_executable(&staged_target,&config.expected_target_image_sha256)?;
        crate::fin04w2_authority::verify_running_same_original_authorization(
            &config.original_supervisor_journal,&config.original_job_id,
            &config.review_only_plan,&config.review_only_plan_hash,&original,
        )?;
        crate::fin04w2_authority::verify_reserved_original_authorization(
            &root,&original,
        )?;
        target.reobserve(&scm_pair)?;
        // The original independent LLVM/PDB source-line oracle, patched
        // kernel event-owner evidence and effective socket-denial receipts
        // are still separate mandatory gates. Do not self-award X/F or
        // fabricate a debugger completion after these partial observations.
        return Err(denied());
    }
    #[cfg(not(feature="fin04w2-split-native"))]
    {
    // Preserve the ORIGINAL C2 fixed benign probe unchanged in the
    // unsplit historical negative/CI acceptance profile.
    let child=job.prepare_fixed_probe(
        &config.fixed_probe.executable,
        &config.fixed_probe.expected_sha256,
        &config.fixed_probe.foreign_file,
    )?;
    signal(pipe,b'S')?;
    expect_signal(pipe,b'G')?;
    // The service—not the client G byte—verifies actual original fsynced
    // grant consumption and Running state before ResumeThread.
    fin04w2_c2::verify_original_path(
        &config.original_supervisor_journal,&config.original_job_id,
        &config.review_only_plan,C2JournalGate::Running,
    )?;
    #[cfg(feature="fin04w2-split-native")]
    scm_pair.reobserve()?;
    child.resume_and_verify()?;
    signal(pipe,b'X')?;
    expect_signal(pipe,b'R')?;
    if let Err(error)=fin04w2_c2::verify_original_path(
        &config.original_supervisor_journal,&config.original_job_id,
        &config.review_only_plan,C2JournalGate::ReleasePending,
    ){
        // Fail closed: J is a rejection only, never a release approval.
        // The owned Job remains held until the peer closes (or this
        // bounded routine errors), then kernel kill-on-close applies.
        let _=deliver_terminal_reply(pipe,b'J');
        return Err(error);
    }
    #[cfg(feature="fin04w2-split-native")]
    scm_pair.reobserve()?;
    drop(job);
    // Keep the exact fixed protocol unchanged. F is evidence the kernel
    // Job was released only after the original ReleasePending was verified.
    deliver_terminal_reply(pipe,b'F')?;
    Ok(())
    } // original unsplit C2 fixed benign-probe path
}
#[cfg(not(feature="fin04w2-c2"))]
fn review_only(pipe:HANDLE,config:&InstalledAuditConfig)
    ->Result<(),SupervisorError>{
    let until=Instant::now()+Duration::from_secs(2);
    loop{
        if STOP.load(Ordering::SeqCst)||Instant::now()>=until{
            return Err(denied());
        }
        let mut available=0u32;
        if unsafe{
            PeekNamedPipe(pipe,ptr::null_mut(),0,ptr::null_mut(),
                &mut available,ptr::null_mut())
        }==0||available>92{return Err(denied());}
        if available==92{break;}
        thread::sleep(Duration::from_millis(20));
    }
    let mut packet=[0u8;92];
    let mut read=0u32;
    if unsafe{
        ReadFile(pipe,packet.as_mut_ptr().cast(),92,
            &mut read,ptr::null_mut())
    }==0||read!=92{return Err(denied());}
    security::authenticate_client_from_kernel_pipe(
        pipe,&config.approved_host_executable,
        &config.expected_host_sha256,&config.approved_host_user_sid,
    )?;
    let reviewed=fin04w2_protocol::inspect_shape_only(
        &packet,&config.review_only_plan,
        &config.review_only_plan_hash,&nonce(&config.review_only_nonce)?,
    )?;
    if reviewed.operation!=fin04w2_protocol::ReviewedOperation::Initialize
        ||AUDIT_USED.swap(true,Ordering::SeqCst){
        return Err(denied());
    }
    let reply=[b'A'];
    let mut sent=0u32;
    if unsafe{
        WriteFile(pipe,reply.as_ptr().cast(),1,
            &mut sent,ptr::null_mut())
    }==0||sent!=1{return Err(denied());}
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
    // W2-B is an isolated startup smoke test, without creator authority.
    // W2-C must NOT inherit this behavior: its fixed worker is runnable
    // only after a fresh ORIGINAL in-memory grant was consumed, the host
    // journal was independently replayed, and a service receipt fsynced.
    #[cfg(all(
        feature = "fin04w2-contained-probe",
        not(feature = "fin04w2-c2")
    ))]
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
