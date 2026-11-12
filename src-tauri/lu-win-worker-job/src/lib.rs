//! Audited Win32 FFI boundary for Lu Core, which forbids unsafe code.
//! Lifetime containment for a Windows llama-server child owned by Lu.
//!
//! The Job is created with KILL_ON_JOB_CLOSE before the child is launched.
//! The caller must assign immediately after spawn, *before* any health probe.
//! Failure to assign is fatal and requires killing/reaping the child. A normal
//! std::process::Command spawn still has a small pre-assignment race; therefore
//! this is defense-in-depth, NOT proof of fully race-free process containment.

#[cfg(windows)]
pub mod system_memory;
#[cfg(windows)]
pub mod file_guard;
#[cfg(windows)]
pub mod metadata_guard;
#[cfg(windows)]
pub mod browser_window;

#[cfg(windows)]
mod windows_owned {


use std::{ffi::c_void, io, mem, os::windows::io::AsRawHandle, process::Child, ptr, sync::atomic::{AtomicBool,Ordering}, time::{Duration,Instant}};

type Handle = *mut c_void;
const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION_CLASS: i32 = 9;
const JOB_OBJECT_BASIC_ACCOUNTING_INFORMATION_CLASS:i32=1;
const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x0000_2000;
const JOB_OBJECT_LIMIT_JOB_MEMORY:u32=0x0000_0200;
const TH32CS_SNAPTHREAD:u32=0x0000_0004;
const THREAD_SUSPEND_RESUME:u32=0x0002;
#[repr(C)]
#[derive(Default)]
struct ThreadEntry32 {
    size:u32,
    usage:u32,
    thread_id:u32,
    owner_process_id:u32,
    base_priority:i32,
    delta_priority:i32,
    flags:u32,
}
const _:()={assert!(mem::size_of::<ThreadEntry32>()==28);};


#[repr(C)]
#[derive(Default)]
struct BasicLimitInformation {
    per_process_user_time_limit: i64,
    per_job_user_time_limit: i64,
    limit_flags: u32,
    minimum_working_set_size: usize,
    maximum_working_set_size: usize,
    active_process_limit: u32,
    affinity: usize,
    priority_class: u32,
    scheduling_class: u32,
}

#[repr(C)]
#[derive(Default)]
struct IoCounters {
    read_operation_count: u64,
    write_operation_count: u64,
    other_operation_count: u64,
    read_transfer_count: u64,
    write_transfer_count: u64,
    other_transfer_count: u64,
}

#[repr(C)]
#[derive(Default)]
struct ExtendedLimitInformation {
    basic_limit_information: BasicLimitInformation,
    io_info: IoCounters,
    process_memory_limit: usize,
    job_memory_limit: usize,
    peak_process_memory_used: usize,
    peak_job_memory_used: usize,
}

// Lu's Windows alpha is x86-64; preserve Win32 structure ABI at the FFI
// boundary even if these definitions are edited in a later refactor.
const _: () = {
    assert!(mem::size_of::<BasicLimitInformation>() == 64);
    assert!(mem::size_of::<IoCounters>() == 48);
    assert!(mem::size_of::<ExtendedLimitInformation>() == 144);
};

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateJobObjectW(attributes: *const c_void, name: *const u16) -> Handle;
    fn SetInformationJobObject(
        job: Handle,
        info_class: i32,
        info: *const c_void,
        length: u32,
    ) -> i32;
    fn TerminateJobObject(job:Handle, exit_code:u32)->i32;
    fn QueryInformationJobObject(
        job:Handle,info_class:i32,info:*mut c_void,length:u32,
        return_length:*mut u32
    )->i32;
    fn AssignProcessToJobObject(job: Handle, process: Handle) -> i32;
    fn CloseHandle(handle: Handle) -> i32;
    fn CreateToolhelp32Snapshot(flags:u32,process_id:u32)->Handle;
    fn Thread32First(snapshot:Handle,entry:*mut ThreadEntry32)->i32;
    fn Thread32Next(snapshot:Handle,entry:*mut ThreadEntry32)->i32;
    fn OpenThread(access:u32,inherit:i32,thread_id:u32)->Handle;
    fn ResumeThread(thread:Handle)->u32;
}

/// Owns one kernel Job Object handle, not the child process handle itself.
/// No clone is allowed: the final CloseHandle must terminate the job's tree.
/// A value impossible to construct outside this native Win32 crate. Its
/// sole producer requires a real assigned Job, kernel termination, and an
/// independent kernel BasicAccounting ActiveProcesses==0 query. This proves
/// only that Job's process tree is empty at observation, not that arbitrary
/// non-job processes elsewhere in the OS have ended.
#[derive(Debug)]
pub struct WindowsJobEmptyProof{_private:()}
impl WindowsJobEmptyProof{
    pub fn method(&self)->&'static str{"WINDOWS_JOB_ACTIVE_ZERO_V1"}
}
#[repr(C)]
#[derive(Default)]
struct JobBasicAccounting{
    total_user_time:i64,total_kernel_time:i64,
    period_user_time:i64,period_kernel_time:i64,
    total_page_fault_count:u32,total_processes:u32,
    active_processes:u32,total_terminated_processes:u32,
}
const _:()={assert!(mem::size_of::<JobBasicAccounting>()==48);};
pub struct Job(Handle,AtomicBool);

// A kernel handle can be closed on a different thread, and ownership of this
// unique handle is never shared by this type.
unsafe impl Send for Job {}

impl Job {
    pub fn new() -> io::Result<Self> {
        // SAFETY: null security attributes/name request a private unnamed Job.
        let handle = unsafe { CreateJobObjectW(ptr::null(), ptr::null()) };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        let job = Self(handle,AtomicBool::new(false));
        let mut info = ExtendedLimitInformation::default();
        info.basic_limit_information.limit_flags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // SAFETY: `info` is a repr(C), initialized value that remains alive for
        // the whole call, and `job` owns a valid open handle.
        let result = unsafe {
            SetInformationJobObject(
                job.0,
                JOB_OBJECT_EXTENDED_LIMIT_INFORMATION_CLASS,
                &info as *const _ as *const c_void,
                mem::size_of::<ExtendedLimitInformation>() as u32,
            )
        };
        if result == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(job)
    }


    /// The host launches the child with CREATE_SUSPENDED. Put that process
    /// into our kill-on-close Job *before* releasing its primary thread.
    /// std::process::Child does not expose the thread handle, so the native
    /// Win32 boundary locates the initial suspended thread by process ID.
    /// Caller must kill/reap on every error; a thread is never resumed before
    /// the Job assignment succeeds.
    pub fn assign_and_resume_suspended(&self,child:&Child)->io::Result<()>{
        self.assign(child)?;
        // SAFETY: THREADENTRY32 has the documented Win32 layout and size.
        let snapshot=unsafe{CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD,0)};
        if snapshot.is_null()||snapshot as isize == -1 {
            return Err(io::Error::last_os_error());
        }
        let mut entry=ThreadEntry32{
            size:mem::size_of::<ThreadEntry32>() as u32,
            ..Default::default()
        };
        let mut found=false;
        // SAFETY: snapshot stays open and entry points to initialized data.
        let mut next=unsafe{Thread32First(snapshot,&mut entry)};
        while next!=0{
            if entry.owner_process_id==child.id(){
                found=true;
                // SAFETY: OpenThread returns a separately owned thread handle.
                let thread=unsafe{OpenThread(THREAD_SUSPEND_RESUME,0,entry.thread_id)};
                if thread.is_null(){
                    let error=io::Error::last_os_error();
                    unsafe{CloseHandle(snapshot)};
                    return Err(error);
                }
                // Must have a suspended primary thread: a returned zero
                // means we cannot prove code was held prior to assignment.
                let previous=unsafe{ResumeThread(thread)};
                let failure=if previous==u32::MAX{
                    Some(io::Error::last_os_error())
                }else if previous==0{
                    Some(io::Error::other("engine thread was not suspended"))
                }else{None};
                unsafe{CloseHandle(thread)};
                if let Some(error)=failure{
                    unsafe{CloseHandle(snapshot)};
                    return Err(error);
                }
                break;
            }
            entry.size=mem::size_of::<ThreadEntry32>() as u32;
            next=unsafe{Thread32Next(snapshot,&mut entry)};
        }
        unsafe{CloseHandle(snapshot)};
        if !found{return Err(io::Error::other("suspended engine thread not found"));}
        Ok(())
    }

    /// Limit only the fixed internal archive codecs, not GGUF inference.
    pub fn set_memory_limit(&self,bytes:usize)->io::Result<()>{
        if bytes<256*1024*1024 || bytes>4*1024*1024*1024usize{
            return Err(io::Error::other("invalid worker memory quota"))
        }
        let mut info=ExtendedLimitInformation::default();
        info.basic_limit_information.limit_flags=
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE|JOB_OBJECT_LIMIT_JOB_MEMORY;
        info.job_memory_limit=bytes;
        let ok=unsafe{SetInformationJobObject(
            self.0,JOB_OBJECT_EXTENDED_LIMIT_INFORMATION_CLASS,
            &info as *const _ as *const c_void,
            mem::size_of::<ExtendedLimitInformation>() as u32
        )};
        if ok==0{Err(io::Error::last_os_error())}else{Ok(())}
    }
    pub fn assign(&self, child: &Child) -> io::Result<()> {
        // SAFETY: both handles remain alive for this synchronous call.
        let result = unsafe { AssignProcessToJobObject(self.0, child.as_raw_handle() as Handle) };
        if result == 0 {
            return Err(io::Error::last_os_error());
        }
        self.1.store(true,Ordering::Release);
        Ok(())
    }
    // Test-only positive precondition: kernel confirms the owned Job
    // contains both the primary shell and a genuine spawned descendant.
    #[cfg(test)]
    pub(crate) fn kernel_live_count_for_test(&self)->io::Result<u32>{
        if !self.1.load(Ordering::Acquire){
            return Err(io::Error::other("No worker assigned to test Job"))
        }
        let mut accounting=JobBasicAccounting::default();
        let mut returned=0u32;
        let ok=unsafe{QueryInformationJobObject(
            self.0,JOB_OBJECT_BASIC_ACCOUNTING_INFORMATION_CLASS,
            &mut accounting as *mut _ as *mut c_void,
            mem::size_of::<JobBasicAccounting>() as u32,
            &mut returned
        )};
        if ok==0{return Err(io::Error::last_os_error())}
        if returned!=mem::size_of::<JobBasicAccounting>() as u32{
            return Err(io::Error::other("Invalid test kernel Job accounting size"))
        }
        Ok(accounting.active_processes)
    }
    /// E1-P08/AGENT-01L: obtain an actual kernel observation while retaining
    /// the original private handle, before any result may be marked complete.
    /// Never accept a PID or Job handle from a saved SQLite record.
    pub fn terminate_and_verify_empty(&self)->io::Result<WindowsJobEmptyProof>{
        if !self.1.load(Ordering::Acquire){
            return Err(io::Error::other("Job never admitted an owned worker"))
        }
        // Query before signalling: a reviewed one-shot may already have
        // exited with no descendants, and an empty Job is still a valid
        // kernel observation. If live children remain, TerminateJobObject
        // targets only THIS assigned Job, never a saved or user-supplied PID.
        let deadline=Instant::now()+Duration::from_secs(5);
        let mut termination_sent=false;
        loop{
            let mut accounting=JobBasicAccounting::default();
            let mut returned=0u32;
            let ok=unsafe{QueryInformationJobObject(
                self.0,JOB_OBJECT_BASIC_ACCOUNTING_INFORMATION_CLASS,
                &mut accounting as *mut _ as *mut c_void,
                mem::size_of::<JobBasicAccounting>() as u32,
                &mut returned
            )};
            if ok==0{return Err(io::Error::last_os_error())}
            if returned!=mem::size_of::<JobBasicAccounting>() as u32{
                return Err(io::Error::other("Invalid kernel job accounting size"))
            }
            if accounting.active_processes==0{
                return Ok(WindowsJobEmptyProof{_private:()})
            }
            if !termination_sent{
                if unsafe{TerminateJobObject(self.0,1)}==0{
                    return Err(io::Error::last_os_error())
                }
                termination_sent=true;
            }
            if Instant::now()>=deadline{
                return Err(io::Error::other("Job still contains live descendants"))
            }
            std::thread::sleep(Duration::from_millis(15));
        }
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        // SAFETY: unique owner; KILL_ON_JOB_CLOSE terminates any remaining
        // processes assigned to this job when its final handle is closed.
        unsafe { CloseHandle(self.0) };
    }
}

}
#[cfg(windows)]
pub use windows_owned::{Job,WindowsJobEmptyProof};

#[cfg(unix)]
pub mod unix_owned {
    use std::{io,process::Child};
    #[cfg(target_os="linux")]
    use std::{ffi::CString,fs::File,os::fd::AsRawFd};
    // POSIX killpg via negative PID. The calling host MUST have spawned this
    // child with CommandExt::process_group(0); never accept a model PID here.
    unsafe extern "C" {fn kill(pid:i32, signal:i32)->i32;}
    const SIGKILL:i32=9;

    // Admission must never block on an untrusted or wedged helper pipe.
    // Poll/read only the host's already-open pipe: no detached reader thread
    // survives a timeout. Child cleanup is owned by AdmissionChild below.
    #[cfg(target_os="linux")]
    fn bounded_guardian_banner(
        pipe:&mut std::process::ChildStdout,
        limit:std::time::Duration,
    )->io::Result<String>{
        use std::{os::{fd::AsRawFd,raw::c_void},time::Instant};
        #[repr(C)]
        struct PollFd{fd:i32,events:i16,revents:i16}
        unsafe extern "C"{
            fn poll(fds:*mut PollFd,nfds:usize,timeout:i32)->i32;
            fn read(fd:i32,buf:*mut c_void,count:usize)->isize;
        }
        const POLLIN:i16=0x0001;
        const POLLERR:i16=0x0008;
        const POLLHUP:i16=0x0010;
        let end=Instant::now()+limit;
        let mut banner=Vec::with_capacity(128);
        loop{
            let now=Instant::now();
            if now>=end{
                return Err(io::Error::new(io::ErrorKind::TimedOut,
                    "Linux guardian readiness deadline exceeded"))
            }
            let remaining=end.duration_since(now).as_millis()
                .min(i32::MAX as u128).max(1) as i32;
            let mut fds=PollFd{fd:pipe.as_raw_fd(),
                events:POLLIN|POLLERR|POLLHUP,revents:0};
            // SAFETY: valid sole-owned stdout FD and initialized pollfd.
            let polled=unsafe{poll(&mut fds,1,remaining)};
            if polled==0{continue}
            if polled<0{
                let err=io::Error::last_os_error();
                if err.kind()==io::ErrorKind::Interrupted{continue}
                return Err(err)
            }
            let mut byte=0u8;
            // Poll reported readable, EOF or error. A one-byte read cannot
            // consume another protocol message and never awaits a newline.
            let n=unsafe{read(fds.fd,(&mut byte as *mut u8).cast(),1)};
            if n==0{
                return Err(io::Error::new(io::ErrorKind::UnexpectedEof,
                    "Linux guardian closed an incomplete readiness line"))
            }
            if n<0{
                let err=io::Error::last_os_error();
                if err.kind()==io::ErrorKind::Interrupted
                    ||err.kind()==io::ErrorKind::WouldBlock{continue}
                return Err(err)
            }
            if banner.len()>=1024{
                return Err(io::Error::other("Linux guardian banner too long"))
            }
            banner.push(byte);
            if byte==b'\n'{
                return String::from_utf8(banner)
                    .map_err(|_|io::Error::other("Non UTF-8 guardian readiness"))
            }
        }
    }
    #[cfg(target_os="linux")]
    struct AdmissionChild(Option<Child>);
    #[cfg(target_os="linux")]
    impl AdmissionChild{
        fn new(child:Child)->Self{Self(Some(child))}
        fn child(&mut self)->&mut Child{
            self.0.as_mut().expect("admission child owned")
        }
        fn admit(mut self)->Child{
            self.0.take().expect("admission child transferred")
        }
    }
    #[cfg(target_os="linux")]
    impl Drop for AdmissionChild{
        fn drop(&mut self){
            // Any timeout, malformed reply, missing pipe or subsequent OS
            // validation failure reaps the exact spawned helper. There is no
            // partial worker admission or PGID-based fallback on failure.
            if let Some(mut child)=self.0.take(){
                if !matches!(child.try_wait(),Ok(Some(_))){
                    let _=child.kill();
                }
                // A Linux kernel task stuck in uninterruptible sleep may
                // not reap despite SIGKILL. Never make host admission hang
                // forever waiting for the OS. No verified custody is minted.
                let deadline=std::time::Instant::now()
                    +std::time::Duration::from_secs(2);
                loop{
                    match child.try_wait(){
                        Ok(Some(_))=>break,
                        Ok(None) if std::time::Instant::now()<deadline=>
                            std::thread::sleep(std::time::Duration::from_millis(10)),
                        _=>break,
                    }
                }
            }
        }
    }
    #[cfg(target_os="linux")]
    fn bounded_guardian_exit(
        child:&mut Child,limit:std::time::Duration,
    )->io::Result<std::process::ExitStatus>{
        let end=std::time::Instant::now()+limit;
        loop{
            if let Some(status)=child.try_wait()?{return Ok(status)}
            if std::time::Instant::now()>=end{
                return Err(io::Error::new(io::ErrorKind::TimedOut,
                    "Linux guardian did not exit after UNAVAILABLE"))
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }


    #[cfg(all(test,target_os="linux"))]
    mod guardian_readiness_tests{
        use super::*;
        use std::{process::{Command,Stdio},time::{Duration,Instant}};

        fn banner_case(script:&str,timeout:Duration)->(io::Result<String>,Duration){
            let mut child=Command::new("/bin/sh")
                .args(["-c",script])
                .stdin(Stdio::null()).stdout(Stdio::piped())
                .stderr(Stdio::null()).spawn().expect("fixture spawned");
            let start=Instant::now();
            let response=bounded_guardian_banner(
                child.stdout.as_mut().unwrap(),timeout);
            let elapsed=start.elapsed();
            let _=child.kill();let _=child.wait();
            (response,elapsed)
        }
        #[test]
        fn still_alive_silent_guardian_cannot_block_admission(){
            let (result,elapsed)=banner_case("exec sleep 2",Duration::from_millis(125));
            assert_eq!(result.unwrap_err().kind(),io::ErrorKind::TimedOut);
            assert!(elapsed<Duration::from_secs(1));
        }
        #[test]
        fn truncated_and_malformed_guardian_banners_are_not_ready(){
            let (truncated,_)=banner_case("printf READY",Duration::from_secs(1));
            assert_eq!(truncated.unwrap_err().kind(),io::ErrorKind::UnexpectedEof);
            let (malformed,_)=banner_case("printf 'NOTREADY\n'",Duration::from_secs(1));
            assert_ne!(malformed.unwrap(),"READY\n");
            let (valid,_)=banner_case("printf 'READY\n'",Duration::from_secs(1));
            assert_eq!(valid.unwrap(),"READY\n");
        }
        #[test]
        fn failed_or_incomplete_guardian_stop_cannot_turn_into_success_on_retry(){
            let mut group_child=Command::new("/bin/true")
                .stdin(Stdio::null()).stdout(Stdio::null())
                .spawn().unwrap();
            let _=group_child.wait().unwrap();
            let mut group=LinuxOwnedGroupGuardian{
                child:group_child,stdin:None,stopped:true,verified_signal:false,
            };
            assert!(group.stop_and_verify_signal().is_err());
            assert!(group.stop_and_verify_signal().is_err());

            let mut cgroup_child=Command::new("/bin/true")
                .stdin(Stdio::null()).stdout(Stdio::null())
                .spawn().unwrap();
            let _=cgroup_child.wait().unwrap();
            let mut cgroup=LinuxCgroupCustodian{
                child:cgroup_child,stdin:None,
                subtree:std::path::PathBuf::new(),
                receipt:std::path::PathBuf::new(),expected_receipt:String::new(),
                stopped:true,verified_empty:false,
            };
            assert!(cgroup.stop_and_verify().is_err());
            assert!(cgroup.stop_and_verify().is_err());
        }
        #[test]
        fn sec06_kernel_receipt_rejects_cross_chat_nonce_truncation_symlinks_and_mode(){
            use std::{fs,os::unix::fs::{PermissionsExt,symlink}};
            let dir=std::env::temp_dir().join(format!(
                "lu-sec06-receipt-{}-{}",std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
            fs::create_dir(&dir).unwrap();
            let receipt=dir.join("original-guard.receipt");
            let exact=format!(
                "LU_CGROUP_V2_EMPTY_V1\nCHAT=c-{}\nTURN=g-{}\nCALL=sec06-gh\nNONCE={}\nPOPULATED=0\n",
                "a".repeat(32),"b".repeat(32),"c".repeat(32)
            );
            fs::write(&receipt,&exact).unwrap();
            fs::set_permissions(&receipt,fs::Permissions::from_mode(0o600))
                .unwrap();
            assert!(validate_linux_cgroup_receipt(&receipt,&exact).is_ok());
            for wrong in [
                exact.replace("CHAT=c-","CHAT=c-other-"),
                exact.replace("CALL=sec06-gh","CALL=other-gh"),
                exact.replace(&"c".repeat(32),&"d".repeat(32)),
                exact.replace("POPULATED=0","POPULATED=1"),
                format!("{exact}REPLAY=1\n"),
                exact.trim_end().to_string(),
            ]{
                fs::write(&receipt,wrong).unwrap();
                assert!(validate_linux_cgroup_receipt(&receipt,&exact).is_err());
            }
            fs::write(&receipt,&exact).unwrap();
            fs::set_permissions(&receipt,fs::Permissions::from_mode(0o666))
                .unwrap();
            assert!(validate_linux_cgroup_receipt(&receipt,&exact).is_err());
            fs::set_permissions(&receipt,fs::Permissions::from_mode(0o600))
                .unwrap();
            fs::write(&receipt,vec![b'x';4097]).unwrap();
            assert!(validate_linux_cgroup_receipt(&receipt,&exact).is_err());
            fs::write(&receipt,&exact).unwrap();
            let link=dir.join("linked.receipt");
            symlink(&receipt,&link).unwrap();
            assert!(validate_linux_cgroup_receipt(&link,&exact).is_err());
            fs::remove_file(&link).unwrap();
            fs::remove_file(&receipt).unwrap();
            fs::remove_dir(dir).unwrap();
        }
        #[test]
        fn unavailable_helper_must_exit_before_no_delegation_result(){
            let mut child=Command::new("/bin/sh").args(["-c","exec sleep 2"])
                .stdin(Stdio::null()).stdout(Stdio::piped())
                .stderr(Stdio::null()).spawn().unwrap();
            let start=Instant::now();
            let err=bounded_guardian_exit(&mut child,Duration::from_millis(125))
                .unwrap_err();
            assert_eq!(err.kind(),io::ErrorKind::TimedOut);
            assert!(start.elapsed()<Duration::from_secs(1));
            let _=child.kill();let _=child.wait();
        }
    }

    /// E1-P08 Linux-only: the private process group stays anchored by a small
    /// independently running first-party guardian, not the disposable
    /// Git/FFprobe leader. After an abnormal Lu exit its stdin pipe reaches
    /// EOF and it kills its OWN process group. No stale saved PID is signalled.
    #[cfg(target_os="linux")]
    pub struct LinuxOwnedGroupGuardian{
        child:Child,
        stdin:Option<std::process::ChildStdin>,
        stopped:bool,
        verified_signal:bool,
    }
    #[cfg(target_os="linux")]
    impl LinuxOwnedGroupGuardian{
        /// Release builds require the sibling first-party installed helper.
        /// Debug builds can run old independent worker tests without bundling
        /// the new helper. The explicit debug-only override is for CI, not a
        /// model-visible executable chooser or production configuration.
        pub fn spawn_if_available()->io::Result<Option<Self>>{
            use std::os::unix::{fs::PermissionsExt,process::CommandExt};
            let current=std::env::current_exe()?;
            let installed=current.parent()
                .ok_or_else(||io::Error::other("Lu executable parent missing"))?
                .join("lu-linux-group-guard");
            let candidate=if cfg!(debug_assertions){
                std::env::var_os("LU_E1_P08_TEST_GUARD_BINARY")
                    .map(std::path::PathBuf::from).unwrap_or(installed)
            }else{installed};
            let meta=match std::fs::symlink_metadata(&candidate){
                Ok(meta)=>meta,
                Err(e) if e.kind()==io::ErrorKind::NotFound&&cfg!(debug_assertions)
                    &&std::env::var_os("LU_E1_P08_TEST_GUARD_BINARY").is_none()=>
                    return Ok(None),
                Err(e)=>return Err(e),
            };
            if !meta.is_file()||meta.file_type().is_symlink()
                ||meta.permissions().mode()&0o111==0
                ||meta.permissions().mode()&0o022!=0{
                return Err(io::Error::other("untrusted Lu group guardian file"))
            }
            let mut command=std::process::Command::new(&candidate);
            command.process_group(0).env_clear()
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null());
            let mut admission=AdmissionChild::new(command.spawn()?);
            let stdout=admission.child().stdout.as_mut()
                .ok_or_else(||io::Error::other("group guardian handshake pipe missing"))?;
            let ready=bounded_guardian_banner(
                stdout,std::time::Duration::from_secs(3))?;
            if ready!="READY\n"{
                return Err(io::Error::other("group guardian failed private-group handshake"))
            }
            // Drop the handshake pipe; only the private stdin lifetime pipe
            // remains. A premature helper exit is an admission failure.
            admission.child().stdout.take();
            let stdin=admission.child().stdin.take()
                .ok_or_else(||io::Error::other("group guardian lifetime pipe missing"))?;
            if admission.child().try_wait()?.is_some(){
                return Err(io::Error::other("group guardian died before worker admission"))
            }
            let child=admission.admit();
            Ok(Some(Self{child,stdin:Some(stdin),stopped:false,verified_signal:false}))
        }
        pub fn group_id(&self)->i32{self.child.id() as i32}
        /// A successful handshake proves private PGID ownership; SIGKILL
        /// of the guardian proves group-wide signalling, not that separately
        /// adopted zombie descendants have been wait()-reaped by Lu.
        pub fn stop_and_verify_signal(&mut self)->io::Result<()>{
            use std::{os::unix::process::ExitStatusExt,time::{Duration,Instant}};
            if self.stopped{
                return if self.verified_signal{Ok(())}
                else{Err(io::Error::other("Group guardian stop was not verified"))}
            }
            if self.child.try_wait()?.is_some(){
                return Err(io::Error::other("group guardian died before closure"))
            }
            drop(self.stdin.take()); // EOF triggers guardian kill(0,SIGKILL)
            let deadline=Instant::now()+Duration::from_secs(3);
            loop{
                if let Some(status)=self.child.try_wait()?{
                    self.stopped=true;
                    self.verified_signal=status.signal()==Some(SIGKILL);
                    return if self.verified_signal{Ok(())}
                    else{Err(io::Error::other("group guardian did not die by group SIGKILL"))};
                }
                if Instant::now()>=deadline{
                    // Kill only the still-owned guardian, never a guessed
                    // PGID or a PID restored from an old SQLite record.
                    let _=self.child.kill();let _=self.child.wait();
                    self.stopped=true;
                    return Err(io::Error::other("group guardian did not acknowledge EOF"))
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
    #[cfg(target_os="linux")]
    impl Drop for LinuxOwnedGroupGuardian{
        fn drop(&mut self){
            if !self.stopped{let _=self.stop_and_verify_signal();}
        }
    }
    /// AGENT-01L Linux cgroup-v2 custodian, independent of Lu's process
    /// lifetime and OUTSIDE its killable worker subtree. The OS scopes are
    /// acquired from the helper's actual /proc/self/cgroup, never user/model
    /// supplied PID or cgroup path.
    /// SEC-06: positive OS receipts must exactly match the original
    /// admission identity, including the random kernel leaf nonce. A
    /// replaced, linked, oversized, relaxed-mode, or edited receipt is
    /// NOT a native cleanup witness. This is local evidence validation,
    /// not protection from another process with full profile privileges.
    #[cfg(target_os="linux")]
    fn validate_linux_cgroup_receipt(
        path:&std::path::Path,expected:&str,
    )->io::Result<()>{
        use std::{fs::OpenOptions,io::Read,
            os::unix::fs::{MetadataExt,OpenOptionsExt,PermissionsExt}};
        // Linux O_NOFOLLOW. Prevent a symlink swap between a metadata
        // inspection and opening this original host-controlled path.
        const O_NOFOLLOW:i32=0o400000;
        let mut file=OpenOptions::new().read(true)
            .custom_flags(O_NOFOLLOW).open(path)?;
        let meta=file.metadata()?;
        if !meta.is_file()||meta.len()>4096||meta.nlink()!=1
            ||meta.permissions().mode()&0o077!=0{
            return Err(io::Error::other("Unsafe original OS receipt"))
        }
        let mut bytes=Vec::new();
        file.take(4097).read_to_end(&mut bytes)?;
        if bytes!=expected.as_bytes(){
            return Err(io::Error::other(
                "Kernel cgroup witness identity or original nonce mismatch"))
        }
        Ok(())
    }
    #[cfg(target_os="linux")]
    pub struct LinuxCgroupCustodian{
        child:Child,
        stdin:Option<std::process::ChildStdin>,
        subtree:std::path::PathBuf,
        receipt:std::path::PathBuf,
        expected_receipt:String,
        stopped:bool,
        verified_empty:bool,
    }
    #[cfg(target_os="linux")]
    impl LinuxCgroupCustodian{
        pub fn spawn_if_delegated(
            profile:&std::path::Path,chat:&str,turn:&str,call:&str,
        )->io::Result<Option<Self>>{
            use std::{os::unix::fs::PermissionsExt,
                process::{Command,Stdio}};
            let valid=|v:&str,p:&str|
                v.len()==p.len()+32&&v.starts_with(p)
                    &&v[p.len()..].bytes().all(|b|
                        b.is_ascii_hexdigit()&&!b.is_ascii_uppercase());
            if !valid(chat,"c-")||!valid(turn,"g-")
                ||call.is_empty()||call.len()>128
                ||!call.bytes().all(|b|b.is_ascii_alphanumeric()
                    ||b==b'-'||b==b'_')
                ||!["-gh","-gs","-gb","-gc","-fm"]
                    .iter().any(|x|call.ends_with(x))
                ||!profile.is_absolute(){
                return Err(io::Error::other("Invalid original native cgroup scope"))
            }
            let executable=std::env::current_exe()?
                .parent().ok_or_else(||io::Error::other("No installed Lu helper directory"))?
                .join("lu-linux-cgroup-guard");
            let image=if cfg!(debug_assertions){
                std::env::var_os("LU_AGENT01L_TEST_CGROUP_GUARD")
                    .map(std::path::PathBuf::from).unwrap_or(executable)
            }else{executable};
            let meta=match std::fs::symlink_metadata(&image){
                Ok(meta)=>meta,
                Err(e) if e.kind()==io::ErrorKind::NotFound
                    &&cfg!(debug_assertions)
                    &&std::env::var_os("LU_AGENT01L_TEST_CGROUP_GUARD").is_none()=>
                    return Ok(None),
                Err(e)=>return Err(e),
            };
            if !meta.is_file()||meta.file_type().is_symlink()
                ||meta.permissions().mode()&0o111==0
                ||meta.permissions().mode()&0o022!=0{
                return Err(io::Error::other("Untrusted first-party cgroup helper"))
            }
            let receipt=profile.join(format!("lu-cgroup-os-{turn}-{call}.receipt"));
            if std::fs::symlink_metadata(&receipt).is_ok(){
                return Err(io::Error::other("Cannot replay prior Linux OS receipt"))
            }
            let mut cmd=Command::new(&image);
            cmd.env_clear().args([profile.as_os_str(),
                std::ffi::OsStr::new(chat),std::ffi::OsStr::new(turn),
                std::ffi::OsStr::new(call)])
                .stdin(Stdio::piped()).stdout(Stdio::piped())
                .stderr(Stdio::null());
            // Unlike the worker, this custodian has no PDEATHSIG and stays
            // in Lu's existing parent cgroup, outside the new subgroup.
            let mut admission=AdmissionChild::new(cmd.spawn()?);
            let raw=admission.child().stdout.as_mut()
                .ok_or_else(||io::Error::other("Custodian handshake missing"))?;
            let banner=bounded_guardian_banner(
                raw,std::time::Duration::from_secs(3))?;
            admission.child().stdout.take();
            if banner=="UNAVAILABLE\n"{
                let status=bounded_guardian_exit(admission.child(),
                    std::time::Duration::from_secs(2))?;
                if !status.success(){
                    return Err(io::Error::other("Delegation detection failed"))
                }
                return Ok(None)
            }
            let Some(encoded)=banner.strip_prefix("READY\t")
                .and_then(|x|x.strip_suffix('\n')) else{
                return Err(io::Error::other("Unrecognized OS cgroup handshake"))
            };
            let group=std::path::PathBuf::from(encoded);
            if !group.is_absolute()||!group.join("cgroup.procs").exists()
                ||!group.join("cgroup.kill").exists()
                ||group.file_name().and_then(|x|x.to_str())
                    .is_none_or(|x|!x.starts_with("lu-review-")
                        ||x.len()!=42
                        ||!x[10..].bytes().all(|b|
                            b.is_ascii_digit()||(b'a'..=b'f').contains(&b))){
                return Err(io::Error::other("Invalid private cgroup identity"))
            }
            let current=std::fs::read_to_string("/proc/self/cgroup")?;
            let own:Vec<&str>=current.lines()
                .filter_map(|s|s.strip_prefix("0::")).collect();
            if own.len()!=1||!own[0].starts_with('/')||
                group.parent()!=Some(std::path::Path::new("/sys/fs/cgroup")
                    .join(own[0].trim_start_matches('/')).as_path()){
                return Err(io::Error::other("Cgroup helper left Lu's delegated root"))
            }
            let stdin=admission.child().stdin.take().ok_or_else(||{
                io::Error::other("Lifetime pipe missing")
            })?;
            if admission.child().try_wait()?.is_some(){
                return Err(io::Error::other("Custodian exited before admission"))
            }
            let nonce=group.file_name().and_then(|v|v.to_str())
                .and_then(|v|v.strip_prefix("lu-review-"))
                .ok_or_else(||io::Error::other("Invalid kernel cgroup nonce"))?;
            let expected_receipt=format!(
                "LU_CGROUP_V2_EMPTY_V1\nCHAT={chat}\nTURN={turn}\nCALL={call}\nNONCE={nonce}\nPOPULATED=0\n"
            );
            let child=admission.admit();
            Ok(Some(Self{
                child,stdin:Some(stdin),subtree:group,receipt,expected_receipt,
                stopped:false,verified_empty:false,
            }))
        }
        pub fn subtree(&self)->&std::path::Path{&self.subtree}
        // This accessor exists only in a destructive, first-party OS test
        // binary. Production callers cannot choose or signal a PID.
        #[cfg(feature="e1-p08-crash-fixture")]
        pub fn fixture_guardian_pid(&self)->u32{self.child.id()}
        /// A live custodian is required during execution, not merely at
        /// shutdown. Losing it invalidates the cgroup lifetime assurance
        /// even if the worker itself remains alive.
        pub fn require_live_guardian(&mut self)->io::Result<()>{
            if self.child.try_wait()?.is_some(){
                return Err(io::Error::other(
                    "P0_SEC_CGROUP_GUARDIAN_LOST_BEFORE_SHUTDOWN"))
            }
            Ok(())
        }
        /// Independently read /proc after spawn: the worker must actually
        /// inhabit the exact kernel subtree. The custodian itself must not.
        /// A transient observation does not promise against later migration
        /// to a writable ancestor cgroup; that gate remains separate.
        pub fn verify_admitted_worker(&mut self,pid:u32)->io::Result<()>{
            self.require_live_guardian()?;
            let rel=self.subtree.strip_prefix("/sys/fs/cgroup")
                .map_err(|_|io::Error::other("Untrusted cgroup subtree"))?;
            let rel=rel.to_str().ok_or_else(||io::Error::other(
                "Non UTF-8 kernel cgroup path"))?;
            let expected=format!("0::/{rel}");
            let group_of=|pid:u32|->io::Result<String>{
                let actual=std::fs::read_to_string(format!("/proc/{pid}/cgroup"))?;
                let lines=actual.lines().collect::<Vec<_>>();
                if lines.len()!=1||!lines[0].starts_with("0::/"){
                    return Err(io::Error::other("Invalid kernel cgroup member"))
                }
                Ok(lines[0].to_owned())
            };
            if group_of(pid)?!=expected{
                return Err(io::Error::other("Worker was not admitted to owned kernel cgroup"))
            }
            let guardian=group_of(self.child.id())?;
            if guardian==expected||guardian.starts_with(&(expected+"/")){
                return Err(io::Error::other("Custodian incorrectly entered killable subtree"))
            }
            self.require_live_guardian()
        }
        /// Close the exact lifetime pipe. The separate custodian performs
        /// cgroup.kill, checks recursive cgroup.events populated 0 and writes
        /// a durable receipt. Lu waits for its completion; a missing ack is
        /// uncertainty, NOT success or a fresh authority grant.
        pub fn stop_and_verify(&mut self)->io::Result<()>{
            use std::time::{Duration,Instant};
            if self.stopped{
                return if self.verified_empty{Ok(())}
                else{Err(io::Error::other("Cgroup closure previously failed verification"))}
            }
            if self.child.try_wait()?.is_some(){
                return Err(io::Error::other("Linux custodian died before OS cleanup"))
            }
            drop(self.stdin.take());
            let deadline=Instant::now()+Duration::from_secs(12);
            loop{
                if let Some(status)=self.child.try_wait()?{
                    self.stopped=true;
                    if !status.success(){
                        return Err(io::Error::other("Kernel cgroup closure not verified"))
                    }
                    validate_linux_cgroup_receipt(
                        &self.receipt,&self.expected_receipt
                    )?;
                    self.verified_empty=true;
                    return Ok(())
                }
                if Instant::now()>=deadline{
                    let _=self.child.kill();let _=self.child.wait();
                    self.stopped=true;
                    return Err(io::Error::other("Cgroup custodian cleanup timed out"))
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
    #[cfg(target_os="linux")]
    impl Drop for LinuxCgroupCustodian{
        fn drop(&mut self){
            if !self.stopped{let _=self.stop_and_verify();}
        }
    }
    /// Enter the kernel-created private subgroup in the child pre_exec.
    /// This occurs BEFORE exec of the reviewed binary: even a malicious
    /// executable cannot fork or setsid before the cgroup membership exists.
    /// The host-only original /proc delegation checks happen beforehand.
    #[cfg(target_os="linux")]
    pub fn bind_worker_to_cgroup_before_exec(
        cmd:&mut std::process::Command,group:&LinuxCgroupCustodian
    )->io::Result<()>{
        use std::os::{unix::{ffi::OsStrExt,process::CommandExt},raw::c_void};
        unsafe extern "C"{
            fn open(path:*const std::os::raw::c_char,flags:i32,...)->i32;
            fn write(fd:i32,buf:*const c_void,size:usize)->isize;
            fn close(fd:i32)->i32;
        }
        const O_WRONLY:i32=1;
        const O_CLOEXEC:i32=0o2000000;
        const O_NOFOLLOW:i32=0o400000;
        let path=group.subtree.join("cgroup.procs");
        let name=CString::new(path.as_os_str().as_bytes())
            .map_err(|_|io::Error::other("Invalid trusted cgroup.procs path"))?;
        // SAFETY: child pre_exec uses only raw syscalls and a preallocated
        // immutable CString. "0" is the kernel's current-process selector,
        // never a model-selected PID. If migration fails, exec is refused.
        unsafe{cmd.pre_exec(move||{
            let fd=open(name.as_ptr(),O_WRONLY|O_CLOEXEC|O_NOFOLLOW);
            if fd<0{return Err(io::Error::last_os_error())}
            let actual=write(fd,b"0".as_ptr().cast(),1);
            let closed=close(fd);
            if actual!=1||closed!=0{
                return Err(io::Error::last_os_error())
            }
            Ok(())
        });}
        Ok(())
    }
    #[cfg(target_os="linux")]
    unsafe extern "C"{
        fn renameat2(oldfd:i32,old:*const std::ffi::c_char,
            newfd:i32,new:*const std::ffi::c_char,flags:u32)->i32;
        fn prctl(option:i32,arg2:usize,arg3:usize,arg4:usize,arg5:usize)->i32;
        fn getppid()->i32;
        fn setrlimit(resource:u32,lim:*const Rlimit)->i32;
    }
    #[cfg(target_os="linux")]
    #[repr(C)]
    struct Rlimit{soft:u64,hard:u64}
    /// No orphan decoder after an abnormal Lu host exit. The immediate PID
    /// recheck closes the race between child spawn and PR_SET_PDEATHSIG.
    #[cfg(target_os="linux")]
    pub fn bind_archive_parent(expected_pid:u32)->io::Result<()>{
        const PR_SET_PDEATHSIG:i32=1;
        const RLIMIT_AS:u32=9;
        let expected=i32::try_from(expected_pid)
            .map_err(|_|io::Error::other("invalid archive parent PID"))?;
        if expected<=1{return Err(io::Error::other("invalid archive parent"))}
        // SAFETY: fixed Linux integer-only prctl ABI.
        if unsafe{prctl(PR_SET_PDEATHSIG,SIGKILL as usize,0,0,0)}!=0{
            return Err(io::Error::last_os_error())
        }
        let limit=Rlimit{soft:1024*1024*1024,hard:1024*1024*1024};
        // SAFETY: repr(C) initialized 64-bit Linux rlimit structure.
        if unsafe{setrlimit(RLIMIT_AS,&limit)}!=0{
            return Err(io::Error::last_os_error())
        }
        // SAFETY: getppid takes no pointers and cannot mutate memory.
        if unsafe{getppid()}!=expected{
            return Err(io::Error::other("Lu parent died before worker admission"))
        }
        Ok(())
    }
    /// Guard any trusted native worker *before exec* against a crash of
    /// its Lu parent. PR_SET_PDEATHSIG is inherited by neither children nor
    /// grandchildren, so our host also creates a process group and terminates
    /// that group on orderly cancellation. Scanners additionally get a hard
    /// Linux address-space quota. This wrapper is the only unsafe pre_exec
    /// boundary; Lu Core remains #![deny(unsafe_code)].
    #[cfg(target_os="linux")]
    pub fn bind_owned_worker_pre_exec(
        command:&mut std::process::Command,
        expected_parent:u32,
        address_space_limit:Option<u64>,
    )->io::Result<()>{
        use std::os::unix::process::CommandExt;
        const PR_SET_PDEATHSIG:i32=1;
        const RLIMIT_AS:u32=9;
        let expected=i32::try_from(expected_parent)
            .map_err(|_|io::Error::from_raw_os_error(22))?;
        if expected<=1{return Err(io::Error::from_raw_os_error(22))}
        if address_space_limit.is_some_and(|v|v<256*1024*1024||v>4*1024*1024*1024){
            return Err(io::Error::from_raw_os_error(22))
        }
        // SAFETY: pre_exec executes in the forked child with only integer
        // syscalls and POD buffers, no allocation, locks or async-signal-
        // unsafe Rust work. Any failure aborts exec and the host fails closed.
        unsafe{
            command.pre_exec(move||{
                if prctl(PR_SET_PDEATHSIG,SIGKILL as usize,0,0,0)!=0{
                    return Err(io::Error::last_os_error())
                }
                if getppid()!=expected{
                    return Err(io::Error::from_raw_os_error(10))
                }
                if let Some(bytes)=address_space_limit{
                    let limit=Rlimit{soft:bytes,hard:bytes};
                    if setrlimit(RLIMIT_AS,&limit)!=0{
                        return Err(io::Error::last_os_error())
                    }
                }
                Ok(())
            });
        }
        Ok(())
    }
    /// Linux native no-overwrite move using a pinned open directory FD.
    /// This is one atomic namespace transition; no hardlink/unlink gap.
    #[cfg(target_os="linux")]
    pub fn rename_noreplace(dir:&File,source:&str,target:&str)->io::Result<()>{
        rename_between_noreplace(dir,source,dir,target)
    }
    #[cfg(target_os="linux")]
    pub fn rename_between_noreplace(source_dir:&File,source:&str,target_dir:&File,target:&str)->io::Result<()>{
        let from=CString::new(source).map_err(|_|io::Error::other("invalid name"))?;
        let to=CString::new(target).map_err(|_|io::Error::other("invalid name"))?;
        // SAFETY: both open directory FDs live during this call; both
        // CStrings remain allocated and NUL-terminated for the syscall.
        let result=unsafe{renameat2(
            source_dir.as_raw_fd(),from.as_ptr(),target_dir.as_raw_fd(),to.as_ptr(),1
        )};
        if result!=0{Err(io::Error::last_os_error())}else{Ok(())}
    }
    pub fn kill_owned_group(child:&Child)->io::Result<()>{
        let pid=i32::try_from(child.id()).map_err(|_|io::Error::other("invalid worker pid"))?;
        if pid<=1 {return Err(io::Error::other("invalid worker process group"));}
        // SAFETY: negative pid targets only the independently created process
        // group whose leader is our trusted Child, not an arbitrary model PID.
        if unsafe{kill(-pid,SIGKILL)}!=0{
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}

#[cfg(all(windows,test))]
mod agent01l_kernel_job_tests{
    use super::Job;
    use std::{process::{Command,Stdio},time::{Duration,Instant},
        os::windows::process::CommandExt};
    #[test]
    fn unassigned_job_cannot_forge_an_empty_process_tree(){
        let job=Job::new().unwrap();
        assert!(job.terminate_and_verify_empty().is_err(),
            "An empty never-assigned Job is not an OS worker custody witness");
    }
    #[test]
    fn native_job_terminates_descendant_cmd_tree_and_queries_kernel_empty(){
        let shell=std::env::var_os("ComSpec").unwrap_or(
            std::ffi::OsString::from(r"C:\Windows\System32\cmd.exe")
        );
        // Launch frozen and bind before first instruction. cmd starts ping
        // as a separate descendant; only the live Job handle can attest its
        // kernel-observed empty process count. No model PID is selected.
        let mut child=Command::new(shell)
            .args(["/D","/C","ping -n 20 127.0.0.1 >NUL"])
            .creation_flags(0x0800_0000|0x0000_0004)
            .stdin(Stdio::null()).stdout(Stdio::null())
            .stderr(Stdio::null()).spawn().unwrap();
        let job=Job::new().unwrap();
        if let Err(e)=job.assign_and_resume_suspended(&child){
            let _=child.kill();let _=child.wait();
            panic!("Reviewed worker Job admission failed: {e}");
        }
        // Do not assume ping spawned because 100 ms elapsed: query THIS Job's
        // live kernel membership until both the shell and its child exist.
        let deadline=Instant::now()+Duration::from_secs(5);
        loop{
            if job.kernel_live_count_for_test().unwrap()>=2{break;}
            if Instant::now()>=deadline{
                let _=child.kill();let _=child.wait();
                panic!("Kernel never observed an actual nested Job descendant");
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let proof=job.terminate_and_verify_empty().unwrap();
        assert_eq!(proof.method(),"WINDOWS_JOB_ACTIVE_ZERO_V1");
        let _=child.wait().unwrap();
        // A second observation must remain possible while owning the same
        // now-empty kernel Job, but cannot be transferred to another Job.
        assert_eq!(job.terminate_and_verify_empty().unwrap().method(),
            "WINDOWS_JOB_ACTIVE_ZERO_V1");
    }
}
