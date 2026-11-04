//! TOOLS-10B native Node/Playwright worker ownership and bounded JSONL protocol.
//! The host alone selects executable, source script, profile, origin grants
//! and optional owned Chromium window. This is not a model-callable shell.
//! An automatic-hidden session requires the actual Chromium Child: owning the
//! Node worker is NOT equivalent to owning the Chromium window.
use crate::{
    browser_host_bridge::VisibilityBridge,
    browser_process_owner::OwnedBrowserWindow,
    browser_visibility_settings::BrowserVisibilityPreference,
};
use serde_json::{json,Value};
use std::{
    io::{self,BufRead,BufReader,Read,Write},
    path::PathBuf,
    process::{Child,ChildStdin,Command,Stdio},
    sync::mpsc::{self,Receiver,RecvTimeoutError},
    thread,
    time::Duration,
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
        && !host.starts_with('.')&&!host.ends_with('.')
        && host.bytes().all(|c|c.is_ascii_alphanumeric()||c==b'.'||c==b'-')
}
impl BrowserLaunchConfig {
    pub fn validate(&self)->Result<(),BrowserSupervisorError>{
        if !self.trusted_node.is_absolute()||!self.trusted_node.is_file()
            ||!self.trusted_script.is_absolute()||!self.trusted_script.is_file()
            ||!self.trusted_profile.is_absolute()||!self.trusted_profile.is_dir()
            ||self.approved_origins.is_empty()||self.approved_origins.len()>32
            ||self.approved_origins.iter().any(|origin|!valid_origin(origin))
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
pub struct OwnedBrowserWorker {
    child:Child,
    stdin:ChildStdin,
    output:Receiver<Result<String,io::Error>>,
    reader:Option<thread::JoinHandle<()>>,
    bridge:VisibilityBridge,
    browser:Option<OwnedBrowserWindow>,
    visibility:BrowserVisibilityPreference,
    timeout:Duration,
    startup_pending:bool,
}
impl OwnedBrowserWorker {
    pub fn launch(config:BrowserLaunchConfig,browser:Option<OwnedBrowserWindow>)
        ->Result<Self,BrowserSupervisorError>{
        config.validate()?;
        if config.visibility==BrowserVisibilityPreference::AutomaticHidden
            && browser.is_none(){
            return Err(BrowserSupervisorError::MissingChromiumCustody);
        }
        let settings=json!({
            "origins":config.approved_origins,
            "profileDirectory":config.trusted_profile,
            "visibilityMode":config.visibility.worker_mode(),
        });
        let mut child=Command::new(&config.trusted_node)
            .arg(&config.trusted_script)
            .env("LU_BROWSER_HOST_CONFIG",settings.to_string())
            .stdin(Stdio::piped()).stdout(Stdio::piped())
            .stderr(Stdio::null()).spawn()
            .map_err(BrowserSupervisorError::Spawn)?;
        let stdin=child.stdin.take().ok_or(BrowserSupervisorError::Protocol)?;
        let stdout=child.stdout.take().ok_or(BrowserSupervisorError::Protocol)?;
        let (tx,rx)=mpsc::channel();
        let reader=thread::spawn(move||{
            let mut reader=BufReader::new(stdout);
            loop {
                // An untrusted worker must not force unbounded allocation
                // by streaming a line without the framing delimiter.
                let mut bytes=Vec::new();
                let result=reader.by_ref().take((MAX_FRAME+2) as u64)
                    .read_until(b'\n',&mut bytes);
                match result {
                    Ok(0)=>break,
                    Ok(_) if bytes.len()<=MAX_FRAME+1&&bytes.last()==Some(&b'\n')=>{
                        let line=String::from_utf8(bytes).map_err(|_|io::Error::new(
                            io::ErrorKind::InvalidData,"non-UTF8 worker frame"));
                        if tx.send(line).is_err(){break;}
                    },
                    Ok(_)=>{
                        let _=tx.send(Err(io::Error::new(
                            io::ErrorKind::InvalidData,"oversized or unterminated worker frame")));
                        break;
                    },
                    Err(e)=>{let _=tx.send(Err(e));break;}
                }
            }
        });
        Ok(Self {
            child,stdin,output:rx,reader:Some(reader),
            bridge:VisibilityBridge::new(),browser,
            visibility:config.visibility,timeout:config.command_timeout,
            startup_pending:true,
        })
    }
    pub fn execute(&mut self,operation:BrowserOperation,arguments:Value)
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
        self.stdin.write_all(encoded.as_bytes())
            .and_then(|_|self.stdin.write_all(b"\n"))
            .and_then(|_|self.stdin.flush())
            .map_err(|_|BrowserSupervisorError::WorkerExited)?;
        // The first worker command also launches the headed browser, which
        // requests the initial native hide before returning READY.
        if self.startup_pending {
            self.startup_pending=false;
            if self.visibility==BrowserVisibilityPreference::AutomaticHidden {
                self.bridge.authorize_next_transition(false);
            }
        }
        let mut first_visibility_done=false;
        loop {
            let frame=self.output.recv_timeout(self.timeout).map_err(|e|match e {
                RecvTimeoutError::Timeout=>BrowserSupervisorError::Deadline,
                RecvTimeoutError::Disconnected=>BrowserSupervisorError::WorkerExited
            })?.map_err(|_|BrowserSupervisorError::WorkerExited)?;
            let value=parse_worker_frame(&frame)?;
            if value.get("type").and_then(Value::as_str)==Some("host_visibility_request"){
                if self.visibility==BrowserVisibilityPreference::AlwaysVisible{
                    return Err(BrowserSupervisorError::UnauthorizedVisibility);
                }
                if !first_visibility_done && self.browser.is_none(){
                    return Err(BrowserSupervisorError::MissingChromiumCustody);
                }
                if !self.bridge_has_pending_start(){
                    let Some(visible)=operation.expected_window_transition() else {
                        return Err(BrowserSupervisorError::UnauthorizedVisibility);
                    };
                    self.bridge.authorize_next_transition(visible);
                }
                let browser=self.browser.as_mut().ok_or(
                    BrowserSupervisorError::MissingChromiumCustody)?;
                let reply=self.bridge.process_owned_window(&frame,browser)
                    .map_err(|_|BrowserSupervisorError::UnauthorizedVisibility)?;
                self.stdin.write_all(reply.as_bytes())
                    .and_then(|_|self.stdin.write_all(b"\n"))
                    .and_then(|_|self.stdin.flush())
                    .map_err(|_|BrowserSupervisorError::WorkerExited)?;
                first_visibility_done=true;
                continue;
            }
            if value.get("ok").and_then(Value::as_bool).is_none(){
                return Err(BrowserSupervisorError::Protocol);
            }
            return Ok(value);
        }
    }
    fn bridge_has_pending_start(&self)->bool{
        self.startup_pending==false && self.bridge.is_transition_authorized()
    }
    pub fn worker_pid(&self)->u32 {self.child.id()}
}
impl Drop for OwnedBrowserWorker {
    fn drop(&mut self){
        let _=self.child.kill();
        let _=self.child.wait();
        if let Some(reader)=self.reader.take(){let _=reader.join();}
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_public_https_origins_are_admitted(){
        assert!(valid_origin("https://archive.example"));
        for candidate in ["http://archive.example","https://x/y",
            "https://foo@bar","https://127.0.0.1:8443",
            "https://evil.example?q=1","https://foo\\bar"] {
            assert!(!valid_origin(candidate),"{candidate}");
        }
    }
    #[test]
    fn parser_refuses_oversized_or_unstructured_frames(){
        assert!(parse_worker_frame("{}").is_ok());
        assert!(parse_worker_frame("[]").is_err());
        assert!(parse_worker_frame(&"x".repeat(MAX_FRAME+1)).is_err());
    }
    #[test]
    fn model_does_not_choose_executable_or_native_window(){
        let _method:fn(&mut OwnedBrowserWorker,BrowserOperation,Value)
            ->Result<Value,BrowserSupervisorError>=OwnedBrowserWorker::execute;
    }
}
