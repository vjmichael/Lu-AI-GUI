//! WO6: bounded Debug Adapter Protocol (DAP) framing and response correlation.
//!
//! This is NOT an adapter launcher. An untrusted debugger has program-execution
//! power and must be pinned, approved, confined and supervised by the native
//! OS worker *before* this protocol processor is attached to its stdio.
//! The default desktop has no DAP executable, launch/evaluate/terminal command,
//! or WebView-to-process bridge. All source and process selection belongs to
//! the trusted host and one-use D3 authorization, not to these messages.
use std::collections::BTreeMap;

use crate::dap_source_lines::{candidate_from_dap_frame, SourceLineCandidate};

use serde_json::{json, Value};

const MAX_FRAME: usize = 64 * 1024;
const MAX_HEADER: usize = 512;
const MAX_BUFFER: usize = MAX_FRAME + MAX_HEADER + 4;
const MAX_PENDING: usize = 8;
const MAX_SEQ: u64 = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DapError {
    InvalidFrame,
    InvalidMessage,
    ExcessiveData,
    UnexpectedResponse,
    AdapterRequestDenied,
    InvalidPhase,
    AdapterFailed,
}
impl DapError {
    pub fn code(self) -> &'static str {
        match self {
            Self::InvalidFrame => "DAP_FRAME_DENIED",
            Self::InvalidMessage => "DAP_MESSAGE_DENIED",
            Self::ExcessiveData => "DAP_RESOURCE_LIMIT",
            Self::UnexpectedResponse => "DAP_UNEXPECTED_RESPONSE",
            Self::AdapterRequestDenied => "DAP_REVERSE_REQUEST_DENIED",
            Self::InvalidPhase => "DAP_PHASE_DENIED",
            Self::AdapterFailed => "DAP_ADAPTER_FAILED",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DapPhase {
    Detached,
    Initializing,
    AwaitingInitializedEvent,
    Ready,
    Launching,
    Stopped,
    Closing,
    Terminated,
    Quarantined,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DapEvent {
    Initialized,
    // Adapter may announce configuration only AFTER the fixed launch request.
    ConfigurationRequested,
    LaunchAcknowledged,
    Stopped { thread_id: u64 },
    Continued,
    Terminated,
    ThreadsReceived,
    StackTraceReceived,
    Disconnected,
}

/// Only available to the explicitly gated disposable native D6C test.
/// These are DAP *protocol* coordinates and are NOT executable authority.
#[cfg(feature = "dev046c-ci-control")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CiStepKind { Over, Into, Out }

#[cfg(feature = "dev046c-ci-control")]
#[derive(Debug, Clone)]
struct CiControlPending {
    name: &'static str,
    thread_id: u64,
    expected_line: Option<u32>,
    response_ack: bool,
    new_stop: bool,
}
#[cfg(feature = "dev046c-ci-control")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CiControlObservation {
    pub response_acknowledged: bool,
    pub new_stop_observed: bool,
    pub thread_id: u64,
    pub is_step: bool,
}

/// This object holds NO authorization. It only checks frames from a debugger
/// already launched by an independently authenticated native supervisor.
pub struct DapSession {
    phase: DapPhase,
    input: Vec<u8>,
    seq: u64,
    pending: BTreeMap<u64, &'static str>,
    initialized_event: bool,
    launch_sent: bool,
    configuration_done_sent: bool,
    // Ephemeral, untrusted adapter coordinates; NEVER a symbol-line proof.
    last_source_candidate: Option<SourceLineCandidate>,
    #[cfg(feature = "dev046c-ci-control")]
    ci_control: Option<CiControlPending>,
    #[cfg(feature = "dev046c-ci-control")]
    last_stopped_thread: Option<u64>,
}
impl Default for DapSession {
    fn default() -> Self {
        Self::new()
    }
}
impl DapSession {
    pub fn new() -> Self {
        Self {
            phase: DapPhase::Detached,
            input: Vec::new(),
            seq: 1,
            pending: BTreeMap::new(),
            initialized_event: false,
            launch_sent: false,
            configuration_done_sent: false,
            last_source_candidate: None,
            #[cfg(feature = "dev046c-ci-control")]
            ci_control: None,
            #[cfg(feature = "dev046c-ci-control")]
            last_stopped_thread: None,
        }
    }
    pub fn phase(&self) -> DapPhase {
        self.phase
    }
    /// Native-only, read-only review input from a CORRELATED stackTrace.
    /// Never offer this to WebView or use it as executable debugger authority.
    pub fn source_candidate(&self) -> Option<&SourceLineCandidate> {
        self.last_source_candidate.as_ref()
    }
    fn fail(&mut self, why: DapError) -> DapError {
        self.phase = DapPhase::Quarantined;
        self.input.clear();
        self.pending.clear();
        self.last_source_candidate = None;
        #[cfg(feature = "dev046c-ci-control")]
        {
            self.ci_control = None;
            self.last_stopped_thread = None;
        }
        why
    }
    fn request(&mut self, name: &'static str, args: Value) -> Result<Vec<u8>, DapError> {
        if self.pending.len() >= MAX_PENDING || self.seq > MAX_SEQ {
            return Err(self.fail(DapError::ExcessiveData));
        }
        let seq = self.seq;
        self.seq += 1;
        let msg = json!({
            "seq": seq, "type": "request", "command": name, "arguments": args
        });
        let body = serde_json::to_vec(&msg).map_err(|_| self.fail(DapError::InvalidMessage))?;
        if body.len() > MAX_FRAME {
            return Err(self.fail(DapError::ExcessiveData));
        }
        self.pending.insert(seq, name);
        let mut wire = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
        wire.extend(body);
        Ok(wire)
    }
    /// Only a protocol handshake; NOT launch or attach authority.
    pub fn initialize(&mut self) -> Result<Vec<u8>, DapError> {
        if self.phase != DapPhase::Detached {
            return Err(DapError::InvalidPhase);
        }
        self.phase = DapPhase::Initializing;
        self.request(
            "initialize",
            json!({
                "adapterID": "lu-ide", "clientID": "lu-ide",
                "linesStartAt1": true, "columnsStartAt1": true,
                "pathFormat": "path", "supportsRunInTerminalRequest": false,
                "supportsVariableType": false
            }),
        )
    }
    /// ONLY the native OS worker may call this with the self-test target
    /// from its independently sealed installer bundle. This method never
    /// accepts an executable path, args or environment from the WebView.
    /// The debuggee inherits the adapter's restricted Job/cgroup.
    pub fn launch_fixed_fixture(&mut self, program: &str) -> Result<Vec<u8>, DapError> {
        if self.phase != DapPhase::Ready {
            return Err(DapError::InvalidPhase);
        }
        let valid = program == "/ls/debuggee/wo6-selftest"
            || (program.len() <= 1024
                && std::path::Path::new(program).is_absolute()
                && program.ends_with("\\debuggee\\wo6-selftest.exe"));
        if !valid || program.contains("..") || program.chars().any(char::is_control) {
            return Err(DapError::InvalidMessage);
        }
        let frame = self.request(
            "launch",
            json!({
                "program": program, "stopOnEntry": true, "noDebug": false,
                // LLVM LLDB-DAP defaults disableASLR=true. In a restricted
                // Bwrap/AppArmor namespace that tries forbidden personality(2)
                // ASLR disabling. Leave kernel ASLR ON; do not widen privileges.
                "disableASLR": false,
                // Test-only, headless fixed fixture never needs terminal IO.
                // LLVM 18 otherwise allocates a pseudo-terminal, which can
                // be denied under the kernel Bwrap/AppArmor namespace.
                // Disabling target stdio reduces exposed I/O; no shell.
                "disableSTDIO": true,
                "args": [], "env": {}, "runInTerminal": false
            }),
        )?;
        self.launch_sent = true;
        self.phase = DapPhase::Launching;
        Ok(frame)
    }

    /// DEV-04.4: accept ONLY a native staged, OS-owned debug target.
    /// A caller cannot select any other path, process, arguments or env.
    /// This builds DAP bytes ONLY; actual authority resides in the D3 plan
    /// with an exact sealed build target digest and native OS containment.
    pub fn launch_verified_project(&mut self,program:&str)->Result<Vec<u8>,DapError>{
        if self.phase!=DapPhase::Ready{return Err(DapError::InvalidPhase);}
        let linux=program.strip_prefix("/debug-target/target/debug/")
            .is_some_and(|file|!file.is_empty() && file.len()<=200 &&
                !file.contains('.') && file.bytes().all(|b|
                b.is_ascii_alphanumeric()||matches!(b,b'-'|b'_')));
        let windows=if program.len()<=1024 {
            let normalized=program.replace('/',"\\");
            let pieces:Vec<_>=normalized.split('\\').collect();
            pieces.len()>4 && pieces.iter().any(|part|*part=="debug-target")
                && normalized.contains("\\debug-target\\target\\debug\\")
                && normalized.to_ascii_lowercase().ends_with(".exe")
                && pieces.last().is_some_and(|name|{
                    let base=name.strip_suffix(".exe").unwrap_or("");
                    !base.is_empty() && base.len()<=196 &&
                    base.bytes().all(|b|
                        b.is_ascii_alphanumeric()||matches!(b,b'-'|b'_'))
                })
                && std::path::Path::new(program).is_absolute()
        } else {false};
        if !(linux||windows)||program.contains("..")||
           program.chars().any(char::is_control) {
            return Err(DapError::InvalidMessage);
        }
        let frame=self.request("launch",json!({
            "program":program,"stopOnEntry":true,"noDebug":false,
            "disableASLR":false,"disableSTDIO":true,
            "args":[],"env":{},"runInTerminal":false
        }))?;
        self.launch_sent=true;
        self.phase=DapPhase::Launching;
        Ok(frame)
    }

    /// Read-only pending signal for adapters that emit initialized immediately
    /// after the response (even before the fixed launch is sent). The trusted
    /// native host may examine this after launch; no DAP instruction is sent.
    pub fn configuration_needed(&self) -> bool {
        self.launch_sent
            && self.initialized_event
            && !self.configuration_done_sent
            && matches!(
                self.phase,
                DapPhase::Launching | DapPhase::Ready | DapPhase::Stopped
            )
    }

    /// Complete ONLY the fixed debugger's empty configuration transaction.
    /// This cannot launch a target, run a terminal, evaluate code or select a
    /// path. It is legal only after a pinned launch and the adapter's event.
    pub fn configuration_done(&mut self) -> Result<Vec<u8>, DapError> {
        if !self.configuration_needed() {
            return Err(DapError::InvalidPhase);
        }
        let frame = self.request("configurationDone", json!({}))?;
        self.configuration_done_sent = true;
        Ok(frame)
    }

    /// Read-only inspection of a debugger whose target was separately approved.
    pub fn threads(&mut self) -> Result<Vec<u8>, DapError> {
        if !matches!(self.phase, DapPhase::Ready | DapPhase::Stopped) {
            return Err(DapError::InvalidPhase);
        }
        self.request("threads", json!({}))
    }
    pub fn stack_trace(&mut self, thread_id: u64) -> Result<Vec<u8>, DapError> {
        if self.phase != DapPhase::Stopped || thread_id == 0 || thread_id > MAX_SEQ {
            return Err(DapError::InvalidPhase);
        }
        // A new, independently correlated stack invalidates prior coordinates.
        self.last_source_candidate = None;
        self.request(
            "stackTrace",
            json!({"threadId":thread_id, "startFrame":0, "levels":16}),
        )
    }
    /// Build one fixed, previously native-authorized step DAP frame for a
    /// *sealed* debuggee. The host protocol never accepts raw user commands.
    /// A DAP ACK alone is not an independently verified result.
    #[cfg(feature = "dev046c-ci-control")]
    pub fn ci_step(&mut self, thread_id: u64, kind: CiStepKind) -> Result<Vec<u8>, DapError> {
        if self.phase != DapPhase::Stopped || thread_id == 0
            || self.last_stopped_thread != Some(thread_id)
            || self.last_source_candidate.is_none()
            || self.ci_control.is_some() || !self.pending.is_empty() {
            return Err(DapError::InvalidPhase);
        }
        let command = match kind {
            CiStepKind::Over => "next",
            CiStepKind::Into => "stepIn",
            CiStepKind::Out => "stepOut",
        };
        let wire = self.request(command, json!({
            "threadId": thread_id, "singleThread": true,
            "granularity": "line"
        }))?;
        self.ci_control = Some(CiControlPending {
            name: command, thread_id, expected_line: None,
            response_ack: false, new_stop: false,
        });
        // Stale original source coordinates must not be used after resume.
        self.last_source_candidate = None;
        self.last_stopped_thread = None;
        self.phase = DapPhase::Ready;
        Ok(wire)
    }
    /// A single already-reviewed compiler source location, with NO
    /// conditions, log messages, sourceReference, expressions or hit counts.
    /// Adapter "verified" is recorded as acknowledgement ONLY.
    #[cfg(feature = "dev046c-ci-control")]
    pub fn ci_set_breakpoint(
        &mut self, compiled_source: &str, line: u32,
    ) -> Result<Vec<u8>, DapError> {
        if self.phase != DapPhase::Stopped || line == 0 || line > 1_000_000
            || self.ci_control.is_some() || !self.pending.is_empty()
            || self.last_source_candidate.as_ref().is_none_or(|candidate|
                candidate.adapter_path != compiled_source || candidate.line != line)
            || compiled_source.len() > 1024 || compiled_source.contains("..")
            || compiled_source.chars().any(char::is_control) {
            return Err(DapError::InvalidPhase);
        }
        let wire = self.request("setBreakpoints", json!({
            "source": { "path": compiled_source },
            "breakpoints": [{"line": line}],
            "lines": [line],
            "sourceModified": false
        }))?;
        self.ci_control = Some(CiControlPending {
            name: "setBreakpoints", thread_id: self.last_stopped_thread.unwrap_or(0),
            expected_line: Some(line), response_ack: false, new_stop: false,
        });
        Ok(wire)
    }
    /// Host must join this adapter observation to an entirely independent
    /// kernel owner event and OS RIP/PE/PDB/source witness. This does NOT
    /// yield success on an adapter response alone.
    #[cfg(feature = "dev046c-ci-control")]
    pub fn ci_take_control_observation(&mut self) -> Option<CiControlObservation> {
        let pending = self.ci_control.as_ref()?;
        let is_step = pending.expected_line.is_none();
        if !pending.response_ack || (is_step && !pending.new_stop) {
            return None;
        }
        let pending = self.ci_control.take()?;
        Some(CiControlObservation {
            response_acknowledged: pending.response_ack,
            new_stop_observed: pending.new_stop,
            thread_id: pending.thread_id,
            is_step,
        })
    }
    /// Disconnect does not promise process-tree termination. The OS supervisor
    /// must kill and independently verify all descendants before journaling
    /// Completed/Cancelled, even if this protocol replies "success".
    pub fn disconnect(&mut self) -> Result<Vec<u8>, DapError> {
        if !matches!(
            self.phase,
            DapPhase::AwaitingInitializedEvent | DapPhase::Ready | DapPhase::Stopped
        ) {
            return Err(DapError::InvalidPhase);
        }
        self.phase = DapPhase::Closing;
        self.request(
            "disconnect",
            json!({"restart":false,"terminateDebuggee":false}),
        )
    }
    /// Never sends launch, attach, evaluate, setExpression, runInTerminal,
    /// breakpoint condition expressions, or any debugger-supplied instruction.
    pub fn receive(&mut self, bytes: &[u8]) -> Result<Vec<DapEvent>, DapError> {
        if matches!(self.phase, DapPhase::Quarantined | DapPhase::Terminated) {
            return Err(DapError::InvalidPhase);
        }
        if bytes.len() > MAX_BUFFER.saturating_sub(self.input.len()) {
            return Err(self.fail(DapError::ExcessiveData));
        }
        self.input.extend_from_slice(bytes);
        let mut events = Vec::new();
        loop {
            let marker = self.input.windows(4).position(|part| part == b"\r\n\r\n");
            let Some(header_end) = marker else {
                if self.input.len() > MAX_HEADER {
                    return Err(self.fail(DapError::ExcessiveData));
                }
                return Ok(events);
            };
            if header_end > MAX_HEADER {
                return Err(self.fail(DapError::ExcessiveData));
            }
            let len = match frame_length(&self.input[..header_end]) {
                Ok(n) => n,
                Err(error) => return Err(self.fail(error)),
            };
            let end = header_end + 4 + len;
            if self.input.len() < end {
                return Ok(events);
            }
            let msg: Value = match serde_json::from_slice(&self.input[header_end + 4..end]) {
                Ok(value) => value,
                Err(_) => return Err(self.fail(DapError::InvalidMessage)),
            };
            // Remove the consumed prefix before dispatch; a failure still
            // quarantines the entire remaining buffer.
            self.input.drain(..end);
            match self.accept(&msg) {
                Ok(Some(event)) => events.push(event),
                Ok(None) => {}
                Err(error) => return Err(self.fail(error)),
            };
        }
    }
    fn accept(&mut self, value: &Value) -> Result<Option<DapEvent>, DapError> {
        let obj = value.as_object().ok_or(DapError::InvalidMessage)?;
        let seq = obj
            .get("seq")
            .and_then(Value::as_u64)
            .ok_or(DapError::InvalidMessage)?;
        // DAP adapter-assigned message sequence values are transport
        // metadata and may start at zero. Only the separately tracked
        // positive client request_seq permits a response to be accepted.
        // Continue to enforce the bounded ceiling for hostile adapters.
        if seq > MAX_SEQ {
            return Err(DapError::InvalidMessage);
        }
        match obj.get("type").and_then(Value::as_str) {
            Some("request") => Err(DapError::AdapterRequestDenied),
            Some("response") => {
                let request_seq = obj
                    .get("request_seq")
                    .and_then(Value::as_u64)
                    .ok_or(DapError::InvalidMessage)?;
                let command = obj.get("command").and_then(Value::as_str);
                let expected = self
                    .pending
                    .remove(&request_seq)
                    .ok_or(DapError::UnexpectedResponse)?;
                if command != Some(expected) || obj.get("success") != Some(&Value::Bool(true)) {
                    // Debug-only, fixed categories of real adapter launch
                    // failure; never print the untrusted adapter message,
                    // target path or debugger-provided source text.
                    if cfg!(debug_assertions)
                        && std::env::var_os("LU_IDE_E2_NATIVE_TRACE").as_deref()
                            == Some(std::ffi::OsStr::new("1"))
                    {
                        // Fixed schema metadata only: distinguish a
                        // structurally unexpected response from LLDB's
                        // explicit launch refusal. Never log raw adapter
                        // paths, messages, child output or error numbers.
                        let mismatch = command != Some(expected);
                        eprintln!(
                            "PROTECTED_DAP_RESPONSE_CORRELATION={}",
                            if mismatch {
                                "wrong_command"
                            } else {
                                "exact_request_command"
                            }
                        );
                        eprintln!(
                            "PROTECTED_DAP_RESPONSE_RESULT={}",
                            if obj.get("success") == Some(&Value::Bool(false)) {
                                "adapter_reported_failure"
                            } else if obj.get("success") == Some(&Value::Bool(true)) {
                                "adapter_reported_success"
                            } else {
                                "missing_or_invalid_success_flag"
                            }
                        );
                        eprintln!(
                            "PROTECTED_DAP_ADAPTER_ERROR_SCHEMA={}",
                            if obj
                                .get("body")
                                .and_then(|body| body.get("error"))
                                .and_then(|error| error.get("format"))
                                .and_then(Value::as_str)
                                .is_some()
                            {
                                "structured_error_format_present"
                            } else {
                                "no_structured_error_format"
                            }
                        );
                        eprintln!(
                            "PROTECTED_DAP_ADAPTER_FAILURE_CLASS={}",
                            classify_native_adapter_error(obj)
                        );
                    }
                    return Err(DapError::AdapterFailed);
                }
                match expected {
                    // DAP explicitly permits launch after initialize RESPONSE.
                    // Blocking on the later initialized EVENT deadlocks LLDB-DAP
                    // because that event may be emitted only after launch.
                    "initialize" if self.phase == DapPhase::Initializing => {
                        self.phase = DapPhase::Ready;
                        Ok(Some(DapEvent::Initialized))
                    }
                    "launch" if matches!(self.phase, DapPhase::Launching | DapPhase::Stopped) => {
                        if self.phase == DapPhase::Launching {
                            self.phase = DapPhase::Ready;
                        }
                        Ok(Some(DapEvent::LaunchAcknowledged))
                    }
                    "configurationDone"
                        if self.configuration_done_sent
                            && matches!(
                                self.phase,
                                DapPhase::Launching | DapPhase::Ready | DapPhase::Stopped
                            ) =>
                    {
                        Ok(None)
                    }
                    "threads" if matches!(self.phase, DapPhase::Ready | DapPhase::Stopped) => {
                        let body = obj
                            .get("body")
                            .and_then(Value::as_object)
                            .ok_or(DapError::InvalidMessage)?;
                        let threads = body
                            .get("threads")
                            .and_then(Value::as_array)
                            .ok_or(DapError::InvalidMessage)?;
                        // A successful but EMPTY response is not evidence
                        // that the protected LLDB adapter observed a thread.
                        if threads.is_empty() {
                            return Err(DapError::InvalidMessage);
                        }
                        if threads.len() > 64
                            || threads
                                .iter()
                                .any(|t| t.get("id").and_then(Value::as_u64).is_none())
                        {
                            return Err(DapError::ExcessiveData);
                        }
                        Ok(Some(DapEvent::ThreadsReceived))
                    }
                    "stackTrace" if self.phase == DapPhase::Stopped => {
                        let frames = obj
                            .get("body")
                            .and_then(|body| body.get("stackFrames"))
                            .and_then(Value::as_array)
                            .ok_or(DapError::InvalidMessage)?;
                        // DEV-04.5B requires at least one actual stack
                        // frame; a zero-length DAP success is not a stop.
                        if frames.is_empty() {
                            return Err(DapError::InvalidMessage);
                        }
                        if frames.len() > 16
                            || frames.iter().any(|f| {
                                f.get("id").and_then(Value::as_u64).is_none()
                                    || f.get("line").and_then(Value::as_u64).is_none()
                            })
                        {
                            return Err(DapError::ExcessiveData);
                        }
                        // Only after a successful, correlated and bounded native
                        // stack reply may a source-coordinate review begin.
                        // The original source and symbol hashes still require
                        // independent native verification before any line claim.
                        self.last_source_candidate = frames
                            .iter()
                            .map(candidate_from_dap_frame)
                            .collect::<Result<Vec<_>, _>>()?
                            .into_iter()
                            .flatten()
                            .next();
                        Ok(Some(DapEvent::StackTraceReceived))
                    }
                    #[cfg(feature = "dev046c-ci-control")]
                    "next" | "stepIn" | "stepOut"
                        if matches!(self.phase, DapPhase::Ready | DapPhase::Stopped) =>
                    {
                        let active = self.ci_control.as_mut()
                            .ok_or(DapError::InvalidPhase)?;
                        if active.name != expected || active.expected_line.is_some()
                            || active.response_ack {
                            return Err(DapError::InvalidMessage);
                        }
                        active.response_ack = true;
                        Ok(None)
                    }
                    #[cfg(feature = "dev046c-ci-control")]
                    "setBreakpoints" if self.phase == DapPhase::Stopped => {
                        let active = self.ci_control.as_mut()
                            .ok_or(DapError::InvalidPhase)?;
                        let expected_line = active.expected_line
                            .ok_or(DapError::InvalidPhase)?;
                        let reported = obj.get("body")
                            .and_then(|v| v.get("breakpoints"))
                            .and_then(Value::as_array)
                            .ok_or(DapError::InvalidMessage)?;
                        if reported.len() != 1
                            || reported[0].get("verified") != Some(&Value::Bool(true))
                            || reported[0].get("line").and_then(Value::as_u64)
                                != Some(u64::from(expected_line))
                            || active.name != "setBreakpoints" || active.response_ack {
                            return Err(DapError::InvalidMessage);
                        }
                        active.response_ack = true;
                        // This is still only LLDB's claim, not a memory/hit proof.
                        Ok(None)
                    }
                    "disconnect" if self.phase == DapPhase::Closing => {
                        self.phase = DapPhase::Terminated;
                        self.last_source_candidate = None;
                        Ok(Some(DapEvent::Disconnected))
                    }
                    _ => Err(DapError::InvalidPhase),
                }
            }
            Some("event") => {
                let event = obj
                    .get("event")
                    .and_then(Value::as_str)
                    .ok_or(DapError::InvalidMessage)?;
                match event {
                    "initialized"
                        if !self.initialized_event
                            && matches!(
                                self.phase,
                                DapPhase::Ready | DapPhase::Launching | DapPhase::Stopped
                            ) =>
                    {
                        self.initialized_event = true;
                        Ok(Some(DapEvent::ConfigurationRequested))
                    }
                    "stopped"
                        if matches!(
                            self.phase,
                            DapPhase::Ready | DapPhase::Launching | DapPhase::Stopped
                        ) =>
                    {
                        let thread_id = obj
                            .get("body")
                            .and_then(|b| b.get("threadId"))
                            .and_then(Value::as_u64)
                            .filter(|v| *v > 0 && *v <= MAX_SEQ)
                            .ok_or(DapError::InvalidMessage)?;
                        self.phase = DapPhase::Stopped;
                        #[cfg(feature = "dev046c-ci-control")]
                        {
                            self.last_stopped_thread = Some(thread_id);
                            if let Some(control) = self.ci_control.as_mut() {
                                if control.expected_line.is_none()
                                    && control.thread_id == thread_id {
                                    control.new_stop = true;
                                }
                            }
                        }
                        Ok(Some(DapEvent::Stopped { thread_id }))
                    }
                    "continued" if matches!(self.phase, DapPhase::Ready | DapPhase::Stopped) => {
                        self.phase = DapPhase::Ready;
                        self.last_source_candidate = None;
                        #[cfg(feature = "dev046c-ci-control")]
                        { self.last_stopped_thread = None; }
                        Ok(Some(DapEvent::Continued))
                    }
                    "terminated" | "exited"
                        if !matches!(self.phase, DapPhase::Detached | DapPhase::Initializing) =>
                    {
                        self.phase = DapPhase::Terminated;
                        self.pending.clear();
                        self.last_source_candidate = None;
                        #[cfg(feature = "dev046c-ci-control")]
                        { self.ci_control = None; self.last_stopped_thread = None; }
                        Ok(Some(DapEvent::Terminated))
                    }
                    // Outputs and capabilities can contain hostile text/paths.
                    // Do not forward them to WebView or execute adapter commands.
                    "output" | "thread" | "module" | "loadedSource" | "process" | "breakpoint"
                    | "capabilities" | "invalidated" | "memory" | "progressStart"
                    | "progressUpdate" | "progressEnd"
                        if !matches!(self.phase, DapPhase::Detached) =>
                    {
                        Ok(None)
                    }
                    _ => Err(DapError::InvalidMessage),
                }
            }
            _ => Err(DapError::InvalidMessage),
        }
    }
}
// A native DAP error may embed project paths and user data. Its diagnostic
// witness is intentionally a fixed vocabulary and never part of authorization.
fn classify_native_adapter_error(response: &serde_json::Map<String, Value>) -> &'static str {
    // LLDB-DAP often puts a generic "error" in message and the actual
    // reason in body.error.format. Classify the BOUNDED nested reason first.
    // Never expose the message, arbitrary paths, or debugger-provided text.
    // LLDB adapters may report launch failures in body.error.format,
    // description, message, or as a plain string. Inspect only bounded
    // in-process diagnostic fragments, never display raw adapter content.
    let error = response.get("body").and_then(|body| body.get("error"));
    let fragments = [
        error
            .and_then(|value| value.get("format"))
            .and_then(Value::as_str),
        error
            .and_then(|value| value.get("description"))
            .and_then(Value::as_str),
        error
            .and_then(|value| value.get("message"))
            .and_then(Value::as_str),
        error.and_then(Value::as_str),
        response
            .get("body")
            .and_then(|body| body.get("message"))
            .and_then(Value::as_str),
        response.get("message").and_then(Value::as_str),
    ];
    let text = fragments
        .into_iter()
        .flatten()
        .map(|value| value.chars().take(384).collect::<String>())
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    if text.contains("ptrace") || text.contains("trace me") {
        "ptrace_or_trace_denied"
    } else if text.contains("personality") || text.contains("aslr") {
        "aslr_personality_or_debuggee_policy_denied"
    } else if text.contains("seccomp") || text.contains("sandbox") {
        "sandbox_execution_denied"
    } else if text.contains("permission denied")
        || text.contains("operation not permitted")
        || text.contains("access denied")
    {
        "kernel_or_file_access_denied"
    } else if text.contains("no such file")
        || text.contains("not found")
        || text.contains("file does not exist")
    {
        "dependency_or_fixed_target_missing"
    } else if text.contains("spawn")
        || text.contains("failed to launch")
        || text.contains("launch failed")
        || text.contains("could not launch")
    {
        "target_launch_failed"
    } else if text.contains("python") || text.contains("resource") || text.contains("platform") {
        "debugger_resource_not_ready"
    } else if text.contains("invalid argument")
        || text.contains("unsupported")
        || text.contains("invalid launch")
    {
        "unsupported_debug_adapter_operation"
    } else if text.trim().is_empty() {
        "no_adapter_error_detail"
    } else {
        "other_adapter_error"
    }
}
fn frame_length(header: &[u8]) -> Result<usize, DapError> {
    let text = std::str::from_utf8(header).map_err(|_| DapError::InvalidFrame)?;
    let mut length = None;
    for line in text.split("\r\n") {
        let Some(raw) = line.strip_prefix("Content-Length: ") else {
            return Err(DapError::InvalidFrame);
        };
        if length.is_some()
            || raw.is_empty()
            || raw.len() > 5
            || !raw.bytes().all(|c| c.is_ascii_digit())
        {
            return Err(DapError::InvalidFrame);
        }
        let value = raw.parse::<usize>().map_err(|_| DapError::InvalidFrame)?;
        if value == 0 || value > MAX_FRAME {
            return Err(DapError::ExcessiveData);
        }
        length = Some(value);
    }
    length.ok_or(DapError::InvalidFrame)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn framed(value: Value) -> Vec<u8> {
        let b = serde_json::to_vec(&value).unwrap();
        let mut result = format!("Content-Length: {}\r\n\r\n", b.len()).into_bytes();
        result.extend(b);
        result
    }
    #[test]
    fn fragmented_initialize_and_correlated_read_only_queries() {
        let mut s = DapSession::new();
        let wire = s.initialize().unwrap();
        assert!(std::str::from_utf8(&wire)
            .unwrap()
            .contains("supportsRunInTerminalRequest"));
        assert_eq!(s.initialize().unwrap_err(), DapError::InvalidPhase);
        let resp = framed(json!({
            "seq":11,"type":"response","request_seq":1,"command":"initialize","success":true
        }));
        assert_eq!(s.receive(&resp).unwrap(), vec![DapEvent::Initialized]);
        assert_eq!(s.phase(), DapPhase::Ready);
        let evt = framed(json!({"seq": 10, "type":"event","event":"initialized"}));
        assert!(s.receive(&evt[..8]).unwrap().is_empty());
        assert_eq!(
            s.receive(&evt[8..]).unwrap(),
            vec![DapEvent::ConfigurationRequested]
        );
        s.threads().unwrap();
        assert_eq!(
            s.receive(&framed(json!({
                "seq":12,"type":"response","request_seq":2,"command":"threads","success":true,
                "body":{"threads":[{"id":1,"name":"main"}]}
            })))
            .unwrap(),
            vec![DapEvent::ThreadsReceived]
        );
        assert_eq!(
            s.receive(&framed(json!({
                "seq":13,"type":"event","event":"stopped",
                "body":{"reason":"breakpoint","threadId":1}
            })))
            .unwrap(),
            vec![DapEvent::Stopped { thread_id: 1 }]
        );
        s.stack_trace(1).unwrap();
        assert_eq!(
            s.receive(&framed(json!({
                "seq":14,"type":"response","request_seq":3,"command":"stackTrace",
                "success":true,"body":{"stackFrames":[{"id":9,"name":"main","line":3}]}
            })))
            .unwrap(),
            vec![DapEvent::StackTraceReceived]
        );
        let disc = s.disconnect().unwrap();
        assert!(std::str::from_utf8(&disc)
            .unwrap()
            .contains("terminateDebuggee"));
        assert_eq!(
            s.receive(&framed(json!({
                "seq":15,"type":"response","request_seq":4,"command":"disconnect","success":true
            })))
            .unwrap(),
            vec![DapEvent::Disconnected]
        );
        assert_eq!(s.phase(), DapPhase::Terminated);
    }
    #[test]
    fn empty_native_dap_threads_and_stack_are_not_runtime_evidence() {
        // Native OS execution must not be accepted on a successful DAP
        // response that contains no real thread or no real stack frame.
        let mut threads = DapSession::new();
        threads.phase = DapPhase::Stopped;
        threads.pending.insert(1, "threads");
        assert_eq!(
            threads.receive(&framed(json!({
                "seq": 2, "type": "response", "request_seq": 1,
                "command": "threads", "success": true,
                "body": {"threads": []}
            }))).unwrap_err(),
            DapError::InvalidMessage
        );
        assert_eq!(threads.phase(), DapPhase::Quarantined);

        let mut stack = DapSession::new();
        stack.phase = DapPhase::Stopped;
        stack.pending.insert(1, "stackTrace");
        assert_eq!(
            stack.receive(&framed(json!({
                "seq": 3, "type": "response", "request_seq": 1,
                "command": "stackTrace", "success": true,
                "body": {"stackFrames": []}
            }))).unwrap_err(),
            DapError::InvalidMessage
        );
        assert_eq!(stack.phase(), DapPhase::Quarantined);
    }
    #[test]
    fn only_native_sealed_project_path_can_form_project_launch_without_arguments(){
        let mut session=DapSession::new();
        session.phase=DapPhase::Ready;
        for input in [
            "/tmp/malicious","/debug-target/target/debug/../../etc/passwd",
            "/debug-target/target/debug/other.so",
            "/debug-target/target/debug/evil;sh",
            "/workspace/target/debug/project",
        ] {
            assert_eq!(session.launch_verified_project(input).unwrap_err(),
                DapError::InvalidMessage);
        }
        let wire=session.launch_verified_project(
            "/debug-target/target/debug/fixture").unwrap();
        let wire=String::from_utf8(wire).unwrap();
        assert!(wire.contains("/debug-target/target/debug/fixture"));
        assert!(wire.contains(r#""runInTerminal":false"#));
        assert!(wire.contains(r#""disableSTDIO":true"#));
        assert!(wire.contains(r#""disableASLR":false"#));
        assert_eq!(session.phase(),DapPhase::Launching);
        assert_eq!(session.launch_verified_project(
            "/debug-target/target/debug/fixture").unwrap_err(),
            DapError::InvalidPhase);
    }
    #[cfg(windows)]
    #[test]
    fn native_appcontainer_project_path_can_launch_but_foreign_windows_paths_cannot(){
        let mut session=DapSession::new();
        session.phase=DapPhase::Ready;
        assert_eq!(session.launch_verified_project(
            r"C:\Windows\System32\cmd.exe").unwrap_err(),DapError::InvalidMessage);
        assert_eq!(session.launch_verified_project(
            r"C:\Temp\debug-target\target\debug\..\evil.exe").unwrap_err(),
            DapError::InvalidMessage);
        let frame=session.launch_verified_project(
            r"C:\Users\runner\AppData\Local\Packages\debug-target\target\debug\fixture.exe")
            .unwrap();
        assert!(String::from_utf8(frame).unwrap().contains("stopOnEntry"));
    }
    #[test]
    fn only_fixed_native_fixture_can_be_launched_and_response_is_correlated() {
        let mut session = DapSession::new();
        session.initialize().unwrap();
        let resp = framed(json!({
            "seq":2,"type":"response","request_seq":1,
            "command":"initialize","success":true
        }));
        session.receive(&resp).unwrap();
        session
            .receive(&framed(json!({
                "seq":3,"type":"event","event":"initialized"
            })))
            .unwrap();
        assert_eq!(session.phase(), DapPhase::Ready);
        assert_eq!(
            session.launch_fixed_fixture("/tmp/other").unwrap_err(),
            DapError::InvalidMessage
        );
        assert_eq!(
            session
                .launch_fixed_fixture("/ls/debuggee/../secrets")
                .unwrap_err(),
            DapError::InvalidMessage
        );
        let wire = session
            .launch_fixed_fixture("/ls/debuggee/wo6-selftest")
            .unwrap();
        assert!(std::str::from_utf8(&wire).unwrap().contains("stopOnEntry"));
        // Preserve ASLR rather than asking the confined debugger to invoke
        // prohibited personality(ADDR_NO_RANDOMIZE), as LLVM itself recommends
        // for isolated native test environments.
        assert!(std::str::from_utf8(&wire)
            .unwrap()
            .contains(r#""disableASLR":false"#));
        assert!(std::str::from_utf8(&wire)
            .unwrap()
            .contains(r#""disableSTDIO":true"#));
        assert_eq!(session.phase(), DapPhase::Launching);
        let observed = session
            .receive(&framed(json!({
                "seq":4,"type":"event","event":"stopped",
                "body":{"reason":"entry","threadId":1}
            })))
            .unwrap();
        assert_eq!(observed, vec![DapEvent::Stopped { thread_id: 1 }]);
        assert_eq!(session.phase(), DapPhase::Stopped);
        assert_eq!(
            session
                .receive(&framed(json!({
                    "seq":5,"type":"response","request_seq":2,
                    "command":"launch","success":true
                })))
                .unwrap(),
            vec![DapEvent::LaunchAcknowledged]
        );
    }

    #[test]
    fn native_adapter_sequence_zero_and_standard_progress_are_not_authority() {
        let mut session = DapSession::new();
        session.initialize().unwrap();
        // Several standards-conforming adapters start their own sequence at 0.
        // That sequence cannot satisfy the independently tracked client ID.
        let event = framed(json!({
            "seq":0,"type":"event","event":"progressStart",
            "body":{"progressId":"test","title":"untrusted output"}
        }));
        assert!(session.receive(&event).unwrap().is_empty());
        let response = framed(json!({
            "seq":0,"type":"response","request_seq":1,
            "command":"initialize","success":true
        }));
        assert_eq!(
            session.receive(&response).unwrap(),
            vec![DapEvent::Initialized]
        );
        assert_eq!(session.phase(), DapPhase::Ready);
        assert_eq!(
            session
                .receive(&framed(json!({
                    "seq":1,"type":"event","event":"initialized"
                })))
                .unwrap(),
            vec![DapEvent::ConfigurationRequested]
        );
        assert_eq!(session.phase(), DapPhase::Ready);
        // An uncorrelated reply stays rejected even with a legal server seq.
        assert_eq!(
            session
                .receive(&framed(json!({
                    "seq":0,"type":"response","request_seq":0,
                    "command":"threads","success":true
                })))
                .unwrap_err(),
            DapError::UnexpectedResponse
        );
        assert_eq!(session.phase(), DapPhase::Quarantined);
    }
    #[test]
    fn launch_after_initialize_response_and_config_only_after_native_event() {
        let mut session = DapSession::new();
        session.initialize().unwrap();
        assert_eq!(
            session.configuration_done().unwrap_err(),
            DapError::InvalidPhase
        );
        assert_eq!(
            session
                .receive(&framed(json!({
                    "seq":1,"type":"response","request_seq":1,
                    "command":"initialize","success":true
                })))
                .unwrap(),
            vec![DapEvent::Initialized]
        );
        assert_eq!(session.phase(), DapPhase::Ready);
        // This must not wait for the adapter's initialized EVENT, which LLDB
        // may send only after the client's fixed, supervisor-authorized launch.
        session
            .launch_fixed_fixture("/ls/debuggee/wo6-selftest")
            .unwrap();
        assert_eq!(
            session.configuration_done().unwrap_err(),
            DapError::InvalidPhase
        );
        assert_eq!(
            session
                .receive(&framed(json!({
                    "seq":2,"type":"event","event":"initialized"
                })))
                .unwrap(),
            vec![DapEvent::ConfigurationRequested]
        );
        let frame = session.configuration_done().unwrap();
        assert!(std::str::from_utf8(&frame)
            .unwrap()
            .contains("configurationDone"));
        assert_eq!(
            session.configuration_done().unwrap_err(),
            DapError::InvalidPhase
        );
        assert!(session
            .receive(&framed(json!({
                "seq":3,"type":"response","request_seq":3,
                "command":"configurationDone","success":true
            })))
            .unwrap()
            .is_empty());
        assert_eq!(
            session
                .receive(&framed(json!({
                    "seq":4,"type":"event","event":"initialized"
                })))
                .unwrap_err(),
            DapError::InvalidMessage
        );
        assert_eq!(session.phase(), DapPhase::Quarantined);
    }

    #[test]
    fn early_valid_initialized_event_is_deferred_until_fixed_launch() {
        let mut session = DapSession::new();
        session.initialize().unwrap();
        session
            .receive(&framed(json!({
                "seq": 1, "type": "response", "request_seq": 1,
                "command": "initialize", "success": true
            })))
            .unwrap();
        assert_eq!(
            session
                .receive(&framed(json!({
                    "seq": 2, "type": "event", "event": "initialized"
                })))
                .unwrap(),
            vec![DapEvent::ConfigurationRequested]
        );
        assert!(!session.configuration_needed());
        assert_eq!(
            session.configuration_done().unwrap_err(),
            DapError::InvalidPhase
        );
        session
            .launch_fixed_fixture("/ls/debuggee/wo6-selftest")
            .unwrap();
        assert!(session.configuration_needed());
        session.configuration_done().unwrap();
        assert!(!session.configuration_needed());
    }

    #[test]
    fn premature_initialized_event_denied_without_response() {
        let mut session = DapSession::new();
        session.initialize().unwrap();
        assert_eq!(
            session
                .receive(&framed(json!({
                    "seq":1,"type":"event","event":"initialized"
                })))
                .unwrap_err(),
            DapError::InvalidMessage
        );
        assert_eq!(session.phase(), DapPhase::Quarantined);
    }

    #[test]
    fn native_adapter_error_classes_never_include_untrusted_paths() {
        let mut response = serde_json::Map::new();
        response.insert(
            "message".into(),
            Value::String("error: ptrace operation not permitted /private/source.c".into()),
        );
        assert_eq!(
            classify_native_adapter_error(&response),
            "ptrace_or_trace_denied"
        );
        response.insert(
            "message".into(),
            Value::String("error: failed to launch /private/source.c".into()),
        );
        assert_eq!(
            classify_native_adapter_error(&response),
            "target_launch_failed"
        );
        response.insert("message".into(), Value::String("error".into()));
        response.insert(
            "body".into(),
            serde_json::json!({"error":{"format":"personality set failed: Operation not permitted"}}),
        );
        assert_eq!(
            classify_native_adapter_error(&response),
            "aslr_personality_or_debuggee_policy_denied"
        );
        response.insert(
            "body".into(),
            serde_json::json!({"error":{"format":"ptrace denied by /secret/project"}}),
        );
        assert_eq!(
            classify_native_adapter_error(&response),
            "ptrace_or_trace_denied"
        );
        response.insert(
            "body".into(),
            serde_json::json!({"error":{"description":"ptrace access denied /private/source.c"}}),
        );
        assert_eq!(
            classify_native_adapter_error(&response),
            "ptrace_or_trace_denied"
        );
        response.insert(
            "body".into(),
            serde_json::json!({"error":{"message":"No such file or directory /private/fixture"}}),
        );
        assert_eq!(
            classify_native_adapter_error(&response),
            "dependency_or_fixed_target_missing"
        );
        response.insert(
            "body".into(),
            serde_json::json!({"error":"operation not permitted /private/fixture"}),
        );
        assert_eq!(
            classify_native_adapter_error(&response),
            "kernel_or_file_access_denied"
        );
        response.insert(
            "body".into(),
            serde_json::json!({"message":"target failed to launch /private/fixture"}),
        );
        assert_eq!(
            classify_native_adapter_error(&response),
            "target_launch_failed"
        );
        response.clear();
        assert_eq!(
            classify_native_adapter_error(&response),
            "no_adapter_error_detail"
        );
    }

    #[test]
    fn reverse_request_is_quarantined_not_dispatched() {
        let mut s = DapSession::new();
        s.initialize().unwrap();
        assert_eq!(
            s.receive(&framed(json!({
                "seq": 2, "type":"request","command":"runInTerminal",
                "arguments":{"kind":"integrated","args":["bash","-c","evil"]}
            })))
            .unwrap_err(),
            DapError::AdapterRequestDenied
        );
        assert_eq!(s.phase(), DapPhase::Quarantined);
        assert_eq!(s.receive(b"").unwrap_err(), DapError::InvalidPhase);
    }
    #[test]
    fn rejects_duplicate_headers_oversized_body_and_missing_response_correlation() {
        assert_eq!(
            frame_length(b"Content-Length: 4\r\nContent-Length: 4").unwrap_err(),
            DapError::InvalidFrame
        );
        assert_eq!(
            frame_length(b"Content-Length: 99999").unwrap_err(),
            DapError::ExcessiveData
        );
        let mut s = DapSession::new();
        s.initialize().unwrap();
        assert_eq!(
            s.receive(&framed(json!({
                "seq": 2,"type":"response","request_seq":50,
                "command":"initialize","success":true
            })))
            .unwrap_err(),
            DapError::UnexpectedResponse
        );
        assert_eq!(s.phase(), DapPhase::Quarantined);
    }
    #[test]
    fn launch_commands_and_restart_do_not_exist() {
        let mut s = DapSession::new();
        assert_eq!(s.threads().unwrap_err(), DapError::InvalidPhase);
        assert_eq!(s.stack_trace(1).unwrap_err(), DapError::InvalidPhase);
        assert_eq!(s.disconnect().unwrap_err(), DapError::InvalidPhase);
        // Program execution authority must never be inferred from DAP JSON.
        s.initialize().unwrap();
        let msg = framed(json!({"seq":9,"type":"event","event":"someUnsafeExtension"}));
        assert_eq!(s.receive(&msg).unwrap_err(), DapError::InvalidMessage);
        assert_eq!(s.phase(), DapPhase::Quarantined);
    }

    #[cfg(feature = "dev046c-ci-control")]
    #[test]
    fn ci_control_denies_unstopped_or_unmapped_threads_without_wire() {
        let mut session = DapSession::new();
        assert!(session.ci_step(7, CiStepKind::Over).is_err());
        session.phase = DapPhase::Stopped;
        session.last_stopped_thread = Some(7);
        assert!(session.ci_step(7, CiStepKind::Over).is_err());
        assert!(session.ci_set_breakpoint(r"C:\reviewed\src\main.rs", 5).is_err());
        assert!(session.ci_take_control_observation().is_none());
    }
    #[cfg(feature = "dev046c-ci-control")]
    #[test]
    fn ci_step_response_is_not_a_result_without_new_stop() {
        let mut session = DapSession::new();
        session.phase = DapPhase::Stopped;
        session.last_stopped_thread = Some(7);
        session.last_source_candidate = Some(candidate_from_dap_frame(&json!({
            "id": 1, "line": 5, "column": 1,
            "instructionPointerReference": "0x140001010",
            "source": {"path": r"C:\reviewed\src\main.rs"}
        })).unwrap().unwrap());
        let wire = session.ci_step(7, CiStepKind::Over).unwrap();
        assert!(wire.windows(4).any(|w| w == b"next"));
        assert!(session.ci_take_control_observation().is_none());
        // A correlated adapter ACK cannot establish an OS step outcome.
        let seq = session.seq - 1;
        session.receive(&framed(json!({
            "seq": 99, "type": "response", "request_seq": seq,
            "command": "next", "success": true
        }))).unwrap();
        assert!(session.ci_take_control_observation().is_none());
        session.receive(&framed(json!({
            "seq": 100, "type": "event", "event": "stopped",
            "body": {"threadId": 7, "reason": "step"}
        }))).unwrap();
        let obs = session.ci_take_control_observation().unwrap();
        assert!(obs.response_acknowledged && obs.new_stop_observed && obs.is_step);
        assert!(session.ci_take_control_observation().is_none());
    }
    #[test]
    fn dev045d_only_correlated_stack_can_supply_bounded_source_candidate() {
        let mut s = DapSession::new();
        s.phase = DapPhase::Stopped;
        s.pending.insert(1, "stackTrace");
        assert!(s.source_candidate().is_none());
        assert_eq!(
            s.receive(&framed(json!({
                "seq": 3, "type": "response", "request_seq": 1,
                "command": "stackTrace", "success": true,
                "body": {"stackFrames": [{
                    "id": 7, "line": 2, "column": 4,
                    "source": {"path": "/trusted/src/main.rs", "sourceReference": 0}
                }]}
            }))).unwrap(),
            vec![DapEvent::StackTraceReceived]
        );
        let candidate = s.source_candidate().unwrap();
        assert_eq!(candidate.frame_id, 7);
        assert_eq!(candidate.line, 2);
        assert_eq!(candidate.column, Some(4));
        assert_eq!(candidate.adapter_path, "/trusted/src/main.rs");
        s.stack_trace(1).unwrap();
        assert!(s.source_candidate().is_none(), "new query must revoke old location");
    }

    #[test]
    fn dev045d_unmapped_frame_never_becomes_source_line_evidence() {
        let mut s = DapSession::new();
        s.phase = DapPhase::Stopped;
        s.pending.insert(1, "stackTrace");
        assert_eq!(
            s.receive(&framed(json!({
                "seq": 2, "type": "response", "request_seq": 1,
                "command": "stackTrace", "success": true,
                "body": {"stackFrames": [{"id": 9, "line": 3}]}
            }))).unwrap(),
            vec![DapEvent::StackTraceReceived]
        );
        assert!(s.source_candidate().is_none());
    }

    #[test]
    fn dev045d_invalid_adapter_source_quarantines_no_executable_authority() {
        let mut s = DapSession::new();
        s.phase = DapPhase::Stopped;
        s.pending.insert(1, "stackTrace");
        assert_eq!(
            s.receive(&framed(json!({
                "seq": 2, "type": "response", "request_seq": 1,
                "command": "stackTrace", "success": true,
                "body": {"stackFrames": [{
                    "id": 9, "line": 1,
                    "source": {"path": "/trusted/../other.rs", "sourceReference": 0}
                }]}
            }))).unwrap_err(),
            DapError::InvalidMessage
        );
        assert_eq!(s.phase(), DapPhase::Quarantined);
        assert!(s.source_candidate().is_none());
    }
}
