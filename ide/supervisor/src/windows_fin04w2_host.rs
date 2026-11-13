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
    mem::size_of, path::Path, ptr, thread,
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
    if bytes.len()!=92 {return Err(denied());}
    let mut written=0u32;
    if unsafe{
        WriteFile(pipe,bytes.as_ptr().cast(),
            bytes.len() as u32,&mut written,ptr::null_mut())
    }==0 || written!=92 {return Err(denied());}
    Ok(())
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
    let mut packet=[0u8;92];
    packet[..8].copy_from_slice(b"LUFW2A01");
    packet[8]=1;
    packet[9]=2; // ONLY LaunchSealed; no arbitrary method.
    packet[12..44].copy_from_slice(&digest(&plan_sha)?);
    packet[44..76].copy_from_slice(&digest(
        plan.debug_target_binding_sha256.as_ref().ok_or_else(denied)?,
    )?);
    packet[76..92].copy_from_slice(&challenge[1..17]);
    write_exact(pipe.0,&packet)?;
    let mut response=[0u8;1];
    // A is only audit; R/READY is not a terminal. X must originate from
    // the authenticated, original service after Job empty+tree reaped.
    read_exact_bounded(pipe.0,&mut response)?;
    if response[0]!=b'X' {return Err(denied());}
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
