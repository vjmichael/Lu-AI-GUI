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
        windows_service_security::{JobSecurity, ws},
    };
    use rand::{rngs::OsRng, RngCore};
    use std::{path::PathBuf, ptr};
    use windows_sys::Win32::{
        Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS},
        System::JobObjects::CreateJobObjectW,
    };
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 { return Err("service path + fixed hash required".into()); }
    let exe = PathBuf::from(&args[0]);
    let hash = args[1].to_string_lossy().into_owned().to_ascii_lowercase();
    if !exe.is_absolute() || hash.len() != 64
        || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("invalid reviewed service binary pin".into());
    }
    let mut nonce = [0u8; 16];
    OsRng.fill_bytes(&mut nonce);
    let id = format!("job-{}", nonce.iter().map(|b| format!("{b:02x}")).collect::<String>());
    let plan = "a".repeat(64);
    let name = ws(&format!("Global\\LuIdeD7-{id}-{}", &plan[..16]));
    let acl = JobSecurity::new().map_err(|e| e.code())?;
    let job = unsafe { CreateJobObjectW(&acl.attributes, name.as_ptr()) };
    if job.is_null() { return Err("CreateJobObjectW denied".into()); }
    struct Close(windows_sys::Win32::Foundation::HANDLE);
    impl Drop for Close {
        fn drop(&mut self) { unsafe { CloseHandle(self.0); } }
    }
    let _guard = Close(job);
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        return Err("unexpected existing Job".into());
    }
    let config = TrustedBroker {
        executable: exe, expected_sha256: hash,
        private_witness_directory: PathBuf::from(r"C:\ProgramData\LuIDE\RecoveryBroker"),
    };
    let mut lease = ServiceLease::begin(job, &id, &plan, &config)
        .map_err(|e| format!("service admission: {}", e.code()))?;
    if !lease.alive() { return Err("protected service departed after READY".into()); }
    lease.release_empty()
        .map_err(|e| format!("protected release: {}", e.code()))?;
    println!("PASS: kernel-attested service admission and durable empty Job release");
    Ok(())
}
#[cfg(not(all(windows, feature = "protected-recovery-service")))]
fn main() {
    eprintln!("DENIED: protected service test requires Windows opt-in feature");
    std::process::exit(2);
}
