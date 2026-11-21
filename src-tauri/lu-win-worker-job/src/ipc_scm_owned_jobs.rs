//! Fixed SCM-process Job custody with exact original-call binding.
//! The installed service must independently authenticate its control request,
//! selected executable, original user-turn authority and restricted token
//! BEFORE calling this registry. A well-formed binding is NOT a grant.
//! No arbitrary PID adoption, model-origin launch, or client-supplied Job proof.
//!
//! These observations are true OS Job-empty proof held in the SCM PROCESS,
//! but are NOT YET independent durable cross-Core-crash signed receipts.
//! Do not reopen the Core journal from this source-only evidence.
#![cfg(windows)]
use std::{
    collections::{BTreeMap,BTreeSet}, io, process::Child, sync::Mutex,
};
use crate::{
    Job,WindowsJobEmptyProof,
    ipc_credential_broker::require_system_service_context,
    ipc_pipe_peer::{observe_host_process,process_is_appcontainer,WindowsPipePeer},
};

const SYSTEM_SID:&str="S-1-5-18";
const MAX_SERVICE_JOBS:usize=32;
const MAX_SERVICE_CALLS:usize=1024;

fn denied(reason:&'static str)->io::Error{
    io::Error::new(io::ErrorKind::PermissionDenied,reason)
}

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum SupervisedJobLane{DlcClient,CoreExecutor}
impl SupervisedJobLane{
    fn discriminator(self)->u8{
        match self{Self::DlcClient=>1,Self::CoreExecutor=>2}
    }
}

/// The exact host-observed original call that both the DLC and native executor
/// Job must belong to. Its validation is syntactic ONLY: the real SCM service
/// has to verify the installer, authenticated host request and original turn
/// authority independently. No public member or constructor grants execution.
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct ServiceCallBinding{
    call_id:String,
    fingerprint_sha256:String,
    module_id:String,
    session_ref:String,
    core_epoch_ref:String,
    conversation_ref:String,
    user_turn_ref:String,
    task_ref:String,
    workspace_ref:String,
}
impl ServiceCallBinding{
    #[allow(clippy::too_many_arguments)]
    pub fn checked(
        call_id:&str,fingerprint_sha256:&str,module_id:&str,
        session_ref:&str,core_epoch_ref:&str,conversation_ref:&str,
        user_turn_ref:&str,task_ref:&str,workspace_ref:&str,
    )->io::Result<Self>{
        fn hex64(v:&str)->bool{
            v.len()==64&&v.bytes().all(|b|b.is_ascii_digit()||(b'a'..=b'f').contains(&b))
        }
        fn scoped(v:&str)->bool{
            !v.is_empty()&&v.len()<=128
                &&v.chars().all(|c|!c.is_control())
        }
        if !hex64(call_id)||!hex64(fingerprint_sha256)
            ||!scoped(module_id)||!scoped(session_ref)
            ||!scoped(core_epoch_ref)||!scoped(conversation_ref)
            ||!scoped(user_turn_ref)||!scoped(task_ref)
            ||!scoped(workspace_ref){
            return Err(denied("SCM original-call identity is incomplete or malformed"));
        }
        Ok(Self{
            call_id:call_id.into(),fingerprint_sha256:fingerprint_sha256.into(),
            module_id:module_id.into(),session_ref:session_ref.into(),
            core_epoch_ref:core_epoch_ref.into(),
            conversation_ref:conversation_ref.into(),
            user_turn_ref:user_turn_ref.into(),task_ref:task_ref.into(),
            workspace_ref:workspace_ref.into(),
        })
    }
    pub fn call_id(&self)->&str{&self.call_id}
    pub fn fingerprint_sha256(&self)->&str{&self.fingerprint_sha256}
    pub fn module_id(&self)->&str{&self.module_id}
    pub fn session_ref(&self)->&str{&self.session_ref}
    pub fn core_epoch_ref(&self)->&str{&self.core_epoch_ref}
    pub fn conversation_ref(&self)->&str{&self.conversation_ref}
    pub fn user_turn_ref(&self)->&str{&self.user_turn_ref}
    pub fn task_ref(&self)->&str{&self.task_ref}
    pub fn workspace_ref(&self)->&str{&self.workspace_ref}
}

/// Non-cloneable, one-use native witness. The exact original call, lane,
/// original worker PID+birth and supervisor PID+birth are all captured from
/// the real OS at admission. This witness is process-local, not a persisted
/// or signed proof that a restarted Core may accept as executor custody.
pub struct ServiceStoppedJob{
    binding:ServiceCallBinding,
    lane:SupervisedJobLane,
    supervisor:WindowsPipePeer,
    original:WindowsPipePeer,
    proof:WindowsJobEmptyProof,
}
impl ServiceStoppedJob{
    pub fn binding(&self)->&ServiceCallBinding{&self.binding}
    pub fn lane(&self)->SupervisedJobLane{self.lane}
    pub fn original_pid(&self)->u32{self.original.pid}
    pub fn original_creation_filetime(&self)->u64{
        self.original.creation_filetime
    }
    pub fn supervisor_pid(&self)->u32{self.supervisor.pid}
    pub fn supervisor_creation_filetime(&self)->u64{
        self.supervisor.creation_filetime
    }
    pub fn method(&self)->&'static str{self.proof.method()}
}

struct ScmHeldChild{
    binding:ServiceCallBinding,
    lane:SupervisedJobLane,
    supervisor:WindowsPipePeer,
    original:WindowsPipePeer,
    job:Job,
    child:Child,
    stopped:bool,
}
impl ScmHeldChild{
    fn attach(
        job:Job,mut child:Child,lane:SupervisedJobLane,
        binding:ServiceCallBinding,require_executor_appcontainer:bool,
    )->io::Result<Self>{
        let supervisor=match observe_host_process(std::process::id()){
            Ok(witness)=>witness,
            Err(_)=>{
                let _=child.kill();let _=child.wait();
                return Err(denied("SCM supervisor process identity unavailable"));
            }
        };
        if supervisor.creation_filetime==0{
            let _=child.kill();let _=child.wait();
            return Err(denied("SCM supervisor process generation unavailable"));
        }
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
            let _=job.terminate_and_verify_empty();
            let _=child.kill();let _=child.wait();
            return Err(denied("SCM worker process generation changed at admission"));
        }
        Ok(Self{binding,lane,supervisor,original,job,child,stopped:false})
    }
    fn stop_exact(
        &mut self,expected_lane:SupervisedJobLane,
        expected_binding:&ServiceCallBinding,
    )->io::Result<ServiceStoppedJob>{
        if self.lane!=expected_lane||&self.binding!=expected_binding{
            return Err(denied("SCM stop does not match original call or Job lane"));
        }
        if self.stopped{return Err(denied("Job stop receipt already consumed"));}
        let proof=self.job.terminate_and_verify_empty()?;
        self.child.wait()?;
        self.stopped=true;
        Ok(ServiceStoppedJob{
            binding:self.binding.clone(),lane:self.lane,
            supervisor:self.supervisor.clone(),
            original:self.original.clone(),proof,
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

#[derive(Default)]
struct ServiceRegistry{
    jobs:Vec<ScmHeldChild>,
    // Kept for this service epoch to reject reuse of an already stopped
    // call/lane. 1024 is a hard fail-closed limit until the independently
    // protected durable receipt/ack channel is installed.
    calls:BTreeMap<String,ServiceCallBinding>,
    seen_lanes:BTreeSet<(String,u8)>,
}

/// The Job and process handles stay in an SCM-owned process, never in the
/// client/Core UI. Per-call identity is immutable even if one lane ends first.
pub struct ScmJobKeeper{
    registry:Mutex<ServiceRegistry>,
}
impl ScmJobKeeper{
    pub fn new_for_service()->io::Result<Self>{
        require_system_service_context()?;
        Ok(Self{registry:Mutex::new(ServiceRegistry::default())})
    }
    /// Only the later protected authenticated service launcher may supply a
    /// child. The launcher MUST construct that child suspended with its own
    /// approved image, original-grant and restricted token; accepting just a
    /// PID, executable path or IPC-provided binding is forbidden.
    pub fn admit_restricted_suspended(
        &self,job:Job,mut child:Child,lane:SupervisedJobLane,
        binding:ServiceCallBinding,
    )->io::Result<()>{
        if let Err(error)=require_system_service_context(){
            let _=child.kill();let _=child.wait();
            return Err(error);
        }
        self.admit_inner(job,child,lane,binding,true)
    }
    fn admit_inner(
        &self,job:Job,mut child:Child,lane:SupervisedJobLane,
        binding:ServiceCallBinding,require_executor_appcontainer:bool,
    )->io::Result<()>{
        let mut state=match self.registry.lock(){
            Ok(guard)=>guard,
            Err(_)=>{
                let _=child.kill();let _=child.wait();
                return Err(denied("SCM Job registry poisoned"));
            }
        };
        let key=(binding.call_id.clone(),lane.discriminator());
        let conflict=state.calls.get(&binding.call_id)
            .is_some_and(|existing|existing!=&binding);
        if state.jobs.len()>=MAX_SERVICE_JOBS
            ||(state.calls.len()>=MAX_SERVICE_CALLS
                &&!state.calls.contains_key(&binding.call_id))
            ||conflict||state.seen_lanes.contains(&key){
            let _=child.kill();let _=child.wait();
            return Err(denied("SCM worker full, duplicate lane, or original-call drift"));
        }
        // The native kernel checks must succeed BEFORE reserving a call/lane.
        let held=ScmHeldChild::attach(
            job,child,lane,binding.clone(),require_executor_appcontainer)?;
        state.seen_lanes.insert(key);
        state.calls.insert(binding.call_id.clone(),binding);
        state.jobs.push(held);
        Ok(())
    }
    /// A stop is accepted only for the exact original call, original trusted
    /// turn scope and intended lane. A client cannot stop an unrelated Job.
    /// No executor receipt leaves this process as independent durable proof.
    pub fn stop_exact_for_service(
        &self,binding:&ServiceCallBinding,lane:SupervisedJobLane,
    )->io::Result<ServiceStoppedJob>{
        require_system_service_context()?;
        self.stop_exact_inner(binding,lane)
    }
    fn stop_exact_inner(
        &self,binding:&ServiceCallBinding,lane:SupervisedJobLane,
    )->io::Result<ServiceStoppedJob>{
        let mut state=self.registry.lock()
            .map_err(|_|denied("SCM Job registry poisoned"))?;
        let index=state.jobs.iter().position(|item|
            item.lane==lane&&item.binding.call_id==binding.call_id)
            .ok_or_else(||denied("SCM original call or lane not active"))?;
        let proof=state.jobs[index].stop_exact(lane,binding)?;
        state.jobs.remove(index);
        Ok(proof)
    }
    /// Shut down every remaining Job before the service exits. If any
    /// observation fails, never return a partial positive all-worker receipt;
    /// Drop still kills unproven children.
    pub fn stop_all(&self)->io::Result<Vec<ServiceStoppedJob>>{
        let mut state=self.registry.lock()
            .map_err(|_|denied("SCM Job registry poisoned"))?;
        let mut receipts=Vec::with_capacity(state.jobs.len());
        let mut failed=false;
        for mut item in std::mem::take(&mut state.jobs){
            let identity=item.binding.clone();
            match item.stop_exact(item.lane,&identity){
                Ok(witness)=>receipts.push(witness),
                Err(_)=>failed=true,
            }
        }
        if failed{
            Err(denied("SCM all-worker OS termination evidence incomplete"))
        }else{Ok(receipts)}
    }
}
impl Drop for ScmJobKeeper{
    fn drop(&mut self){
        if let Ok(mut state)=self.registry.lock(){state.jobs.clear();}
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    use std::{os::windows::process::CommandExt,process::{Command,Stdio}};
    fn binding()->ServiceCallBinding{
        ServiceCallBinding::checked(
            &"a".repeat(64),&"b".repeat(64),"lu.ide",
            "session-one","core-epoch-one","conversation-1",
            "user-turn-1","task-1","workspace-1",
        ).unwrap()
    }
    fn suspended_fixture_child()->(Job,Child){
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
    fn exact_call_binding_rejects_incomplete_or_foreign_provenance(){
        assert!(ServiceCallBinding::checked(
            "untrusted",&"b".repeat(64),"lu.ide","s","e","c","t","q","w"
        ).is_err());
        assert!(ServiceCallBinding::checked(
            &"A".repeat(64),&"b".repeat(64),"lu.ide","s","e","c","t","q","w"
        ).is_err());
        assert!(ServiceCallBinding::checked(
            &"a".repeat(64),&"b".repeat(64),"lu.ide","s","e","c","","q","w"
        ).is_err());
        assert!(ServiceCallBinding::checked(
            &"a".repeat(64),&"b".repeat(64),"lu.ide","s","e","c","t","q","bad\nscope"
        ).is_err());
    }
    #[test]
    fn actual_assigned_job_proof_is_exact_call_lane_and_one_use(){
        let (job,child)=suspended_fixture_child();
        // CI's ordinary user is NOT an AppContainer; this tests real Job
        // accounting only. Production requires independent token proof.
        let binding=binding();
        let mut held=ScmHeldChild::attach(
            job,child,SupervisedJobLane::CoreExecutor,binding.clone(),false)
            .unwrap();
        let mut drift=binding.clone();
        drift.user_turn_ref="forged-user-turn".into();
        assert!(held.stop_exact(SupervisedJobLane::CoreExecutor,&drift).is_err());
        assert!(held.stop_exact(SupervisedJobLane::DlcClient,&binding).is_err());
        let proof=held.stop_exact(SupervisedJobLane::CoreExecutor,&binding).unwrap();
        assert_eq!(proof.binding(),&binding);
        assert_eq!(proof.lane(),SupervisedJobLane::CoreExecutor);
        assert_ne!(proof.original_pid(),0);
        assert_ne!(proof.original_creation_filetime(),0);
        assert_ne!(proof.supervisor_pid(),0);
        assert_ne!(proof.supervisor_creation_filetime(),0);
        assert_eq!(proof.method(),"WINDOWS_JOB_ACTIVE_ZERO_V1");
        assert!(held.stop_exact(SupervisedJobLane::CoreExecutor,&binding).is_err());
    }
    #[test]
    fn service_local_registry_refuses_foreign_replay_and_lane_confusion(){
        let keeper=ScmJobKeeper{registry:Mutex::new(ServiceRegistry::default())};
        let approved=binding();
        let (job,child)=suspended_fixture_child();
        keeper.admit_inner(
            job,child,SupervisedJobLane::DlcClient,approved.clone(),false
        ).unwrap();
        let mut foreign=approved.clone();
        foreign.core_epoch_ref="old-epoch".into();
        assert!(keeper.stop_exact_inner(
            &foreign,SupervisedJobLane::DlcClient).is_err());
        assert!(keeper.stop_exact_inner(
            &approved,SupervisedJobLane::CoreExecutor).is_err());
        let (job2,child2)=suspended_fixture_child();
        assert!(keeper.admit_inner(
            job2,child2,SupervisedJobLane::DlcClient,approved.clone(),false
        ).is_err());
        let stopped=keeper.stop_exact_inner(
            &approved,SupervisedJobLane::DlcClient).unwrap();
        assert_eq!(stopped.binding(),&approved);
        assert_eq!(stopped.lane(),SupervisedJobLane::DlcClient);
        assert!(keeper.stop_exact_inner(
            &approved,SupervisedJobLane::DlcClient).is_err());
        let (job3,child3)=suspended_fixture_child();
        assert!(keeper.admit_inner(
            job3,child3,SupervisedJobLane::DlcClient,approved.clone(),false
        ).is_err());
        let (job4,child4)=suspended_fixture_child();
        assert!(keeper.admit_inner(
            job4,child4,SupervisedJobLane::CoreExecutor,foreign,false
        ).is_err());
    }
    #[test]
    fn host_token_observation_never_equates_non_system_with_appcontainer(){
        let peer=observe_host_process(std::process::id()).unwrap();
        let observed=process_is_appcontainer(peer.pid).unwrap();
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
