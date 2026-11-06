//! D7D native crash acceptance fixture. Fixed self-spawned sleeper only;
//! no project code, arbitrary command, shell, Tauri or Lu Core entrypoint.
#[cfg(not(all(windows, feature = "protected-recovery-service")))]
fn main() {
    eprintln!("DENIED: Windows protected-recovery-service feature required");
    std::process::exit(2);
}
#[cfg(all(windows, feature = "protected-recovery-service"))]
mod native {
    use lu_ide_execution_supervisor::{
        windows_broker::TrustedBroker,
        windows_service_client::{ServiceLease, release_after_verified_recovery},
        windows_service_security::{ws, JobSecurity},
    };
    use rand::{rngs::OsRng, RngCore};
    use std::{
        ffi::OsStr, fs, mem::size_of, os::windows::ffi::OsStrExt,
        path::PathBuf, ptr, thread, time::Duration,
    };
    use windows_sys::Win32::{
        Foundation::{CloseHandle, GetLastError, HANDLE, ERROR_ALREADY_EXISTS, WAIT_TIMEOUT},
        System::{
            JobObjects::{AssignProcessToJobObject, CreateJobObjectW, OpenJobObjectW,
                QueryInformationJobObject, SetInformationJobObject,
                JobObjectBasicAccountingInformation, JobObjectExtendedLimitInformation,
                JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
                JOB_OBJECT_LIMIT_ACTIVE_PROCESS, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE},
            Threading::{CreateProcessW, ResumeThread, TerminateProcess,
                WaitForSingleObject, CREATE_NO_WINDOW, CREATE_SUSPENDED,
                PROCESS_INFORMATION, STARTUPINFOW},
        },
    };
    struct Handle(HANDLE);
    impl Handle {
        fn new(h: HANDLE) -> Result<Self, String> {
            if h.is_null() { Err("DENIED: missing kernel handle".into()) } else { Ok(Self(h)) }
        }
    }
    impl Drop for Handle {
        fn drop(&mut self) { unsafe { CloseHandle(self.0); } }
    }
    fn wide(s: &OsStr) -> Vec<u16> { s.encode_wide().chain([0]).collect() }
    fn valid_id(s: &str) -> bool {
        s.len() == 36 && s.starts_with("job-")
            && s[4..].bytes().all(|b| b.is_ascii_hexdigit())
    }
    fn valid_sha(s: &str) -> bool {
        s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
    }
    fn config(path: &str, hash: &str) -> Result<TrustedBroker, String> {
        let exe = PathBuf::from(path);
        if !exe.is_absolute() || !valid_sha(hash) {
            return Err("DENIED: pinned service path/hash required".into());
        }
        Ok(TrustedBroker {
            executable: exe, expected_sha256: hash.to_ascii_lowercase(),
            private_witness_directory: PathBuf::from(r"C:\ProgramData\LuIDERecoveryBroker"),
        })
    }
    fn job_name(id: &str, hash: &str) -> Result<Vec<u16>, String> {
        if !valid_id(id) || !valid_sha(hash) { return Err("DENIED: bad job scope".into()); }
        Ok(ws(&format!("Global\\LuIdeD7-{id}-{}", &hash[..16])))
    }
    pub fn run() -> Result<(), String> {
        let args: Vec<String> = std::env::args().skip(1).collect();
        if args.len() == 1 && args[0] == "--sleep" {
            thread::sleep(Duration::from_secs(120));
            return Ok(());
        }
        if args.len() == 5 && args[0] == "--recover" {
            let (id, plan) = (&args[3], "a".repeat(64));
            if !valid_id(id) || args[4] != plan { return Err("DENIED: bad recovered plan".into()); }
            // Independent restarted process, not a replayed JSON file,
            // opens the exact old kernel object and confirms it is empty.
            let job = Handle::new(unsafe {
                OpenJobObjectW(0x0004, 0, job_name(id, &plan)?.as_ptr())
            })?;
            let mut status = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
            if unsafe { QueryInformationJobObject(
                job.0, JobObjectBasicAccountingInformation,
                (&mut status as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
                size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                ptr::null_mut(),
            ) } == 0 || status.ActiveProcesses != 0 {
                return Err("DENIED: old kernel Job was not independently verified empty".into());
            }
            release_after_verified_recovery(id, &plan,
                &config(&args[1], &args[2])?,
            ).map_err(|e| e.code().to_owned())?;
            println!("PASS: restarted host kernel proof + protected fsynced service recovery ACK");
            return Ok(());
        }
        if args.len() != 4 || !matches!(args[0].as_str(), "--owner" | "--owner-watch") {
            return Err("DENIED: fixed --owner/--recover/--sleep modes only".into());
        }
        let cfg = config(&args[1], &args[2])?;
        let marker = PathBuf::from(&args[3]);
        if !marker.is_absolute() || marker.exists() {
            return Err("DENIED: exclusive absolute observer marker required".into());
        }
        let mut bytes = [0u8; 16];
        OsRng.fill_bytes(&mut bytes);
        let id = format!("job-{}",
            bytes.iter().map(|b| format!("{b:02x}")).collect::<String>());
        let plan = "a".repeat(64);
        let acl = JobSecurity::new().map_err(|e| e.code())?;
        let job = Handle::new(unsafe {
            CreateJobObjectW(&acl.attributes, job_name(&id, &plan)?.as_ptr())
        })?;
        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            return Err("DENIED: stale named Job".into());
        }
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags =
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_ACTIVE_PROCESS;
        limits.BasicLimitInformation.ActiveProcessLimit = 3;
        if unsafe { SetInformationJobObject(
            job.0, JobObjectExtendedLimitInformation, (&raw const limits).cast(),
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        ) } == 0 {
            return Err("DENIED: Job limits not installed".into());
        }
        // The independent SCM service opens the empty Job, writes and fsyncs
        // protected READY, then replies. Only afterward can child be created.
        let mut lease = ServiceLease::begin(job.0, &id, &plan, &cfg)
            .map_err(|e| format!("service admit denied: {}", e.code()))?;
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let executable = wide(exe.as_os_str());
        let mut command = wide(OsStr::new(&format!("\"{}\" --sleep", exe.display())));
        let mut startup = STARTUPINFOW::default();
        startup.cb = size_of::<STARTUPINFOW>() as u32;
        let mut process = PROCESS_INFORMATION::default();
        if unsafe { CreateProcessW(
            executable.as_ptr(), command.as_mut_ptr(), ptr::null(), ptr::null(),
            0, CREATE_SUSPENDED | CREATE_NO_WINDOW, ptr::null(), ptr::null(),
            &startup, &mut process,
        ) } == 0 {
            return Err("DENIED: fixed child creation failed".into());
        }
        let child = Handle::new(process.hProcess)?;
        let child_thread = Handle::new(process.hThread)?;
        if unsafe { AssignProcessToJobObject(job.0, child.0) } == 0 {
            unsafe { TerminateProcess(child.0, 1); }
            return Err("DENIED: child not contained before resume".into());
        }
        if unsafe { ResumeThread(child_thread.0) } == u32::MAX
            || unsafe { WaitForSingleObject(child.0, 0) } != WAIT_TIMEOUT {
            return Err("DENIED: contained child did not run".into());
        }
        fs::write(&marker, format!("{}\n{}\n{}\n{}\n",
            std::process::id(), process.dwProcessId, id, plan,
        )).map_err(|e| e.to_string())?;
        if args[0] == "--owner-watch" {
            // Exercise the same fail-closed service-liveness rule that the
            // production native worker checks between AppContainer waits.
            // Losing the sole trusted service never grants a completion
            // receipt: the owner kills the Job and refuses further work.
            for _ in 0..250 {
                if !lease.alive() {
                    if unsafe { windows_sys::Win32::System::JobObjects::TerminateJobObject(
                        job.0, 77,
                    ) } == 0 ||
                        unsafe { WaitForSingleObject(child.0, 8000) } != 0 {
                        return Err("DENIED: service-loss Job tree not reaped".into());
                    }
                    fs::write(marker.with_extension("watchdog"), b"service_lost_job_reaped")
                        .map_err(|e| e.to_string())?;
                    return Ok(());
                }
                thread::sleep(Duration::from_millis(40));
            }
            return Err("DENIED: service-loss watchdog never observed failure".into());
        }
        // External PowerShell observer MUST kill this process. A spontaneous
        // exit means the crash test failed, not that cleanup was proven.
        thread::sleep(Duration::from_secs(120));
        Err("DENIED: crash injector never killed owner".into())
    }
}
#[cfg(all(windows, feature = "protected-recovery-service"))]
fn main() {
    if let Err(e) = native::run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
