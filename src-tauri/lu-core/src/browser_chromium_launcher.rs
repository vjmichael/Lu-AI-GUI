//! Host-only headed Chromium process launch and loopback DevTools admission.
//! This is not an arbitrary browser executable/path tool for the model.
//! A private, dedicated profile and host-owned child are required.
use std::{
    fs::{self,File,OpenOptions},
    io::{self,Read},
    path::{Path,PathBuf},
    process::{Child,Command,Stdio},
    thread,
    time::{Duration,Instant},
};
const PORT_FILE:&str="DevToolsActivePort";
const LOCK_FILE:&str=".lu-owned-browser.lock";
#[derive(Debug)]
pub enum BrowserLaunchError {
    InvalidConfig,
    ProfileInUse,
    Spawn(io::Error),
    StartupFailed,
    Deadline,
    UntrustedEndpoint,
}
/// Keep this guard in the same supervisor that owns the Chromium Child.
/// It prevents a second Lu instance from sharing the same cookies/profile.
pub struct ChromiumProfileLease {
    _file:File,
    lock_path:PathBuf,
}
impl Drop for ChromiumProfileLease {
    fn drop(&mut self){let _=fs::remove_file(&self.lock_path);}
}
pub struct LaunchedChromium {
    child:Option<Child>,
    lease:Option<ChromiumProfileLease>,
    endpoint:String,
    #[cfg(windows)]
    job:Option<lu_win_worker_job::Job>,
}
impl LaunchedChromium {
    pub fn endpoint(&self)->&str{&self.endpoint}
    #[cfg(windows)]
    pub fn take_windows_job(&mut self)->Option<lu_win_worker_job::Job>{
        self.job.take()
    }
    pub fn profile_dir(&self)->&Path{
        self.lease.as_ref().and_then(|l|l.lock_path.parent())
            .expect("owned Chromium always has a profile lock")
    }
    pub fn take_owned_parts(mut self)
      ->(Child,ChromiumProfileLease,String){
        (self.child.take().expect("owned launched child"),
         self.lease.take().expect("owned profile lock"),
         self.endpoint.clone())
    }
}
impl Drop for LaunchedChromium {
    fn drop(&mut self){
        if let Some(mut child)=self.child.take(){
            let _=child.kill();let _=child.wait();
        }
        // Profile lease drops only after process terminates.
    }
}
fn loopback_port_from_file(path:&Path)->Result<Option<u16>,BrowserLaunchError>{
    let metadata=match fs::symlink_metadata(path){
        Ok(meta)=>meta,
        Err(e) if e.kind()==io::ErrorKind::NotFound=>return Ok(None),
        Err(_)=>return Err(BrowserLaunchError::UntrustedEndpoint),
    };
    if !metadata.file_type().is_file()||metadata.len()>512 {
        return Err(BrowserLaunchError::UntrustedEndpoint);
    }
    let mut content=String::new();
    File::open(path).and_then(|mut file|file.take(513).read_to_string(&mut content))
        .map_err(|_|BrowserLaunchError::UntrustedEndpoint)?;
    let mut lines=content.lines();
    let port=lines.next().ok_or(BrowserLaunchError::UntrustedEndpoint)?;
    let websocket=lines.next().ok_or(BrowserLaunchError::UntrustedEndpoint)?;
    // Chromium writes the browser websocket path; never allow an arbitrary
    // remote hostname to appear in a model- or page-influenced response.
    if !websocket.starts_with("/devtools/browser/")
        ||websocket.len()>256||websocket.bytes().any(|b|b.is_ascii_control()){
        return Err(BrowserLaunchError::UntrustedEndpoint);
    }
    let port=port.parse::<u16>().map_err(|_|BrowserLaunchError::UntrustedEndpoint)?;
    if port==0 {return Err(BrowserLaunchError::UntrustedEndpoint);}
    Ok(Some(port))
}
pub fn launch_owned_headed_chromium(
    browser_executable:&Path,
    dedicated_profile:&Path,
    timeout:Duration,
)->Result<LaunchedChromium,BrowserLaunchError>{
    if !browser_executable.is_absolute()||!browser_executable.is_file()
        ||!dedicated_profile.is_absolute()||!dedicated_profile.is_dir()
        ||timeout<Duration::from_millis(100)||timeout>Duration::from_secs(30){
        return Err(BrowserLaunchError::InvalidConfig);
    }
    let lock_path=dedicated_profile.join(LOCK_FILE);
    let lock=OpenOptions::new().write(true).create_new(true).open(&lock_path)
        .map_err(|e|if e.kind()==io::ErrorKind::AlreadyExists{
            BrowserLaunchError::ProfileInUse
        }else{BrowserLaunchError::Spawn(e)})?;
    let lease=ChromiumProfileLease{_file:lock,lock_path};
    // Ignore stale DevTools port records; profile must be fresh or prepared
    // by the trusted host before starting a new child.
    let port_file=dedicated_profile.join(PORT_FILE);
    if port_file.exists(){
        return Err(BrowserLaunchError::InvalidConfig);
    }
    #[cfg(windows)]
    let job=lu_win_worker_job::Job::new().map_err(BrowserLaunchError::Spawn)?;
    let mut child=Command::new(browser_executable)
        .arg("--remote-debugging-port=0")
        .arg("--remote-debugging-address=127.0.0.1")
        .arg("--no-first-run").arg("--no-default-browser-check")
        .arg("--disable-background-networking")
        .arg(format!("--user-data-dir={}",dedicated_profile.display()))
        .arg("about:blank")
        .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
        .spawn().map_err(BrowserLaunchError::Spawn)?;
    #[cfg(windows)]
    if let Err(error)=job.assign(&child){
        let _=child.kill();
        let _=child.wait();
        return Err(BrowserLaunchError::Spawn(error));
    }
    let start=Instant::now();
    loop {
        if child.try_wait().map_err(BrowserLaunchError::Spawn)?.is_some(){
            return Err(BrowserLaunchError::StartupFailed);
        }
        match loopback_port_from_file(&port_file){
            Ok(Some(port))=>{
                return Ok(LaunchedChromium{
                    child:Some(child),lease:Some(lease),
                    endpoint:format!("http://127.0.0.1:{port}"),
                    #[cfg(windows)]
                    job:Some(job)
                });
            },
            Ok(None)=>{},
            Err(e)=>{let _=child.kill();let _=child.wait();return Err(e);}
        }
        if start.elapsed()>=timeout{
            let _=child.kill();let _=child.wait();
            return Err(BrowserLaunchError::Deadline);
        }
        thread::sleep(Duration::from_millis(50));
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_port_file_is_not_interpreted_as_success(){
        let file=std::env::temp_dir().join(
            format!("lu-no-cdp-{}",std::process::id()));
        assert!(matches!(loopback_port_from_file(&file),Ok(None)));
    }
    #[test]
    fn zero_port_or_untrusted_path_never_becomes_a_CDP_endpoint(){
        let path=std::env::temp_dir().join(
            format!("lu-invalid-cdp-{}",std::process::id()));
        fs::write(&path,b"0\n/devtools/browser/abcdef\n").unwrap();
        assert!(matches!(loopback_port_from_file(&path),Err(BrowserLaunchError::UntrustedEndpoint)));
        fs::write(&path,b"44444\nhttp://malicious.test/devtools/browser/abc\n").unwrap();
        assert!(matches!(loopback_port_from_file(&path),Err(BrowserLaunchError::UntrustedEndpoint)));
        let _=fs::remove_file(path);
    }
}
