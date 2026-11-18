//! Linux Observe & Learn opt-in must be authorized by an OS polkit admin
//! agent, never by a password field in Lu or a model-supplied shell command.
#![cfg(target_os="linux")]
use std::{fs,process::{Command,Stdio},thread,time::{Duration,Instant}};
const ACTION:&str="org.lu.desktop.observe-learn";
fn subject()->Result<String,String>{
    let pid=std::process::id();
    let stat=fs::read_to_string(format!("/proc/{pid}/stat"))
        .map_err(|_|"Cannot attest Linux process creation identity")?;
    // Linux stat field 2 (comm) is parenthesized and can contain spaces.
    // After the final closing parenthesis, field 3 is token 0; field 22
    // starttime is token 19. pkcheck must receive pid,starttime,uid.
    let rest=stat.rsplit_once(") ").ok_or("Invalid Linux process stat")?.1;
    let start=rest.split_whitespace().nth(19)
        .ok_or("Missing Linux process starttime")?;
    let start:u64=start.parse().map_err(|_|"Invalid Linux process starttime")?;
    if start==0{return Err("Zero Linux process starttime".into());}
    let status=fs::read_to_string(format!("/proc/{pid}/status"))
        .map_err(|_|"Cannot attest Linux process UID")?;
    let uid=status.lines().find_map(|line|line.strip_prefix("Uid:"))
        .and_then(|line|line.split_whitespace().next())
        .ok_or("Missing Linux process UID")?;
    let uid:u32=uid.parse().map_err(|_|"Invalid Linux process UID")?;
    Ok(format!("{pid},{start},{uid}"))
}
pub(crate) fn require_admin_for_observation()->Result<(),String>{
    let subject=subject()?;
    // The signed package must install the separate policy in polkit's
    // system actions directory. An absent policy/agent fails closed.
    let mut child=Command::new("/usr/bin/pkcheck")
        .args(["--action-id",ACTION,"--process",&subject,"--allow-user-interaction"])
        .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
        .spawn().map_err(|_|"Linux administrator authorization unavailable")?;
    let deadline=Instant::now()+Duration::from_secs(90);
    loop{
        match child.try_wait(){
            Ok(Some(status)) if status.success()=>return Ok(()),
            Ok(Some(_))=>return Err("Linux administrator did not authorize Observe & Learn; it stays OFF".into()),
            Ok(None) if Instant::now()<deadline=>thread::sleep(Duration::from_millis(200)),
            _=>{
                let _=child.kill();
                let _=child.wait();
                return Err("Linux polkit authentication unavailable or timed out; Observe & Learn stays OFF".into());
            }
        }
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn exact_process_subject_includes_pid_starttime_and_uid(){
        let value=subject().unwrap();
        let p=value.split(',').collect::<Vec<_>>();
        assert_eq!(p.len(),3);
        assert_eq!(p[0].parse::<u32>().unwrap(),std::process::id());
        assert!(p[1].parse::<u64>().unwrap()>0);
        assert!(p[2].parse::<u32>().is_ok());
    }
}
