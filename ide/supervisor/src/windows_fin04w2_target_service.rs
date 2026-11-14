//! FIN-04W2 target-side, separate SCM-owned AppContainer *preparation*.
//! A fixed, kernel-authenticated debugger-broker pipe may request only the
//! exact installed original JobPlan already approved by the human host.
//! The target service independently replays the protected original journal
//! and fsyncs a single irreversible authorization reservation BEFORE it
//! creates the sealed target under its own independently held Job.
//!
//! This profile never resumes the target, attaches a debugger, accepts a
//! caller-selected path/PID, or converts its suspended-preparation receipt
//! into a successful debugger execution. Cancellation, peer/service death,
//! timeout and restart drop the suspended process and kill-on-close Job.
//! A later, separately reviewed LLDB attach path must establish its own
//! Running/RIP/WFP/cancellation evidence; a prepared target is NOT that proof.
#![cfg(all(windows,debug_assertions,feature="fin04w2-split-native"))]

use crate::{
    fin04w2_authority,fin04w2_c2,
    windows_fin04w2_kernel as kernel,
    windows_fin04w2_scm_pair as scm_pair,
    windows_fin04w2_split_launcher,
    windows_service_security as security,
    JobPhase,JobPlan,SupervisorError,
};
use serde::Deserialize;
use std::{
    fs,io::Read,os::windows::fs::MetadataExt,
    path::{Path,PathBuf},ptr,
    sync::atomic::{AtomicBool,Ordering},thread,
    time::{Duration,Instant},
};
use windows_sys::Win32::{
    Foundation::{CloseHandle,GetLastError,HANDLE,ERROR_PIPE_CONNECTED,INVALID_HANDLE_VALUE},
    Security::RevertToSelf,
    Storage::FileSystem::{CreateFileW,ReadFile,WriteFile,OPEN_EXISTING,PIPE_ACCESS_DUPLEX},
    System::{
        Pipes::{ConnectNamedPipe,CreateNamedPipeW,DisconnectNamedPipe,
            ImpersonateNamedPipeClient,PeekNamedPipe,
            PIPE_READMODE_MESSAGE,PIPE_REJECT_REMOTE_CLIENTS,PIPE_TYPE_MESSAGE,PIPE_WAIT},
        Services::{
            RegisterServiceCtrlHandlerExW,SetServiceStatus,StartServiceCtrlDispatcherW,
            SERVICE_ACCEPT_STOP,SERVICE_CONTROL_STOP,SERVICE_RUNNING,SERVICE_START_PENDING,
            SERVICE_STATUS,SERVICE_STOPPED,SERVICE_STOP_PENDING,SERVICE_TABLE_ENTRYW,
            SERVICE_WIN32_OWN_PROCESS,
        },
        Threading::GetProcessId,
    },
};

const SERVICE:&str="LuIdeDebugTargetBroker";
const ROOT:&str="LuIDEDebugTargetBroker";
const ENDPOINT:&str=r"\\.\pipe\LuIdeDebugTargetBroker.v1";
const LIMIT:u64=8192;
const FIRST_PIPE_INSTANCE:u32=0x0008_0000;
static STOP:AtomicBool=AtomicBool::new(false);
fn denied()->SupervisorError{SupervisorError::NoContainment}
fn wide(v:&str)->Vec<u16>{v.encode_utf16().chain([0]).collect()}
struct Owned(HANDLE);
impl Drop for Owned{
    fn drop(&mut self){
        if !self.0.is_null()&&self.0!=INVALID_HANDLE_VALUE{
            unsafe{CloseHandle(self.0);}
        }
    }
}
#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct InstalledConfig{
    schema:String,
    service_name:String,
    endpoint:String,
    expected_service_sha256:String,
    approved_debugger_service_executable:PathBuf,
    expected_debugger_service_sha256:String,
    approved_host_user_sid:String,
    original_supervisor_journal:PathBuf,
    original_job_id:String,
    sealed_plan:JobPlan,
    sealed_plan_sha256:String,
    approved_target_image:PathBuf,
    expected_target_image_sha256:String,
    approved_target_pdb:PathBuf,
    expected_target_pdb_sha256:String,
}
impl InstalledConfig{
    fn read()->Result<(PathBuf,Self),SupervisorError>{
        let root=PathBuf::from(std::env::var_os("ProgramData").ok_or_else(denied)?)
            .join(ROOT);
        let file=root.join("service.json");
        let meta=fs::symlink_metadata(&file).map_err(|_|denied())?;
        if !meta.is_file()||meta.file_attributes()&0x400!=0
            ||meta.len()==0||meta.len()>LIMIT{return Err(denied());}
        let mut bytes=Vec::new();
        fs::File::open(&file).map_err(|_|denied())?.take(LIMIT+1)
            .read_to_end(&mut bytes).map_err(|_|denied())?;
        if bytes.len() as u64>LIMIT{return Err(denied());}
        let cfg:Self=serde_json::from_slice(&bytes).map_err(|_|denied())?;
        if cfg.schema!="lu.ide.fin04w2.target.ipc.v2"
            ||cfg.service_name!=SERVICE||cfg.endpoint!=ENDPOINT
            ||!cfg.approved_debugger_service_executable.is_absolute()
            ||!cfg.original_supervisor_journal.is_absolute()
            ||!cfg.approved_target_image.is_absolute()
            ||!cfg.approved_target_pdb.is_absolute()
            ||cfg.approved_target_image==cfg.approved_target_pdb
            ||cfg.original_job_id.len()!=36
            ||!cfg.original_job_id.starts_with("job-")
            ||!cfg.original_job_id[4..].bytes().all(|c|c.is_ascii_hexdigit())
            ||!crate::validate_plan(&cfg.sealed_plan)
            ||cfg.sealed_plan.recipe_id!="dap_lldb_project"
            ||cfg.sealed_plan.effect_class!="debug_inspection_isolated"
            ||cfg.sealed_plan_sha256!=crate::fingerprint(&cfg.sealed_plan)?
        {return Err(denied());}
        for sha in [
            &cfg.expected_service_sha256,
            &cfg.expected_debugger_service_sha256,
            &cfg.expected_target_image_sha256,
            &cfg.expected_target_pdb_sha256,
        ]{
            if sha.len()!=64||!sha.bytes().all(|c|c.is_ascii_hexdigit()){
                return Err(denied());
            }
        }
        security::validate_approved_host_user_sid(&cfg.approved_host_user_sid)?;
        // These private file ACLs are reobserved independently of hashes or
        // serialized approval. The journal is owned by the original host.
        let sid=security::sid_of_named_service(SERVICE)?;
        security::verify_private_evidence_root_for(&root,&sid)?;
        // Original journal ACL is read later, only while impersonating the
        // independently authenticated debugger SCM peer for bounded READ.
        // The target SCM service itself has no original-journal write/read
        // grant and does not widen the existing three-principal ACL.
        security::hash_executable(
            &std::env::current_exe().map_err(|_|denied())?,
            &cfg.expected_service_sha256,
        )?;
        security::hash_executable(
            &cfg.approved_debugger_service_executable,
            &cfg.expected_debugger_service_sha256,
        )?;
        security::hash_executable(
            &cfg.approved_target_image,&cfg.expected_target_image_sha256,
        )?;
        security::hash_executable(
            &cfg.approved_target_pdb,&cfg.expected_target_pdb_sha256,
        )?;
        Ok((root,cfg))
    }
}
fn status(state:u32,controls:u32,exit:u32)->SERVICE_STATUS{
    SERVICE_STATUS{
        dwServiceType:SERVICE_WIN32_OWN_PROCESS,
        dwCurrentState:state,dwControlsAccepted:controls,
        dwWin32ExitCode:exit,dwServiceSpecificExitCode:0,
        dwCheckPoint:0,dwWaitHint:3000,
    }
}
unsafe extern "system" fn control(code:u32,_event:u32,_data:*mut std::ffi::c_void,
    _context:*mut std::ffi::c_void)->u32{
    if code==SERVICE_CONTROL_STOP{
        STOP.store(true,Ordering::SeqCst);
        // Wake the single synchronous named-pipe accept loop, with no
        // data/approval capability or externally influenced command.
        let pipe=unsafe{CreateFileW(
            wide(ENDPOINT).as_ptr(),0,0,ptr::null(),OPEN_EXISTING,0,ptr::null_mut(),
        )};
        if !pipe.is_null()&&pipe!=INVALID_HANDLE_VALUE{
            unsafe{CloseHandle(pipe);}
        }
    }
    0
}
fn read_fixed(pipe:HANDLE,out:&mut [u8])->Result<(),SupervisorError>{
    if !matches!(out.len(),1|9|74){return Err(denied());}
    let deadline=Instant::now()+Duration::from_secs(if out.len()==74{5}else{30});
    loop{
        if STOP.load(Ordering::SeqCst)||Instant::now()>=deadline{
            return Err(denied());
        }
        let mut available=0u32;
        if unsafe{PeekNamedPipe(
            pipe,ptr::null_mut(),0,ptr::null_mut(),&mut available,ptr::null_mut(),
        )}==0||available>out.len() as u32{return Err(denied());}
        if available==out.len() as u32{break;}
        thread::sleep(Duration::from_millis(20));
    }
    let mut read=0u32;
    if unsafe{ReadFile(
        pipe,out.as_mut_ptr().cast(),out.len() as u32,&mut read,ptr::null_mut(),
    )}==0||read!=out.len() as u32{return Err(denied());}
    Ok(())
}
fn write_all(pipe:HANDLE,bytes:&[u8])->Result<(),SupervisorError>{
    if !matches!(bytes.len(),1|13|17|141){return Err(denied());}
    let mut written=0u32;
    if unsafe{WriteFile(
        pipe,bytes.as_ptr().cast(),bytes.len() as u32,&mut written,ptr::null_mut(),
    )}==0||written!=bytes.len() as u32{return Err(denied());}
    Ok(())
}
/// Caller service never sends arbitrary path/attach/PID/plan material. The
/// sole request is C2's exact 74-byte (job, plan digest, target nonce) frame.
/// The target independently loads all source/PE/PDB data from private config.
fn prepare_one_target(
    pipe:HANDLE,cfg:&InstalledConfig,root:&Path,
    job:&kernel::EmptyAuditJob,
)->Result<(),SupervisorError>{
    let pair=scm_pair::inspect_independent_service_pair(
        &cfg.approved_debugger_service_executable,
        &cfg.expected_debugger_service_sha256,
        &std::env::current_exe().map_err(|_|denied())?,
        &cfg.expected_service_sha256,
    )?;
    pair.attest_debugger_client(pipe)?;
    use rand::{rngs::OsRng,RngCore};
    let mut nonce=[0u8;16];
    OsRng.fill_bytes(&mut nonce);
    let mut challenge=[0u8;17];
    challenge[0]=b'C';
    challenge[1..].copy_from_slice(&nonce);
    write_all(pipe,&challenge)?;
    let mut packet=[0u8;fin04w2_c2::WIRE_BYTES];
    read_fixed(pipe,&mut packet)?;
    pair.attest_debugger_client(pipe)?;
    fin04w2_c2::verify_fixed_request(
        &packet,&cfg.original_job_id,&cfg.sealed_plan,&nonce,
    )?;
    // The original host journal is intentionally readable by its exact
    // debugger-service SID, NOT the new target service. Borrow only the
    // already kernel-attested debugger peer's restricted read identity,
    // replay the original hash chain independently here, and ALWAYS revert
    // before touching the target service's private reservation/Job.
    let original={
        if unsafe{ImpersonateNamedPipeClient(pipe)}==0{return Err(denied());}
        struct Revert;
        impl Drop for Revert{
            fn drop(&mut self){
                if unsafe{RevertToSelf()}==0{
                    // Continuing with another service's token after a
                    // failed revert is unacceptable even on unwind.
                    std::process::abort();
                }
            }
        }
        let _revert=Revert;
        security::verify_original_approval_journal_acl(
            &cfg.original_supervisor_journal,&cfg.approved_host_user_sid,
        )?;
        fin04w2_authority::verify_original_authorized_file(
            &cfg.original_supervisor_journal,&cfg.original_job_id,
            &cfg.sealed_plan,&cfg.sealed_plan_sha256,
        )?
    };
    // Irreversible atomic fsynced CREATE_NEW; a torn reservation is a
    // permanent replay blocker pending independently authorized recovery.
    fin04w2_authority::reserve_fresh_original_authorization(root,&original)?;
    pair.reobserve()?;
    // Actual native AppContainer/Job preparation INSIDE the target SCM
    // service, not a second child Job in the debugger's service process.
    let mut suspended=windows_fin04w2_split_launcher::prepare_appcontainer_target(
        job,&cfg.original_job_id,root,
        &cfg.approved_target_image,&cfg.expected_target_image_sha256,
        &cfg.approved_target_pdb,&cfg.expected_target_pdb_sha256,
    )?;
    // Independently reopen and compare the actual service-sealed PE/PDB
    // CodeView-to-PDB identity; a hash alone does not establish symbols.
    crate::debug_symbol_verification::verify_exact_sealed_windows_pe_pdb(
        suspended.image(),suspended.pdb(),
        &cfg.expected_target_image_sha256,&cfg.expected_target_pdb_sha256,
    )?;
    let pid=unsafe{GetProcessId(suspended.process())};
    if pid==0{return Err(denied());}
    pair.attest_debugger_client(pipe)?;
    // The target-side original sealed PE/PDB hashes accompany the OS PID.
    // The debugger's separate administrator-pinned config must match BOTH.
    // No caller may supply a path, target PID, executable or attach command.
    // Fixed 141-byte sealed preparation receipt:
    // S(1) + kernel PID(4) + image(32) + PDB(32) + exact ORIGINAL human
    // Authorized entry SHA(32) + original supervisor session ID(40).
    // A different protected journal or restarted session cannot be swapped.
    let mut response=[0u8;141];
    response[0]=b'S';
    response[1..5].copy_from_slice(&pid.to_le_bytes());
    for (hex,offset) in [
        (&cfg.expected_target_image_sha256,5usize),
        (&cfg.expected_target_pdb_sha256,37usize),
        (&original.approved_entry_sha256,69usize),
    ]{
        if hex.len()!=64||!hex.bytes().all(|b|b.is_ascii_hexdigit()){
            return Err(denied());
        }
        for i in 0..32{
            response[offset+i]=u8::from_str_radix(&hex[i*2..i*2+2],16)
                .map_err(|_|denied())?;
        }
    }
    if original.session_id.len()!=40{return Err(denied());}
    response[101..141].copy_from_slice(original.session_id.as_bytes());
    write_all(pipe,&response)?;
    // G is NOT a grant. The target service must independently replay the
    // ORIGINAL host's fsynced Running receipt AFTER the one-use grant was
    // consumed. No caller-submitted PID/path/boolean or approval JSON.
    let mut grant=[0u8;1];read_fixed(pipe,&mut grant)?;
    if grant!=[b'G']||STOP.load(Ordering::SeqCst){return Err(denied());}
    pair.attest_debugger_client(pipe)?;
    {
        if unsafe{ImpersonateNamedPipeClient(pipe)}==0{return Err(denied());}
        struct Revert;
        impl Drop for Revert{
            fn drop(&mut self){if unsafe{RevertToSelf()}==0{std::process::abort();}}
        }
        let _revert=Revert;
        security::verify_original_approval_journal_acl(
            &cfg.original_supervisor_journal,&cfg.approved_host_user_sid,
        )?;
        fin04w2_authority::verify_original_execution_phase(
            &cfg.original_supervisor_journal,&cfg.original_job_id,
            &cfg.sealed_plan,&cfg.sealed_plan_sha256,JobPhase::Running,
        )?;
    }
    // The effective-socket denial matrix remains a separate native gate;
    // an OS WFP filter snapshot alone is not represented as such a pass.
    crate::windows_fin04w2_wfp::require_target_outbound_blocks(suspended.image())?;
    pair.reobserve()?;
    suspended.resume_after_original_running()?;
    write_all(pipe,b'T')?;
    // Native readback is independent of the LLDB-DAP stopped message.
    // V carries only the DAP-observed PC for comparison with actual Job,
    // loaded image, and two matching Win32 stopped-thread RIP samples.
    let mut observation=[0u8;9];read_fixed(pipe,&mut observation)?;
    if observation[0]!=b'V'||STOP.load(Ordering::SeqCst){return Err(denied());}
    pair.attest_debugger_client(pipe)?;
    let rip=u64::from_le_bytes(observation[1..9].try_into().map_err(|_|denied())?);
    if rip==0{return Err(denied());}
    let module=crate::windows_kernel_pc::observe_job_module(
        job.held_split_job_handle(),suspended.image(),
        suspended.image_sha256(),rip,
    )?;
    let sample=crate::windows_kernel_pc::sample_job_thread_rip(
        job.held_split_job_handle(),&module,
    )?;
    if !sample.two_matching_samples||!sample.job_membership_rechecked
        ||sample.process_id!=pid||sample.observed_rip!=rip{return Err(denied());}
    let mut observed=[0u8;13];observed[0]=b'P';
    observed[1..5].copy_from_slice(&sample.thread_id.to_le_bytes());
    observed[5..13].copy_from_slice(&sample.observed_rip.to_le_bytes());
    write_all(pipe,&observed)?;
    // Still no terminal receipt. Hold this service-owned process + Job
    // until the caller closes; dropping either lease kills the entire tree.
    let deadline=Instant::now()+Duration::from_secs(45);
    while !STOP.load(Ordering::SeqCst)&&Instant::now()<deadline{
        let mut available=0u32;
        if unsafe{PeekNamedPipe(pipe,ptr::null_mut(),0,ptr::null_mut(),
            &mut available,ptr::null_mut())}==0||available!=0{return Err(denied());}
        pair.attest_debugger_client(pipe)?;
        thread::sleep(Duration::from_millis(100));
    }
    Err(denied())
}
fn run_scm_service(handle:windows_sys::Win32::System::Services::SERVICE_STATUS_HANDLE)
    ->Result<(),SupervisorError>{
    STOP.store(false,Ordering::SeqCst);
    kernel::inspect_current_target_service_principal()?;
    let (root,cfg)=InstalledConfig::read()?;
    let job=kernel::create_bounded_empty_target_job()?;
    let acl=security::split_target_pipe_security()?;
    let raw=unsafe{CreateNamedPipeW(
        wide(ENDPOINT).as_ptr(),PIPE_ACCESS_DUPLEX|FIRST_PIPE_INSTANCE,
        PIPE_TYPE_MESSAGE|PIPE_READMODE_MESSAGE|PIPE_WAIT|PIPE_REJECT_REMOTE_CLIENTS,
        1,512,512,0,&acl.attributes,
    )};
    let pipe=Owned(raw);
    if raw.is_null()||raw==INVALID_HANDLE_VALUE{return Err(denied());}
    security::verify_split_target_pipe_security(raw)?;
    if unsafe{SetServiceStatus(
        handle,&status(SERVICE_RUNNING,SERVICE_ACCEPT_STOP,0),
    )}==0{return Err(denied());}
    if STOP.load(Ordering::SeqCst){return Ok(());}
    if unsafe{ConnectNamedPipe(raw,ptr::null_mut())}==0
        &&unsafe{GetLastError()}!=ERROR_PIPE_CONNECTED{return Err(denied());}
    if !STOP.load(Ordering::SeqCst){
        // Exactly ONE request per service lifetime. A failed/abandoned
        // one-use transaction cannot be retried by restarting this process.
        let _=prepare_one_target(raw,&cfg,&root,&job);
    }
    unsafe{DisconnectNamedPipe(raw);}
    let _=unsafe{SetServiceStatus(handle,&status(SERVICE_STOP_PENDING,0,0))};
    // Both the suspended child and this one service-owned Job are destroyed
    // by RAII on every exit, including peer disconnect or timeout.
    Ok(())
}
unsafe extern "system" fn main_service(_argc:u32,_argv:*mut *mut u16){
    let registered=unsafe{RegisterServiceCtrlHandlerExW(
        wide(SERVICE).as_ptr(),Some(control),ptr::null_mut())};
    if registered.is_null(){return;}
    let _=unsafe{SetServiceStatus(
        registered,&status(SERVICE_START_PENDING,0,0))};
    let finished=std::panic::catch_unwind(||run_scm_service(registered));
    let success=matches!(finished,Ok(Ok(())))&&STOP.load(Ordering::SeqCst);
    let _=unsafe{SetServiceStatus(
        registered,&status(SERVICE_STOPPED,0,if success{0}else{1}))};
}
pub fn dispatch()->Result<(),SupervisorError>{
    let service=wide(SERVICE);
    let table=[
        SERVICE_TABLE_ENTRYW{
            lpServiceName:service.as_ptr() as *mut u16,
            lpServiceProc:Some(main_service),
        },
        SERVICE_TABLE_ENTRYW{lpServiceName:ptr::null_mut(),lpServiceProc:None},
    ];
    if unsafe{StartServiceCtrlDispatcherW(table.as_ptr())}==0{
        return Err(denied());
    }
    Ok(())
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn unapproved_interactive_user_cannot_enter_target_scm(){
        assert!(kernel::inspect_current_target_service_principal().is_err());
    }
    #[test]
    fn target_ipc_is_not_public_host_or_legacy_92_byte_audit(){
        assert_eq!(ENDPOINT,r"\\.\pipe\LuIdeDebugTargetBroker.v1");
        assert_eq!(fin04w2_c2::WIRE_BYTES,74);
        assert_eq!(SERVICE,"LuIdeDebugTargetBroker");
    }
}