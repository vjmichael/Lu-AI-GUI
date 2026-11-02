//! Audited Win32 FFI boundary for Lu Core, which forbids unsafe code.
//! Lifetime containment for a Windows llama-server child owned by Lu.
//!
//! The Job is created with KILL_ON_JOB_CLOSE before the child is launched.
//! The caller must assign immediately after spawn, *before* any health probe.
//! Failure to assign is fatal and requires killing/reaping the child. A normal
//! std::process::Command spawn still has a small pre-assignment race; therefore
//! this is defense-in-depth, NOT proof of fully race-free process containment.
#![cfg(windows)]

use std::{ffi::c_void, io, mem, os::windows::io::AsRawHandle, process::Child, ptr};

type Handle = *mut c_void;
const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION_CLASS: i32 = 9;
const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x0000_2000;

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
