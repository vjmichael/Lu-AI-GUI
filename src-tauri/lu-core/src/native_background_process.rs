//! Host-owned native worker process lifecycle for TOOLS-11.
//! This is deliberately NOT a model-facing "run executable" tool. The Lu host
//! chooses a reviewed executable digest, argv, workspace and task identity;
//! untrusted model output never becomes a command, PID or process-kill target.
use std::{
    collections::BTreeMap,ffi::OsString,fs,io::{self,Read,BufReader},path::{Path,PathBuf},
    process::{Child,Command,Stdio},sync::{Arc,Mutex},
    time::{Duration,Instant},
};
use sha2::{Digest,Sha256};
use crate::tasks::{TaskState,TaskStore};

#[derive(Debug,Clone,PartialEq,Eq)]
pub enum BackgroundProcessError{
    InvalidPlan,AlreadyOwned,NotOwned,WrongWorkspace,SpawnFailed,
    ProcessUncertain,StoreUnavailable,Poisoned,
}
pub struct TrustedWorkerPlan {
    pub task_id:String,
    pub workspace_id:String,
    pub executable:PathBuf,
    pub expected_sha256:String,
    pub args:Vec<OsString>,
    /// Only reviewed workers with an authenticated progress contract may
    /// request stdout IPC; arbitrary subprocess output is discarded.
    pub progress_protocol:bool,
    /// Explicit host deadline, not a model generation or heartbeat timeout.
    /// None means there is no overall task deadline.
    pub max_runtime_seconds:Option<u64>,
}
struct OwnedWorker {
    workspace_id:String,
    child:Child,
    progress_reader:Option<std::thread::JoinHandle<bool>>,
    deadline:Option<Instant>,
    #[cfg(windows)]
    job:Option<lu_win_worker_job::Job>,
}
pub struct BackgroundProcessSupervisor {
    store:Arc<TaskStore>,
    owned:Mutex<BTreeMap<String,OwnedWorker>>,
}
fn valid_id(s:&str)->bool{
    !s.is_empty()&&s.len()<=128
       &&s.bytes().all(|b|b.is_ascii_alphanumeric()||matches!(b,b'-'|b'_'))
}
fn verified_executable(path:&Path,digest:&str)->Result<PathBuf,BackgroundProcessError>{
    if !path.is_absolute()||digest.len()!=64||
       !digest.bytes().all(|b|b.is_ascii_hexdigit()){
        return Err(BackgroundProcessError::InvalidPlan);
    }
    let meta=fs::symlink_metadata(path).map_err(|_|BackgroundProcessError::InvalidPlan)?;
    if !meta.file_type().is_file()||meta.len()==0{
        return Err(BackgroundProcessError::InvalidPlan);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes()&0x400!=0{
            return Err(BackgroundProcessError::InvalidPlan);
        }
    }
    let canonical=path.canonicalize().map_err(|_|BackgroundProcessError::InvalidPlan)?;
    let mut file=fs::File::open(&canonical).map_err(|_|BackgroundProcessError::InvalidPlan)?;
    let mut sha=Sha256::new();
    let mut buf=[0u8;65536];
    loop{
        let n=file.read(&mut buf).map_err(|_|BackgroundProcessError::InvalidPlan)?;
        if n==0{break;}
        sha.update(&buf[..n]);
    }
    if format!("{:x}",sha.finalize())!=digest.to_ascii_lowercase(){
        return Err(BackgroundProcessError::InvalidPlan);
    }
    Ok(canonical)
}
/// Worker progress stream is a deliberately tiny, bounded native protocol.
/// The caller has already pinned the executable hash, so this is a receipt,
/// never an authorization grant or a model-provided status claim.
fn read_worker_progress(pipe:impl Read,store:&TaskStore,task_id:&str)->bool{
    let mut input=BufReader::new(pipe);
    let mut line=Vec::with_capacity(96);
    let mut final_result=false;
    let mut total_seen=None::<u64>;
    let mut last_bytes=0u64;
    loop{
        line.clear();
        // Limit each line before allocating an attacker-sized buffer.
        for _ in 0..128{
            let mut byte=[0u8;1];
            match input.read(&mut byte){
                Ok(0)=>return final_result,
                Err(_)=>return false,
                Ok(_)=>{
                    if byte[0]==b'\n'{break;}
                    line.push(byte[0]);
                }
            }
        }
        if line.len()>=128{return false;}
        let Ok(text)=std::str::from_utf8(&line) else{return false};
        if let Some(value)=text.strip_prefix("P\t"){
            if final_result{return false;}
            let Some((read,total))=value.split_once('\t') else{return false};
            let (Ok(read),Ok(total))=(read.parse::<u64>(),total.parse::<u64>())
                else{return false};
            if total==0||read>total||read<last_bytes
                ||total_seen.is_some_and(|seen|seen!=total){return false;}
            last_bytes=read;
            total_seen=Some(total);
            let percent=((u128::from(read)*99)/u128::from(total)) as u8;
            if store.checkpoint(task_id,percent,
                &format!("Verified {} of {} bytes",read,total),&[]).is_err(){
                return false;
            }
        }else if let Some(digest)=text.strip_prefix("D\t"){
            if final_result||total_seen!=Some(last_bytes)
                ||digest.len()!=64
                ||!digest.bytes().all(|b|b.is_ascii_hexdigit()){
                return false;
            }
            if store.checkpoint(task_id,100,&format!("SHA-256: {digest}"),&[])
                .is_err(){return false;}
            final_result=true;
        }else{return false;}
    }
}

impl BackgroundProcessSupervisor {
    pub fn new(store:Arc<TaskStore>)->Self{
        Self{store,owned:Mutex::new(BTreeMap::new())}
    }
    /// Host-only admission. The caller must obtain the user's task grant
    /// independently and build this plan from a trusted executable catalog.
    /// No Tauri command or ModelToolCatalog invokes this API directly.
    pub fn start_host_approved(&self,plan:TrustedWorkerPlan)
        ->Result<(),BackgroundProcessError>{
        if !valid_id(&plan.task_id)||!valid_id(&plan.workspace_id)
          ||plan.max_runtime_seconds.is_some_and(|n|n==0||n>86_400)
          ||plan.args.len()>64||plan.args.iter().any(|arg|
               arg.to_string_lossy().len()>4096
               ||arg.to_string_lossy().contains('\0')){
            return Err(BackgroundProcessError::InvalidPlan);
        }
        let executable=verified_executable(&plan.executable,&plan.expected_sha256)?;
        let mut workers=self.owned.lock().map_err(|_|BackgroundProcessError::Poisoned)?;
        if workers.contains_key(&plan.task_id){
            return Err(BackgroundProcessError::AlreadyOwned);
        }
        self.store.create(&plan.task_id,Some(&plan.workspace_id),None)
            .map_err(|_|BackgroundProcessError::StoreUnavailable)?;
        let mut command=Command::new(&executable);
        command.args(&plan.args).env_clear().stdin(Stdio::null())
            .stdout(if plan.progress_protocol{Stdio::piped()}else{Stdio::null()})
            .stderr(Stdio::null())
            .current_dir(executable.parent().ok_or(BackgroundProcessError::InvalidPlan)?);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000 | 0x0000_0004);
        }
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        #[cfg(windows)]
        let job=lu_win_worker_job::Job::new().map_err(|_|{
            let _=self.store.fail_pending(&plan.task_id);
            BackgroundProcessError::SpawnFailed
        })?;
        #[cfg(windows)]
        if plan.args.first().and_then(|s|s.to_str())
            .is_some_and(|mode|matches!(mode,
                "--lu-archive-extract-worker"|"--lu-archive-preview-worker"))
            &&job.set_memory_limit(1024*1024*1024).is_err(){
            let _=self.store.fail_pending(&plan.task_id);
            return Err(BackgroundProcessError::SpawnFailed)
        }
        let mut child=command.spawn().map_err(|_|{
            let _=self.store.fail_pending(&plan.task_id);
            BackgroundProcessError::SpawnFailed
        })?;
        #[cfg(windows)]
        if job.assign_and_resume_suspended(&child).is_err(){
            let _=child.kill();
            let _=child.wait();
            let _=self.store.fail_pending(&plan.task_id);
            return Err(BackgroundProcessError::SpawnFailed);
        }
        // Delay progress consumption until the SQLite RUNNING transition.
        // A fast worker must not lose its first checkpoint to a startup race.
        let progress_pipe=if plan.progress_protocol{child.stdout.take()}else{None};
        let owner=OwnedWorker{
            workspace_id:plan.workspace_id,
            child,
            progress_reader:None,
            deadline:plan.max_runtime_seconds.map(|n|Instant::now()+Duration::from_secs(n)),
            #[cfg(windows)]
            job:Some(job),
        };
        workers.insert(plan.task_id.clone(),owner);
        if self.store.start(&plan.task_id).is_err(){
            let mut owner=workers.remove(&plan.task_id).expect("new worker registered");
            let _=terminate_owned(&mut owner);
            let _=self.store.fail_pending(&plan.task_id);
            return Err(BackgroundProcessError::StoreUnavailable);
        }
        if let Some(pipe)=progress_pipe{
            let store=Arc::clone(&self.store);
            let task_id=plan.task_id.clone();
            let reader=std::thread::spawn(move||
                read_worker_progress(pipe,&store,&task_id)
            );
            workers.get_mut(&plan.task_id)
                .expect("just registered running worker")
                .progress_reader=Some(reader);
        }
        Ok(())
    }
    /// Non-blocking native host observation. No model tokens are consumed.
    pub fn poll(&self)->Result<usize,BackgroundProcessError>{
        let mut workers=self.owned.lock().map_err(|_|BackgroundProcessError::Poisoned)?;
        let mut finished=Vec::new();
        for (id,worker) in workers.iter_mut(){
            match worker.child.try_wait(){
                Ok(Some(status))=>finished.push((id.clone(),Some(status.success()))),
                Ok(None)=>{
                    if worker.deadline.is_some_and(|deadline|Instant::now()>=deadline){
                        // Host deadline, never a model decision or an inferred
                        // lost heartbeat. Kill and reap before publishing.
                        terminate_owned(worker)?;
                        finished.push((id.clone(),None));
                    }
                },
                Err(_)=>return Err(BackgroundProcessError::ProcessUncertain),
            }
        }
        let count=finished.len();
        for (id,outcome) in finished{
            let mut owner=workers.remove(&id).expect("owned worker still registered");
            #[cfg(windows)]
            drop(owner.job.take()); // Close Job and any inherited stdout handles.
            let proof=owner.progress_reader.take().map(|handle|handle.join().unwrap_or(false));
            drop(owner); // Release Windows Job handle before terminal receipt.
            let result=match outcome{
                Some(true) if proof.unwrap_or(true)=>self.store.complete(&id),
                Some(_)=>self.store.fail(&id),
                None=>self.store.timed_out(&id),
            };
            result.map_err(|_|BackgroundProcessError::StoreUnavailable)?;
        }
        Ok(count)
    }
    /// A cancel never targets a model-selected PID. It only affects a worker
    /// which remains owned by this manager and matches the host workspace.
    pub fn cancel_owned(&self,task_id:&str,workspace_id:&str)
        ->Result<(),BackgroundProcessError>{
        let mut workers=self.owned.lock().map_err(|_|BackgroundProcessError::Poisoned)?;
        let worker=workers.get_mut(task_id).ok_or(BackgroundProcessError::NotOwned)?;
        if worker.workspace_id!=workspace_id{
            return Err(BackgroundProcessError::WrongWorkspace);
        }
        terminate_owned(worker)?;
        let mut owner=workers.remove(task_id).expect("live worker owned");
        if let Some(reader)=owner.progress_reader.take(){let _=reader.join();}
        drop(owner); // Release the Windows Job handle (kills descendants).
        self.store.cancelled(task_id)
            .map_err(|_|BackgroundProcessError::StoreUnavailable)
    }
    /// Local operator action: derive ownership from the host's active map,
    /// never from a model/WebView-supplied workspace or process identifier.
    pub fn cancel_operator_owned(&self,task_id:&str)
        ->Result<(),BackgroundProcessError>{
        let workspace={
            let workers=self.owned.lock().map_err(|_|BackgroundProcessError::Poisoned)?;
            workers.get(task_id).ok_or(BackgroundProcessError::NotOwned)?
                .workspace_id.clone()
        };
        self.cancel_owned(task_id,&workspace)
    }
    pub fn can_cancel(&self,task_id:&str)->bool{
        self.owned.lock().is_ok_and(|owned|owned.contains_key(task_id))
    }
}
fn terminate_owned(worker:&mut OwnedWorker)->Result<(),BackgroundProcessError>{
    #[cfg(unix)]
    {
        // Entire host-created process group, not a model supplied PID.
        if lu_win_worker_job::unix_owned::kill_owned_group(&worker.child).is_err(){
            // If the original child is already dead, a direct kill is harmless,
            // but cannot prove descendants were reaped. Do not report success.
            return Err(BackgroundProcessError::ProcessUncertain);
        }
    }
    #[cfg(windows)]
    {
        // Closing the Job first terminates all descendants, including those
        // inheriting our stdout pipe. Only then can reader.join() be safe.
        drop(worker.job.take());
    }
    worker.child.wait().map_err(|_|BackgroundProcessError::ProcessUncertain)?;
    Ok(())
}
impl Drop for BackgroundProcessSupervisor{
    fn drop(&mut self){
        if let Ok(mut workers)=self.owned.lock(){
            for (_,mut worker) in std::mem::take(&mut *workers){
                let _=terminate_owned(&mut worker);
                if let Some(reader)=worker.progress_reader.take(){let _=reader.join();}
            }
        }
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn genuine_scanner_receipts_update_sqlite_without_model(){
        use std::io::Cursor;
        let store=TaskStore::open(Path::new(":memory:")).unwrap();
        store.create("media123",Some("media"),None).unwrap();
        store.start("media123").unwrap();
        let digest="a".repeat(64);
        let frames=format!("P\t0\t20\nP\t10\t20\nP\t20\t20\nD\t{digest}\n");
        assert!(read_worker_progress(Cursor::new(frames.into_bytes()),&store,"media123"));
        let record=store.get("media123").unwrap().unwrap();
        assert_eq!(record.progress,100);
        assert_eq!(record.detail,format!("SHA-256: {digest}"));
        assert!(store.events_after("media123","other",0,12).unwrap().is_empty());
        let events=store.events_after("media123","media",0,12).unwrap();
        // One durable START event plus four worker checkpoints.
        assert_eq!(events.len(),5);
        assert_eq!(events.last().unwrap().progress,100);
    }
    #[test]
    fn invalid_and_truncated_worker_reports_cannot_complete(){
        use std::io::Cursor;
        for frames in [
            "P\t0\t100\nP\t100\t100\n",
            "P\t100\t100\nP\t50\t100\n",
            "P\t100\t100\nD\tnot-a-sha256\n",
            "P\t2\t1\n",
            "P\t0\t0\n",
            "P\t5\t10\nD\t",
        ]{
            let store=TaskStore::open(Path::new(":memory:")).unwrap();
            store.create("t",Some("w"),None).unwrap();
            store.start("t").unwrap();
            assert!(!read_worker_progress(Cursor::new(frames.as_bytes()),&store,"t"));
            assert!(store.get("t").unwrap().unwrap().progress<100);
        }
    }
    #[test]
    fn unreviewed_executable_never_creates_a_job(){
        let store=Arc::new(TaskStore::open(Path::new(":memory:")).unwrap());
        let manager=BackgroundProcessSupervisor::new(Arc::clone(&store));
        let plan=TrustedWorkerPlan{
            task_id:"video-1".into(),workspace_id:"media".into(),
            executable:PathBuf::from("relative-untrusted-executable"),
            expected_sha256:"0".repeat(64),args:vec![],progress_protocol:false,max_runtime_seconds:None,
        };
        assert_eq!(manager.start_host_approved(plan),
            Err(BackgroundProcessError::InvalidPlan));
        assert!(store.get("video-1").unwrap().is_none());
        assert_eq!(manager.cancel_owned("video-1","media"),
            Err(BackgroundProcessError::NotOwned));
    }
    #[cfg(windows)]
    #[test]
    fn windows_owned_job_is_contained_and_reaped_on_cancellation(){
        // An ordinary Windows system binary stands in for a reviewed worker.
        // Cmd starts a child ping process, exercising the Job object tree.
        let executable=PathBuf::from(std::env::var_os("ComSpec")
            .unwrap_or_else(||OsString::from(r"C:\Windows\System32\cmd.exe")));
        if !executable.is_file(){return;}
        let mut sha=Sha256::new();
        sha.update(fs::read(&executable).unwrap());
        let digest=format!("{:x}",sha.finalize());
        let store=Arc::new(TaskStore::open(Path::new(":memory:")).unwrap());
        let manager=BackgroundProcessSupervisor::new(Arc::clone(&store));
        manager.start_host_approved(TrustedWorkerPlan{
            task_id:"win-media1".into(),workspace_id:"video".into(),
            executable,expected_sha256:digest,
            args:vec!["/D".into(),"/C".into(),
                "ping -n 20 127.0.0.1 >NUL".into()],progress_protocol:false,max_runtime_seconds:None,
        }).unwrap();
        assert!(manager.can_cancel("win-media1"));
        assert_eq!(manager.cancel_owned("win-media1","wrong"),
            Err(BackgroundProcessError::WrongWorkspace));
        manager.cancel_owned("win-media1","video").unwrap();
        assert!(!manager.can_cancel("win-media1"));
        assert_eq!(store.get("win-media1").unwrap().unwrap().state,
            TaskState::Cancelled);
    }
    #[cfg(unix)]
    #[test]
    fn actual_owned_worker_deadline_is_a_timed_out_terminal_event(){
        let executable=PathBuf::from("/bin/sleep").canonicalize().unwrap();
        let mut sha=Sha256::new();
        sha.update(fs::read(&executable).unwrap());
        let digest=format!("{:x}",sha.finalize());
        let store=Arc::new(TaskStore::open(Path::new(":memory:")).unwrap());
        let manager=BackgroundProcessSupervisor::new(Arc::clone(&store));
        manager.start_host_approved(TrustedWorkerPlan{
            task_id:"deadline1".into(),workspace_id:"c-task-chat".into(),
            executable,expected_sha256:digest,
            args:vec![OsString::from("30")],
            progress_protocol:false,max_runtime_seconds:Some(1),
        }).unwrap();
        std::thread::sleep(Duration::from_millis(1250));
        assert_eq!(manager.poll().unwrap(),1);
        assert!(!manager.can_cancel("deadline1"));
        assert_eq!(store.get("deadline1").unwrap().unwrap().state,TaskState::TimedOut);
        let notices=store.pending_notices_for_workspace("c-task-chat",8).unwrap();
        assert_eq!(notices.len(),1);
        assert_eq!(notices[0].state,TaskState::TimedOut);
    }
    #[cfg(unix)]
    #[test]
    fn real_supervised_worker_can_be_cancelled_without_touching_other_job(){
        let sleep=PathBuf::from("/bin/sleep");
        if !sleep.is_file(){return;}
        // Distributions may expose /bin/sleep through a symlink chain.
        // The admission rule rightly refuses symlink executable paths;
        // use the actual canonical target in this trusted test fixture.
        let sleep=sleep.canonicalize().unwrap();
        let mut sha=Sha256::new();
        sha.update(fs::read(&sleep).unwrap());
        let digest=format!("{:x}",sha.finalize());
        let store=Arc::new(TaskStore::open(Path::new(":memory:")).unwrap());
        let manager=BackgroundProcessSupervisor::new(Arc::clone(&store));
        manager.start_host_approved(TrustedWorkerPlan{
            task_id:"worker-1".into(),workspace_id:"media".into(),
            executable:sleep,expected_sha256:digest,
            args:vec![OsString::from("30")],progress_protocol:false,max_runtime_seconds:None,
        }).unwrap();
        assert_eq!(manager.cancel_owned("worker-1","unrelated"),
            Err(BackgroundProcessError::WrongWorkspace));
        assert!(manager.can_cancel("worker-1"));
        manager.cancel_owned("worker-1","media").unwrap();
        assert!(!manager.can_cancel("worker-1"));
        assert_eq!(store.get("worker-1").unwrap().unwrap().state,TaskState::Cancelled);
        assert_eq!(store.reconcile_interrupted().unwrap(),0);
    }
}
