//! PERF-01 P7L: concrete, independently owned KoboldCpp local GGUF child.
//! Evaluation-only, never registered in installed Lu's backend picker.
//!
//! Upstream KoboldCpp uses --model/--contextsize/--usecpu, KCPP_PASSWORD,
//! authenticated /v1/chat/completions SSE + tool_calls, and --mmproj.
//! This adapter owns the actual child, OS Job/process group, original GGUF
//! pins, authenticated loopback and durable startup/run/reap receipts.
//! The host's original InferenceBridge/TrustedInferenceSupervisor/Protobuf,
//! policy and ANSF work remain the ONLY authority for tool execution.
//!
//! Intentional blockers: CPU-only, no backend auto-fallback or network,
//! no model-provided CLI, no server-side agent, no unverified KV reuse,
//! no production promotion, no claim of full native OS process security.
use crate::{
    browser_tool_stream::CompletedToolCall,
    browser_model_request::build_read_only_browser_tool_request,
    inference_host::{Cancellation,EngineError,GenerationRequest,InferenceBackend,TokenChunk},
    mmproj_admission::AdmittedProjector,
    model_admission::AdmittedModel,
    multimodal_http::{generate_chat_stream_body,generate_browser_tool_stream_body},
    multimodal_saved_turn::{build_structured_chat_request,PinnedImage},
    p7_backend_evaluation::{CandidateKind,VerifiedCandidate},
    p7_kobold_custody::KoboldCustodyJournal,
};
use std::{
    env,
    io::{Read,Write},
    net::{Ipv4Addr,TcpListener,TcpStream,SocketAddr},
    num::NonZeroUsize,
    path::PathBuf,
    process::{Child,Command,Stdio},
    thread,
    time::{Duration,Instant},
};

#[cfg(windows)]
use lu_win_worker_job::Job;
#[cfg(target_os="linux")]
use std::os::unix::process::CommandExt;
#[cfg(target_os="linux")]
unsafe extern "C" {
    fn kill(pid:i32,sig:i32)->i32;
}

#[derive(Debug,Clone)]
pub struct KoboldCppLimits {
    pub threads:NonZeroUsize,
    pub context_tokens:u32,
    pub startup_timeout:Duration,
    pub request_timeout:Duration,
}
pub struct KoboldCppConfig {
    pub candidate:VerifiedCandidate,
    pub limits:KoboldCppLimits,
    pub host_secret:String,
    pub session_id:String,
    /// Dedicated durable journal for this exact test owner/engine lane.
    pub journal_path:PathBuf,
}
pub struct KoboldCppBackend {
    config:KoboldCppConfig,
    journal:KoboldCustodyJournal,
    process:Option<Child>,
    port:Option<u16>,
    projector_loaded:bool,
    journal_armed:bool,
    custody_uncertain:bool,
    #[cfg(windows)]
    job:Option<Job>,
}

fn valid_ref(s:&str)->bool{
    !s.is_empty()&&s.len()<=128&&s.bytes().all(|b|
        b.is_ascii_alphanumeric()||matches!(b,b'-'|b'_'|b'.'))
}
fn valid_key(s:&str)->bool{
    s.len()==64&&s.bytes().all(|b|b.is_ascii_hexdigit())
}
impl KoboldCppBackend {
    pub fn new(config:KoboldCppConfig)->Result<Self,String>{
        if config.candidate.kind!=CandidateKind::KoboldCpp{
            return Err("P7L refuses wrong alternative backend family".into())
        }
        if !valid_key(&config.host_secret)||!valid_ref(&config.session_id)||
            !(256..=262_144).contains(&config.limits.context_tokens)||
            config.limits.threads.get()>64||
            config.limits.startup_timeout.is_zero()||
            config.limits.startup_timeout>Duration::from_secs(600)||
            config.limits.request_timeout.is_zero()||
            config.limits.request_timeout>Duration::from_secs(3600){
            return Err("P7L invalid owner-only resource limits or secret".into())
        }
        config.candidate.recheck_originals()
            .map_err(|_|"P7L original binary/model identity changed")?;
        let journal=KoboldCustodyJournal::new(config.journal_path.clone())
            .map_err(|_|"P7L durable session journal unavailable")?;
        // A stale RUNNING/STARTING record is never automatically cleared
        // merely because the user's Windows session or app restarted.
        if journal.pending().map_err(|_|"P7L journal read failed")?{
            return Err("P7L unreconciled original model process after restart".into())
        }
        Ok(Self{
            config,journal,process:None,port:None,
            projector_loaded:false,journal_armed:false,
            custody_uncertain:false,
            #[cfg(windows)]job:None,
        })
    }
    pub fn pid(&self)->Option<u32>{self.process.as_ref().map(Child::id)}
    pub fn custody_uncertain(&self)->bool{self.custody_uncertain}
    pub fn journal_pending(&self)->Result<bool,String>{
        self.journal.pending().map_err(|_|"P7L journal state unavailable".into())
    }
    fn alive(&mut self)->bool{
        self.process.as_mut().is_some_and(|c|
            matches!(c.try_wait(),Ok(None)))
    }
    fn active_port(&mut self)->Result<u16,String>{
        if !self.alive(){
            self.unload();
            return Err("P7L supervised child exited or is unavailable".into())
        }
        self.port.ok_or("P7L original endpoint missing".into())
    }
    /// POST to a protected *non-generating* token count endpoint. A
    /// different key MUST return 401 and the exact host secret MUST return
    /// 200, otherwise an unowned or unauthenticated listener cannot satisfy
    /// ready. This does not by itself prove OS binary/package identity.
    fn authenticated_probe(port:u16,secret:&str)->Result<bool,String>{
        let mut bad=secret.as_bytes().to_vec();
        bad[0]=if bad[0]==b'0'{b'1'}else{b'0'};
        let invalid=String::from_utf8(bad)
            .map_err(|_|"P7L invalid nonce")?;
        let wrong=probe_token_count(port,&invalid)?;
        let correct=probe_token_count(port,secret)?;
        Ok(wrong==401&&correct==200)
    }
    fn wait_ready(&mut self,deadline:Instant,port:u16)->Result<(),String>{
        loop{
            if !self.alive(){return Err("P7L original child exited before readiness".into())}
            if Instant::now()>=deadline {
                return Err("P7L original child failed authenticated startup".into())
            }
            if Self::authenticated_probe(port,&self.config.host_secret).unwrap_or(false){
                return Ok(())
            }
            thread::sleep(Duration::from_millis(100));
        }
    }
    /// Only the operator's exact reviewed executable, local GGUF and
    /// selected original projector are accepted. No model downloads or
    /// runtime config/env overrides can create other tool authority.
    fn load_native(
        &mut self,admitted:&AdmittedModel,
        projector:Option<&AdmittedProjector>,
    )->Result<(),String>{
        if self.process.is_some()||self.journal_armed||self.custody_uncertain{
            return Err("P7L prior child is still owned or unreconciled".into())
        }
        if admitted.path()!=self.config.candidate.model_path()||
            projector.map(|p|p.path())!=self.config.candidate.projector_path(){
            return Err("P7L model/MMProj differs from reviewed candidate".into())
        }
        admitted.recheck().map_err(|_|"P7L original GGUF admission changed")?;
        if let Some(p)=projector{
            p.recheck().map_err(|_|"P7L original MMProj admission changed")?;
        }
        self.config.candidate.recheck_originals()
            .map_err(|_|"P7L original executable or GGUF bytes changed")?;
        let listener=TcpListener::bind((Ipv4Addr::LOCALHOST,0))
            .map_err(|_|"P7L cannot allocate loopback port")?;
        let port=listener.local_addr().map_err(|_|"P7L invalid local port")?.port();
        drop(listener);
        let binary=self.config.candidate.executable_path();
        let parent=binary.parent().ok_or("P7L candidate package path invalid")?;
        let mut command=Command::new(binary);
        command.arg("--model").arg(admitted.path())
            .arg("--contextsize").arg(self.config.limits.context_tokens.to_string())
            .arg("--threads").arg(self.config.limits.threads.get().to_string())
            .arg("--usecpu").arg("--gpulayers").arg("0")
            .arg("--host").arg("127.0.0.1")
            .arg("--port").arg(port.to_string())
            .arg("--quiet")
            .current_dir(parent)
            .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        if let Some(p)=projector{
            command.arg("--mmproj").arg(p.path());
        }
        // No inherited KCPP_* flag override, loader injection, untrusted
        // model directory, proxy, network cache or agent source variables.
        // Only the native launch code supplies an ephemeral secret.
        command.env_clear()
            .env("KCPP_PASSWORD",&self.config.host_secret)
            .env("HF_HUB_OFFLINE","1")
            .env("HF_DATASETS_OFFLINE","1")
            .env("TRANSFORMERS_OFFLINE","1")
            .env("NO_PROXY","*");
        #[cfg(target_os="linux")]
        {
            command.env("LD_LIBRARY_PATH",parent);
            // Do not accept the operator's LD_PRELOAD/LD_AUDIT/etc.
            command.process_group(0);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000 | 0x0000_0004); // CREATE_NO_WINDOW | CREATE_SUSPENDED
            let root=env::var_os("SystemRoot")
                .ok_or("P7L Windows system runtime location unavailable")?;
            let system=PathBuf::from(&root);
            command.env("SystemRoot",&root)
                .env("WINDIR",&root)
                .env("PATH",env::join_paths([
                    parent.to_path_buf(),system.join("System32")
                ]).map_err(|_|"P7L isolated DLL search path invalid")?);
        }
        #[cfg(windows)]
        let job=Job::new().map_err(|_|"P7L protected Job admission failed")?;
        self.journal.begin(
            &self.config.session_id,
            self.config.candidate.engine_sha256(),
            self.config.candidate.model_sha256()
        ).map_err(|_|"P7L existing unsettled original execution")?;
        self.journal_armed=true;
        // Keep verified original executable/model handles owned by the
        // VerifiedCandidate across the full original process lifetime.
        let mut child=match command.spawn(){
            Ok(child)=>child,
            Err(_)=>{
                self.finish_no_child();
                return Err("P7L reviewed executable did not start".into())
            }
        };
        #[cfg(windows)]
        if job.assign_and_resume_suspended(&child).is_err(){
            let _=child.kill();
            let waited=child.wait().is_ok();
            // Release the original Job handle, including any descendants,
            // BEFORE declaring the corresponding durable lease reaped.
            drop(job);
            self.finish_after_reap(waited);
            return Err("P7L original child refused protected Job custody".into())
        }
        let pid=child.id();
        #[cfg(windows)]
        {self.job=Some(job);}
        self.process=Some(child);
        self.port=Some(port);
        if self.journal.mark_spawned(&self.config.session_id,pid).is_err(){
            self.unload();
            return Err("P7L native child spawned but durable PID record failed".into())
        }
        if let Err(error)=self.wait_ready(
            Instant::now()+self.config.limits.startup_timeout,port){
            self.unload();
            return Err(error)
        }
        self.projector_loaded=projector.is_some();
        Ok(())
    }
    fn finish_no_child(&mut self){
        self.finish_after_reap(true)
    }
    fn finish_after_reap(&mut self,reaped:bool){
        if self.journal_armed {
            if reaped && self.journal.finish_verified(
                &self.config.session_id,true
            ).is_ok(){
                self.journal_armed=false;
            }else{
                self.custody_uncertain=true;
            }
        }
    }
    fn stream(
        &mut self,request:&GenerationRequest,images:&[PinnedImage],
        cancel:&Cancellation,
        emit:&mut dyn FnMut(TokenChunk)->Result<(),EngineError>,
    )->Result<(),String>{
        if cancel.is_cancelled(){
            self.unload();
            return Err("P7L original generation cancelled before send".into())
        }
        if !images.is_empty()&&!self.projector_loaded{
            return Err("P7L no original MMProj; image downgrade forbidden".into())
        }
        let port=self.active_port()?;
        let body=build_structured_chat_request(
            &request.prompt,&request.history,images,request.max_new_tokens
        ).map_err(|_|"P7L invalid scoped original chat request")?;
        let mut index=0u32;
        let result=generate_chat_stream_body(
            port,&self.config.host_secret,&body,
            self.config.limits.request_timeout,
            ||cancel.is_cancelled(),
            |piece|{
                if piece.is_empty(){return Err("P7L empty SSE chunk".into())}
                let next=index.checked_add(1)
                    .ok_or("P7L SSE index overflow")?;
                emit(TokenChunk{text:piece,index})
                    .map_err(|_|"P7L original event consumer unavailable")?;
                index=next;
                Ok(())
            },
        );
        let cancelled=cancel.is_cancelled();
        if result.is_err()||cancelled{
            // Disconnect alone is not proof Kobold stopped decoding;
            // kill/reap the original supervised process. No automatic
            // replay, partial effect approval or silent backend fallback.
            self.unload();
        }
        if cancelled{return Err("P7L original generation cancelled".into())}
        result
    }
}
impl InferenceBackend for KoboldCppBackend {
    fn process_id(&self)->Option<u32>{self.pid()}
    fn load(&mut self,model:&AdmittedModel)->Result<(),String>{
        self.load_native(model,None)
    }
    fn load_with_projector(
        &mut self,model:&AdmittedModel,projector:Option<&AdmittedProjector>,
    )->Result<(),String>{
        self.load_native(model,projector)
    }
    fn unload(&mut self){
        self.projector_loaded=false;
        self.port=None;
        let mut reaped=true;
        if let Some(mut child)=self.process.take(){
            #[cfg(target_os="linux")]
            {
                let pid=child.id();
                if pid>i32::MAX as u32 {
                    reaped=false;
                }else{
                    // The process started in its OWN process group. Signal
                    // group members before waiting for the original parent.
                    let code=unsafe{kill(-(pid as i32),9)};
                    if code!=0&&
                        std::io::Error::last_os_error().raw_os_error()!=Some(3){
                        reaped=false;
                    }
                }
            }
            let _=child.kill();
            if child.wait().is_err(){reaped=false;}
        }
        #[cfg(windows)]
        {
            // The parent exit alone does NOT prove all Job descendants died.
            // Retain the Job handle while querying the native kernel; refuse
            // to clear the durable lease if the Job cannot be proven empty.
            if let Some(job)=self.job.take(){
                if job.terminate_and_verify_empty().is_err(){
                    reaped=false;
                }
                drop(job); // kill-on-close is additional defense, not proof
            }else if self.journal_armed {
                reaped=false;
            }
        }
        self.finish_after_reap(reaped);
    }
    fn set_trusted_cache_scope(&mut self,_scope:Option<&str>){
        // No verified original KV parent protocol on KoboldCpp.
    }
    fn invalidate_prefix_cache(&mut self){
        // No cross-turn reuse or manual cache injection admitted.
    }
    fn generate(
        &mut self,request:&GenerationRequest,cancel:&Cancellation,
        emit:&mut dyn FnMut(TokenChunk)->Result<(),EngineError>
    )->Result<(),String>{
        self.stream(request,&[],cancel,emit)
    }
    fn generate_with_images(
        &mut self,request:&GenerationRequest,images:&[PinnedImage],
        cancel:&Cancellation,
        emit:&mut dyn FnMut(TokenChunk)->Result<(),EngineError>
    )->Result<(),String>{
        if images.is_empty(){return Err("P7L image request has no images".into())}
        self.stream(request,images,cancel,emit)
    }
    fn generate_browser_function_calls(
        &mut self,request:&GenerationRequest,
        admitted_read_schemas:&[serde_json::Value],
        cancel:&Cancellation,
    )->Result<Vec<CompletedToolCall>,String>{
        if cancel.is_cancelled(){
            self.unload();
            return Err("P7L read-only tool generation cancelled".into())
        }
        if admitted_read_schemas.is_empty()||admitted_read_schemas.len()>2{
            return Err("P7L original tool schemas are absent".into())
        }
        let port=self.active_port()?;
        let body=build_read_only_browser_tool_request(
            &request.prompt,&request.history,&[],request.max_new_tokens,
            admitted_read_schemas
        ).map_err(|_|"P7L unapproved tool request")?;
        let result=generate_browser_tool_stream_body(
            port,&self.config.host_secret,&body,
            self.config.limits.request_timeout,
            ||cancel.is_cancelled(),
            std::collections::BTreeSet::from(["lu_browser_read".into()]),
        );
        let cancelled=cancel.is_cancelled();
        if result.is_err()||cancelled{
            self.unload();
        }
        if cancelled{return Err("P7L tool proposal cancelled".into())}
        result
    }
}
impl Drop for KoboldCppBackend {
    fn drop(&mut self){self.unload()}
}

/// Strict small HTTP status reader. A 401 without the host secret plus a 200
/// with it is required to reject unauthenticated/foreign local listeners.
/// This may be invoked without the model being asked to *generate* content.
fn probe_token_count(port:u16,secret:&str)->Result<u16,String>{
    if port==0||!valid_key(secret){
        return Err("P7L probe requires valid local key and port".into())
    }
    let address=SocketAddr::from((Ipv4Addr::LOCALHOST,port));
    let mut socket=TcpStream::connect_timeout(
        &address,Duration::from_millis(150)
    ).map_err(|_|"P7L protected probe unavailable")?;
    socket.set_read_timeout(Some(Duration::from_millis(250)))
        .map_err(|_|"P7L socket timeout")?;
    socket.set_write_timeout(Some(Duration::from_millis(250)))
        .map_err(|_|"P7L socket timeout")?;
    let body=b"{\"prompt\":\"P7L synthetic local readiness probe\"}";
    let header=format!(
        "POST /api/extra/tokencount HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {secret}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    socket.write_all(header.as_bytes()).map_err(|_|"P7L probe send failed")?;
    socket.write_all(body).map_err(|_|"P7L probe send failed")?;
    let mut status=Vec::new();
    for _ in 0..128{
        let mut byte=[0u8;1];
        socket.read_exact(&mut byte).map_err(|_|"P7L no valid HTTP status")?;
        if byte[0]==b'\n'{break}
        status.push(byte[0]);
    }
    let text=std::str::from_utf8(&status).map_err(|_|"P7L invalid HTTP status")?;
    if !text.starts_with("HTTP/1.1 ")&&!text.starts_with("HTTP/1.0 "){
        return Err("P7L foreign HTTP listener".into())
    }
    let code=text.split_whitespace().nth(1)
        .ok_or("P7L missing HTTP code")?
        .parse::<u16>().map_err(|_|"P7L invalid HTTP code")?;
    Ok(code)
}
#[cfg(test)]
mod tests{
    use super::*;
    use std::{fs,io::BufReader,io::BufRead};
    #[test]
    fn refuses_invalid_secret_family_and_probe_port(){
        let mut bad=String::from("a").repeat(64);
        assert!(valid_key(&bad));
        bad.push('z');
        assert!(!valid_key(&bad));
        assert!(!valid_ref("../../../other-profile"));
        assert!(!valid_ref("bad session"));
        assert!(probe_token_count(0,&"a".repeat(64)).is_err());
    }
    #[test]
    fn foreign_passwordless_listener_is_never_accepted_as_original_child(){
        let listener=TcpListener::bind((Ipv4Addr::LOCALHOST,0)).unwrap();
        let port=listener.local_addr().unwrap().port();
        let server=thread::spawn(move||{
            // A foreign server that ignores Authorization is not accepted.
            for _ in 0..2{
                let (mut sock,_)=listener.accept().unwrap();
                let mut reader=BufReader::new(&mut sock);
                loop {
                    let mut line=String::new();
                    if reader.read_line(&mut line).unwrap()==0{break}
                    if line=="\r\n"||line=="\n"{break}
                }
                drop(reader);
                let _=sock.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                );
            }
        });
        assert!(!KoboldCppBackend::authenticated_probe(
            port,&"a".repeat(64)).unwrap());
        server.join().unwrap();
    }
    #[test]
    fn protected_readiness_requires_wrong_key_401_and_original_key_200(){
        let server=TcpListener::bind((Ipv4Addr::LOCALHOST,0)).unwrap();
        let port=server.local_addr().unwrap().port();
        let host_secret="a".repeat(64);
        let secret=host_secret.clone();
        let child=thread::spawn(move||{
            for _ in 0..2{
                let (mut sock,_)=server.accept().unwrap();
                sock.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
                let mut reader=BufReader::new(&mut sock);
                let mut line=String::new();
                let mut incoming=String::new();
                loop {
                    line.clear();
                    reader.read_line(&mut line).unwrap();
                    if line=="\r\n"||line=="\n"{break}
                    incoming.push_str(&line);
                }
                let valid=incoming.contains(&format!("Bearer {secret}"));
                drop(reader);
                let code=if valid{200}else{401};
                let response=format!(
                    "HTTP/1.1 {code} Ready\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                );
                sock.write_all(response.as_bytes()).unwrap();
            }
        });
        assert!(KoboldCppBackend::authenticated_probe(
            port,&host_secret).unwrap());
        child.join().unwrap();
    }
}
