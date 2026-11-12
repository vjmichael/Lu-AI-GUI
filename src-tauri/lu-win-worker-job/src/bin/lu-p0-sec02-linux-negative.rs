//! SEC-02 destructive Linux-only fixture; compiled only with CI test feature.
 //! It is NOT a generic process runner and is never installed or model-facing.
 //! The separate verifier owns SIGKILL/SIGSTOP to the exact live guardian PID.
#[cfg(target_os="linux")]
fn main(){
    use std::{env,fs,io::{BufRead,BufReader},os::unix::process::CommandExt,
        path::PathBuf,process::{Command,Stdio},time::{Duration,Instant}};
    let argv=env::args_os().collect::<Vec<_>>();
    if argv.len()!=5{
        eprintln!("Usage: fixture <mode> <host-profile> <evidence> <outcome>");
        std::process::exit(92)
    }
    let mode=argv[1].to_str().expect("mode utf8");
    if !matches!(mode,"guardian-death"|"guardian-stall"|"worker-stall"){
        std::process::exit(92)
    }
    let profile=PathBuf::from(&argv[2]);
    let evidence=PathBuf::from(&argv[3]);
    let outcome=PathBuf::from(&argv[4]);
    let chat=format!("c-{}","7".repeat(32));
    let turn=format!("g-{}","8".repeat(32));
    let call="p0-sec02-negative-gh";
    let mut custody=match lu_win_worker_job::unix_owned::
        LinuxCgroupCustodian::spawn_if_delegated(&profile,&chat,&turn,call){
        Ok(Some(guard))=>guard,
        Ok(None)=>{
            fs::write(outcome,"NOT_AVAILABLE\n").unwrap();
            std::process::exit(77)
        },
        Err(err)=>panic!("No original guardian admission: {err}"),
    };
    let script=r#"import pathlib,subprocess,sys
entries=[x[3:] for x in pathlib.Path('/proc/self/cgroup').read_text().splitlines() if x.startswith('0::')]
if len(entries)!=1: raise RuntimeError('missing kernel cgroup')
root=pathlib.Path('/sys/fs/cgroup')/entries[0].lstrip('/')
nested=root/'p0-sec02-nested'
nested.mkdir()
descendant=subprocess.Popen(['/bin/sleep','90'], start_new_session=True,
    stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
(nested/'cgroup.procs').write_text(str(descendant.pid))
print(descendant.pid,flush=True)
descendant.wait()
"#;
    let mut cmd=Command::new("/usr/bin/python3");
    cmd.args(["-u","-c",script]).env_clear().process_group(0)
        .stdin(Stdio::null()).stdout(Stdio::piped())
        .stderr(Stdio::null());
    lu_win_worker_job::unix_owned::bind_worker_to_cgroup_before_exec(
        &mut cmd,&custody).expect("kernel cgroup pre-exec binding");
    lu_win_worker_job::unix_owned::bind_owned_worker_pre_exec(
        &mut cmd,std::process::id(),None).expect("parent death safety");
    let mut worker=cmd.spawn().expect("controlled Python worker launched");
    custody.verify_admitted_worker(worker.id())
        .expect("real kernel worker admission");
    let mut line=String::new();
    BufReader::new(worker.stdout.take().unwrap())
        .read_line(&mut line).expect("controlled child identity");
    let escaped:u32=line.trim().parse().expect("escaped PID numeric");
    let nested=fs::read_to_string(format!("/proc/{escaped}/cgroup"))
        .expect("original nested kernel membership");
    let parent=fs::read_to_string(format!("/proc/{}/cgroup",worker.id()))
        .expect("original worker membership");
    assert_eq!(nested.trim_end(),format!(
        "{}/p0-sec02-nested",parent.trim_end()));
    let receipt=profile.join(format!("lu-cgroup-os-{turn}-{call}.receipt"));
    // The verifier accepts only this fixed first-party, disposable scope.
    fs::write(&evidence,format!(
        "READY\nMODE={mode}\nHOST={}\nGUARDIAN={}\nLEADER={}\nESCAPED={escaped}\nCGROUP={}\nRECEIPT={}\n",
        std::process::id(),custody.fixture_guardian_pid(),worker.id(),
        custody.subtree().display(),receipt.display()
    )).expect("publish original native fixture identity");
    let trigger=profile.join("p0-sec02-trigger");
    let wait_until=Instant::now()+Duration::from_secs(15);
    while !trigger.exists(){
        if Instant::now()>=wait_until{
            panic!("native negative fixture never received verifier trigger")
        }
        std::thread::sleep(Duration::from_millis(15));
    }
    let result=match mode{
        "guardian-death"=>{
            // No positive OS custody can survive the helper's premature death.
            if custody.require_live_guardian().is_err(){
                let _=worker.kill();let _=worker.wait();
                "GUARDIAN_LOST_UNVERIFIED\n".to_string()
            }else{
                "FAIL_GUARDIAN_UNEXPECTEDLY_ALIVE\n".to_string()
            }
        },
        "guardian-stall"=>{
            // The verifier SIGSTOPs the live guardian, so EOF cannot
            // acknowledge worker termination. 12s host deadline must fail.
            match custody.stop_and_verify(){
                Err(_)=>{
                    let _=worker.kill();let _=worker.wait();
                    "GUARDIAN_STALLED_UNVERIFIED\n".to_string()
                },
                Ok(())=>"FAIL_STALLED_GUARDIAN_CLAIMED_SUCCESS\n".to_string()
            }
        },
        "worker-stall"=>{
            // A deliberately unresponsive worker (plus setsid grandchild)
            // is terminated by a healthy independent kernel custodian.
            match custody.stop_and_verify(){
                Ok(())=>{
                    let _=worker.wait();
                    "WORKER_STALL_NATIVE_CGROUP_EMPTY\n".to_string()
                },
                Err(err)=>format!("FAIL_WORKER_CLEANUP_UNVERIFIED={err}\n"),
            }
        },
        _=>unreachable!(),
    };
    fs::write(outcome,&result).expect("durable observed negative fixture outcome");
    if result.starts_with("FAIL_"){std::process::exit(93)}
}
#[cfg(not(target_os="linux"))]
fn main(){std::process::exit(98)}
