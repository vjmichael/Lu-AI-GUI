//! E1-P08C candidate: native Linux cgroup-v2 lifetime custody for a worker.
//!
//! The first-party guardian stays OUTSIDE the worker leaf; when its sole
//! host-owned stdin writer closes (even on SIGKILL), it writes cgroup.kill
//! and requires recursive cgroup.events populated=0. The worker joins the
//! leaf in pre_exec, BEFORE any third-party executable may run.
//!
//! Only a *previously delegated and writable* cgroup is admitted. Never run
//! sudo, create a privileged unit, accept a model-supplied PID/path, or
//! silently fall back to PGID signalling for a strong-containment request.
//!
//! Limitation: the ancestor's cgroup.procs permissions and systemd single-
//! writer delegation still need independent validation for untrusted apps.
//! This API is NOT yet advertised by Lu's model-facing tool catalogue.
use std::{
    fs::{self,File,OpenOptions},io::{self,Read,Write},
    os::{fd::AsRawFd,unix::{fs::PermissionsExt,process::CommandExt}},
    path::{Component,Path,PathBuf},
    process::{Child,ChildStdin,Command,Stdio},
    time::{Duration,Instant},
};
const MOUNT:&str="/sys/fs/cgroup";
const PREFIX:&str="lu-e1-custody-";
const MAX_EVENTS:usize=8192;

fn error(message:&str)->io::Error{io::Error::other(message)}
fn read_small(path:&Path,limit:usize)->io::Result<String>{
    let bytes=fs::read(path)?;
    if bytes.len()>limit{return Err(error("oversized kernel cgroup state"))}
    String::from_utf8(bytes).map_err(|_|error("non-UTF8 kernel cgroup state"))
}
fn check_no_links(path:&Path)->io::Result<()>{
    let mut current=PathBuf::new();
    for component in path.components(){
        match component{
            Component::RootDir=>current.push("/"),
            Component::Normal(part)=>{
                current.push(part);
                let meta=fs::symlink_metadata(&current)?;
                if meta.file_type().is_symlink()||!meta.is_dir(){
                    return Err(error("linked or non-directory cgroup component"))
                }
            },
            _=>return Err(error("unsafe cgroup component"))
        }
    }
    Ok(())
}
/// Trusted kernel identity of THIS Lu / guardian process. This is NOT a
/// user-provided path, and the mount is required to be unified cgroup v2.
pub fn delegated_parent()->io::Result<PathBuf>{
    let mount_info=read_small(Path::new("/proc/self/mountinfo"),128*1024)?;
    let mount_found=mount_info.lines().any(|line|
        line.split(" - ").next().is_some_and(|left|
            left.split_whitespace().nth(4)==Some(MOUNT))
        &&line.split(" - ").nth(1)
            .is_some_and(|right|right.starts_with("cgroup2 "))
    );
    if !mount_found{return Err(error("unified cgroup-v2 mount unavailable"))}
    let process_cgroup=read_small(Path::new("/proc/self/cgroup"),8192)?;
    let path=process_cgroup.lines().find_map(|line|
        line.strip_prefix("0::")).ok_or_else(||
            error("process is not in the unified cgroup hierarchy"))?;
    if !path.starts_with('/'){return Err(error("nonabsolute kernel cgroup"))}
    let relative=path.trim_start_matches('/');
    let mut result=PathBuf::from(MOUNT);
    if !relative.is_empty(){
        for segment in Path::new(relative).components(){
            match segment{
                Component::Normal(name)=>result.push(name),
                _=>return Err(error("invalid kernel cgroup path"))
            }
        }
    }
    check_no_links(&result)?;
    if !result.join("cgroup.procs").is_file(){
        return Err(error("cgroup.procs missing"))
    }
    Ok(result)
}
fn leaf_path(parent:&Path,guardian_pid:u32)->io::Result<PathBuf>{
    if guardian_pid<=1{return Err(error("invalid guardian identity"))}
    Ok(parent.join(format!("{PREFIX}{guardian_pid}")))
}
fn populated_zero(path:&Path)->io::Result<bool>{
    let state=read_small(&path.join("cgroup.events"),MAX_EVENTS)?;
    state.lines().find_map(|line|line.strip_prefix("populated "))
        .map(|value|value=="0")
        .ok_or_else(||error("kernel cgroup.events lacks populated"))
}
fn verify_admitted_leaf(path:&Path)->io::Result<()>{
    check_no_links(path)?;
    let state=fs::symlink_metadata(path)?;
    if !state.is_dir(){return Err(error("cgroup leaf was replaced"))}
    for name in ["cgroup.procs","cgroup.kill","cgroup.events"]{
        let file=path.join(name);
        if !fs::symlink_metadata(&file)?.is_file(){
            return Err(error("missing or linked cgroup-v2 interface"))
        }
    }
    if !populated_zero(path)?{
        return Err(error("worker group not empty at admission"))
    }
    Ok(())
}

/// Entry point of the installed FIRST-PARTY guardian. The host alone holds
/// the writer end of stdin; on host SIGKILL EOF still triggers kernel kill.
/// It runs in the parent cgroup; moving this guardian into the killable leaf
/// would remove the crash-time cleanup guarantee and is forbidden.
pub fn guardian_main()->io::Result<()>{
    let parent=delegated_parent()?;
    // The Linux desktop must have actual delegation. Creating a leaf
    // tests that condition; a read-only or non-delegated mount fails closed.
    let leaf=leaf_path(&parent,std::process::id())?;
    fs::create_dir(&leaf)?;
    let result=(||->io::Result<()>{
        verify_admitted_leaf(&leaf)?;
        let mut killer=OpenOptions::new().write(true).open(leaf.join("cgroup.kill"))?;
        // Don't claim readiness before the kernel kill interface is opened.
        let mut out=io::stdout().lock();
        out.write_all(b"READY\n")?;
        out.flush()?;
        drop(out);
        let mut input=io::stdin().lock();
        let mut buffer=[0u8;32];
        loop{
            match input.read(&mut buffer){
                Ok(0)=>break,
                Ok(_)=>{}, // no commands are accepted over this pipe
                Err(ref e) if e.kind()==io::ErrorKind::Interrupted=>{},
                Err(e)=>return Err(e)
            }
        }
        killer.write_all(b"1")?;
        let deadline=Instant::now()+Duration::from_secs(8);
        while !populated_zero(&leaf)?{
            if Instant::now()>=deadline{
                return Err(error("cgroup.kill did not empty subtree"))
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        Ok(())
    })();
    // Retain evidence of an unemptied subtree. Never erase the directory
    // or report success if kernel cleanup failed.
    if result.is_ok(){fs::remove_dir(&leaf)?}
    result
}
pub struct LinuxCgroupGuardian{
    child:Child,
    lifetime:Option<ChildStdin>,
    /// Opened from kernel-verified, newly created leaf before first spawn.
    /// FD_CLOEXEC prevents this write capability leaking into the program.
    procs:File,
    leaf:PathBuf,
    stopped:bool,
}
impl LinuxCgroupGuardian{
    pub fn spawn()->io::Result<Self>{
        let current=std::env::current_exe()?;
        let sibling=current.parent().ok_or_else(||error("missing Lu executable directory"))?
            .join("lu-linux-cgroup-guard");
        let executable=if cfg!(debug_assertions){
            std::env::var_os("LU_E1_P08C_TEST_GUARD_BINARY")
                .map(PathBuf::from).unwrap_or(sibling)
        }else{sibling};
        let meta=fs::symlink_metadata(&executable)?;
        if !meta.is_file()||meta.file_type().is_symlink()
            ||meta.permissions().mode()&0o111==0
            ||meta.permissions().mode()&0o022!=0{
            return Err(error("untrusted first-party cgroup guard"))
        }
        // The parent identity must match the guardian's inherited cgroup.
        let parent=delegated_parent()?;
        let mut command=Command::new(&executable);
        command.env_clear().stdin(Stdio::piped())
            .stdout(Stdio::piped()).stderr(Stdio::null());
        let mut child=command.spawn()?;
        let leaf=leaf_path(&parent,child.id())?;
        let mut banner=[0u8;6];
        let handshake=child.stdout.take()
            .ok_or_else(||error("missing guardian readiness pipe"))?
            .read_exact(&mut banner);
        if handshake.is_err()||&banner!=b"READY\n"{
            let _=child.kill();let _=child.wait();
            return Err(error("cgroup delegation unavailable or guard failed"))
        }
        let admitted=(||->io::Result<(File,ChildStdin)>{
            verify_admitted_leaf(&leaf)?;
            // The owner of the delegation must be able to write the
            // group membership file; the guardian is outside this leaf.
            let procs=OpenOptions::new().write(true).open(leaf.join("cgroup.procs"))?;
            let lifetime=child.stdin.take()
                .ok_or_else(||error("missing guardian lifetime pipe"))?;
            if child.try_wait()?.is_some(){
                return Err(error("guardian died during admission"))
            }
            Ok((procs,lifetime))
        })();
        match admitted{
            Ok((procs,lifetime))=>Ok(Self{
                child,lifetime:Some(lifetime),procs,leaf,stopped:false
            }),
            Err(e)=>{
                // Closing stdin allows the guard's own cleanup path to run.
                drop(child.stdin.take());let _=child.wait();Err(e)
            }
        }
    }
    /// A host-created worker joins its trusted leaf before ANY external
    /// executable can start. This is never an after-spawn PID migration.
    /// Other pre_exec hooks (PDEATHSIG, quota) are registered separately.
    pub fn bind_before_exec(&self,command:&mut Command)->io::Result<()>{
        let fd=self.procs.as_raw_fd();
        unsafe extern "C"{
            fn write(fd:i32,buf:*const std::ffi::c_void,count:usize)->isize;
        }
        // SAFETY: only the fixed, pinned kernel cgroup.procs fd and
        // literal "0" are used. Linux interprets 0 as current child.
        // The fd stays open until Command::spawn returns.
        unsafe{
            command.pre_exec(move||{
                if write(fd,b"0".as_ptr().cast(),1)!=1{
                    return Err(io::Error::last_os_error())
                }
                Ok(())
            });
        }
        Ok(())
    }
    pub fn leaf(&self)->&Path{&self.leaf}
    /// Only reports success after the independent guardian exited 0,
    /// which requires kernel cgroup.kill and populated 0, plus rmdir.
    pub fn stop_and_verify_empty(&mut self)->io::Result<()>{
        if self.stopped{return Ok(())}
        drop(self.lifetime.take());
        let deadline=Instant::now()+Duration::from_secs(12);
        loop{
            if let Some(status)=self.child.try_wait()?{
                self.stopped=true;
                if !status.success()||self.leaf.exists(){
                    return Err(error("kernel cgroup empty acknowledgement missing"))
                }
                return Ok(())
            }
            if Instant::now()>=deadline{
                let _=self.child.kill();let _=self.child.wait();self.stopped=true;
                return Err(error("cgroup guardian cleanup timed out"))
            }
            std::thread::sleep(Duration::from_millis(15));
        }
    }
}
impl Drop for LinuxCgroupGuardian{
    fn drop(&mut self){
        if !self.stopped{let _=self.stop_and_verify_empty();}
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn invalid_kernel_process_group_identity_fails_closed(){
        assert!(leaf_path(Path::new(MOUNT),0).is_err());
        assert!(leaf_path(Path::new(MOUNT),1).is_err());
        assert!(leaf_path(Path::new(MOUNT),42).unwrap()
            .ends_with("lu-e1-custody-42"));
    }
    #[test]
    fn exact_cgroup_events_parsing(){
        let tmp=std::env::temp_dir().join(format!("lu-e1-events-{}",std::process::id()));
        let _=fs::create_dir(&tmp);
        fs::write(tmp.join("cgroup.events"),"populated 0\nfrozen 0\n").unwrap();
        assert!(populated_zero(&tmp).unwrap());
        fs::write(tmp.join("cgroup.events"),"populated 1\n").unwrap();
        assert!(!populated_zero(&tmp).unwrap());
        fs::remove_dir_all(tmp).unwrap();
    }
}
