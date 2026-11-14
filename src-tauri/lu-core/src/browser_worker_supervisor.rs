//! TOOLS-10B native Node/Playwright worker ownership and bounded JSONL protocol.
//! The host alone selects executable, source script, profile, origin grants
//! and optional owned Chromium window. This is not a model-callable shell.
//! An automatic-hidden session requires the actual Chromium Child: owning the
//! Node worker is NOT equivalent to owning the Chromium window.
use crate::{
    browser_host_bridge::VisibilityBridge,
    browser_process_owner::OwnedBrowserWindow,
    browser_chromium_launcher::{LaunchedChromium,ChromiumProfileLease},

    browser_visibility_settings::BrowserVisibilityPreference,
};
use serde_json::{json,Value};
#[cfg(target_os="linux")]
use crate::browser_chromium_launcher::acquire_original_browser_profile;
#[cfg(target_os="linux")]
use lu_win_worker_job::os_owned_browser::LinuxOsOwnedBrowserSession;
#[cfg(target_os="linux")]
use sha2::{Digest,Sha256};
#[cfg(target_os="linux")]
use std::{fs::File,io::Read,path::Path};
use std::{
    io::{self,BufRead,BufReader,Write},
    path::PathBuf,
    process::{Child,ChildStdin,Command,Stdio},
    sync::mpsc::{self,Receiver,RecvTimeoutError},
    thread,
    time::{Duration,Instant},
};
const MAX_FRAME:usize=3_000_000;
#[derive(Debug)]
pub enum BrowserSupervisorError {
    InvalidConfig,
    Spawn(io::Error),
    Protocol,
    Deadline,
    WorkerExited,
    UnauthorizedVisibility,
    MissingChromiumCustody,
    KernelCustodyUnavailable,
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum BrowserOperation {
    Status,Navigate,Read,Click,ClickSelector,Type,Press,Scroll,
    Accessibility,Screenshot,Back,Forward,HumanResolved,Show,Hide,
}
impl BrowserOperation {
    pub fn operation_name(self)->&'static str{
        match self {
            Self::Status=>"status",Self::Navigate=>"navigate",Self::Read=>"read",
            Self::Click=>"click",Self::ClickSelector=>"click_selector",
            Self::Type=>"type",Self::Press=>"press",Self::Scroll=>"scroll",
            Self::Accessibility=>"accessibility",Self::Screenshot=>"screenshot",
            Self::Back=>"back",Self::Forward=>"forward",
            Self::HumanResolved=>"human_resolved",Self::Show=>"show_browser",
            Self::Hide=>"hide_browser",
        }
    }
    fn expected_window_transition(self)->Option<bool>{
        match self {
            Self::Navigate|Self::Read|Self::Show=>Some(true),
            Self::Hide|Self::HumanResolved=>Some(false),
            _=>None,
        }
    }
}
pub struct BrowserLaunchConfig {
    pub trusted_node:PathBuf,
    pub trusted_script:PathBuf,
    pub trusted_profile:PathBuf,
    pub approved_origins:Vec<String>,
    /// Host-discovered DevTools endpoint of a separately launched Chromium.
    pub cdp_endpoint:Option<String>,
    pub visibility:BrowserVisibilityPreference,
    pub command_timeout:Duration,
}
fn valid_origin(input:&str)->bool{
    if input.len()>256||!input.starts_with("https://")||input.contains('@')
        ||input.contains('?')||input.contains('#')||input.contains('\\'){
        return false;
    }
    let host=&input[8..];
    !host.is_empty()&&!host.contains('/')&&!host.contains(':')
        && !host.starts_with('.')&&!host.ends_with('.')&&host.contains('.')
        &&!host.ends_with(".local")&&!host.ends_with(".internal")
        &&!host.ends_with(".localhost")
        &&!host.bytes().any(|b|b.is_ascii_uppercase())
        &&host.rsplit('.').next().is_some_and(|tld|
            tld.len()>=2&&tld.bytes().all(|c|c.is_ascii_lowercase()))
        && host.split('.').all(|label| !label.is_empty() && label.len()<=63
            && label.as_bytes().first().is_some_and(u8::is_ascii_alphanumeric)
            && label.as_bytes().last().is_some_and(u8::is_ascii_alphanumeric))
        && host.bytes().all(|c|c.is_ascii_alphanumeric()||c==b'.'||c==b'-')
}
fn valid_loopback_cdp(input:&str)->bool {
    let Some(port)=input.strip_prefix("http://127.0.0.1:") else{return false;};
    !port.is_empty()&&port.bytes().all(|b|b.is_ascii_digit())
        &&port.parse::<u16>().ok().is_some_and(|n|n>0)
}
impl BrowserLaunchConfig {
    pub fn validate(&self)->Result<(),BrowserSupervisorError>{
        if !self.trusted_node.is_absolute()||!self.trusted_node.is_file()
            ||!self.trusted_script.is_absolute()||!self.trusted_script.is_file()
            ||!self.trusted_profile.is_absolute()||!self.trusted_profile.is_dir()
            ||self.approved_origins.is_empty()||self.approved_origins.len()>32
            ||self.approved_origins.iter().any(|origin|!valid_origin(origin))
            ||self.cdp_endpoint.as_ref().is_some_and(|endpoint|!valid_loopback_cdp(endpoint))
            ||self.command_timeout<Duration::from_millis(100)
            ||self.command_timeout>Duration::from_secs(60){
            return Err(BrowserSupervisorError::InvalidConfig);
        }
        Ok(())
    }
}
fn parse_worker_frame(line:&str)->Result<Value,BrowserSupervisorError>{
    if line.len()>MAX_FRAME{return Err(BrowserSupervisorError::Protocol);}
    let result:Value=serde_json::from_str(line)
        .map_err(|_|BrowserSupervisorError::Protocol)?;
    if !result.is_object(){return Err(BrowserSupervisorError::Protocol);}
    Ok(result)
}
/// Read only a single bounded JSONL frame. Reading through BufRead's
/// internal buffer permits a strict cap even for a malicious unterminated line.
fn read_bounded_worker_line<R:BufRead>(reader:&mut R)->io::Result<Option<String>>{
    let mut bytes=Vec::new();
    loop {
        let buf=reader.fill_buf()?;
        if buf.is_empty(){
            if bytes.is_empty(){return Ok(None);}
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof,
                "unterminated browser-worker frame"));
        }
        let count=buf.iter().position(|&byte|byte==b'\n')
            .map_or(buf.len(),|index|index+1);
        if bytes.len()+count>MAX_FRAME+1 {
            return Err(io::Error::new(io::ErrorKind::InvalidData,
                "browser-worker frame exceeded limit"));
        }
        let completed=buf[count-1]==b'\n';
        bytes.extend_from_slice(&buf[..count]);
        reader.consume(count);
        if completed {
            let line=String::from_utf8(bytes)
                .map_err(|_|io::Error::new(io::ErrorKind::InvalidData,
                    "non-UTF8 browser-worker frame"))?;
            return Ok(Some(line));
        }
    }
}
pub struct OwnedBrowserWorker {
    child:Child,
    stdin:Option<ChildStdin>,
    output:Receiver<Result<String,io::Error>>,
    reader:Option<thread::JoinHandle<()>>,
    bridge:VisibilityBridge,
    browser:Option<OwnedBrowserWindow>,
    _profile_lease:Option<ChromiumProfileLease>,
    #[cfg(target_os="linux")]
    kernel_session:Option<LinuxOsOwnedBrowserSession>,
    visibility:BrowserVisibilityPreference,
    timeout:Duration,
    startup_pending:bool,
    failed:bool,
    #[cfg(windows)]
    _node_job:Option<lu_win_worker_job::Job>,
    #[cfg(windows)]
    _chromium_job:Option<lu_win_worker_job::Job>,
}
#[cfg(target_os="linux")]
fn require_joint_os_owned_node_scope()->Result<(),BrowserSupervisorError>{
    Err(BrowserSupervisorError::KernelCustodyUnavailable)
}

#[cfg(target_os="linux")]
fn original_image_sha(path:&Path)->Result<String,BrowserSupervisorError>{
    let mut file=File::open(path).map_err(BrowserSupervisorError::Spawn)?;
    let mut h=Sha256::new();
    let mut chunk=[0u8;65536];
    loop{
        let size=file.read(&mut chunk).map_err(BrowserSupervisorError::Spawn)?;
        if size==0{break;}
        h.update(&chunk[..size]);
    }
    Ok(format!("{:x}",h.finalize()))
}
#[cfg(target_os="linux")]
fn original_browser_policy_sha(config:&BrowserLaunchConfig)->Result<String,BrowserSupervisorError>{
    let profile=config.trusted_profile.to_str()
        .ok_or(BrowserSupervisorError::InvalidConfig)?;
    let mut hasher=Sha256::new();
    for part in std::iter::once(profile)
        .chain(std::iter::once(config.visibility.worker_mode()))
        .chain(config.approved_origins.iter().map(String::as_str)){
        hasher.update(part.as_bytes());hasher.update([0u8]);
    }
    Ok(format!("{:x}",hasher.finalize()))
}
impl OwnedBrowserWorker {
    #[cfg(target_os="linux")]
    pub fn sec06c_installed()->bool{
        LinuxOsOwnedBrowserSession::installed()
    }
    #[cfg(target_os="linux")]
    pub fn sec06c_reconcile_previous(profile:&Path)->Result<(),BrowserSupervisorError>{
        if !profile.exists(){return Ok(())}
        LinuxOsOwnedBrowserSession::reconcile_previous(profile)
            .map_err(BrowserSupervisorError::Spawn)
    }
    pub fn launch(config:BrowserLaunchConfig,browser:Option<OwnedBrowserWindow>)
        ->Result<Self,BrowserSupervisorError>{
        // Before verified joint SEC-06C browser custody exists, the old
        // stand-alone Node entry point cannot execute on Linux even when
        // called outside NativeBrowserHost or its reviewed grant path.
        #[cfg(target_os="linux")]
        require_joint_os_owned_node_scope()?;
        config.validate()?;
        if config.visibility==BrowserVisibilityPreference::AutomaticHidden
            && (browser.is_none()||config.cdp_endpoint.is_none()){
            return Err(BrowserSupervisorError::MissingChromiumCustody);
        }
        let settings=json!({
            "origins":config.approved_origins,
            "profileDirectory":config.trusted_profile,
            "visibilityMode":config.visibility.worker_mode(),
            "cdpEndpoint":config.cdp_endpoint,
        });
        #[cfg(windows)]
        let node_job=lu_win_worker_job::Job::new()
            .map_err(BrowserSupervisorError::Spawn)?;
        let mut child=Command::new(&config.trusted_node)
            .arg(&config.trusted_script)
            .env("LU_BROWSER_HOST_CONFIG",settings.to_string())
            .stdin(Stdio::piped()).stdout(Stdio::piped())
            .stderr(Stdio::null()).spawn()
            .map_err(BrowserSupervisorError::Spawn)?;
        #[cfg(windows)]
        if let Err(error)=node_job.assign(&child){
            let _=child.kill();let _=child.wait();
            return Err(BrowserSupervisorError::Spawn(error));
        }
        let stdin=child.stdin.take().ok_or(BrowserSupervisorError::Protocol)?;
        let stdout=child.stdout.take().ok_or(BrowserSupervisorError::Protocol)?;
        // Bound worker output even when Node floods frames faster than the host
        // can process them. Backpressure stops untrusted memory growth.
        let (tx,rx)=mpsc::sync_channel(32);
        let reader=thread::spawn(move||{
            let mut reader=BufReader::new(stdout);
            loop {
                // An untrusted worker must not force unbounded allocation
                // by streaming a line without the framing delimiter.
                let result=read_bounded_worker_line(&mut reader);
                match result {
                    Ok(None)=>break,
                    Ok(Some(line))=>{if tx.send(Ok(line)).is_err(){break;}},
                    Err(e)=>{let _=tx.send(Err(e));break;}
                }
            }
        });
        Ok(Self {
            child,stdin:Some(stdin),output:rx,reader:Some(reader),
            bridge:VisibilityBridge::new(),browser,_profile_lease:None,
            #[cfg(target_os="linux")]
            kernel_session:None,
            visibility:config.visibility,timeout:config.command_timeout,
            startup_pending:true,failed:false,
            #[cfg(windows)]
            _node_job:Some(node_job),
            #[cfg(windows)]
            _chromium_job:None,
        })
    }
    /// Joint Linux browser startup under the dedicated, fixed root-owned
    /// SEC-06C unit. Node and headed Chromium share ONE non-user-writable
    /// cgroup leaf. The original user-turn task ID, binary SHA-256s, and
    /// exact HTTPS origin policy are bound to a single one-use OS nonce.
    #[cfg(target_os="linux")]
    pub fn launch_sec06c(
        mut config:BrowserLaunchConfig,chromium:&Path,
        trusted_xdotool:PathBuf,
        chat:&str,turn:&str,call:&str,
        display:&str,xauthority:Option<&Path>,
    )->Result<Self,BrowserSupervisorError>{
        config.validate()?;
        // Replay denial on restart: inspect every previous user-owned
        // admission journal against the independent exact root-owned kernel
        // receipt BEFORE issuing a fresh session. Old grants are never reused.
        LinuxOsOwnedBrowserSession::reconcile_previous(&config.trusted_profile)
            .map_err(BrowserSupervisorError::Spawn)?;
        if config.cdp_endpoint.is_some()||!chromium.is_absolute()
            ||!chromium.is_file()||!trusted_xdotool.is_absolute()
            ||!trusted_xdotool.is_file(){
            return Err(BrowserSupervisorError::InvalidConfig);
        }
        let lease=acquire_original_browser_profile(&config.trusted_profile)
            .map_err(|_|BrowserSupervisorError::InvalidConfig)?;
        let chromium_sha=original_image_sha(chromium)?;
        let node_sha=original_image_sha(&config.trusted_node)?;
        let script_sha=original_image_sha(&config.trusted_script)?;
        let policy_sha=original_browser_policy_sha(&config)?;
        let (mut child,mut kernel)=LinuxOsOwnedBrowserSession::spawn_if_installed(
            chromium,&chromium_sha,
            &config.trusted_node,&node_sha,
            &config.trusted_script,&script_sha,
            &config.trusted_profile,&policy_sha,
            config.visibility.worker_mode(),display,xauthority,
            &config.approved_origins,chat,turn,call,
        ).map_err(BrowserSupervisorError::Spawn)?
            .ok_or(BrowserSupervisorError::KernelCustodyUnavailable)?;
        let admission=kernel.admission();
        config.cdp_endpoint=Some(admission.endpoint.clone());
        let browser=OwnedBrowserWindow::from_sec06c_admission(
            admission.chromium_pid,admission.cgroup.clone(),trusted_xdotool);
        let browser=match browser{
            Ok(x)=>x,
            Err(_)=>{
                drop(child.stdin.take());
                let _=kernel.stop_and_verify(&mut child);
                return Err(BrowserSupervisorError::MissingChromiumCustody);
            }
        };
        let stdin=child.stdin.take()
            .ok_or(BrowserSupervisorError::Protocol)?;
        let stdout=child.stdout.take()
            .ok_or(BrowserSupervisorError::Protocol)?;
        let (tx,rx)=mpsc::sync_channel(32);
        let reader=thread::spawn(move||{
            let mut reader=BufReader::new(stdout);
            loop{
                match read_bounded_worker_line(&mut reader){
                    Ok(None)=>break,
                    Ok(Some(line))=>if tx.send(Ok(line)).is_err(){break;},
                    Err(err)=>{let _=tx.send(Err(err));break;}
                }
            }
        });
        Ok(Self{
            child,stdin:Some(stdin),output:rx,reader:Some(reader),
            bridge:VisibilityBridge::new(),browser:Some(browser),
            _profile_lease:Some(lease),kernel_session:Some(kernel),
            visibility:config.visibility,timeout:config.command_timeout,
            startup_pending:true,failed:false,
        })
    }
    /// Establish matching ownership: same profile, same host-launched
    /// Chromium Child and loopback CDP endpoint. No second Chromium launch.
    #[cfg(windows)]
    pub fn launch_attached(mut config:BrowserLaunchConfig,mut launched:LaunchedChromium)
      ->Result<Self,BrowserSupervisorError>{
        if config.trusted_profile!=launched.profile_dir() {
            return Err(BrowserSupervisorError::InvalidConfig);
        }
        let chromium_job=launched.take_windows_job()
            .ok_or(BrowserSupervisorError::MissingChromiumCustody)?;
        let (child,lease,endpoint)=launched.take_owned_parts();
        config.cdp_endpoint=Some(endpoint);
        let owner=OwnedBrowserWindow::new(child)
            .map_err(|_|BrowserSupervisorError::MissingChromiumCustody)?;
        let mut worker=Self::launch(config,Some(owner))?;
        worker._profile_lease=Some(lease);
        worker._chromium_job=Some(chromium_job);
        Ok(worker)
    }
    #[cfg(target_os="linux")]
    pub fn launch_attached(mut config:BrowserLaunchConfig,launched:LaunchedChromium,
      trusted_xdotool:PathBuf)->Result<Self,BrowserSupervisorError>{
        if config.trusted_profile!=launched.profile_dir() {
            return Err(BrowserSupervisorError::InvalidConfig);
        }
        let (child,lease,endpoint)=launched.take_owned_parts();
        config.cdp_endpoint=Some(endpoint);
        let owner=OwnedBrowserWindow::new(child,trusted_xdotool)
            .map_err(|_|BrowserSupervisorError::MissingChromiumCustody)?;
        let mut worker=Self::launch(config,Some(owner))?;
        worker._profile_lease=Some(lease);
        Ok(worker)
    }
    pub fn execute(&mut self,operation:BrowserOperation,arguments:Value)
        ->Result<Value,BrowserSupervisorError>{
        if self.failed {return Err(BrowserSupervisorError::WorkerExited);}
        let result=self.execute_inner(operation,arguments);
        if result.is_err(){
            // Late replies after a deadline must never be mistaken for
            // replies to a future command. Poison this session and kill its
            // entire owned browser; recovery must explicitly launch anew.
            self.failed=true;
            // Linux cancellation closes the ORIGINAL root broker pipe and
            // requires its independent one-use cgroup-empty receipt. Never
            // kill only the sudo PID and call Chrome/Node descendants gone.
            let _=self.stop_verified();
        }
        result
    }
    fn execute_inner(&mut self,operation:BrowserOperation,arguments:Value)
        ->Result<Value,BrowserSupervisorError>{
        let Some(args)=arguments.as_object() else {
            return Err(BrowserSupervisorError::Protocol);
        };
        if args.contains_key("op")||args.contains_key("type"){
            return Err(BrowserSupervisorError::Protocol);
        }
        let mut request=args.clone();
        request.insert("op".into(),json!(operation.operation_name()));
        let encoded=Value::Object(request).to_string();
        if encoded.len()>8192{return Err(BrowserSupervisorError::Protocol);}
        self.stdin.as_mut().ok_or(BrowserSupervisorError::WorkerExited)?
            .write_all(encoded.as_bytes())
            .and_then(|_|self.stdin.as_mut().ok_or_else(||io::Error::other("closed browser owner"))?
                .write_all(b"\n"))
            .and_then(|_|self.stdin.as_mut().ok_or_else(||io::Error::other("closed browser owner"))?
                .flush())
            .map_err(|_|BrowserSupervisorError::WorkerExited)?;
        // The first worker command also launches the headed browser, which
        // requests the initial native hide before returning READY.
        if self.startup_pending {
            self.startup_pending=false;
            if self.visibility==BrowserVisibilityPreference::AutomaticHidden {
                self.bridge.authorize_next_transition(false);
            }
        }
        let mut operation_visibility_used=false;
        // One whole-command deadline, including host visibility handshakes:
        // repeated worker events must not refresh the time budget.
        let deadline=Instant::now()+self.timeout;
        loop {
            let remaining=deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero(){return Err(BrowserSupervisorError::Deadline);}
            let frame=self.output.recv_timeout(remaining).map_err(|e|match e {
                RecvTimeoutError::Timeout=>BrowserSupervisorError::Deadline,
                RecvTimeoutError::Disconnected=>BrowserSupervisorError::WorkerExited
            })?.map_err(|_|BrowserSupervisorError::WorkerExited)?;
            let value=parse_worker_frame(&frame)?;
            if value.get("type").and_then(Value::as_str)==Some("host_visibility_request"){
                if self.visibility==BrowserVisibilityPreference::AlwaysVisible{
                    return Err(BrowserSupervisorError::UnauthorizedVisibility);
                }
                if self.browser.is_none(){
                    return Err(BrowserSupervisorError::MissingChromiumCustody);
                }
                if !self.bridge.is_transition_authorized(){
                    // Exactly one operation-specific transition is permitted.
                    // The initial automatic hide is a distinct launch event.
                    if operation_visibility_used {
                        return Err(BrowserSupervisorError::UnauthorizedVisibility);
                    }
                    let Some(visible)=operation.expected_window_transition() else {
                        return Err(BrowserSupervisorError::UnauthorizedVisibility);
                    };
                    self.bridge.authorize_next_transition(visible);
                    operation_visibility_used=true;
                }
                let browser=self.browser.as_mut().ok_or(
                    BrowserSupervisorError::MissingChromiumCustody)?;
                let reply=self.bridge.process_owned_window(&frame,browser)
                    .map_err(|_|BrowserSupervisorError::UnauthorizedVisibility)?;
                self.stdin.as_mut().ok_or(BrowserSupervisorError::WorkerExited)?
                    .write_all(reply.as_bytes())
                    .and_then(|_|self.stdin.as_mut().ok_or_else(||io::Error::other("closed browser owner"))?
                        .write_all(b"\n"))
                    .and_then(|_|self.stdin.as_mut().ok_or_else(||io::Error::other("closed browser owner"))?
                        .flush())
                    .map_err(|_|BrowserSupervisorError::WorkerExited)?;
                continue;
            }
            if value.get("ok").and_then(Value::as_bool).is_none(){
                return Err(BrowserSupervisorError::Protocol);
            }
            return Ok(value);
        }
    }
    /// Health reflects the real Node and Chromium child processes, not the
    /// presence of an in-memory session reference.
    pub fn is_alive(&mut self)->bool {
        !self.failed && matches!(self.child.try_wait(),Ok(None))
            &&self.browser.as_mut().is_some_and(|browser|browser.is_alive())
            &&{
                #[cfg(target_os="linux")]
                {self.kernel_session.as_ref().is_none_or(|session|session.session_live())}
                #[cfg(not(target_os="linux"))]
                {true}
            }
    }
    pub fn worker_pid(&self)->u32 {
        #[cfg(target_os="linux")]
        if let Some(scope)=self.kernel_session.as_ref(){return scope.admission().node_pid}
        self.child.id()
    }
    /// Stop the root-owned joint browser subtree and independently verify
    /// the durable fsynced kernel-empty receipt before publishing a stop.
    pub fn stop_verified(&mut self)->Result<(),BrowserSupervisorError>{
        self.failed=true;
        drop(self.stdin.take());
        #[cfg(target_os="linux")]
        if let Some(scope)=self.kernel_session.as_mut(){
            scope.stop_and_verify(&mut self.child)
                .map_err(BrowserSupervisorError::Spawn)?;
            self.browser.take();
            return Ok(())
        }
        let _=self.child.kill();
        self.child.wait().map_err(BrowserSupervisorError::Spawn)?;
        self.browser.take();
        Ok(())
    }
}
impl Drop for OwnedBrowserWorker {
    fn drop(&mut self){
        let _=self.stop_verified();
        // A descendant may have inherited stdout. Waiting forever on a
        // blocking reader thread would deadlock Lu shutdown; detach rather
        // than join until OS process-tree containment is fully verified.
        self.reader.take();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_public_https_origins_are_admitted(){
        assert!(valid_origin("https://archive.example"));
        assert!(valid_loopback_cdp("http://127.0.0.1:41007"));
        assert!(!valid_loopback_cdp("http://127.0.0.1:99999"));
        assert!(!valid_loopback_cdp("http://localhost:41007"));
        for candidate in ["http://archive.example","https://x/y",
            "https://foo@bar","https://127.0.0.1:8443",
            "https://evil.example?q=1","https://foo\\bar",
            "https://127.0.0.1","https://printer.local","https://LOCAL.Example",
            "https://a..example","https://-host.example","https://host-.example"] {
            assert!(!valid_origin(candidate),"{candidate}");
        }
    }
    #[test]
    fn origin_rejects_oversized_dns_label(){
        let oversized=format!("https://{}.example","a".repeat(64));
        assert!(!valid_origin(&oversized));
        let maximum=format!("https://{}.example","a".repeat(63));
        assert!(valid_origin(&maximum));
    }
    #[test]
    fn parser_refuses_oversized_or_unstructured_frames(){
        assert!(parse_worker_frame("{}").is_ok());
        assert!(parse_worker_frame("[]").is_err());
        assert!(parse_worker_frame(&"x".repeat(MAX_FRAME+1)).is_err());
    }
    #[test]
    fn worker_framing_accepts_valid_lines_and_refuses_truncated_frames(){
        let mut reader=std::io::BufReader::new(b"{\"ok\":true}\n{\"ok\":false}\n".as_slice());
        assert_eq!(read_bounded_worker_line(&mut reader).unwrap().as_deref(),Some("{\"ok\":true}\n"));
        assert_eq!(read_bounded_worker_line(&mut reader).unwrap().as_deref(),Some("{\"ok\":false}\n"));
        assert_eq!(read_bounded_worker_line(&mut reader).unwrap(),None);
        let mut truncated=std::io::BufReader::new(b"{\"ok\":true}".as_slice());
        assert_eq!(read_bounded_worker_line(&mut truncated).unwrap_err().kind(),
            io::ErrorKind::UnexpectedEof);
    }
    #[test]
    fn oversized_worker_lines_are_rejected_before_allocation(){
        let oversized=vec![b'x';MAX_FRAME+200];
        let mut reader=std::io::BufReader::new(oversized.as_slice());
        assert_eq!(read_bounded_worker_line(&mut reader).unwrap_err().kind(),
            io::ErrorKind::InvalidData);
    }

    /// The actual root-owned fixed browser package and real Xvfb display
    /// are provisioned in the independent Linux SEC-06C native CI job.
    /// This is NEVER a release acceptance or a model-selected executable.
    #[cfg(target_os="linux")]
    #[test]
    #[ignore="requires real systemd-owned Chromium+Node/Xvfb admission"]
    fn sec06c_real_linux_browser_original_grant_handoff_and_durable_recovery(){
        use std::fs;
        let bundle=PathBuf::from(std::env::var_os("LU_SEC06C_BROWSER_REVIEWED_BUNDLE")
            .expect("must pin original reviewed Linux CI browser bundle"));
        let chromium=bundle.join("chrome");
        let xdotool=bundle.join("xdotool");
        let profile=std::env::temp_dir().join(format!(
            "lu-sec06c-native-browser-{}-{}",std::process::id(),
            std::time::SystemTime::now().duration_since(
                std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir(&profile).expect("create original one-use dedicated profile");
        let display=std::env::var("DISPLAY").expect("real Xvfb DISPLAY required");
        let xauthority=std::env::var_os("XAUTHORITY").map(PathBuf::from);
        let cfg=BrowserLaunchConfig{
            trusted_node:bundle.join("node"),
            trusted_script:bundle.join("lu_browser_worker.mjs"),
            trusted_profile:profile.clone(),
            approved_origins:vec!["https://example.com".into()],
            cdp_endpoint:None,
            visibility:BrowserVisibilityPreference::AutomaticHidden,
            command_timeout:Duration::from_secs(30),
        };
        let mut worker=OwnedBrowserWorker::launch_sec06c(
            cfg,&chromium,xdotool,
            &format!("c-{}","a".repeat(32)),
            &format!("g-{}","b".repeat(32)),
            "browser-task-real-native", &display,xauthority.as_deref(),
        ).expect("genuine original root-owned Chromium+Node admission");
        assert!(worker.is_alive(),
            "original Chromium, Node and root-owned leaf must still exist");
        let initial=worker.execute(BrowserOperation::Status,json!({}))
            .expect("genuine nonroot browser initial human-hide handshake");
        assert_eq!(initial["ok"],true);
        assert_eq!(initial["result"]["windowVisibility"],"HIDDEN");
        let shown=worker.execute(BrowserOperation::Show,json!({}))
            .expect("original Chromium human-visible request");
        assert_eq!(shown["result"]["windowVisibility"],"VISIBLE");
        let hidden=worker.execute(BrowserOperation::Hide,json!({}))
            .expect("original Chromium human-hiding recheck");
        assert_eq!(hidden["result"]["windowVisibility"],"HIDDEN");
        worker.stop_verified().expect("root cgroup.kill, empty and fsynced receipt");
        assert!(LinuxOsOwnedBrowserSession::reconcile_previous(&profile).is_ok(),
            "restart may only verify original immutable root receipt, never replay");
        fs::remove_dir_all(&profile).expect("remove finished ephemeral CI profile");
        println!("SEC06C_ORIGINAL_RUST_BROWSER_KERNEL_PASS=1 original_turn=1 real_browser=1 independent_root_receipt=1 restart_no_replay=1");
    }
    #[test]
    fn model_does_not_choose_executable_or_native_window(){
        let _method:fn(&mut OwnedBrowserWorker,BrowserOperation,Value)
            ->Result<Value,BrowserSupervisorError>=OwnedBrowserWorker::execute;
    }
}
