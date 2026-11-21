// FIN-04W2 fixed native Windows-only hostile WFP test *target* source.
// Reviewed, compiler-debug-info source path is exactly src/main.rs.
// Admin/CI builds it as the original approved PE/PDB. This is NOT a service
// launcher, a general purpose probe or a debugger command. No runtime args.
// The independently observed original Security/5157 WFP events are still
// essential: WSAEACCES alone can also result from AppContainer restrictions.
#[cfg(windows)]
fn main() {
    use std::{
        hint::black_box,
        net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream},
        time::{Duration, Instant},
    };
    fn denied() -> ! { std::process::exit(124) }
    for address in [
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 49281),
        SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 49281),
    ] {
        match TcpStream::connect_timeout(&address, Duration::from_millis(900)) {
            Err(error) if error.raw_os_error() == Some(10013) => {}
            _ => denied(),
        }
    }
    // Keep the original sealed PE Job member alive for the independently
    // authorized trusted LLDB attach and kernel EXCEPTION_DEBUG_EVENT.
    // This intentionally bounded debug fixture cannot modify any source.
    let until = Instant::now() + Duration::from_secs(40);
    let mut counter = 0_u64;
    while Instant::now() < until {
        counter = black_box(counter.wrapping_add(1));
        std::hint::spin_loop();
    }
    black_box(counter);
}
#[cfg(not(windows))]
fn main() { panic!("FIN04W2_NATIVE_WINDOWS_ONLY"); }
