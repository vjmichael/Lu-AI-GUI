//! SEC-06C dedicated jointly OS-owned Linux Chromium + Node session.
//!
//! This is a fixed service launch, NOT a general privileged command API.
//! The original human chat, turn, task, reviewed SHA-256 images and origin
//! policy are bound to one random nonce in a root-owned systemd unit.
//! Chromium, Node and all their descendants live in the SAME root-owned
//! cgroup leaf; no process-group kill or child exit can witness cleanup.
#![cfg(target_os="linux")]
use std::{
    fs::{self,File,OpenOptions},io::{self,Read},
    os::{fd::AsRawFd,unix::fs::{MetadataExt,OpenOptionsExt,PermissionsExt}},
    path::{Path,PathBuf},process::{Child,Command,Stdio},
    time::{Duration,Instant},
};
const LAUNCHER:&str="/usr/libexec/lu/lu-sec06c-launch-browser";
const RECEIPTS:&str="/var/lib/lu-sec06c/browser-receipts";
const NOFOLLOW:i32=0o400000;
const POLLIN:i16=1;
#[repr(C)]
struct PollFd { fd:i32, events:i16, revents:i16 }
unsafe extern "C" { fn poll(fds:*mut PollFd,nfds:usize,timeout:i32)->i32; fn geteuid()->u32; }

fn hex(raw:&str,len:usize)->bool{
    raw.len()==len&&raw.bytes().all(|c|c.is_ascii_digit()||(b'a'..=b'f').contains(&c))
}
fn id(raw:&str)->bool{
    !raw.is_empty()&&raw.len()<=128&&raw.bytes().all(|b|
        b.is_ascii_alphanumeric()||b==b'-'||b==b'_')
}
fn root_file(path:&Path)->io::Result<bool>{
    let m=fs::symlink_metadata(path)?;
    Ok(m.is_file()&&!m.file_type().is_symlink()&&m.uid()==0
        &&m.permissions().mode()&0o022==0)
}
fn original_nonce()->io::Result<String>{
    let mut bytes=[0u8;16];File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(bytes.iter().map(|x|format!("{x:02x}")).collect())
}
fn safe_origin(s:&str)->bool{
    if s.len()>256||!s.starts_with("https://")
        ||s[8..].contains(['@','?','#','\\','/'].as_slice()) {return false}
    let host=&s[8..];
    host.contains('.')&&!host.starts_with('.')&&!host.ends_with('.')
        &&!host.ends_with(".local")&&!host.ends_with(".internal")
        &&!host.ends_with(".localhost")
        &&host.split('.').all(|part|part.len()>0&&part.len()<=63
            &&part.as_bytes()[0].is_ascii_alphanumeric()
            &&part.as_bytes()[part.len()-1].is_ascii_alphanumeric()
            &&part.bytes().all(|c|c.is_ascii_lowercase()
                ||c.is_ascii_digit()||c==b'-'))
}
fn browser_identity(line:&[u8],nonce:&str)->io::Result<(u32,u32,u16)>{
    let text=std::str::from_utf8(line).map_err(|_|io::Error::other(
        "SEC06C browser readiness not ASCII"))?;
    let parts=text.strip_suffix('\n').ok_or_else(||
        io::Error::other("SEC06C incomplete browser readiness"))?
        .split('\t').collect::<Vec<_>>();
    if parts.len()!=5||parts[0]!="LU_SEC06C_BROWSER_READY_V1"||parts[1]!=nonce{
        return Err(io::Error::other("SEC06C original browser readiness mismatch"))
    }
    let chrome=parts[2].parse::<u32>().map_err(|_|
        io::Error::other("SEC06C Chrome pid not numeric"))?;
    let node=parts[3].parse::<u32>().map_err(|_|
        io::Error::other("SEC06C Node pid not numeric"))?;
    let port=parts[4].parse::<u16>().map_err(|_|
        io::Error::other("SEC06C port not numeric"))?;
    if chrome<=1||node<=1||chrome==node||port==0 {
        return Err(io::Error::other("SEC06C missing original browser process pair"))
    }
    Ok((chrome,node,port))
}
fn bounded_handshake(child:&mut Child,nonce:&str)->io::Result<(u32,u32,u16)>{
    let reader=child.stdout.as_mut().ok_or_else(||
        io::Error::other("SEC06C original browser readiness pipe missing"))?;
    let mut bytes=Vec::new();
    let deadline=Instant::now()+Duration::from_secs(38);
    while Instant::now()<deadline {
        let ms=deadline.saturating_duration_since(Instant::now())
            .as_millis().min(100) as i32;
        let mut fd=PollFd{fd:reader.as_raw_fd(),events:POLLIN,revents:0};
        let status=unsafe{poll(&mut fd,1,ms)};
        if status<0{return Err(io::Error::last_os_error())}
        if status==0{continue}
        let mut b=[0u8;1];
        if reader.read_exact(&mut b).is_err(){
            return Err(io::Error::other("SEC06C browser service exited before ready"))
        }
        bytes.push(b[0]);
        if bytes.len()>256{
            return Err(io::Error::other("SEC06C oversized original browser readiness"))
        }
        if b[0]==b'\n'{return browser_identity(&bytes,nonce)}
    }
    Err(io::Error::other("SEC06C actual Chromium and Node admission timed out"))
}
fn verified_member(pid:u32,unit:&str,nonce:&str)->io::Result<PathBuf>{
    let raw=fs::read_to_string(format!("/proc/{pid}/cgroup"))?;
    let cgroup=raw.trim_end().strip_prefix("0::/").ok_or_else(||
        io::Error::other("SEC06C missing original browser cgroup v2"))?;
    if !cgroup.ends_with(&format!("/{unit}/lu-sec06c-leaf-{nonce}")){
        return Err(io::Error::other("SEC06C browser escaped root-owned unit"))
    }
    let path=Path::new("/sys/fs/cgroup").join(cgroup);
    let meta=fs::symlink_metadata(&path)?;
    if !meta.is_dir()||meta.uid()!=0||meta.permissions().mode()&0o022!=0{
        return Err(io::Error::other("SEC06C browser kernel subtree mutable"))
    }
    Ok(path)
}
pub struct BrowserKernelAdmission {
    pub chromium_pid:u32,
    pub node_pid:u32,
    pub endpoint:String,
    pub cgroup:PathBuf,
}
pub struct LinuxOsOwnedBrowserSession{
    nonce:String,chat:String,turn:String,call:String,
    chromium_sha:String,node_sha:String,script_sha:String,policy_sha:String,
    uid:u32,admission:BrowserKernelAdmission,verified:bool,
}
impl LinuxOsOwnedBrowserSession {
    pub fn installed()->bool{
        let exe=Path::new(LAUNCHER);
        exe.is_file()&&root_file(exe).unwrap_or(false)
            &&root_file(Path::new("/usr/bin/sudo")).unwrap_or(false)
    }
    /// None only if the fixed root-owned broker was never provisioned.
    pub fn spawn_if_installed(
        chromium:&Path,chromium_sha:&str,node:&Path,node_sha:&str,
        script:&Path,script_sha:&str,profile:&Path,policy_sha:&str,
        mode:&str,display:&str,xauthority:Option<&Path>,origins:&[String],
        chat:&str,turn:&str,call:&str,
    )->io::Result<Option<(Child,Self)>>{
        if !Self::installed(){return Ok(None)}
        if !id(chat)||!id(turn)||!id(call)
            ||!chat.starts_with("c-")||chat.len()!=34||!hex(&chat[2..],32)
            ||!turn.starts_with("g-")||turn.len()!=34||!hex(&turn[2..],32)
            ||!hex(chromium_sha,64)||!hex(node_sha,64)
            ||!hex(script_sha,64)||!hex(policy_sha,64)
            ||!matches!(mode,"automatic"|"always-visible")
            ||origins.is_empty()||origins.len()>8
            ||origins.iter().any(|o|!safe_origin(o))
            ||!display.starts_with(':')||display.len()>10
            ||!display[1..].bytes().all(|b|b.is_ascii_digit()||b==b'.')
            ||!profile.is_absolute()||!profile.is_dir()
            ||chromium.file_name().is_none_or(|n|n!="chrome"&&n!="chromium")
            ||node.file_name().is_none_or(|n|n!="node")
            ||script.file_name().is_none_or(|n|n!="lu_browser_worker.mjs"){
            return Err(io::Error::other("SEC06C original browser grant invalid"))
        }
        let uid=unsafe{geteuid()};
        if uid==0{return Err(io::Error::other("SEC06C browser host must not be root"))}
        let nonce=original_nonce()?;
        let mut cmd=Command::new("/usr/bin/sudo");
        cmd.args(["-n",LAUNCHER,"--nonce",&nonce,"--chat",chat,
                  "--turn",turn,"--call",call,"--chromium"]);
        cmd.arg(chromium).arg("--chromium-sha").arg(chromium_sha)
            .arg("--node").arg(node).arg("--node-sha").arg(node_sha)
            .arg("--script").arg(script).arg("--script-sha").arg(script_sha)
            .arg("--profile").arg(profile).arg("--policy-sha").arg(policy_sha)
            .arg("--mode").arg(mode).arg("--display").arg(display);
        if let Some(xauth)=xauthority{
            cmd.arg("--xauthority").arg(xauth);
        }
        for origin in origins{cmd.arg(format!("--origin={origin}"));}
        cmd.env_clear().env("PATH","/usr/bin:/bin")
            .stdin(Stdio::piped()).stdout(Stdio::piped())
            .stderr(if cfg!(debug_assertions)
                &&std::env::var_os("LU_SEC06C_BROKER_DEBUG").is_some(){
                Stdio::inherit()
            }else{Stdio::null()});
        let mut child=cmd.spawn()?;
        let admission=(||{
            let (chrome_pid,node_pid,port)=bounded_handshake(&mut child,&nonce)?;
            let unit=format!("lu-sec06c-browser-{nonce}.service");
            let chrome=verified_member(chrome_pid,&unit,&nonce)?;
            let node_group=verified_member(node_pid,&unit,&nonce)?;
            if chrome!=node_group {
                return Err(io::Error::other("SEC06C Chrome and Node not in ONE kernel leaf"))
            }
            Ok(BrowserKernelAdmission{
                chromium_pid:chrome_pid,node_pid,
                endpoint:format!("http://127.0.0.1:{port}"),
                cgroup:chrome,
            })
        })();
        let admission=match admission{
            Ok(v)=>v,
            Err(e)=>{
                // Admission is uncertain after sudo spawned: never forge
                // a receipt or kill a guessed browser descendant PID.
                drop(child.stdin.take());
                let _=child.kill();let _=child.wait();
                return Err(e);
            }
        };
        Ok(Some((child,Self{
            nonce,chat:chat.to_owned(),turn:turn.to_owned(),call:call.to_owned(),
            chromium_sha:chromium_sha.to_owned(),node_sha:node_sha.to_owned(),
            script_sha:script_sha.to_owned(),policy_sha:policy_sha.to_owned(),
            uid,admission,verified:false,
        })))
    }
    pub fn admission(&self)->&BrowserKernelAdmission{&self.admission}
    pub fn session_live(&self)->bool{
        let unit=format!("lu-sec06c-browser-{}.service",self.nonce);
        let a=verified_member(self.admission.chromium_pid,&unit,&self.nonce);
        let b=verified_member(self.admission.node_pid,&unit,&self.nonce);
        matches!((a,b),(Ok(a),Ok(b)) if a==b&&a==self.admission.cgroup)
    }
    fn receipt(&self)->io::Result<()>{
        let dir=Path::new(RECEIPTS);
        let root=fs::symlink_metadata(dir)?;
        if !root.is_dir()||root.file_type().is_symlink()
            ||root.uid()!=0||root.permissions().mode()&0o022!=0{
            return Err(io::Error::other("SEC06C browser receipt root untrusted"))
        }
        let path=dir.join(format!("{}.receipt",self.nonce));
        let mut file=OpenOptions::new().read(true).custom_flags(NOFOLLOW).open(path)?;
        let meta=file.metadata()?;
        if !meta.is_file()||meta.uid()!=0||meta.nlink()!=1
            ||meta.len()>2048||meta.permissions().mode()&0o022!=0{
            return Err(io::Error::other("SEC06C browser receipt mutable"))
        }
        let mut bytes=Vec::new();(&mut file).take(2049).read_to_end(&mut bytes)?;
        let read=String::from_utf8(bytes)
            .map_err(|_|io::Error::other("SEC06C non-ASCII browser receipt"))?;
        let expected=format!(
            "LU_SEC06C_OWNED_BROWSER_V1\nNONCE={}\nCHAT={}\nTURN={}\nCALL={}\nCHROMIUM_SHA256={}\nNODE_SHA256={}\nSCRIPT_SHA256={}\nPOLICY_SHA256={}\nUNIT=lu-sec06c-browser-{}.service\nORIGINAL_UID={}\nKERNEL_EMPTY=1\nEFFECT=STOPPED\n",
            self.nonce,self.chat,self.turn,self.call,self.chromium_sha,
            self.node_sha,self.script_sha,self.policy_sha,self.nonce,self.uid);
        if read!=expected{return Err(io::Error::other(
            "SEC06C browser original kernel receipt/one-use identity mismatch"))}
        Ok(())
    }
    /// Caller must close the original ChildStdin BEFORE invoking this method.
    /// An unprivileged worker exit never substitutes for the root-owned
    /// manager's independently fsynced recursive kernel-empty receipt.
    pub fn stop_and_verify(&mut self,client:&mut Child)->io::Result<()>{
        if self.verified{return Ok(())}
        let deadline=Instant::now()+Duration::from_secs(42);
        loop{
            if client.try_wait()?.is_some(){
                self.receipt()?;self.verified=true;return Ok(());
            }
            if Instant::now()>=deadline {
                return Err(io::Error::other("SEC06C browser kernel cleanup not attested"))
            }
            std::thread::sleep(Duration::from_millis(30));
        }
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn original_pair_nonce_and_endpoint_not_model_selected(){
        let n="a".repeat(32);
        assert_eq!(browser_identity(
            format!("LU_SEC06C_BROWSER_READY_V1\t{n}\t1204\t1205\t43843\n").as_bytes(),
            &n).unwrap(),(1204,1205,43843));
        for record in [
            format!("LU_SEC06C_BROWSER_READY_V1\t{n}\t1204\t1204\t43843\n"),
            format!("LU_SEC06C_BROWSER_READY_V1\t{n}\t0\t2\t100\n"),
            format!("LU_SEC06C_BROWSER_READY_V1\t{n}\t1\t2\t0\n"),
            "FAKE\t123\t12\t13\t14\n".into(),
            format!("LU_SEC06C_BROWSER_READY_V1\t{n}\t1204\t1205\t43843\nEXTRA"),
        ]{
            assert!(browser_identity(record.as_bytes(),&n).is_err());
        }
    }
    #[test]
    fn browser_origin_scope_refuses_urls_and_local_hosts(){
        for bad in ["http://example.com","https://example.com/path",
            "https://localhost","https://printer.local","https://user@example.com",
            "https://example.com:8443","https://CAPITAL.EXAMPLE"]{
            assert!(!safe_origin(bad),"{bad}");
        }
        assert!(safe_origin("https://example.com"));
    }
}
