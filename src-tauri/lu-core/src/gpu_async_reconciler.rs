//! LU-IPC-GPU-01C: durable, bounded nonblocking NVP GPU transitions.
//!
//! Accepted != GRANTED. The original Core-host authorization is checked
//! synchronously before WAL QUEUED, then rechecked by the supervised native
//! worker before any actual model/VRAM operation. The worker owns real GGUF
//! unload/reload, OS GPU observation and independently installed NVP Job
//! witness through the existing CoreGpuLeaseManager. Never run on the Tokio
//! socket accept thread, never auto-replay a crashed effect and never infer
//! VRAM release from a timeout or textual all-clear.
use crate::gpu_resource_leases::{
    CoreGpuLeaseManager,GpuLeaseRequest,GrantedGpuLease,HostGpuObserver,
    HostModelControl,LeaseRefusal,NvpWorkerStopVerifier,OriginalTurnAuthority,
    ReleaseOutcome,
};
use rusqlite::{params,Connection,OptionalExtension,TransactionBehavior};
use sha2::{Digest,Sha256};
use std::{path::Path,sync::{Arc,Mutex}};
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum GpuAsyncKind{Acquire,Release}
impl GpuAsyncKind{
    fn code(self)->&'static str{match self{Self::Acquire=>"ACQUIRE",Self::Release=>"RELEASE"}}
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum GpuAsyncPhase{Queued,Running,Granted,Restored,Denied,Uncertain}
impl GpuAsyncPhase{
    fn code(self)->&'static str{match self{
        Self::Queued=>"QUEUED",Self::Running=>"RUNNING",
        Self::Granted=>"GRANTED",Self::Restored=>"RESTORED",
        Self::Denied=>"DENIED",Self::Uncertain=>"UNCERTAIN",
    }}
    fn parse(s:&str)->Result<Self,LeaseRefusal>{match s{
        "QUEUED"=>Ok(Self::Queued),"RUNNING"=>Ok(Self::Running),
        "GRANTED"=>Ok(Self::Granted),"RESTORED"=>Ok(Self::Restored),
        "DENIED"=>Ok(Self::Denied),"UNCERTAIN"=>Ok(Self::Uncertain),
        _=>Err(LeaseRefusal::Database),
    }}
}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct GpuAsyncStatus{
    pub lease_ref:String,
    pub kind:GpuAsyncKind,
    pub phase:GpuAsyncPhase,
    pub observed_available_vram_bytes:Option<u64>,
    pub reason:&'static str,
}
fn reason_for(err:&LeaseRefusal)->&'static str{
    match err{
        LeaseRefusal::Deferred|LeaseRefusal::ModelBusy=>"MODEL_BUSY",
        LeaseRefusal::Occupied=>"GPU_LEASE_CONFLICT",
        LeaseRefusal::Replayed=>"REPLAY_REFUSED",
        LeaseRefusal::NotAuthorized=>"ORIGINAL_GRANT_REVOKED",
        LeaseRefusal::WrongOwner=>"ORIGINAL_SCOPE_CHANGED",
        LeaseRefusal::InvalidScope=>"INVALID_GPU_REQUEST",
        LeaseRefusal::NoVerifiedCapacity=>"GPU_CAPACITY_UNVERIFIED",
        LeaseRefusal::NoIndependentWorkerStop=>"NVP_JOB_STOP_UNVERIFIED",
        LeaseRefusal::Quarantined=>"RECOVERY_REQUIRED",
        LeaseRefusal::ModelFailed=>"MODEL_RESTORE_UNCERTAIN",
        LeaseRefusal::Database=>"DURABLE_GPU_JOURNAL_UNAVAILABLE",
    }
}
fn name(s:&str)->bool{
    !s.is_empty()&&s.len()<=128&&s.bytes().all(|b|
        b.is_ascii_alphanumeric()||matches!(b,b'-'|b'_'|b'.'|b':'|b'/'))
}
fn fingerprint(req:&GpuLeaseRequest,kind:GpuAsyncKind)->[u8;32]{
    let mut h=Sha256::new();
    for field in [
        "LU_GPU_ASYNC_ORIGINAL_V1",kind.code(),&req.lease_ref,
        &req.original.module_id,&req.original.workspace_ref,
        &req.original.conversation_ref,&req.original.user_turn_ref,
        &req.original.task_ref,&req.original.core_epoch_ref,
        &req.original.session_ref,&req.device_ref
    ]{
        h.update((field.len() as u32).to_le_bytes());h.update(field.as_bytes());
    }
    h.update(req.required_free_vram_bytes.to_le_bytes());
    h.update(req.max_duration_ms.to_le_bytes());
    h.finalize().into()
}
pub trait NativeGpuWork:Send+Sync+'static{
    fn authorize(&self,request:&GpuLeaseRequest,kind:GpuAsyncKind)
        ->Result<(),LeaseRefusal>;
    fn execute(&self,request:&GpuLeaseRequest,kind:GpuAsyncKind)
        ->Result<Option<u64>,LeaseRefusal>;
}
/// The only native transition backend. The OS probe and NVP stop witness
/// remain private and independently installed, not pluggable from IPC bytes.
pub struct CoreInstalledGpuWork<M,A,P,W>{
    pub manager:Arc<CoreGpuLeaseManager>,
    pub model:Arc<M>,
    pub authority:Arc<A>,
    pub probe:Arc<P>,
    pub workers:Arc<W>,
}
impl<M,A,P,W> NativeGpuWork for CoreInstalledGpuWork<M,A,P,W>
where M:HostModelControl+Send+Sync+'static,
      A:OriginalTurnAuthority+Send+Sync+'static,
      P:HostGpuObserver+Send+Sync+'static,
      W:NvpWorkerStopVerifier+Send+Sync+'static {
    fn authorize(&self,request:&GpuLeaseRequest,kind:GpuAsyncKind)
        ->Result<(),LeaseRefusal>{
        match kind{
            GpuAsyncKind::Acquire=>self.authority.admit(request),
            GpuAsyncKind::Release=>self.authority.admit_cleanup(request),
        }
    }
    fn execute(&self,request:&GpuLeaseRequest,kind:GpuAsyncKind)
        ->Result<Option<u64>,LeaseRefusal>{
        match kind{
            GpuAsyncKind::Acquire=>{
                let granted=self.manager.request_handoff(
                    request,self.authority.as_ref(),self.model.as_ref(),
                    self.probe.as_ref())?;
                Ok(Some(granted.available_vram_bytes))
            },
            GpuAsyncKind::Release=>{
                match self.manager.release_and_restore(
                    request,self.authority.as_ref(),self.workers.as_ref(),
                    self.probe.as_ref(),self.model.as_ref())?{
                    ReleaseOutcome::Restored|ReleaseOutcome::AlreadyRestored=>Ok(None),
                }
            },
        }
    }
}
pub struct DurableGpuAsyncQueue{
    conn:Mutex<Connection>,
}
impl DurableGpuAsyncQueue{
    pub fn open(path:&Path)->Result<Self,LeaseRefusal>{
        let conn=Connection::open(path).map_err(|_|LeaseRefusal::Database)?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=FULL;
             CREATE TABLE IF NOT EXISTS lu_gpu_async_original_operations (
               lease_ref TEXT NOT NULL,
               kind TEXT NOT NULL CHECK(kind IN ('ACQUIRE','RELEASE')),
               fingerprint BLOB NOT NULL CHECK(length(fingerprint)=32),
               original_json BLOB NOT NULL CHECK(length(original_json)>0),
               phase TEXT NOT NULL CHECK(phase IN
                 ('QUEUED','RUNNING','GRANTED','RESTORED','DENIED','UNCERTAIN')),
               reason TEXT NOT NULL,
               verified_bytes INTEGER,
               updated_utc TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
               PRIMARY KEY(lease_ref,kind)
             );
             CREATE UNIQUE INDEX IF NOT EXISTS one_gpu_async_work_at_a_time
               ON lu_gpu_async_original_operations((1))
               WHERE phase IN ('QUEUED','RUNNING');
             UPDATE lu_gpu_async_original_operations
               SET phase='UNCERTAIN',reason='RESTART_RECONCILE_REQUIRED',
                   updated_utc=CURRENT_TIMESTAMP
               WHERE phase IN ('QUEUED','RUNNING');"
        ).map_err(|_|LeaseRefusal::Database)?;
        Ok(Self{conn:Mutex::new(conn)})
    }
    fn connect(&self)->Result<std::sync::MutexGuard<'_,Connection>,LeaseRefusal>{
        self.conn.lock().map_err(|_|LeaseRefusal::Database)
    }
    fn read(&self,request:&GpuLeaseRequest,kind:GpuAsyncKind)
        ->Result<Option<GpuAsyncStatus>,LeaseRefusal>{
        let conn=self.connect()?;
        let found:Option<(Vec<u8>,String,String,Option<i64>)>=conn.query_row(
            "SELECT fingerprint,phase,reason,verified_bytes
             FROM lu_gpu_async_original_operations
             WHERE lease_ref=?1 AND kind=?2",
            params![request.lease_ref,kind.code()],
            |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))
        ).optional().map_err(|_|LeaseRefusal::Database)?;
        let Some((sha,phase,reason,bytes))=found else{return Ok(None)};
        if sha.as_slice()!=&fingerprint(request,kind)[..] {
            return Err(LeaseRefusal::WrongOwner)
        }
        let code=match reason.as_str(){
            "QUEUED"=>"QUEUED","RUNNING"=>"RUNNING",
            "GPU_READY"=>"GPU_READY","LU_MODEL_READY"=>"LU_MODEL_READY",
            "MODEL_BUSY"=>"MODEL_BUSY","GPU_LEASE_CONFLICT"=>"GPU_LEASE_CONFLICT",
            "REPLAY_REFUSED"=>"REPLAY_REFUSED",
            "ORIGINAL_GRANT_REVOKED"=>"ORIGINAL_GRANT_REVOKED",
            "ORIGINAL_SCOPE_CHANGED"=>"ORIGINAL_SCOPE_CHANGED",
            "INVALID_GPU_REQUEST"=>"INVALID_GPU_REQUEST",
            "GPU_CAPACITY_UNVERIFIED"=>"GPU_CAPACITY_UNVERIFIED",
            "NVP_JOB_STOP_UNVERIFIED"=>"NVP_JOB_STOP_UNVERIFIED",
            "RECOVERY_REQUIRED"=>"RECOVERY_REQUIRED",
            "MODEL_RESTORE_UNCERTAIN"=>"MODEL_RESTORE_UNCERTAIN",
            "DURABLE_GPU_JOURNAL_UNAVAILABLE"=>"DURABLE_GPU_JOURNAL_UNAVAILABLE",
            "RESTART_RECONCILE_REQUIRED"=>"RESTART_RECONCILE_REQUIRED",
            _=>return Err(LeaseRefusal::Database),
        };
        Ok(Some(GpuAsyncStatus{
            lease_ref:request.lease_ref.clone(),kind,
            phase:GpuAsyncPhase::parse(&phase)?,
            observed_available_vram_bytes:bytes
                .map(|b|u64::try_from(b).map_err(|_|LeaseRefusal::Database))
                .transpose()?,
            reason:code,
        }))
    }
    pub fn original_for_poll(&self,lease_ref:&str,kind:GpuAsyncKind)
        ->Result<GpuLeaseRequest,LeaseRefusal>{
        if !name(lease_ref){return Err(LeaseRefusal::InvalidScope)}
        let conn=self.connect()?;
        let found:Option<(Vec<u8>,Vec<u8>)>=conn.query_row(
            "SELECT original_json,fingerprint
             FROM lu_gpu_async_original_operations
             WHERE lease_ref=?1 AND kind=?2",
            params![lease_ref,kind.code()],
            |r|Ok((r.get(0)?,r.get(1)?))
        ).optional().map_err(|_|LeaseRefusal::Database)?;
        let (serialized,digest)=found.ok_or(LeaseRefusal::WrongOwner)?;
        if serialized.len()>4096{return Err(LeaseRefusal::Database)}
        let request:GpuLeaseRequest=serde_json::from_slice(&serialized)
            .map_err(|_|LeaseRefusal::Database)?;
        if request.lease_ref!=lease_ref ||
           digest.as_slice()!=&fingerprint(&request,kind)[..]{
            return Err(LeaseRefusal::WrongOwner)
        }
        Ok(request)
    }
    pub fn has_unreconciled(&self)->Result<bool,LeaseRefusal>{
        let conn=self.connect()?;
        let pending:i64=conn.query_row(
            "SELECT COUNT(*) FROM lu_gpu_async_original_operations
             WHERE phase IN ('QUEUED','RUNNING','UNCERTAIN')",
            [],|r|r.get(0)).map_err(|_|LeaseRefusal::Database)?;
        Ok(pending!=0)
    }
    /// Must be called during DesktopState::new, before making its GGUF
    /// supervisor available to the UI, native tools or the IPC listener.
    pub fn open_and_fence_at_startup(
        path:&Path,model:&impl HostModelControl,
    )->Result<Self,LeaseRefusal>{
        let ledger=Self::open(path)?;
        if ledger.has_unreconciled()?{model.hold_new_inference()?;}
        Ok(ledger)
    }
    pub fn status(&self,request:&GpuLeaseRequest,kind:GpuAsyncKind)
        ->Result<GpuAsyncStatus,LeaseRefusal>{
        if !name(&request.lease_ref){return Err(LeaseRefusal::InvalidScope)}
        self.read(request,kind)?.ok_or(LeaseRefusal::WrongOwner)
    }
    fn insert_queued(&self,request:&GpuLeaseRequest,kind:GpuAsyncKind)
        ->Result<bool,LeaseRefusal>{
        let digest=fingerprint(request,kind);
        let mut conn=self.connect()?;
        let transaction=conn.transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_|LeaseRefusal::Database)?;
        let prior:Option<Vec<u8>>=transaction.query_row(
            "SELECT fingerprint FROM lu_gpu_async_original_operations
             WHERE lease_ref=?1 AND kind=?2",
            params![request.lease_ref,kind.code()],
            |r|r.get(0)).optional().map_err(|_|LeaseRefusal::Database)?;
        if let Some(previous)=prior{
            if previous.as_slice()!=digest{return Err(LeaseRefusal::WrongOwner)}
            return Ok(false) // idempotent; NEVER launch duplicate work
        }
        let other:Option<String>=transaction.query_row(
            "SELECT lease_ref FROM lu_gpu_async_original_operations
             WHERE phase IN ('QUEUED','RUNNING') LIMIT 1",
            [],|r|r.get(0)).optional().map_err(|_|LeaseRefusal::Database)?;
        if other.is_some(){return Err(LeaseRefusal::Occupied)}
        transaction.execute(
            "INSERT INTO lu_gpu_async_original_operations
               (lease_ref,kind,fingerprint,original_json,phase,reason)
             VALUES(?1,?2,?3,?4,'QUEUED','QUEUED')",
            params![request.lease_ref,kind.code(),digest.as_slice(),
                serde_json::to_vec(request).map_err(|_|LeaseRefusal::Database)?])
            .map_err(|_|LeaseRefusal::Database)?;
        transaction.commit().map_err(|_|LeaseRefusal::Database)?;
        Ok(true)
    }
    fn mark(&self,request:&GpuLeaseRequest,kind:GpuAsyncKind,
        from:GpuAsyncPhase,to:GpuAsyncPhase,reason:&str,verified:Option<u64>)
        ->Result<(),LeaseRefusal>{
        let conn=self.connect()?;
        let n=conn.execute(
            "UPDATE lu_gpu_async_original_operations
               SET phase=?1,reason=?2,verified_bytes=?3,updated_utc=CURRENT_TIMESTAMP
             WHERE lease_ref=?4 AND kind=?5 AND fingerprint=?6 AND phase=?7",
            params![to.code(),reason,verified.map(|v|v as i64),
                request.lease_ref,kind.code(),fingerprint(request,kind).as_slice(),
                from.code()]
        ).map_err(|_|LeaseRefusal::Database)?;
        if n!=1{return Err(LeaseRefusal::Quarantined)}
        Ok(())
    }
    /// WAL QUEUED is committed BEFORE returning a DEFERRED acknowledgment.
    /// A bounded runtime worker runs the GGUF transition without blocking
    /// NVP's authenticated named-pipe read/write loop.
    pub fn submit(self:&Arc<Self>,request:GpuLeaseRequest,kind:GpuAsyncKind,
        backend:Arc<dyn NativeGpuWork>)
        ->Result<GpuAsyncStatus,LeaseRefusal>{
        if !name(&request.lease_ref){return Err(LeaseRefusal::InvalidScope)}
        backend.authorize(&request,kind)?;
        let fresh=self.insert_queued(&request,kind)?;
        if fresh {
            let queue=Arc::clone(self);
            let original=request.clone();
            tokio::task::spawn_blocking(move||{
                if queue.mark(&original,kind,GpuAsyncPhase::Queued,
                    GpuAsyncPhase::Running,"RUNNING",None).is_err(){return;}
                let result=backend.authorize(&original,kind)
                    .and_then(|_|backend.execute(&original,kind));
                let (phase,reason,observed)=match result{
                    Ok(observed)=>match kind{
                        GpuAsyncKind::Acquire=>(
                            GpuAsyncPhase::Granted,"GPU_READY",observed),
                        GpuAsyncKind::Release=>(
                            GpuAsyncPhase::Restored,"LU_MODEL_READY",None),
                    },
                    Err(refusal)=>{
                        let state=if matches!(refusal,
                            LeaseRefusal::ModelFailed|LeaseRefusal::Database|
                            LeaseRefusal::NoVerifiedCapacity|
                            LeaseRefusal::NoIndependentWorkerStop|
                            LeaseRefusal::Quarantined){
                            GpuAsyncPhase::Uncertain
                        } else {GpuAsyncPhase::Denied};
                        (state,reason_for(&refusal),None)
                    },
                };
                let _=queue.mark(&original,kind,GpuAsyncPhase::Running,
                    phase,reason,observed);
                // Failed durable mark => surviving RUNNING becomes UNCERTAIN
                // at next startup. No second worker is ever auto-started.
            });
        }
        self.status(&request,kind)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize,Ordering};
    struct Worker{calls:AtomicUsize}
    impl NativeGpuWork for Worker{
        fn authorize(&self,request:&GpuLeaseRequest,_:GpuAsyncKind)
            ->Result<(),LeaseRefusal>{
            if request.original.user_turn_ref=="turn-1"{Ok(())}
            else{Err(LeaseRefusal::NotAuthorized)}
        }
        fn execute(&self,_:&GpuLeaseRequest,kind:GpuAsyncKind)
            ->Result<Option<u64>,LeaseRefusal>{
            self.calls.fetch_add(1,Ordering::SeqCst);
            Ok(match kind{GpuAsyncKind::Acquire=>Some(8_000_000_000),
                GpuAsyncKind::Release=>None})
        }
    }
    fn request()->GpuLeaseRequest{
        GpuLeaseRequest{
            lease_ref:"lease-test-1".into(),device_ref:"gpu-0".into(),
            required_free_vram_bytes:8_000_000_000,max_duration_ms:120_000,
            original:crate::gpu_resource_leases::LeaseOrigin{
                module_id:"nvp.film".into(),workspace_ref:"film-1".into(),
                conversation_ref:"chat-1".into(),user_turn_ref:"turn-1".into(),
                task_ref:"task-1".into(),core_epoch_ref:"epoch-1".into(),
                session_ref:"session-1".into(),
            }
        }
    }
    fn path()->std::path::PathBuf{
        let n=std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        std::env::temp_dir().join(format!("lu-gpu-async-{}-{n}.sqlite",std::process::id()))
    }
    #[tokio::test]
    async fn async_prepared_is_not_granted_until_worker_commits(){
        let p=path();
        let queue=Arc::new(DurableGpuAsyncQueue::open(&p).unwrap());
        let backend=Arc::new(Worker{calls:AtomicUsize::new(0)});
        let original=request();
        let first=queue.submit(original.clone(),GpuAsyncKind::Acquire,backend.clone()).unwrap();
        assert!(matches!(first.phase,GpuAsyncPhase::Queued|GpuAsyncPhase::Running|GpuAsyncPhase::Granted));
        assert_eq!(queue.submit(original.clone(),GpuAsyncKind::Acquire,backend.clone()).unwrap().lease_ref,
            original.lease_ref);
        for _ in 0..60{
            if queue.status(&original,GpuAsyncKind::Acquire).unwrap().phase==GpuAsyncPhase::Granted {break}
            tokio::task::yield_now().await;
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        assert_eq!(queue.status(&original,GpuAsyncKind::Acquire).unwrap().phase,
            GpuAsyncPhase::Granted);
        assert_eq!(backend.calls.load(Ordering::SeqCst),1);
        drop(queue);
        let _=std::fs::remove_file(p);
    }
    #[test]
    fn accepted_before_spawn_and_reopened_unknown_never_replayed(){
        let p=path();
        let original=request();
        let queue=DurableGpuAsyncQueue::open(&p).unwrap();
        assert!(queue.insert_queued(&original,GpuAsyncKind::Acquire).unwrap());
        drop(queue);
        let recovered=DurableGpuAsyncQueue::open(&p).unwrap();
        assert_eq!(recovered.status(&original,GpuAsyncKind::Acquire).unwrap().phase,
            GpuAsyncPhase::Uncertain);
        assert!(!recovered.insert_queued(&original,GpuAsyncKind::Acquire).unwrap());
        assert!(recovered.has_unreconciled().unwrap());
        assert_eq!(recovered.original_for_poll("lease-test-1",GpuAsyncKind::Acquire).unwrap(),original);
        let mut changed=original.clone();changed.original.user_turn_ref="evil".into();
        assert_eq!(recovered.status(&changed,GpuAsyncKind::Acquire),
            Err(LeaseRefusal::WrongOwner));
        let _=std::fs::remove_file(p);
    }
}
