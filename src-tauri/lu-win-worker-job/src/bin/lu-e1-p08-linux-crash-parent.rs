//! CI-only abnormal Lu-host exit fixture. NEVER included in a release.
//! Its own lifetime is intentionally ended with SIGKILL by the test runner.
//! A separate guardian must kill the background shell's sleep descendant.
#[cfg(target_os="linux")]
fn main(){
    use std::{io::{BufRead,BufReader},os::unix::process::CommandExt,
        path::PathBuf,process::{Command,Stdio}};
    let path=PathBuf::from(std::env::args_os().nth(1)
        .expect("test-owned output path required"));
    let mut group=lu_win_worker_job::unix_owned::LinuxOwnedGroupGuardian
        ::spawn_if_available().expect("guardian startup")
        .expect("native Linux guardian image must be staged for this test");
    let mut shell=Command::new("/bin/sh")
        .args(["-c","sleep 90 & echo $!; wait"])
        .process_group(group.group_id())
        .stdin(Stdio::null()).stdout(Stdio::piped())
        .stderr(Stdio::null()).spawn().expect("test child");
    let mut line=String::new();
    BufReader::new(shell.stdout.take().unwrap())
        .read_line(&mut line).expect("descendant PID");
    let descendant:u32=line.trim().parse().expect("valid sleep PID");
    std::fs::write(&path,format!(
        "{} {} {}\n",group.group_id(),shell.id(),descendant
    )).expect("write native evidence");
    // A kernel SIGKILL of this owner intentionally skips Rust Drop.
    // The guardian must detect EOF of its only writer and kill its group.
    loop{std::thread::park();}
}
#[cfg(not(target_os="linux"))]
fn main(){std::process::exit(98)}
