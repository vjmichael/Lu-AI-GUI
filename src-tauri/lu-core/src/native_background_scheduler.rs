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
impl ResourceBudget {
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
struct Active{
    plan:TrustedWorkerPlan,
    opts:ScheduleOptions,
    series_id:String,
    sequence_no:u32,
    retry_count:u8,
}
struct Inner {
    db:Connection,
    plans:BTreeMap<String,Active>,
    running:BTreeMap<String,ResourceClaim>,
    external:BTreeMap<String,(String,ResourceClaim)>,
    capacity:ResourceBudget,
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
        inner.db.execute(
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
        inner.db.execute(
            "INSERT INTO lu_schedule_events(task_id,workspace_id,event)
             VALUES(?1,?2,'QUEUED')",
            params![id,&plan.workspace_id]).map_err(db_error)?;
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
            retry_count:0,plan,opts,
        });
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
            g.db.execute(
              "UPDATE lu_schedule SET state=?2,last_result=?2
               WHERE task_id=?1 AND state='RUNNING'",
               params![&id,terminal]).map_err(db_error)?;
            g.db.execute(
              "INSERT INTO lu_schedule_events(task_id,workspace_id,event)
               VALUES(?1,?2,?3)",
               params![&id,&record.workspace_id.as_deref().unwrap_or(""),terminal])
                .map_err(db_error)?;
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
                    retry_count:next_retry,
                });
            }
        }
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
            if self.process.start_host_approved(plan).is_err(){
                g.db.execute(
                  "UPDATE lu_schedule SET state='FAILED',
                   last_result='worker_admission_failed' WHERE task_id=?1",
                   [&id]).map_err(db_error)?;
                g.plans.remove(&id);
                continue
            }
            // The host has launched an owned process. If state persistence
            // fails, terminate that exact process, then report uncertainty.
            if g.db.execute(
                "UPDATE lu_schedule SET state='RUNNING' WHERE task_id=?1
                 AND state='QUEUED'",[&id]).map_err(db_error)?!=1{
                let workspace=g.plans.get(&id)
                    .map(|v|v.plan.workspace_id.clone()).unwrap_or_default();
                let _=self.process.cancel_owned(&id,&workspace);
                return Err(SchedulerError::Database)
            }
            g.running.insert(id.clone(),claim);
            g.db.execute(
              "INSERT INTO lu_schedule_events(task_id,workspace_id,event)
               VALUES(?1,?2,'RUNNING')",
              params![&id,&g.plans[&id].plan.workspace_id]).map_err(db_error)?;
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
        if g.external.contains_key(task_id)||g.running.contains_key(task_id)
            ||g.plans.contains_key(task_id){return Err(SchedulerError::Duplicate)}
        let used=g.running.values().fold(ResourceClaim::default(),|acc,x|acc.add(*x));
        let used=g.external.values().fold(used,|acc,(_,x)|acc.add(*x));
        if !g.capacity.supports(used,claim){return Err(SchedulerError::ResourceRejected)}
        g.external.insert(task_id.into(),(workspace_id.into(),claim));
        Ok(GuardianLease{owner:Arc::clone(self),
            task_id:task_id.into(),workspace_id:workspace_id.into()})
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
        let affected=g.db.execute(
            "UPDATE lu_schedule SET state=?4 WHERE task_id=?1
             AND workspace_id=?2 AND state=?3",
            params![id,workspace,expected,to]).map_err(db_error)?;
        if affected!=1{return Err(SchedulerError::NotQueued)}
        if to=="CANCELLED"{g.plans.remove(id);}
        g.db.execute(
            "INSERT INTO lu_schedule_events(task_id,workspace_id,event)
             VALUES(?1,?2,?3)",params![id,workspace,to]).map_err(db_error)?;
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
            retry_count:0,plan:new_plan,opts,
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
}
