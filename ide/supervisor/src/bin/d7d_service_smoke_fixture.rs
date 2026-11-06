//! CI-only native admission against a genuine SCM LocalService instance.
//! It never creates a worker, consumes an approval or runs arbitrary commands.
#[cfg(all(windows, feature = "protected-recovery-service"))]
fn main() {
    if let Err(e) = smoke() {
        eprintln!("D7D SERVICE DENIED: {}", e);
        std::process::exit(1);
    }
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
