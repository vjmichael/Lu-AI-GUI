//! FIN-04W2-B CI-only harmless native child/descendant test binary.
//! No LLDB, shell, arbitrary command, network listener, or process attach.
//! The independent restricted LocalService SCM broker launches this exact
//! hash-pinned executable into its empty, capped Job before any code runs.
#[cfg(all(windows, debug_assertions, feature = "fin04w2-contained-probe"))]
fn main() {
    use std::{env, fs::{File,OpenOptions}, io::ErrorKind,
        net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream},
        path::Path, process::{self,Command}, thread, time::Duration};
    use windows_sys::Win32::System::JobObjects::IsProcessInJob;
    use windows_sys::Win32::System::Threading::GetCurrentProcess;
    fn fail() -> ! { eprintln!("FIN04W2_FIXED_PROBE=DENIED"); process::exit(4) }
    let args: Vec<_> = env::args_os().collect();
    if args.len() == 2 && args[1] == "--descendant" {
        let mut in_job = 0i32;
        if unsafe { IsProcessInJob(GetCurrentProcess(), std::ptr::null_mut(), &mut in_job) } == 0 || in_job == 0 { fail(); }
        thread::sleep(Duration::from_secs(5));
        process::exit(0);
    }
    if args.len() != 3 || args[1] != "--worker" { fail(); }
    let foreign = Path::new(&args[2]);
    if !foreign.is_absolute() || foreign.to_string_lossy().len() > 384 { fail(); }
    let mut in_job = 0i32;
    if unsafe { IsProcessInJob(GetCurrentProcess(), std::ptr::null_mut(), &mut in_job) } == 0 || in_job == 0 { fail(); }
    let denial = |e: std::io::Error| e.kind() == ErrorKind::PermissionDenied || e.raw_os_error() == Some(5);
    match File::open(foreign) { Err(e) if denial(e) => (), _ => fail() }
    match OpenOptions::new().write(true).open(foreign) { Err(e) if denial(e) => (), _ => fail() }
    // A separate OS-level WFP firewall rule for this exact executable is
    // admin-provisioned and independently read back by CI. A failed socket
    // by itself is NOT proof of WFP enforcement (host could lack Internet).
    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(1,1,1,1)), 443);
    if TcpStream::connect_timeout(&address, Duration::from_secs(2)).is_ok() { fail(); }
    let exe = env::current_exe().unwrap_or_else(|_| fail());
    let mut descendant = Command::new(exe).arg("--descendant").spawn().unwrap_or_else(|_| fail());
    thread::sleep(Duration::from_secs(3));
    if !descendant.wait().map(|s|s.success()).unwrap_or(false) { fail(); }
    process::exit(0);
}
#[cfg(not(all(windows, debug_assertions, feature = "fin04w2-contained-probe")))]
fn main() {
    eprintln!("DENIED: fixed Windows containment fixture is not enabled");
    std::process::exit(2);
}
