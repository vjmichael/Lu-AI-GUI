//! CI-only Linux host-crash + escaped-descendant cgroup v2 test parent.
//! Compiled only with the e1-p08-crash-fixture feature. No release packaging.
//! A separate test process SIGKILLs this owner, skips Drop, then requires the
//! out-of-process first-party custodian to terminate a setsid descendant.
#[cfg(target_os="linux")]
fn main(){
    use std::{env,fs,io::{BufRead,BufReader},os::unix::process::CommandExt,
        process::{Command,Stdio},path::PathBuf};
    let args=env::args_os().collect::<Vec<_>>();
    if args.len()!=3{
        eprintln!("Usage: trusted-fixture <host-profile> <evidence-file>");
        std::process::exit(92)
    }
    let profile=PathBuf::from(args[1].clone());
    let output=PathBuf::from(args[2].clone());
    let chat=format!("c-{}","1".repeat(32));
    let turn=format!("g-{}","2".repeat(32));
    let call="agent01l-cgroup-escaped-gh";
    let mut custodian=match lu_win_worker_job::unix_owned::
        LinuxCgroupCustodian::spawn_if_delegated(&profile,&chat,&turn,call){
        Ok(Some(g))=>g,
        Ok(None)=>{
            fs::write(&output,"NOT_AVAILABLE\n").expect("write no-delegation evidence");
            std::process::exit(77)
        }
        Err(e)=>{
            fs::write(&output,format!("ERROR {e}\n"))
                .expect("write failed-delegation evidence");
            std::process::exit(93)
        }
    };
    // Positive custody must survive BOTH setsid() and a descendant that
    // creates a nested cgroup inside this custodian's owned leaf.
    let py=r#"import subprocess,sys,pathlib
raw=pathlib.Path('/proc/self/cgroup').read_text().splitlines()
path=[row[3:] for row in raw if row.startswith('0::')]
if len(path)!=1: raise RuntimeError('untrusted cgroup membership')
root=pathlib.Path('/sys/fs/cgroup')/path[0].lstrip('/')
nested=root/'lu-nested-host-test'
nested.mkdir()
child=subprocess.Popen(['/bin/sleep','90'],start_new_session=True,
    stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,
    stderr=subprocess.DEVNULL)
(nested/'cgroup.procs').write_text(str(child.pid))
print(child.pid,flush=True)
child.wait()
"#;
    let mut cmd=Command::new("/usr/bin/python3");
    cmd.args(["-u","-c",py]).env_clear().process_group(0)
        .stdin(Stdio::null()).stdout(Stdio::piped())
        .stderr(Stdio::null());
    lu_win_worker_job::unix_owned::bind_worker_to_cgroup_before_exec(
        &mut cmd,&custodian
    ).expect("attach before exec");
    lu_win_worker_job::unix_owned::bind_owned_worker_pre_exec(
        &mut cmd,std::process::id(),None
    ).expect("independent parent-death signal");
    let mut worker=cmd.spawn().expect("kernel-gated child exec");
    let mut pid_line=String::new();
    BufReader::new(worker.stdout.take().unwrap()).read_line(&mut pid_line)
        .expect("escaped grandchild identifier");
    let escaped:u32=pid_line.trim().parse().expect("valid escaped PID");
    let owner_cgroup=fs::read_to_string(
        format!("/proc/{}/cgroup",worker.id())
    ).expect("owner cgroup");
    let escaped_cgroup=fs::read_to_string(
        format!("/proc/{escaped}/cgroup")
    ).expect("escaped descendant cgroup");
    // A different nested cgroup is allowed; leaving the custodian's
    // private subtree is not. Both identities come from procfs.
    let expected_nested=format!("{}/lu-nested-host-test",owner_cgroup.trim_end());
    if escaped_cgroup.trim_end()!=expected_nested{
        panic!("setsid descendant did not join the expected nested cgroup")
    }
    let pgid=fs::read_to_string(
        format!("/proc/{escaped}/stat")
    ).expect("kernel proc stat");
    let workerstat=fs::read_to_string(
        format!("/proc/{}/stat",worker.id())
    ).expect("kernel worker stat");
    let group=|s:String|s.rsplit_once(')').expect("proc stat").1
        .split_whitespace().nth(2).expect("pgrp").to_owned();
    if group(pgid)==group(workerstat){
        panic!("Grandchild did not escape the original process group")
    }
    let receipt=profile.join(format!("lu-cgroup-os-{turn}-{call}.receipt"));
    fs::write(&output,format!(
        "READY\nHOST={}\nLEADER={}\nESCAPED={}\nCGROUP={}\nRECEIPT={}\n",
        std::process::id(),worker.id(),escaped,
        custodian.subtree().display(),receipt.display()
    )).expect("persist host-worker test identifiers");
    // The operator kills Lu here. Drop will NOT execute.
    let _=&mut custodian;
    loop{std::thread::park()}
}
#[cfg(not(target_os="linux"))]
fn main(){std::process::exit(98)}
