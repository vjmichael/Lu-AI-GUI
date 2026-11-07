//! Wave D2-W1: Windows Job Object containment acceptance fixture.
//! This binary is not the IDE worker. It accepts ONLY built-in child modes,
//! never project paths, commands, scripts, arguments, or authorization tokens.
//! It is isolated from the IDE native/Tauri dependency graph.
#[cfg(not(windows))]
fn main() {
    eprintln!("Windows Job Object probe requires Windows; no fallback");
    std::process::exit(2);
}

#[cfg(windows)]
mod windows {
    use std::{
        ffi::OsStr,
        io,
        mem::size_of,
        os::windows::ffi::OsStrExt,
        ptr, thread,
        time::{Duration, Instant},
    };
    use windows_sys::Win32::{
        Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT},
        System::{
            JobObjects::{
                AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
                SetInformationJobObject, TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
                JOB_OBJECT_LIMIT_ACTIVE_PROCESS, JOB_OBJECT_LIMIT_JOB_MEMORY,
                JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            },
            Threading::{
                CreateProcessW, GetExitCodeProcess, OpenProcess, ResumeThread, TerminateProcess,
                WaitForSingleObject, CREATE_NO_WINDOW, CREATE_SUSPENDED,
                CREATE_UNICODE_ENVIRONMENT, PROCESS_INFORMATION, PROCESS_QUERY_LIMITED_INFORMATION,
                PROCESS_SYNCHRONIZE, STARTUPINFOW,
            },
        },
    };

    const JOB_MEMORY_BYTES: usize = 128 * 1024 * 1024;
    const MAX_PROCESSES: u32 = 4;

    fn win_error(action: &'static str) -> io::Error {
        let inner = io::Error::last_os_error();
        io::Error::new(inner.kind(), format!("{action}: {inner}"))
    }

    struct Handle(HANDLE);
    impl Handle {
        fn from_raw(handle: HANDLE, action: &'static str) -> io::Result<Self> {
            if handle.is_null() {
                Err(win_error(action))
            } else {
                Ok(Self(handle))
            }
        }
    }
    impl Drop for Handle {
        fn drop(&mut self) {
            // SAFETY: all owned handles were returned by Win32 handle-producing
            // calls, and each is wrapped exactly once; no raw handle escapes.
            unsafe {
                CloseHandle(self.0);
            }
        }
    }

    enum FixedMode {
        Exit,
        Sleep,
        SpawnGrandchild { marker_owner_pid: u32 },
    }
    impl FixedMode {
        fn args(&self) -> String {
            match self {
                Self::Exit => "--fixture-exit".into(),
                Self::Sleep => "--fixture-sleep".into(),
                Self::SpawnGrandchild { marker_owner_pid } => {
                    format!("--fixture-descendant {marker_owner_pid}")
                }
            }
        }
    }

    struct Contained {
        // Closing job kills all processes, including descendants.
        job: Handle,
        process: Handle,
        thread: Handle,
        pid: u32,
    }

    impl Contained {
        fn launch(mode: FixedMode) -> io::Result<Self> {
            let exe = std::env::current_exe()?;
            if !exe.is_absolute() || !exe.is_file() {
                return Err(io::Error::other("fixed acceptance executable unavailable"));
            }
            let exe_wide = wide(exe.as_os_str());
            let mut cmd = wide(OsStr::new(&format!(
                "\"{}\" {}",
                exe.display(),
                mode.args()
            )));
            // A narrowly populated, double-NUL-terminated environment block.
            // The fixed fixture needs the same temporary directory in parent/child.
            // No credentials, HOME, SSH, tokens or project environment are inherited.
            let temp = std::env::temp_dir();
            let system_root =
                std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_owned());
            let environment: Vec<u16> = format!(
                "SystemRoot={system_root}\0TEMP={}\0TMP={}\0\0",
                temp.display(),
                temp.display()
            )
            .encode_utf16()
            .collect();
            let mut startup = STARTUPINFOW::default();
            startup.cb = size_of::<STARTUPINFOW>() as u32;
            let mut process_info = PROCESS_INFORMATION::default();

            // SAFETY: all Win32 pointers reference live fixed buffers; names,
            // command tokens, and arguments are trusted compile-time fixtures.
            // CREATE_SUSPENDED prevents child code before Job assignment.
            let launched = unsafe {
                CreateProcessW(
                    exe_wide.as_ptr(),
                    cmd.as_mut_ptr(),
                    ptr::null(),
                    ptr::null(),
                    0,
                    CREATE_SUSPENDED | CREATE_NO_WINDOW | CREATE_UNICODE_ENVIRONMENT,
                    environment.as_ptr().cast(),
                    ptr::null(),
                    &startup,
                    &mut process_info,
                )
            };
            if launched == 0 {
                return Err(win_error("CreateProcessW suspended"));
            }
            let process = Handle::from_raw(process_info.hProcess, "process handle")?;
            let thread = match Handle::from_raw(process_info.hThread, "thread handle") {
                Ok(value) => value,
                Err(error) => {
                    unsafe {
                        TerminateProcess(process.0, 1);
                    }
                    return Err(error);
                }
            };

            // SAFETY: a private unnamed Job Object with null attributes is
            // created and owned by this supervisor probe, not project code.
            let job = match Handle::from_raw(
                unsafe { CreateJobObjectW(ptr::null(), ptr::null()) },
                "CreateJobObjectW",
            ) {
                Ok(value) => value,
                Err(error) => {
                    unsafe {
                        TerminateProcess(process.0, 1);
                    }
                    return Err(error);
                }
            };
            let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
                | JOB_OBJECT_LIMIT_JOB_MEMORY
                | JOB_OBJECT_LIMIT_ACTIVE_PROCESS;
            limits.BasicLimitInformation.ActiveProcessLimit = MAX_PROCESSES;
            limits.JobMemoryLimit = JOB_MEMORY_BYTES;

            // SAFETY: the limit buffer matches JobObjectExtendedLimitInformation
            // in type and size and stays valid throughout the OS call.
            let configured = unsafe {
                SetInformationJobObject(
                    job.0,
                    JobObjectExtendedLimitInformation,
                    (&raw const limits).cast(),
                    size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                )
            };
            if configured == 0 {
                let error = win_error("SetInformationJobObject");
                unsafe {
                    TerminateProcess(process.0, 1);
                }
                return Err(error);
            }

            // SAFETY: child remains suspended; if assignment fails it is killed
            // before any first instruction. No fallback to uncontained execution.
            if unsafe { AssignProcessToJobObject(job.0, process.0) } == 0 {
                let error = win_error("AssignProcessToJobObject");
                unsafe {
                    TerminateProcess(process.0, 1);
                }
                return Err(error);
            }
            // SAFETY: the thread is the original primary thread of a process
            // successfully assigned to the configured Job Object.
            if unsafe { ResumeThread(thread.0) } == u32::MAX {
                return Err(win_error("ResumeThread"));
            }

            Ok(Self {
                job,
                process,
                thread,
                pid: process_info.dwProcessId,
            })
        }

        fn await_exit(&self, deadline: Duration) -> io::Result<Option<u32>> {
            let millis = u32::try_from(deadline.as_millis()).unwrap_or(u32::MAX);
            // SAFETY: self.process is a valid process handle owned by self.
            let result = unsafe { WaitForSingleObject(self.process.0, millis) };
            if result == WAIT_TIMEOUT {
                return Ok(None);
            }
            if result != WAIT_OBJECT_0 {
                return Err(win_error("WaitForSingleObject"));
            }
            let mut code = 0u32;
            // SAFETY: output is a valid mutable pointer and process handle is
            // held alive for the full call.
            if unsafe { GetExitCodeProcess(self.process.0, &mut code) } == 0 {
                return Err(win_error("GetExitCodeProcess"));
            }
            Ok(Some(code))
        }

        fn terminate_tree(&self) -> io::Result<()> {
            // SAFETY: Job handle is valid and the OS terminates all member
            // processes atomically with their assigned descendants.
            if unsafe { TerminateJobObject(self.job.0, 77) } == 0 {
                return Err(win_error("TerminateJobObject"));
            }
            if self.await_exit(Duration::from_secs(5))?.is_none() {
                return Err(io::Error::other(
                    "Job child failed to terminate on deadline",
                ));
            }
            Ok(())
        }
    }

    fn wide(value: &OsStr) -> Vec<u16> {
        value.encode_wide().chain(std::iter::once(0)).collect()
    }

    fn marker_path(owner_pid: u32) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("lu-ide-win-job-descendant-{owner_pid}.pid"))
    }

    fn child_mode(args: &[String]) -> io::Result<bool> {
        match args.first().map(String::as_str) {
            Some("--fixture-exit") if args.len() == 1 => Ok(true),
            Some("--fixture-sleep") if args.len() == 1 => {
                thread::sleep(Duration::from_secs(120));
                Ok(true)
            }
            Some("--fixture-descendant") if args.len() == 2 => {
                let owner: u32 = args[1]
                    .parse()
                    .map_err(|_| io::Error::other("invalid fixed PID parameter"))?;
                let exe = std::env::current_exe()?;
                let descendant = std::process::Command::new(exe)
                    .arg("--fixture-sleep")
                    .env_clear()
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()?;
                std::fs::write(marker_path(owner), descendant.id().to_string())?;
                thread::sleep(Duration::from_secs(120));
                Ok(true)
            }
            None => Ok(false),
            _ => Err(io::Error::other("unsupported probe child mode")),
        }
    }

    fn verify_descendant_terminated(pid: u32) -> io::Result<()> {
        // SAFETY: this handle grants only query/wait rights to the identified
        // fixture child PID. If it was already reaped, OpenProcess fails.
        let raw = unsafe {
            OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
                0,
                pid,
            )
        };
        if raw.is_null() {
            return Ok(());
        }
        let opened = Handle(raw);
        // SAFETY: opened is a live query/synchronize process handle.
        let result = unsafe { WaitForSingleObject(opened.0, 5000) };
        if result != WAIT_OBJECT_0 {
            return Err(io::Error::other(
                "descendant still running after Job cancellation",
            ));
        }
        Ok(())
    }

    fn run_tests() -> io::Result<()> {
        // Admission: configured job before resuming the fixed test process.
        let clean = Contained::launch(FixedMode::Exit)?;
        if clean.await_exit(Duration::from_secs(10))? != Some(0) {
            return Err(io::Error::other("contained zero-exit fixture failed"));
        }
        println!("PASS: suspended spawn -> Job assignment -> successful completion");
        drop(clean);

        let stuck = Contained::launch(FixedMode::Sleep)?;
        if stuck.await_exit(Duration::from_millis(200))?.is_some() {
            return Err(io::Error::other("sleep fixture exited before deadline"));
        }
        stuck.terminate_tree()?;
        println!("PASS: deadline -> full Job termination");
        drop(stuck);

        let owner = std::process::id();
        let marker = marker_path(owner);
        let _ = std::fs::remove_file(&marker);
        let tree = Contained::launch(FixedMode::SpawnGrandchild {
            marker_owner_pid: owner,
        })?;
        let began = Instant::now();
        let descendant_pid = loop {
            if let Ok(value) = std::fs::read_to_string(&marker) {
                let id: u32 = value
                    .parse()
                    .map_err(|_| io::Error::other("invalid descendant marker"))?;
                break id;
            }
            if began.elapsed() >= Duration::from_secs(8) {
                return Err(io::Error::other("contained descendant did not start"));
            }
            thread::sleep(Duration::from_millis(25));
        };
        tree.terminate_tree()?;
        verify_descendant_terminated(descendant_pid)?;
        std::fs::remove_file(marker)?;
        println!("PASS: descendant process killed with parent Job");
        drop(tree);
        println!(
            "PROBE ONLY: no IDE task, build script, arbitrary source or installed desktop tested"
        );
        Ok(())
    }

    // A separate PowerShell process captures kernel handles to both
    // descendants *before* forcibly killing this supervisor. The probe
    // itself cannot certify its own post-crash cleanup.
    fn external_crash_fixture() -> io::Result<()> {
        let owner = std::process::id();
        let descendant_marker = marker_path(owner);
        let witness_marker = std::env::temp_dir().join(format!("lu-ide-d7b-crash-{owner}.txt"));
        let _ = std::fs::remove_file(&descendant_marker);
        let _ = std::fs::remove_file(&witness_marker);
        let tree = Contained::launch(FixedMode::SpawnGrandchild {
            marker_owner_pid: owner,
        })?;
        let start = Instant::now();
        let grandchild_id: u32 = loop {
            if let Ok(value) = std::fs::read_to_string(&descendant_marker) {
                break value
                    .parse()
                    .map_err(|_| io::Error::other("invalid grandchild PID"))?;
            }
            if start.elapsed() > Duration::from_secs(10) {
                return Err(io::Error::other("grandchild marker missing"));
            }
            thread::sleep(Duration::from_millis(25));
        };
        // Only process IDs are written; the independent observer must open
        // and retain OS process handles before killing this supervisor.
        std::fs::write(
            &witness_marker,
            format!("{}\n{}\n{}\n", owner, tree.pid, grandchild_id),
        )?;
        // The observer is required to terminate this process externally.
        // The owner never calls TerminateJobObject in this scenario.
        thread::sleep(Duration::from_secs(120));
        Err(io::Error::other(
            "independent crash observer did not terminate owner",
        ))
    }

    pub fn main_impl() -> io::Result<()> {
        let args: Vec<String> = std::env::args().skip(1).collect();
        // This fixed parent-only hostile mode is not a child mode. Dispatch
        // it before rejecting unknown child arguments; the independent
        // PowerShell observer must see two live descendant handles before
        // killing this owner, not a false early-exit failure.
        if args.len() == 1 && args[0] == "--fixture-external-crash" {
            return external_crash_fixture();
        }
        if child_mode(&args)? {
            return Ok(());
        }
        if !args.is_empty() {
            return Err(io::Error::other("no caller-selected actions supported"));
        }
        run_tests()
    }
}

#[cfg(windows)]
fn main() {
    if let Err(error) = windows::main_impl() {
        eprintln!("Windows Job Object containment probe FAILED: {error}");
        std::process::exit(1);
    }
}
