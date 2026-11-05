//! Host-owned native worker process lifecycle for TOOLS-11.
//! This is deliberately NOT a model-facing "run executable" tool. The Lu host
//! chooses a reviewed executable digest, argv, workspace and task identity;
//! untrusted model output never becomes a command, PID or process-kill target.
use std::{
    collections::BTreeMap,ffi::OsString,fs,io::{self,Read},path::{Path,PathBuf},
    process::{Child,Command,Stdio},sync::{Arc,Mutex},
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
}
struct OwnedWorker {
    workspace_id:String,
    child:Child,
    #[cfg(windows)]
    job:lu_win_worker_job::Job,
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
            .stdout(Stdio::null()).stderr(Stdio::null())
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
        let owner=OwnedWorker{
            workspace_id:plan.workspace_id,
            child,
            #[cfg(windows)]
            job,
        };
        workers.insert(plan.task_id.clone(),owner);
        if self.store.start(&plan.task_id).is_err(){
            let mut owner=workers.remove(&plan.task_id).expect("new worker registered");
            let _=terminate_owned(&mut owner);
            let _=self.store.fail_pending(&plan.task_id);
            return Err(BackgroundProcessError::StoreUnavailable);
        }
        Ok(())
    }
    /// Non-blocking native host observation. No model tokens are consumed.
    pub fn poll(&self)->Result<usize,BackgroundProcessError>{
        let mut workers=self.owned.lock().map_err(|_|BackgroundProcessError::Poisoned)?;
        let mut finished=Vec::new();
        for (id,worker) in workers.iter_mut(){
            match worker.child.try_wait(){
                Ok(Some(status))=>finished.push((id.clone(),status.success())),
                Ok(None)=>{},
                Err(_)=>return Err(BackgroundProcessError::ProcessUncertain),
            }
        }
        let count=finished.len();
        for (id,ok) in finished{
            let _owner=workers.remove(&id).expect("owned worker still registered");
            if ok{self.store.complete(&id)}else{self.store.fail(&id)}
                .map_err(|_|BackgroundProcessError::StoreUnavailable)?;
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
        workers.remove(task_id);
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
        // Kill-on-close of the Job handle terminates the owned process tree
        // when this owner is dropped. Direct process kill precedes reap.
        worker.child.kill().map_err(|_|BackgroundProcessError::ProcessUncertain)?;
    }
    worker.child.wait().map_err(|_|BackgroundProcessError::ProcessUncertain)?;
    Ok(())
}
impl Drop for BackgroundProcessSupervisor{
    fn drop(&mut self){
        if let Ok(mut workers)=self.owned.lock(){
            for (_,mut worker) in std::mem::take(&mut *workers){
                let _=terminate_owned(&mut worker);
            }
        }
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn unreviewed_executable_never_creates_a_job(){
        let store=Arc::new(TaskStore::open(Path::new(":memory:")).unwrap());
        let manager=BackgroundProcessSupervisor::new(Arc::clone(&store));
        let plan=TrustedWorkerPlan{
            task_id:"video-1".into(),workspace_id:"media".into(),
            executable:PathBuf::from("relative-untrusted-executable"),
            expected_sha256:"0".repeat(64),args:vec![],
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
                "ping -n 20 127.0.0.1 >NUL".into()],
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
            args:vec![OsString::from("30")],
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
