//! Disposable OS-kernel AppContainer ACL witness for IDE-05.6.
//! Only compiled with ide056-ci-acl-positive. Does not enable Lu package jobs.
#[cfg(not(all(windows, feature = "ide056-ci-acl-positive")))]
fn main() {
    eprintln!("DENIED: test-only native Windows ACL feature required");
    std::process::exit(2);
}

#[cfg(all(windows, feature = "ide056-ci-acl-positive"))]
mod native {
    use lu_ide_execution_supervisor::inspect_ide056_readonly_tree;
    use std::{
        env,
        ffi::{c_void, OsStr},
        fs, io,
        mem::size_of,
        os::windows::ffi::OsStrExt,
        path::{Path, PathBuf},
        ptr,
        time::{SystemTime, UNIX_EPOCH},
    };
    use windows_sys::Win32::{
        Foundation::{CloseHandle, LocalFree, HANDLE, WAIT_OBJECT_0},
        Security::{
            Authorization::{
                ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
                ConvertStringSidToSidW, SetNamedSecurityInfoW, SE_FILE_OBJECT,
            },
            FreeSid, GetSecurityDescriptorDacl, GetSidSubAuthority, GetSidSubAuthorityCount,
            GetTokenInformation,
            Isolation::{
                CreateAppContainerProfile, DeleteAppContainerProfile, GetAppContainerFolderPath,
            },
            TokenIntegrityLevel, TokenIsAppContainer, TokenUser, DACL_SECURITY_INFORMATION,
            OWNER_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION, PSID,
            SECURITY_CAPABILITIES, TOKEN_MANDATORY_LABEL, TOKEN_QUERY, TOKEN_USER,
        },
        Storage::FileSystem::{
            CreateFileW, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
        },
        System::{
            Com::CoTaskMemFree,
            JobObjects::{
                CreateJobObjectW, IsProcessInJob, JobObjectExtendedLimitInformation,
                SetInformationJobObject, TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
                JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            },
            Threading::{
                CreateProcessW, DeleteProcThreadAttributeList, GetCurrentProcess,
                GetExitCodeProcess, InitializeProcThreadAttributeList, OpenProcessToken,
                ResumeThread, UpdateProcThreadAttribute, WaitForSingleObject, CREATE_NO_WINDOW,
                CREATE_SUSPENDED, EXTENDED_STARTUPINFO_PRESENT, PROCESS_INFORMATION,
                PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES, STARTUPINFOEXW,
            },
        },
    };

    type R<T> = Result<T, Box<dyn std::error::Error>>;
    fn fail(code: &'static str) -> io::Error {
        io::Error::other(code)
    }
    fn ensure(ok: bool, code: &'static str) -> R<()> {
        if ok {
            Ok(())
        } else {
            Err(fail(code).into())
        }
    }
    fn wide(s: &OsStr) -> Vec<u16> {
        s.encode_wide().chain([0]).collect()
    }
    fn w(s: &str) -> Vec<u16> {
        wide(OsStr::new(s))
    }
    unsafe fn read_wide(p: *const u16) -> R<String> {
        ensure(!p.is_null(), "IDE056_NATIVE_NULL_WSTRING")?;
        let mut n = 0usize;
        while n < 32768 && *p.add(n) != 0 {
            n += 1;
        }
        ensure(n < 32768, "IDE056_NATIVE_WSTRING_TOO_LONG")?;
        Ok(String::from_utf16(std::slice::from_raw_parts(p, n))?)
    }
    struct KernelHandle(HANDLE);
    impl Drop for KernelHandle {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    CloseHandle(self.0);
                }
            }
        }
    }
    struct Profile {
        name: Vec<u16>,
        sid: PSID,
    }
    impl Drop for Profile {
        fn drop(&mut self) {
            unsafe {
                if !self.sid.is_null() {
                    FreeSid(self.sid);
                }
                DeleteAppContainerProfile(self.name.as_ptr());
            }
        }
    }
    fn sid_string(sid: PSID) -> R<String> {
        let mut ptr: *mut u16 = ptr::null_mut();
        ensure(
            unsafe { ConvertSidToStringSidW(sid, &mut ptr) } != 0 && !ptr.is_null(),
            "IDE056_SID_STRING_UNAVAILABLE",
        )?;
        let string = unsafe { read_wide(ptr) };
        unsafe {
            LocalFree(ptr.cast());
        }
        string
    }
    fn create_profile() -> R<Profile> {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let name = w(&format!("LuIde056ACL{}{}", std::process::id(), stamp));
        let desc = w("Lu IDE disposable host-verified ACL acceptance");
        let mut sid = ptr::null_mut();
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
        ensure(hr >= 0 && !sid.is_null(), "IDE056_PROFILE_CREATION_FAILED")?;
        Ok(Profile { name, sid })
    }
    fn local_state(profile: &Profile) -> R<PathBuf> {
        let name = w(&sid_string(profile.sid)?);
        let mut ptr: *mut u16 = ptr::null_mut();
        let hr = unsafe { GetAppContainerFolderPath(name.as_ptr(), &mut ptr) };
        ensure(
            hr >= 0 && !ptr.is_null(),
            "IDE056_PACKAGE_FOLDER_UNAVAILABLE",
        )?;
        let result = unsafe { read_wide(ptr) };
        unsafe {
            CoTaskMemFree(ptr.cast());
        }
        let path = PathBuf::from(result?).join("LocalState");
        fs::create_dir_all(&path)?;
        Ok(path)
    }
    fn host_user_sid() -> R<String> {
        let mut handle = ptr::null_mut();
        ensure(
            unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut handle) } != 0,
            "IDE056_HOST_TOKEN_UNAVAILABLE",
        )?;
        let token = KernelHandle(handle);
        let mut bytes = 0u32;
        unsafe {
            GetTokenInformation(token.0, TokenUser, ptr::null_mut(), 0, &mut bytes);
        }
        ensure(
            bytes as usize >= size_of::<TOKEN_USER>() && bytes <= 4096,
            "IDE056_HOST_SID_INVALID",
        )?;
        let mut data = vec![0usize; (bytes as usize).div_ceil(size_of::<usize>())];
        ensure(
            unsafe {
                GetTokenInformation(
                    token.0,
                    TokenUser,
                    data.as_mut_ptr().cast(),
                    bytes,
                    &mut bytes,
                )
            } != 0,
            "IDE056_HOST_SID_QUERY_FAILED",
        )?;
        let sid = unsafe { (*(data.as_ptr() as *const TOKEN_USER)).User.Sid };
        sid_string(sid)
    }
    fn seal(path: &Path, user: &str, package: &str, writable: bool) -> R<()> {
        // Only three explicit ACEs; protected DACL prevents broad inherited
        // grants. Package source rights never include delete, owner or WRITE_DAC.
        let pkg_mask = if writable { "0x001301bf" } else { "0x001200a9" };
        let sddl = w(&format!(
            "D:P(A;OICI;0x001f01ff;;;SY)(A;OICI;0x001f01ff;;;{user})(A;OICI;{pkg_mask};;;{package})"
        ));
        let mut descriptor: *mut c_void = ptr::null_mut();
        let mut bytes = 0u32;
        ensure(
            unsafe {
                ConvertStringSecurityDescriptorToSecurityDescriptorW(
                    sddl.as_ptr(),
                    1,
                    &mut descriptor,
                    &mut bytes,
                )
            } != 0
                && !descriptor.is_null(),
            "IDE056_DACL_PARSE_FAILED",
        )?;
        let mut present = 0;
        let mut dacl = ptr::null_mut();
        let mut defaulted = 0;
        let ok = unsafe {
            GetSecurityDescriptorDacl(descriptor, &mut present, &mut dacl, &mut defaulted)
        } != 0
            && present != 0
            && !dacl.is_null();
        if !ok {
            unsafe {
                LocalFree(descriptor);
            }
            return Err(fail("IDE056_DACL_MISSING").into());
        }
        // CI runner tokens may create files with the Administrators group
        // as implicit OWNER rather than TokenUser. Our independent native
        // observer requires *exactly* the reviewed host TokenUser SID.
        // Assign that owner explicitly; never relax the observer to accept
        // Administrator, SYSTEM, package SID or an arbitrary inherited ACE.
        let owner_text = w(user);
        let mut owner_sid: PSID = ptr::null_mut();
        if unsafe { ConvertStringSidToSidW(owner_text.as_ptr(), &mut owner_sid) } == 0
            || owner_sid.is_null()
        {
            unsafe {
                LocalFree(descriptor);
            }
            return Err(fail("IDE056_EXACT_OWNER_SID_INVALID").into());
        }
        let mut name = wide(path.as_os_str());
        let result = unsafe {
            SetNamedSecurityInfoW(
                name.as_mut_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION
                    | PROTECTED_DACL_SECURITY_INFORMATION
                    | OWNER_SECURITY_INFORMATION,
                owner_sid,
                ptr::null_mut(),
                dacl,
                ptr::null_mut(),
            )
        };
        unsafe {
            LocalFree(owner_sid);
            LocalFree(descriptor);
        }
        ensure(result == 0, "IDE056_EXACT_OWNER_AND_DACL_SEAL_DENIED")
    }
    fn attest_low_integrity_token() -> R<()> {
        let mut raw = ptr::null_mut();
        ensure(
            unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut raw) } != 0
                && !raw.is_null(),
            "IDE056_LOW_INTEGRITY_TOKEN_UNAVAILABLE",
        )?;
        let token = KernelHandle(raw);
        let mut size = 0u32;
        unsafe {
            GetTokenInformation(token.0, TokenIntegrityLevel, ptr::null_mut(), 0, &mut size);
        }
        ensure(
            size >= size_of::<TOKEN_MANDATORY_LABEL>() as u32 && size < 4096,
            "IDE056_LOW_INTEGRITY_SIZE_INVALID",
        )?;
        let mut data = vec![0usize; (size as usize).div_ceil(size_of::<usize>())];
        ensure(
            unsafe {
                GetTokenInformation(
                    token.0,
                    TokenIntegrityLevel,
                    data.as_mut_ptr().cast(),
                    size,
                    &mut size,
                )
            } != 0,
            "IDE056_LOW_INTEGRITY_QUERY_FAILED",
        )?;
        let sid = unsafe { (*(data.as_ptr() as *const TOKEN_MANDATORY_LABEL)).Label.Sid };
        ensure(!sid.is_null(), "IDE056_INTEGRITY_SID_MISSING")?;
        let subcount = unsafe { GetSidSubAuthorityCount(sid) };
        ensure(!subcount.is_null(), "IDE056_INTEGRITY_SID_INVALID")?;
        let n = unsafe { *subcount } as u32;
        ensure(n > 0 && n <= 8, "IDE056_INTEGRITY_SID_SUBAUTHORITY_DENIED")?;
        let rid = unsafe { GetSidSubAuthority(sid, n - 1) };
        ensure(!rid.is_null(), "IDE056_INTEGRITY_SID_RID_MISSING")?;
        // AppContainer must never inherit a medium/high host token.
        // Kernel MIC checks run in addition to DACL restrictions.
        ensure(unsafe { *rid } <= 0x1000, "IDE056_APP_CONTAINER_NOT_LOW_IL")?;
        Ok(())
    }

    fn attest_owner_rights_denied(source: &Path) -> R<()> {
        // Explicitly probe the host-owned file from inside the AppContainer:
        // owner implicit WRITE_DAC/WRITE_OWNER must not cross the package
        // SID's limited access intersection.
        let name = wide(source.as_os_str());
        for access in [0x0004_0000u32, 0x0008_0000u32] {
            let handle = unsafe {
                CreateFileW(
                    name.as_ptr(),
                    access,
                    FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                    ptr::null(),
                    OPEN_EXISTING,
                    0,
                    ptr::null_mut(),
                )
            };
            if handle != windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
                if !handle.is_null() {
                    unsafe {
                        CloseHandle(handle);
                    }
                }
                return Err(fail("IDE056_APPCONTAINER_CAN_MODIFY_OWNER_OR_DACL").into());
            }
        }
        Ok(())
    }

    fn child(source: &Path, output: &Path) -> R<()> {
        attest_low_integrity_token()?;
        attest_owner_rights_denied(source)?;
        ensure(
            fs::read(source)? == b"immutable-acl-source",
            "IDE056_CHILD_CANNOT_READ_SOURCE",
        )?;
        ensure(
            fs::OpenOptions::new().write(true).open(source).is_err(),
            "IDE056_CHILD_CAN_WRITE_SOURCE",
        )?;
        ensure(
            fs::rename(source, source.with_extension("renamed")).is_err(),
            "IDE056_CHILD_CAN_RENAME_SOURCE",
        )?;
        ensure(
            fs::remove_file(source).is_err(),
            "IDE056_CHILD_CAN_DELETE_SOURCE",
        )?;
        fs::write(output.join("child-proof.txt"), b"private-output-only")?;
        Ok(())
    }
    fn run_probe(
        exe: &Path,
        stage: &Path,
        source: &Path,
        output: &Path,
        profile: &Profile,
    ) -> R<()> {
        let raw_job = unsafe { CreateJobObjectW(ptr::null(), ptr::null()) };
        ensure(!raw_job.is_null(), "IDE056_JOB_CREATION_FAILED")?;
        let job = KernelHandle(raw_job);
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        ensure(
            unsafe {
                SetInformationJobObject(
                    job.0,
                    JobObjectExtendedLimitInformation,
                    (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                    size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                )
            } != 0,
            "IDE056_JOB_LIMITS_MISSING",
        )?;
        let mut bytes = 0usize;
        unsafe {
            InitializeProcThreadAttributeList(ptr::null_mut(), 2, 0, &mut bytes);
        }
        ensure(bytes > 0 && bytes < 8192, "IDE056_ATTRIBUTE_SIZE_INVALID")?;
        let mut data = vec![0usize; bytes.div_ceil(size_of::<usize>())];
        let attr = data.as_mut_ptr().cast();
        ensure(
            unsafe { InitializeProcThreadAttributeList(attr, 2, 0, &mut bytes) } != 0,
            "IDE056_ATTRIBUTES_UNAVAILABLE",
        )?;
        let mut caps = SECURITY_CAPABILITIES::default();
        caps.AppContainerSid = profile.sid;
        caps.Capabilities = ptr::null_mut();
        caps.CapabilityCount = 0;
        let jobs = [job.0];
        let setup = unsafe {
            UpdateProcThreadAttribute(
                attr,
                0,
                PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES as usize,
                (&caps as *const SECURITY_CAPABILITIES).cast(),
                size_of::<SECURITY_CAPABILITIES>(),
                ptr::null_mut(),
                ptr::null(),
            ) != 0
                && UpdateProcThreadAttribute(
                    attr,
                    0,
                    131085,
                    jobs.as_ptr().cast(),
                    size_of::<HANDLE>(),
                    ptr::null_mut(),
                    ptr::null(),
                ) != 0
        };
        if !setup {
            unsafe {
                DeleteProcThreadAttributeList(attr);
            }
            return Err(fail("IDE056_APP_CONTAINER_JOB_ATTRIBUTES_MISSING").into());
        }
        let mut startup = STARTUPINFOEXW::default();
        startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
        startup.lpAttributeList = attr;
        let mut process = PROCESS_INFORMATION::default();
        let exe_wide = wide(exe.as_os_str());
        let cwd_wide = wide(stage.as_os_str());
        let mut command = w(&format!(
            "\"{}\" --probe \"{}\" \"{}\"",
            exe.display(),
            source.display(),
            output.display()
        ));
        let spawned = unsafe {
            CreateProcessW(
                exe_wide.as_ptr(),
                command.as_mut_ptr(),
                ptr::null(),
                ptr::null(),
                0,
                CREATE_SUSPENDED | CREATE_NO_WINDOW | EXTENDED_STARTUPINFO_PRESENT,
                ptr::null(),
                cwd_wide.as_ptr(),
                &startup.StartupInfo,
                &mut process,
            )
        };
        unsafe {
            DeleteProcThreadAttributeList(attr);
        }
        ensure(spawned != 0, "IDE056_NATIVE_APP_CONTAINER_LAUNCH_FAILED")?;
        let child_handle = KernelHandle(process.hProcess);
        let thread = KernelHandle(process.hThread);
        let mut in_job = 0;
        ensure(
            unsafe { IsProcessInJob(child_handle.0, job.0, &mut in_job) } != 0 && in_job != 0,
            "IDE056_PROCESS_ESCAPED_JOB",
        )?;
        let mut child_token = ptr::null_mut();
        ensure(
            unsafe { OpenProcessToken(child_handle.0, TOKEN_QUERY, &mut child_token) } != 0,
            "IDE056_CHILD_TOKEN_MISSING",
        )?;
        let token = KernelHandle(child_token);
        let mut is_app = 0u32;
        let mut returned = 0u32;
        ensure(
            unsafe {
                GetTokenInformation(
                    token.0,
                    TokenIsAppContainer,
                    (&mut is_app as *mut u32).cast(),
                    size_of::<u32>() as u32,
                    &mut returned,
                )
            } != 0
                && is_app == 1,
            "IDE056_CHILD_NOT_APP_CONTAINER",
        )?;
        ensure(
            unsafe { ResumeThread(thread.0) } != u32::MAX,
            "IDE056_CHILD_RESUME_FAILED",
        )?;
        let waited = unsafe { WaitForSingleObject(child_handle.0, 30000) };
        if waited != WAIT_OBJECT_0 {
            unsafe {
                TerminateJobObject(job.0, 1);
            }
            return Err(fail("IDE056_CHILD_DID_NOT_EXIT_IN_BUDGET").into());
        }
        let mut code = u32::MAX;
        ensure(
            unsafe { GetExitCodeProcess(child_handle.0, &mut code) } != 0 && code == 0,
            "IDE056_CHILD_NATIVE_ACL_PROBE_FAILED",
        )?;
        Ok(())
    }
    pub fn run() -> R<()> {
        let args: Vec<String> = env::args().collect();
        if args.len() == 4 && args[1] == "--probe" {
            return child(Path::new(&args[2]), Path::new(&args[3]));
        }
        ensure(args.len() == 1, "IDE056_UNSUPPORTED_ACCEPTANCE_MODE")?;
        let profile = create_profile()?;
        println!("IDE_G03_WINDOWS_ACL_STAGE=profile_created");
        let package = sid_string(profile.sid)?;
        let user = host_user_sid()?;
        let stage = local_state(&profile)?.join("ide056-native-acl");
        let source_dir = stage.join("source");
        let output = stage.join("output");
        fs::create_dir(&stage)?;
        fs::create_dir(&source_dir)?;
        fs::create_dir(&output)?;
        let source = source_dir.join("original.txt");
        let exe = stage.join("acl-probe.exe");
        fs::write(&source, b"immutable-acl-source")?;
        fs::copy(env::current_exe()?, &exe)?;
        for path in [&source, &source_dir, &exe, &stage] {
            seal(path, &user, &package, false)?;
        }
        seal(&output, &user, &package, true)?;
        println!("IDE_G03_WINDOWS_ACL_STAGE=owner_and_dacl_sealed");
        let audited =
            inspect_ide056_readonly_tree(&source_dir, profile.sid).map_err(|e| fail(e.code()))?;
        ensure(audited == 2, "IDE056_SOURCE_ACL_AUDIT_NOT_EXACT")?;
        println!("IDE_G03_WINDOWS_ACL_STAGE=independent_source_acl_accepted");
        run_probe(&exe, &stage, &source, &output, &profile)?;
        println!("IDE_G03_WINDOWS_ACL_STAGE=native_child_verified");
        ensure(
            fs::read(&source)? == b"immutable-acl-source",
            "IDE056_SOURCE_CHANGED_AFTER_CHILD",
        )?;
        ensure(
            fs::read(output.join("child-proof.txt"))? == b"private-output-only",
            "IDE056_CHILD_PRIVATE_OUTPUT_NOT_OBSERVED",
        )?;
        println!("IDE056_WINDOWS_REAL_APPCONTAINER_SOURCE_WRITE_RENAME_DELETE_DENIED=PASS");
        println!("IDE056_WINDOWS_REAL_APPCONTAINER_PRIVATE_OUTPUT_WRITE=PASS");
        println!("IDE056_WINDOWS_REAL_APPCONTAINER_JOB_AND_TOKEN=PASS");
        println!("IDE056_WINDOWS_REAL_APPCONTAINER_LOW_IL_AND_OWNER_CONTROL_DENIED=PASS");
        println!("IDE056_HUMAN_APPROVAL=OPEN;SCM_RECOVERY=OPEN;MIC_ATTESTATION=OPEN");
        fs::remove_dir_all(stage)?;
        Ok(())
    }
}

#[cfg(all(windows, feature = "ide056-ci-acl-positive"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    native::run()
}
