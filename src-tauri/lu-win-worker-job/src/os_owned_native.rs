//! SEC-06C: finite reviewed first-party desktop operations. This is NOT
//! an arbitrary privileged command interface. The only accepted modes are
//! scanner, archive preview/extract, fixed version, fixed artifact and the
//! independently host-reviewed external application version flag.
//!
//! The root-owned OS manager executes a fixed first-party nonroot mediator;
//! it cannot choose an executable from the model or change the arguments.
//! Original SQLite task/turn/nonce + binary/argv hashes are mandatory.
#![cfg(target_os="linux")]
use std::{
    ffi::{OsStr,OsString},
    fs::{self,File,OpenOptions},
    io::{self,Read},
    os::unix::fs::{MetadataExt,OpenOptionsExt,PermissionsExt},
    path::{Path,PathBuf},
    process::{Child,ChildStdin,Command,Stdio},
    time::{Duration,Instant},
};
const LAUNCHER:&str="/usr/libexec/lu/lu-sec06c-launch-native";
const RECEIPTS:&str="/var/lib/lu-sec06c/native-receipts";
const NOFOLLOW:i32=0o400000;
fn hex(s:&str,n:usize)->bool{
    s.len()==n&&s.bytes().all(|c|c.is_ascii_digit()||(b'a'..=b'f').contains(&c))
}
fn id(s:&str)->bool{
    !s.is_empty()&&s.len()<=128&&s.bytes().all(|c|
        c.is_ascii_alphanumeric()||c==b'_'||c==b'-')
}
fn valid_name(s:&str)->bool{
    !s.is_empty()&&s.len()<=128&&!s.starts_with('.')
        &&!s.contains("..")&&s.bytes().all(|c|
            c.is_ascii_alphanumeric()||matches!(c,b'.'|b'_'|b'-'))
}
fn canonical_existing(raw:&str,dir:bool)->bool{
    let p=Path::new(raw);
    p.is_absolute()&&!p.components().any(|x|matches!(x,
       std::path::Component::ParentDir|std::path::Component::CurDir))
       &&p.canonicalize().is_ok_and(|target|target==p
           &&(if dir{target.is_dir()}else{target.is_file()}))
}
fn canonical_output(raw:&str)->bool{
    let p=Path::new(raw);
    let Some(parent)=p.parent()else{return false};
    p.is_absolute()&&parent.canonicalize().is_ok_and(|x|x==parent)
        &&p.file_name().and_then(OsStr::to_str).is_some_and(valid_name)
        &&!p.exists()
}
fn pid(s:&str)->bool{s.parse::<u32>().is_ok_and(|n|n>1)}
/// Exact permitted host source shapes; no external shell, path-based command
/// selection, language interpreter flags or browser endpoints are accepted.
pub fn finite_kind(args:&[OsString])->io::Result<&'static str>{
    let words:Vec<&str>=args.iter().map(|arg|arg.to_str())
        .collect::<Option<Vec<&str>>>()
        .ok_or_else(||io::Error::other("native args not canonical UTF-8"))?;
    let allowed=match words.as_slice(){
        ["--lu-file-scan-worker",path] if canonical_existing(path,false)=>
            Some("scan-file"),
        ["--lu-archive-preview-worker",source,name,output,identity,parent]
            if canonical_existing(source,true)&&valid_name(name)
              &&canonical_output(output)&&hex(identity,32)&&pid(parent)=>
            Some("archive-preview"),
        ["--lu-archive-extract-worker",source,name,dest,target,digest,sid,did,parent]
            if canonical_existing(source,true)&&valid_name(name)
              &&canonical_existing(dest,true)&&valid_name(target)
              &&hex(digest,64)&&hex(sid,32)&&hex(did,32)&&pid(parent)=>
            Some("archive-extract"),
        ["--lu-e1-version-one-shot"]=>Some("fixed-version"),
        ["--lu-e1-artifact-one-shot",("docx"|"xlsx")]=>Some("fixed-artifact"),
        [("--version"|"-version")]=>Some("app-version"),
        _=>None
    };
    allowed.ok_or_else(||io::Error::other(
        "SEC06C native reviewed worker finite argv contract denied"))
}
fn original_nonce()->io::Result<String>{
    let mut b=[0u8;16];File::open("/dev/urandom")?.read_exact(&mut b)?;
    Ok(b.iter().map(|v|format!("{v:02x}")).collect())
}
fn is_root_file(path:&Path)->io::Result<bool>{
    let m=fs::symlink_metadata(path)?;
    Ok(m.is_file()&&!m.file_type().is_symlink()&&m.uid()==0
        &&m.permissions().mode()&0o022==0)
}
pub struct LinuxOsOwnedReviewedNative{
    lifetime:Option<ChildStdin>,
    nonce:String,chat:String,turn:String,call:String,kind:String,
    binary_sha:String,args_sha:String,uid:u32,verified:bool,
}
impl LinuxOsOwnedReviewedNative{
    pub fn spawn_if_installed(
        executable:&Path,binary_sha:&str,args:&[OsString],args_sha:&str,
        chat:&str,turn:&str,call:&str,
    )->io::Result<Option<(Child,Self)>>{
        if !id(chat)||!id(turn)||!id(call)
            ||!hex(binary_sha,64)||!hex(args_sha,64){
            return Err(io::Error::other("SEC06C native original turn/hash invalid"))
        }
        let kind=finite_kind(args)?;
        let launcher=Path::new(LAUNCHER);
        if !launcher.exists(){return Ok(None)}
        if !is_root_file(launcher)?||!is_root_file(Path::new("/usr/bin/sudo"))?{
            return Err(io::Error::other("SEC06C native root-owned launcher missing"))
        }
        unsafe extern "C"{fn geteuid()->u32;}
        let uid=unsafe{geteuid()};
        if uid==0{return Err(io::Error::other("SEC06C native worker must be nonroot"))}
        let nonce=original_nonce()?;
        let mut cmd=Command::new("/usr/bin/sudo");
        cmd.args(["-n",LAUNCHER,"--nonce",&nonce,"--chat",chat,
                  "--turn",turn,"--call",call,"--kind",kind,
                  "--binary"]);
        cmd.arg(executable).arg("--binary-sha").arg(binary_sha)
            .arg("--args-sha").arg(args_sha);
        for arg in args{
            let value=arg.to_str().ok_or_else(||
                io::Error::other("SEC06C native non-UTF8 argument forbidden"))?;
            cmd.arg(format!("--arg={value}"));
        }
        cmd.env_clear().env("PATH","/usr/bin:/bin")
            .stdin(Stdio::piped()).stdout(Stdio::piped())
            .stderr(if cfg!(debug_assertions)
                &&std::env::var_os("LU_SEC06C_BROKER_DEBUG").is_some(){
                Stdio::inherit()
            }else{Stdio::null()});
        let mut child=cmd.spawn()?;
        let lifetime=child.stdin.take()
            .ok_or_else(||io::Error::other("SEC06C native lifetime pipe absent"))?;
        Ok(Some((child,Self{
            lifetime:Some(lifetime),nonce,chat:chat.into(),turn:turn.into(),
            call:call.into(),kind:kind.into(),binary_sha:binary_sha.into(),
            args_sha:args_sha.into(),uid,verified:false
        })))
    }
    pub fn nonce(&self)->&str{&self.nonce}
    pub fn kind(&self)->&str{&self.kind}
    pub fn binary_sha(&self)->&str{&self.binary_sha}
    pub fn args_sha(&self)->&str{&self.args_sha}
    pub fn verify_original_kernel_receipt(&mut self)->io::Result<()>{
        if self.verified{return Ok(())}
        let dir=Path::new(RECEIPTS);
        let m=fs::symlink_metadata(dir)?;
        if !m.is_dir()||m.file_type().is_symlink()||m.uid()!=0
            ||m.permissions().mode()&0o022!=0{
            return Err(io::Error::other("SEC06C root-native receipt directory invalid"))
        }
        let mut f=OpenOptions::new().read(true).custom_flags(NOFOLLOW)
            .open(dir.join(format!("{}.receipt",self.nonce)))?;
        let m=f.metadata()?;
        if !m.is_file()||m.uid()!=0||m.nlink()!=1
            ||m.permissions().mode()&0o022!=0||m.len()>1024{
            return Err(io::Error::other("SEC06C altered native OS kernel receipt"))
        }
        let mut buf=Vec::new();(&mut f).take(1025).read_to_end(&mut buf)?;
        let raw=String::from_utf8(buf)
            .map_err(|_|io::Error::other("SEC06C invalid native receipt encoding"))?;
        let expected=format!(
            "LU_SEC06C_OWNED_NATIVE_V1\nNONCE={}\nCHAT={}\nTURN={}\nCALL={}\nKIND={}\nBINARY_SHA256={}\nARGV_SHA256={}\nUNIT=lu-sec06c-native-{}.service\nORIGINAL_UID={}\nKERNEL_EMPTY=1\n",
            self.nonce,self.chat,self.turn,self.call,self.kind,self.binary_sha,
            self.args_sha,self.nonce,self.uid);
        if !raw.starts_with(&expected)
            ||!matches!(raw.strip_prefix(&expected),
                Some("EFFECT=COMPLETED\n"|"EFFECT=INTERRUPTED\n")){
            return Err(io::Error::other("SEC06C root-owned native receipt mismatch"))
        }
        self.verified=true;Ok(())
    }
    pub fn stop_and_verify(&mut self,child:&mut Child)->io::Result<()>{
        if self.verified{return Ok(())}
        drop(self.lifetime.take());
        let deadline=Instant::now()+Duration::from_secs(55);
        loop{
            if child.try_wait()?.is_some(){return self.verify_original_kernel_receipt()}
            if Instant::now()>=deadline{
                return Err(io::Error::other("SEC06C native original OS cleanup timeout"))
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    fn v(args:&[&str])->Vec<OsString>{
        args.iter().map(|x|OsString::from(*x)).collect()
    }
    #[test]
    fn reject_arbitrary_exec_and_archive_escape(){
        for args in [
            vec!["-c","print('no')"],
            vec!["--lu-file-scan-worker","https://private.invalid"],
            vec!["--lu-archive-extract-worker","/tmp","../x","/tmp","out"],
            vec!["--lu-archive-preview-worker","/tmp","a.zip","/tmp/out"],
            vec!["--lu-e1-artifact-one-shot","sh"],
            vec!["--version","--eval"],
            vec!["--lu-file-scan-worker","/proc/self/environ","--output=/tmp"],
        ]{
            assert!(finite_kind(&v(&args)).is_err(),"{args:?}");
        }
        assert_eq!(finite_kind(&v(&["--lu-e1-version-one-shot"])).unwrap(),
            "fixed-version");
        assert_eq!(finite_kind(&v(&["--lu-e1-artifact-one-shot","docx"])).unwrap(),
            "fixed-artifact");
        assert_eq!(finite_kind(&v(&["--version"])).unwrap(),"app-version");
    }
}
