//! Fixed CI-only Linux delegated OS host for real protected uv offline miss.
//! Never consumes human authority, accepts arbitrary paths or launches a
//! package manager outside the already pinned supervisor/cgroup worker.
#[cfg(not(target_os="linux"))]
fn main(){std::process::exit(2);}
#[cfg(target_os="linux")]
fn main(){
    if let Err(e)=run(){
        eprintln!("IDE055_NATIVE_DELEGATED_HOST_DENIED: {e}");
        std::process::exit(2);
    }
}
#[cfg(target_os="linux")]
fn run()->Result<(),&'static str>{
    use std::{env,fs,path::Path,process::Command};
    let args=env::args().skip(1).collect::<Vec<_>>();
    if args.as_slice()!=["--offline-miss"]
        && args.as_slice()!=["--cancel"]
        && args.as_slice()!=["--positive"]
        && args.as_slice()!=["--hostile-recovery"]
    {
        return Err("only fixed package acceptance modes");
    }
    let root=Path::new(env!("CARGO_MANIFEST_DIR"));
    let prep=root.join("target/debug/linux_cgroup_prepare");
    let runner=root.join("../supervisor/target/debug/examples/ide05_native_uv_acceptance");
    for path in [&prep,&runner]{
        let meta=fs::symlink_metadata(path)
            .map_err(|_|"fixed native executable missing")?;
        if !meta.is_file()||meta.file_type().is_symlink(){
            return Err("fixed native executable unsafe");
        }
    }
    if !Command::new(&prep).env_clear().status()
        .map_err(|_|"delegated OS preparation could not execute")?.success(){
        return Err("real systemd delegated cgroup not established");
    }
    if args.as_slice() == ["--hostile-recovery"] {
        use std::os::unix::process::ExitStatusExt;
        let status = Command::new(&runner)
            .arg("--crash-after-running")
            .env_clear()
            .env("HOME", "/tmp")
            .env("LANG", "C")
            .env("LU_IDE055_CI_CRASH_AFTER_RUNNING", "1")
            .status()
            .map_err(|_| "fixed crash worker could not start")?;
        if status.signal() != Some(6) {
            return Err("CI interruption was not a genuine SIGABRT");
        }
        if !Command::new(&runner)
            .arg("--recover")
            .env_clear()
            .env("HOME", "/tmp")
            .env("LANG", "C")
            .status()
            .map_err(|_| "independent native journal recovery could not start")?
            .success()
        {
            return Err("kernel-backed journal recovery failed");
        }
    } else if !Command::new(&runner)
        .arg(&args[0])
        .env_clear()
        .env("HOME","/tmp")
        .env("LANG","C")
        .status()
        .map_err(|_|"fixed protected uv fixture could not execute")?
        .success(){
        return Err("native uv acceptance outcome unverified");
    }
    println!("IDE055_REAL_DELEGATED_UV_OS_GATE=PASS; mode={}; user_approval=false; install_accepted=false",args[0]);
    Ok(())
}
