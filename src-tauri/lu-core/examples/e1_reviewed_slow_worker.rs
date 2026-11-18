//! Slow but otherwise identical reviewed E1 protocol worker fixture.
//! Used ONLY to prove OS process reaping, task revocation and cancellation.
use sha2::{Digest,Sha256};
use lu_core::native_app_execution::{E1_VERSION_MARKER_BYTES,E1_WORKER_MODE};
fn main(){
    let mut args=std::env::args_os().skip(1);
    if args.next().as_deref()!=Some(std::ffi::OsStr::new(E1_WORKER_MODE))
       ||args.next().is_some(){
        std::process::exit(2)
    }
    std::thread::sleep(std::time::Duration::from_secs(8));
    let n=E1_VERSION_MARKER_BYTES.len();
    println!("P\t0\t{n}");
    println!("P\t{n}\t{n}");
    println!("D\t{:x}",Sha256::digest(E1_VERSION_MARKER_BYTES));
}
