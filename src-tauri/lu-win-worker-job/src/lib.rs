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


use std::{ffi::c_void, io, mem, os::windows::io::AsRawHandle, process::Child, ptr};

type Handle = *mut c_void;
const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION_CLASS: i32 = 9;
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
pub struct Job(Handle);

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
        let job = Self(handle);
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
        if bytes<512*1024*1024 || bytes>4*1024*1024*1024usize{
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
        Ok(())
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
pub use windows_owned::Job;

#[cfg(unix)]
pub mod unix_owned {
    use std::{io,process::Child};
    #[cfg(target_os="linux")]
    use std::{ffi::CString,fs::File,os::fd::AsRawFd};
    // POSIX killpg via negative PID. The calling host MUST have spawned this
    // child with CommandExt::process_group(0); never accept a model PID here.
    unsafe extern "C" {fn kill(pid:i32, signal:i32)->i32;}
    const SIGKILL:i32=9;
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
            })
        }
        Ok(())
    }
    /// Linux native no-overwrite move using a pinned open directory FD.
    /// This is one atomic namespace transition; no hardlink/unlink gap.
    #[cfg(target_os="linux")]
    pub fn rename_noreplace(dir:&File,source:&str,target:&str)->io::Result<()>{
        let from=CString::new(source).map_err(|_|io::Error::other("invalid name"))?;
        let to=CString::new(target).map_err(|_|io::Error::other("invalid name"))?;
        // SAFETY: both open directory FDs live during this call; both
        // CStrings remain allocated and NUL-terminated for the syscall.
        let result=unsafe{renameat2(
            dir.as_raw_fd(),from.as_ptr(),dir.as_raw_fd(),to.as_ptr(),1
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
