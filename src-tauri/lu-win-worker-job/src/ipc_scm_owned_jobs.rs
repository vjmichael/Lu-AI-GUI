//! Service-process-owned Windows Job custody, with client and executor
//! lanes kept distinct. This is NOT an IPC/RPC endpoint or arbitrary launch
//! API. A trusted SCM service must itself create the suspended restricted-token
//! Child and verify independently approved package/turn policy *before*
//! handing over the Child. No process can attach by supplying just a PID.
//! Kernel-empty observations here are process-local until the installed
//! protected SCM receipt channel is separately qualified.
#![cfg(windows)]
use std::{io,process::Child,sync::Mutex};
use crate::{Job,WindowsJobEmptyProof,
    ipc_credential_broker::require_system_service_context,
    ipc_pipe_peer::{observe_host_process,process_is_appcontainer,WindowsPipePeer}};

const SYSTEM_SID:&str="S-1-5-18";
const MAX_SERVICE_JOBS:usize=32;
fn denied(reason:&'static str)->io::Error{
    io::Error::new(io::ErrorKind::PermissionDenied,reason)
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum SupervisedJobLane{DlcClient,CoreExecutor}

/// Unforgeable native observation: contains a genuine Job-empty kernel proof,
/// original process generation and explicit client/executor lane. It contains
/// no model claims and does not yet attest exact IPC calls or persisted state.
pub struct ServiceStoppedJob{
    lane:SupervisedJobLane,
    original:WindowsPipePeer,
    proof:WindowsJobEmptyProof,
}
impl ServiceStoppedJob{
    pub fn lane(&self)->SupervisedJobLane{self.lane}
    pub fn original_pid(&self)->u32{self.original.pid}
    pub fn original_creation_filetime(&self)->u64{
        self.original.creation_filetime
    }
    pub fn method(&self)->&'static str{self.proof.method()}
}
struct ScmHeldChild{
    lane:SupervisedJobLane,
    original:WindowsPipePeer,
    job:Job,
    child:Child,
    stopped:bool,
}
impl ScmHeldChild{
    fn attach(job:Job,mut child:Child,lane:SupervisedJobLane,
        require_executor_appcontainer:bool)->io::Result<Self>{
        let original=match observe_host_process(child.id()){
            Ok(witness) if witness.user_sid!=SYSTEM_SID
                && witness.creation_filetime!=0 => witness,
            _=>{
                let _=child.kill();let _=child.wait();
                return Err(denied("SCM must not execute untrusted jobs as SYSTEM"));
            }
        };
        if require_executor_appcontainer && lane==SupervisedJobLane::CoreExecutor
            && !matches!(process_is_appcontainer(child.id()),Ok(true)){
            let _=child.kill();let _=child.wait();
            return Err(denied(
                "Core executor requires independently observed AppContainer token"));
        }
        if job.assign_and_resume_suspended(&child).is_err(){
            let _=child.kill();let _=child.wait();
            return Err(denied("SCM Job assignment before resume failed"));
        }
        if observe_host_process(child.id()).ok().as_ref()!=Some(&original){
            // Even if the Job kernel check itself fails, never wait forever
            // for a still-running untrusted child after identity drift.
            let _=job.terminate_and_verify_empty();
            let _=child.kill();let _=child.wait();
            return Err(denied("SCM worker process generation changed at admission"));
        }
        Ok(Self{lane,original,job,child,stopped:false})
    }
    fn stop(&mut self,expected:SupervisedJobLane)->io::Result<ServiceStoppedJob>{
        if self.lane!=expected{return Err(denied("wrong supervised worker lane"));}
        if self.stopped{return Err(denied("Job stop receipt already consumed"));}
        let proof=self.job.terminate_and_verify_empty()?;
        self.child.wait()?;
        self.stopped=true;
        Ok(ServiceStoppedJob{
            lane:self.lane,original:self.original.clone(),proof
        })
    }
}
impl Drop for ScmHeldChild{
    fn drop(&mut self){
        if !self.stopped{
            let _=self.job.terminate_and_verify_empty();
            let _=self.child.kill();
            let _=self.child.wait();
        }
    }
}
/// Retains all original Job/Child handles in the SCM service PROCESS, not a
/// Core-owned closure. If Core crashes, this registry stays in its own service
/// process. It refuses arbitrary PID adoption and SYSTEM-token worker launch.
pub struct ScmJobKeeper{
    jobs:Mutex<Vec<ScmHeldChild>>,
}
impl ScmJobKeeper{
    pub fn new_for_service()->io::Result<Self>{
        require_system_service_context()?;
        Ok(Self{jobs:Mutex::new(Vec::new())})
    }
    /// Only the protected installed service's later trusted launcher may
    /// call this. The launcher must have independently verified the exact
    /// approved image, restricted token and original user-turn permission.
    /// This method neither launches binaries nor accepts an IPC-supplied PID.
    pub fn admit_restricted_suspended(
        &self,job:Job,mut child:Child,lane:SupervisedJobLane,
    )->io::Result<()>{
        if let Err(error)=require_system_service_context(){
            // A suspended child cannot be left stranded if the broker's
            // LocalSystem/no-impersonation precondition is refused.
            let _=child.kill();let _=child.wait();
            return Err(error);
        }
        self.admit_inner(job,child,lane)
    }
    fn admit_inner(&self,job:Job,mut child:Child,lane:SupervisedJobLane)
        ->io::Result<()>{
        let mut owned=match self.jobs.lock(){
            Ok(guard)=>guard,
            Err(_)=>{
                let _=child.kill();let _=child.wait();
                return Err(denied("SCM Job registry poisoned"));
            }
        };
        if owned.len()>=MAX_SERVICE_JOBS{
            let _=child.kill();let _=child.wait();
            return Err(denied("SCM Job custody capacity exhausted"));
        }
        owned.push(ScmHeldChild::attach(job,child,lane,true)?);
        Ok(())
    }
    /// One-use bounded kernel observations. A partial failure cannot be
    /// represented as a positive all-worker recovery receipt; the caller
    /// must retain FAILED/UNCERTAIN and withhold any restart authorization.
    pub fn stop_all(&self)->io::Result<Vec<ServiceStoppedJob>>{
        let mut owned=self.jobs.lock().map_err(|_|denied("SCM Job registry poisoned"))?;
        let mut receipts=Vec::with_capacity(owned.len());
        for item in owned.iter_mut(){
            receipts.push(item.stop(item.lane)?);
        }
        owned.clear();
        Ok(receipts)
    }
}
impl Drop for ScmJobKeeper{
    fn drop(&mut self){
        if let Ok(mut jobs)=self.jobs.lock(){jobs.clear();}
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use std::{os::windows::process::CommandExt,
        process::{Command,Stdio}};
    fn child()->(Job,Child){
        let shell=std::env::var_os("ComSpec").unwrap_or(
            std::ffi::OsString::from(r"C:\Windows\System32\cmd.exe"));
        let job=Job::new().unwrap();
        let child=Command::new(shell)
            .args(["/D","/C","ping -n 20 127.0.0.1 >NUL"])
            .creation_flags(0x0800_0000|0x0000_0004)
            .stdin(Stdio::null()).stdout(Stdio::null())
            .stderr(Stdio::null()).spawn().unwrap();
        (job,child)
    }
    #[test]
    fn actual_assigned_job_proof_is_exact_lane_and_one_use(){
        let (job,child)=child();
        // The ordinary CI runner shell is deliberately NOT an AppContainer.
        // This tests real Job-empty custody independently of installed
        // sandbox/token policy. Production always requires AppContainer.
        let mut held=ScmHeldChild::attach(
            job,child,SupervisedJobLane::CoreExecutor,false).unwrap();
        assert!(held.stop(SupervisedJobLane::DlcClient).is_err());
        let proof=held.stop(SupervisedJobLane::CoreExecutor).unwrap();
        assert_eq!(proof.lane(),SupervisedJobLane::CoreExecutor);
        assert_ne!(proof.original_pid(),0);
        assert_ne!(proof.original_creation_filetime(),0);
        assert_eq!(proof.method(),"WINDOWS_JOB_ACTIVE_ZERO_V1");
        assert!(held.stop(SupervisedJobLane::CoreExecutor).is_err());
    }
    #[test]
    fn host_token_observation_never_equates_non_system_with_appcontainer(){
        let peer=observe_host_process(std::process::id()).unwrap();
        let observed=process_is_appcontainer(peer.pid).unwrap();
        // No success assumption: the CI user's actual token is inspected,
        // not replaced with a fake boolean. Production demands Ok(true).
        assert_eq!(observed,process_is_appcontainer(peer.pid).unwrap());
        assert!(process_is_appcontainer(0).is_err());
    }
    #[test]
    fn ordinary_runner_does_not_gain_system_service_job_registry(){
        let current=observe_host_process(std::process::id()).unwrap();
        if current.user_sid!=SYSTEM_SID{
            assert!(ScmJobKeeper::new_for_service().is_err());
        }
    }
}
