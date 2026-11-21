//! Fixed, test-only readback of production Cargo toolchain identity.
#[cfg(all(target_os = "linux", feature = "linux-worker"))]
fn main() {
    match lu_ide_execution_supervisor::linux_worker::installed_toolchain_digest() {
        Ok(digest) => println!("IDE_LINUX05_TOOLCHAIN_SHA256={digest}"),
        Err(e) => {
            eprintln!("IDE_LINUX05_TOOLCHAIN_DENIED={}", e.code());
            std::process::exit(2);
        }
    }
}
#[cfg(not(all(target_os = "linux", feature = "linux-worker")))]
fn main() {
    std::process::exit(2);
}
