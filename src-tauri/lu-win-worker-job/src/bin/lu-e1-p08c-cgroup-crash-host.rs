//! Dedicated CI-only crash host: uses the production cgroup ownership API.
//! Never distribute this binary; the test SIGKILLs it without running Drop.
#[cfg(target_os="linux")]
fn main(){
    use std::{fs,io::{BufRead,BufReader},process::{Command,Stdio}};
    let receipt=std::path::PathBuf::from(
        std::env::args_os().nth(1).expect("native receipt path"));
    let custody=lu_win_worker_job::linux_cgroup::LinuxCgroupGuardian::spawn()
        .expect("cgroup not delegated: fail native kernel acceptance");
    let mut child=Command::new("/usr/bin/python3");
    child.args(["-u","-c",
        "import subprocess; p=subprocess.Popen(['/bin/sleep','90'], start_new_session=True); print(p.pid,flush=True); p.wait()"])
        .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
    custody.bind_before_exec(&mut child)
        .expect("cannot bind process before exec");
    let mut leader=child.spawn().expect("worker must be born inside cgroup");
    let mut line=String::new();
    BufReader::new(leader.stdout.take().unwrap())
        .read_line(&mut line).expect("escaped child's pid");
    let descendant:u32=line.trim().parse().expect("valid child pid");
    // A genuine cgroup membership check while process is running.
    let membership=fs::read_to_string(format!("/proc/{descendant}/cgroup"))
        .expect("descendant cgroup");
    let expected=custody.leaf().file_name().unwrap().to_string_lossy();
    assert!(membership.contains(expected.as_ref()),
        "setsid descendant not in worker's cgroup subtree");
    fs::write(receipt,format!("{} {} {}\n",
        custody.leaf().display(),leader.id(),descendant))
        .expect("write exact kernel-receipt locator");
    loop{std::thread::park();}
}
#[cfg(not(target_os="linux"))]
fn main(){std::process::exit(98)}
