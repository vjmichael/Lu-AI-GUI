//! Persistent Lu-owned GGUF engine based on an explicitly pinned llama.cpp
//! \`llama-server\` binary. One supervised child retains weights across calls.
//!
//! The authenticated 127.0.0.1 HTTP interface is an implementation boundary
//! to upstream llama.cpp, NOT an internal Lu/MCP tool protocol. Lu commands,
//! policies and model/tool events remain Protobuf-native in the host.
//! Windows alpha requests Vulkan GPU offload; Linux keeps CPU baseline.
//! Real GPU offload remains a physical Windows acceptance gate.
//! A signed, immutable engine package and model-file identity are required
//! before a production release.
use crate::inference_host::{
    Cancellation, EngineError, GenerationRequest, InferenceBackend, TokenChunk,
};
use crate::backend_restart_ledger::{BackendRestartLedger,RestartAdmission};
use crate::mmproj_admission::AdmittedProjector;
use crate::model_admission::AdmittedModel;
use crate::prefix_kv_cache::PromptCacheGate;
use crate::multimodal_http::{generate_chat_stream_body,generate_verified_image_body,ImageDeliveryReceipt,image_failure_code};
use crate::multimodal_saved_turn::{build_structured_chat_request, PinnedImage};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Read, Write},
    net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    num::NonZeroUsize,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
    sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}},
};

/// Values originate from a host-controlled engine manifest and configuration;
/// none may be taken from model-generated text or tool arguments.
pub struct PersistentEngineConfig {
    pub executable: PathBuf,
    pub expected_sha256: [u8; 32],
    pub threads: NonZeroUsize,
    pub context_tokens: u32,
    pub startup_timeout: Duration,
    pub request_timeout: Duration,
    pub api_key: String,
    /// Host observer receives only known stage and operating-system PID.
    pub load_observer: Option<Arc<dyn Fn(&'static str, Option<u32>) + Send + Sync>>,
}

/// Exact host-owned decoding parameters. No generic model-supplied CLI flags.
#[derive(Debug, Clone, Copy)]
pub struct SamplingProfile {
    pub temperature: f32,
    pub top_p: f32,
    pub top_k: u32,
    pub min_p: f32,
}
impl Default for SamplingProfile {
    fn default() -> Self {
        Self { temperature: 0.0, top_p: 0.95, top_k: 20, min_p: 0.0 }
    }
}
impl SamplingProfile {
    pub fn validate(self) -> Result<Self, String> {
        if !self.temperature.is_finite() || !(0.0..=2.0).contains(&self.temperature)
            || !self.top_p.is_finite() || !(0.01..=1.0).contains(&self.top_p)
            || !(1..=100).contains(&self.top_k)
            || !self.min_p.is_finite() || !(0.0..=1.0).contains(&self.min_p)
        {
            return Err("invalid host-owned llama.cpp sampling settings".into());
        }
        Ok(self)
    }
    fn apply(self, request: &mut serde_json::Value) {
        request["temperature"] = serde_json::json!(self.temperature);
        request["top_p"] = serde_json::json!(self.top_p);
        request["top_k"] = serde_json::json!(self.top_k);
        request["min_p"] = serde_json::json!(self.min_p);
    }
}
pub struct PersistentLlamaBackend {
    config: PersistentEngineConfig,
    sampling: SamplingProfile,
    canonical_executable: PathBuf,
    pinned_model_file: Option<File>,
    pinned_projector_file: Option<File>,
    // The cancellation watchdog must retain custody of this exact child
    // even while the generation thread holds the host inference mutex.
    process: Option<Arc<Mutex<Child>>>,
    port: Option<u16>,
    cache_gate: PromptCacheGate,
    next_trusted_cache_scope: Option<String>,
    last_image_delivery:Option<ImageDeliveryReceipt>,
    durable_spawn_gate: Option<HostEngineSpawnGate>,
    #[cfg(windows)]
    job: Option<lu_win_worker_job::Job>,
}

/// Only the trusted host may enroll a durable, profile-local attempt gate.
/// This is NOT a model tool or a permission to restart after cancellation.
struct HostEngineSpawnGate{
    ledger:Arc<BackendRestartLedger>,
    host_key:String,
}
impl HostEngineSpawnGate{
    fn check_before_spawn(&self)->Result<(),String>{
        match self.ledger.admit_now(&self.host_key){
            Ok(RestartAdmission::Allowed)=>Ok(()),
            Ok(RestartAdmission::CircuitOpen)=>
                Err("llama.cpp start circuit open: repeated attempts within five minutes; human review or cooldown required".into()),
            Err(_)=>Err("llama.cpp durable start receipt unavailable; worker not launched".into()),
        }
    }
}

fn verify_engine(path: &Path, expected: &[u8; 32]) -> Result<PathBuf, String> {
    if expected.iter().all(|byte| *byte == 0) {
        return Err("engine must have a trusted nonzero SHA-256 digest".into());
    }
    if !fs::symlink_metadata(path)
        .map_err(|e| e.to_string())?
        .file_type()
        .is_file()
    {
        return Err("engine is not an ordinary file".into());
    }
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::PermissionsExt;
        if fs::metadata(path)
            .map_err(|e| e.to_string())?
            .permissions()
            .mode()
            & 0o111
            == 0
        {
            return Err("bundled Linux engine is not executable".into());
        }
    }
    let canonical = path.canonicalize().map_err(|e| e.to_string())?;
    let mut file = File::open(&canonical).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    if hasher.finalize().as_slice() != expected {
        return Err("engine SHA-256 mismatch".into());
    }
    Ok(canonical)
}

/// Sufficient entropy must be generated by the trusted host. No API key is
/// written into repo config. For now the host must supply 64 hex characters.
fn valid_api_key(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Eliminate server flags from the operator environment. On Linux, untrusted
/// dynamic-loader knobs must also be cleared before loading verified .so files.
fn inherited_engine_env_is_forbidden(key: &str) -> bool {
    key.starts_with("LLAMA_ARG_")
        || key == "LLAMA_API_KEY"
        || (cfg!(target_os = "linux") && (key.starts_with("LD_") || key == "GLIBC_TUNABLES"))
}

impl PersistentLlamaBackend {
    /// Use the already running, trusted localhost llama.cpp engine. Only
    /// the exact host-advertised read-only browser function can be generated.
    /// Tool proposals are not interpreted as commands until native dispatch.
    pub fn generate_browser_function_calls(
        &mut self,
        request:&GenerationRequest,
        admitted_read_schemas:&[serde_json::Value],
        cancel:&Cancellation,
    )->Result<Vec<crate::browser_tool_stream::CompletedToolCall>,String>{
        // Tool schemas are per-turn grants; no chat KV can cross them.
        self.cache_gate.invalidate();
        self.next_trusted_cache_scope=None;
        if cancel.is_cancelled(){return Err("browser tool generation cancelled".into());}
        let port=self.active_port()?;
        let body=crate::browser_model_request::build_read_only_browser_tool_request(
            &request.prompt,&request.history,&[],request.max_new_tokens,
            admitted_read_schemas,
        ).map_err(|_|"unapproved llama.cpp browser tool schema")?;
        let mut json:serde_json::Value=serde_json::from_str(&body)
            .map_err(|_|"invalid scoped browser JSON")?;
        self.cache_gate.disable_for_other_body(&mut json)
            .map_err(str::to_owned)?;
        self.sampling.apply(&mut json);
        let body=json.to_string();
        let child = self.process.as_ref().cloned()
            .ok_or("llama.cpp worker is not loaded")?;
        let result = Self::run_with_watchdog(child, cancel, || {
            crate::multimodal_http::generate_browser_tool_stream_body(
                port, &self.config.api_key, &body, self.config.request_timeout,
                || cancel.is_cancelled(),
                std::collections::BTreeSet::from(["lu_browser_read".into()]),
            )
        });
        self.ensure_stopped_after_stream(port, cancel, result)
    }
    pub fn new(config: PersistentEngineConfig) -> Result<Self, String> {
        if !valid_api_key(&config.api_key) {
            return Err("engine requires a host-generated 256-bit hexadecimal API key".into());
        }
        if config.context_tokens < 256
            || config.context_tokens > 1_048_576
            || config.threads.get() > 256
            || config.startup_timeout.is_zero()
            || config.startup_timeout > Duration::from_secs(600)
            || config.request_timeout.is_zero()
            || config.request_timeout > Duration::from_secs(3600)
        {
            return Err("invalid persistent inference resource limits".into());
        }
        let canonical_executable = verify_engine(&config.executable, &config.expected_sha256)?;
        Ok(Self {
            config,
            sampling: SamplingProfile::default(),
            canonical_executable,
            pinned_model_file: None,
            pinned_projector_file: None,
            process: None,
            port: None,
            cache_gate: PromptCacheGate::default(),
            next_trusted_cache_scope: None,
            last_image_delivery:None,
            durable_spawn_gate: None,
            #[cfg(windows)]
            job: None,
        })
    }
    /// Bind a trusted application-profile SQLite circuit before any future
    /// llama.cpp child spawn. The host key is derived only from its reviewed
    /// model/engine catalog. Failed spawns count; the ledger survives restart.
    pub fn with_durable_spawn_gate(
        mut self, ledger:Arc<BackendRestartLedger>,host_key:String,
    )->Result<Self,String>{
        BackendRestartLedger::validate(&host_key)
            .map_err(|_|"Invalid trusted llama.cpp restart admission key")?;
        self.durable_spawn_gate=Some(HostEngineSpawnGate{ledger,host_key});
        Ok(self)
    }
    /// Called only by the host at load admission. Already loaded engine
    /// settings cannot silently change during a running conversation.
    pub fn with_sampling_profile(mut self, profile: SamplingProfile) -> Result<Self, String> {
        self.sampling = profile.validate()?;
        Ok(self)
    }
    fn observe(&self, phase: &'static str, pid: Option<u32>) {
        if let Some(observer) = self.config.load_observer.as_ref() {
            observer(phase, pid);
        }
    }
    pub fn pid(&self) -> Option<u32> {
        self.process.as_ref().map(|child| {
            child.lock().unwrap_or_else(|poison| poison.into_inner()).id()
        })
    }
    pub fn is_running(&mut self) -> bool {
        self.process.as_ref().is_some_and(|child| {
            let mut owned = child.lock().unwrap_or_else(|poison| poison.into_inner());
            matches!(owned.try_wait(), Ok(None))
        })
    }
    fn active_port(&mut self) -> Result<u16, String> {
        if !self.is_running() {
            self.cache_gate.invalidate();
            self.next_trusted_cache_scope=None;
            return Err("persistent llama.cpp engine is not running".into());
        }
        self.port
            .ok_or("persistent inference endpoint missing".into())
    }
    /// Independent of the HTTP reader and Lu's generation mutex. Cancellation
    /// must still terminate the exact owned child if the HTTP decoder wedges.
    fn run_with_watchdog<T>(
        child: Arc<Mutex<Child>>,
        cancellation: &Cancellation,
        request: impl FnOnce() -> Result<T, String>,
    ) -> Result<T, String> {
        let completed = Arc::new(AtomicBool::new(false));
        let observed = Arc::clone(&completed);
        let cancellation = cancellation.clone();
        let watchdog = thread::spawn(move || {
            while !observed.load(Ordering::Acquire) {
                if cancellation.is_cancelled() {
                    let deadline = Instant::now() + Duration::from_millis(750);
                    while Instant::now() < deadline {
                        if observed.load(Ordering::Acquire) {
                            return;
                        }
                        thread::sleep(Duration::from_millis(25));
                    }
                    if !observed.load(Ordering::Acquire) {
                        let mut owned = child.lock()
                            .unwrap_or_else(|poison| poison.into_inner());
                        if matches!(owned.try_wait(), Ok(None)) {
                            let _ = owned.kill();
                        }
                    }
                    return;
                }
                thread::sleep(Duration::from_millis(25));
            }
        });
        let result = request();
        completed.store(true, Ordering::Release);
        let _ = watchdog.join();
        result
    }

    /// A completed SSE response is not proof that upstream llama.cpp stopped.
    /// Require the authenticated single --parallel 1 slot to be actually idle.
    fn slots_idle(port: u16, api_key: &str) -> bool {
        let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
        let Ok(mut socket) = TcpStream::connect_timeout(&address, Duration::from_millis(300))
            else { return false; };
        if socket.set_read_timeout(Some(Duration::from_millis(300))).is_err()
            || socket.set_write_timeout(Some(Duration::from_millis(300))).is_err() {
            return false;
        }
        let request = format!(
            "GET /slots HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {api_key}\r\nConnection: close\r\n\r\n"
        );
        if socket.write_all(request.as_bytes()).is_err() {
            return false;
        }
        let mut data = Vec::new();
        if socket.take(64 * 1024 + 1).read_to_end(&mut data).is_err()
            || data.len() > 64 * 1024 {
            return false;
        }
        let Some(split) = data.windows(4).position(|w| w == b"\r\n\r\n")
            else { return false; };
        let header = &data[..split];
        if !(header.starts_with(b"HTTP/1.1 200 ") ||
             header.starts_with(b"HTTP/1.0 200 ")) {
            return false;
        }
        if std::str::from_utf8(header).ok().is_some_and(|text|
            text.to_ascii_lowercase().contains("transfer-encoding: chunked")) {
            return false;
        }
        let Ok(slots) = serde_json::from_slice::<serde_json::Value>(&data[split + 4..])
            else { return false; };
        let Some(slots) = slots.as_array() else { return false; };
        slots.len() == 1 && slots[0].get("is_processing")
            .and_then(serde_json::Value::as_bool) == Some(false)
    }

    /// Never report Ready or retain a cache parent until the actual llama.cpp
    /// slot is idle; a cancelled, failed, or uncertain worker is ejected.
    fn ensure_stopped_after_stream<T>(
        &mut self,
        port: u16,
        cancellation: &Cancellation,
        result: Result<T, String>,
    ) -> Result<T, String> {
        let value = match result {
            Ok(value) if !cancellation.is_cancelled() => value,
            Ok(_) => {
                self.unload();
                return Err("generation cancelled before verified stop".into());
            }
            Err(error) => {
                self.unload();
                return Err(error);
            }
        };
        for _ in 0..20 {
            if cancellation.is_cancelled() {
                self.unload();
                return Err("generation cancelled before verified stop".into());
            }
            if Self::slots_idle(port, &self.config.api_key) {
                return Ok(value);
            }
            thread::sleep(Duration::from_millis(50));
        }
        self.unload();
        Err("llama.cpp stream ended without verified idle slot; worker ejected".into())
    }

    fn wait_ready(&mut self, deadline: Instant, port: u16) -> Result<(), String> {
        loop {
            if !self.is_running() {
                self.observe("engine_exited_before_ready", self.pid());
                return Err("persistent llama.cpp engine exited during model loading".into());
            }
            if Instant::now() >= deadline {
                self.observe("engine_health_timeout", self.pid());
                return Err("persistent GGUF model load timed out".into());
            }
            let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
            if let Ok(mut socket) = TcpStream::connect_timeout(&address, Duration::from_millis(150))
            {
                let _ = socket.set_read_timeout(Some(Duration::from_millis(250)));
                let _ = socket.set_write_timeout(Some(Duration::from_millis(250)));
                let request = format!(
                    "GET /health HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {}\r\nConnection: close\r\n\r\n",
                    self.config.api_key
                );
                if socket.write_all(request.as_bytes()).is_ok() {
                    let mut reader = BufReader::new(socket);
                    let mut status = String::new();
                    if reader.read_line(&mut status).is_ok() && status.starts_with("HTTP/1.1 200 ")
                    {
                        self.observe("engine_health_200", self.pid());
                        return Ok(());
                    }
                }
            }
            thread::sleep(Duration::from_millis(100));
        }
    }
}
impl InferenceBackend for PersistentLlamaBackend {
    fn process_id(&self) -> Option<u32> {
        self.pid()
    }
    fn take_image_delivery_receipt(&mut self)->Option<ImageDeliveryReceipt>{
        self.last_image_delivery.take()
    }
    fn set_trusted_cache_scope(&mut self, scope: Option<&str>) {
        self.next_trusted_cache_scope=scope.map(str::to_owned);
    }
    fn invalidate_prefix_cache(&mut self) {
        self.cache_gate.invalidate();
        self.next_trusted_cache_scope=None;
    }
    fn generate_browser_function_calls(
        &mut self,
        request:&GenerationRequest,
        admitted_read_schemas:&[serde_json::Value],
        cancel:&Cancellation,
    )->Result<Vec<crate::browser_tool_stream::CompletedToolCall>,String>{
        PersistentLlamaBackend::generate_browser_function_calls(
            self,request,admitted_read_schemas,cancel
        )
    }
    fn load(&mut self, admitted: &AdmittedModel) -> Result<(), String> {
        self.load_with_projector(admitted, None)
    }
    fn load_with_projector(
        &mut self,
        admitted: &AdmittedModel,
        projector: Option<&AdmittedProjector>,
    ) -> Result<(), String> {
        if self.process.is_some() {
            return Err("persistent engine already owns a model".into());
        }
        // Every model/projector/engine identity is confined to one child.
        // New weights or MMProj must never inherit a prior process' KV.
        self.cache_gate.invalidate();
        self.next_trusted_cache_scope=None;
        admitted.recheck().map_err(|e| format!("{e:?}"))?;
        // Windows share-read mode rejects normal replacement of the model file.
        // A production loader must additionally pin the complete engine package
        // and link its process to a Windows kill-on-close Job Object.
        let mut open = OpenOptions::new();
        open.read(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            open.share_mode(0x00000001); // FILE_SHARE_READ
        }
        let file = open.open(admitted.path()).map_err(|e| e.to_string())?;
        if file.metadata().map_err(|e| e.to_string())?.len() != admitted.header().file_bytes {
            return Err("GGUF size changed at engine start".into());
        }
        admitted.recheck().map_err(|e| format!("{e:?}"))?;
        let projector_file = if let Some(projector) = projector {
            projector.recheck().map_err(|e| format!("{e:?}"))?;
            let f = open
                .open(projector.path())
                .map_err(|_| "projector file is inaccessible")?;
            if f.metadata()
                .map_err(|_| "projector metadata unavailable")?
                .len()
                != projector.header().file_bytes
            {
                return Err("projector file changed".into());
            }
            projector
                .recheck()
                .map_err(|_| "projector admission changed")?;
            Some(f)
        } else {
            None
        };
        verify_engine(&self.canonical_executable, &self.config.expected_sha256)?;

        // Host picks an ephemeral loopback port, never accepting an endpoint
        // supplied by the model. A bind race is handled by failing closed if
        // llama-server does not start; it never falls back to a foreign server.
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).map_err(|e| e.to_string())?;
        let port = listener.local_addr().map_err(|e| e.to_string())?.port();
        drop(listener);

        let mut command = Command::new(&self.canonical_executable);
        command
            .arg("--model")
            .arg(admitted.path())
            .arg("--host")
            .arg("127.0.0.1")
            .arg("--port")
            .arg(port.to_string())
            .arg("--api-key")
            .arg(&self.config.api_key)
            .arg("--threads")
            .arg(self.config.threads.get().to_string())
            .arg("--ctx-size")
            .arg(self.config.context_tokens.to_string())
            .arg("--parallel")
            .arg("1")
            .arg("--n-gpu-layers")
            .arg(if cfg!(target_os = "windows") {
                "99"
            } else {
                "0"
            })
            .arg("--no-warmup")
            .current_dir(
                self.canonical_executable
                    .parent()
                    .ok_or("invalid engine location")?,
            )
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if let Some(projector) = projector {
            command.arg("--mmproj").arg(projector.path());
        }
        for (name, _) in std::env::vars_os() {
            let key = name.to_string_lossy();
            if inherited_engine_env_is_forbidden(&key) {
                command.env_remove(name);
            }
        }
        #[cfg(target_os = "linux")]
        {
            // No inherited loader injection and no reliance on the operator's
            // shell environment. Every bundled .so is hash-checked by Lu's
            // trusted package verifier before a model can be loaded.
            command.env(
                "LD_LIBRARY_PATH",
                self.canonical_executable
                    .parent()
                    .ok_or("invalid Linux engine location")?,
            );
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // CREATE_NO_WINDOW only. The std::process::Child interface does
            // not expose the suspended primary thread handle needed to resume.
            // Job assignment immediately follows spawn, before health probing.
            // This has a short assignment race: lifetime supervision, NOT a
            // security sandbox or a claim of race-free containment.
            command.creation_flags(0x0800_0000);
        }
        // Establish the lifetime guard before launching the engine. If the
        // process cannot enter the Job, kill and reap it rather than running an
        // uncontained worker. Assignment precedes any health or model checks.
        #[cfg(windows)]
        let job = lu_win_worker_job::Job::new()
            .map_err(|e| format!("cannot establish Lu engine job: {e}"))?;
        // Journal the exact source-pinned worker start BEFORE spawning it.
        // A process-crash loop cannot erase this SQLite receipt on restart.
        if let Some(gate)=self.durable_spawn_gate.as_ref(){
            gate.check_before_spawn()?;
        }
        #[allow(unused_mut)]
        let mut child = command.spawn().map_err(|e| e.to_string())?;
        #[cfg(windows)]
        if let Err(error) = job.assign(&child) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("cannot contain Lu engine child: {error}"));
        }
        #[cfg(windows)]
        {
            self.job = Some(job);
        }
        self.process = Some(Arc::new(Mutex::new(child)));
        self.observe("engine_spawned", self.pid());
        self.pinned_model_file = Some(file);
        self.pinned_projector_file = projector_file;
        self.port = Some(port);
        let deadline = Instant::now() + self.config.startup_timeout;
        if let Err(error) = self.wait_ready(deadline, port) {
            self.unload();
            return Err(error);
        }
        // Neither healthy HTTP nor a loaded GGUF proves an idle decoder.
        if !Self::slots_idle(port, &self.config.api_key) {
            self.unload();
            return Err("llama.cpp model load lacks verified idle-slot support".into());
        }
        Ok(())
    }
    fn unload(&mut self) {
        self.cache_gate.invalidate();
        self.next_trusted_cache_scope=None;
        if let Some(child) = self.process.take() {
            let mut owned = child.lock().unwrap_or_else(|poison| poison.into_inner());
            let _ = owned.kill();
            let _ = owned.wait();
        }
        self.port = None;
        self.pinned_model_file = None;
        self.pinned_projector_file = None;
        #[cfg(windows)]
        {
            // Closing the final job handle also cleans up descendants that
            // outlived the directly supervised llama-server process.
            self.job = None;
        }
    }
    fn generate_with_images(
        &mut self,
        request: &GenerationRequest,
        images: &[PinnedImage],
        cancel: &Cancellation,
        emit: &mut dyn FnMut(TokenChunk) -> Result<(), EngineError>,
    ) -> Result<(), String> {
        self.cache_gate.invalidate();
        self.next_trusted_cache_scope=None;
        let mut receipt=ImageDeliveryReceipt::new();
        let result=(||->Result<(),String>{
            if cancel.is_cancelled(){return Err("generation cancelled".into())}
            if self.pinned_projector_file.is_none(){
                return Err("load a compatible MMProj before image generation".into());
            }
            let port=match self.active_port(){
                Ok(port)=>port,
                Err(error)=>{self.unload();return Err(error)}
            };
            let body=build_structured_chat_request(&request.prompt,&request.history,images,
                request.max_new_tokens).map_err(|_|"invalid structured image request".to_owned())?;
            let mut json:serde_json::Value=serde_json::from_str(&body)
                .map_err(|_|"invalid structured image JSON")?;
            self.cache_gate.disable_for_other_body(&mut json).map_err(str::to_owned)?;
            self.sampling.apply(&mut json);
            let body=json.to_string();
            let mut piece=0u32;
            let child=self.process.as_ref().cloned().ok_or("llama.cpp worker is not loaded")?;
            let result=Self::run_with_watchdog(child,cancel,||{
                generate_verified_image_body(port,&self.config.api_key,&body,&request.prompt,
                    &request.history,images,request.max_new_tokens,self.config.request_timeout,
                    ||cancel.is_cancelled(),|text|{
                        emit(TokenChunk{text,index:piece})
                            .map_err(|_|"chat stream receiver unavailable".to_owned())?;
                        piece=piece.saturating_add(1);Ok(())
                    },&mut receipt)
            });
            let verified=self.ensure_stopped_after_stream(port,cancel,result);
            receipt.worker_idle_verified=verified.is_ok();
            verified
        })();
        if let Err(error)=&result{
            receipt.failure_code=Some(image_failure_code(error));
        }
        let code=receipt.failure_code;
        self.last_image_delivery=Some(receipt);
        result.map_err(|_|code.unwrap_or("CP01_IMAGE_BACKEND_FAILED").to_owned())
    }
    fn generate(
        &mut self,
        request: &GenerationRequest,
        cancel: &Cancellation,
        emit: &mut dyn FnMut(TokenChunk) -> Result<(), EngineError>,
    ) -> Result<(), String> {
        if cancel.is_cancelled() {
            return Err("generation cancelled".into());
        }
        let port = self.active_port()?;
        let trusted_scope=self.next_trusted_cache_scope.take();
        let body = build_structured_chat_request(
            &request.prompt,
            &request.history,
            &[],
            request.max_new_tokens,
        )
        .map_err(|_| "invalid structured text request".to_string())?;
        let mut json:serde_json::Value=serde_json::from_str(&body)
            .map_err(|_|"invalid structured text JSON")?;
        let attempt=self.cache_gate.begin_text(trusted_scope.as_deref(),&mut json)
            .map_err(str::to_owned)?;
        self.sampling.apply(&mut json);
        let body=json.to_string();
        let mut piece = 0u32;
        let child = self.process.as_ref().cloned()
            .ok_or("llama.cpp worker is not loaded")?;
        let result = Self::run_with_watchdog(child, cancel, || {
            generate_chat_stream_body(
                port,
                &self.config.api_key,
                &body,
                self.config.request_timeout,
                || cancel.is_cancelled(),
                |text| {
                    emit(TokenChunk { text, index: piece })
                        .map_err(|_| "chat stream receiver unavailable".to_string())?;
                    piece = piece.saturating_add(1);
                    Ok(())
                },
            )
        });
        let verified = self.ensure_stopped_after_stream(port, cancel, result);
        self.cache_gate.complete(attempt, verified.is_ok() && !cancel.is_cancelled());
        verified
    }
}
impl Drop for PersistentLlamaBackend {
    fn drop(&mut self) {
        self.unload();
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_durable_start_gate_blocks_third_pre_spawn_attempt_after_reopen(){
        let stamp=std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path=std::env::temp_dir().join(format!(
            "lu-llama-gate-{}-{stamp}.sqlite",std::process::id()
        ));
        let key="llama.reviewed-model".to_owned();
        {
            let ledger=Arc::new(BackendRestartLedger::open(&path).unwrap());
            let gate=HostEngineSpawnGate{ledger,host_key:key.clone()};
            assert!(gate.check_before_spawn().is_ok());
            assert!(gate.check_before_spawn().is_ok());
            assert!(gate.check_before_spawn().is_err());
        }
        {
            let ledger=Arc::new(BackendRestartLedger::open(&path).unwrap());
            let gate=HostEngineSpawnGate{ledger,host_key:key};
            assert!(gate.check_before_spawn().is_err(),
                "a restarted Lu process must not get a fresh quota");
        }
        let _=std::fs::remove_file(&path);
        let _=std::fs::remove_file(path.with_extension("sqlite-wal"));
        let _=std::fs::remove_file(path.with_extension("sqlite-shm"));
    }
    #[test]
    fn original_watchdog_kills_exact_owned_child_even_when_decoder_blocks() {
        #[cfg(windows)]
        let mut command = {
            let mut c = Command::new("cmd");
            c.args(["/C", "ping -n 30 127.0.0.1 > NUL"]);
            c
        };
        #[cfg(not(windows))]
        let mut command = {
            let mut c = Command::new("sleep");
            c.arg("30");
            c
        };
        let child = Arc::new(Mutex::new(
            command.stdin(Stdio::null()).stdout(Stdio::null())
                .stderr(Stdio::null()).spawn().unwrap()
        ));
        let cancellation = Cancellation::default();
        let for_cancel = cancellation.clone();
        let stopper = thread::spawn(move || {
            thread::sleep(Duration::from_millis(30));
            for_cancel.cancel();
        });
        let owner = Arc::clone(&child);
        let start = Instant::now();
        let outcome = PersistentLlamaBackend::run_with_watchdog(
            Arc::clone(&child), &cancellation, move || {
                while start.elapsed() < Duration::from_secs(8) {
                    if owner.lock().unwrap().try_wait().unwrap().is_some() {
                        return Ok(());
                    }
                    thread::sleep(Duration::from_millis(25));
                }
                Err("watchdog could not terminate original child".into())
            },
        );
        stopper.join().unwrap();
        assert!(outcome.is_ok(), "{outcome:?}");
        let mut owner = child.lock().unwrap();
        assert!(owner.try_wait().unwrap().is_some());
        let _ = owner.wait();
    }

    #[test]
    fn original_slot_verification_rejects_busy_multiple_and_forged_idleness() {
        for (json, expected) in [
            (r#"[{"id":0,"is_processing":false}]"#, true),
            (r#"[{"id":0,"is_processing":true}]"#, false),
            (r#"[{"id":0,"is_processing":false},{"id":1,"is_processing":false}]"#, false),
            (r#"[{"id":0,"is_processing":"false"}]"#, false),
        ] {
            let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
            let port = listener.local_addr().unwrap().port();
            let payload = json.to_string();
            let server = thread::spawn(move || {
                let (mut socket, _) = listener.accept().unwrap();
                socket.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
                let mut request = [0u8; 1024];
                let size = socket.read(&mut request).unwrap();
                assert!(request[..size].starts_with(b"GET /slots HTTP/1.1"));
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    payload.len(), payload
                );
                socket.write_all(response.as_bytes()).unwrap();
            });
            assert_eq!(PersistentLlamaBackend::slots_idle(port, &"ab".repeat(32)), expected);
            server.join().unwrap();
        }
    }

    #[test]
    fn host_sampling_profile_preserves_text_and_image_template_guards(){
        let mut body=serde_json::json!({
            "messages":[{"role":"user","content":"hello"}],
            "temperature":0.0,
            "chat_template_kwargs":{"enable_thinking":false}
        });
        let sample=SamplingProfile{
            temperature:0.6,top_p:0.9,top_k:30,min_p:0.04
        }.validate().unwrap();
        sample.apply(&mut body);
        assert_eq!(body["temperature"],serde_json::json!(0.6_f32));
        assert_eq!(body["top_k"],30);
        assert_eq!(body["chat_template_kwargs"]["enable_thinking"],false);
        assert!(SamplingProfile{temperature:f32::NAN,..sample}.validate().is_err());
        assert!(SamplingProfile{top_p:0.0,..sample}.validate().is_err());
    }
    #[test]
    fn api_key_requires_full_host_entropy() {
        assert!(!valid_api_key("1234"));
        assert!(valid_api_key(&"0a".repeat(32)));
    }
    #[test]
    fn strips_untrusted_engine_environment_without_erasing_other_settings() {
        assert!(inherited_engine_env_is_forbidden("LLAMA_API_KEY"));
        assert!(inherited_engine_env_is_forbidden("LLAMA_ARG_THREADS"));
        assert!(!inherited_engine_env_is_forbidden("HOME"));
        assert!(!inherited_engine_env_is_forbidden("PATH"));
        #[cfg(target_os = "linux")]
        {
            assert!(inherited_engine_env_is_forbidden("LD_PRELOAD"));
            assert!(inherited_engine_env_is_forbidden("LD_AUDIT"));
            assert!(inherited_engine_env_is_forbidden("LD_LIBRARY_PATH"));
            assert!(inherited_engine_env_is_forbidden("LD_DEBUG_OUTPUT"));
            assert!(inherited_engine_env_is_forbidden("GLIBC_TUNABLES"));
        }
    }

    #[test]
    fn image_generation_requires_loaded_mmproj_before_http() {
        let executable = std::env::current_exe().unwrap();
        let digest: [u8; 32] = Sha256::digest(std::fs::read(&executable).unwrap()).into();
        let config = PersistentEngineConfig {
            executable,
            expected_sha256: digest,
            threads: NonZeroUsize::new(2).unwrap(),
            context_tokens: 4096,
            startup_timeout: Duration::from_secs(5),
            request_timeout: Duration::from_secs(5),
            api_key: "ab".repeat(32),
            load_observer: None,
        };
        let mut backend = PersistentLlamaBackend::new(config).unwrap();
        let image = crate::multimodal_saved_turn::PinnedImage {
            image_ref: "img-demo".into(),
            mime_type: "image/png",
            sha256: [0x52; 32],
            bytes: vec![1, 2, 3],
        };
        let result = backend.generate_with_images(
            &GenerationRequest {
                history: Vec::new(),
                prompt: "What is shown?".into(),
                max_new_tokens: 100,
            },
            &[image],
            &Cancellation::default(),
            &mut |_| Ok(()),
        );
        assert_eq!(result.unwrap_err(),"CP01_IMAGE_PROJECTOR_MISSING");
        let receipt=backend.take_image_delivery_receipt().unwrap();
        assert_eq!(receipt.failure_code,Some("CP01_IMAGE_PROJECTOR_MISSING"));
        assert!(!receipt.payload_verified&&!receipt.upload_completed);
    }
    #[test]
    fn observer_receives_only_phase_and_pid() {
        use std::sync::Mutex;
        let executable = std::env::current_exe().unwrap();
        let digest: [u8; 32] = Sha256::digest(std::fs::read(&executable).unwrap()).into();
        let captured = Arc::new(Mutex::new(Vec::new()));
        let destination = Arc::clone(&captured);
        let observer: Arc<dyn Fn(&'static str, Option<u32>) + Send + Sync> =
            Arc::new(move |phase, pid| destination.lock().unwrap().push((phase, pid)));
        let backend = PersistentLlamaBackend::new(PersistentEngineConfig {
            executable,
            expected_sha256: digest,
            threads: NonZeroUsize::new(2).unwrap(),
            context_tokens: 4096,
            startup_timeout: Duration::from_secs(5),
            request_timeout: Duration::from_secs(5),
            api_key: "ab".repeat(32),
            load_observer: Some(observer),
        }).unwrap();
        backend.observe("engine_spawned", Some(1234));
        backend.observe("engine_health_200", Some(1234));
        assert_eq!(&*captured.lock().unwrap(), &[
            ("engine_spawned", Some(1234)),
            ("engine_health_200", Some(1234)),
        ]);
    }

    #[test]
    fn refuses_unsigned_engine_identity() {
        assert!(verify_engine(Path::new("missing"), &[0u8; 32]).is_err());
    }
}
