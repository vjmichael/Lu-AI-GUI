//! Wave 2: native archive extraction in an owned, terminable desktop child.
//! This host-only plan is created only after an exact human-turn approval.
//! The process is contained by the existing Job Object/Unix process group.
//! No model request can select an executable, command line, or cancellation PID.
use std::{
    ffi::OsString,fs::File,io::Read,path::PathBuf,
    sync::Arc,thread,time::Duration,
};
use cap_std::fs::Dir;
use sha2::{Digest,Sha256};
use crate::{
    native_archive_tree::{
        ArchiveCancellation,TreeManifest,TreeReceipt,TreeExtractError,
        observe_tree_completion,verify_tree_manifest,
    },
    native_background_process::{BackgroundProcessSupervisor,TrustedWorkerPlan},
    tasks::{TaskState,TaskStore},
};

fn fail(_:impl std::fmt::Debug)->TreeExtractError{
    TreeExtractError::PublishedNeedsReconciliation
}
/// Identity is obtained from the *opened directory handle* (not a path).
/// The worker must re-open an absolute parent and match this exact token.
pub fn opened_directory_identity(dir:&Dir)->Result<String,TreeExtractError>{
    let opened=dir.try_clone().map_err(fail)?.into_std_file();
    let meta=opened.metadata().map_err(fail)?;
    if !meta.is_dir(){return Err(TreeExtractError::UnsafeMember)}
    #[cfg(windows)]
    {return crate::windows_file_guard::opened_directory_identity(&opened).map_err(fail)}
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        return Ok(format!("{:016x}{:016x}",meta.dev(),meta.ino()))
    }
    #[cfg(not(any(windows,unix)))]
    {Err(TreeExtractError::UnsafeMember)}
}
/// Entered only by the Linux native child process, never the Tauri host
/// or a Rust test harness with unrelated process lifetime constraints.
#[cfg(target_os="linux")]
pub fn bind_archive_worker_parent(expected_pid:u32)->std::io::Result<()>{
    lu_win_worker_job::unix_owned::bind_archive_parent(expected_pid)
}
fn executable_sha256(executable:&PathBuf)->Result<String,TreeExtractError>{
    let mut f=File::open(executable).map_err(fail)?;
    let mut digest=Sha256::new();
    let mut buf=[0u8;65536];
    loop{
        let n=f.read(&mut buf).map_err(fail)?;
        if n==0{break}
        digest.update(&buf[..n]);
    }
    Ok(format!("{:x}",digest.finalize()))
}
/// The host retains every value; none is produced by a WebView or model.
#[derive(Clone)]
pub struct SupervisedArchivePlan{
    pub processes:Arc<BackgroundProcessSupervisor>,
    pub tasks:Arc<TaskStore>,
    pub executable:PathBuf,
    pub source_parent:PathBuf,
    pub destination_parent:PathBuf,
    pub source_parent_identity:String,
    pub destination_parent_identity:String,
    pub manifest:TreeManifest,
    pub workspace_id:String,
    pub task_id:String,
}
struct LiveWorker<'a>{plan:&'a SupervisedArchivePlan,owned:bool}
impl Drop for LiveWorker<'_>{
    fn drop(&mut self){
        if self.owned{
            let _=self.plan.processes.cancel_owned(
                &self.plan.task_id,&self.plan.workspace_id
            );
        }
    }
}
impl SupervisedArchivePlan{
    pub fn execute(&self,source:&Dir,name:&str,destination:&Dir,target:&str,
        approved_digest:&str,cancel:&ArchiveCancellation)
        ->Result<TreeReceipt,TreeExtractError>{
        if cancel.is_cancelled(){return Err(TreeExtractError::Cancelled)}
        if !verify_tree_manifest(&self.manifest)||
            self.manifest.sha256!=approved_digest{
            return Err(TreeExtractError::ManifestChanged)
        }
        if opened_directory_identity(source)?!=self.source_parent_identity||
            opened_directory_identity(destination)?!=self.destination_parent_identity {
            return Err(TreeExtractError::UnsafeMember)
        }
        match destination.symlink_metadata(target){
            Ok(_)=>return Err(TreeExtractError::TargetExists),
            Err(e) if e.kind()==std::io::ErrorKind::NotFound=>{},
            Err(e)=>return Err(TreeExtractError::Io(e.to_string())),
        }
        if cancel.is_cancelled(){return Err(TreeExtractError::Cancelled)}
        let executable=self.executable.canonicalize().map_err(fail)?;
        let exe_hash=executable_sha256(&executable)?;
        if cancel.is_cancelled(){return Err(TreeExtractError::Cancelled)}
        let argv:Vec<OsString>=vec![
            "--lu-archive-extract-worker".into(),
            self.source_parent.as_os_str().into(),name.into(),
            self.destination_parent.as_os_str().into(),target.into(),
            approved_digest.into(),self.source_parent_identity.clone().into(),
            self.destination_parent_identity.clone().into(),
            std::process::id().to_string().into(),
        ];
        let plan=TrustedWorkerPlan{
            task_id:self.task_id.clone(),workspace_id:self.workspace_id.clone(),
            executable,expected_sha256:exe_hash,args:argv,
            progress_protocol:false,max_runtime_seconds:Some(600),
        };
        // After start, all interruptions are uncertain, including a crash
        // before mkdir, because the parent cannot prove a worker's write point.
        self.processes.start_host_approved(plan).map_err(fail)?;
        let mut lease=LiveWorker{plan:self,owned:true};
        loop{
            // Poll is safe to call concurrently with the background watchdog.
            self.processes.poll().map_err(fail)?;
            let state=self.tasks.get(&self.task_id).map_err(fail)?
                .ok_or(TreeExtractError::PublishedNeedsReconciliation)?.state;
            match state{
                TaskState::Completed=>{
                    lease.owned=false;
                    // A zero-exit process is NOT evidence enough. Recheck the
                    // full reviewed manifest from the parent's pinned handle.
                    if observe_tree_completion(destination,target,&self.manifest)
                        .map_err(fail)? {
                        return Ok(TreeReceipt{
                            destination_folder:target.into(),
                            files_extracted:self.manifest.file_count,
                            bytes_extracted:self.manifest.total_bytes,
                            manifest_sha256:self.manifest.sha256.clone(),
                        })
                    }
                    return Err(TreeExtractError::PublishedNeedsReconciliation)
                },
                TaskState::Failed|TaskState::Interrupted|TaskState::Cancelled|
                TaskState::TimedOut=>{
                    lease.owned=false;
                    return Err(TreeExtractError::PublishedNeedsReconciliation)
                },
                TaskState::Pending|TaskState::Running|TaskState::Paused=>{},
            }
            if cancel.is_cancelled(){
                // Kill-and-reap the owned Job/Unix process group. Do not
                // release the journal as FAILED: bytes might exist already.
                self.processes.cancel_owned(&self.task_id,&self.workspace_id)
                    .map_err(fail)?;
                lease.owned=false;
                return Err(TreeExtractError::PublishedNeedsReconciliation)
            }
            thread::sleep(Duration::from_millis(40));
        }
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use cap_std::ambient_authority;
    use std::fs;
    #[test]
    fn source_and_destination_identity_follows_handle_not_swapped_path(){
        let tmp=std::env::temp_dir().join(format!(
            "lu-archive-parent-identity-{}-{}",
            std::process::id(),std::thread::current().name().unwrap_or("worker")
        ));
        let _=fs::remove_dir_all(&tmp);
        fs::create_dir_all(tmp.join("parent")).unwrap();
        let pinned=Dir::open_ambient_dir(tmp.join("parent"),ambient_authority()).unwrap();
        let expected=opened_directory_identity(&pinned).unwrap();
        fs::rename(tmp.join("parent"),tmp.join("old-parent")).unwrap();
        fs::create_dir(tmp.join("parent")).unwrap();
        let substituted=Dir::open_ambient_dir(tmp.join("parent"),ambient_authority()).unwrap();
        assert_ne!(opened_directory_identity(&substituted).unwrap(),expected);
        assert_eq!(opened_directory_identity(&pinned).unwrap(),expected);
        drop(substituted);drop(pinned);fs::remove_dir_all(&tmp).unwrap();
    }
}


// Genuine OS-process failure is journal-uncertain, not a reusable clean
// cancellation. The worker manager's separate native tests cover group kill.
#[cfg(all(test,unix))]
mod failure_matrix{
    use super::*;
    use std::fs;
    use cap_std::ambient_authority;
    #[test]
    fn failed_owned_worker_preserves_target_absence_and_blocks_replay(){
        let tmp=std::env::temp_dir().join(format!(
            "lu-archive-worker-failure-{}",std::process::id()
        ));
        let _=fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        let dir=Dir::open_ambient_dir(&tmp,ambient_authority()).unwrap();
        fs::write(tmp.join("source.txt"),b"source").unwrap();
        crate::native_linux_archive::create_linux_archive_new(
            &dir,"source.tar",&["source.txt".into()],
            "archive-worker-stage-123456789"
        ).unwrap();
        let manifest=crate::native_archive_tree::preview_archive_tree(
            &dir,"source.tar"
        ).unwrap();
        let store=Arc::new(TaskStore::open(std::path::Path::new(":memory:")).unwrap());
        let supervisor=Arc::new(BackgroundProcessSupervisor::new(Arc::clone(&store)));
        let plan=SupervisedArchivePlan{
            processes:supervisor,tasks:store.clone(),
            executable:PathBuf::from("/bin/false").canonicalize().unwrap(),
            source_parent:tmp.clone(),destination_parent:tmp.clone(),
            source_parent_identity:opened_directory_identity(&dir).unwrap(),
            destination_parent_identity:opened_directory_identity(&dir).unwrap(),
            manifest:manifest.clone(),workspace_id:"c-test".into(),
            task_id:"archive-fail-123".into(),
        };
        let first=plan.execute(&dir,"source.tar",&dir,"new",&manifest.sha256,
            &ArchiveCancellation::default());
        assert_eq!(first,Err(TreeExtractError::PublishedNeedsReconciliation));
        assert!(!tmp.join("new").exists());
        assert_eq!(store.get("archive-fail-123").unwrap().unwrap().state,
            TaskState::Failed);
        let second=plan.execute(&dir,"source.tar",&dir,"new",&manifest.sha256,
            &ArchiveCancellation::default());
        assert_eq!(second,Err(TreeExtractError::PublishedNeedsReconciliation));
        assert!(!tmp.join("new").exists());
        drop(plan);drop(dir);fs::remove_dir_all(tmp).unwrap();
    }
}
