//! Linux-only first-party process-group lifetime guardian.
//! Launched by the native Lu host into a NEW process group (PGID == self PID).
//! A pipe is held open only by Lu; on EOF, including abnormal Lu exit, the
//! guardian kills its OWN group. No PID, path or signal target comes from a
//! model, WebView, argv or a restored SQLite record.
//! Limits: a child that deliberately leaves the process group is not covered.
//! This is an OS lifetime mechanism, not a namespace/sandbox.
#[cfg(target_os="linux")]
fn main(){
    use std::{io::{self,Read,Write},os::raw::c_int};
    unsafe extern "C" {
        fn getpid()->c_int;
        fn getpgrp()->c_int;
        fn kill(pid:c_int,signal:c_int)->c_int;
    }
    // A failed group creation must never make kill(0) target Lu's group.
    if unsafe{getpid()}<=1 || unsafe{getpgrp()!=getpid()}{
        std::process::exit(95)
    }
    let mut ready=io::stdout();
    if ready.write_all(b"READY\n").is_err()||ready.flush().is_err(){
        std::process::exit(96)
    }
    let mut stdin=io::stdin().lock();
    let mut buf=[0u8;32];
    loop{
        match stdin.read(&mut buf){
            Ok(0)=>break,
            Ok(_)=>{},
            Err(ref e) if e.kind()==io::ErrorKind::Interrupted=>{},
            Err(_)=>break,
        }
    }
    // Signal the kernel-created private process group, including this
    // guardian; expected exit is SIGKILL, not a clean exit status.
    unsafe{kill(0,9);}
    std::process::exit(97)
}
#[cfg(not(target_os="linux"))]
fn main(){std::process::exit(98)}
