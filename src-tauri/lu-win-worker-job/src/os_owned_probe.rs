//! SEC-06C finite read-only media/metadata supervisor, separate from Git.
//! Only FFprobe WAV/MP4, PDFInfo PDF and ExifTool PNG/JPEG are admitted.
//! The pinned reviewed binary/resource SHA pair and original human turn are
//! bound to a separate one-use root-owned systemd unit with no generic CLI.
#![cfg(target_os="linux")]
use std::{
  ffi::{OsStr,OsString},fs::{self,File,OpenOptions},io::{self,Read},
  os::unix::fs::{MetadataExt,OpenOptionsExt,PermissionsExt},
  path::{Path,PathBuf},process::{Child,ChildStdin,Command,Stdio},
  time::{Duration,Instant},
};
const LAUNCHER:&str="/usr/libexec/lu/lu-sec06c-launch-probe";
const RECEIPTS:&str="/var/lib/lu-sec06c/probe-receipts";
const NOFOLLOW:i32=0o400000;
fn valid_id(s:&str)->bool{
    !s.is_empty()&&s.len()<=128&&s.bytes().all(|c|
        c.is_ascii_alphanumeric()||c==b'_'||c==b'-')
}
fn hex(s:&str,n:usize)->bool{
    s.len()==n&&s.bytes().all(|c|c.is_ascii_digit()||(b'a'..=b'f').contains(&c))
}
fn nonce()->io::Result<String>{
    let mut b=[0u8;16];
    File::open("/dev/urandom")?.read_exact(&mut b)?;
    Ok(b.iter().map(|x|format!("{x:02x}")).collect())
}
fn root_owned(path:&Path)->io::Result<bool>{
    let m=fs::symlink_metadata(path)?;
    Ok(m.is_file()&&!m.file_type().is_symlink()&&m.uid()==0
        &&m.permissions().mode()&0o022==0)
}
fn kind_and_input<'a>(exe:&Path,args:&'a [OsString])
    ->io::Result<(&'static str,&'a Path)>{
    let name=exe.file_name().and_then(OsStr::to_str)
        .ok_or_else(||io::Error::other("SEC06C invalid finite executable"))?;
    let words:Vec<&str>=args.iter().map(|x|x.to_str())
        .collect::<Option<Vec<&str>>>()
        .ok_or_else(||io::Error::other("SEC06C probe arguments must be UTF-8"))?;
    let (kind,index)=if name=="ffprobe"&&words.len()==12
        &&words[0..5]==["-v","error","-protocol_whitelist","file","-f"]
        &&words[6..11]==["-show_entries",
            "format=duration:stream=index,codec_type,codec_name,width,height,sample_rate,channels",
            "-of","json","-i"]{
        match words[5]{
            "wav"=>("ffprobe-wav",11),
            "mov"=>("ffprobe-mp4",11),
            _=>return Err(io::Error::other("SEC06C unreviewed FFprobe demuxer")),
        }
    }else if name=="pdfinfo"&&words.len()==2
        &&words[0]=="-isodates"{
        ("pdfinfo-pdf",1)
    }else if name=="exiftool"&&words.len()==8
        &&words[0..7]==["-j","-n","-ImageWidth","-ImageHeight",
                        "-FileType","-FileSize#","--"]{
        ("exiftool-image",7)
    }else{
        return Err(io::Error::other("SEC06C forbids arbitrary media/metadata argv"))
    };
    let path=Path::new(&args[index]);
    if !path.is_absolute()||!path.is_file()||path.components().any(|c|
        matches!(c,std::path::Component::CurDir
              |std::path::Component::ParentDir)){
        return Err(io::Error::other("SEC06C original selected input unavailable"))
    }
    let ext=path.extension().and_then(OsStr::to_str).unwrap_or("");
    let permitted=match kind{
        "ffprobe-wav"=>ext.eq_ignore_ascii_case("wav"),
        "ffprobe-mp4"=>ext.eq_ignore_ascii_case("mp4"),
        "pdfinfo-pdf"=>ext.eq_ignore_ascii_case("pdf"),
        "exiftool-image"=>matches!(ext.to_ascii_lowercase().as_str(),
                                    "png"|"jpg"|"jpeg"),
        _=>false
    };
    if !permitted{return Err(io::Error::other("SEC06C wrong source media type"))}
    Ok((kind,path))
}
pub struct LinuxOsOwnedReviewedProbe{
    lifetime:Option<ChildStdin>,
    nonce:String,chat:String,turn:String,call:String,
    kind:String,binary_sha:String,resource_sha:String,uid:u32,
    verified:bool,
}
impl LinuxOsOwnedReviewedProbe{
    pub fn spawn_if_installed(
        exe:&Path,expected_binary_sha:&str,args:&[OsString],
        reviewed_input_sha:&str,chat:&str,turn:&str,call:&str,
    )->io::Result<Option<(Child,Self)>>{
        if !valid_id(chat)||!valid_id(turn)||!valid_id(call)
            ||!hex(expected_binary_sha,64)||!hex(reviewed_input_sha,64){
            return Err(io::Error::other("SEC06C original probe authorization invalid"))
        }
        let (kind,input)=kind_and_input(exe,args)?;
        let launcher=Path::new(LAUNCHER);
        if !launcher.exists(){return Ok(None)}
        if !root_owned(launcher)?||!root_owned(Path::new("/usr/bin/sudo"))?{
            return Err(io::Error::other("SEC06C probe launcher not root-owned"))
        }
        unsafe extern "C"{fn geteuid()->u32;}
        let uid=unsafe{geteuid()};
        if uid==0{return Err(io::Error::other("SEC06C root media worker forbidden"))}
        let nonce=nonce()?;
        let mut cmd=Command::new("/usr/bin/sudo");
        cmd.args(["-n",LAUNCHER,"--nonce",&nonce,"--chat",chat,
                  "--turn",turn,"--call",call,"--kind",kind,"--binary"]);
        cmd.arg(exe).arg("--binary-sha").arg(expected_binary_sha)
            .arg("--file").arg(input).arg("--resource-sha")
            .arg(reviewed_input_sha);
        cmd.env_clear().env("PATH","/usr/bin:/bin")
            .stdin(Stdio::piped()).stdout(Stdio::piped())
            .stderr(if cfg!(debug_assertions)
                &&std::env::var_os("LU_SEC06C_BROKER_DEBUG").is_some(){
                Stdio::inherit()
            }else{Stdio::null()});
        let mut child=cmd.spawn()?;
        let lifetime=child.stdin.take()
            .ok_or_else(||io::Error::other("SEC06C original lifetime pipe missing"))?;
        Ok(Some((child,Self{
            lifetime:Some(lifetime),nonce,chat:chat.into(),turn:turn.into(),
            call:call.into(),kind:kind.into(),
            binary_sha:expected_binary_sha.into(),resource_sha:reviewed_input_sha.into(),
            uid,verified:false,
        })))
    }
    pub fn nonce(&self)->&str{&self.nonce}
    pub fn kind(&self)->&str{&self.kind}
    pub fn executable_sha(&self)->&str{&self.binary_sha}
    pub fn resource_sha(&self)->&str{&self.resource_sha}
    pub fn verify_original_kernel_receipt(&mut self)->io::Result<()>{
        if self.verified{return Ok(())}
        let root=Path::new(RECEIPTS);
        let r=fs::symlink_metadata(root)?;
        if !r.is_dir()||r.file_type().is_symlink()
            ||r.uid()!=0||r.permissions().mode()&0o022!=0{
            return Err(io::Error::other("SEC06C root probe receipts directory untrusted"))
        }
        let path=root.join(format!("{}.receipt",self.nonce));
        let mut file=OpenOptions::new().read(true)
            .custom_flags(NOFOLLOW).open(path)?;
        let m=file.metadata()?;
        if !m.is_file()||m.uid()!=0||m.nlink()!=1
            ||m.permissions().mode()&0o022!=0||m.len()>1024{
            return Err(io::Error::other("SEC06C altered root-owned probe receipt"))
        }
        let mut bytes=Vec::new();
        (&mut file).take(1025).read_to_end(&mut bytes)?;
        let read=String::from_utf8(bytes)
            .map_err(|_|io::Error::other("SEC06C invalid probe receipt encoding"))?;
        let expected=format!(
            "LU_SEC06C_OWNED_PROBE_V1\nNONCE={}\nCHAT={}\nTURN={}\nCALL={}\nKIND={}\nBINARY_SHA256={}\nRESOURCE_SHA256={}\nUNIT=lu-sec06c-probe-{}.service\nORIGINAL_UID={}\nKERNEL_EMPTY=1\n",
            self.nonce,self.chat,self.turn,self.call,self.kind,
            self.binary_sha,self.resource_sha,self.nonce,self.uid
        );
        if !read.starts_with(&expected)||
            !matches!(read.strip_prefix(&expected),
                Some("EFFECT=COMPLETED\n"|"EFFECT=INTERRUPTED\n")){
            return Err(io::Error::other("SEC06C mismatched first-party kernel receipt"))
        }
        self.verified=true;
        Ok(())
    }
    pub fn stop_and_verify(&mut self,child:&mut Child)->io::Result<()>{
        if self.verified{return Ok(())}
        drop(self.lifetime.take());
        let deadline=Instant::now()+Duration::from_secs(40);
        loop{
            if child.try_wait()?.is_some(){return self.verify_original_kernel_receipt()}
            if Instant::now()>=deadline{
                return Err(io::Error::other("SEC06C native OS probe not reconciled"))
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn reject_unreviewed_probe_flags_and_formats(){
        let r=std::env::temp_dir();
        for (program,args) in [
            ("ffprobe",vec!["-i","https://example.test/stream"]),
            ("ffprobe",vec!["-v","error","-f","hls"]),
            ("exiftool",vec!["-overwrite_original","-all="]),
            ("pdfinfo",vec!["-upw","hunter2"]),
        ]{
            let a:Vec<OsString>=args.iter().map(|x|OsString::from(*x)).collect();
            assert!(kind_and_input(Path::new(program),&a).is_err())
        }
        assert!(!r.join("does-not-exist").exists());
    }
}
