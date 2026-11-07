//! WO6: bounded Debug Adapter Protocol (DAP) framing and response correlation.
//!
//! This is NOT an adapter launcher. An untrusted debugger has program-execution
//! power and must be pinned, approved, confined and supervised by the native
//! OS worker *before* this protocol processor is attached to its stdio.
//! The default desktop has no DAP executable, launch/evaluate/terminal command,
//! or WebView-to-process bridge. All source and process selection belongs to
//! the trusted host and one-use D3 authorization, not to these messages.
use std::collections::BTreeMap;

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
        }
    }
    pub fn phase(&self) -> DapPhase {
        self.phase
    }
    fn fail(&mut self, why: DapError) -> DapError {
        self.phase = DapPhase::Quarantined;
        self.input.clear();
        self.pending.clear();
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
                "args": [], "env": {}, "runInTerminal": false
            }),
        )?;
        self.launch_sent = true;
        self.phase = DapPhase::Launching;
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
        self.request(
            "stackTrace",
            json!({"threadId":thread_id, "startFrame":0, "levels":16}),
        )
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
                        if frames.len() > 16
                            || frames.iter().any(|f| {
                                f.get("id").and_then(Value::as_u64).is_none()
                                    || f.get("line").and_then(Value::as_u64).is_none()
                            })
                        {
                            return Err(DapError::ExcessiveData);
                        }
                        Ok(Some(DapEvent::StackTraceReceived))
                    }
                    "disconnect" if self.phase == DapPhase::Closing => {
                        self.phase = DapPhase::Terminated;
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
                        Ok(Some(DapEvent::Stopped { thread_id }))
                    }
                    "continued" if matches!(self.phase, DapPhase::Ready | DapPhase::Stopped) => {
                        self.phase = DapPhase::Ready;
                        Ok(Some(DapEvent::Continued))
                    }
                    "terminated" | "exited"
                        if !matches!(self.phase, DapPhase::Detached | DapPhase::Initializing) =>
                    {
                        self.phase = DapPhase::Terminated;
                        self.pending.clear();
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
    let detail = response
        .get("body")
        .and_then(|value| value.get("error"))
        .and_then(|value| value.get("format"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let headline = response
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("");
    let text = format!("{detail} {headline}").to_ascii_lowercase();
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
}
