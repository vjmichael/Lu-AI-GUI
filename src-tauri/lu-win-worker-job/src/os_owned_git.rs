//! SEC-06C: original host-reviewed Git, launched through a fixed root-owned
//! systemd service broker rather than a same-UID delegated cgroup leaf.
//!
//! This is NOT a model-facing command API. The installed privileged launcher
//! accepts only the four reviewed Git-read operations; its only ExecStart is
//! a root-owned first-party unprivileged worker, protected by systemd's
//! root-owned cgroup, Delegate=no and ProtectControlGroups=strict.
//! If installation, receipt, or independent kernel custody is absent, Lu
//! never mints a full-descendant cleanup witness.
#![cfg(target_os="linux")]
use std::{
    ffi::{OsStr,OsString},fs::{self,File,OpenOptions},io::{self,Read},
    os::unix::fs::{MetadataExt,OpenOptionsExt,PermissionsExt},
    path::{Path,PathBuf},process::{Child,ChildStdin,Command,Stdio},
    time::{Duration,Instant},
};
const LAUNCHER:&str="/usr/libexec/lu/lu-sec06c-launch-git";
const RECEIPT_ROOT:&str="/var/lib/lu-sec06c/receipts";
const MAX_RECEIPT:u64=1024;
const NOFOLLOW:i32=0o400000;

fn valid_id(s:&str)->bool{
    !s.is_empty()&&s.len()<=128&&s.bytes().all(|b|
        b.is_ascii_alphanumeric()||b==b'_'||b==b'-')
}
fn nonce()->io::Result<String>{
    let mut bytes=[0u8;16];
    File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(bytes.iter().map(|v|format!("{v:02x}")).collect())
}
fn reviewed_op(args:&[OsString])->io::Result<(&Path,&'static str)>{
    if args.len()<7||args.get(0)!=Some(&OsString::from("-c"))
        ||args.get(1)!=Some(&OsString::from("core.fsmonitor=false"))
        ||args.get(2)!=Some(&OsString::from("--no-optional-locks"))
        ||args.get(3)!=Some(&OsString::from("-C")){
        return Err(io::Error::other("SEC06C Git operation not fixed or reviewed"))
    }
    let repo=Path::new(&args[4]);
    if !repo.is_absolute()||!repo.is_dir(){
        return Err(io::Error::other("SEC06C Git repository not canonical"))
    }
    let expected:&[&str]=match args[5].to_str(){
        Some("rev-parse")=>&["rev-parse","--verify","HEAD"],
        Some("status")=>&["status","--porcelain=v1","--untracked-files=no"],
        Some("branch")=>&["branch","--show-current","--no-color"],
        Some("rev-list")=>&["rev-list","--count","HEAD"],
        _=>return Err(io::Error::other("SEC06C forbids arbitrary Git commands")),
    };
    if args.len()!=5+expected.len()||args[5..].iter().zip(expected.iter())
        .any(|(a,e)|a!=OsStr::new(*e)){
        return Err(io::Error::other("SEC06C Git command arguments differ from reviewed op"))
    }
    let op=match args[5].to_str(){
        Some("rev-parse")=>"head",Some("status")=>"status",
        Some("branch")=>"branch",Some("rev-list")=>"count",
        _=>unreachable!(),
    };
    Ok((repo,op))
}
fn root_owned(path:&Path)->io::Result<bool>{
    let meta=fs::symlink_metadata(path)?;
    Ok(meta.is_file()&&!meta.file_type().is_symlink()
        &&meta.uid()==0&&meta.permissions().mode()&0o022==0)
}
pub struct LinuxOsOwnedReviewedGit{
    lifetime:Option<ChildStdin>,
    nonce:String,
    chat:String,
    turn:String,
    call:String,
    digest:String,
    original_uid:u32,
    verified:bool,
}
impl LinuxOsOwnedReviewedGit{
    /// Returns None ONLY when the root-owned broker was never installed.
    /// An installed-but-modified broker is a hard denial, not PGID fallback.
    pub fn spawn_if_installed(
        git:&Path,digest:&str,args:&[OsString],
        chat:&str,turn:&str,call:&str,
    )->io::Result<Option<(Child,Self)>>{
        if !valid_id(chat)||!valid_id(turn)||!valid_id(call)
          ||digest.len()!=64||!digest.bytes().all(|b|
              b.is_ascii_digit()||(b'a'..=b'f').contains(&b)){
            return Err(io::Error::other("SEC06C original reviewed identity invalid"))
        }
        if git.file_name()!=Some(OsStr::new("git"))||!git.is_absolute(){
            return Ok(None) // Not an approved Linux Git binary.
        }
        let (repo,op)=reviewed_op(args)?;
        let exe=Path::new(LAUNCHER);
        if !exe.exists(){return Ok(None)}
        if !root_owned(exe)?||!root_owned(Path::new("/usr/bin/sudo"))?{
            return Err(io::Error::other("SEC06C privileged launcher not root-owned"))
        }
        unsafe extern "C"{fn geteuid()->u32;}
        let uid=unsafe{geteuid()};
        if uid==0{return Err(io::Error::other(
            "SEC06C must not launch reviewed Git as root"))}
        let id=nonce()?;
        let mut command=Command::new("/usr/bin/sudo");
        command.args(["-n",LAUNCHER,"--nonce",&id,"--chat",chat,
            "--turn",turn,"--call",call,"--git"]);
        command.arg(git).arg("--sha").arg(digest)
            .arg("--repo").arg(repo).arg("--op").arg(op);
        command.env_clear().env("PATH","/usr/bin:/bin")
            .stdin(Stdio::piped()).stdout(Stdio::piped())
            .stderr(if cfg!(debug_assertions)
                &&std::env::var_os("LU_SEC06C_BROKER_DEBUG").is_some(){
                Stdio::inherit() // Explicit debug/test invocation only.
            }else{Stdio::null()});
        let mut child=command.spawn()?;
        let lifetime=child.stdin.take().ok_or_else(||io::Error::other(
            "SEC06C host lifetime pipe missing"))?;
        Ok(Some((child,Self{
            lifetime:Some(lifetime),nonce:id,chat:chat.to_owned(),
            turn:turn.to_owned(),call:call.to_owned(),
            digest:digest.to_owned(),original_uid:uid,verified:false,
        })))
    }
    pub fn nonce(&self)->&str{&self.nonce}
    pub fn reviewed_git_digest(&self)->&str{&self.digest}
    /// The proof has a fixed path in a ROOT-OWNED, nonwritable directory.
    /// A worker can neither replace the original kernel receipt nor invent
    /// one with SQLite, model output, or same-user filesystem access.
    pub fn verify_original_kernel_receipt(&mut self)->io::Result<()>{
        if self.verified{return Ok(())}
        let parent=Path::new(RECEIPT_ROOT);
        let meta=fs::symlink_metadata(parent)?;
        if !meta.is_dir()||meta.file_type().is_symlink()
            ||meta.uid()!=0||meta.permissions().mode()&0o022!=0{
            return Err(io::Error::other("SEC06C root receipt directory untrusted"))
        }
        let receipt=parent.join(format!("{}.receipt",self.nonce));
        let mut input=OpenOptions::new().read(true)
            .custom_flags(NOFOLLOW).open(receipt)?;
        let meta=input.metadata()?;
        if !meta.is_file()||meta.uid()!=0||meta.nlink()!=1
           ||meta.permissions().mode()&0o022!=0||meta.len()>MAX_RECEIPT{
            return Err(io::Error::other("SEC06C original kernel receipt was altered"))
        }
        let mut bytes=Vec::new();
        input.take(MAX_RECEIPT+1).read_to_end(&mut bytes)?;
        let raw=String::from_utf8(bytes)
            .map_err(|_|io::Error::other("SEC06C non UTF-8 OS receipt"))?;
        let expected= format!(
            "LU_SEC06C_OWNED_GIT_V1\nNONCE={}\nCHAT={}\nTURN={}\nCALL={}\nGIT_SHA256={}\nUNIT=lu-sec06c-git-{}.service\nORIGINAL_UID={}\nKERNEL_EMPTY=1\n",
            self.nonce,self.chat,self.turn,self.call,self.digest,self.nonce,
            self.original_uid
        );
        if !raw.starts_with(&expected)||
            !matches!(raw.strip_prefix(&expected),
                Some("EFFECT=COMPLETED\n"|"EFFECT=INTERRUPTED\n")){
            return Err(io::Error::other("SEC06C kernel unit identity mismatch"))
        }
        self.verified=true;
        Ok(())
    }
    /// Closing the original pipe signals the first-party managed worker to
    /// terminate. The independently privileged manager must then prove the
    /// kernel cgroup empty; merely killing the sudo/systemd-run client is
    /// NOT descendant containment.
    pub fn stop_and_verify(&mut self,client:&mut Child)->io::Result<()>{
        if self.verified{return Ok(())}
        drop(self.lifetime.take());
        let deadline=Instant::now()+Duration::from_secs(40);
        loop{
            if client.try_wait()?.is_some(){
                return self.verify_original_kernel_receipt()
            }
            if Instant::now()>=deadline{
                return Err(io::Error::other(
                    "SEC06C OS-managed worker custody not reconciled"))
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn reviewed_git_rejects_unbounded_or_write_operations(){
        let repo=std::env::temp_dir();
        for tail in [
            vec!["push","origin","main"],
            vec!["-c","core.fsmonitor=true","status"],
            vec!["rev-parse","HEAD","--exec-path"],
        ]{
            let mut args:Vec<OsString>=["-c","core.fsmonitor=false",
                "--no-optional-locks","-C"].iter().map(|v|OsString::from(*v)).collect();
            args.push(repo.as_os_str().to_owned());
            args.extend(tail.iter().map(|v|OsString::from(*v)));
            assert!(reviewed_op(&args).is_err());
        }
    }
    #[test]
    fn original_ids_and_digests_cannot_be_empty_or_model_forged(){
        let too_long="x".repeat(129);
        for id in ["","../escape",too_long.as_str()]{
            assert!(!valid_id(id))
        }
        assert!(valid_id("c-0123456789abcdef"));
        assert!(!valid_id("a+b"))
    }
}
