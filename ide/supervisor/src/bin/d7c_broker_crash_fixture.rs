//! Dedicated D7C independent-broker crash acceptance fixture.
//! Intentionally fixed tests only; NO arbitrary project execution or shell.
#[cfg(not(all(windows, feature = "windows-worker")))]
fn main() {
    eprintln!("DENIED: Windows native recovery fixture not enabled");
    std::process::exit(2);
}
#[cfg(all(windows, feature = "windows-worker"))]
mod native {
    use lu_ide_execution_supervisor::windows_broker::TrustedBroker;
    use lu_ide_execution_supervisor::windows_broker::BrokerLease;
    use rand::{rngs::OsRng, RngCore};
    use std::{
        ffi::OsStr,
        fs,
        mem::size_of,
        os::windows::ffi::OsStrExt,
        path::PathBuf,
        ptr, thread,
        time::Duration,
    };
    use windows_sys::Win32::{
        Foundation::{CloseHandle, GetLastError, HANDLE, ERROR_ALREADY_EXISTS},
        System::{
            JobObjects::{AssignProcessToJobObject, CreateJobObjectW,
                SetInformationJobObject, JobObjectExtendedLimitInformation,
                JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
                JOB_OBJECT_LIMIT_ACTIVE_PROCESS, JOB_OBJECT_LIMIT_JOB_MEMORY,
                JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE},
            Threading::{CreateProcessW, ResumeThread, TerminateProcess,
                WaitForSingleObject, CREATE_NO_WINDOW, CREATE_SUSPENDED,
                PROCESS_INFORMATION, STARTUPINFOW},
        }
    };
    fn wide(s: &OsStr) -> Vec<u16> {
        s.encode_wide().chain([0]).collect()
    }
    struct Handle(HANDLE);
    impl Drop for Handle {
        fn drop(&mut self) { if !self.0.is_null() { unsafe { CloseHandle(self.0); } } }
    }
    fn denied() -> Box<dyn std::error::Error> { "DENIED: native fixed broker fixture failed".into() }
    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        let args: Vec<String> = std::env::args().skip(1).collect();
        if args.len() == 1 && args[0] == "--sleep" {
            thread::sleep(Duration::from_secs(120));
            return Ok(());
        }
        if args.len() != 5 || args[0] != "--owner" {
            return Err("DENIED: fixed --owner broker SHA witness-root marker only".into());
        }
        let broker_exe = PathBuf::from(&args[1]);
        let expected_sha = args[2].clone();
        let witness_root = PathBuf::from(&args[3]);
        let marker = PathBuf::from(&args[4]);
        if !broker_exe.is_absolute() || !witness_root.is_absolute() ||
            !marker.is_absolute() || marker.exists() {
            return Err(denied());
        }
        let mut random = [0_u8; 16];
        OsRng.fill_bytes(&mut random);
        let id = format!("job-{}", random.iter().map(|b| format!("{b:02x}")).collect::<String>());
        let hash = "a".repeat(64);
        let name = wide(OsStr::new(&format!("Local\\LuIdeD7-{id}-{}", &hash[..16])));
        let job = Handle(unsafe { CreateJobObjectW(ptr::null(), name.as_ptr()) });
        if job.0.is_null() || unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            return Err(denied());
        }
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags =
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE |
            JOB_OBJECT_LIMIT_ACTIVE_PROCESS | JOB_OBJECT_LIMIT_JOB_MEMORY;
        limits.BasicLimitInformation.ActiveProcessLimit = 3;
        limits.JobMemoryLimit = 128 * 1024 * 1024;
        if unsafe {
            SetInformationJobObject(job.0, JobObjectExtendedLimitInformation,
                (&raw const limits).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32)
        } == 0 { return Err(denied()); }
        let config = TrustedBroker {
            executable: broker_exe,
            expected_sha256: expected_sha,
            private_witness_directory: witness_root,
        };
        // This handshake creates the SECOND OS process and fsyncs its
        // independent Ready receipt BEFORE the fixed test child is spawned.
        let _lease = BrokerLease::begin(job.0, &id, &hash, &config)
            .map_err(|_| denied())?;
        let exe = std::env::current_exe()?;
        let exe_w = wide(exe.as_os_str());
        let mut command = wide(OsStr::new(&format!("\"{}\" --sleep", exe.display())));
        let mut startup = STARTUPINFOW::default();
        startup.cb = size_of::<STARTUPINFOW>() as u32;
        let mut process = PROCESS_INFORMATION::default();
        let status = unsafe {
            CreateProcessW(exe_w.as_ptr(), command.as_mut_ptr(), ptr::null(),
                ptr::null(), 0, CREATE_SUSPENDED | CREATE_NO_WINDOW,
                ptr::null(), ptr::null(), &startup, &mut process)
        };
        if status == 0 { return Err(denied()); }
        let child = Handle(process.hProcess);
        let main_thread = Handle(process.hThread);
        // Test-fixture-only fixed child; no process code can run until
        // the Job assignment succeeds. D7 proper uses atomic Job attribute.
        if unsafe { AssignProcessToJobObject(job.0, child.0) } == 0 {
            let _ = unsafe { TerminateProcess(child.0, 103) };
            let _ = unsafe { WaitForSingleObject(child.0, 5000) };
            return Err(denied());
        }
        if unsafe { ResumeThread(main_thread.0) } == u32::MAX {
            return Err(denied());
        }
        fs::write(&marker, format!(
            "{}\n{}\n{}\n{}\n", std::process::id(), process.dwProcessId, id, hash
        ))?;
        // Never perform a normal release. The external observer MUST kill
        // this owner and prove the broker independently cleaned the Job.
        thread::sleep(Duration::from_secs(120));
        Err("DENIED: independent external crash test did not terminate owner".into())
    }
}
#[cfg(all(windows, feature = "windows-worker"))]
fn main() {
    if let Err(error) = native::run() {
        eprintln!("Native broker fixture failed: {error}");
        std::process::exit(1);
    }
}
