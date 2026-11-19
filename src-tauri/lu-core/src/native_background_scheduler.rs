//! P02 — durable, host-owned background queue and conservative resource guardian.
//!
//! Reuses the existing TaskStore terminal/notice journal and the real
//! BackgroundProcessSupervisor for OS process ownership. No shell, model
//! filesystem authority or duplicated worker launcher is introduced.
//! Queue rows contain no executable, arguments, user file paths or tokens.
//! After a host crash previously queued/running rows require new operator
//! admission: never silently replay a possibly applied effect.
use std::{
    collections::BTreeMap,
    path::Path,
    sync::{Arc,Mutex},
    time::{SystemTime,UNIX_EPOCH},
};
use rusqlite::{params,Connection,OptionalExtension};
use crate::{
    native_background_process::{BackgroundProcessSupervisor,TrustedWorkerPlan},
    tasks::{TaskState,TaskStore},
};

const MAX_QUEUE:usize=128;
const MAX_VIEW:usize=100;
const MAX_PRIORITY:u8=2;
const MAX_REPEAT:u64=365*24*60*60;
const MIN_REPEAT:u64=60;

#[derive(Debug,Clone,Copy,PartialEq,Eq,Default)]
pub struct ResourceClaim {
    pub workers:u16, pub cpu:u16, pub ram_mib:u32,
    pub gpu:u16, pub vram_mib:u32, pub disk:u16, pub network:u16,
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct ResourceBudget {
    pub workers:u16, pub cpu:u16, pub ram_mib:u32,
    pub gpu:u16, pub vram_mib:u32, pub disk:u16, pub network:u16,
}

/// PERF-01 P4: the native host's interactive admission decision. This
/// reports *conservative permissions to launch new workers*, not realtime
/// per-process OS CPU/RAM consumption or a verified VRAM measurement.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum InteractiveAdmissionReason {
    ForegroundModelTurn,
    SingleProcessor,
    UnknownMemory,
    UnknownCpuWithResidentModel,
    MemoryPressure,
    CpuPressure,
    BackgroundPermitted,
}
impl InteractiveAdmissionReason {
    pub fn as_str(self)->&'static str {
        match self {
            Self::ForegroundModelTurn=>"FOREGROUND_MODEL_TURN",
            Self::SingleProcessor=>"SINGLE_PROCESSOR",
            Self::UnknownMemory=>"UNKNOWN_MEMORY",
            Self::UnknownCpuWithResidentModel=>"UNKNOWN_CPU_WITH_RESIDENT_MODEL",
            Self::MemoryPressure=>"MEMORY_PRESSURE",
            Self::CpuPressure=>"CPU_PRESSURE",
            Self::BackgroundPermitted=>"BACKGROUND_PERMITTED",
        }
    }
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct InteractiveAdmission {
    pub budget:ResourceBudget,
    pub defer_new_background:bool,
    pub reason:InteractiveAdmissionReason,
}
impl ResourceBudget {
    /// Re-evaluated from original host-only pressure samples each watchdog
    /// cycle. A resident GGUF retains one logical CPU and at least 1 GiB
    /// available physical RAM for foreground prompt assembly. Never infer
    /// free VRAM from host RAM or pretend a zero-GPU claim uses no GPU.
    ///
    /// Defer affects NEW background starts only; existing host-supervised
    /// children retain original custody and durable reconciliation.
    pub fn interactive_admission(
        logical_cpus:usize,
        model_resident:bool,
        foreground_pending:bool,
        pressure:crate::native_resource_pressure::HostPressure,
    )->InteractiveAdmission {
        let mut budget=Self::for_parallelism(logical_cpus);
        if model_resident {
            budget.workers=budget.workers.min(1);
            budget.cpu=budget.cpu.min(1);
            budget.ram_mib=budget.ram_mib.min(512);
        }
        budget=budget.under_host_pressure(pressure);
        // No independent first-party GPU/VRAM oracle is available yet.
        // These are unconditional ZERO even on physical Windows/Radeon.
        budget.gpu=0;
        budget.vram_mib=0;
        let reason=if foreground_pending {
            InteractiveAdmissionReason::ForegroundModelTurn
        }else if logical_cpus<=1 {
            InteractiveAdmissionReason::SingleProcessor
        }else if pressure.available_ram_mib.is_none() {
            InteractiveAdmissionReason::UnknownMemory
        }else if model_resident && pressure.cpu_busy_percent.is_none() {
            InteractiveAdmissionReason::UnknownCpuWithResidentModel
        }else if pressure.available_ram_mib.is_some_and(|n|n<1536) {
            InteractiveAdmissionReason::MemoryPressure
        }else if pressure.cpu_busy_percent.is_some_and(|n|n>=85) {
            InteractiveAdmissionReason::CpuPressure
        }else{
            InteractiveAdmissionReason::BackgroundPermitted
        };
        InteractiveAdmission {
            budget,defer_new_background:reason!=InteractiveAdmissionReason::BackgroundPermitted,
            reason,
        }
    }
    /// Reserve at least one CPU for foreground work; GPU budget defaults to
    /// zero until a runtime/driver-specific guardian verifies it.
    pub fn interactive_first() -> Self {
        let total=std::thread::available_parallelism()
            .map(|v|v.get()).unwrap_or(1);
        Self::for_parallelism(total)
    }
    fn for_parallelism(total:usize)->Self{
        let slots=total.saturating_sub(1).clamp(1,2) as u16;
        // The scheduler's capacity must stay nonzero for its SQLite state
        // machine, but no job may be admitted on a genuine single-CPU host.
        let memory=if total<=1{1}else{1024};
        Self{workers:slots,cpu:slots,ram_mib:memory,
            gpu:0,vram_mib:0,disk:1,network:0}
    }
    /// Physical RAM and CPU pressure gate starts without claiming GPU VRAM
    /// or arbitrary OS-wide enforcement. Unknown RAM forbids new jobs.
    pub fn under_host_pressure(mut self,p:crate::native_resource_pressure::HostPressure)->Self{
        self.ram_mib=match p.available_ram_mib{
            Some(free)=>self.ram_mib.min(
                free.saturating_sub(1024).min(u64::from(u32::MAX)).max(1) as u32),
            None=>1,
        };
        if let Some(cpu)=p.cpu_busy_percent{
            if cpu>=80{self.workers=self.workers.min(1);self.cpu=self.cpu.min(1);}
            if cpu>=95{self.ram_mib=1;}
        }
        self
    }
    fn supports(self,used:ResourceClaim,job:ResourceClaim)->bool{
        used.workers.checked_add(job.workers).is_some_and(|n|n<=self.workers)
            &&used.cpu.checked_add(job.cpu).is_some_and(|n|n<=self.cpu)
            &&used.ram_mib.checked_add(job.ram_mib).is_some_and(|n|n<=self.ram_mib)
            &&used.gpu.checked_add(job.gpu).is_some_and(|n|n<=self.gpu)
            &&used.vram_mib.checked_add(job.vram_mib).is_some_and(|n|n<=self.vram_mib)
            &&used.disk.checked_add(job.disk).is_some_and(|n|n<=self.disk)
            &&used.network.checked_add(job.network).is_some_and(|n|n<=self.network)
    }
}
impl ResourceClaim{
    pub fn file_scan()->Self{
        Self{workers:1,cpu:1,ram_mib:512,gpu:0,
            vram_mib:0,disk:1,network:0}
    }
    pub fn ocr()->Self{
        Self{workers:1,cpu:1,ram_mib:512,gpu:0,
            vram_mib:0,disk:0,network:0}
    }
    /// P06 WGC OS picker plus one bounded BGRA D3D11 readback. Uses one
    /// ordinary worker, CPU and RAM slot. GPU/VRAM remain ZERO because WGC
    /// has no validated GPU accounting here; this is *not* a GPU limit or
    /// evidence that the worker uses no GPU resources.
    pub fn capture_one_shot()->Self{
        Self{workers:1,cpu:1,ram_mib:512,gpu:0,
            vram_mib:0,disk:0,network:0}
    }
    fn add(self,b:Self)->Self{
        Self{
            workers:self.workers.saturating_add(b.workers),
            cpu:self.cpu.saturating_add(b.cpu),
            ram_mib:self.ram_mib.saturating_add(b.ram_mib),
            gpu:self.gpu.saturating_add(b.gpu),
            vram_mib:self.vram_mib.saturating_add(b.vram_mib),
            disk:self.disk.saturating_add(b.disk),
            network:self.network.saturating_add(b.network),
        }
    }
    fn valid(self)->bool{
        self.workers==1&&self.cpu>=1&&self.cpu<=64
            &&self.ram_mib>=1&&self.ram_mib<=65536
            &&self.gpu<=8&&self.disk<=8&&self.network<=8
    }
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum SchedulerError {
    InvalidRequest, Duplicate, QueueFull, WrongOwner, NotFound,
    NotQueued, NotRunning, ResourceRejected, Database, Poisoned,
    WorkerLaunch, WorkerControl, TaskStore,
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum Priority{Background=0,Normal=1,Interactive=2}
impl Priority {
    fn score(self)->u8{self as u8}
    fn decode(v:u8)->Option<Self>{
        match v{0=>Some(Self::Background),1=>Some(Self::Normal),
            2=>Some(Self::Interactive),_=>None}
    }
    pub fn name(self)->&'static str{
        match self{Self::Background=>"BACKGROUND",Self::Normal=>"NORMAL",
            Self::Interactive=>"INTERACTIVE"}
    }
}
#[derive(Debug,Clone)]
pub struct ScheduleOptions {
    pub kind:String,
    pub due_unix:u64,
    pub repeat_seconds:Option<u64>,
    pub priority:Priority,
    pub claim:ResourceClaim,
    pub dependency:Option<String>,
    /// Retries are allowed *only* for a host-confirmed idempotent job.
    pub safe_retry:bool,
    pub max_retries:u8,
}
impl ScheduleOptions{
    pub fn file_scan(due_unix:u64,repeat_seconds:Option<u64>,priority:Priority)->Self{
        Self{kind:"file_scan".into(),due_unix,repeat_seconds,
            priority,claim:ResourceClaim::file_scan(),
            dependency:None,safe_retry:false,max_retries:0}
    }
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct ScheduleView{
    pub task_id:String,pub workspace_id:String,pub kind:String,
    pub state:String,pub priority:String,pub due_unix:u64,
    pub repeat_seconds:Option<u64>,pub dependency:Option<String>,
    pub can_pause:bool,pub can_resume:bool,pub can_cancel:bool,
    pub requires_review:bool,pub last_result:String,
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct GuardianSnapshot{
    pub capacity:ResourceBudget,pub used:ResourceClaim,
    pub queued:u32,pub running:u32,pub paused:u32,
}

/// Bounded host plan, prepared by the trusted native picker.
/// Hold the exact input read-only with write and delete sharing denied, and
/// verify the same file ID in the child BEFORE reading any content.
#[cfg(any(windows,target_os="linux"))]
pub struct ReviewedScan{
    plan:TrustedWorkerPlan,
    input:std::fs::File,
    ancestors:Vec<std::fs::File>,
    display:String,
    identity:String,
    bytes:u64,
    turn:String,
}
#[cfg(any(windows,target_os="linux"))]
impl ReviewedScan{
    #[cfg(windows)]
    pub fn prepare(task_id:&str,workspace:&str,turn:&str,selected:&Path)->Result<Self,SchedulerError>{
        use std::{fs::{self,OpenOptions},io::Read,os::windows::fs::{OpenOptionsExt,MetadataExt},path::{Component,Prefix}};
        use sha2::{Digest,Sha256};
        if !valid_id(task_id)||!task_id.starts_with("scan-")||!valid_id(workspace)
            ||!matches!(selected.components().next(),Some(Component::Prefix(p)) if matches!(p.kind(),Prefix::Disk(_)|Prefix::VerbatimDisk(_)))
            ||!selected.is_absolute()||selected.components().any(|p|matches!(p,Component::ParentDir|Component::CurDir)){
            return Err(SchedulerError::InvalidRequest)
        }
        // Lock the local directory chain root-to-leaf against rename or
        // reparse alteration until the selected worker is reaped.
        let mut ancestors=Vec::new();
        let chain=selected.ancestors().skip(1).filter(|p|p.has_root()).collect::<Vec<_>>();
        if chain.is_empty()||chain.len()>64{return Err(SchedulerError::InvalidRequest)}
        for ancestor in chain.into_iter().rev(){
            let dir=OpenOptions::new().access_mode(0x80).share_mode(1)
                .custom_flags(0x0220_0000).open(ancestor).map_err(|_|SchedulerError::InvalidRequest)?;
            crate::windows_file_guard::opened_directory_identity(&dir)
                .map_err(|_|SchedulerError::InvalidRequest)?;
            ancestors.push(dir);
        }
        let meta=fs::symlink_metadata(selected).map_err(|_|SchedulerError::InvalidRequest)?;
        if meta.file_attributes()&0x400!=0{return Err(SchedulerError::InvalidRequest)}
        let input=OpenOptions::new().read(true).share_mode(1)
            .custom_flags(0x0020_0000).open(selected).map_err(|_|SchedulerError::InvalidRequest)?;
        let identity=crate::windows_file_guard::inspect_open_file(&input)
            .map_err(|_|SchedulerError::InvalidRequest)?.token();
        let bytes=input.metadata().map_err(|_|SchedulerError::InvalidRequest)?.len();
        if bytes==0||bytes>64*1024*1024*1024{return Err(SchedulerError::InvalidRequest)}
        let display=selected.to_str().filter(|s|s.len()<=2048&&!s.chars().any(|c|
            c.is_control()||matches!(c,'\u{202a}'..='\u{202e}'|'\u{2066}'..='\u{2069}'|'\u{200e}'|'\u{200f}'|'\u{061c}')))
            .ok_or(SchedulerError::InvalidRequest)?.to_owned();
        let executable=std::env::current_exe().map_err(|_|SchedulerError::InvalidRequest)?;
        let mut image=fs::File::open(&executable).map_err(|_|SchedulerError::InvalidRequest)?;
        let mut hash=Sha256::new();let mut buffer=[0u8;65536];
        loop{let n=image.read(&mut buffer).map_err(|_|SchedulerError::InvalidRequest)?;
            if n==0{break}hash.update(&buffer[..n]);}
        let plan=TrustedWorkerPlan{task_id:task_id.into(),workspace_id:workspace.into(),executable,
            expected_sha256:format!("{:x}",hash.finalize()),progress_protocol:true,max_runtime_seconds:Some(3600),
            args:vec!["--lu-file-scan-worker".into(),selected.as_os_str().to_owned(),identity.clone().into(),bytes.to_string().into()]};
        Ok(Self{plan,input,ancestors,display,identity,bytes,turn:turn.into()})
    }

    #[cfg(target_os="linux")]
    pub fn prepare(task_id:&str,workspace:&str,turn:&str,selected:&Path)->Result<Self,SchedulerError>{
        use std::{fs::{self,OpenOptions},io::Read,os::unix::fs::OpenOptionsExt};
        use sha2::{Digest,Sha256};
        let id=|s:&str,p:&str|s.starts_with(p)&&s.len()==p.len()+32&&s[p.len()..].bytes().all(|b|
            b.is_ascii_digit()||matches!(b,b'a'..=b'f'));
        if !id(task_id,"scan-")||!id(workspace,"c-")||!id(turn,"g-")||
            !selected.is_absolute()||selected.canonicalize().ok().as_deref()!=Some(selected){
            return Err(SchedulerError::InvalidRequest)
        }
        let display=selected.to_str().filter(|s|s.len()<=2048&&!s.chars().any(|c|c.is_control()||
            matches!(c,'\u{202a}'..='\u{202e}'|'\u{2066}'..='\u{2069}'|'\u{200e}'|'\u{200f}'|'\u{061c}')))
            .ok_or(SchedulerError::InvalidRequest)?.to_owned();
        // O_NOFOLLOW | O_NONBLOCK prevents symlink and substituted FIFO
        // admission. Keep the opened inode alive to prevent ID reuse.
        let input=OpenOptions::new().read(true).custom_flags(0o400000|0o4000)
            .open(selected).map_err(|_|SchedulerError::InvalidRequest)?;
        let identity=linux_scan_identity(&input).map_err(|_|SchedulerError::InvalidRequest)?;
        let bytes=input.metadata().map_err(|_|SchedulerError::InvalidRequest)?.len();
        let executable=std::env::current_exe().map_err(|_|SchedulerError::InvalidRequest)?
            .canonicalize().map_err(|_|SchedulerError::InvalidRequest)?;
        let mut file=fs::File::open(&executable).map_err(|_|SchedulerError::InvalidRequest)?;
        let mut hash=Sha256::new();let mut buffer=[0u8;65536];
        loop{let n=file.read(&mut buffer).map_err(|_|SchedulerError::InvalidRequest)?;
            if n==0{break}hash.update(&buffer[..n]);}
        let plan=TrustedWorkerPlan{task_id:task_id.into(),workspace_id:workspace.into(),executable,
            expected_sha256:format!("{:x}",hash.finalize()),progress_protocol:true,max_runtime_seconds:Some(90),
            args:vec!["--lu-file-scan-worker".into(),selected.as_os_str().to_owned(),identity.clone().into(),bytes.to_string().into()]};
        Ok(Self{plan,input,ancestors:Vec::new(),display,identity,bytes,turn:turn.into()})
    }
    pub fn task_id(&self)->&str{&self.plan.task_id}
    pub fn workspace(&self)->&str{&self.plan.workspace_id}
    pub fn display(&self)->&str{&self.display}
    pub fn identity(&self)->&str{&self.identity}
    pub fn bytes(&self)->u64{self.bytes}
    pub fn executable_sha256(&self)->&str{&self.plan.expected_sha256}
}


// Linux cannot deny writes like a Windows share-mode handle. Bind device,
// inode, length and kernel change/write timestamps before reading, and
// require the same fingerprint after reading before accepting a digest.
#[cfg(target_os="linux")]
#[path="reviewed_scan_identity.rs"]
mod scan_identity;
#[cfg(target_os="linux")]
pub use scan_identity::linux_scan_identity;

struct Active{
    plan:TrustedWorkerPlan,
    opts:ScheduleOptions,
    series_id:String,
    sequence_no:u32,
    retry_count:u8,
    // Keep the originally opened, write/delete-denied input alive until terminal custody.
    _reviewed_input:Option<(std::fs::File,Vec<std::fs::File>)>,
    reviewed_turn:Option<String>,
}
struct Inner {
    db:Connection,
    plans:BTreeMap<String,Active>,
    running:BTreeMap<String,ResourceClaim>,
    external:BTreeMap<String,(String,ResourceClaim)>,
    capacity:ResourceBudget,
    // Applies to new scheduled launches AND explicit guardian leases.
    // Never cancels an existing verified native process.
    interactive_admission_hold:bool,
}
/// A short-lived, host-only lease for native OCR and other approved
/// non-scheduler tool workers. Dropping it releases only a reservation; the
/// called tool's original authority, cancellation and cleanup remain intact.
pub struct GuardianLease {
    owner:Arc<BackgroundScheduler>,
    task_id:String,
    workspace_id:String,
}
impl Drop for GuardianLease{
    fn drop(&mut self){
        if let Ok(mut g)=self.owner.inner.lock(){
            if g.external.get(&self.task_id)
                .is_some_and(|(workspace,_)|workspace==&self.workspace_id){
                g.external.remove(&self.task_id);
            }
        }
    }
}
pub struct BackgroundScheduler{
    store:Arc<TaskStore>,
    process:Arc<BackgroundProcessSupervisor>,
    inner:Mutex<Inner>,
}
fn valid_id(s:&str)->bool{
    !s.is_empty()&&s.len()<=96&&s.bytes().all(|b|
        b.is_ascii_alphanumeric()||matches!(b,b'-'|b'_'))
}
pub fn now_unix()->u64{
    SystemTime::now().duration_since(UNIX_EPOCH)
        .map_or(0,|v|v.as_secs())
}
fn to_sql(v:u64)->Result<i64,SchedulerError>{
    i64::try_from(v).map_err(|_|SchedulerError::InvalidRequest)
}
fn db_error(_:rusqlite::Error)->SchedulerError{SchedulerError::Database}
impl BackgroundScheduler{
    pub fn open(
        path:&Path,store:Arc<TaskStore>,
        process:Arc<BackgroundProcessSupervisor>,budget:ResourceBudget
    )->Result<Self,SchedulerError>{
        if budget.workers==0||budget.cpu==0||budget.ram_mib==0{
            return Err(SchedulerError::InvalidRequest)
        }
        let db=Connection::open(path).map_err(db_error)?;
        db.pragma_update(None,"journal_mode","WAL").map_err(db_error)?;
        db.pragma_update(None,"synchronous","FULL").map_err(db_error)?;
        db.execute_batch(
          "CREATE TABLE IF NOT EXISTS lu_schedule(
             task_id TEXT PRIMARY KEY,
             workspace_id TEXT NOT NULL,
             kind TEXT NOT NULL,
             state TEXT NOT NULL CHECK(state IN
               ('QUEUED','RUNNING','PAUSED','COMPLETED','FAILED',
                'CANCELLED','TIMED_OUT','INTERRUPTED','NEEDS_REVIEW')),
             priority INTEGER NOT NULL CHECK(priority BETWEEN 0 AND 2),
             due_unix INTEGER NOT NULL,
             repeat_seconds INTEGER,
             dependency TEXT,
             workers INTEGER NOT NULL,cpu INTEGER NOT NULL,
             ram_mib INTEGER NOT NULL,gpu INTEGER NOT NULL,
             vram_mib INTEGER NOT NULL,disk INTEGER NOT NULL,
             network INTEGER NOT NULL,
             safe_retry INTEGER NOT NULL,max_retries INTEGER NOT NULL,
             retry_count INTEGER NOT NULL,
             series_id TEXT NOT NULL,sequence_no INTEGER NOT NULL,
             last_result TEXT NOT NULL DEFAULT '',
             created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
           );
           CREATE INDEX IF NOT EXISTS lu_schedule_workspace
              ON lu_schedule(workspace_id,state,due_unix);
           CREATE INDEX IF NOT EXISTS lu_schedule_due
              ON lu_schedule(state,due_unix,priority);
           CREATE TABLE IF NOT EXISTS lu_schedule_events(
              sequence INTEGER PRIMARY KEY AUTOINCREMENT,
              task_id TEXT NOT NULL, workspace_id TEXT NOT NULL,
              event TEXT NOT NULL, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
           );"
        ).map_err(db_error)?;
        // A crash cannot restore lost in-memory executable/path authority.
        // The original TaskStore independently marks prior RUNNING as
        // INTERRUPTED; these queue rows are NOT replayed on startup.
        db.execute("UPDATE lu_schedule SET state='NEEDS_REVIEW',
                    last_result='host_restart_rebind_required'
                    WHERE state IN ('QUEUED','RUNNING','PAUSED')",[])
            .map_err(db_error)?;
        Ok(Self{store,process,inner:Mutex::new(Inner{
            db,plans:BTreeMap::new(),running:BTreeMap::new(),
            external:BTreeMap::new(),capacity:budget,
            interactive_admission_hold:false,
        })})
    }
    fn insert(
        inner:&mut Inner,plan:&TrustedWorkerPlan,opts:&ScheduleOptions,
        series:&str,sequence_no:u32,retry_count:u8
    )->Result<(),SchedulerError>{
        let id=&plan.task_id;
        let count:i64=inner.db.query_row(
            "SELECT COUNT(*) FROM lu_schedule WHERE state IN
             ('QUEUED','RUNNING','PAUSED')",[],|r|r.get(0))
            .map_err(db_error)?;
        if count>=MAX_QUEUE as i64{return Err(SchedulerError::QueueFull)}
        let tx=inner.db.transaction().map_err(db_error)?;
        tx.execute(
          "INSERT INTO lu_schedule
           (task_id,workspace_id,kind,state,priority,due_unix,repeat_seconds,
            dependency,workers,cpu,ram_mib,gpu,vram_mib,disk,network,
            safe_retry,max_retries,retry_count,series_id,sequence_no)
           VALUES(?1,?2,?3,'QUEUED',?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,
                  ?14,?15,?16,?17,?18,?19)",
          params![id,&plan.workspace_id,&opts.kind,
            i64::from(opts.priority.score()),to_sql(opts.due_unix)?,
            opts.repeat_seconds.map(to_sql).transpose()?,
            &opts.dependency,i64::from(opts.claim.workers),i64::from(opts.claim.cpu),
            i64::from(opts.claim.ram_mib),i64::from(opts.claim.gpu),
            i64::from(opts.claim.vram_mib),i64::from(opts.claim.disk),
            i64::from(opts.claim.network),
            if opts.safe_retry{1}else{0},i64::from(opts.max_retries),
            i64::from(retry_count),series,i64::from(sequence_no)]
        ).map_err(|e|if e.to_string().contains("UNIQUE"){
            SchedulerError::Duplicate}else{SchedulerError::Database})?;
        tx.execute(
            "INSERT INTO lu_schedule_events(task_id,workspace_id,event)
             VALUES(?1,?2,'QUEUED')",
            params![id,&plan.workspace_id]).map_err(db_error)?;
        tx.commit().map_err(db_error)?;
        Ok(())
    }
    /// The host must already have a reviewed plan from a real operator
    /// workflow. This API is never model/HTML accessible. The plan is kept
    /// in memory only; durable metadata is not a persistent execution grant.
    pub fn enqueue_host_plan(&self,plan:TrustedWorkerPlan,opts:ScheduleOptions)
        ->Result<(),SchedulerError>{
        if !valid_id(&plan.task_id)||!valid_id(&plan.workspace_id)
           ||!valid_id(&opts.kind)||!opts.claim.valid()
           ||opts.max_retries>3||(!opts.safe_retry&&opts.max_retries>0)
           ||opts.repeat_seconds.is_some_and(|s|s<MIN_REPEAT||s>MAX_REPEAT)
           ||opts.dependency.as_ref().is_some_and(|d|!valid_id(d)||*d==plan.task_id)
           ||opts.due_unix>now_unix().saturating_add(MAX_REPEAT)
           ||plan.expected_sha256.len()!=64
           ||!plan.expected_sha256.bytes().all(|b|b.is_ascii_hexdigit())
           ||!plan.executable.is_absolute(){
            return Err(SchedulerError::InvalidRequest)
        }
        let mut g=self.inner.lock().map_err(|_|SchedulerError::Poisoned)?;
        if !g.capacity.supports(ResourceClaim::default(),opts.claim){
            return Err(SchedulerError::ResourceRejected)
        }
        if let Some(dep)=opts.dependency.as_ref(){
            let owner:Option<String>=g.db.query_row(
                "SELECT workspace_id FROM lu_schedule WHERE task_id=?1",
                [dep],|r|r.get(0)).optional().map_err(db_error)?;
            if owner.as_deref()!=Some(&plan.workspace_id){
                return Err(SchedulerError::WrongOwner)
            }
        }
        Self::insert(&mut g,&plan,&opts,&plan.task_id,0,0)?;
        g.plans.insert(plan.task_id.clone(),Active{
            series_id:plan.task_id.clone(),sequence_no:0,
            retry_count:0,plan,opts,_reviewed_input:None,reviewed_turn:None,
        });
        Ok(())
    }

    /// Same resource-aware scheduler; no process start in this foreground
    /// turn and no replayable file/executable authority in persistent SQLite.
    #[cfg(any(windows,target_os="linux"))]
    pub fn enqueue_reviewed_scan(&self,reviewed:ReviewedScan)->Result<(),SchedulerError>{
        let ReviewedScan{plan,input,ancestors,identity,bytes,turn,..}=reviewed;
        #[cfg(windows)]
        let actual=crate::windows_file_guard::inspect_open_file(&input)
            .map_err(|_|SchedulerError::InvalidRequest)?.token();
        #[cfg(target_os="linux")]
        let actual=linux_scan_identity(&input).map_err(|_|SchedulerError::InvalidRequest)?;
        if actual!=identity||input.metadata().map_err(|_|SchedulerError::InvalidRequest)?.len()!=bytes{
            return Err(SchedulerError::InvalidRequest)
        }
        let mut opts=ScheduleOptions::file_scan(now_unix(),None,Priority::Normal);
        opts.kind="reviewed_file_scan".into();
        let mut g=self.inner.lock().map_err(|_|SchedulerError::Poisoned)?;
        if !g.capacity.supports(ResourceClaim::default(),opts.claim){
            return Err(SchedulerError::ResourceRejected)
        }
        Self::insert(&mut g,&plan,&opts,&plan.task_id,0,0)?;
        g.plans.insert(plan.task_id.clone(),Active{series_id:plan.task_id.clone(),
            sequence_no:0,retry_count:0,plan,opts,_reviewed_input:Some((input,ancestors)),reviewed_turn:Some(turn)});
        Ok(())
    }
    fn terminal_name(state:TaskState)->Option<&'static str>{
        match state{
            TaskState::Completed=>Some("COMPLETED"),
            TaskState::Failed=>Some("FAILED"),
            TaskState::Cancelled=>Some("CANCELLED"),
            TaskState::TimedOut=>Some("TIMED_OUT"),
            TaskState::Interrupted=>Some("INTERRUPTED"),
            _=>None,
        }
    }
    /// Called by the Lu host watchdog, never an LLM polling loop.
    /// The existing supervisor is solely responsible for real OS process
    /// spawn, deadline, cancellation, reaping and TaskStore receipts.
    pub fn tick(&self)->Result<usize,SchedulerError>{
        self.tick_at(now_unix())
    }
    pub fn tick_at(&self,now:u64)->Result<usize,SchedulerError>{
        self.tick_at_guarded(now,false)
    }
    /// The desktop's trusted foreground model queue gates only NEW worker
    /// dispatch. Existing native operations retain custody, may complete and
    /// are reconciled above this gate; no running effect is preempted.
    pub fn tick_foreground_guarded(&self,foreground_pending:bool)
        ->Result<usize,SchedulerError>{
        self.tick_at_guarded(now_unix(),foreground_pending)
    }
    /// PERF-01 P4: apply the host-observed resource gate, then recheck the
    /// original foreground queue immediately before EACH native spawn.
    /// The probe must be read-only and bounded. It narrows the admission race
    /// but is not an atomic OS-wide preemption of already-approved workers.
    pub fn tick_interactive_guarded(
        &self,defer_new_background:bool,
        foreground_probe:impl Fn()->bool,
    )->Result<usize,SchedulerError>{
        self.tick_at_guarded_with_probe(
            now_unix(),defer_new_background,foreground_probe
        )
    }
    fn tick_at_guarded(&self,now:u64,foreground_pending:bool)
        ->Result<usize,SchedulerError>{
        self.tick_at_guarded_with_probe(now,foreground_pending,||false)
    }
    fn tick_at_guarded_with_probe(
        &self,now:u64,foreground_pending:bool,foreground_probe:impl Fn()->bool,
    )->Result<usize,SchedulerError>{
        self.process.poll().map_err(|_|SchedulerError::WorkerControl)?;
        let mut g=self.inner.lock().map_err(|_|SchedulerError::Poisoned)?;
        let active_ids=g.running.keys().cloned().collect::<Vec<_>>();
        for id in active_ids{
            let Some(record)=self.store.get(&id).map_err(|_|SchedulerError::TaskStore)? else{
                // An uncertain missing journal must never free resource
                // reservations or cause a replacement to spawn.
                continue
            };
            let Some(terminal)=Self::terminal_name(record.state)else{continue};
            let owner=g.plans.get(&id).ok_or(SchedulerError::Database)?.plan.workspace_id.clone();
            if record.workspace_id.as_deref()!=Some(owner.as_str())||self.process.can_cancel(&id)||self.store.process_uncertainty(&id).map_err(|_|SchedulerError::TaskStore)?.is_some(){
                continue
            }
            let tx=g.db.transaction().map_err(db_error)?;
            if tx.execute(
              "UPDATE lu_schedule SET state=?2,last_result=?2
               WHERE task_id=?1 AND workspace_id=?3 AND state IN ('RUNNING','QUEUED')",
               params![&id,terminal,&owner]).map_err(db_error)?!=1{
                return Err(SchedulerError::Database)
            }
            tx.execute(
              "INSERT INTO lu_schedule_events(task_id,workspace_id,event)
               VALUES(?1,?2,?3)",params![&id,&owner,terminal]).map_err(db_error)?;
            tx.commit().map_err(db_error)?;
            g.running.remove(&id);
            let Some(last)=g.plans.remove(&id)else{continue};
            // A cancelled or interrupted task never automatically replays.
            let retry=matches!(record.state,TaskState::Failed|TaskState::TimedOut)
                &&last.opts.safe_retry&&last.retry_count<last.opts.max_retries;
            let recurring=record.state==TaskState::Completed
                &&last.opts.repeat_seconds.is_some();
            if retry||recurring{
                let next_seq=last.sequence_no.saturating_add(1);
                if next_seq==last.sequence_no{continue}
                let next_id=format!("{}-n{}",last.series_id,next_seq);
                if !valid_id(&next_id){continue}
                let mut next_opts=last.opts.clone();
                next_opts.due_unix=if retry{
                    now.saturating_add(30u64.saturating_mul(1u64<<last.retry_count))
                }else{
                    now.saturating_add(last.opts.repeat_seconds.unwrap_or(0))
                };
                let next_retry=if retry{last.retry_count+1}else{0};
                let mut next_plan=last.plan;
                next_plan.task_id=next_id.clone();
                Self::insert(&mut g,&next_plan,&next_opts,&last.series_id,
                    next_seq,next_retry)?;
                g.plans.insert(next_id,Active{
                    plan:next_plan,opts:next_opts,
                    series_id:last.series_id,sequence_no:next_seq,
                    retry_count:next_retry,_reviewed_input:None,reviewed_turn:None,
                });
            }
        }
        // Preserve terminal reconciliation and durable receipts above while
        // a human model turn is queued/running; do not launch a new worker
        // that competes with the user's first token.
        if foreground_pending || g.interactive_admission_hold { return Ok(0); }
        let mut queued=Vec::new();
        {
            let mut stmt=g.db.prepare(
              "SELECT task_id,priority,due_unix,created_at
               FROM lu_schedule WHERE state='QUEUED' AND due_unix<=?1"
            ).map_err(db_error)?;
            let rows=stmt.query_map([to_sql(now)?],|r|
                Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?,
                    r.get::<_,i64>(2)?,r.get::<_,String>(3)?)))
                .map_err(db_error)?;
            for row in rows{queued.push(row.map_err(db_error)?);}
        }
        // Priority aging prevents an endless sequence of newly requested
        // foreground-adjacent jobs from starving old normal/background work.
        queued.sort_by(|a,b|{
            let aa=(a.1+((to_sql(now).unwrap_or(0)-a.2).max(0)/3600).min(2))
                .min(MAX_PRIORITY as i64);
            let bb=(b.1+((to_sql(now).unwrap_or(0)-b.2).max(0)/3600).min(2))
                .min(MAX_PRIORITY as i64);
            bb.cmp(&aa).then_with(||a.3.cmp(&b.3)).then_with(||a.0.cmp(&b.0))
        });
        let mut started=0;
        for (id,_,_,_) in queued{
            // An accepted worker whose RUNNING publication failed retains
            // reservation and custody until terminal reconciliation.
            if g.running.contains_key(&id){continue}
            let Some(job)=g.plans.get(&id)else{
                // A historical queued job without volatile host approval is
                // never executable. Startup reconciliation marks it review.
                g.db.execute(
                  "UPDATE lu_schedule SET state='NEEDS_REVIEW',
                   last_result='host_plan_unavailable' WHERE task_id=?1",
                  [&id]).map_err(db_error)?;
                continue
            };
            if let Some(dep)=job.opts.dependency.as_ref(){
                let state:Option<String>=g.db.query_row(
                    "SELECT state FROM lu_schedule WHERE task_id=?1",
                    [dep],|r|r.get(0)).optional().map_err(db_error)?;
                match state.as_deref(){
                    Some("COMPLETED")=>{},
                    Some("FAILED"|"CANCELLED"|"TIMED_OUT"|
                         "INTERRUPTED"|"NEEDS_REVIEW")=>{
                        g.db.execute(
                          "UPDATE lu_schedule SET state='NEEDS_REVIEW',
                           last_result='dependency_unavailable'
                           WHERE task_id=?1",
                          [&id]).map_err(db_error)?;
                        g.plans.remove(&id);
                        continue
                    },
                    _=>continue,
                }
            }
            let occupied=g.running.values().fold(ResourceClaim::default(),
                |total,x|total.add(*x));
            let occupied=g.external.values().fold(occupied,
                |total,(_,claim)|total.add(*claim));
            if !g.capacity.supports(occupied,job.opts.claim){continue}
            let plan=TrustedWorkerPlan{
                task_id:job.plan.task_id.clone(),
                workspace_id:job.plan.workspace_id.clone(),
                executable:job.plan.executable.clone(),
                expected_sha256:job.plan.expected_sha256.clone(),
                args:job.plan.args.clone(),
                progress_protocol:job.plan.progress_protocol,
                max_runtime_seconds:job.plan.max_runtime_seconds,
            };
            let claim=job.opts.claim;
            // A human Send may have queued during the admission scan. Do
            // not start another native OS child behind a pending first token.
            // Already-running workers are never killed or silently replayed.
            if foreground_probe(){break}
            #[cfg(target_os="linux")]
            let started_worker=if let Some(turn)=job.reviewed_turn.as_deref(){
                self.process.start_host_approved_reviewed_scan(plan,turn)
            }else{self.process.start_host_approved(plan)};
            #[cfg(not(target_os="linux"))]
            let started_worker=self.process.start_host_approved(plan);
            if started_worker.is_err(){
                // A broker/admission failure after possible process creation
                // cannot release the pinned input or authorize another spawn.
                if self.process.can_cancel(&id)||self.store.process_uncertainty(&id)
                    .map_err(|_|SchedulerError::TaskStore)?.is_some(){
                    g.running.insert(id.clone(),claim);
                    g.db.execute("UPDATE lu_schedule SET last_result='worker_admission_uncertain'
                        WHERE task_id=?1",[&id]).map_err(db_error)?;
                    return Err(SchedulerError::WorkerControl)
                }
                g.db.execute(
                  "UPDATE lu_schedule SET state='FAILED',
                   last_result='worker_admission_failed' WHERE task_id=?1",
                   [&id]).map_err(db_error)?;
                g.plans.remove(&id);
                continue
            }
            // Reserve the worker immediately after successful OS admission.
            // Even a SQLite error must not permit another spawn or release
            // the reviewed input while termination is uncertain.
            g.running.insert(id.clone(),claim);
            let workspace=g.plans.get(&id)
                .map(|v|v.plan.workspace_id.clone()).ok_or(SchedulerError::Database)?;
            let persist=(||->Result<(),SchedulerError>{
                let tx=g.db.transaction().map_err(db_error)?;
                if tx.execute("UPDATE lu_schedule SET state='RUNNING' WHERE task_id=?1
                    AND state='QUEUED'",[&id]).map_err(db_error)?!=1{
                    return Err(SchedulerError::Database)
                }
                tx.execute("INSERT INTO lu_schedule_events(task_id,workspace_id,event)
                    VALUES(?1,?2,'RUNNING')",params![&id,&workspace]).map_err(db_error)?;
                tx.commit().map_err(db_error)?;
                Ok(())
            })();
            if let Err(e)=persist{
                let _=self.process.cancel_owned(&id,&workspace);
                // Keep the in-memory fence and file leases. A later tick
                // can publish terminal state only from the TaskStore.
                return Err(e)
            }
            started+=1;
        }
        Ok(started)
    }
    pub fn list_owned(&self,workspace:&str,limit:usize)
        ->Result<Vec<ScheduleView>,SchedulerError>{
        if !valid_id(workspace)||limit==0||limit>MAX_VIEW{
            return Err(SchedulerError::InvalidRequest)
        }
        let g=self.inner.lock().map_err(|_|SchedulerError::Poisoned)?;
        let mut stmt=g.db.prepare(
          "SELECT task_id,workspace_id,kind,state,priority,due_unix,
             repeat_seconds,dependency,last_result
           FROM lu_schedule WHERE workspace_id=?1
           ORDER BY CASE state WHEN 'RUNNING' THEN 0
             WHEN 'QUEUED' THEN 1 WHEN 'PAUSED' THEN 2
             WHEN 'NEEDS_REVIEW' THEN 3 ELSE 4 END,
             due_unix ASC,task_id ASC LIMIT ?2"
        ).map_err(db_error)?;
        let rows=stmt.query_map(params![workspace,limit as i64],|r|{
            Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,
                r.get::<_,String>(2)?,r.get::<_,String>(3)?,
                r.get::<_,u8>(4)?,r.get::<_,u64>(5)?,
                r.get::<_,Option<u64>>(6)?,r.get::<_,Option<String>>(7)?,
                r.get::<_,String>(8)?))
        }).map_err(db_error)?;
        let mut out=Vec::new();
        for row in rows{
            let (task_id,workspace_id,kind,state,priority,due_unix,
                repeat_seconds,dependency,last_result)=row.map_err(db_error)?;
            out.push(ScheduleView{
                can_pause:state=="QUEUED",can_resume:state=="PAUSED",
                can_cancel:matches!(state.as_str(),"QUEUED"|"PAUSED"|"RUNNING"),
                requires_review:state=="NEEDS_REVIEW",
                priority:Priority::decode(priority).ok_or(SchedulerError::Database)?
                    .name().into(),
                task_id,workspace_id,kind,state,due_unix,
                repeat_seconds,dependency,last_result,
            });
        }
        Ok(out)
    }
    /// Host-only limit updates; shrinking a budget stops further starts,
    /// never kills an already admitted process.
    /// Called only by authenticated native host tool paths, never by model
    /// JSON, WebView-provided ids or unreviewed external worker executables.
    pub fn admit_host_worker(
        self:&Arc<Self>,workspace_id:&str,task_id:&str,claim:ResourceClaim
    )->Result<GuardianLease,SchedulerError>{
        if !valid_id(workspace_id)||!valid_id(task_id)||!claim.valid(){
            return Err(SchedulerError::InvalidRequest)
        }
        let mut g=self.inner.lock().map_err(|_|SchedulerError::Poisoned)?;
        if g.interactive_admission_hold {
            return Err(SchedulerError::ResourceRejected)
        }
        if g.external.contains_key(task_id)||g.running.contains_key(task_id)
            ||g.plans.contains_key(task_id){return Err(SchedulerError::Duplicate)}
        let used=g.running.values().fold(ResourceClaim::default(),|acc,x|acc.add(*x));
        let used=g.external.values().fold(used,|acc,(_,x)|acc.add(*x));
        if !g.capacity.supports(used,claim){return Err(SchedulerError::ResourceRejected)}
        g.external.insert(task_id.into(),(workspace_id.into(),claim));
        Ok(GuardianLease{owner:Arc::clone(self),
            task_id:task_id.into(),workspace_id:workspace_id.into()})
    }
    /// Single host-owned mutation: update resource reservations and whether
    /// NEW workers may be admitted together under the existing guardian
    /// mutex. A model/controller/renderer cannot clear this hold.
    pub fn apply_interactive_admission(
        &self,admission:InteractiveAdmission
    )->Result<(),SchedulerError>{
        let budget=admission.budget;
        if budget.workers==0||budget.cpu==0||budget.ram_mib==0 {
            return Err(SchedulerError::InvalidRequest)
        }
        let mut g=self.inner.lock().map_err(|_|SchedulerError::Poisoned)?;
        g.capacity=budget;
        g.interactive_admission_hold=admission.defer_new_background;
        Ok(())
    }
    pub fn set_budget(&self,budget:ResourceBudget)->Result<(),SchedulerError>{
        if budget.workers==0||budget.cpu==0||budget.ram_mib==0{
            return Err(SchedulerError::InvalidRequest)
        }
        self.inner.lock().map_err(|_|SchedulerError::Poisoned)?.capacity=budget;
        Ok(())
    }
    pub fn guardian_snapshot(&self,workspace:&str)
        ->Result<GuardianSnapshot,SchedulerError>{
        if !valid_id(workspace){return Err(SchedulerError::InvalidRequest)}
        let g=self.inner.lock().map_err(|_|SchedulerError::Poisoned)?;
        let mut counts=[0u32;3];
        {
            let mut stmt=g.db.prepare(
                "SELECT state,COUNT(*) FROM lu_schedule
                 WHERE workspace_id=?1 AND state IN('QUEUED','RUNNING','PAUSED')
                 GROUP BY state").map_err(db_error)?;
            let rows=stmt.query_map([workspace],|r|
                Ok((r.get::<_,String>(0)?,r.get::<_,u32>(1)?)))
                .map_err(db_error)?;
            for row in rows{
                let (state,count)=row.map_err(db_error)?;
                match state.as_str(){
                    "QUEUED"=>counts[0]=count,"RUNNING"=>counts[1]=count,
                    "PAUSED"=>counts[2]=count,_=>{},
                }
            }
        }
        let used=g.running.values().fold(ResourceClaim::default(),
            |a,b|a.add(*b));
        let used=g.external.values().fold(used,
            |a,(_,b)|a.add(*b));
        Ok(GuardianSnapshot{
            capacity:g.capacity,used,
            queued:counts[0],running:counts[1],paused:counts[2],
        })
    }
    fn change_queue_state(&self,id:&str,workspace:&str,expected:&str,to:&str)
        ->Result<(),SchedulerError>{
        if !valid_id(id)||!valid_id(workspace){
            return Err(SchedulerError::InvalidRequest)
        }
        let mut g=self.inner.lock().map_err(|_|SchedulerError::Poisoned)?;
        // A failed RUNNING publication may leave a durable QUEUED row with
        // a live reservation. Never discard its original input or custody.
        if g.running.contains_key(id){return Err(SchedulerError::NotQueued)}
        let tx=g.db.transaction().map_err(db_error)?;
        let affected=tx.execute(
            "UPDATE lu_schedule SET state=?4 WHERE task_id=?1
             AND workspace_id=?2 AND state=?3",
            params![id,workspace,expected,to]).map_err(db_error)?;
        if affected!=1{return Err(SchedulerError::NotQueued)}
        tx.execute(
            "INSERT INTO lu_schedule_events(task_id,workspace_id,event)
             VALUES(?1,?2,?3)",params![id,workspace,to]).map_err(db_error)?;
        tx.commit().map_err(db_error)?;
        if to=="CANCELLED"{g.plans.remove(id);}
        Ok(())
    }

    /// Only a volatile, originally reviewed, never-dispatched one-shot scan
    /// qualifies. A stale QUEUED row after a failed RUNNING publication is
    /// not authority to cancel the worker or release its input lease.
    pub fn reviewed_unstarted_state(&self,id:&str,workspace:&str)
        ->Result<Option<String>,SchedulerError>{
        if !valid_id(id)||!valid_id(workspace){return Err(SchedulerError::InvalidRequest)}
        let g=self.inner.lock().map_err(|_|SchedulerError::Poisoned)?;
        if g.running.contains_key(id)||g.plans.get(id).is_none_or(|p|
            p.reviewed_turn.is_none()||p.plan.workspace_id!=workspace||
            p.opts.kind!="reviewed_file_scan")||self.process.can_cancel(id)
            ||self.store.get(id).map_err(|_|SchedulerError::TaskStore)?.is_some(){
            return Ok(None)
        }
        g.db.query_row("SELECT state FROM lu_schedule WHERE task_id=?1 AND workspace_id=?2
            AND kind='reviewed_file_scan' AND state IN ('QUEUED','PAUSED')",
            params![id,workspace],|r|r.get(0)).optional().map_err(db_error)
    }
    /// Cancellation of an exact reviewed unstarted plan is serialized with
    /// watchdog admission. No conversion to RUNNING cancellation if the
    /// state changed while a human reviewed it. No child/kill receipt.
    pub fn cancel_reviewed_unstarted(&self,id:&str,workspace:&str,expected:&str)
        ->Result<(),SchedulerError>{
        if !matches!(expected,"QUEUED"|"PAUSED")||!valid_id(id)||!valid_id(workspace){
            return Err(SchedulerError::InvalidRequest)
        }
        let mut g=self.inner.lock().map_err(|_|SchedulerError::Poisoned)?;
        if g.running.contains_key(id)||g.plans.get(id).is_none_or(|p|
            p.reviewed_turn.is_none()||p.plan.workspace_id!=workspace||
            p.opts.kind!="reviewed_file_scan")||self.process.can_cancel(id)
            ||self.store.get(id).map_err(|_|SchedulerError::TaskStore)?.is_some(){
            return Err(SchedulerError::NotQueued)
        }
        let tx=g.db.transaction().map_err(db_error)?;
        if tx.execute("UPDATE lu_schedule SET state='CANCELLED',last_result='cancelled_before_start'
            WHERE task_id=?1 AND workspace_id=?2 AND state=?3 AND kind='reviewed_file_scan'",
            params![id,workspace,expected]).map_err(db_error)?!=1{
            return Err(SchedulerError::NotQueued)
        }
        tx.execute("INSERT INTO lu_schedule_events(task_id,workspace_id,event)
            VALUES(?1,?2,'CANCELLED_BEFORE_START')",params![id,workspace]).map_err(db_error)?;
        tx.commit().map_err(db_error)?;
        g.plans.remove(id);
        Ok(())
    }
    pub fn pause_queued(&self,id:&str,workspace:&str)
        ->Result<(),SchedulerError>{
        self.change_queue_state(id,workspace,"QUEUED","PAUSED")
    }
    pub fn resume_queued(&self,id:&str,workspace:&str)
        ->Result<(),SchedulerError>{
        self.change_queue_state(id,workspace,"PAUSED","QUEUED")
    }
    pub fn cancel_owned(&self,id:&str,workspace:&str)
        ->Result<(),SchedulerError>{
        if !valid_id(id)||!valid_id(workspace){
            return Err(SchedulerError::InvalidRequest)
        }
        let state={
            let g=self.inner.lock().map_err(|_|SchedulerError::Poisoned)?;
            g.db.query_row("SELECT state FROM lu_schedule WHERE task_id=?1
                AND workspace_id=?2",params![id,workspace],|r|r.get::<_,String>(0))
                .optional().map_err(db_error)?
        };
        match state.as_deref(){
            Some("QUEUED")=>self.change_queue_state(id,workspace,"QUEUED","CANCELLED"),
            Some("PAUSED")=>self.change_queue_state(id,workspace,"PAUSED","CANCELLED"),
            Some("RUNNING")=>{
                self.process.cancel_owned(id,workspace)
                    .map_err(|_|SchedulerError::WorkerControl)?;
                // Terminal state is finalized by tick, after process reaping.
                Ok(())
            },
            Some(_)=>Err(SchedulerError::NotRunning),
            None=>Err(SchedulerError::NotFound),
        }
    }
    /// After a crash no previous executable/input grant is recoverable.
    /// A trusted desktop picker and fresh native confirmation must build an
    /// entirely new host plan. This explicitly supersedes one NEEDS_REVIEW
    /// schedule without giving a stored path or credential new authority.
    pub fn reapprove_reviewed(
        &self,original_id:&str,new_plan:TrustedWorkerPlan
    )->Result<(),SchedulerError>{
        if !valid_id(original_id)||!valid_id(&new_plan.task_id)||
            !valid_id(&new_plan.workspace_id)||original_id==new_plan.task_id||
            new_plan.args.len()!=2||
            new_plan.args.first().and_then(|s|s.to_str())!=
                Some("--lu-file-scan-worker")||
            !new_plan.executable.is_absolute()||
            new_plan.expected_sha256.len()!=64||
            !new_plan.expected_sha256.bytes().all(|b|b.is_ascii_hexdigit()){
            return Err(SchedulerError::InvalidRequest)
        }
        let mut g=self.inner.lock().map_err(|_|SchedulerError::Poisoned)?;
        let old=g.db.query_row(
            "SELECT kind,state,priority,due_unix,repeat_seconds,dependency,
                workers,cpu,ram_mib,gpu,vram_mib,disk,network,safe_retry,max_retries
             FROM lu_schedule WHERE task_id=?1 AND workspace_id=?2",
            params![original_id,&new_plan.workspace_id],|r|{
                Ok((
                    r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,u8>(2)?,
                    r.get::<_,u64>(3)?,r.get::<_,Option<u64>>(4)?,
                    r.get::<_,Option<String>>(5)?,
                    r.get::<_,u16>(6)?,r.get::<_,u16>(7)?,
                    r.get::<_,u32>(8)?,r.get::<_,u16>(9)?,
                    r.get::<_,u32>(10)?,r.get::<_,u16>(11)?,
                    r.get::<_,u16>(12)?,r.get::<_,bool>(13)?,
                    r.get::<_,u8>(14)?,
                ))
            }).optional().map_err(db_error)?
            .ok_or(SchedulerError::NotFound)?;
        let (kind,state,priority,due,repeat,dependency,
            workers,cpu,ram_mib,gpu,vram_mib,disk,network,
            safe_retry,max_retries)=old;
        if kind!="file_scan"||state!="NEEDS_REVIEW"{
            return Err(SchedulerError::NotQueued)
        }
        let opts=ScheduleOptions{
            kind,priority:Priority::decode(priority)
                .ok_or(SchedulerError::InvalidRequest)?,
            due_unix:due.max(now_unix()).min(now_unix().saturating_add(MAX_REPEAT)),
            repeat_seconds:repeat,dependency,
            claim:ResourceClaim{workers,cpu,ram_mib,gpu,vram_mib,disk,network},
            safe_retry,max_retries,
        };
        if !g.capacity.supports(ResourceClaim::default(),opts.claim)
           ||!opts.claim.valid(){return Err(SchedulerError::ResourceRejected)}
        Self::insert(&mut g,&new_plan,&opts,&new_plan.task_id,0,0)?;
        let changed=g.db.execute(
            "UPDATE lu_schedule SET state='CANCELLED',
             last_result='superseded_after_explicit_reapproval'
             WHERE task_id=?1 AND workspace_id=?2 AND state='NEEDS_REVIEW'",
            params![original_id,&new_plan.workspace_id]).map_err(db_error)?;
        if changed!=1{return Err(SchedulerError::NotQueued)}
        g.db.execute(
            "INSERT INTO lu_schedule_events(task_id,workspace_id,event)
             VALUES(?1,?2,'REAPPROVED')",
            params![original_id,&new_plan.workspace_id]).map_err(db_error)?;
        g.plans.insert(new_plan.task_id.clone(),Active{
            series_id:new_plan.task_id.clone(),sequence_no:0,
            retry_count:0,plan:new_plan,opts,_reviewed_input:None,reviewed_turn:None,
        });
        Ok(())
    }
    pub fn state_for(&self,id:&str,workspace:&str)
        ->Result<Option<String>,SchedulerError>{
        if !valid_id(id)||!valid_id(workspace){
            return Err(SchedulerError::InvalidRequest)
        }
        let g=self.inner.lock().map_err(|_|SchedulerError::Poisoned)?;
        g.db.query_row(
            "SELECT state FROM lu_schedule
             WHERE task_id=?1 AND workspace_id=?2",
            params![id,workspace],|r|r.get(0))
            .optional().map_err(db_error)
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    fn test_scheduler()->(Arc<TaskStore>,Arc<BackgroundProcessSupervisor>,BackgroundScheduler){
        let store=Arc::new(TaskStore::open(Path::new(":memory:")).unwrap());
        let processes=Arc::new(BackgroundProcessSupervisor::new(Arc::clone(&store)));
        let schedule=BackgroundScheduler::open(Path::new(":memory:"),
            Arc::clone(&store),Arc::clone(&processes),ResourceBudget{
                workers:1,cpu:1,ram_mib:512,gpu:0,vram_mib:0,disk:1,network:0,
            }).unwrap();
        (store,processes,schedule)
    }
    fn plan(id:&str)->TrustedWorkerPlan{
        TrustedWorkerPlan{task_id:id.into(),workspace_id:"c-owner".into(),
            executable:std::env::current_exe().unwrap(),
            expected_sha256:"a".repeat(64),args:vec![],
            progress_protocol:false,max_runtime_seconds:None}
    }
    #[test]
    fn queue_is_durable_and_keeps_host_executable_out_of_sqlite(){
        let (_,_,queue)=test_scheduler();
        let mut opts=ScheduleOptions::file_scan(now_unix()+300,None,Priority::Background);
        queue.enqueue_host_plan(plan("scan-01"),opts.clone()).unwrap();
        let list=queue.list_owned("c-owner",8).unwrap();
        assert_eq!(list.len(),1);
        assert_eq!(list[0].state,"QUEUED");
        assert!(!format!("{list:?}").contains("expected_sha256"));
        assert_eq!(queue.list_owned("c-other",8).unwrap().len(),0);
        opts.priority=Priority::Interactive;
        assert_eq!(queue.enqueue_host_plan(plan("scan-01"),opts),
            Err(SchedulerError::Duplicate));
    }
    #[test]
    fn p06_guardian_capture_reserves_cpu_ram_without_unverified_gpu_entitlement(){
        let (_,_,queue)=test_scheduler();
        let queue=Arc::new(queue);
        let claim=ResourceClaim::capture_one_shot();
        assert_eq!((claim.workers,claim.cpu,claim.ram_mib),(1,1,512));
        assert_eq!((claim.gpu,claim.vram_mib,claim.network),(0,0,0));
        let lease=queue.admit_host_worker("c-one","obs-frame-1",claim).unwrap();
        assert!(queue.admit_host_worker("c-one","obs-frame-1",claim).is_err());
        drop(lease);
        assert!(queue.admit_host_worker("c-one","obs-frame-1",claim).is_ok());
    }
    #[test]
    fn perf01_p4_new_workers_yield_to_human_and_unknown_pressure(){
        use crate::native_resource_pressure::HostPressure;
        let calm=HostPressure{available_ram_mib:Some(8192),cpu_busy_percent:Some(25)};
        let free=ResourceBudget::interactive_admission(8,false,false,calm);
        assert!(!free.defer_new_background);
        assert_eq!(free.reason,InteractiveAdmissionReason::BackgroundPermitted);
        assert_eq!((free.budget.gpu,free.budget.vram_mib),(0,0));
        let human=ResourceBudget::interactive_admission(8,true,true,calm);
        assert!(human.defer_new_background);
        assert_eq!(human.reason,InteractiveAdmissionReason::ForegroundModelTurn);
        assert_eq!((human.budget.workers,human.budget.cpu,human.budget.ram_mib),
            (1,1,512));
        let unknown=ResourceBudget::interactive_admission(8,true,false,
            HostPressure{available_ram_mib:None,cpu_busy_percent:None});
        assert!(unknown.defer_new_background);
        assert_eq!(unknown.reason,InteractiveAdmissionReason::UnknownMemory);
        assert_eq!(unknown.budget.ram_mib,1);
        let no_cpu=ResourceBudget::interactive_admission(8,true,false,
            HostPressure{available_ram_mib:Some(8192),cpu_busy_percent:None});
        assert!(no_cpu.defer_new_background);
        assert_eq!(no_cpu.reason,InteractiveAdmissionReason::UnknownCpuWithResidentModel);
    }
    #[test]
    fn perf01_p4_single_processor_cpu_and_memory_pressure_close_new_admission(){
        use crate::native_resource_pressure::HostPressure;
        let nominal=HostPressure{available_ram_mib:Some(8192),cpu_busy_percent:Some(30)};
        assert_eq!(ResourceBudget::interactive_admission(1,false,false,nominal).reason,
            InteractiveAdmissionReason::SingleProcessor);
        let high=HostPressure{available_ram_mib:Some(8192),cpu_busy_percent:Some(86)};
        let busy=ResourceBudget::interactive_admission(16,true,false,high);
        assert_eq!(busy.reason,InteractiveAdmissionReason::CpuPressure);
        assert!(busy.defer_new_background);
        let low=HostPressure{available_ram_mib:Some(1200),cpu_busy_percent:Some(20)};
        let pressure=ResourceBudget::interactive_admission(16,true,false,low);
        assert_eq!(pressure.reason,InteractiveAdmissionReason::MemoryPressure);
        assert!(pressure.defer_new_background);
        assert_eq!(pressure.budget.gpu,0);
        assert_eq!(pressure.budget.vram_mib,0);
        assert_eq!(pressure.budget.ram_mib,176); // 1200 MiB - 1024 MiB reserve
    }
    #[test]
    fn foreground_gate_holds_due_workers_without_revoking_reviewed_approval(){
        let (_,_,queue)=test_scheduler();
        let now=now_unix();
        queue.enqueue_host_plan(plan("held-for-human"),
            ScheduleOptions::file_scan(now,None,Priority::Interactive)).unwrap();
        assert_eq!(queue.tick_at_guarded(now,true).unwrap(),0);
        assert_eq!(queue.state_for("held-for-human","c-owner").unwrap().as_deref(),
            Some("QUEUED"));
        assert_eq!(queue.guardian_snapshot("c-owner").unwrap().used.workers,0);
        // After foreground clears, original admission still applies.
        // Our intentionally invalid test hash is never executed by the OS.
        assert_eq!(queue.tick_at_guarded(now,false).unwrap(),0);
        assert_eq!(queue.state_for("held-for-human","c-owner").unwrap().as_deref(),
            Some("FAILED"));
    }
    #[test]
    fn perf01_p4_guardian_hold_blocks_new_lease_but_preserves_existing_custody(){
        use crate::native_resource_pressure::HostPressure;
        let (_,_,queue)=test_scheduler();
        let queue=Arc::new(queue);
        let claim=ResourceClaim::file_scan();
        let already=queue.admit_host_worker("c-owner","existing-lease",claim).unwrap();
        assert_eq!(queue.guardian_snapshot("c-owner").unwrap().used.workers,1);
        let pressure=HostPressure{
            available_ram_mib:Some(8192),cpu_busy_percent:Some(20)
        };
        let blocked=ResourceBudget::interactive_admission(8,true,true,pressure);
        queue.apply_interactive_admission(blocked).unwrap();
        assert_eq!(queue.admit_host_worker("c-owner","new-lease",claim)
            .err(),Some(SchedulerError::ResourceRejected));
        // Holding new admissions does not revoke prior OS process custody.
        assert_eq!(queue.guardian_snapshot("c-owner").unwrap().used.workers,1);
        drop(already);
        assert_eq!(queue.guardian_snapshot("c-owner").unwrap().used.workers,0);
        let open=ResourceBudget::interactive_admission(8,true,false,pressure);
        queue.apply_interactive_admission(open).unwrap();
        assert!(!open.defer_new_background);
        assert!(queue.admit_host_worker("c-owner","later-lease",claim).is_ok());
    }
    #[test]
    fn perf01_p4_second_probe_stops_native_starts_after_queue_scan(){
        use std::sync::atomic::{AtomicUsize,Ordering};
        let (_,_,scheduler)=test_scheduler();
        let now=now_unix();
        scheduler.enqueue_host_plan(plan("p4-queued"),
            ScheduleOptions::file_scan(now,None,Priority::Interactive)).unwrap();
        let probes=AtomicUsize::new(0);
        let started=scheduler.tick_at_guarded_with_probe(now,false,||{
            probes.fetch_add(1,Ordering::SeqCst);
            true
        }).unwrap();
        assert_eq!(started,0);
        assert!(probes.load(Ordering::SeqCst)>=1);
        assert_eq!(scheduler.state_for("p4-queued","c-owner").unwrap().as_deref(),
            Some("QUEUED"));
        assert_eq!(scheduler.guardian_snapshot("c-owner").unwrap().used.workers,0);
    }
    #[test]
    fn priority_and_resource_admission_reject_gpu_without_verified_budget(){
        let (_,_,queue)=test_scheduler();
        let mut opts=ScheduleOptions::file_scan(now_unix()+60,None,Priority::Normal);
        opts.claim.gpu=1;opts.claim.vram_mib=128;
        assert_eq!(queue.enqueue_host_plan(plan("scan-02"),opts),
            Err(SchedulerError::ResourceRejected));
        assert_eq!(queue.guardian_snapshot("c-owner").unwrap().used,
            ResourceClaim::default());
    }
    #[test]
    fn queued_pause_resume_and_cancel_need_exact_workspace(){
        let (_,_,queue)=test_scheduler();
        let opts=ScheduleOptions::file_scan(now_unix()+90,None,Priority::Normal);
        queue.enqueue_host_plan(plan("scan-03"),opts).unwrap();
        assert!(queue.pause_queued("scan-03","c-other").is_err());
        queue.pause_queued("scan-03","c-owner").unwrap();
        assert_eq!(queue.list_owned("c-owner",5).unwrap()[0].state,"PAUSED");
        queue.resume_queued("scan-03","c-owner").unwrap();
        queue.cancel_owned("scan-03","c-owner").unwrap();
        assert_eq!(queue.list_owned("c-owner",5).unwrap()[0].state,"CANCELLED");
    }
    #[test]
    fn missing_runtime_plan_after_restart_requires_review(){
        let (_,_,queue)=test_scheduler();
        let now=now_unix();
        queue.enqueue_host_plan(plan("scan-04"),
            ScheduleOptions::file_scan(now+30,Some(86400),Priority::Background)).unwrap();
        // In-memory SQLite restart is emulated by dropping plan custody and
        // making the queued row due; the scheduler refuses to launch it.
        let mut g=queue.inner.lock().unwrap();
        g.plans.clear();
        g.db.execute("UPDATE lu_schedule SET due_unix=?1 WHERE task_id=?2",
            params![to_sql(now).unwrap(),"scan-04"]).unwrap();
        drop(g);
        queue.tick_at(now).unwrap();
        assert_eq!(queue.list_owned("c-owner",5).unwrap()[0].state,"NEEDS_REVIEW");
    }
    #[test]
    fn reviewed_worker_runs_through_real_os_supervisor_and_persists_completion(){
        use sha2::{Digest,Sha256};
        use std::{path::PathBuf,fs,thread,time::Duration};
        #[cfg(windows)]
        let executable=PathBuf::from(std::env::var_os("ComSpec")
            .unwrap_or_else(||std::ffi::OsString::from(
                r"C:\Windows\System32\cmd.exe")));
        #[cfg(unix)]
        let executable=PathBuf::from("/bin/true");
        // Symlink executable paths are correctly denied by the worker
        // admission check. Resolve the canonical binary in this trusted
        // fixture instead of relaxing the actual containment boundary.
        let executable=executable.canonicalize().expect("real OS fixture missing");
        assert!(executable.is_file(),"reviewed real OS worker fixture missing");
        let hash=format!("{:x}",Sha256::digest(fs::read(&executable).unwrap()));
        #[cfg(windows)]
        let args=vec!["/D".into(),"/C".into(),"exit /b 0".into()];
        #[cfg(unix)]
        let args=vec![];
        let (store,_,queue)=test_scheduler();
        let now=now_unix();
        queue.enqueue_host_plan(TrustedWorkerPlan{
            task_id:"scan-live".into(),workspace_id:"c-owner".into(),
            executable,expected_sha256:hash,args,
            progress_protocol:false,max_runtime_seconds:Some(10),
        },ScheduleOptions::file_scan(now,None,Priority::Normal)).unwrap();
        assert_eq!(queue.tick_at(now).unwrap(),1);
        let mut done=false;
        for _ in 0..100{
            thread::sleep(Duration::from_millis(10));
            queue.tick_at(now_unix()).unwrap();
            if queue.state_for("scan-live","c-owner").unwrap().as_deref()
                ==Some("COMPLETED"){done=true;break}
        }
        assert!(done,"reviewed worker should produce a real terminal receipt");
        assert_eq!(store.get("scan-live").unwrap().unwrap().state,TaskState::Completed);
        assert_eq!(queue.guardian_snapshot("c-owner").unwrap().used.workers,0);
    }
    #[test]
    fn guardian_external_lease_obeys_budget_and_releases_on_drop(){
        let (_,_,queue)=test_scheduler();
        let queue=Arc::new(queue);
        let first=queue.admit_host_worker("c-owner","ocr-1",
            ResourceClaim::file_scan()).unwrap();
        assert_eq!(queue.guardian_snapshot("c-owner").unwrap().used.workers,1);
        assert!(queue.admit_host_worker("c-owner","ocr-2",
            ResourceClaim::file_scan()).is_err());
        drop(first);
        assert_eq!(queue.guardian_snapshot("c-owner").unwrap().used.workers,0);
        let second=queue.admit_host_worker("c-owner","ocr-2",
            ResourceClaim::file_scan()).unwrap();
        drop(second);
    }
    #[test]
    fn a_single_cpu_host_cannot_start_background_workers(){
        let budget=ResourceBudget::for_parallelism(1);
        assert_eq!(budget.ram_mib,1);
        assert!(!budget.supports(ResourceClaim::default(),ResourceClaim::file_scan()));
        assert!(!budget.supports(ResourceClaim::default(),ResourceClaim::ocr()));
        let dual=ResourceBudget::for_parallelism(2);
        assert!(dual.supports(ResourceClaim::default(),ResourceClaim::file_scan()));
    }
    #[test]
    fn physical_ram_and_cpu_pressure_reduce_new_admission_without_fake_vram(){
        use crate::native_resource_pressure::HostPressure;
        let base=ResourceBudget::interactive_first();
        assert_eq!(base.under_host_pressure(HostPressure{
            available_ram_mib:Some(512),cpu_busy_percent:Some(95)
        }).ram_mib,1);
        assert_eq!(base.under_host_pressure(HostPressure{
            available_ram_mib:None,cpu_busy_percent:None
        }).ram_mib,1);
        let normal=base.under_host_pressure(HostPressure{
            available_ram_mib:Some(8192),cpu_busy_percent:Some(10)
        });
        assert_eq!(normal.gpu,0);
        assert!(normal.ram_mib>=512);
    }
    #[test]
    fn reviewed_schedule_requires_fresh_plan_and_is_rebound_once(){
        let (_,_,queue)=test_scheduler();
        let old="scan-original";
        queue.enqueue_host_plan(plan(old),
            ScheduleOptions::file_scan(now_unix()+240,
                Some(3600),Priority::Normal)).unwrap();
        {
            let mut g=queue.inner.lock().unwrap();
            g.plans.clear();
            g.db.execute("UPDATE lu_schedule SET state='NEEDS_REVIEW'
                WHERE task_id=?1",[old]).unwrap();
        }
        let mut wrong=plan("scan-second");
        wrong.workspace_id="c-other".into();
        wrong.args=vec!["--lu-file-scan-worker".into(),"source".into()];
        assert!(queue.reapprove_reviewed(old,wrong).is_err());
        let mut fresh=plan("scan-second");
        fresh.args=vec!["--lu-file-scan-worker".into(),"new-picked-file".into()];
        queue.reapprove_reviewed(old,fresh).unwrap();
        assert_eq!(queue.state_for(old,"c-owner").unwrap().as_deref(),
            Some("CANCELLED"));
        let rows=queue.list_owned("c-owner",10).unwrap();
        let new=rows.iter().find(|r|r.task_id=="scan-second").unwrap();
        assert_eq!(new.state,"QUEUED");
        assert_eq!(new.repeat_seconds,Some(3600));
        let mut duplicate=plan("scan-third");
        duplicate.args=vec!["--lu-file-scan-worker".into(),"new-picked-file".into()];
        assert!(queue.reapprove_reviewed(old,duplicate).is_err());
    }
    #[test]
    fn dependency_is_owner_scoped_and_unmet_jobs_do_not_dispatch(){
        let (_,_,queue)=test_scheduler();
        let now=now_unix();
        queue.enqueue_host_plan(plan("scan-first"),
            ScheduleOptions::file_scan(now+300,None,Priority::Normal)).unwrap();
        let mut opts=ScheduleOptions::file_scan(now+300,None,Priority::Normal);
        opts.dependency=Some("scan-first".into());
        queue.enqueue_host_plan(plan("scan-second"),opts).unwrap();
        assert_eq!(queue.list_owned("c-owner",8).unwrap().len(),2);
        let mut bad=ScheduleOptions::file_scan(now+300,None,Priority::Normal);
        bad.dependency=Some("made-up".into());
        assert_eq!(queue.enqueue_host_plan(plan("scan-third"),bad),
            Err(SchedulerError::WrongOwner));
    }

    /// Real compiled scanner, installed finite root broker, original
    /// per-task conversation binding, scheduler start and natural
    /// completion, plus interruption from a running scan. No mocks.
    #[cfg(target_os="linux")]
    #[test]
    #[ignore="requires disposable nonroot systemd host and installed reviewed scanner"]
    fn cp06er2_root_owned_scheduler_complete_and_stop(){
        use sha2::{Digest,Sha256};
        let nonce=format!("{}-{}",std::process::id(),now_unix());
        let root=std::env::temp_dir().join(format!("lu-reviewed-native-{nonce}"));
        std::fs::create_dir(&root).unwrap();
        let root=root.canonicalize().unwrap();
        let exe=std::path::PathBuf::from("/usr/libexec/lu/lu-file-scan-worker-acceptance");
        let engine_sha=format!("{:x}",Sha256::digest(std::fs::read(&exe).unwrap()));
        let small=root.join("small.bin");std::fs::write(&small,b"actual reviewed native bytes").unwrap();
        let large=root.join("large.bin");let large_handle=std::fs::File::create(&large).unwrap();
        large_handle.set_len(4*1024*1024*1024).unwrap();drop(large_handle);
        let store=Arc::new(TaskStore::open(&root.join("tasks.sqlite")).unwrap());
        let process=Arc::new(BackgroundProcessSupervisor::new(Arc::clone(&store)));
        let budget=ResourceBudget{workers:1,cpu:1,ram_mib:1024,gpu:0,vram_mib:0,disk:1,network:0};
        let db=root.join("schedule.sqlite");
        let q=BackgroundScheduler::open(&db,Arc::clone(&store),Arc::clone(&process),budget).unwrap();
        let chat=format!("c-{}","a".repeat(32));let turn=format!("g-{}","b".repeat(32));
        let done=format!("scan-{}","c".repeat(32));let stop=format!("scan-{}","d".repeat(32));
        for (id,input) in [(&done,&small),(&stop,&large)]{
            let mut reviewed=ReviewedScan::prepare(id,&chat,&turn,input).unwrap();
            // Test-only first-party scanner executable is separately built
            // from the production source and installed root-owned.
            reviewed.plan.executable=exe.clone();reviewed.plan.expected_sha256=engine_sha.clone();
            q.enqueue_reviewed_scan(reviewed).unwrap();
            assert_eq!(q.tick_foreground_guarded(true).unwrap(),0);
            assert!(store.get(id).unwrap().is_none());
            assert_eq!(q.tick().unwrap(),1);
            assert!(process.can_cancel_protected(id,&chat));
            let deadline=std::time::Instant::now()+std::time::Duration::from_secs(80);
            if id==&done{
                loop{
                    q.tick().unwrap();
                    if q.state_for(id,&chat).unwrap().as_deref()==Some("COMPLETED"){break}
                    assert!(std::time::Instant::now()<deadline,"completion not witnessed");
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                let result=store.verified_result(id,&chat).unwrap().unwrap();
                assert_eq!(result.sha256,format!("{:x}",Sha256::digest(b"actual reviewed native bytes")));
                assert!(process.cancel_owned_protected(id,&chat).is_err());
            }else{
                // Wait for real progress (not only a client's spawned PID).
                loop{
                    let row=store.get(id).unwrap().unwrap();
                    if row.progress>0&&row.state==TaskState::Running{break}
                    assert!(row.state==TaskState::Running&&std::time::Instant::now()<deadline,"real scanner progress not witnessed");
                    std::thread::sleep(std::time::Duration::from_millis(2));
                }
                assert!(process.cancel_owned_protected(id,"c-other").is_err());
                process.cancel_owned_protected(id,&chat).unwrap();
                q.tick().unwrap();
                assert_eq!(q.state_for(id,&chat).unwrap().as_deref(),Some("CANCELLED"));
                assert!(store.verified_result(id,&chat).unwrap().is_none());
            }
            assert!(!process.can_cancel(id));assert!(store.process_uncertainty(id).unwrap().is_none());
        }
        drop(q);
        let reopened=BackgroundScheduler::open(&db,Arc::clone(&store),Arc::clone(&process),budget).unwrap();
        assert_eq!(reopened.state_for(&done,&chat).unwrap().as_deref(),Some("COMPLETED"));
        assert_eq!(reopened.state_for(&stop,&chat).unwrap().as_deref(),Some("CANCELLED"));
        assert_eq!(reopened.tick().unwrap(),0);
        println!("CP06ER2_ROOT_OWNED_SCHEDULER_PASS");
        drop(reopened);drop(process);drop(store);std::fs::remove_dir_all(root).unwrap();
    }

}

#[cfg(all(test,windows))]
#[test]
fn reviewed_scan_queue_holds_original_file_until_cancel(){
    let root=std::env::temp_dir().join(format!("lu-reviewed-scan-{}-{}",std::process::id(),now_unix()));
    std::fs::create_dir(&root).unwrap();
    let file=root.join("reviewed.bin");std::fs::write(&file,b"approved bytes").unwrap();
    let store=Arc::new(TaskStore::open(Path::new(":memory:")).unwrap());
    let process=Arc::new(BackgroundProcessSupervisor::new(Arc::clone(&store)));
    let budget=ResourceBudget{workers:1,cpu:1,ram_mib:1024,gpu:0,vram_mib:0,disk:1,network:0};
    let scheduler=BackgroundScheduler::open(Path::new(":memory:"),store,process,budget).unwrap();
    let plan=ReviewedScan::prepare("scan-reviewed","c-owner","g-turn",&file).unwrap();
    assert_eq!(plan.bytes(),14);
    scheduler.enqueue_reviewed_scan(plan).unwrap();
    assert_eq!(scheduler.state_for("scan-reviewed","c-owner").unwrap().as_deref(),Some("QUEUED"));
    assert_eq!(scheduler.state_for("scan-reviewed","c-other").unwrap(),None);
    assert!(std::fs::write(&file,b"substitution").is_err());
    assert!(scheduler.cancel_owned("scan-reviewed","c-other").is_err());
    scheduler.cancel_owned("scan-reviewed","c-owner").unwrap();
    assert_eq!(scheduler.state_for("scan-reviewed","c-owner").unwrap().as_deref(),Some("CANCELLED"));
    std::fs::write(&file,b"after cancellation").unwrap();
    drop(scheduler);std::fs::remove_dir_all(root).unwrap();
}

#[cfg(all(test,any(windows,target_os="linux")))]
#[test]
fn reviewed_unstarted_cancel_is_owner_state_and_restart_bound(){
    let root=std::env::temp_dir().join(format!("lu-reviewed-unstarted-{}-{}",std::process::id(),now_unix()));
    std::fs::create_dir(&root).unwrap();
    let file=root.join("reviewed.bin");std::fs::write(&file,b"approved bytes").unwrap();
    let file=file.canonicalize().unwrap();
    let store=Arc::new(TaskStore::open(&root.join("tasks.sqlite")).unwrap());
    let process=Arc::new(BackgroundProcessSupervisor::new(Arc::clone(&store)));
    let budget=ResourceBudget{workers:1,cpu:1,ram_mib:1024,gpu:0,vram_mib:0,disk:1,network:0};
    let db=root.join("schedule.sqlite");
    let chat=format!("c-{}","a".repeat(32));let turn=format!("g-{}","b".repeat(32));
    let id=format!("scan-{}","c".repeat(32));let id2=format!("scan-{}","d".repeat(32));
    let q=BackgroundScheduler::open(&db,Arc::clone(&store),Arc::clone(&process),budget).unwrap();
    q.enqueue_reviewed_scan(ReviewedScan::prepare(&id,&chat,&turn,&file).unwrap()).unwrap();
    assert_eq!(q.reviewed_unstarted_state(&id,&chat).unwrap().as_deref(),Some("QUEUED"));
    assert!(q.cancel_reviewed_unstarted(&id,"c-other","QUEUED").is_err());
    q.pause_queued(&id,&chat).unwrap();
    assert!(q.cancel_reviewed_unstarted(&id,&chat,"QUEUED").is_err());
    assert_eq!(q.reviewed_unstarted_state(&id,&chat).unwrap().as_deref(),Some("PAUSED"));
    q.cancel_reviewed_unstarted(&id,&chat,"PAUSED").unwrap();
    assert!(q.cancel_reviewed_unstarted(&id,&chat,"PAUSED").is_err());
    assert_eq!(q.state_for(&id,&chat).unwrap().as_deref(),Some("CANCELLED"));
    assert!(store.get(&id).unwrap().is_none());
    assert!(!process.can_cancel(&id));
    assert_eq!(q.tick().unwrap(),0);
    q.enqueue_reviewed_scan(ReviewedScan::prepare(&id2,&chat,&turn,&file).unwrap()).unwrap();
    drop(q);
    let reopened=BackgroundScheduler::open(&db,Arc::clone(&store),Arc::clone(&process),budget).unwrap();
    assert_eq!(reopened.state_for(&id2,&chat).unwrap().as_deref(),Some("NEEDS_REVIEW"));
    assert!(reopened.reviewed_unstarted_state(&id2,&chat).unwrap().is_none());
    assert!(reopened.cancel_reviewed_unstarted(&id2,&chat,"QUEUED").is_err());
    assert_eq!(reopened.tick().unwrap(),0);
    assert!(store.get(&id2).unwrap().is_none());
    drop(reopened);drop(process);drop(store);std::fs::remove_dir_all(root).unwrap();
}
