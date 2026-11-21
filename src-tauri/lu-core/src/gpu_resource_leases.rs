//! LU-IPC-GPU-01A: Core-owned exclusive GPU VRAM handoff for NVP.
//!
//! Device capacity and worker-stop evidence must come from independently
//! installed host providers. NVP's claimed capacity, text "all clear", lease
//! expiry or a raw Protobuf frame NEVER releases a GPU lease. This module does
//! not run diffusion or claim physical Windows driver acceptance.
//!
//! Lu model/GGUF authority remains with TrustedInferenceSupervisor. SQLite
//! persists *intent before unload*, original user turn and exact restored
//! model identity. Every interrupted lease is QUARANTINED at restart; it is
//! never auto-replayed, expired/released, or assumed to have freed memory.
use std::{path::Path,sync::Mutex};
use rusqlite::{params,Connection,OptionalExtension,TransactionBehavior};

const MAX_VRAM:u64=512*1024*1024*1024;
const MAX_DURATION:u64=8*60*60*1000;

#[derive(Debug,Clone,PartialEq,Eq)]
pub enum LeaseRefusal{
    InvalidScope,NotAuthorized,Occupied,Deferred,NoVerifiedCapacity,
    NoIndependentWorkerStop,WrongOwner,Replayed,Quarantined,
    ModelBusy,ModelFailed,Database,
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum LeasePhase{Draining,Granted,Releasing,Restoring,Restored,Quarantined}
impl LeasePhase{
    fn value(self)->&'static str{
        match self{
            Self::Draining=>"DRAINING",Self::Granted=>"GRANTED",
            Self::Releasing=>"RELEASING",Self::Restoring=>"RESTORING",
            Self::Restored=>"RESTORED",Self::Quarantined=>"QUARANTINED"
        }
    }
    fn parse(s:&str)->Result<Self,LeaseRefusal>{
        match s{
            "DRAINING"=>Ok(Self::Draining),"GRANTED"=>Ok(Self::Granted),
            "RELEASING"=>Ok(Self::Releasing),"RESTORING"=>Ok(Self::Restoring),
            "RESTORED"=>Ok(Self::Restored),"QUARANTINED"=>Ok(Self::Quarantined),
            _=>Err(LeaseRefusal::Database),
        }
    }
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct LeaseOrigin{
    pub module_id:String,
    pub workspace_ref:String,
    pub conversation_ref:String,
    pub user_turn_ref:String,
    pub task_ref:String,
    pub core_epoch_ref:String,
    pub session_ref:String,
}
impl LeaseOrigin{
    fn checked(&self)->Result<(),LeaseRefusal>{
        for x in [&self.module_id,&self.workspace_ref,&self.conversation_ref,
            &self.user_turn_ref,&self.task_ref,&self.core_epoch_ref,
            &self.session_ref]{
            if !safe_ref(x){return Err(LeaseRefusal::InvalidScope)}
        }
        if self.module_id!="nvp.film"{return Err(LeaseRefusal::NotAuthorized)}
        Ok(())
    }
}
fn safe_ref(s:&str)->bool{
    !s.is_empty()&&s.len()<=128&&s.bytes().all(|x|
        x.is_ascii_alphanumeric()||matches!(x,b'-'|b'_'|b'.'|b':'|b'/'))
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct GpuLeaseRequest{
    /// Host-minted correlation ID; never a raw client-chosen OS capability.
    pub lease_ref:String,
    pub original:LeaseOrigin,
    pub device_ref:String,
    pub required_free_vram_bytes:u64,
    pub max_duration_ms:u64,
}
impl GpuLeaseRequest{
    fn checked(&self)->Result<(),LeaseRefusal>{
        self.original.checked()?;
        if !safe_ref(&self.lease_ref)||!safe_ref(&self.device_ref)||
            self.required_free_vram_bytes==0||
            self.required_free_vram_bytes>MAX_VRAM||
            !(1..=MAX_DURATION).contains(&self.max_duration_ms){
            return Err(LeaseRefusal::InvalidScope)
        }
        Ok(())
    }
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct ModelAtHandoff{
    pub selected_model_ref:Option<String>,
    pub original_session_ref:Option<String>,
    pub active_generation:bool,
}
pub trait HostModelControl{
    /// Must be backed by the genuine sole Core inference supervisor.
    fn snapshot(&self)->Result<ModelAtHandoff,LeaseRefusal>;
    /// Atomically refuse subsequent NEW GGUF loads/generations.
    fn hold_new_inference(&self)->Result<(),LeaseRefusal>;
    fn unload_selected(&self,session:&str,lease_ref:&str)->Result<(),LeaseRefusal>;
    fn observed_unloaded(&self)->Result<bool,LeaseRefusal>;
    fn restore_selected(&self,model_ref:&str,session:&str,lease_ref:&str)
        ->Result<(),LeaseRefusal>;
    /// Genuine model runtime must be back in READY, not just catalogued.
    fn ready_after_restore(&self)->Result<bool,LeaseRefusal>;
    fn resume_new_inference(&self)->Result<(),LeaseRefusal>;
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct GpuSample{
    pub device_ref:String,
    /// Host driver's observed *available budget*, not self-claimed NVP usage.
    pub available_vram_bytes:u64,
    pub sample_sequence:u64,
    pub source_code:String,
}
pub trait HostGpuObserver{
    /// Must use a trusted OS/vendor driver observer; no NVP-provided values.
    fn observe(&self,device_ref:&str)->Result<GpuSample,LeaseRefusal>;
}
pub trait OriginalTurnAuthority{
    /// Must check Core's current immutable user turn, task and scoped NVP
    /// package/session as a verified OS peer, independently of Protobuf.
    fn admit(&self,request:&GpuLeaseRequest)->Result<(),LeaseRefusal>;
    /// Release is cleanup of a PREVIOUSLY approved effect, not a new GPU
    /// effect. The installed broker may permit cleanup after user revocation
    /// only against the exact original durable grant and stopped worker.
    fn admit_cleanup(&self,request:&GpuLeaseRequest)->Result<(),LeaseRefusal>{
        self.admit(request)
    }
}
/// Separately protected Core journal/supervisor reconstruction of the
/// ORIGINAL lease authority after crash; not a current NVP IPC claim.
pub trait OriginalRecoveryAuthority{
    fn admit_original_recovery(&self,stored:&GpuLeaseRequest)
        ->Result<(),LeaseRefusal>;
}
pub trait NvpWorkerStopVerifier{
    /// Must independently establish original NVP worker-group EMPTY under
    /// installed OS custody. An NVP all-clear message is only a trigger.
    fn witnessed_empty(&self,request:&GpuLeaseRequest)->Result<bool,LeaseRefusal>;
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct GrantedGpuLease{
    pub lease_ref:String,
    pub device_ref:String,
    pub available_vram_bytes:u64,
    pub core_epoch_ref:String,
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum ReleaseOutcome{Restored,AlreadyRestored}
#[derive(Debug,Clone,PartialEq,Eq)]
struct Recorded{
    request:GpuLeaseRequest,
    phase:LeasePhase,
    model_ref:Option<String>,
    model_session:Option<String>,
}
pub struct CoreGpuLeaseManager{
    connection:Mutex<Connection>,
    /// Serializes external OS model/worker transitions with durable rows.
    transition:Mutex<()>,
}
impl CoreGpuLeaseManager{
    /// Host startup must construct this BEFORE the first LLM/model
    /// admission. Reconstructs every unfinished row as QUARANTINED and
    /// synchronously asserts the inference fence before returning control.
    pub fn open_and_fence_at_startup(
        path:&Path,model:&impl HostModelControl,
    )->Result<Self,LeaseRefusal>{
        let manager=Self::open(path)?;
        manager.reassert_recovery_fence(model)?;
        Ok(manager)
    }
    pub fn open(path:&Path)->Result<Self,LeaseRefusal>{
        let connection=Connection::open(path).map_err(|_|LeaseRefusal::Database)?;
        connection.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=FULL;
             PRAGMA foreign_keys=ON;
             CREATE TABLE IF NOT EXISTS lu_gpu_nvp_leases(
               lease_ref TEXT PRIMARY KEY NOT NULL,
               device_ref TEXT NOT NULL,
               module_id TEXT NOT NULL,
               workspace_ref TEXT NOT NULL,
               conversation_ref TEXT NOT NULL,
               user_turn_ref TEXT NOT NULL,
               task_ref TEXT NOT NULL,
               core_epoch_ref TEXT NOT NULL,
               session_ref TEXT NOT NULL,
               required_bytes INTEGER NOT NULL CHECK(required_bytes>0),
               duration_ms INTEGER NOT NULL CHECK(duration_ms>0),
               phase TEXT NOT NULL CHECK(phase IN
                 ('DRAINING','GRANTED','RELEASING','RESTORING',
                  'RESTORED','QUARANTINED')),
               selected_model_ref TEXT,
               selected_session_ref TEXT,
               observed_free_bytes INTEGER,
               updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
             );
             -- Initially only one whole-host diffusion lease is permitted.
             -- Until multi-GPU model residency has real device attribution,
             -- different device strings must not allow model reload races.
             CREATE UNIQUE INDEX IF NOT EXISTS one_nvp_gpu_lease_global
               ON lu_gpu_nvp_leases((1)) WHERE phase!='RESTORED';
             UPDATE lu_gpu_nvp_leases
               SET phase='QUARANTINED', updated_at=CURRENT_TIMESTAMP
               WHERE phase IN ('DRAINING','GRANTED','RELEASING','RESTORING');"
        ).map_err(|_|LeaseRefusal::Database)?;
        Ok(Self{connection:Mutex::new(connection),transition:Mutex::new(())})
    }
    fn lock_conn(&self)->Result<std::sync::MutexGuard<'_,Connection>,LeaseRefusal>{
        self.connection.lock().map_err(|_|LeaseRefusal::Database)
    }
    fn mark(&self,lease_ref:&str,from:LeasePhase,to:LeasePhase)
        ->Result<(),LeaseRefusal>{
        let conn=self.lock_conn()?;
        let count=conn.execute(
            "UPDATE lu_gpu_nvp_leases SET phase=?1,updated_at=CURRENT_TIMESTAMP
             WHERE lease_ref=?2 AND phase=?3",
            params![to.value(),lease_ref,from.value()])
            .map_err(|_|LeaseRefusal::Database)?;
        if count!=1{return Err(LeaseRefusal::Quarantined)}
        Ok(())
    }
    fn quarantine(&self,lease_ref:&str){
        if let Ok(conn)=self.connection.lock(){
            let _=conn.execute(
                "UPDATE lu_gpu_nvp_leases SET phase='QUARANTINED',
                 updated_at=CURRENT_TIMESTAMP WHERE lease_ref=?1 AND phase!='RESTORED'",
                [lease_ref]);
        }
    }
    fn require_model_restored(
        model:&impl HostModelControl,selected:Option<&str>,session:Option<&str>,
    )->Result<(),LeaseRefusal>{
        let observed=model.snapshot()?;
        if observed.active_generation ||
           observed.selected_model_ref.as_deref()!=selected ||
           observed.original_session_ref.as_deref()!=session ||
           (selected.is_some() && !model.ready_after_restore()?) {
            return Err(LeaseRefusal::ModelFailed)
        }
        Ok(())
    }
    fn prove_capacity(
        probe:&impl HostGpuObserver,device:&str,required:u64,
    )->Result<GpuSample,LeaseRefusal>{
        let a=probe.observe(device)?;
        let b=probe.observe(device)?;
        let valid=|s:&GpuSample|{
            s.device_ref==device && s.available_vram_bytes>=required &&
            s.available_vram_bytes<=MAX_VRAM &&
            safe_ref(&s.source_code)
        };
        if !valid(&a)||!valid(&b)||
            a.source_code!=b.source_code||
            a.sample_sequence==0||b.sample_sequence<=a.sample_sequence{
            return Err(LeaseRefusal::NoVerifiedCapacity);
        }
        Ok(b)
    }
    /// A genuine Core request persists DRAINING *before* the model unload.
    /// Both independent OS capacity observations and supervisor unloaded
    /// state must agree; otherwise remain quarantined without a GPU grant.
    pub fn request_handoff(
        &self,request:&GpuLeaseRequest,authority:&impl OriginalTurnAuthority,
        model:&impl HostModelControl,probe:&impl HostGpuObserver,
    )->Result<GrantedGpuLease,LeaseRefusal>{
        request.checked()?;
        authority.admit(request)?;
        let _guard=self.transition.lock().map_err(|_|LeaseRefusal::Database)?;
        let model_state=model.snapshot()?;
        if model_state.active_generation{return Err(LeaseRefusal::Deferred);}
        if model_state.selected_model_ref.is_some()!=model_state.original_session_ref.is_some(){
            return Err(LeaseRefusal::ModelFailed);
        }
        model.hold_new_inference()?;
        // A pre-existing reservation could have entered between snapshot
        // and the pause. Recheck under the new inference admission fence.
        let fenced=model.snapshot()?;
        if fenced.active_generation || fenced!=model_state {
            if !self.has_unreconciled().unwrap_or(true){
                let _=model.resume_new_inference();
            }
            return Err(LeaseRefusal::Deferred);
        }
        // Hold admission across the insertion and unload; on any ambiguity,
        // retain the hold and quarantine rather than racing a new generation.
        let created=(||->Result<(),LeaseRefusal>{
            let mut conn=self.lock_conn()?;
            let tx=conn.transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(|_|LeaseRefusal::Database)?;
            let consumed:Option<String>=tx.query_row(
                "SELECT phase FROM lu_gpu_nvp_leases WHERE lease_ref=?1",
                [&request.lease_ref],|r|r.get(0))
                .optional().map_err(|_|LeaseRefusal::Database)?;
            if consumed.is_some(){return Err(LeaseRefusal::Replayed)}
            let conflict:Option<String>=tx.query_row(
                "SELECT lease_ref FROM lu_gpu_nvp_leases
                 WHERE phase!='RESTORED' LIMIT 1",
                [],|r|r.get(0))
                .optional().map_err(|_|LeaseRefusal::Database)?;
            if conflict.is_some(){return Err(LeaseRefusal::Occupied)}
            let required=i64::try_from(request.required_free_vram_bytes)
                .map_err(|_|LeaseRefusal::InvalidScope)?;
            tx.execute(
                "INSERT INTO lu_gpu_nvp_leases
                 (lease_ref,device_ref,module_id,workspace_ref,conversation_ref,
                  user_turn_ref,task_ref,core_epoch_ref,session_ref,required_bytes,
                  duration_ms,phase,selected_model_ref,selected_session_ref)
                 VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,'DRAINING',?12,?13)",
                params![request.lease_ref,request.device_ref,request.original.module_id,
                    request.original.workspace_ref,request.original.conversation_ref,
                    request.original.user_turn_ref,request.original.task_ref,
                    request.original.core_epoch_ref,request.original.session_ref,
                    required,i64::try_from(request.max_duration_ms).map_err(|_|LeaseRefusal::InvalidScope)?,
                    model_state.selected_model_ref,model_state.original_session_ref]
            ).map_err(|_|LeaseRefusal::Database)?;
            tx.commit().map_err(|_|LeaseRefusal::Database)?;
            Ok(())
        })();
        if let Err(err)=created{
            // Existing active reservation means no new owner may resume the
            // model, even if this contender was rejected.
            if !self.has_unreconciled().unwrap_or(true){
                let _=model.resume_new_inference();
            }
            return Err(err);
        }
        let evidence=(||->Result<GpuSample,LeaseRefusal>{
            if let Some(session)=&model_state.original_session_ref{
                model.unload_selected(session,&request.lease_ref)?;
            }
            if !model.observed_unloaded()?{
                return Err(LeaseRefusal::ModelFailed)
            }
            Self::prove_capacity(probe,&request.device_ref,
                request.required_free_vram_bytes)
        })();
        match evidence{
            Ok(sample)=>{
                // Admission may be revoked while the large GGUF context is
                // draining. Recheck immediately before committing GRANTED.
                if let Err(reason)=authority.admit(request){
                    self.quarantine(&request.lease_ref);
                    return Err(reason)
                }
                let done=(||->Result<(),LeaseRefusal>{
                    let conn=self.lock_conn()?;
                    let count=conn.execute(
                        "UPDATE lu_gpu_nvp_leases
                         SET phase='GRANTED',observed_free_bytes=?1,
                         updated_at=CURRENT_TIMESTAMP
                         WHERE lease_ref=?2 AND phase='DRAINING'",
                        params![i64::try_from(sample.available_vram_bytes)
                            .map_err(|_|LeaseRefusal::NoVerifiedCapacity)?,request.lease_ref])
                        .map_err(|_|LeaseRefusal::Database)?;
                    if count!=1{return Err(LeaseRefusal::Quarantined)}
                    Ok(())
                })();
                if let Err(err)=done{self.quarantine(&request.lease_ref);return Err(err)}
                Ok(GrantedGpuLease{lease_ref:request.lease_ref.clone(),
                    device_ref:request.device_ref.clone(),
                    available_vram_bytes:sample.available_vram_bytes,
                    core_epoch_ref:request.original.core_epoch_ref.clone()})
            }
            Err(err)=>{self.quarantine(&request.lease_ref);Err(err)}
        }
    }
    fn stored(&self,ref_id:&str)->Result<Recorded,LeaseRefusal>{
        let conn=self.lock_conn()?;
        let row:Option<(String,String,String,String,String,String,String,String,
            String,i64,i64,String,Option<String>,Option<String>)>=conn.query_row(
            "SELECT lease_ref,device_ref,module_id,workspace_ref,
               conversation_ref,user_turn_ref,task_ref,core_epoch_ref,
               session_ref,required_bytes,duration_ms,phase,
               selected_model_ref,selected_session_ref
             FROM lu_gpu_nvp_leases WHERE lease_ref=?1",
            [ref_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,
                r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?,r.get(8)?,
                r.get(9)?,r.get(10)?,r.get(11)?,r.get(12)?,r.get(13)?))
        ).optional().map_err(|_|LeaseRefusal::Database)?;
        let Some((lease,device,module,workspace,conv,turn,task,epoch,
            session,bytes,duration,phase,model_ref,model_session))=row else{
            return Err(LeaseRefusal::WrongOwner)
        };
        Ok(Recorded{request:GpuLeaseRequest{
            lease_ref:lease,device_ref:device,
            original:LeaseOrigin{module_id:module,workspace_ref:workspace,
                conversation_ref:conv,user_turn_ref:turn,task_ref:task,
                core_epoch_ref:epoch,session_ref:session},
            required_free_vram_bytes:u64::try_from(bytes).map_err(|_|LeaseRefusal::Database)?,
            max_duration_ms:u64::try_from(duration).map_err(|_|LeaseRefusal::Database)?,
        },phase:LeasePhase::parse(&phase)?,model_ref,model_session})
    }
    /// Exact immutable original stored scope for a received release ref.
    /// The installed GPU ingress independently verifies the signed peer,
    /// host user-turn origin and Core-minted original operation identity.
    pub fn original_request(&self,lease_ref:&str)
        ->Result<GpuLeaseRequest,LeaseRefusal>{
        Ok(self.stored(lease_ref)?.request)
    }
    pub fn phase(&self,lease_ref:&str)->Result<LeasePhase,LeaseRefusal>{
        Ok(self.stored(lease_ref)?.phase)
    }
    /// NVP's all-clear is only a request: independent supervised worker
    /// termination and TWO host-origin GPU samples gate model restoration.
    pub fn release_and_restore(
        &self,request:&GpuLeaseRequest,
        authority:&impl OriginalTurnAuthority,
        workers:&impl NvpWorkerStopVerifier,
        probe:&impl HostGpuObserver,
        model:&impl HostModelControl,
    )->Result<ReleaseOutcome,LeaseRefusal>{
        request.checked()?;
        authority.admit_cleanup(request)?;
        let _lock=self.transition.lock().map_err(|_|LeaseRefusal::Database)?;
        let old=self.stored(&request.lease_ref)?;
        if old.request!=*request{return Err(LeaseRefusal::WrongOwner)}
        if old.phase==LeasePhase::Restored{return Ok(ReleaseOutcome::AlreadyRestored)}
        if old.phase!=LeasePhase::Granted{return Err(LeaseRefusal::Quarantined)}
        if !workers.witnessed_empty(request)?{
            return Err(LeaseRefusal::NoIndependentWorkerStop);
        }
        self.mark(&request.lease_ref,LeasePhase::Granted,LeasePhase::Releasing)?;
        let completed=(||->Result<(),LeaseRefusal>{
            // Stop witness alone cannot verify GPU VRAM returned.
            Self::prove_capacity(probe,&request.device_ref,
                request.required_free_vram_bytes)?;
            self.mark(&request.lease_ref,LeasePhase::Releasing,LeasePhase::Restoring)?;
            if let (Some(model_ref),Some(session))=
                (old.model_ref.as_deref(),old.model_session.as_deref()){
                model.restore_selected(model_ref,session,&request.lease_ref)?;
            }
            Self::require_model_restored(model,old.model_ref.as_deref(),
                old.model_session.as_deref())?;
            model.resume_new_inference()?;
            if let Err(e)=self.mark(&request.lease_ref,LeasePhase::Restoring,
                LeasePhase::Restored){
                let _=model.hold_new_inference();
                return Err(e)
            }
            Ok(())
        })();
        if let Err(e)=completed{self.quarantine(&request.lease_ref);return Err(e)}
        Ok(ReleaseOutcome::Restored)
    }
    /// Explicit post-crash recovery. The quarantined row is immutable
    /// original authority; never trust a new NVP lease request to reconstruct
    /// it. Requires independent stopped ORIGINAL worker group, fresh device
    /// measurement, original Core signer/epoch admission, and a model pause.
    /// This is NOT called automatically on startup.
    pub fn reconcile_quarantined_original(
        &self,lease_ref:&str,
        authority:&impl OriginalRecoveryAuthority,
        workers:&impl NvpWorkerStopVerifier,
        probe:&impl HostGpuObserver,
        model:&impl HostModelControl,
    )->Result<ReleaseOutcome,LeaseRefusal>{
        if !safe_ref(lease_ref){return Err(LeaseRefusal::InvalidScope)}
        let _lock=self.transition.lock().map_err(|_|LeaseRefusal::Database)?;
        let original=self.stored(lease_ref)?;
        if original.phase==LeasePhase::Restored{
            return Ok(ReleaseOutcome::AlreadyRestored)
        }
        if original.phase!=LeasePhase::Quarantined{
            return Err(LeaseRefusal::Quarantined)
        }
        authority.admit_original_recovery(&original.request)?;
        // Assert this BEFORE probing or restoring: current model generations
        // must not slip in after a cold-start/quarantine reconciliation.
        model.hold_new_inference()?;
        if !workers.witnessed_empty(&original.request)?{
            return Err(LeaseRefusal::NoIndependentWorkerStop)
        }
        Self::prove_capacity(probe,&original.request.device_ref,
            original.request.required_free_vram_bytes)?;
        let status=model.snapshot()?;
        if status.active_generation ||
           (status.selected_model_ref.is_some() &&
            (status.selected_model_ref!=original.model_ref ||
             status.original_session_ref!=original.model_session)){
            return Err(LeaseRefusal::ModelBusy)
        }
        self.mark(lease_ref,LeasePhase::Quarantined,LeasePhase::Restoring)?;
        let complete=(||->Result<(),LeaseRefusal>{
            match (original.model_ref.as_deref(),original.model_session.as_deref()){
                (Some(model_ref),Some(session)) if status.selected_model_ref.is_none()=>{
                    model.restore_selected(model_ref,session,lease_ref)?;
                }
                (Some(_),Some(_))=>(), // prior interrupted restore; exact pinned match
                (None,None) if status.selected_model_ref.is_none()=>(),
                _=>return Err(LeaseRefusal::ModelFailed),
            }
            Self::require_model_restored(model,original.model_ref.as_deref(),
                original.model_session.as_deref())?;
            // A Core process can crash after model load but before durable
            // RESTORED; the next reconcile will recheck same exact model.
            model.resume_new_inference()?;
            if let Err(e)=self.mark(lease_ref,LeasePhase::Restoring,LeasePhase::Restored){
                let _=model.hold_new_inference();
                return Err(e)
            }
            Ok(())
        })();
        if let Err(err)=complete{
            self.quarantine(lease_ref);
            return Err(err)
        }
        Ok(ReleaseOutcome::Restored)
    }
    /// Host startup prerequisite. This must be wired before admitting the
    /// first Core inference after a crash: a persisted unclosed NVP lease
    /// fences all new GPU model/generation admissions until OS reconciliation.
    pub fn reassert_recovery_fence(
        &self,model:&impl HostModelControl,
    )->Result<bool,LeaseRefusal>{
        let outstanding=self.has_unreconciled()?;
        if outstanding{model.hold_new_inference()?;}
        Ok(outstanding)
    }
    /// All persisted in-flight states become QUARANTINED on open. The caller
    /// must HOLD model admission and obtain fresh original worker/GPU proof.
    /// Time passing or process death is not a positive release receipt.
    pub fn has_unreconciled(&self)->Result<bool,LeaseRefusal>{
        let conn=self.lock_conn()?;
        let count:i64=conn.query_row(
            "SELECT COUNT(*) FROM lu_gpu_nvp_leases WHERE phase!='RESTORED'",
            [],|r|r.get(0)).map_err(|_|LeaseRefusal::Database)?;
        Ok(count>0)
    }
}
/// Read-only actual native Windows observer. No NVP self-report or
/// process-local DXGI budget alone is accepted. Missing WDDM global counters
/// return failure; Linux kernel GPU ownership is independently qualified.
#[derive(Default)]
pub struct InstalledHostGpuObserver;
impl HostGpuObserver for InstalledHostGpuObserver {
    fn observe(&self,device_ref:&str)->Result<GpuSample,LeaseRefusal>{
        #[cfg(windows)]
        {
            let native=lu_win_worker_job::ipc_gpu_vram::sample_device(device_ref)
                .map_err(|_|LeaseRefusal::NoVerifiedCapacity)?;
            Ok(GpuSample{
                device_ref:native.device_ref,
                available_vram_bytes:native.available_vram_bytes,
                sample_sequence:native.sequence,
                source_code:"WIN_DXGI_WDDM_GLOBAL_V1".into(),
            })
        }
        #[cfg(not(windows))]
        {
            let _=device_ref;
            Err(LeaseRefusal::NoVerifiedCapacity)
        }
    }
}
/// Independent installed NVP supervisor receipt checker. NVP's message
/// only supplies the original lease_ref; the verifier reads protected local
/// SCM-origin evidence, checks the fixed installer public pin and binds every
/// original user-turn/worker field. No raw signed bytes are taken from IPC.
#[derive(Default)]
pub struct InstalledNvpWorkerStopVerifier;
impl NvpWorkerStopVerifier for InstalledNvpWorkerStopVerifier{
    fn witnessed_empty(&self,request:&GpuLeaseRequest)->Result<bool,LeaseRefusal>{
        #[cfg(windows)]
        {
            let origin=&request.original;
            let bound=lu_win_worker_job::ipc_nvp_gpu_stop::NvpOriginalGpuLease{
                lease_ref:&request.lease_ref,
                device_ref:&request.device_ref,
                module_id:&origin.module_id,
                workspace_ref:&origin.workspace_ref,
                conversation_ref:&origin.conversation_ref,
                user_turn_ref:&origin.user_turn_ref,
                task_ref:&origin.task_ref,
                core_epoch_ref:&origin.core_epoch_ref,
                nvp_session_ref:&origin.session_ref,
            };
            lu_win_worker_job::ipc_nvp_gpu_stop::
                independently_verified_nvp_worker_empty(&bound)
                .map(|_|true)
                .map_err(|_|LeaseRefusal::NoIndependentWorkerStop)
        }
        #[cfg(not(windows))]
        {
            let _=request;
            Err(LeaseRefusal::NoIndependentWorkerStop)
        }
    }
}
/// The genuine Lu supervisor implements Core's model load/unload boundary.
/// The API adapter does NOT provide an OS driver VRAM verifier.
impl<B:crate::inference_host::InferenceBackend+Send+'static>
    HostModelControl for crate::inference_supervisor::TrustedInferenceSupervisor<B>{
    fn snapshot(&self)->Result<ModelAtHandoff,LeaseRefusal>{
        let s=self.status().map_err(|_|LeaseRefusal::ModelFailed)?;
        Ok(ModelAtHandoff{selected_model_ref:s.model_ref,
            original_session_ref:s.session_id,
            active_generation:s.generation_id.is_some()})
    }
    fn hold_new_inference(&self)->Result<(),LeaseRefusal>{
        self.pause_for_gpu_lease(true).map_err(|_|LeaseRefusal::ModelFailed)
    }
    fn unload_selected(&self,session:&str,id:&str)->Result<(),LeaseRefusal>{
        self.unload(session,id).map(|_|()).map_err(|err|match err{
            crate::inference_supervisor::SupervisorError::Busy=>LeaseRefusal::ModelBusy,
            _=>LeaseRefusal::ModelFailed})
    }
    fn observed_unloaded(&self)->Result<bool,LeaseRefusal>{
        self.status().map(|s|s.session_id.is_none()&&s.pid.is_none())
            .map_err(|_|LeaseRefusal::ModelFailed)
    }
    fn restore_selected(&self,model_ref:&str,session:&str,id:&str)
        ->Result<(),LeaseRefusal>{
        self.load_for_gpu_lease_restore(model_ref,session.to_owned(),id)
            .map(|_|()).map_err(|_|LeaseRefusal::ModelFailed)
    }
    fn ready_after_restore(&self)->Result<bool,LeaseRefusal>{
        self.status().map(|s|s.phase==crate::inference_host::EnginePhase::Ready &&
            s.session_id.is_some() && s.model_ref.is_some())
            .map_err(|_|LeaseRefusal::ModelFailed)
    }
    fn resume_new_inference(&self)->Result<(),LeaseRefusal>{
        self.pause_for_gpu_lease(false).map_err(|_|LeaseRefusal::ModelFailed)
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use std::sync::atomic::{AtomicBool,AtomicU64,Ordering};
    struct Auth;
    impl OriginalTurnAuthority for Auth{
        fn admit(&self,r:&GpuLeaseRequest)->Result<(),LeaseRefusal>{
            if r.original.user_turn_ref=="turn-1"{Ok(())}
            else{Err(LeaseRefusal::NotAuthorized)}
        }
    }
    struct Model{paused:AtomicBool,loaded:AtomicBool,busy:AtomicBool}
    impl Model{
        fn new()->Self{Self{paused:AtomicBool::new(false),
            loaded:AtomicBool::new(true),busy:AtomicBool::new(false)}}
    }
    impl HostModelControl for Model{
        fn snapshot(&self)->Result<ModelAtHandoff,LeaseRefusal>{
            let loaded=self.loaded.load(Ordering::SeqCst);
            Ok(ModelAtHandoff{
                selected_model_ref:loaded.then(||"pinned-gguf".into()),
                original_session_ref:loaded.then(||"core-model-session".into()),
                active_generation:self.busy.load(Ordering::SeqCst)})
        }
        fn hold_new_inference(&self)->Result<(),LeaseRefusal>{
            self.paused.store(true,Ordering::SeqCst);Ok(())
        }
        fn unload_selected(&self,_:&str,_:&str)->Result<(),LeaseRefusal>{
            if self.busy.load(Ordering::SeqCst){return Err(LeaseRefusal::ModelBusy)}
            self.loaded.store(false,Ordering::SeqCst);Ok(())
        }
        fn observed_unloaded(&self)->Result<bool,LeaseRefusal>{
            Ok(!self.loaded.load(Ordering::SeqCst))
        }
        fn restore_selected(&self,_:&str,_:&str,_:&str)->Result<(),LeaseRefusal>{
            self.loaded.store(true,Ordering::SeqCst);Ok(())
        }
        fn ready_after_restore(&self)->Result<bool,LeaseRefusal>{
            Ok(self.loaded.load(Ordering::SeqCst))
        }
        fn resume_new_inference(&self)->Result<(),LeaseRefusal>{
            self.paused.store(false,Ordering::SeqCst);Ok(())
        }
    }
    struct Probe{n:AtomicU64,free:u64}
    impl HostGpuObserver for Probe{
        fn observe(&self,d:&str)->Result<GpuSample,LeaseRefusal>{
            Ok(GpuSample{device_ref:d.into(),available_vram_bytes:self.free,
                sample_sequence:self.n.fetch_add(1,Ordering::SeqCst)+1,
                source_code:"fixture-only".into()})
        }
    }
    struct Stop(bool);
    impl NvpWorkerStopVerifier for Stop{
        fn witnessed_empty(&self,_:&GpuLeaseRequest)->Result<bool,LeaseRefusal>{Ok(self.0)}
    }
    fn req()->GpuLeaseRequest{
        GpuLeaseRequest{lease_ref:"lease-1".into(),device_ref:"gpu-0".into(),
            required_free_vram_bytes:8*1024*1024*1024,max_duration_ms:360000,
            original:LeaseOrigin{module_id:"nvp.film".into(),workspace_ref:"film-1".into(),
                conversation_ref:"conversation-1".into(),user_turn_ref:"turn-1".into(),
                task_ref:"task-1".into(),core_epoch_ref:"epoch-1".into(),
                session_ref:"nvp-session-1".into()}}
    }
    fn db()->std::path::PathBuf{
        let t=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .unwrap().as_nanos();
        std::env::temp_dir().join(format!("lu-gpu-lease-test-{}-{t}.sqlite",std::process::id()))
    }
    #[test]
    fn genuine_two_phase_handoff_and_no_unwitnessed_nvp_all_clear(){
        let path=db();
        let mgr=CoreGpuLeaseManager::open(&path).unwrap();
        let m=Model::new();let a=Auth;
        let p=Probe{n:AtomicU64::new(0),free:12*1024*1024*1024};
        let request=req();
        let granted=mgr.request_handoff(&request,&a,&m,&p).unwrap();
        assert_eq!(granted.lease_ref,"lease-1");
        assert!(m.paused.load(Ordering::SeqCst));
        assert!(!m.loaded.load(Ordering::SeqCst));
        assert_eq!(mgr.phase("lease-1"),Ok(LeasePhase::Granted));
        assert_eq!(mgr.request_handoff(&request,&a,&m,&p),
            Err(LeaseRefusal::Replayed));
        assert_eq!(mgr.release_and_restore(&request,&a,&Stop(false),&p,&m),
            Err(LeaseRefusal::NoIndependentWorkerStop));
        assert!(m.paused.load(Ordering::SeqCst));
        assert_eq!(mgr.release_and_restore(&request,&a,&Stop(true),&p,&m),
            Ok(ReleaseOutcome::Restored));
        assert!(m.loaded.load(Ordering::SeqCst));
        assert!(!m.paused.load(Ordering::SeqCst));
        assert!(!mgr.has_unreconciled().unwrap());
        assert_eq!(mgr.release_and_restore(&request,&a,&Stop(true),&p,&m),
            Ok(ReleaseOutcome::AlreadyRestored));
        drop(mgr);
        let _=std::fs::remove_file(path);
    }
    #[test]
    fn crash_reopens_only_in_quarantine_not_auto_reload(){
        let path=db();
        let m=Model::new();let a=Auth;
        let p=Probe{n:AtomicU64::new(0),free:12*1024*1024*1024};
        let request=req();
        {
            let mgr=CoreGpuLeaseManager::open(&path).unwrap();
            mgr.request_handoff(&request,&a,&m,&p).unwrap();
        }
        let recovered=CoreGpuLeaseManager::open_and_fence_at_startup(&path,&m).unwrap();
        assert!(m.paused.load(Ordering::SeqCst));
        assert_eq!(recovered.phase("lease-1"),Ok(LeasePhase::Quarantined));
        assert!(recovered.has_unreconciled().unwrap());
        assert_eq!(recovered.release_and_restore(&request,&a,&Stop(true),&p,&m),
            Err(LeaseRefusal::Quarantined));
        assert!(!m.loaded.load(Ordering::SeqCst));
        assert!(recovered.reassert_recovery_fence(&m).unwrap());
        struct Recovery;
        impl OriginalRecoveryAuthority for Recovery{
            fn admit_original_recovery(&self,r:&GpuLeaseRequest)->Result<(),LeaseRefusal>{
                if r.original.user_turn_ref=="turn-1"{Ok(())}
                else{Err(LeaseRefusal::NotAuthorized)}
            }
        }
        assert_eq!(recovered.reconcile_quarantined_original(
            "lease-1",&Recovery,&Stop(false),&p,&m),
            Err(LeaseRefusal::NoIndependentWorkerStop));
        assert_eq!(recovered.reconcile_quarantined_original(
            "lease-1",&Recovery,&Stop(true),&p,&m),Ok(ReleaseOutcome::Restored));
        assert!(m.loaded.load(Ordering::SeqCst));
        assert!(!m.paused.load(Ordering::SeqCst));
        let _=std::fs::remove_file(path);
    }
    #[test]
    fn foreign_turn_busy_model_or_unmeasured_vram_never_grants(){
        let m=Model::new();let a=Auth;
        let p=Probe{n:AtomicU64::new(0),free:32};
        let path=db();let mgr=CoreGpuLeaseManager::open(&path).unwrap();
        let mut request=req();request.original.user_turn_ref="other".into();
        assert_eq!(mgr.request_handoff(&request,&a,&m,&p),
            Err(LeaseRefusal::NotAuthorized));
        request=req();m.busy.store(true,Ordering::SeqCst);
        assert_eq!(mgr.request_handoff(&request,&a,&m,&p),
            Err(LeaseRefusal::Deferred));
        assert!(!m.paused.load(Ordering::SeqCst));
        m.busy.store(false,Ordering::SeqCst);
        assert_eq!(mgr.request_handoff(&request,&a,&m,&p),
            Err(LeaseRefusal::NoVerifiedCapacity));
        assert_eq!(mgr.phase("lease-1"),Ok(LeasePhase::Quarantined));
        assert!(m.paused.load(Ordering::SeqCst));
        let _=std::fs::remove_file(path);
    }
}
