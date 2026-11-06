//! CI-only native admission against a genuine SCM LocalService instance.
//! It never creates a worker, consumes an approval or runs arbitrary commands.
#[cfg(all(windows, feature = "protected-recovery-service"))]
fn main() {
    if let Err(e) = smoke() {
        eprintln!("D7D SERVICE DENIED: {}", e);
        std::process::exit(1);
    }
}
/// CI-only independent access check on a throwaway native pipe with the
/// exact same DACL as the restricted SCM service. This is NOT authentication,
/// and cannot replace an actual kernel-verified service round trip.
#[cfg(all(windows, feature = "protected-recovery-service"))]
fn ci_exact_user_pipe_dacl_probe() -> Result<(), String> {
    use lu_ide_execution_supervisor::windows_service_security::{ws, PipeSecurity};
    use rand::{rngs::OsRng, RngCore};
    use std::ptr;
    use windows_sys::Win32::{
        Foundation::{CloseHandle, GetLastError, INVALID_HANDLE_VALUE},
        Security::{
            Authorization::{SetSecurityInfo, SE_FILE_OBJECT},
            GetSecurityDescriptorDacl, DACL_SECURITY_INFORMATION,
        },
        Storage::FileSystem::{CreateFileW, OPEN_EXISTING, PIPE_ACCESS_DUPLEX},
        System::Pipes::{
            CreateNamedPipeW, PIPE_READMODE_MESSAGE, PIPE_REJECT_REMOTE_CLIENTS,
            PIPE_TYPE_MESSAGE, PIPE_WAIT,
        },
    };
    if std::env::var_os("GITHUB_ACTIONS").as_deref()
        != Some(std::ffi::OsStr::new("true"))
        || std::env::var_os("CI").as_deref() != Some(std::ffi::OsStr::new("true"))
    {
        return Ok(());
    }
    let expected_sid = std::env::var("LU_IDE_D7_DIAGNOSTIC_HOST_SID")
        .map_err(|_| "DENIED: installer host token test SID absent in CI")?;
    let acl = PipeSecurity::new(&expected_sid).map_err(|e| e.code().to_string())?;
    let mut random = [0u8; 16];
    OsRng.fill_bytes(&mut random);
    let nonce: String = random.iter().map(|b| format!("{b:02x}")).collect();
    let endpoint = ws(&format!(r"\\.\pipe\LuIdeD7AclProbe{nonce}"));
    // The real SCM service has the protected service SID needed to
    // CREATE a pipe under that final DACL. This ordinary host does not:
    // build one unguessable CI-only instance under its local default ACL,
    // then replace its DACL with the identical service-issued ACL BEFORE
    // any client opens. This tests client access, not service identity.
    const FIRST_INSTANCE: u32 = 0x0008_0000;
    const WRITE_DAC: u32 = 0x0004_0000;
    let server = unsafe {
        CreateNamedPipeW(
            endpoint.as_ptr(),
            PIPE_ACCESS_DUPLEX | FIRST_INSTANCE | WRITE_DAC,
            PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            1,
            4096,
            4096,
            0,
            ptr::null(),
        )
    };
    if server.is_null() || server == INVALID_HANDLE_VALUE {
        return Err(format!("DENIED: CI pipe creation Win32={}", unsafe { GetLastError() }));
    }
    let mut present = 0i32;
    let mut defaulted = 0i32;
    let mut dacl = ptr::null_mut();
    let extracted = unsafe {
        GetSecurityDescriptorDacl(
            acl.attributes.lpSecurityDescriptor,
            &mut present,
            &mut dacl,
            &mut defaulted,
        )
    };
    if extracted == 0 || present == 0 || dacl.is_null() {
        unsafe { CloseHandle(server); }
        return Err("DENIED: exact-user service DACL extraction failed".into());
    }
    let applied = unsafe {
        SetSecurityInfo(
            server,
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            ptr::null_mut(),
            ptr::null_mut(),
            dacl,
            ptr::null_mut(),
        )
    };
    if applied != 0 {
        unsafe { CloseHandle(server); }
        return Err(format!("DENIED: exact-user CI DACL application Win32={applied}"));
    }
    // Match the production client precisely: no broad pipe-create right,
    // no privileged alternate identity and no forged service response.
    const FILE_READ_DATA: u32 = 0x0000_0001;
    const FILE_WRITE_DATA: u32 = 0x0000_0002;
    const SYNCHRONIZE: u32 = 0x0010_0000;
    const SECURITY_QOS: u32 = 0x0012_0000;
    let client = unsafe {
        CreateFileW(
            endpoint.as_ptr(),
            FILE_READ_DATA | FILE_WRITE_DATA | SYNCHRONIZE,
            0,
            ptr::null(),
            OPEN_EXISTING,
            SECURITY_QOS,
            ptr::null_mut(),
        )
    };
    let failure = if client.is_null() || client == INVALID_HANDLE_VALUE {
        Some(unsafe { GetLastError() })
    } else {
        unsafe { CloseHandle(client); }
        None
    };
    unsafe { CloseHandle(server); }
    if let Some(error) = failure {
        return Err(format!("DENIED: exact-user CI pipe self-check Win32={error}"));
    }
    eprintln!("D7D_PIPE_CI_SELF_PROBE=exact_acl_client_opened");
    Ok(())
}
#[cfg(all(windows, feature = "protected-recovery-service"))]
fn smoke() -> Result<(), String> {
    use lu_ide_execution_supervisor::{
        windows_broker::TrustedBroker,
        windows_service_client::ServiceLease,
        windows_service_security::{ws, JobSecurity},
    };
    use rand::{rngs::OsRng, RngCore};
    use std::{path::PathBuf, ptr};
    use windows_sys::Win32::{
        Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS},
        Storage::FileSystem::PIPE_ACCESS_DUPLEX,
        System::{
            JobObjects::CreateJobObjectW,
            Pipes::{
                CreateNamedPipeW, PIPE_READMODE_MESSAGE, PIPE_REJECT_REMOTE_CLIENTS,
                PIPE_TYPE_MESSAGE, PIPE_WAIT,
            },
        },
    };
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("service path + fixed hash required".into());
    }
    let exe = PathBuf::from(&args[0]);
    let hash = args[1].to_string_lossy().into_owned().to_ascii_lowercase();
    if !exe.is_absolute() || hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("invalid reviewed service binary pin".into());
    }
    let mut nonce = [0u8; 16];
    OsRng.fill_bytes(&mut nonce);
    let id = format!(
        "job-{}",
        nonce.iter().map(|b| format!("{b:02x}")).collect::<String>()
    );
    ci_exact_user_pipe_dacl_probe()?;
    let plan = "a".repeat(64);
    let name = ws(&format!("Global\\LuIdeD7-{id}-{}", &plan[..16]));
    let acl = JobSecurity::new().map_err(|e| e.code())?;
    let job = unsafe { CreateJobObjectW(&acl.attributes, name.as_ptr()) };
    if job.is_null() {
        return Err("CreateJobObjectW denied".into());
    }
    struct Close(windows_sys::Win32::Foundation::HANDLE);
    impl Drop for Close {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
    let _guard = Close(job);
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        return Err("unexpected existing Job".into());
    }
    let config = TrustedBroker {
        executable: exe,
        expected_sha256: hash,
        private_witness_directory: PathBuf::from(r"C:\ProgramData\LuIDERecoveryBroker"),
    };
    let mut lease = ServiceLease::begin(job, &id, &plan, &config)
        .map_err(|e| format!("service admission: {}", e.code()))?;
    if !lease.alive() {
        return Err("protected service departed after READY".into());
    }
    lease
        .release_empty()
        .map_err(|e| format!("protected release: {}", e.code()))?;
    // No ordinary interactive user may create another server instance on the
    // service's name, even when the caller is an administrator in CI.
    let rogue = unsafe {
        CreateNamedPipeW(
            ws(lu_ide_execution_supervisor::windows_service_security::SERVICE_ENDPOINT).as_ptr(),
            PIPE_ACCESS_DUPLEX,
            PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            16,
            4096,
            4096,
            0,
            ptr::null(),
        )
    };
    if rogue != windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
        unsafe {
            CloseHandle(rogue);
        }
        return Err("same-user process created a forged service pipe".into());
    }
    // The protected witness/config bytes cannot be modified under ordinary
    // interactive rights, regardless of who launched the native IDE.
    let config_path = config.private_witness_directory.join("service.json");
    if std::fs::OpenOptions::new()
        .append(true)
        .open(&config_path)
        .is_ok()
    {
        return Err("interactive host can append to protected config".into());
    }
    println!("PASS: rogue pipe instance and interactive config tampering rejected");
    println!("PASS: kernel-attested service admission and durable empty Job release");
    Ok(())
}
#[cfg(not(all(windows, feature = "protected-recovery-service")))]
fn main() {
    eprintln!("DENIED: protected service test requires Windows opt-in feature");
    std::process::exit(2);
}
