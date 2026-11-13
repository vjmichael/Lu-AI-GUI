//! FIN-04W2-C trusted NATIVE host handoff, never a Tauri command.
//!
//! Original UI must first call the existing Supervisor::record_human_approval
//! and original contained-worker admission (which consumes the in-memory grant
//! and fsyncs AdmissionPending). This code CANNOT mint approval itself.
//! It then verifies a separately installed restricted SCM debugger broker,
//! obtains an unguessable service challenge and submits exactly one fixed
//! request for the admin-pinned benign fixture. No client-chosen path,
//! arbitrary PID, shell, adapter arguments or LLDB command enters the pipe.
//! A positive fixed fixture reply is not terminal/PE/PDB debugger acceptance.
#![cfg(all(windows, debug_assertions, feature="fin04w2-approved-fixed-exec"))]
use crate::{fin04w2_authority, windows_service_security as security,
    JobPhase, JobPlan, Supervisor, SupervisorError};
use std::{
    path::Path, ptr, thread,
    time::{Duration,Instant},
};
use windows_sys::Win32::{
    Foundation::{CloseHandle,GetLastError,ERROR_FILE_NOT_FOUND,ERROR_PIPE_BUSY,
        HANDLE,INVALID_HANDLE_VALUE},
    Storage::FileSystem::{CreateFileW,ReadFile,WriteFile,OPEN_EXISTING},
    System::Pipes::PeekNamedPipe,
};
const ENDPOINT: &str=r"\\.\pipe\LuIdeDebuggerBroker.v1";
const FILE_READ_DATA: u32=0x0001;
const FILE_WRITE_DATA: u32=0x0002;
const SYNCHRONIZE: u32=0x0010_0000;
const SECURITY_SQOS_PRESENT: u32=0x0010_0000;
const SECURITY_IMPERSONATION: u32=0x0002_0000;
fn denied()->SupervisorError {SupervisorError::NoContainment}
struct Pipe(HANDLE);
impl Drop for Pipe {
    fn drop(&mut self) { if !self.0.is_null() && self.0!=INVALID_HANDLE_VALUE {
        unsafe {CloseHandle(self.0);}
    }}
}
fn digest(sha:&str)->Result<[u8;32],SupervisorError>{
    if sha.len()!=64 || !sha.bytes().all(|b|b.is_ascii_hexdigit()){
        return Err(denied());
    }
    let mut out=[0u8;32];
    for (i,v) in out.iter_mut().enumerate(){
        *v=u8::from_str_radix(&sha[i*2..i*2+2],16).map_err(|_|denied())?;
    }
    Ok(out)
}
fn connect(service_binary:&Path,service_sha:&str)
    ->Result<Pipe,SupervisorError>{
    let deadline=Instant::now()+Duration::from_secs(8);
    while Instant::now()<deadline {
        let raw=unsafe {
            CreateFileW(
                security::ws(ENDPOINT).as_ptr(),
                FILE_READ_DATA|FILE_WRITE_DATA|SYNCHRONIZE,0,
                ptr::null(),OPEN_EXISTING,
                SECURITY_SQOS_PRESENT|SECURITY_IMPERSONATION,
                ptr::null_mut(),
            )
        };
        if raw!=INVALID_HANDLE_VALUE && !raw.is_null(){
            let pipe=Pipe(raw);
            security::authenticate_debugger_service_server(
                pipe.0,service_binary,service_sha,
            )?;
            return Ok(pipe);
        }
        let code=unsafe{GetLastError()};
        if code!=ERROR_FILE_NOT_FOUND && code!=ERROR_PIPE_BUSY{
            return Err(denied());
        }
        thread::sleep(Duration::from_millis(40));
    }
    Err(denied())
}
fn read_exact_bounded(pipe:HANDLE,bytes:&mut [u8])
    ->Result<(),SupervisorError>{
    if bytes.is_empty() || bytes.len()>32 {return Err(denied());}
    let deadline=Instant::now()+Duration::from_secs(5);
    loop {
        if Instant::now()>=deadline {return Err(denied());}
        let mut available=0u32;
        if unsafe{
            PeekNamedPipe(pipe,ptr::null_mut(),0,ptr::null_mut(),
                &mut available,ptr::null_mut())
        }==0 {return Err(denied());}
        if available>bytes.len() as u32{return Err(denied());}
        if available==bytes.len() as u32{break;}
        thread::sleep(Duration::from_millis(15));
    }
    let mut received=0u32;
    if unsafe {
        ReadFile(pipe,bytes.as_mut_ptr().cast(),
            bytes.len() as u32,&mut received,ptr::null_mut())
    }==0 || received!=bytes.len() as u32 {return Err(denied());}
    Ok(())
}
fn write_exact(pipe:HANDLE,bytes:&[u8])->Result<(),SupervisorError>{
    if bytes.len()!=108 {return Err(denied());}
    let mut written=0u32;
    if unsafe{
        WriteFile(pipe,bytes.as_ptr().cast(),
            bytes.len() as u32,&mut written,ptr::null_mut())
    }==0 || written!=108 {return Err(denied());}
    Ok(())
}
fn send_fixed_signal(pipe:HANDLE, marker:u8)->Result<(),SupervisorError>{
    let mut written=0u32;
    if unsafe {
        WriteFile(pipe,(&marker as *const u8).cast(),1,
            &mut written,ptr::null_mut())
    }==0 || written!=1 {return Err(denied());}
    Ok(())
}
// If the host loses the service, exits early, cannot fsync a receipt or
// observes an invalid fixed signal, preserve an original RecoveryRequired
// receipt rather than inventing Completed, silently restarting or replaying
// a consumed creator grant. The independently held service Job is dropped
// and terminated on its corresponding pipe loss.
struct RecoveryGuard<'a>{
    supervisor:&'a Supervisor,
    job_id:&'a str,
    completed:bool,
}
impl Drop for RecoveryGuard<'_>{
    fn drop(&mut self){
        if !self.completed {
            let _=self.supervisor.native_quarantine_after_error(self.job_id);
        }
    }
}
/// The native IDE host may supply its existing original approval-and-OS
/// admission workflow as a synchronous callback AFTER all sealed-source
/// preflight. That trusted workflow MUST obtain a genuine current desktop
/// confirmation, call Supervisor::record_human_approval, and consume the
/// original grant only after its independently verified OS custody proof.
/// No Boolean, CI flag, model result, WebView message or shell command can
/// stand in for that callback. This adapter refuses to proceed unless the
/// original supervisor actually reached a fresh fsynced AdmissionPending.
/// We deliberately do NOT fabricate an OsContainmentProof in this module.
pub fn execute_after_original_native_approval<F>(
    supervisor:&Supervisor,
    job_id:&str,
    sealed_plan:&JobPlan,
    current_host_observed_source_head:&str,
    approved_service_binary:&Path,
    approved_service_sha256:&str,
    original_native_approval_and_admission:F,
)->Result<(),SupervisorError>
where F:FnOnce(&Supervisor,&str,&JobPlan)->Result<(),SupervisorError>{
    let sha=crate::fingerprint(sealed_plan)?;
    if sealed_plan.source_head!=current_host_observed_source_head
        || !crate::validate_plan(sealed_plan)
        || sealed_plan.recipe_id!="dap_lldb_project"
        || supervisor.inspect()?.iter().all(|job|
            job.job_id!=job_id || job.plan_sha256!=sha
                || job.phase!=JobPhase::Prepared
                || job.requires_reconciliation
        ) {
        return Err(SupervisorError::WrongProjectOrSource);
    }
    original_native_approval_and_admission(
        supervisor,job_id,sealed_plan,
    )?;
    // The normal entry checks the actual ORIGINAL in-memory one-use grant
    // consumption + durable journal. A callback which merely returns Ok,
    // prints "approved" or replays prior approval cannot pass.
    submit_original_approved_fixed_job(
        supervisor,job_id,sealed_plan,
        approved_service_binary,approved_service_sha256,
    )
}
/// Native bridge may be called only AFTER the original host obtained a
/// genuine, fresh desktop approval and consumed it through original OS
/// admission. It refuses to convert a prepared/authorized-only grant or a
/// reopened journal into service authority. A successful return certifies
/// only the ORIGINAL fixed benign fixture's service-observed completion:
/// terminal journal receipt/LLDB are separate work, never faked here.
pub fn submit_original_approved_fixed_job(
    supervisor:&Supervisor,
    job_id:&str,
    plan:&JobPlan,
    approved_service_binary:&Path,
    approved_service_sha256:&str,
)->Result<(),SupervisorError>{
    let plan_sha=crate::fingerprint(plan)?;
    if !crate::validate_plan(plan)
        || plan.recipe_id!="dap_lldb_project"
        || plan.debug_target_binding_sha256.is_none()
        || supervisor.inspect()?.iter().all(|j|
            j.job_id!=job_id || j.plan_sha256!=plan_sha
                || j.phase!=JobPhase::AdmissionPending
                || j.requires_reconciliation
        )
    {return Err(denied());}
    let mut recovery=RecoveryGuard {
        supervisor,job_id,completed:false,
    };
    // This is a privileged, internal native-host reference, not a WebView
    // filename. The service still re-reads original journal from its OWN
    // independently installed and ACL-pinned path after client attestation.
    let original=fin04w2_authority::verify_original_journal_file(
        &supervisor.journal_path,job_id,plan,&plan_sha,
    )?;
    if original.job_id!=job_id {return Err(denied());}
    let pipe=connect(approved_service_binary,approved_service_sha256)?;
    let mut challenge=[0u8;17];
    read_exact_bounded(pipe.0,&mut challenge)?;
    if challenge[0]!=b'C' || challenge[1..].iter().all(|b|*b==0){
        return Err(denied());
    }
    // The original native Job ID is a separate fixed 16-byte binding:
    // another job with identical source/plan SHA cannot spend this grant.
    if job_id.len()!=36 || !job_id.starts_with("job-") {
        return Err(denied());
    }
    let mut job_bytes=[0u8;16];
    for (i,item) in job_bytes.iter_mut().enumerate(){
        *item=u8::from_str_radix(&job_id[4+i*2..4+i*2+2],16)
            .map_err(|_|denied())?;
    }
    let mut packet=[0u8;108];
    packet[..8].copy_from_slice(b"LUFW2A01");
    packet[8]=1;
    packet[9]=2; // ONLY LaunchSealed; no arbitrary method.
    packet[12..44].copy_from_slice(&digest(&plan_sha)?);
    packet[44..76].copy_from_slice(&digest(
        plan.debug_target_binding_sha256.as_ref().ok_or_else(denied)?,
    )?);
    packet[76..92].copy_from_slice(&challenge[1..17]);
    packet[92..108].copy_from_slice(&job_bytes);
    write_exact(pipe.0,&packet)?;
    let mut response=[0u8;1];
    // S is emitted only by the original restricted service after it has
    // created a suspended hash-pinned child INSIDE its exact held Job.
    // Fsync the original Running receipt before allowing ANY instruction.
    read_exact_bounded(pipe.0,&mut response)?;
    if response[0]!=b'S' {return Err(denied());}
    supervisor.native_mark_running(job_id)?;
    send_fixed_signal(pipe.0,b'G')?;
    // X is independently OS-verified empty tree plus successful benign
    // worker/descendant exit, but the SCM Job is still retained.
    read_exact_bounded(pipe.0,&mut response)?;
    if response[0]!=b'X' {return Err(denied());}
    supervisor.native_mark_release_pending(job_id)?;
    send_fixed_signal(pipe.0,b'R')?;
    // F is sent only AFTER the service releases the original Job handle.
    // No Completed receipt is allowed on early pipe failure or unverified
    // release, even if the process wrote a successful stdout message.
    read_exact_bounded(pipe.0,&mut response)?;
    if response[0]!=b'F' {return Err(denied());}
    let terminal=supervisor.native_finish(job_id,true,true,true)?;
    if terminal.phase!=JobPhase::Completed
        || terminal.requires_reconciliation {return Err(denied());}
    recovery.completed=true;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_frame_decoder_rejects_any_missing_source_sha(){
        assert!(digest("").is_err());
        assert!(digest(&"a".repeat(63)).is_err());
        assert!(digest(&"z".repeat(64)).is_err());
        assert_eq!(digest(&"ab".repeat(32)).unwrap(),[0xab;32]);
    }
}
