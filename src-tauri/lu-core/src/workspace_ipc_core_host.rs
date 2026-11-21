//! LU-IPC-01D-R4-R2: Lu CORE-side admitted DLC invocation boundary.
//!
//! This is NOT an IDE client, installer, credential issuer, or a second source
//! of user-task authority. A raw Protobuf OriginRefs/session_ref/catalog is
//! never a grant. Only a native, OS/package-attested transport can enter the
//! production Windows path. The host supplies the immutable original USER
//! turn origin, workspace scope, current policy and exact typed-input decoder.
//! All effects still run through ConversationAuthority + durable dispatch.
//!
//! Linux has the source-only inner gate for later separately authenticated
//! Unix-domain peer admission; there is no generic production entry that can
//! claim Linux package custody from a self-reported PID or UID.
use crate::{
    contracts::{ToolAuthorizer, ToolDescriptor, ToolInvocation, ProtoPayload},
    conversation_authority::{BoundCallOrigin, ConversationAuthority},
    journal::{EffectJournal,ExclusiveIpcEffectJournal},

    registry::{NativeToolRegistry, ToolResponse},
    workspace_ipc::{proto, MAX_TYPED_BYTES},
    workspace_ipc_transport::{TransportError, TransportSession},
};
#[cfg(windows)]
use crate::journal::IpcKernelStopWitness;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    sync::{Arc, atomic::{AtomicBool, AtomicUsize, Ordering}, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
const MAX_PARALLEL:u32=4;
const MAX_DEADLINE_MS:u64=300_000;
const SOURCE_DOMAIN:&[u8]=b"LU_CORE_DLC_EFFECT_CALL_V1";

#[derive(Debug,Clone,PartialEq,Eq)]
pub enum CoreDlcGateError {
    SessionMismatch,
    SessionClosed,
    CapacityExhausted,
    InvalidInvoke,
    InvalidOrigin,
    InvalidDeadline,
    UnresolvedObjectRef,
    UnregisteredTool,
    InvalidInput,
    Denied,
    Unavailable,
    Transport,
}
#[cfg(windows)]
fn installed_ipc_effect_allowed(effect:crate::contracts::ToolEffect)->bool{
    matches!(effect,crate::contracts::ToolEffect::ReadWorkspace)
}
/// The trusted *host* must decode the exact registered Protobuf request and
/// validate its semantics, argument scope, ownership and resource references.
/// Returning true without decoding is a host integration error, not authority.
/// No default verifier is available; bytes from a DLC are always untrusted.
pub trait TrustedCoreInputVerifier: Send + Sync {
    fn verify_exact(&self, descriptor:&ToolDescriptor, encoded:&[u8])->bool;
}
/// Lu Core cannot invent proof of OS custody. The production implementation
/// below owns a genuine suspended, Job-assigned Windows child. This trait is
/// deliberately private: an arbitrary plugin cannot attest itself by
/// supplying a no-op implementation.
trait CoreDlcWorkerCustody: Send + Sync {
    fn terminate_and_reap(&self)->Result<(),CoreDlcGateError>;
}

#[cfg(windows)]
struct WindowsOwnedChild {
    job:lu_win_worker_job::Job,
    child:std::process::Child,
    reaped:bool,
}

/// Real Windows Job ownership, not an imported PID, package claim or handle.
/// The caller must create the Child in CREATE_SUSPENDED and transfer ownership
/// immediately. The constructor assigns the original private Job BEFORE the
/// primary thread is resumed. A failed assignment kills and reaps the child.
#[cfg(windows)]
pub struct WindowsDlcJobCustody {
    pid:u32,
    worker:Mutex<WindowsOwnedChild>,
}
#[cfg(windows)]
impl WindowsDlcJobCustody {
    pub fn take_suspended(
        job:lu_win_worker_job::Job,mut child:std::process::Child,
    )->Result<Self,CoreDlcGateError>{
        let pid=child.id();
        if pid==0 || job.assign_and_resume_suspended(&child).is_err(){
            let _=child.kill();
            let _=child.wait();
            return Err(CoreDlcGateError::Unavailable);
        }
        Ok(Self{pid,worker:Mutex::new(WindowsOwnedChild{
            job,child,reaped:false,
        })})
    }
    pub fn pid(&self)->u32{self.pid}

    /// Produce the native crate's unforgeable kernel-empty observation from
    /// THIS original private Job handle, then reap the exact supervised child.
    /// This proof is still process-local, not a post-crash SCM service receipt.
    fn kernel_empty_after_reap(
        &self,
    )->Result<lu_win_worker_job::WindowsJobEmptyProof,CoreDlcGateError>{
        let mut owned=self.worker.lock().map_err(|_|CoreDlcGateError::Unavailable)?;
        let proof=owned.job.terminate_and_verify_empty()
            .map_err(|_|CoreDlcGateError::Unavailable)?;
        if !owned.reaped{
            owned.child.wait().map_err(|_|CoreDlcGateError::Unavailable)?;
            owned.reaped=true;
        }
        Ok(proof)
    }
}
#[cfg(windows)]
impl CoreDlcWorkerCustody for WindowsDlcJobCustody {
    fn terminate_and_reap(&self)->Result<(),CoreDlcGateError>{
        self.kernel_empty_after_reap().map(|_|())
    }
}
#[cfg(windows)]
impl Drop for WindowsDlcJobCustody {
    fn drop(&mut self){let _=self.terminate_and_reap();}
}

pub struct HostCoreTask<'a>{
    /// Created by the trusted original-user-turn supervisor, NEVER by copying
    /// incoming OriginRefs. Existing ConversationAuthority rechecks this.
    pub origin:&'a BoundCallOrigin,
    pub workspace_scope:&'a str,
    pub policy:&'a dyn ToolAuthorizer,
    pub input_verifier:&'a dyn TrustedCoreInputVerifier,
}
struct GateState{
    closed:AtomicBool,
    in_flight:AtomicUsize,
    bound_task:Mutex<Option<String>>,
    // Exact durable call IDs and canonical fingerprints admitted on THIS
    // original authenticated session. Never obtained from a receipt file.
    calls:Mutex<BTreeMap<String,String>>,
    worker:Option<Arc<dyn CoreDlcWorkerCustody>>,
}
impl Drop for GateState {
    fn drop(&mut self){
        self.closed.store(true,Ordering::Release);
        if let Some(worker)=&self.worker {
            let _=worker.terminate_and_reap();
        }
    }
}
/// An authenticated native transport's host-owned lifecycle and bounded
/// dispatch credits. It holds no HMAC key and cannot sign an IDE package.
pub struct CoreDlcHostGate {
    session:TransportSession,
    state:Arc<GateState>,
    #[cfg(windows)]
    native_worker:Option<Arc<WindowsDlcJobCustody>>,
}
struct CallSlot {
    state:Arc<GateState>,
}
impl Drop for CallSlot {
    fn drop(&mut self){
        self.state.in_flight.fetch_sub(1,Ordering::AcqRel);
    }
}
impl CoreDlcHostGate {
    fn from_trusted_session(
        session:TransportSession,worker:Option<Arc<dyn CoreDlcWorkerCustody>>,
    )->Self {
        Self {session,state:Arc::new(GateState{
            closed:AtomicBool::new(false),
            in_flight:AtomicUsize::new(0),
            bound_task:Mutex::new(None),
            calls:Mutex::new(BTreeMap::new()),
            worker,
        }),
        #[cfg(windows)]
        native_worker:None,
        }
    }
    /// Only a genuine installed signed Windows named-pipe session may create
    /// a PRODUCTION ingress. The debug signed-pwsh surrogate has no installer
    /// revocation witness and MUST NOT qualify for effects.
    #[cfg(windows)]
    pub fn from_installed_windows(
        installed:&crate::workspace_ipc_transport::windows::SignedWindowsSession,
        worker:Arc<WindowsDlcJobCustody>,
    )->Result<Self,CoreDlcGateError>{
        if !installed.installed_enrollment_active() ||
            installed.check_alive().is_err() ||
            installed.peer_pid()!=worker.pid() ||
            installed.peer_creation_filetime()==0 {
            return Err(CoreDlcGateError::Transport);
        }
        let mut gate=Self::from_trusted_session(
            installed.session().clone(),
            Some(Arc::clone(&worker) as Arc<dyn CoreDlcWorkerCustody>),
        );
        gate.native_worker=Some(worker);
        Ok(gate)
    }
    /// Only an installed SCM-brokered NVP session may enter without a
    /// Core-owned DLC Child Job: NVP owns its independently supervised
    /// signed worker-stop issuer. The GPU release path still independently
    /// verifies that issuer; authentication is never worker-empty evidence.
    #[cfg(windows)]
    pub fn from_scm_brokered_nvp(
        installed:&crate::workspace_ipc_transport::windows::SignedWindowsSession,
    )->Result<Self,CoreDlcGateError>{
        if !installed.brokered_nvp_active() ||
           !installed.gpu_lease_feature_negotiated() ||
           installed.session().module_id!="nvp.film" ||
           installed.peer_pid()==0 ||
           installed.peer_creation_filetime()==0 ||
           installed.check_alive().is_err(){
            return Err(CoreDlcGateError::Transport)
        }
        Ok(Self::from_trusted_session(installed.session().clone(),None))
    }
    #[cfg(test)]
    fn test_only(session:TransportSession)->Self{
        Self::from_trusted_session(session,None)
    }

    /// Refuse admission first, then request actual worker-tree termination and
    /// wait for a native empty/reaped observation. A custody error must be
    /// surfaced to the host and must NEVER be treated as successful recovery.
    pub fn cancel_and_reap(&self)->Result<(),CoreDlcGateError>{
        self.state.closed.store(true,Ordering::Release);
        if let Some(worker)=&self.state.worker {
            worker.terminate_and_reap()?;
        }
        Ok(())
    }
    pub fn close(&self)->Result<(),CoreDlcGateError>{
        self.cancel_and_reap()
    }

    /// Observe and record the original signed DLC *client* Job as stopped
    /// while its true private handle remains owned by this Core instance.
    /// This records only DLC peer custody, not independent custody of the
    /// Core-native tool executors or their separately spawned descendants.
    /// Consequently the IPC restart barrier remains CLOSED even afterward.
    /// A crashed Core cannot reconstruct this proof from a PID or SQLite row;
    /// a protected installed SCM supervisor is still required.
    #[cfg(windows)]
    pub fn record_verified_dlc_client_stop(
        &self,
        installed:&crate::workspace_ipc_transport::windows::SignedWindowsSession,
        journal:&ExclusiveIpcEffectJournal,
    )->Result<(),CoreDlcGateError>{
        if installed.session()!=&self.session{
            return Err(CoreDlcGateError::SessionMismatch);
        }
        if !installed.installed_enrollment_active(){
            return Err(CoreDlcGateError::Transport);
        }
        let worker=self.native_worker.as_ref()
            .ok_or(CoreDlcGateError::Unavailable)?;
        if worker.pid()!=installed.peer_pid(){
            return Err(CoreDlcGateError::SessionMismatch);
        }
        self.state.closed.store(true,Ordering::Release);
        let native_proof=worker.kernel_empty_after_reap()?;
        if self.active_calls()!=0{
            // A future may still be writing a final effect receipt. The
            // gate remains CLOSED; retry only after all slots have drained.
            return Err(CoreDlcGateError::Unavailable);
        }
        let pending=journal.pending_for_supervisor()
            .map_err(|_|CoreDlcGateError::Unavailable)?;
        let calls=self.state.calls.lock()
            .map_err(|_|CoreDlcGateError::Unavailable)?;
        let mut witnesses=Vec::with_capacity(pending.len());
        for original in &pending{
            if calls.get(&original.call_id)!=Some(&original.fingerprint){
                return Err(CoreDlcGateError::Denied);
            }
            witnesses.push(IpcKernelStopWitness::from_windows_job(
                original,&self.session,worker.pid(),
                installed.peer_creation_filetime(),&native_proof));
        }
        journal.record_original_dlc_stop(&witnesses)
            .map_err(|_|CoreDlcGateError::Unavailable)
    }

    /// SYSTEM-broker-only retirement of the exact installed session's
    /// protected OS credential. Never accept a caller-supplied package digest:
    /// the immutable pin is read from the installed native session witness.
    /// Key deletion precedes worker reaping so later frames fail even when
    /// kernel custody cannot yet be independently proved. The gate closes
    /// regardless of either outcome; failure is never reported as success.
    #[cfg(windows)]
    pub fn revoke_installed_package(
        &self,
        installed:&crate::workspace_ipc_transport::windows::SignedWindowsSession,
    )->Result<(),CoreDlcGateError>{
        if installed.session()!=&self.session{
            return Err(CoreDlcGateError::SessionMismatch);
        }
        let package=installed.installed_package_sha256()
            .ok_or(CoreDlcGateError::Transport)?;
        self.state.closed.store(true,Ordering::Release);
        // Never revoke an NVP film package by mutating an IDE credential
        // target (or vice versa). Trust immutable installed transport module.
        let key_deleted=match self.session.module_id.as_str(){
            "lu.ide"=>lu_win_worker_job::ipc_credential_broker::
                revoke_installed_ide_key(package).is_ok(),
            "nvp.film"=>lu_win_worker_job::ipc_credential_broker::
                revoke_installed_nvp_key(package).is_ok(),
            _=>false,
        };
        // Still attempt reaping if the vault is unavailable, otherwise the
        // child may outlive session revocation. Retain the original Job handle.
        let worker_reaped=if let Some(worker)=&self.state.worker{
            worker.terminate_and_reap().is_ok()
        }else{false};
        if !key_deleted || !worker_reaped{
            return Err(CoreDlcGateError::Unavailable);
        }
        Ok(())
    }
    pub fn is_closed(&self)->bool{
        self.state.closed.load(Ordering::Acquire)
    }
    pub fn active_calls(&self)->usize{
        self.state.in_flight.load(Ordering::Acquire)
    }
    fn acquire(&self)->Result<CallSlot,CoreDlcGateError>{
        if self.is_closed(){return Err(CoreDlcGateError::SessionClosed);}
        let claimed=self.state.in_flight.fetch_update(
            Ordering::AcqRel,Ordering::Acquire,
            |n| if n<(MAX_PARALLEL as usize){Some(n+1)}else{None},
        );
        if claimed.is_err(){return Err(CoreDlcGateError::CapacityExhausted);}
        let slot=CallSlot{state:Arc::clone(&self.state)};
        if self.is_closed(){return Err(CoreDlcGateError::SessionClosed);}
        Ok(slot)
    }
    fn bind_host_task(&self,authority:&ConversationAuthority,task_id:&str)
        ->Result<(),CoreDlcGateError>{
        if task_id.is_empty(){return Err(CoreDlcGateError::Denied);}
        let mut guard=self.state.bound_task.lock()
            .map_err(|_|CoreDlcGateError::Unavailable)?;
        match guard.as_deref(){
            Some(current) if current==task_id=>return Ok(()),
            Some(_)=>return Err(CoreDlcGateError::Denied),
            None=>{}
        }
        // Revoking the original task closes this session's new admissions.
        // The original authority still checks permissions on every dispatch.
        let state=Arc::downgrade(&self.state);
        let pinned=task_id.to_owned();
        authority.on_task_revoked(Arc::new(move |revoked|{
            if revoked==pinned {
                if let Some(gate)=state.upgrade(){
                    gate.closed.store(true,Ordering::Release);
                    if let Some(worker)=&gate.worker{
                        // Revocation is synchronous; a failed kernel-empty
                        // observation keeps this session closed and requires
                        // explicit host recovery, never optimistic re-admit.
                        let _=worker.terminate_and_reap();
                    }
                }
            }
        })).map_err(|_|CoreDlcGateError::Unavailable)?;
        *guard=Some(task_id.to_owned());
        Ok(())
    }
    fn require_session(&self,session:&TransportSession)
        ->Result<(),CoreDlcGateError>{
        if session!=&self.session{return Err(CoreDlcGateError::SessionMismatch);}
        if self.is_closed(){return Err(CoreDlcGateError::SessionClosed);}
        Ok(())
    }
    fn call_id(&self,host:&HostCoreTask<'_>,op:&str)->String{
        let mut hash=Sha256::new();
        for part in [
            SOURCE_DOMAIN,
            self.session.module_id.as_bytes(),
            host.origin.conversation_id.as_bytes(),
            host.origin.user_turn_id.as_bytes(),
            host.origin.task_id.as_bytes(),
            host.workspace_scope.as_bytes(),
            op.as_bytes(),
        ]{
            hash.update((part.len() as u64).to_le_bytes());
            hash.update(part);
        }
        format!("{:x}",hash.finalize())
    }
    fn authenticated_invocation(
        &self,frame:&proto::WorkspaceFrame,
        host:&HostCoreTask<'_>,registry:&NativeToolRegistry,
    )->Result<(ToolInvocation,ToolDescriptor),CoreDlcGateError>{
        self.session.check_request(frame)
            .map_err(|_|CoreDlcGateError::SessionMismatch)?;
        let Some(proto::workspace_frame::Body::Invoke(request))=frame.body.as_ref()
            else{return Err(CoreDlcGateError::InvalidInvoke)};
        let Some(origin)=request.origin.as_ref()
            else{return Err(CoreDlcGateError::InvalidOrigin)};
        if origin.workspace_ref!=host.workspace_scope
            ||origin.conversation_ref!=host.origin.conversation_id
            ||origin.user_turn_ref!=host.origin.user_turn_id
            ||origin.task_ref!=host.origin.task_id
            ||origin.operation_ref.is_empty()
            ||host.workspace_scope.is_empty(){
            return Err(CoreDlcGateError::InvalidOrigin);
        }
        if request.capability_version!=1{
            return Err(CoreDlcGateError::UnregisteredTool);
        }
        // Object locators need a separate host-owned capability resolver;
        // never silently forward a DLC-supplied object_ref as a file path.
        if !request.input_refs.is_empty(){
            return Err(CoreDlcGateError::UnresolvedObjectRef);
        }
        let now=SystemTime::now().duration_since(UNIX_EPOCH)
            .map_err(|_|CoreDlcGateError::Unavailable)?
            .as_millis() as u64;
        if request.deadline_unix_ms<=now
            ||request.deadline_unix_ms>now.saturating_add(MAX_DEADLINE_MS){
            return Err(CoreDlcGateError::InvalidDeadline);
        }
        let encoded=request.input.as_ref().ok_or(CoreDlcGateError::InvalidInput)?;
        if encoded.encoded_message.len()>MAX_TYPED_BYTES{
            return Err(CoreDlcGateError::InvalidInput);
        }
        let tool=registry.tools().map_err(|_|CoreDlcGateError::Unavailable)?
            .into_iter().find(|descriptor|descriptor.id==request.capability_id)
            .ok_or(CoreDlcGateError::UnregisteredTool)?;
        if encoded.type_url!=tool.input_type_url ||
           !host.input_verifier.verify_exact(&tool,&encoded.encoded_message){
            return Err(CoreDlcGateError::InvalidInput);
        }
        Ok((ToolInvocation {
            call_id:self.call_id(host,&origin.operation_ref),
            tool_id:tool.id.clone(),
            workspace_id:Some(host.workspace_scope.to_owned()),
            input:ProtoPayload{
                type_url:encoded.type_url.clone(),
                bytes:encoded.encoded_message.clone(),
            },
        },tool))
    }
    /// Invoked ONLY with frames already read using the OS-authenticated
    /// transport's per-frame message-token observer. Cannot be called from
    /// arbitrary external application code or an unverified socket.
    async fn dispatch_verified_frame(
        &self,frame:&proto::WorkspaceFrame,
        host:&HostCoreTask<'_>,
        authority:&ConversationAuthority,
        registry:&NativeToolRegistry,
        journal:&EffectJournal,
    )->Result<ToolResponse,CoreDlcGateError>{
        let _slot=self.acquire()?;
        self.bind_host_task(authority,&host.origin.task_id)?;
        if self.is_closed(){return Err(CoreDlcGateError::SessionClosed);}
        let (invocation,descriptor)=self.authenticated_invocation(frame,host,registry)?;
        // Preflight with current task and host policies. The durable native
        // dispatcher independently rechecks these *before* journal prepare.
        authority.authorize_worker_call(
            host.origin,&invocation,&descriptor,host.policy
        ).map_err(|_|CoreDlcGateError::Denied)?;
        if self.is_closed(){return Err(CoreDlcGateError::SessionClosed);}
        {
            let digest=crate::journal::fingerprint(&invocation);
            let mut calls=self.state.calls.lock()
                .map_err(|_|CoreDlcGateError::Unavailable)?;
            if let Some(existing)=calls.get(&invocation.call_id){
                if existing!=&digest{return Err(CoreDlcGateError::Denied);}
            }else{
                calls.insert(invocation.call_id.clone(),digest);
            }
        }
        if self.is_closed(){return Err(CoreDlcGateError::SessionClosed);}
        // Closing a session must CANCEL its in-flight dispatch future.
        // The durable ExecutingEffectGuard then classifies a started effect
        // UNCERTAIN, preventing unsafe replay after reconnect/restart.
        let wait_cancel=async {
            loop {
                if self.is_closed(){break;}
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        };
        let result=tokio::select! {
            biased;
            _=wait_cancel=>return Err(CoreDlcGateError::SessionClosed),
            reply=authority.dispatch_bound_durable(
                registry,journal,invocation,host.origin.clone(),host.policy
            )=>reply,
        };
        if self.is_closed(){return Err(CoreDlcGateError::SessionClosed);}
        Ok(result)
    }
    /// Host production bridge: only the installed, pinned signed native
    /// Windows lease reads frames. The raw frame cannot be submitted here.
    #[cfg(windows)]
    pub async fn receive_authorized_windows(
        &self,
        installed:&crate::workspace_ipc_transport::windows::SignedWindowsSession,
        pipe:&mut tokio::net::windows::named_pipe::NamedPipeServer,
        host:&HostCoreTask<'_>,
        authority:&ConversationAuthority,
        registry:&NativeToolRegistry,
        journal:&ExclusiveIpcEffectJournal,
        supervisor:&crate::workspace_ipc_scm::VerifiedScmSupervisor,
    )->Result<ToolResponse,CoreDlcGateError>{
        // The OS SCM service is a separately signed and independently pinned
        // host requirement, NOT a client Hello claim or executor stop receipt.
        supervisor.require_live().map_err(|_|CoreDlcGateError::Unavailable)?;
        // Acquire the SQLite OS lock and check startup-recovery disposition
        // before any authenticated DLC effect is allowed through.
        let live=journal.ready_journal()
            .map_err(|_|CoreDlcGateError::Unavailable)?;
        self.require_session(installed.session())?;
        if !installed.installed_enrollment_active(){
            return Err(CoreDlcGateError::Transport);
        }
        let worker=self.native_worker.as_ref()
            .ok_or(CoreDlcGateError::Unavailable)?;
        if installed.peer_pid()!=worker.pid() ||
            installed.peer_creation_filetime()==0 {
            return Err(CoreDlcGateError::SessionMismatch);
        }
        let frame=installed.read_verified_frame(pipe).await
            .map_err(|_|CoreDlcGateError::Transport)?;
        supervisor.require_live().map_err(|_|CoreDlcGateError::Unavailable)?;
        // The verified native process is persisted as the owner BEFORE
        // dispatch can transition PREPARED->EXECUTING. Auth is checked again
        // by dispatch, including the current original-user-turn grant.
        let (call,descriptor)=self.authenticated_invocation(&frame,host,registry)?;
        // This installed-session ingress does not yet independently own
        // native *executor* custody. Until the protected host executor is
        // integrated, admit only the reviewed read-only capability class.
        // Never down-classify NVP EXECUTE+WRITE or other effects to READ.
        if !installed_ipc_effect_allowed(descriptor.effect){
            return Err(CoreDlcGateError::Denied);
        }
        // Install synchronous revocation custody BEFORE persisting the
        // PREPARED effect intent; an already-revoked task stays denied.
        self.bind_host_task(authority,&host.origin.task_id)?;
        authority.authorize_worker_call(
            host.origin,&call,&descriptor,host.policy
        ).map_err(|_|CoreDlcGateError::Denied)?;
        if self.is_closed(){return Err(CoreDlcGateError::SessionClosed);}
        journal.bind_authenticated_call(
            &call,&self.session,worker.pid(),installed.peer_creation_filetime())
            .map_err(|_|CoreDlcGateError::Unavailable)?;
        let result=self.dispatch_verified_frame(
            &frame,host,authority,registry,live).await?;
        supervisor.require_live().map_err(|_|CoreDlcGateError::Unavailable)?;
        Ok(result)
    }
}

#[cfg(windows)]
pub use lu_win_worker_job::ipc_scm_control_wire::OriginalCall;

/// Explicit Core-host user-approved READ + EXECUTE pair. NVP's claimed
/// OriginRefs, package manifest and LLM text cannot construct these grants.
/// Device control itself is NOT elevated/physical-device ToolEffect: it is a
/// host resource scheduler operation fencing an already approved NVP render.
#[cfg(windows)]
pub struct ApprovedNvpGpuPlan<'a>{
    pub read:&'a ToolInvocation,
    pub read_descriptor:&'a ToolDescriptor,
    pub execute:&'a ToolInvocation,
    pub execute_descriptor:&'a ToolDescriptor,
}
/// Source-authenticated film registration is separate from IPC MAC and
/// human approval. A synthetic descriptor with a plausible nvp.film.*
/// prefix must never acquire the model/GPU through a forged host task.
#[cfg(windows)]
fn require_registered_nvp_film_plan(
    registry:&NativeToolRegistry,
    plan:&ApprovedNvpGpuPlan<'_>,
)->Result<(),CoreDlcGateError>{
    let modules=registry.modules()
        .map_err(|_|CoreDlcGateError::Unavailable)?;
    if !modules.iter().any(|m|
        m.id=="nvp.film" &&
        m.kind==crate::contracts::ModuleKind::Workspace){
        return Err(CoreDlcGateError::UnregisteredTool)
    }
    let tools=registry.tools().map_err(|_|CoreDlcGateError::Unavailable)?;
    for (invocation,descriptor,effect) in [
        (plan.read,plan.read_descriptor,crate::contracts::ToolEffect::ReadWorkspace),
        (plan.execute,plan.execute_descriptor,crate::contracts::ToolEffect::ExecuteCode),
    ]{
        if !descriptor.id.starts_with("nvp.film.") ||
           descriptor.effect!=effect ||
           invocation.tool_id!=descriptor.id ||
           invocation.workspace_id.is_none() ||
           invocation.input.type_url!=descriptor.input_type_url ||
           !tools.iter().any(|available|available==descriptor){
            return Err(CoreDlcGateError::UnregisteredTool)
        }
    }
    if plan.read.call_id==plan.execute.call_id ||
       plan.read.workspace_id!=plan.execute.workspace_id{
        return Err(CoreDlcGateError::Denied)
    }
    Ok(())
}

#[cfg(windows)]
struct OriginalNvpGpuAuthority<'a>{
    authority:&'a ConversationAuthority,
    original:&'a BoundCallOrigin,
    workspace_scope:&'a str,
    session:&'a TransportSession,
    policy:&'a dyn ToolAuthorizer,
    plan:&'a ApprovedNvpGpuPlan<'a>,
}
#[cfg(windows)]
impl OriginalNvpGpuAuthority<'_>{
    fn check_scoped(
        &self,request:&crate::gpu_resource_leases::GpuLeaseRequest,
    )->Result<(),crate::gpu_resource_leases::LeaseRefusal>{
        use crate::gpu_resource_leases::LeaseRefusal;
        let o=&request.original;
        if self.session.module_id!="nvp.film" ||
           o.module_id!=self.session.module_id ||
           o.session_ref!=self.session.session_ref ||
           o.core_epoch_ref!=self.session.core_epoch_ref ||
           o.workspace_ref!=self.workspace_scope ||
           o.conversation_ref!=self.original.conversation_id ||
           o.user_turn_ref!=self.original.user_turn_id ||
           o.task_ref!=self.original.task_id ||
           self.plan.read.workspace_id.as_deref()!=Some(self.workspace_scope) ||
           self.plan.execute.workspace_id.as_deref()!=Some(self.workspace_scope) ||
           self.plan.read_descriptor.effect!=crate::contracts::ToolEffect::ReadWorkspace ||
           self.plan.execute_descriptor.effect!=crate::contracts::ToolEffect::ExecuteCode ||
           !self.plan.read_descriptor.id.starts_with("nvp.film.") ||
           !self.plan.execute_descriptor.id.starts_with("nvp.film.") ||
           self.plan.read.tool_id!=self.plan.read_descriptor.id ||
           self.plan.execute.tool_id!=self.plan.execute_descriptor.id ||
           self.plan.read.call_id==self.plan.execute.call_id {
            return Err(LeaseRefusal::NotAuthorized)
        }
        Ok(())
    }
}
#[cfg(windows)]
impl crate::gpu_resource_leases::OriginalTurnAuthority for OriginalNvpGpuAuthority<'_>{
    fn admit(&self,request:&crate::gpu_resource_leases::GpuLeaseRequest)
        ->Result<(),crate::gpu_resource_leases::LeaseRefusal>{
        self.check_scoped(request)?;
        // These capabilities are from a trusted, verified human task; no
        // newly minted DeviceControl model privilege is introduced.
        self.authority.authorize_worker_call(
            self.original,self.plan.read,self.plan.read_descriptor,self.policy)
            .map_err(|_|crate::gpu_resource_leases::LeaseRefusal::NotAuthorized)?;
        self.authority.authorize_worker_call(
            self.original,self.plan.execute,self.plan.execute_descriptor,self.policy)
            .map_err(|_|crate::gpu_resource_leases::LeaseRefusal::NotAuthorized)
    }
    fn admit_cleanup(&self,request:&crate::gpu_resource_leases::GpuLeaseRequest)
        ->Result<(),crate::gpu_resource_leases::LeaseRefusal>{
        // Cleanup of an ALREADY recorded lease may proceed without creating
        // a new render grant. Only after live installed peer identity,
        // original exact scope and signed kernel Job stop verification.
        self.check_scoped(request)
    }
}
/// The synchronous verified-host frame scope is converted to a wholly
/// owned, immutable original approval before a blocking GGUF worker is
/// scheduled. No borrowed IPC client, pipe, UI or model text crosses into
/// the worker thread. Revocation is rechecked before effect and GRANTED.
#[cfg(windows)]
struct OwnedNvpGpuAuthority {
    gate:std::sync::Weak<GateState>,
    authority:ConversationAuthority,
    origin:BoundCallOrigin,
    workspace_scope:String,
    session:TransportSession,
    policy:Arc<dyn ToolAuthorizer>,
    registry:NativeToolRegistry,
    read:ToolInvocation,
    read_descriptor:ToolDescriptor,
    execute:ToolInvocation,
    execute_descriptor:ToolDescriptor,
    broker:Option<crate::workspace_ipc_transport::windows::BrokeredNvpGrantRecheck>,
}
#[cfg(windows)]
impl crate::gpu_resource_leases::OriginalTurnAuthority for OwnedNvpGpuAuthority {
    fn admit(&self,request:&crate::gpu_resource_leases::GpuLeaseRequest)
        ->Result<(),crate::gpu_resource_leases::LeaseRefusal>{
        if !self.gate.upgrade().is_some_and(|g|
            !g.closed.load(Ordering::Acquire)){
            return Err(crate::gpu_resource_leases::LeaseRefusal::NotAuthorized)
        }
        let plan=ApprovedNvpGpuPlan {
            read:&self.read,read_descriptor:&self.read_descriptor,
            execute:&self.execute,execute_descriptor:&self.execute_descriptor,
        };
        require_registered_nvp_film_plan(&self.registry,&plan)
            .map_err(|_|crate::gpu_resource_leases::LeaseRefusal::NotAuthorized)?;
        let scoped=OriginalNvpGpuAuthority{
            authority:&self.authority,original:&self.origin,
            workspace_scope:&self.workspace_scope,
            session:&self.session,policy:self.policy.as_ref(),plan:&plan,
        };
        crate::gpu_resource_leases::OriginalTurnAuthority::admit(&scoped,request)
    }
    fn fence_final_grant<R>(
        &self,request:&crate::gpu_resource_leases::GpuLeaseRequest,
        commit:impl FnOnce()->Result<R,crate::gpu_resource_leases::LeaseRefusal>,
    )->Result<R,crate::gpu_resource_leases::LeaseRefusal>{
        // Preflight may run host policy, but the final original-task
        // read guard is held ONLY during the short SQLite GRANTED commit.
        // No Vulkan, GGUF unload, worker wait or network I/O runs inside.
        self.admit(request)?;
        // The native GGUF unload may have taken minutes. Revalidate the
        // *same* SCM credential, original OS generation, protected pins and
        // session deadline before the short durable GRANTED commit. Never
        // run the broker inside ConversationAuthority's critical lock.
        if let Some(broker)=&self.broker{
            broker.check_before_grant()
                .map_err(|_|crate::gpu_resource_leases::LeaseRefusal::NotAuthorized)?;
        }
        let gate=self.gate.upgrade().ok_or(
            crate::gpu_resource_leases::LeaseRefusal::NotAuthorized)?;
        // Host policy may involve an external provider. It is checked
        // BEFORE both short native locks; never hold the registry lock
        // around a host policy call or an OS broker round trip.
        self.authority.with_authorized_worker_launch(
            &self.origin,
            &self.read,&self.read_descriptor,
            &self.execute,&self.execute_descriptor,
            self.policy.as_ref(),
            || {
                if gate.closed.load(Ordering::Acquire){
                    return Err(crate::gpu_resource_leases::LeaseRefusal::NotAuthorized)
                }
                // Both original-turn authority and *current* native NVP
                // registration now remain stable for the one bounded
                // SQLite GRANTED commit. Unregister must wait until it ends.
                self.registry.with_registered_workspace_pair(
                    "nvp.film",&self.read_descriptor,&self.execute_descriptor,
                    commit,
                ).map_err(|_|crate::gpu_resource_leases::LeaseRefusal::NotAuthorized)?
            }
        ).map_err(|_|crate::gpu_resource_leases::LeaseRefusal::NotAuthorized)?
    }
    fn admit_cleanup(&self,request:&crate::gpu_resource_leases::GpuLeaseRequest)
        ->Result<(),crate::gpu_resource_leases::LeaseRefusal>{
        if !self.gate.upgrade().is_some_and(|g|
            !g.closed.load(Ordering::Acquire)){
            return Err(crate::gpu_resource_leases::LeaseRefusal::NotAuthorized)
        }
        let plan=ApprovedNvpGpuPlan {
            read:&self.read,read_descriptor:&self.read_descriptor,
            execute:&self.execute,execute_descriptor:&self.execute_descriptor,
        };
        let scoped=OriginalNvpGpuAuthority{
            authority:&self.authority,original:&self.origin,
            workspace_scope:&self.workspace_scope,
            session:&self.session,policy:self.policy.as_ref(),plan:&plan,
        };
        crate::gpu_resource_leases::OriginalTurnAuthority::admit_cleanup(&scoped,request)
    }
}
#[cfg(windows)]
fn gpu_async_outcome(
    operation_ref:&str,
    status:crate::gpu_async_reconciler::GpuAsyncStatus,
)->proto::GpuLeaseOutcome{
    use crate::gpu_async_reconciler::{GpuAsyncPhase as Phase,GpuAsyncKind as Kind};
    let (decision,phase,lease_ref,observed)=match status.phase{
        Phase::Queued|Phase::Running=>(
            proto::AdmissionDecision::Deferred,
            match status.kind{
                Kind::Acquire=>proto::GpuLeasePhase::Draining,
                Kind::Release=>proto::GpuLeasePhase::Releasing,
            },
            status.lease_ref,None),
        Phase::Granted=>(
            proto::AdmissionDecision::Allowed,
            proto::GpuLeasePhase::Granted,status.lease_ref,
            status.observed_available_vram_bytes),
        Phase::Restored=>(
            proto::AdmissionDecision::Allowed,
            proto::GpuLeasePhase::Restored,status.lease_ref,None),
        Phase::Denied=>(
            proto::AdmissionDecision::Denied,
            proto::GpuLeasePhase::Quarantined,String::new(),None),
        Phase::Uncertain=>(
            proto::AdmissionDecision::Unknown,
            proto::GpuLeasePhase::Quarantined,String::new(),None),
    };
    proto::GpuLeaseOutcome{
        operation_ref:operation_ref.to_owned(),
        decision:decision as i32,phase:phase as i32,lease_ref,
        measured_available_vram_bytes:observed,
        reason_code:status.reason.into(),
    }
}
#[cfg(windows)]
impl CoreDlcHostGate{
    /// Verified installed NVP GPU request on the EXISTING Core Protobuf
    /// named pipe. Every potentially 180-second llama.cpp unload/load or
    /// WDDM probe is moved to a bounded blocking worker after durable QUEUED
    /// admission; this method replies immediately with DEFERRED and a
    /// correlation ref, never an unverified GPU grant. NVP polls the exact
    /// original operation and may start LTX only after ALLOWED+GRANTED.
    ///
    /// The caller must pass the SAME Arc policy behind HostCoreTask.policy.
    /// No policy, native worker/signer witness or original user task may be
    /// constructed from a Protobuf frame by the host listener.
    pub async fn receive_nvp_gpu_windows<B>(
        &self,
        installed:&crate::workspace_ipc_transport::windows::SignedWindowsSession,
        pipe:&mut tokio::net::windows::named_pipe::NamedPipeServer,
        host:&HostCoreTask<'_>,
        authority:&ConversationAuthority,
        registry:&NativeToolRegistry,
        plan:&ApprovedNvpGpuPlan<'_>,
        policy_owned:Arc<dyn ToolAuthorizer>,
        queue:&Arc<crate::gpu_async_reconciler::DurableGpuAsyncQueue>,
        manager:&Arc<crate::gpu_resource_leases::CoreGpuLeaseManager>,
        model:&Arc<crate::inference_supervisor::TrustedInferenceSupervisor<B>>,
        supervisor:&crate::workspace_ipc_scm::VerifiedScmSupervisor,
    )->Result<proto::WorkspaceFrame,CoreDlcGateError>
    where B:crate::inference_host::InferenceBackend+Send+'static {
        use crate::gpu_async_reconciler::{
            GpuAsyncKind,CoreInstalledGpuWork,
        };
        use crate::gpu_resource_leases::{
            GpuLeaseRequest,LeaseOrigin,OriginalTurnAuthority,
            HostModelControl,InstalledHostGpuObserver,
            InstalledScmNvpWorkerStopVerifier,
        };
        self.require_session(installed.session())?;
        require_registered_nvp_film_plan(registry,plan)?;
        if !installed.installed_enrollment_active() ||
           !installed.gpu_lease_feature_negotiated() ||
           self.session.module_id!="nvp.film" ||
           !std::ptr::eq(host.policy,policy_owned.as_ref()) {
            return Err(CoreDlcGateError::Denied)
        }
        supervisor.require_live().map_err(|_|CoreDlcGateError::Unavailable)?;
        installed.check_authenticated().await
            .map_err(|_|CoreDlcGateError::Transport)?;
        if installed.peer_creation_filetime()==0 ||
           self.native_worker.as_ref().map(|native|
               native.pid()!=installed.peer_pid()).unwrap_or(
                   !installed.brokered_nvp_active()
               ){
            return Err(CoreDlcGateError::SessionMismatch)
        }
        let _slot=self.acquire()?;
        self.bind_host_task(authority,&host.origin.task_id)?;
        // Startup must separately call both open_and_fence_at_startup
        // constructors before exposing the real model to UI/IPC ingress.
        // Here only reassert an existing unresolved state, never auto-clear.
        manager.reassert_recovery_fence(model.as_ref())
            .map_err(|_|CoreDlcGateError::Unavailable)?;
        // Only startup recovery asserts an unfinished-operation fence.
        // A fresh QUEUED request does NOT hold the model before the native
        // manager checks for an already active human generation; otherwise a
        // simple DEFERRED/busy request could permanently wedge inference.
        let frame=installed.read_verified_frame(pipe).await
            .map_err(|_|CoreDlcGateError::Transport)?;
        if self.is_closed(){return Err(CoreDlcGateError::SessionClosed)}
        let origin=match frame.body.as_ref(){
            Some(proto::workspace_frame::Body::GpuLeaseRequest(v))=>v.origin.as_ref(),
            Some(proto::workspace_frame::Body::GpuLeaseRelease(v))=>v.origin.as_ref(),
            Some(proto::workspace_frame::Body::GpuLeasePoll(v))=>v.origin.as_ref(),
            _=>return Err(CoreDlcGateError::InvalidInvoke),
        }.ok_or(CoreDlcGateError::InvalidOrigin)?;
        if origin.workspace_ref!=host.workspace_scope ||
           origin.conversation_ref!=host.origin.conversation_id ||
           origin.user_turn_ref!=host.origin.user_turn_id ||
           origin.task_ref!=host.origin.task_id ||
           origin.operation_ref.is_empty(){
            return Err(CoreDlcGateError::InvalidOrigin)
        }
        let expected_lease_ref=format!("gpu-{}",self.call_id(host,&origin.operation_ref));
        // Only the already verified Core-signed SCM service is allowed
        // to open SYSTEM/Administrators-only NVP stop receipts. Keep its
        // actual OS process-birth witness in the blocking GPU worker;
        // the desktop never accesses that protected receipt file.
        let stop_verifier=Arc::new(
            InstalledScmNvpWorkerStopVerifier::new(
                supervisor.original_process_witness()
                    .map_err(|_|CoreDlcGateError::Unavailable)?
            ).map_err(|_|CoreDlcGateError::Unavailable)?
        );
        let owner=Arc::new(OwnedNvpGpuAuthority{
            gate:Arc::downgrade(&self.state),
            authority:authority.clone(),origin:host.origin.clone(),
            workspace_scope:host.workspace_scope.into(),
            session:self.session.clone(),policy:policy_owned,
            registry:registry.clone(),
            read:plan.read.clone(),read_descriptor:plan.read_descriptor.clone(),
            execute:plan.execute.clone(),execute_descriptor:plan.execute_descriptor.clone(),
            broker:installed.original_grant_recheck(),
        });
        let result=match frame.body.as_ref().unwrap(){
            proto::workspace_frame::Body::GpuLeaseRequest(v)=>{
                // The enrolled host (not the DLC text) chooses the DXGI
                // physical GPU LUID and independently verifies its capacity.
                lu_win_worker_job::ipc_gpu_vram::checked_device(&v.device_ref)
                    .map_err(|_|CoreDlcGateError::Denied)?;
                let original=GpuLeaseRequest{
                    lease_ref:expected_lease_ref,
                    device_ref:v.device_ref.clone(),
                    required_free_vram_bytes:v.required_free_vram_bytes,
                    max_duration_ms:v.max_duration_ms,
                    original:LeaseOrigin{
                        module_id:self.session.module_id.clone(),
                        workspace_ref:host.workspace_scope.into(),
                        conversation_ref:host.origin.conversation_id.clone(),
                        user_turn_ref:host.origin.user_turn_id.clone(),
                        task_ref:host.origin.task_id.clone(),
                        core_epoch_ref:self.session.core_epoch_ref.clone(),
                        session_ref:self.session.session_ref.clone(),
                    }
                };
                let worker=Arc::new(CoreInstalledGpuWork{
                    manager:Arc::clone(manager),model:Arc::clone(model),
                    authority:Arc::clone(&owner),
                    probe:Arc::new(InstalledHostGpuObserver),
                    workers:Arc::clone(&stop_verifier),
                });
                queue.submit(original,GpuAsyncKind::Acquire,worker)
                    .map(|s|gpu_async_outcome(&origin.operation_ref,s))
                    .map_err(|_|CoreDlcGateError::Denied)?
            },
            proto::workspace_frame::Body::GpuLeaseRelease(v)=>{
                if v.lease_ref!=expected_lease_ref{
                    return Err(CoreDlcGateError::Denied)
                }
                let original=manager.original_request(&v.lease_ref)
                    .map_err(|_|CoreDlcGateError::Denied)?;
                let worker=Arc::new(CoreInstalledGpuWork{
                    manager:Arc::clone(manager),model:Arc::clone(model),
                    authority:Arc::clone(&owner),
                    probe:Arc::new(InstalledHostGpuObserver),
                    workers:Arc::clone(&stop_verifier),
                });
                queue.submit(original,GpuAsyncKind::Release,worker)
                    .map(|s|gpu_async_outcome(&origin.operation_ref,s))
                    .map_err(|_|CoreDlcGateError::Denied)?
            },
            proto::workspace_frame::Body::GpuLeasePoll(v)=>{
                if v.lease_ref!=expected_lease_ref{
                    return Err(CoreDlcGateError::Denied)
                }
                // Literal wire numbers 1/2 are frozen in workspace.proto.
                // Validate the generated enum first, without assuming prost
                // strips the long enumeration prefix to a short variant.
                if proto::GpuLeaseTransitionKind::try_from(v.transition).is_err(){
                    return Err(CoreDlcGateError::Denied)
                }
                let kind=match v.transition{
                    1=>GpuAsyncKind::Acquire,
                    2=>GpuAsyncKind::Release,
                    _=>return Err(CoreDlcGateError::Denied),
                };
                let original=queue.original_for_poll(&v.lease_ref,kind)
                    .map_err(|_|CoreDlcGateError::Denied)?;
                owner.admit_cleanup(&original).map_err(|_|CoreDlcGateError::Denied)?;
                queue.status(&original,kind)
                    .map(|s|gpu_async_outcome(&origin.operation_ref,s))
                    .map_err(|_|CoreDlcGateError::Denied)?
            },
            _=>return Err(CoreDlcGateError::InvalidInvoke),
        };
        if self.is_closed(){return Err(CoreDlcGateError::SessionClosed)}
        supervisor.require_live().map_err(|_|CoreDlcGateError::Unavailable)?;
        installed.check_authenticated().await
            .map_err(|_|CoreDlcGateError::Transport)?;
        let response=proto::WorkspaceFrame{
            protocol_major:crate::workspace_ipc::WIRE_MAJOR,
            protocol_minor:crate::workspace_ipc::WIRE_MINOR,
            request_id:frame.request_id,
            session_ref:self.session.session_ref.clone(),
            sequence:0,
            body:Some(proto::workspace_frame::Body::GpuLeaseOutcome(result)),
        };
        crate::workspace_ipc::validate(&response)
            .map_err(|_|CoreDlcGateError::Unavailable)?;
        crate::workspace_ipc_transport::write_frame(pipe,&response).await
            .map_err(|_|CoreDlcGateError::Transport)?;
        Ok(response)
    }
}

/// Actual protected Core-side public named-pipe acceptance loop for ONE
/// independently approved original NVP film job, not a global auto-starting
/// desktop RPC endpoint. The native pipe itself establishes NVP PID, session
/// and process birth; no caller can select or impersonate an enrolled PID. Its caller must be the original Lu user-turn host;
/// the NVP client has NO way to create the HostCoreTask, approved tool pair,
/// SCM signer witness or source-pinned GPU manager. The recovered WALs are
/// already open/fenced by DesktopState before any caller may invoke this.
#[cfg(windows)]
pub async fn serve_host_approved_nvp_gpu_windows<B>(
    endpoint:&str,
    identity:&crate::workspace_ipc_transport::ServerIdentity,
    original:&lu_win_worker_job::ipc_scm_control_wire::OriginalCall,
    host:&HostCoreTask<'_>,
    authority:&ConversationAuthority,
    registry:&NativeToolRegistry,
    plan:&ApprovedNvpGpuPlan<'_>,
    policy_owned:Arc<dyn ToolAuthorizer>,
    queue:&Arc<crate::gpu_async_reconciler::DurableGpuAsyncQueue>,
    manager:&Arc<crate::gpu_resource_leases::CoreGpuLeaseManager>,
    model:&Arc<crate::inference_supervisor::TrustedInferenceSupervisor<B>>,
    supervisor:&crate::workspace_ipc_scm::VerifiedScmSupervisor,
)->Result<(),CoreDlcGateError>
where B:crate::inference_host::InferenceBackend+Send+'static{
    // The operation fingerprint and exact job identity come from the
    // original durable Lu tool invocation, not from NVP's OriginRefs.
    if original.module_id!="nvp.film" ||
       original.session_ref!=identity.session_ref ||
       original.core_epoch_ref!=identity.core_epoch_ref ||
       original.conversation_ref!=host.origin.conversation_id ||
       original.user_turn_ref!=host.origin.user_turn_id ||
       original.task_ref!=host.origin.task_id ||
       original.workspace_ref!=host.workspace_scope ||
       original.call_id!=plan.execute.call_id ||
       original.fingerprint_sha256!=crate::journal::fingerprint(plan.execute) ||
       !std::ptr::eq(host.policy,policy_owned.as_ref()) ||
       plan.execute.workspace_id.as_deref()!=Some(host.workspace_scope) ||
       plan.read.workspace_id.as_deref()!=Some(host.workspace_scope){
        return Err(CoreDlcGateError::InvalidOrigin)
    }
    // A host-created TaskAuthority alone cannot register a tool or load a
    // first-party NVP Workspace. Require both exact source descriptors
    // currently present in the native owner registry before reading SCM.
    require_registered_nvp_film_plan(registry,plan)?;
    authority.authorize_worker_call(
        host.origin,plan.read,plan.read_descriptor,host.policy)
        .map_err(|_|CoreDlcGateError::Denied)?;
    authority.authorize_worker_call(
        host.origin,plan.execute,plan.execute_descriptor,host.policy)
        .map_err(|_|CoreDlcGateError::Denied)?;
    let scm=supervisor.original_process_witness()
        .map_err(|_|CoreDlcGateError::Unavailable)?;
    // This is an actual local OS named-pipe instance with a restricted
    // owner/SYSTEM DACL and no remote clients. No TCP or private SCM DLC pipe.
    let mut pipe=crate::workspace_ipc_transport::windows::bind_one(endpoint)
        .map_err(|_|CoreDlcGateError::Transport)?;
    let installed=crate::workspace_ipc_transport::windows::
        accept_scm_brokered_nvp_film(
            &mut pipe,identity,original,&scm)
        .await.map_err(|_|CoreDlcGateError::Transport)?;
    supervisor.require_live().map_err(|_|CoreDlcGateError::Unavailable)?;
    let gate=CoreDlcHostGate::from_scm_brokered_nvp(&installed)?;
    // A single authenticated client may submit bounded original-scoped
    // acquire/poll/release frames. Disconnect, timeout, refusal or restart
    // closes authority; an unresolved GPU lease remains WAL-QUARANTINED.
    let mut terminal=false;
    for _ in 0..128 {
        let reply=gate.receive_nvp_gpu_windows(
            &installed,&mut pipe,host,authority,registry,plan,
            Arc::clone(&policy_owned),queue,manager,model,supervisor
        ).await;
        match reply{
            Ok(frame)=>{
                if matches!(frame.body,
                    Some(proto::workspace_frame::Body::GpuLeaseOutcome(ref v))
                        if v.decision==proto::AdmissionDecision::Allowed as i32 &&
                           v.phase==proto::GpuLeasePhase::Restored as i32){
                    terminal=true;
                    break;
                }
            }
            Err(err)=>{
                let _=gate.close();
                let _=installed.revoke_brokered_nvp().await;
                return Err(err)
            }
        }
    }
    gate.close()?;
    installed.revoke_brokered_nvp().await
        .map_err(|_|CoreDlcGateError::Unavailable)?;
    if !terminal{
        return Err(CoreDlcGateError::CapacityExhausted)
    }
    Ok(())
}

#[cfg(windows)]
fn gpu_failure(error:&crate::gpu_resource_leases::LeaseRefusal)->(i32,i32,&'static str){
    use crate::gpu_resource_leases::LeaseRefusal::*;
    let (decision,phase,reason)=match error{
        Deferred|ModelBusy=>(
            proto::AdmissionDecision::Deferred,proto::GpuLeasePhase::Draining,"INFERENCE_BUSY"),
        Occupied|Replayed|WrongOwner|InvalidScope|NotAuthorized=>(
            proto::AdmissionDecision::Denied,proto::GpuLeasePhase::Quarantined,"GPU_SCOPE_DENIED"),
        NoIndependentWorkerStop=>(
            proto::AdmissionDecision::Deferred,proto::GpuLeasePhase::Granted,"NVP_WORKERS_ACTIVE"),
        NoVerifiedCapacity=>(
            proto::AdmissionDecision::Unknown,proto::GpuLeasePhase::Quarantined,"GPU_CAPACITY_UNVERIFIED"),
        Quarantined=>(
            proto::AdmissionDecision::Unknown,proto::GpuLeasePhase::Quarantined,"RECOVERY_REQUIRED"),
        ModelFailed|Database=>(
            proto::AdmissionDecision::Unknown,proto::GpuLeasePhase::Quarantined,"GPU_LEASE_UNCERTAIN"),
    };
    (decision as i32,phase as i32,reason)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        contracts::{AuthorizationError, ModuleKind, ModuleManifest,
            NativeTool, ToolEffect, ToolFuture, ToolOutput, CORE_CONTRACT_VERSION},
        conversation_authority::{AuthorizedCapability, TaskAuthority,
            TrustedUserTurnVerifier},
        journal::EffectState,
        workspace_ipc::{WIRE_MAJOR, WIRE_MINOR},
    };
    use std::{fs, path::PathBuf, sync::atomic::AtomicUsize, time::Duration};

    const REQ:&str="type.googleapis.com/lu.v1.TestRequest";
    const OUT:&str="type.googleapis.com/lu.v1.TestReply";
    struct RealUser;
    impl TrustedUserTurnVerifier for RealUser {
        fn verify_user_turn(&self,conv:&str,turn:&str)->bool{
            conv=="human-conversation" && turn=="original-user-turn"
        }
    }
    struct HostPolicy;
    impl ToolAuthorizer for HostPolicy {
        fn authorize(&self,_:&ToolInvocation,_:&ToolDescriptor)
            ->Result<(),AuthorizationError>{Ok(())}
    }
    struct Deny;
    impl ToolAuthorizer for Deny {
        fn authorize(&self,_:&ToolInvocation,_:&ToolDescriptor)
            ->Result<(),AuthorizationError>{
            Err(AuthorizationError::new("trusted policy declined"))
        }
    }
    struct TypedVerifier;
    impl TrustedCoreInputVerifier for TypedVerifier{
        fn verify_exact(&self,descriptor:&ToolDescriptor,raw:&[u8])->bool{
            descriptor.id=="lu.core.read"&&descriptor.input_type_url==REQ&&raw==[8,1]
        }
    }
    struct TestRead {
        descriptor:ToolDescriptor,
        count:Arc<AtomicUsize>,
        delay:Duration,
    }
    impl NativeTool for TestRead {
        fn descriptor(&self)->&ToolDescriptor{&self.descriptor}
        fn invoke(&self,_:ToolInvocation)->ToolFuture<'_>{
            Box::pin(async move{
                self.count.fetch_add(1,Ordering::SeqCst);
                if self.delay>Duration::ZERO{
                    tokio::time::sleep(self.delay).await;
                }
                Ok(ToolOutput{payload:ProtoPayload{type_url:OUT.into(),bytes:vec![8,1]}})
            })
        }
    }
    fn setup(delay:Duration)->(NativeToolRegistry,Arc<AtomicUsize>){
        let registry=NativeToolRegistry::new();
        let count=Arc::new(AtomicUsize::new(0));
        let descriptor=ToolDescriptor{
            id:"lu.core.read".into(),display_name:"Core verified read".into(),
            effect:ToolEffect::ReadWorkspace,input_type_url:REQ.into(),
            output_type_url:OUT.into(),
        };
        registry.register(ModuleManifest{
            id:"lu.core".into(),display_name:"Lu Core fixture".into(),
            contract_version:CORE_CONTRACT_VERSION,kind:ModuleKind::Core,
            tools:vec![descriptor.clone()],panels:vec![],workflows:vec![],
        },vec![Arc::new(TestRead{descriptor,count:Arc::clone(&count),delay})]).unwrap();
        (registry,count)
    }
    fn origin()->BoundCallOrigin{
        BoundCallOrigin{conversation_id:"human-conversation".into(),
            user_turn_id:"original-user-turn".into(),task_id:"task-a".into()}
    }
    fn authority()->ConversationAuthority{
        let authority=ConversationAuthority::new();
        authority.open_user_task(TaskAuthority{
            conversation_id:"human-conversation".into(),
            user_turn_id:"original-user-turn".into(),task_id:"task-a".into(),
            execution_scope_id:"workspace-a".into(),
            capabilities:vec![AuthorizedCapability{
                tool_id:"lu.core.read".into(),effect:ToolEffect::ReadWorkspace,
            }],expires_at:None,
        },&RealUser).unwrap();
        authority
    }
    fn gateway()->CoreDlcHostGate{
        CoreDlcHostGate::test_only(TransportSession{
            session_ref:"native-host-session-a".into(),
            module_id:"thirdparty.workspace".into(),
            core_epoch_ref:"native-core-epoch-a".into(),
        })
    }
    fn frame(operation:&str)->proto::WorkspaceFrame{
        let now=SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;
        proto::WorkspaceFrame{
            protocol_major:WIRE_MAJOR,protocol_minor:WIRE_MINOR,
            request_id:"request-a".into(),
            session_ref:"native-host-session-a".into(),sequence:0,
            body:Some(proto::workspace_frame::Body::Invoke(proto::CapabilityInvoke{
                origin:Some(proto::OriginRefs{
                    workspace_ref:"workspace-a".into(),
                    conversation_ref:"human-conversation".into(),
                    user_turn_ref:"original-user-turn".into(),
                    task_ref:"task-a".into(),
                    operation_ref:operation.into(),
                }),
                capability_id:"lu.core.read".into(),
                capability_version:1,
                input:Some(proto::TypedValue{
                    type_url:REQ.into(),encoded_message:vec![8,1]
                }),
                input_refs:vec![],deadline_unix_ms:now+15_000,
            }))
        }
    }
    fn test_path()->PathBuf{
        use std::time::{SystemTime,UNIX_EPOCH};
        let id=SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path=std::env::temp_dir().join(format!(
            "lu-core-dlc-r4r2-{}-{id}",std::process::id()));
        fs::create_dir_all(&path).unwrap();
        path
    }
    // Windows SQLite WAL/SHM files can retain a sharing lease briefly
    // after the final connection closes (e.g. a filesystem scanner).
    // Cleanup is not the journal's acceptance criterion: the test above
    // already required persisted APPLIED replay across a true DB reopen.
    fn cleanup_fixture(root:&PathBuf){
        for _ in 0..6{
            if fs::remove_dir_all(root).is_ok() || !root.exists(){return;}
            std::thread::sleep(Duration::from_millis(25));
        }
        // Unprivileged test infrastructure must not turn a delayed Windows
        // file-handle release into a false journal-implementation failure.
    }
    #[tokio::test]
    async fn host_bound_invoke_uses_current_user_turn_and_durable_idempotency(){
        let root=test_path();
        let db=root.join("effects.sqlite");
        let journal=EffectJournal::open(&db).unwrap();
        let (registry,count)=setup(Duration::ZERO);
        let authority=authority();
        let gate=gateway();
        let trusted=origin();
        let host=HostCoreTask{
            origin:&trusted,workspace_scope:"workspace-a",
            policy:&HostPolicy,input_verifier:&TypedVerifier
        };
        let request=frame("operation-a");
        assert!(gate.dispatch_verified_frame(
            &request,&host,&authority,&registry,&journal).await.unwrap().is_ok());
        assert!(gate.dispatch_verified_frame(
            &request,&host,&authority,&registry,&journal).await.unwrap().is_ok());
        assert_eq!(count.load(Ordering::SeqCst),1,
            "no effect may be re-executed with a second frame or retry");
        let id=gate.call_id(&host,"operation-a");
        assert_eq!(journal.get(&id).unwrap().unwrap().state,EffectState::Applied);
        assert_eq!(gate.active_calls(),0);
        drop(journal);
        let journal=EffectJournal::open(&db).unwrap();
        assert!(gate.dispatch_verified_frame(
            &request,&host,&authority,&registry,&journal).await.unwrap().is_ok());
        assert_eq!(count.load(Ordering::SeqCst),1,
            "receipt must survive a durable journal reopen");
        drop(journal);
        cleanup_fixture(&root);
    }
    #[tokio::test]
    async fn claimed_origin_scope_payload_and_policy_cannot_mint_permission(){
        let root=test_path();
        let journal=EffectJournal::open(&root.join("effects.sqlite")).unwrap();
        let (registry,count)=setup(Duration::ZERO);
        let authority=authority();
        let gate=gateway();
        let trusted=origin();
        let host=HostCoreTask{
            origin:&trusted,workspace_scope:"workspace-a",
            policy:&HostPolicy,input_verifier:&TypedVerifier
        };
        let original=frame("op-denied");
        let mut bad=original.clone();
        if let Some(proto::workspace_frame::Body::Invoke(i))=bad.body.as_mut(){
            i.origin.as_mut().unwrap().user_turn_ref="generated-by-model".into();
        }
        assert_eq!(gate.dispatch_verified_frame(
            &bad,&host,&authority,&registry,&journal).await.err(),
            Some(CoreDlcGateError::InvalidOrigin));
        let mut bad=original.clone();
        if let Some(proto::workspace_frame::Body::Invoke(i))=bad.body.as_mut(){
            i.origin.as_mut().unwrap().workspace_ref="other-workspace".into();
        }
        assert_eq!(gate.dispatch_verified_frame(
            &bad,&host,&authority,&registry,&journal).await.err(),
            Some(CoreDlcGateError::InvalidOrigin));
        let mut bad=original.clone();
        if let Some(proto::workspace_frame::Body::Invoke(i))=bad.body.as_mut(){
            i.input.as_mut().unwrap().encoded_message=vec![8,2];
        }
        assert_eq!(gate.dispatch_verified_frame(
            &bad,&host,&authority,&registry,&journal).await.err(),
            Some(CoreDlcGateError::InvalidInput));
        let mut bad=original.clone();
        if let Some(proto::workspace_frame::Body::Invoke(i))=bad.body.as_mut(){
            i.input_refs.push(proto::ObjectRef{
                owner_module_id:"thirdparty.workspace".into(),
                object_ref:"fake-object".into(),sha256:vec![0x44;32],
                size_bytes:4,expiry_unix_ms:4_000_000_000_000,
            });
        }
        assert_eq!(gate.dispatch_verified_frame(
            &bad,&host,&authority,&registry,&journal).await.err(),
            Some(CoreDlcGateError::UnresolvedObjectRef));
        let mut bad=original.clone();
        bad.session_ref="another-session".into();
        assert_eq!(gate.dispatch_verified_frame(
            &bad,&host,&authority,&registry,&journal).await.err(),
            Some(CoreDlcGateError::SessionMismatch));
        let deny=HostCoreTask{
            origin:&trusted,workspace_scope:"workspace-a",
            policy:&Deny,input_verifier:&TypedVerifier,
        };
        assert_eq!(gate.dispatch_verified_frame(
            &original,&deny,&authority,&registry,&journal).await.err(),
            Some(CoreDlcGateError::Denied));
        assert_eq!(count.load(Ordering::SeqCst),0);
        drop(journal);
        cleanup_fixture(&root);
    }
    #[tokio::test]
    async fn revoke_original_turn_closes_admission_and_prevents_new_effect(){
        let root=test_path();
        let journal=EffectJournal::open(&root.join("effects.sqlite")).unwrap();
        let (registry,count)=setup(Duration::ZERO);
        let authority=authority();
        let gate=gateway();
        let trusted=origin();
        let host=HostCoreTask{
            origin:&trusted,workspace_scope:"workspace-a",
            policy:&HostPolicy,input_verifier:&TypedVerifier
        };
        assert!(gate.dispatch_verified_frame(
            &frame("before-revoke"),&host,&authority,&registry,&journal)
            .await.unwrap().is_ok());
        authority.revoke_task("task-a").unwrap();
        assert!(gate.is_closed());
        assert_eq!(gate.dispatch_verified_frame(
            &frame("after-revoke"),&host,&authority,&registry,&journal).await.err(),
            Some(CoreDlcGateError::SessionClosed));
        assert_eq!(count.load(Ordering::SeqCst),1);
        drop(journal);
        cleanup_fixture(&root);
    }
    #[test]
    fn bounded_credits_and_explicit_shutdown_prevent_unbounded_calls(){
        let gate=gateway();
        let slots=(0..MAX_PARALLEL).map(|_|gate.acquire().unwrap()).collect::<Vec<_>>();
        assert_eq!(gate.active_calls(),MAX_PARALLEL as usize);
        assert!(matches!(gate.acquire(),Err(CoreDlcGateError::CapacityExhausted)));
        gate.close().unwrap();
        assert!(matches!(gate.acquire(),Err(CoreDlcGateError::SessionClosed)));
        drop(slots);
        assert_eq!(gate.active_calls(),0);
    }
    #[tokio::test]
    async fn aborted_future_is_uncertain_and_recovery_refuses_reexecution(){
        let root=test_path();
        let db=root.join("effects.sqlite");
        let journal=EffectJournal::open(&db).unwrap();
        let (registry,count)=setup(Duration::from_secs(5));
        let authority=authority();
        let gate=gateway();
        let trusted=origin();
        let host=HostCoreTask{
            origin:&trusted,workspace_scope:"workspace-a",
            policy:&HostPolicy,input_verifier:&TypedVerifier
        };
        let request=frame("operation-cancelled");
        let call_id=gate.call_id(&host,"operation-cancelled");
        let timed=tokio::time::timeout(Duration::from_millis(80),
            gate.dispatch_verified_frame(&request,&host,&authority,&registry,&journal)).await;
        assert!(timed.is_err(),"the fixture must be interrupted before effects finish");
        assert_eq!(count.load(Ordering::SeqCst),1);
        assert_eq!(gate.active_calls(),0);
        assert_eq!(journal.get(&call_id).unwrap().unwrap().state,EffectState::Uncertain,
            "future cancellation must never leave effect EXECUTING or retryable");
        drop(journal);
        let reopened=EffectJournal::open(&db).unwrap();
        assert_eq!(reopened.reconcile_interrupted().unwrap(),0);
        let retry=gate.dispatch_verified_frame(
            &request,&host,&authority,&registry,&reopened).await.unwrap();
        assert!(retry.is_err(),"an uncertain effect must not be replayed");
        assert_eq!(count.load(Ordering::SeqCst),1);
        drop(reopened);
        cleanup_fixture(&root);
    }
    #[test]
    fn journal_id_is_stable_across_connection_epoch_but_bound_to_original_task(){
        let gate=gateway();
        let origin=origin();
        let host=HostCoreTask{origin:&origin,workspace_scope:"workspace-a",
            policy:&HostPolicy,input_verifier:&TypedVerifier};
        let same=CoreDlcHostGate::test_only(TransportSession{
            session_ref:"different-after-restart".into(),
            module_id:"thirdparty.workspace".into(),core_epoch_ref:"new-core-epoch".into(),
        });
        assert_eq!(gate.call_id(&host,"op-one"),same.call_id(&host,"op-one"));
        assert_ne!(gate.call_id(&host,"op-one"),gate.call_id(&host,"op-two"));
        let mut other=origin.clone();
        other.task_id="another-task".into();
        let another=HostCoreTask{origin:&other,workspace_scope:host.workspace_scope,
            policy:host.policy,input_verifier:host.input_verifier};
        assert_ne!(gate.call_id(&host,"op-one"),gate.call_id(&another,"op-one"));
    }

    /// Unit custody is synthetic only: the production constructor accepts a
    /// concrete Windows Job-backed custodian and compares its original PID.
    struct FixtureCustody(AtomicUsize);
    impl CoreDlcWorkerCustody for FixtureCustody {
        fn terminate_and_reap(&self)->Result<(),CoreDlcGateError>{
            self.0.fetch_add(1,Ordering::SeqCst);
            Ok(())
        }
    }
    #[tokio::test]
    async fn revocation_cancels_inflight_durable_effect_and_requests_reap(){
        let root=test_path();
        let db=root.join("effects.sqlite");
        let journal=EffectJournal::open(&db).unwrap();
        let (registry,count)=setup(Duration::from_secs(5));
        let authority=authority();
        let worker=Arc::new(FixtureCustody(AtomicUsize::new(0)));
        let gate=CoreDlcHostGate::from_trusted_session(
            TransportSession{session_ref:"native-host-session-a".into(),
                module_id:"thirdparty.workspace".into(),
                core_epoch_ref:"native-core-epoch-a".into()},
            Some(Arc::clone(&worker) as Arc<dyn CoreDlcWorkerCustody>),
        );
        let trusted=origin();
        let host=HostCoreTask{origin:&trusted,workspace_scope:"workspace-a",
            policy:&HostPolicy,input_verifier:&TypedVerifier};
        let request=frame("revoke-during-effect");
        let id=gate.call_id(&host,"revoke-during-effect");
        let revoke=async {
            tokio::time::sleep(Duration::from_millis(100)).await;
            authority.revoke_task("task-a").unwrap();
        };
        let (out,())=tokio::join!(
            gate.dispatch_verified_frame(&request,&host,&authority,&registry,&journal),
            revoke
        );
        assert_eq!(out.err(),Some(CoreDlcGateError::SessionClosed));
        assert_eq!(count.load(Ordering::SeqCst),1);
        assert!(worker.0.load(Ordering::SeqCst)>=1);
        assert_eq!(journal.get(&id).unwrap().unwrap().state,EffectState::Uncertain);
        drop(journal);
        let reopened=EffectJournal::open(&db).unwrap();
        assert_eq!(reopened.reconcile_interrupted().unwrap(),0);
        assert_eq!(reopened.get(&id).unwrap().unwrap().state,EffectState::Uncertain);
        drop(reopened);
        cleanup_fixture(&root);
    }
    #[test]
    fn cancelled_gate_is_not_reopened_by_completed_reaping(){
        let worker=Arc::new(FixtureCustody(AtomicUsize::new(0)));
        let gate=CoreDlcHostGate::from_trusted_session(
            TransportSession{session_ref:"native-host-session-a".into(),
                module_id:"thirdparty.workspace".into(),
                core_epoch_ref:"native-core-epoch-a".into()},
            Some(Arc::clone(&worker) as Arc<dyn CoreDlcWorkerCustody>),
        );
        gate.cancel_and_reap().unwrap();
        assert!(gate.is_closed());
        assert!(matches!(gate.acquire(),Err(CoreDlcGateError::SessionClosed)));
        assert!(worker.0.load(Ordering::SeqCst)>=1);
    }

    #[cfg(windows)]
    #[test]
    fn gpu_plan_must_use_live_registered_nvp_workspace_descriptors(){
        let registry=NativeToolRegistry::new();
        let read=ToolDescriptor{
            id:"nvp.film.production.read".into(),
            display_name:"Read approved film production".into(),
            effect:ToolEffect::ReadWorkspace,
            input_type_url:REQ.into(),output_type_url:OUT.into(),
        };
        let execute=ToolDescriptor{
            id:"nvp.film.production.render".into(),
            display_name:"Execute approved film production".into(),
            effect:ToolEffect::ExecuteCode,
            input_type_url:REQ.into(),output_type_url:OUT.into(),
        };
        let read_call=ToolInvocation{
            call_id:"nvp-read-1".into(),tool_id:read.id.clone(),
            workspace_id:Some("film-project".into()),
            input:ProtoPayload{type_url:REQ.into(),bytes:vec![8,1]},
        };
        let exec_call=ToolInvocation{
            call_id:"nvp-exec-1".into(),tool_id:execute.id.clone(),
            workspace_id:Some("film-project".into()),
            input:ProtoPayload{type_url:REQ.into(),bytes:vec![8,2]},
        };
        let approved=ApprovedNvpGpuPlan{
            read:&read_call,read_descriptor:&read,
            execute:&exec_call,execute_descriptor:&execute,
        };
        assert_eq!(require_registered_nvp_film_plan(&registry,&approved),
            Err(CoreDlcGateError::UnregisteredTool),
            "string-prefixed unregistered tools cannot stand in for film approval");
        let count=Arc::new(AtomicUsize::new(0));
        registry.register(ModuleManifest{
            id:"nvp.film".into(),display_name:"Native NVP film fixture".into(),
            contract_version:CORE_CONTRACT_VERSION,
            kind:ModuleKind::Workspace,
            tools:vec![read.clone(),execute.clone()],
            panels:vec![],workflows:vec![],
        },vec![
            Arc::new(TestRead{descriptor:read.clone(),
                count:Arc::clone(&count),delay:Duration::ZERO}),
            Arc::new(TestRead{descriptor:execute.clone(),
                count:Arc::clone(&count),delay:Duration::ZERO}),
        ]).unwrap();
        assert_eq!(require_registered_nvp_film_plan(&registry,&approved),Ok(()));
        let observed=registry.with_registered_workspace_pair(
            "nvp.film",&read,&execute,||42).unwrap();
        assert_eq!(observed,42);
        let mut tampered=execute.clone();
        tampered.input_type_url="type.googleapis.com/nvp.film.Forged".into();
        let forged=ApprovedNvpGpuPlan{
            read:&read_call,read_descriptor:&read,
            execute:&exec_call,execute_descriptor:&tampered,
        };
        assert_eq!(require_registered_nvp_film_plan(&registry,&forged),
            Err(CoreDlcGateError::UnregisteredTool));
        assert!(registry.with_registered_workspace_pair(
            "nvp.film",&read,&tampered,||42).is_err(),
            "a replaced descriptor cannot reach the durable GPU commit");
        registry.unregister_workspace("nvp.film").unwrap();
        assert_eq!(require_registered_nvp_film_plan(&registry,&approved),
            Err(CoreDlcGateError::UnregisteredTool),
            "unloaded NVP must not retain model/GPU admission");
        assert!(registry.with_registered_workspace_pair(
            "nvp.film",&read,&execute,||42).is_err(),
            "an unloaded workspace cannot reenter the final GPU grant fence");
    }
    #[cfg(windows)]
    #[test]
    fn genuine_original_windows_job_requires_kernel_empty_and_reaps_child(){
        use std::{process::{Command,Stdio},os::windows::process::CommandExt};
        let shell=std::env::var_os("ComSpec").unwrap_or(
            std::ffi::OsString::from(r"C:\Windows\System32\cmd.exe"));
        let child=Command::new(shell)
            .args(["/D","/C","ping -n 20 127.0.0.1 >NUL"])
            .creation_flags(0x0800_0000|0x0000_0004)
            .stdin(Stdio::null()).stdout(Stdio::null())
            .stderr(Stdio::null()).spawn().unwrap();
        let job=lu_win_worker_job::Job::new().unwrap();
        let worker=WindowsDlcJobCustody::take_suspended(job,child).unwrap();
        assert_ne!(worker.pid(),0);
        let proof=worker.kernel_empty_after_reap().unwrap();
        assert_eq!(proof.method(),"WINDOWS_JOB_ACTIVE_ZERO_V1");
        // The original private Job stays owned and can independently
        // observe empty again without guessing from a stale PID.
        assert_eq!(worker.kernel_empty_after_reap().unwrap().method(),
            "WINDOWS_JOB_ACTIVE_ZERO_V1");
    }

    #[cfg(windows)]
    #[test]
    fn installed_ipc_cannot_downgrade_write_execute_network_or_elevated_effects(){
        assert!(installed_ipc_effect_allowed(ToolEffect::ReadWorkspace));
        for effect in [ToolEffect::WriteWorkspace,ToolEffect::ExecuteCode,
            ToolEffect::NetworkRead,ToolEffect::NetworkWrite,
            ToolEffect::DeviceControl,ToolEffect::Elevated]{
            assert!(!installed_ipc_effect_allowed(effect));
        }
    }
}
