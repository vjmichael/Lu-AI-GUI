//! E1-P03 actual OS deadline test: valid reviewed executable that intentionally
//! exceeds the trusted host's fixed 15-second deadline. Never returns a
//! protocol receipt before timeout, never accepts model-controlled arguments.
use sha2::{Digest,Sha256};
use lu_core::native_app_execution::{E1_VERSION_MARKER_BYTES,E1_WORKER_MODE};
fn main(){
    let mut args=std::env::args_os().skip(1);
    if args.next().as_deref()!=Some(std::ffi::OsStr::new(E1_WORKER_MODE))
       ||args.next().is_some(){std::process::exit(2)}
    std::thread::sleep(std::time::Duration::from_secs(22));
    let count=E1_VERSION_MARKER_BYTES.len();
    println!("P\t0\t{count}");
    println!("P\t{count}\t{count}");
    println!("D\t{:x}",Sha256::digest(E1_VERSION_MARKER_BYTES));
}
