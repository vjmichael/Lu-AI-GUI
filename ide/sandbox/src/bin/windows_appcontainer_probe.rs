//! Wave D6 fixed Windows AppContainer + Job Object admission probe.
//!
//! This tests a security boundary with a self-copy only; no project code,
//! model-authored command, or Lu IDE/Tauri bridge is accepted.
//! A fresh no-capability AppContainer profile is created for each probe;
//! CreateProcessW starts suspended with SECURITY_CAPABILITIES, assignment
//! to a quota-limited Job Object happens BEFORE ResumeThread.
//! The child must attest TokenIsAppContainer and fail to read a host secret.
#[cfg(not(windows))]
fn main() {
    eprintln!("AppContainer admission requires Windows; no fallback");
    std::process::exit(2);
}

#[cfg(windows)]
mod windows {
    use std::{
        ffi::{OsStr, OsString},
        fs, io,
        mem::size_of,
        net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener, TcpStream},
        os::windows::ffi::{OsStrExt, OsStringExt},
        path::{Path, PathBuf},
        ptr,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };
    use windows_sys::Win32::{
        Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT},
        Security::{
            Authorization::ConvertSidToStringSidW,
            FreeSid, GetTokenInformation,
            Isolation::{
                CreateAppContainerProfile, DeleteAppContainerProfile, GetAppContainerFolderPath,
            },
            TokenIsAppContainer, PSID, SECURITY_CAPABILITIES, TOKEN_QUERY,
        },
        System::{
            Com::CoTaskMemFree,
            JobObjects::{
                AssignProcessToJobObject, CreateJobObjectW, JobObjectBasicAccountingInformation,
                JobObjectExtendedLimitInformation, QueryInformationJobObject,
                SetInformationJobObject, TerminateJobObject,
                JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
                JOB_OBJECT_LIMIT_ACTIVE_PROCESS, JOB_OBJECT_LIMIT_JOB_MEMORY,
                JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            },
            Threading::{
                CreateProcessW, DeleteProcThreadAttributeList, GetCurrentProcess,
                GetExitCodeProcess, InitializeProcThreadAttributeList, OpenProcessToken,
                ResumeThread, UpdateProcThreadAttribute, WaitForSingleObject, CREATE_NO_WINDOW,
                CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT, EXTENDED_STARTUPINFO_PRESENT,
                PROCESS_INFORMATION, PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES, STARTUPINFOEXW,
            },
        },
    };

    fn error(action: &str) -> io::Error {
        let inner = io::Error::last_os_error();
        io::Error::new(inner.kind(), format!("{action}: {inner}"))
    }
    fn wide(s: &OsStr) -> Vec<u16> {
        s.encode_wide().chain(std::iter::once(0)).collect()
    }
    fn wide_string(s: &str) -> Vec<u16> {
        wide(OsStr::new(s))
    }

    struct Handle(HANDLE);
    impl Drop for Handle {
        fn drop(&mut self) {
            if !self.0.is_null() {
                // SAFETY: Win32 OS handle is uniquely owned by this wrapper.
                unsafe {
                    CloseHandle(self.0);
                }
            }
        }
    }
    impl Handle {
        fn checked(raw: HANDLE, name: &str) -> io::Result<Self> {
            if raw.is_null() {
                Err(error(name))
            } else {
                Ok(Self(raw))
            }
        }
    }

    struct Profile {
        name: Vec<u16>,
        sid: PSID,
    }
    impl Drop for Profile {
        fn drop(&mut self) {
            // SAFETY: SID returned by CreateAppContainerProfile; profile name
            // is a stable NUL-terminated wide string created by this process.
            unsafe {
                if !self.sid.is_null() {
                    FreeSid(self.sid);
                }
                let _ = DeleteAppContainerProfile(self.name.as_ptr());
            }
        }
    }

    struct AttrList {
        // usize allocation ensures correct alignment for Win32 opaque data.
        storage: Vec<usize>,
        initialized: bool,
    }
    impl AttrList {
        fn create() -> io::Result<Self> {
            let mut size = 0_usize;
            // SAFETY: documented sizing query, NULL list and live size ptr.
            unsafe {
                InitializeProcThreadAttributeList(ptr::null_mut(), 1, 0, &mut size);
            }
            if size == 0 || size > 32 * 1024 {
                return Err(io::Error::other("invalid OS attribute list size"));
            }
            let slots = size.div_ceil(size_of::<usize>());
            let mut storage = vec![0_usize; slots];
            // SAFETY: buffer is properly aligned and at least requested size.
            let ok = unsafe {
                InitializeProcThreadAttributeList(storage.as_mut_ptr().cast(), 1, 0, &mut size)
            };
            if ok == 0 {
                return Err(error("InitializeProcThreadAttributeList"));
            }
            Ok(Self {
                storage,
                initialized: true,
            })
        }
        fn raw(&mut self) -> *mut core::ffi::c_void {
            self.storage.as_mut_ptr().cast()
        }
    }
    impl Drop for AttrList {
        fn drop(&mut self) {
            if self.initialized {
                // SAFETY: initialized attribute list owned by this buffer.
                unsafe {
                    DeleteProcThreadAttributeList(self.storage.as_mut_ptr().cast());
                }
            }
        }
    }

    fn sid_string(sid: PSID) -> io::Result<String> {
        let mut raw: *mut u16 = ptr::null_mut();
        // SAFETY: SID is valid while Profile owns it; OS owns output.
        if unsafe { ConvertSidToStringSidW(sid, &mut raw) } == 0 {
            return Err(error("ConvertSidToStringSidW"));
        }
        if raw.is_null() {
            return Err(io::Error::other("empty SID string"));
        }
        let mut count = 0;
        // SAFETY: OS returns a NUL-terminated wide-string buffer.
        while unsafe { *raw.add(count) } != 0 && count < 256 {
            count += 1;
        }
        let result = String::from_utf16(unsafe { std::slice::from_raw_parts(raw, count) })
            .map_err(|_| io::Error::other("SID UTF16 invalid"));
        // ConvertSidToStringSidW output is LocalAlloc owned, freed by LocalFree
        // in a separate wrapper below.
        unsafe {
            windows_sys::Win32::Foundation::LocalFree(raw.cast());
        }
        result
    }

    fn container_folder(sid: PSID) -> io::Result<PathBuf> {
        let sid_text = sid_string(sid)?;
        let wide_sid = wide_string(&sid_text);
        let mut raw: *mut u16 = ptr::null_mut();
        // SAFETY: API allocates resulting path from COM task allocator.
        let hr = unsafe { GetAppContainerFolderPath(wide_sid.as_ptr(), &mut raw) };
        if hr < 0 {
            return Err(io::Error::other(format!(
                "GetAppContainerFolderPath HRESULT {hr:#x}"
            )));
        }
        if raw.is_null() {
            return Err(io::Error::other("AppContainer folder empty"));
        }
        let mut count = 0;
        while unsafe { *raw.add(count) } != 0 && count < 32760 {
            count += 1;
        }
        let path = PathBuf::from(OsString::from_wide(unsafe {
            std::slice::from_raw_parts(raw, count)
        }));
        // SAFETY: allocated by COM task allocator.
        unsafe {
            CoTaskMemFree(raw.cast());
        }
        Ok(path)
    }

    fn profile() -> io::Result<Profile> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|x| x.as_nanos())
            .unwrap_or(0);
        let name = wide_string(&format!("LuIdeD6{:x}{:x}", std::process::id(), nonce));
        let desc = wide_string("Lu IDE fixed OS rights probe");
        let mut sid: PSID = ptr::null_mut();
        // SAFETY: fixed profile names and zero capability array => no network.
        let hr = unsafe {
            CreateAppContainerProfile(
                name.as_ptr(),
                desc.as_ptr(),
                desc.as_ptr(),
                ptr::null(),
                0,
                &mut sid,
            )
        };
        if hr < 0 {
            return Err(io::Error::other(format!(
                "CreateAppContainerProfile HRESULT {hr:#x}"
            )));
        }
        if sid.is_null() {
            return Err(io::Error::other("AppContainer SID missing"));
        }
        Ok(Profile { name, sid })
    }

    // The parent checks the secret is readable before spawn. The child may
    // report success ONLY for a genuine ACL denial, never for a missing file,
    // malformed path, or unrelated I/O failure.
    fn child(args: &[String]) -> io::Result<Option<i32>> {
        if args.first().map(String::as_str) != Some("--appcontainer-child") {
            return Ok(None);
        }
        if args.len() != 3 {
            return Err(io::Error::other("invalid fixed child args"));
        }
        let secret = Path::new(&args[1]);
        let port: u16 = args[2]
            .parse()
            .map_err(|_| io::Error::other("invalid loopback probe port"))?;
        let mut raw: HANDLE = ptr::null_mut();
        // SAFETY: current process pseudo handle, live output pointer.
        if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut raw) } == 0 {
            return Err(error("OpenProcessToken child"));
        }
        let token = Handle::checked(raw, "token")?;
        let mut appcontainer: u32 = 0;
        let mut size: u32 = 0;
        // SAFETY: OS token handle and correctly sized output.
        let got = unsafe {
            GetTokenInformation(
                token.0,
                TokenIsAppContainer,
                (&mut appcontainer as *mut u32).cast(),
                size_of::<u32>() as u32,
                &mut size,
            )
        };
        if got == 0 || appcontainer != 1 {
            return Ok(Some(10));
        }
        match fs::read(secret) {
            Ok(_) => return Ok(Some(11)),
            Err(err) if err.raw_os_error() == Some(5) => {}
            Err(_) => return Ok(Some(12)),
        }
        // The host has a live listener on this exact loopback port. Only
        // WSAEACCES (10013) counts as OS network isolation. ECONNREFUSED
        // or timeout could be a dead endpoint, so neither proves denial.
        let loopback = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port));
        match TcpStream::connect_timeout(&loopback, Duration::from_millis(800)) {
            Ok(_) => Ok(Some(13)),
            Err(err) if err.raw_os_error() == Some(10013) => Ok(Some(0)),
            // Encode the actual WinSock failure for native CI diagnosis.
            // Do NOT turn an arbitrary timeout, routing error or refusal
            // into a security pass. Only WSAEACCES above passes.
            Err(err) => Ok(Some(1000 + err.raw_os_error().unwrap_or(99))),
        }
    }

    fn job() -> io::Result<Handle> {
        let job = Handle::checked(
            // SAFETY: unnamed private Job Object with default OS attributes.
            unsafe { CreateJobObjectW(ptr::null(), ptr::null()) },
            "CreateJobObjectW",
        )?;
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
            | JOB_OBJECT_LIMIT_ACTIVE_PROCESS
            | JOB_OBJECT_LIMIT_JOB_MEMORY;
        limits.BasicLimitInformation.ActiveProcessLimit = 3;
        limits.JobMemoryLimit = 128 * 1024 * 1024;
        // SAFETY: size matches the selected OS job info class.
        if unsafe {
            SetInformationJobObject(
                job.0,
                JobObjectExtendedLimitInformation,
                (&raw const limits).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        } == 0
        {
            return Err(error("SetInformationJobObject"));
        }
        Ok(job)
    }

    fn check_no_active(job: &Handle) -> io::Result<()> {
        let mut info = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
        // SAFETY: query buffer initialized and correctly sized.
        if unsafe {
            QueryInformationJobObject(
                job.0,
                JobObjectBasicAccountingInformation,
                (&mut info as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
                size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                ptr::null_mut(),
            )
        } == 0
        {
            return Err(error("QueryInformationJobObject"));
        }
        if info.ActiveProcesses != 0 {
            return Err(io::Error::other(
                "AppContainer Job still has active processes",
            ));
        }
        Ok(())
    }

    struct FixtureCleanup {
        exe: PathBuf,
        secret: PathBuf,
    }
    impl Drop for FixtureCleanup {
        fn drop(&mut self) {
            // The guard handles all error paths, including failed launch or
            // timeout. The profile is deleted after these files are removed.
            let _ = fs::remove_file(&self.exe);
            let _ = fs::remove_file(&self.secret);
        }
    }

    fn terminate_unassigned_child(child: &Handle) -> io::Result<()> {
        // A CREATE_SUSPENDED child rejected by Job assignment is NOT a Job
        // member. TerminateJobObject alone would leave this orphan alive.
        if unsafe { windows_sys::Win32::System::Threading::TerminateProcess(child.0, 103) } == 0 {
            return Err(error("TerminateProcess unassigned AppContainer child"));
        }
        if unsafe { WaitForSingleObject(child.0, 5000) } != WAIT_OBJECT_0 {
            return Err(io::Error::other(
                "unassigned AppContainer process termination not verified",
            ));
        }
        Ok(())
    }

    fn run() -> io::Result<()> {
        let profile = profile()?;
        let folder = container_folder(profile.sid)?;
        let local = folder.join("LocalState");
        fs::create_dir_all(&local)?;
        let exe = local.join("lu-ide-fixed-appcontainer-probe.exe");
        let secret =
            std::env::temp_dir().join(format!("lu-ide-outside-secret-{}.txt", std::process::id()));
        let cleanup = FixtureCleanup {
            exe: exe.clone(),
            secret: secret.clone(),
        };
        fs::copy(std::env::current_exe()?, &exe)?;
        fs::write(&secret, b"PRIVATE HOST TEST SECRET")?;
        if fs::read(&secret)?.as_slice() != b"PRIVATE HOST TEST SECRET" {
            return Err(io::Error::other("host secret preflight failed"));
        }
        // Keep a real OS-owned listener live for the full child lifetime.
        // This avoids false network isolation passes from a closed endpoint.
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let loopback = listener.local_addr()?;
        let preflight = TcpStream::connect_timeout(&loopback, Duration::from_secs(1))?;
        let (_accepted, _) = listener.accept()?;
        drop(preflight);
        let mut caps = SECURITY_CAPABILITIES::default();
        caps.AppContainerSid = profile.sid;
        caps.Capabilities = ptr::null_mut();
        caps.CapabilityCount = 0;
        let mut attrs = AttrList::create()?;
        // SAFETY: fixed, live SECURITY_CAPABILITIES until CreateProcess returns.
        let ok = unsafe {
            UpdateProcThreadAttribute(
                attrs.raw(),
                0,
                PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES as usize,
                (&caps as *const SECURITY_CAPABILITIES).cast(),
                size_of::<SECURITY_CAPABILITIES>(),
                ptr::null_mut(),
                ptr::null(),
            )
        };
        if ok == 0 {
            return Err(error("UpdateProcThreadAttribute"));
        }

        let program = wide(exe.as_os_str());
        let mut cmd = wide_string(&format!(
            "\"{}\" --appcontainer-child \"{}\" {}",
            exe.display(),
            secret.display(),
            loopback.port()
        ));
        let mut startup = STARTUPINFOEXW::default();
        startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
        startup.lpAttributeList = attrs.raw();
        let mut proc = PROCESS_INFORMATION::default();
        // Windows CreateProcessW requires a usable SystemRoot for a native
        // AppContainer image. A double-NUL-only block produces ERROR_ENVVAR_NOT_FOUND
        // (203) on actual Windows runners. Preserve ONLY these OS bootstrap keys.
        // No HOME, token, credential, SSH, or project environment is inherited.
        let system_root = std::env::var_os("SystemRoot")
            .ok_or_else(|| io::Error::other("SystemRoot unavailable; deny launch"))?;
        if !Path::new(&system_root).is_absolute() {
            return Err(io::Error::other("SystemRoot is not absolute"));
        }
        // CreateProcessW's AppContainer startup resolves the package profile
        // under LOCALAPPDATA. On real GitHub Windows runners, omitting it
        // returns ERROR_ENVVAR_NOT_FOUND (203), even when SystemRoot is set.
        let local_app_data = std::env::var_os("LOCALAPPDATA")
            .ok_or_else(|| io::Error::other("LOCALAPPDATA unavailable; deny launch"))?;
        if !Path::new(&local_app_data).is_absolute() {
            return Err(io::Error::other("LOCALAPPDATA is not absolute"));
        }
        let temp = std::env::temp_dir();
        let env: Vec<u16> = format!(
            "LOCALAPPDATA={}\0SystemRoot={}\0TEMP={}\0TMP={}\0\0",
            local_app_data.to_string_lossy(),
            system_root.to_string_lossy(),
            temp.display(),
            temp.display()
        )
        .encode_utf16()
        .collect();
        // SAFETY: all pointers live; child cannot execute until assigned to Job.
        let launched = unsafe {
            CreateProcessW(
                program.as_ptr(),
                cmd.as_mut_ptr(),
                ptr::null(),
                ptr::null(),
                0,
                CREATE_SUSPENDED
                    | EXTENDED_STARTUPINFO_PRESENT
                    | CREATE_NO_WINDOW
                    | CREATE_UNICODE_ENVIRONMENT,
                env.as_ptr().cast(),
                ptr::null(),
                &startup.StartupInfo,
                &mut proc,
            )
        };
        if launched == 0 {
            return Err(error("CreateProcessW AppContainer"));
        }
        let child_handle = Handle::checked(proc.hProcess, "AppContainer child process")?;
        let thread_handle = match Handle::checked(proc.hThread, "AppContainer child thread") {
            Ok(value) => value,
            Err(reason) => {
                terminate_unassigned_child(&child_handle)?;
                return Err(reason);
            }
        };
        let job = match job() {
            Ok(value) => value,
            Err(reason) => {
                terminate_unassigned_child(&child_handle)?;
                return Err(reason);
            }
        };
        // SAFETY: child is still CREATE_SUSPENDED.
        if unsafe { AssignProcessToJobObject(job.0, child_handle.0) } == 0 {
            let reason = error("AssignProcessToJobObject");
            terminate_unassigned_child(&child_handle)?;
            return Err(reason);
        }
        // SAFETY: original child thread already assigned to Job Object.
        if unsafe { ResumeThread(thread_handle.0) } == u32::MAX {
            unsafe {
                TerminateJobObject(job.0, 101);
            }
            return Err(error("ResumeThread"));
        }
        // SAFETY: Wait on owned process; kill whole tree if deadline exceeded.
        let waited = unsafe { WaitForSingleObject(child_handle.0, 8000) };
        if waited == WAIT_TIMEOUT {
            unsafe {
                TerminateJobObject(job.0, 102);
            }
            return Err(io::Error::other("AppContainer child timed out"));
        }
        if waited != WAIT_OBJECT_0 {
            return Err(error("WaitForSingleObject"));
        }
        let mut exit = 1_u32;
        // SAFETY: owned process handle remains live.
        if unsafe { GetExitCodeProcess(child_handle.0, &mut exit) } == 0 {
            return Err(error("GetExitCodeProcess"));
        }
        if exit != 0 {
            return Err(io::Error::other(format!(
                "child rejected: AppContainer token/host secret check exit={exit}"
            )));
        }
        check_no_active(&job)?;
        fs::remove_file(&cleanup.exe)?;
        fs::remove_file(&cleanup.secret)?;
        println!("PASS: fixed AppContainer child denied host secret (ERROR_ACCESS_DENIED) and live loopback socket (WSAEACCES), Job Object assignment before resume");
        println!("UNVERIFIED: arbitrary external network endpoints, production Cargo toolchain, disk ACL races, crash recovery");
        Ok(())
    }

    pub fn main_impl() -> io::Result<()> {
        let args: Vec<String> = std::env::args().skip(1).collect();
        if let Some(exit) = child(&args)? {
            std::process::exit(exit);
        }
        if !args.is_empty() {
            return Err(io::Error::other(
                "only fixed probe or fixed child mode supported",
            ));
        }
        run()
    }
}

#[cfg(windows)]
fn main() {
    if let Err(err) = windows::main_impl() {
        eprintln!("DENIED: AppContainer Job Object admission probe: {err}");
        std::process::exit(1);
    }
}
