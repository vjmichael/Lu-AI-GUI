//! FIN-04W2-C2: narrow trusted native host handshake with the exact
//! independent restricted SCM debugger broker, never WebView/model-facing.
//! No caller-chosen command, child PID, DAP method or LLDB executable.
//!
//! The original native UI must have produced the actual Supervisor
//! Authorized state and live in-memory grant BEFORE this function is called.
//! C2 never generates user approval. Only the authenticated service can
//! report OS-custodied suspended admission. The original supervisor
//! continues to own and fsync the durable journal/restart policy.
#![cfg(all(windows,debug_assertions,feature="fin04w2-c2"))]
use crate::{
    fin04w2_c2::{self,C2JournalGate},
    windows_service_security as security,
    JobPhase,JobPlan,Supervisor,SupervisorError,
};
use std::{path::Path,ptr,thread,time::{Duration,Instant}};
use windows_sys::Win32::{
    Foundation::{CloseHandle,GetLastError,HANDLE,INVALID_HANDLE_VALUE,
        ERROR_FILE_NOT_FOUND,ERROR_PIPE_BUSY},
    Storage::FileSystem::{CreateFileW,ReadFile,WriteFile,OPEN_EXISTING},
    System::Pipes::PeekNamedPipe,
};
const PIPE:&str=r"\\.\pipe\LuIdeDebuggerBroker.v1";
const FILE_READ_DATA:u32=1;
const FILE_WRITE_DATA:u32=2;
const SYNCHRONIZE:u32=0x0010_0000;
const SECURITY_SQOS_PRESENT:u32=0x0010_0000;
const SECURITY_IMPERSONATION:u32=0x0002_0000;
fn denied()->SupervisorError{SupervisorError::NoContainment}
struct OwnedPipe(HANDLE);
impl Drop for OwnedPipe{
    fn drop(&mut self){
        if !self.0.is_null()&&self.0!=INVALID_HANDLE_VALUE{
            unsafe{CloseHandle(self.0);}
        }
    }
}
fn connect_exact_service(binary:&Path,sha:&str)
    ->Result<OwnedPipe,SupervisorError>{
    if !binary.is_absolute(){return Err(denied());}
    let until=Instant::now()+Duration::from_secs(8);
    while Instant::now()<until{
        let raw=unsafe{
            CreateFileW(
                security::ws(PIPE).as_ptr(),
                FILE_READ_DATA|FILE_WRITE_DATA|SYNCHRONIZE,0,
                ptr::null(),OPEN_EXISTING,
                SECURITY_SQOS_PRESENT|SECURITY_IMPERSONATION,
                ptr::null_mut(),
            )
        };
        if !raw.is_null()&&raw!=INVALID_HANDLE_VALUE{
            let owned=OwnedPipe(raw);
            // OS derives server PID, SCM process binary hash and the real
            // restricted LocalService principal + restricting service SID.
            security::authenticate_debugger_service_server(
                owned.0,binary,sha,
            )?;
            return Ok(owned);
        }
        let error=unsafe{GetLastError()};
        if error!=ERROR_FILE_NOT_FOUND&&error!=ERROR_PIPE_BUSY{
            return Err(denied());
        }
        thread::sleep(Duration::from_millis(25));
    }
    Err(denied())
}
fn read_fixed(pipe:HANDLE,out:&mut [u8],timeout:Duration)
    ->Result<(),SupervisorError>{
    if out.is_empty()||out.len()>17{return Err(denied());}
    let until=Instant::now()+timeout;
    loop{
        if Instant::now()>=until{return Err(denied());}
        let mut count=0u32;
        if unsafe{
            PeekNamedPipe(pipe,ptr::null_mut(),0,
                ptr::null_mut(),&mut count,ptr::null_mut())
        }==0||count>out.len() as u32{return Err(denied());}
        if count==out.len() as u32{break;}
        thread::sleep(Duration::from_millis(20));
    }
    let mut read=0u32;
    if unsafe{
        ReadFile(pipe,out.as_mut_ptr().cast(),out.len() as u32,
            &mut read,ptr::null_mut())
    }==0||read!=out.len() as u32{return Err(denied());}
    Ok(())
}
fn write_fixed(pipe:HANDLE,bytes:&[u8])->Result<(),SupervisorError>{
    if bytes.len()!=fin04w2_c2::WIRE_BYTES&&bytes.len()!=1{
        return Err(denied());
    }
    let mut written=0u32;
    if unsafe{
        WriteFile(pipe,bytes.as_ptr().cast(),bytes.len() as u32,
            &mut written,ptr::null_mut())
    }==0||written!=bytes.len() as u32{return Err(denied());}
    Ok(())
}
/// This type has NO public constructor or caller-settable OS Boolean.
/// It is minted only after the real SCM server was attested against its
/// installed image and token, the fresh challenge was bound to a fixed
/// original Job+plan request, and the server acknowledged suspended
/// membership in its own kernel-held Job. The native host cannot itself
/// nominate an arbitrary executor, PID, path or resource limit.
pub(crate) struct TrustedC2Admission{
    job_id:String,
    plan_sha:String,
    service_sha:String,
}
impl TrustedC2Admission{
    pub(crate) fn matches(&self,job_id:&str,plan:&JobPlan)
        ->Result<bool,SupervisorError>{
        Ok(self.job_id==job_id
            &&self.plan_sha==crate::fingerprint(plan)?
            &&self.service_sha.len()==64
            &&self.service_sha.bytes().all(|b|b.is_ascii_hexdigit()))
    }
}
struct Recovery<'a>{
    supervisor:&'a Supervisor,job:&'a str,done:bool,
}
impl Drop for Recovery<'_>{
    fn drop(&mut self){
        if !self.done{
            // Even a lost pre-resume service acknowledgment may have
            // reserved the original one-use admission. Never replay it.
            let _=self.supervisor.native_quarantine_after_error(self.job);
        }
    }
}
/// Original native host entry. This is NOT a Tauri command and cannot
/// synthesize human consent or obtain an OsContainmentProof from CI flags.
pub fn run_fixed_after_original_approval(
    supervisor:&Supervisor,job_id:&str,plan:&JobPlan,
    pinned_service_binary:&Path,pinned_service_sha:&str,
)->Result<(),SupervisorError>{
    let source=crate::fingerprint(plan)?;
    // Do not disclose plan bytes or journal paths in diagnostic output.
    // Keep the exact pre-admission guard; stage reporting never admits a job.
    if supervisor.inspect()?.iter().all(|j|
        j.job_id!=job_id||j.plan_sha256!=source
            ||j.phase!=JobPhase::Authorized||j.requires_reconciliation){
        eprintln!("FIN04W2_C2_STAGE=LIVE_SUPERVISOR_AUTHORIZED_MISMATCH");
        return Err(denied());
    }
    fin04w2_c2::verify_original_path(
        &supervisor.journal_path,job_id,plan,C2JournalGate::Authorized,
    ).map_err(|error| {
        eprintln!("FIN04W2_C2_STAGE=ORIGINAL_AUTHORIZED_JOURNAL_DENIED");
        error
    })?;
    let mut recovery=Recovery{supervisor,job:job_id,done:false};
    let pipe=connect_exact_service(pinned_service_binary,pinned_service_sha).map_err(|error| { eprintln!("FIN04W2_C2_STAGE=SERVICE_PEER_ATTESTATION_DENIED"); error })?;
    let mut challenge=[0u8;17];
    read_fixed(pipe.0,&mut challenge,Duration::from_secs(5)).map_err(|error| { eprintln!("FIN04W2_C2_STAGE=SERVICE_CHALLENGE_DENIED"); error })?;
    if challenge[0]!=b'C'||challenge[1..].iter().all(|v|*v==0){
        eprintln!("FIN04W2_C2_STAGE=SERVICE_NONCE_SHAPE_DENIED");
        return Err(denied());
    }
    let mut nonce=[0u8;16];nonce.copy_from_slice(&challenge[1..]);
    let packet=fin04w2_c2::approved_request(job_id,plan,&nonce)?;
    write_fixed(pipe.0,&packet).map_err(|error|{eprintln!("FIN04W2_C2_STAGE=FIXED_REQUEST_WRITE_DENIED");error})?;
    let mut status=[0u8;1];
    read_fixed(pipe.0,&mut status,Duration::from_secs(8)).map_err(|error| { eprintln!("FIN04W2_C2_STAGE=SUSPENDED_JOB_ACK_DENIED"); error })?;
    if status[0]!=b'S'{eprintln!("FIN04W2_C2_STAGE=SUSPENDED_JOB_ACK_WRONG");return Err(denied());}
    let admission=TrustedC2Admission{
        job_id:job_id.to_owned(),plan_sha:source,
        service_sha:pinned_service_sha.to_owned(),
    };
    supervisor.consume_grant_after_c2_service_admission(
        job_id,plan,&admission,
    ).map_err(|error| { eprintln!("FIN04W2_C2_STAGE=ONE_USE_GRANT_DENIED"); error })?;
    supervisor.native_mark_running(job_id)?;
    write_fixed(pipe.0,b"G").map_err(|error|{eprintln!("FIN04W2_C2_STAGE=RUN_GRANT_WRITE_DENIED");error})?;
    read_fixed(pipe.0,&mut status,Duration::from_secs(18)).map_err(|error| { eprintln!("FIN04W2_C2_STAGE=KERNEL_TREE_EMPTY_ACK_DENIED"); error })?;
    if status[0]!=b'X'{eprintln!("FIN04W2_C2_STAGE=KERNEL_TREE_EMPTY_ACK_WRONG");return Err(denied());}
    supervisor.native_mark_release_pending(job_id)?;
    write_fixed(pipe.0,b"R").map_err(|error|{eprintln!("FIN04W2_C2_STAGE=RELEASE_WRITE_DENIED");error})?;
    read_fixed(pipe.0,&mut status,Duration::from_secs(8)).map_err(|error|{eprintln!("FIN04W2_C2_STAGE=SERVICE_RELEASE_ACK_DENIED");error})?;
    if status[0]!=b'F'{eprintln!("FIN04W2_C2_STAGE=SERVICE_RELEASE_ACK_WRONG");return Err(denied());}
    let done=supervisor.native_finish(job_id,true,true,true)?;
    if done.phase!=JobPhase::Completed||done.requires_reconciliation{
        eprintln!("FIN04W2_C2_STAGE=ORIGINAL_TERMINAL_RECEIPT_DENIED");
        return Err(denied());
    }
    recovery.done=true;
    Ok(())
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]fn no_caller_can_construct_an_unbound_service_witness(){
        let witness=TrustedC2Admission{
            job_id:format!("job-{}","a".repeat(32)),
            plan_sha:"f".repeat(64),service_sha:"b".repeat(64),
        };
        let plan=JobPlan{
            project_id:format!("ide-{}","a".repeat(32)),
            recipe_id:"dap_lldb_project".into(),source_head:"b".repeat(40),
            input_snapshot_sha256:"c".repeat(64),manifest_sha256:"d".repeat(64),
            toolchain_sha256:"e".repeat(64),
            debug_target_binding_sha256:Some("f".repeat(64)),
            runtime_limit_ms:90_000,memory_limit_bytes:512*1024*1024,
            process_limit:16,log_limit_bytes:65536,
            effect_class:"debug_inspection_isolated".into(),
        };
        assert!(!witness.matches(&witness.job_id,&plan).unwrap());
    }
}
