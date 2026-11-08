//! E1-P08C first-party cgroup-v2 cleanup process.
//! Kept OUTSIDE the worker leaf so Lu SIGKILL leaves a live kernel cleaner.
//! Never accepts PID, worker path, or commands from argv/stdin.
#[cfg(target_os="linux")]
fn main(){
    if let Err(error)=lu_win_worker_job::linux_cgroup::guardian_main(){
        eprintln!("Lu cgroup guard: {error}");
        std::process::exit(70);
    }
}
#[cfg(not(target_os="linux"))]
fn main(){std::process::exit(98)}
