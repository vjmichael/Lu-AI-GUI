//! WO6 protected Debug Adapter Protocol transport.
//!
//! An OS-specific native adapter MUST first authenticate a trusted, pinned
//! LLDB-DAP process inside an already-running restricted Job/cgroup and supply
//! an owned kernel-bound pipe implementation. This is NOT an adapter launcher.
//! A WebView, AI tool, project or D1 intent cannot construct this gateway.
use crate::{
    lsp_transport::RestrictedLspProcess, JobPhase, NativeTreeTerminator, OsContainmentProof,
    Supervisor, SupervisorError,
};
use lu_ide_workspace::dap_session::{DapEvent, DapPhase, DapSession};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

fn trace_dap(stage: &'static str) {
    if cfg!(debug_assertions)
        && std::env::var_os("LU_IDE_E2_NATIVE_TRACE").as_deref() == Some(std::ffi::OsStr::new("1"))
    {
        eprintln!("PROTECTED_DAP_GATEWAY_STAGE={stage}");
    }
}

const MAX_READ: usize = 16 * 1024;
const MAX_WRITE: usize = 64 * 1024 + 512;
const MAX_TOTAL_IO: usize = 16 * 1024 * 1024;
const READ_WAIT: Duration = Duration::from_millis(400);
const MAX_LIFETIME: Duration = Duration::from_secs(3 * 60);

pub(crate) struct DapGateway {
    supervisor: Arc<Supervisor>,
    job_id: String,
    plan_sha256: String,
    session: DapSession,
    process: Option<Box<dyn RestrictedLspProcess>>,
    started: Instant,
    transferred: usize,
    finished: bool,
}
impl DapGateway {
    /// Must be invoked only by the native protected adapter worker, after
    /// receiving kernel Job/cgroup evidence and recording Running durably.
    /// Passing arbitrary booleans is never substitute for a live Job handle.
    pub(crate) fn attach_native(
        supervisor: Arc<Supervisor>,
        job_id: &str,
        expected_plan_sha256: &str,
        kernel_proof: &OsContainmentProof,
        process: Box<dyn RestrictedLspProcess>,
    ) -> Result<Self, SupervisorError> {
        let summary = supervisor
            .inspect()?
            .into_iter()
            .find(|job| job.job_id == job_id)
            .ok_or(SupervisorError::NoSuchJob)?;
        if summary.plan_sha256 != expected_plan_sha256
            || summary.phase != JobPhase::Running
            || summary.requires_reconciliation
            || !kernel_proof.complete()
            || !matches!(kernel_proof.platform.as_str(), "windows" | "linux")
        {
            return Err(SupervisorError::NoContainment);
        }
        Ok(Self {
            supervisor,
            job_id: job_id.to_owned(),
            plan_sha256: expected_plan_sha256.to_owned(),
            session: DapSession::new(),
            process: Some(process),
            started: Instant::now(),
            transferred: 0,
            finished: false,
        })
    }
    pub(crate) fn phase(&self) -> DapPhase {
        self.session.phase()
    }
    /// Read-only native inspection after a correlated stackTrace; adapter
    /// supplied coordinates are untrusted until an independent symbol oracle.
    pub(crate) fn source_candidate(
        &self,
    ) -> Option<lu_ide_workspace::dap_source_lines::SourceLineCandidate> {
        self.session.source_candidate().cloned()
    }
    /// FIN-04R-1: only the active protected DAP actor can ask its own
    /// AppContainer/SCM Job to run the pinned read-only PDB symbolizer.
    /// Results remain untrusted until the independent source-seal parser
    /// and the current first-party event owner are rechecked.
    #[cfg(all(windows, feature = "fin04-release-owner-evidence"))]
    pub(crate) fn read_pe_pdb_inside_owned_job(
        &mut self, binary:&std::path::Path, pdb:&std::path::Path,
        binary_sha256:&str, pdb_sha256:&str, bundle_sha256:&str,
        slide:u64, rip:u64,
    )->Result<serde_json::Value,SupervisorError>{
        if self.session.phase()!=DapPhase::Stopped ||
           self.session.source_candidate()
             .and_then(|f|f.instruction_address)!=Some(rip) ||
           self.supervisor.inspect()?.iter().all(|job|
             job.job_id!=self.job_id ||
             job.plan_sha256!=self.plan_sha256 ||
             job.phase!=JobPhase::Running || job.requires_reconciliation)
        {
            return Err(SupervisorError::NoContainment);
        }
        // A helper crash, timeout, malformed output or any lost native
        // custody cannot leave the original debugger Job running merely
        // because a caller later decides not to drop its session. Quarantine
        // immediately, journal uncertainty, and reap *that same* Job.
        let result=match self.process.as_mut()
            .ok_or(SupervisorError::NoContainment)?
            .read_protected_pe_pdb(binary,pdb,binary_sha256,pdb_sha256,
                bundle_sha256,slide,rip) {
                Ok(result)=>result,
                Err(_)=>return self.quarantine(),
            };
        if self.session.phase()!=DapPhase::Stopped ||
           self.session.source_candidate()
               .and_then(|f|f.instruction_address)!=Some(rip) ||
           self.supervisor.inspect()?.iter().all(|job|
               job.job_id!=self.job_id ||
               job.plan_sha256!=self.plan_sha256 ||
               job.phase!=JobPhase::Running || job.requires_reconciliation)
        {
            return self.quarantine();
        }
        Ok(result)
    }
    /// Read-only OS evidence from the *owned* native Job. A module hit is
    /// never a verified stopped x64 thread context or a PDB line-table pass.
    #[cfg(windows)]
    pub(crate) fn observe_windows_project_module(
        &mut self,
        expected_binary: &std::path::Path,
        sha: &str,
        pc: u64,
    ) -> Result<crate::windows_kernel_pc::WindowsLoadedModuleWitness, SupervisorError> {
        if self.session.phase() != DapPhase::Stopped
            || self
                .session
                .source_candidate()
                .and_then(|f| f.instruction_address)
                != Some(pc)
        {
            return Err(SupervisorError::NoContainment);
        }
        let process = self
            .process
            .as_mut()
            .ok_or(SupervisorError::NoContainment)?;
        process.observe_windows_job_module(expected_binary, sha, pc)
    }
    /// The only permitted test-only continue is the existing fixed DAP
    /// disconnect, never arbitrary step/continue. The original LLDB
    /// event-owner thread must emit CONTINUED before terminal success.
    #[cfg(all(windows, any(all(feature = "dev045d5-ci-event-owner", debug_assertions), feature="fin04-release-owner-evidence")))]
    pub(crate) fn close_with_actual_owner_continue_for_ci(
        mut self,
        seq: u64,
    ) -> Result<(), SupervisorError> {
        if self.session.phase() != DapPhase::Stopped || seq == 0 {
            return Err(SupervisorError::NoContainment);
        }
        let disconnect = self.session.disconnect().map_err(protocol_denied)?;
        self.send(&disconnect)?;
        self.process
            .as_mut()
            .ok_or(SupervisorError::NoContainment)?
            .wait_owner_continued_for_ci(seq)?;
        self.close()
    }
    /// Fixed CI-only worker failure injection: native process handle only;
    /// Drop of DapGateway must still journal cancellation and kill Job tree.
    #[cfg(all(windows, feature = "dev045d5-ci-event-owner", debug_assertions))]
    pub(crate) fn crash_protected_lldb_for_ci(&mut self) -> Result<(), SupervisorError> {
        if self.session.phase() != DapPhase::Stopped || self.session.source_candidate().is_none() {
            return Err(SupervisorError::NoContainment);
        }
        self.process
            .as_mut()
            .ok_or(SupervisorError::NoContainment)?
            .crash_owned_lldb_for_ci()
    }
    /// Exact kernel DebuggerThread sideband from a separately allowlisted
    /// pipe, not DAP stdout; this has no public Tauri/WebView action.
    #[cfg(all(windows, any(all(debug_assertions, feature = "dev045d5-ci-event-owner"), feature = "fin04-release-owner-evidence")))]
    pub(crate) fn verify_lldb_kernel_event_owner(
        &mut self,
        pid: u32,
        tid: u32,
        rip: u64,
    ) -> Result<crate::windows_event_owner::OwnerEvent, SupervisorError> {
        if self.session.phase() != DapPhase::Stopped
            || self
                .session
                .source_candidate()
                .and_then(|f| f.instruction_address)
                != Some(rip)
        {
            return Err(SupervisorError::NoContainment);
        }
        self.process
            .as_mut()
            .ok_or(SupervisorError::NoContainment)?
            .verify_lldb_kernel_event_owner(pid, tid, rip)
    }
    #[cfg(windows)]
    pub(crate) fn verify_windows_native_debug_port(
        &mut self,
        module: &crate::windows_kernel_pc::WindowsLoadedModuleWitness,
    ) -> Result<crate::windows_kernel_pc::WindowsDebugPortCorroboration, SupervisorError> {
        if self.session.phase() != DapPhase::Stopped
            || self
                .session
                .source_candidate()
                .and_then(|f| f.instruction_address)
                != Some(module.observed_pc)
        {
            return Err(SupervisorError::NoContainment);
        }
        let process = self
            .process
            .as_mut()
            .ok_or(SupervisorError::NoContainment)?;
        process.corroborate_windows_debug_port(module)
    }
    #[cfg(windows)]
    pub(crate) fn sample_windows_stopped_thread_rip(
        &mut self,
        module: &crate::windows_kernel_pc::WindowsLoadedModuleWitness,
    ) -> Result<crate::windows_kernel_pc::WindowsThreadRipSample, SupervisorError> {
        if self.session.phase() != DapPhase::Stopped
            || self
                .session
                .source_candidate()
                .and_then(|f| f.instruction_address)
                != Some(module.observed_pc)
        {
            return Err(SupervisorError::NoContainment);
        }
        let process = self
            .process
            .as_mut()
            .ok_or(SupervisorError::NoContainment)?;
        process.sample_windows_job_thread_rip(module)
    }
    fn send(&mut self, bytes: &[u8]) -> Result<(), SupervisorError> {
        if bytes.is_empty()
            || bytes.len() > MAX_WRITE
            || self.transferred.saturating_add(bytes.len()) > MAX_TOTAL_IO
            || self.started.elapsed() > MAX_LIFETIME
        {
            return self.quarantine();
        }
        self.transferred += bytes.len();
        if self
            .process
            .as_mut()
            .ok_or(SupervisorError::Quarantined)?
            .write_bounded(bytes)
            .is_err()
        {
            return self.quarantine();
        }
        Ok(())
    }
    pub(crate) fn initialize(&mut self) -> Result<(), SupervisorError> {
        let message = self.session.initialize().map_err(protocol_denied)?;
        trace_dap("initialize_request_built");
        self.send(&message)
    }
    /// The native OS worker provides a single immutable bundled self-test
    /// executable path. The WebView cannot choose or modify this target.
    pub(crate) fn launch_fixed_fixture(&mut self, target: &str) -> Result<(), SupervisorError> {
        let frame = self
            .session
            .launch_fixed_fixture(target)
            .map_err(protocol_denied)?;
        self.send(&frame)
    }
    pub(crate) fn launch_verified_project(&mut self, target: &str) -> Result<(), SupervisorError> {
        let frame = self
            .session
            .launch_verified_project(target)
            .map_err(protocol_denied)?;
        self.send(&frame)
    }
    pub(crate) fn configuration_needed(&self) -> bool {
        self.session.configuration_needed()
    }
    pub(crate) fn configuration_done(&mut self) -> Result<(), SupervisorError> {
        // The pinned native adapter can emit initialized only after launch.
        // This is the empty, nonexecuting DAP configuration confirmation.
        let frame = self.session.configuration_done().map_err(protocol_denied)?;
        trace_dap("fixed_configuration_done_requested");
        self.send(&frame)
    }
    pub(crate) fn threads(&mut self) -> Result<(), SupervisorError> {
        let frame = self.session.threads().map_err(protocol_denied)?;
        self.send(&frame)
    }
    pub(crate) fn stack_trace(&mut self, thread_id: u64) -> Result<(), SupervisorError> {
        let frame = self
            .session
            .stack_trace(thread_id)
            .map_err(protocol_denied)?;
        self.send(&frame)
    }
    /// CI-only native dispatcher; private Job-owned DAP protocol object,
    /// not a Tauri/AI tool. Claim consumption and kernel stop validation
    /// must happen in WindowsDapSession BEFORE reaching this gateway.
    #[cfg(all(windows, debug_assertions, feature = "dev046c-ci-control"))]
    pub(crate) fn ci_send_step(
        &mut self,
        thread_id: u64,
        kind: lu_ide_workspace::dap_session::CiStepKind,
    ) -> Result<(), SupervisorError> {
        let frame = self
            .session
            .ci_step(thread_id, kind)
            .map_err(protocol_denied)?;
        self.send(&frame)
    }
    #[cfg(all(windows, debug_assertions, feature = "dev046c-ci-control"))]
    pub(crate) fn ci_send_one_line_breakpoint(&mut self) -> Result<(), SupervisorError> {
        let source = self
            .session
            .source_candidate()
            .ok_or(SupervisorError::NoContainment)?
            .clone();
        let frame = self
            .session
            .ci_set_breakpoint(&source.adapter_path, source.line)
            .map_err(protocol_denied)?;
        self.send(&frame)
    }
    /// One OS-scoped, previously consumed and reviewed read-only inspection.
    /// The gateway does not mint a grant: WindowsDapSession must bind the
    /// originating frame to the same OS-owned stopped RIP/PDB/Job receipt,
    /// durable one-use approval, and unchanged source before either call.
    /// Never available to the ordinary installed/release IDE.
    #[cfg(all(windows, debug_assertions, feature = "dev046c-ci-control"))]
    pub(crate) fn ci_send_scopes(&mut self, frame_id: u64) -> Result<(), SupervisorError> {
        let frame = self.session.ci_scopes(frame_id).map_err(protocol_denied)?;
        self.send(&frame)
    }
    #[cfg(all(windows, debug_assertions, feature = "dev046c-ci-control"))]
    pub(crate) fn ci_scopes_ready(&self) -> bool {
        self.session.ci_scope_ready()
    }
    #[cfg(all(windows, debug_assertions, feature = "dev046c-ci-control"))]
    pub(crate) fn ci_send_variables(&mut self) -> Result<(), SupervisorError> {
        let frame = self.session.ci_variables().map_err(protocol_denied)?;
        self.send(&frame)
    }
    #[cfg(all(windows, debug_assertions, feature = "dev046c-ci-control"))]
    pub(crate) fn ci_take_redacted_variables(
        &mut self,
    ) -> Option<lu_ide_workspace::dap_session::CiVariableReview> {
        self.session.ci_take_redacted_variables()
    }
    /// The LLDB response is correlation ONLY. It does not prove source
    /// breakpoint installation or completed stepping.
    #[cfg(all(windows, debug_assertions, feature = "dev046c-ci-control"))]
    pub(crate) fn ci_take_observation(
        &mut self,
    ) -> Option<lu_ide_workspace::dap_session::CiControlObservation> {
        self.session.ci_take_control_observation()
    }
    pub(crate) fn poll(&mut self) -> Result<Vec<DapEvent>, SupervisorError> {
        if self.started.elapsed() > MAX_LIFETIME {
            return self.quarantine();
        }
        trace_dap("poll_started");
        let bytes = match self
            .process
            .as_mut()
            .ok_or(SupervisorError::Quarantined)?
            .read_bounded(MAX_READ, READ_WAIT)
        {
            Ok(bytes) => bytes,
            Err(_) => {
                trace_dap("native_stdio_read_failed");
                return self.quarantine();
            }
        };
        if bytes.len() > MAX_READ || self.transferred.saturating_add(bytes.len()) > MAX_TOTAL_IO {
            return self.quarantine();
        }
        self.transferred += bytes.len();
        if bytes.is_empty() {
            trace_dap("no_stdout_yet");
            return Ok(Vec::new());
        }
        trace_dap("stdout_bytes_observed");
        match self.session.receive(&bytes) {
            Ok(events) => {
                trace_dap("dap_frame_accepted");
                Ok(events)
            }
            Err(error) => {
                match error {
                    lu_ide_workspace::dap_session::DapError::InvalidFrame => {
                        trace_dap("invalid_frame")
                    }
                    lu_ide_workspace::dap_session::DapError::InvalidMessage => {
                        trace_dap("invalid_message")
                    }
                    lu_ide_workspace::dap_session::DapError::UnexpectedResponse => {
                        trace_dap("unmatched_response")
                    }
                    lu_ide_workspace::dap_session::DapError::AdapterRequestDenied => {
                        trace_dap("reverse_request_denied")
                    }
                    lu_ide_workspace::dap_session::DapError::AdapterFailed => {
                        trace_dap("adapter_error_response")
                    }
                    _ => trace_dap("dap_phase_or_limit_denied"),
                }
                self.quarantine()
            }
        }
    }
    fn quarantine<T>(&mut self) -> Result<T, SupervisorError> {
        // Inspect only fixed debug-only native error categories and real
        // cgroup PID/OOM counters BEFORE cgroup.kill or Windows Job teardown.
        // A DAP error response itself is never an authority grant.
        if let Some(process) = self.process.as_mut() {
            process.trace_native_fault("dap_protocol_quarantined");
        }
        let _ = self.supervisor.native_quarantine_after_error(&self.job_id);
        if let Some(process) = self.process.as_mut() {
            let _ = process.terminate_tree_and_verify();
        }
        Err(SupervisorError::Quarantined)
    }
    /// A successful DAP disconnect ACK is NOT kernel cleanup. Always require
    /// empty Job/cgroup, protected service release, and a durable receipt.
    pub(crate) fn close(mut self) -> Result<(), SupervisorError> {
        if matches!(
            self.session.phase(),
            DapPhase::Ready | DapPhase::Stopped | DapPhase::AwaitingInitializedEvent
        ) {
            if let Ok(frame) = self.session.disconnect() {
                let _ = self.send(&frame);
            }
        }
        if !self
            .process
            .as_mut()
            .ok_or(SupervisorError::Quarantined)?
            .terminate_tree_and_verify()?
        {
            return self.quarantine();
        }
        self.supervisor.native_mark_release_pending(&self.job_id)?;
        self.process
            .as_mut()
            .ok_or(SupervisorError::Quarantined)?
            .release_after_verified_terminal()?;
        self.supervisor
            .native_finish(&self.job_id, true, true, true)?;
        self.finished = true;
        Ok(())
    }
}
fn protocol_denied(_: lu_ide_workspace::dap_session::DapError) -> SupervisorError {
    SupervisorError::NoContainment
}
struct DapTerminator<'a> {
    process: &'a mut dyn RestrictedLspProcess,
    id: &'a str,
    plan: &'a str,
}
impl NativeTreeTerminator for DapTerminator<'_> {
    fn terminate_and_verify_tree(
        &mut self,
        job: &str,
        hash: &str,
    ) -> Result<bool, SupervisorError> {
        if job != self.id || hash != self.plan {
            return Err(SupervisorError::WrongProjectOrSource);
        }
        self.process.terminate_tree_and_verify()
    }
}
impl Drop for DapGateway {
    fn drop(&mut self) {
        if self.finished {
            return;
        }
        if let Some(process) = self.process.as_mut() {
            let mut broker = DapTerminator {
                process: process.as_mut(),
                id: &self.job_id,
                plan: &self.plan_sha256,
            };
            // Record CancelRequested BEFORE OS termination; a failed
            // descendant kill leaves Uncertain and blocks later admissions.
            if self
                .supervisor
                .cancel_running_with(&self.job_id, &mut broker)
                .is_ok()
            {
                if process.release_after_verified_terminal().is_err() {
                    let _ = self.supervisor.native_quarantine_after_error(&self.job_id);
                }
            } else {
                let _ = process.terminate_tree_and_verify();
                let _ = self.supervisor.native_quarantine_after_error(&self.job_id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::JobPlan;
    use serde_json::{json, Value};
    use std::{collections::VecDeque, io, sync::Mutex};
    fn frame(value: Value) -> Vec<u8> {
        let bytes = serde_json::to_vec(&value).unwrap();
        let mut message = format!("Content-Length: {}\r\n\r\n", bytes.len()).into_bytes();
        message.extend(bytes);
        message
    }
    fn plan() -> JobPlan {
        JobPlan {
            project_id: format!("ide-{}", "a".repeat(32)),
            recipe_id: "dap_lldb_fixed".into(),
            source_head: "b".repeat(40),
            input_snapshot_sha256: "c".repeat(64),
            manifest_sha256: "d".repeat(64),
            toolchain_sha256: "e".repeat(64),
            debug_target_binding_sha256: None,
            runtime_limit_ms: 45_000,
            memory_limit_bytes: 256 * 1024 * 1024,
            process_limit: 4,
            log_limit_bytes: 32 * 1024,
            effect_class: "debug_inspection_isolated".into(),
        }
    }
    fn proof() -> OsContainmentProof {
        OsContainmentProof {
            platform: "linux".into(),
            restricted_identity: true,
            filesystem_access_limited: true,
            network_access_limited: true,
            child_contained_before_resume: true,
            process_tree_kill_verified: true,
            memory_quota: true,
            cpu_quota: true,
            pid_quota: true,
            protected_journal_broker: true,
        }
    }
    #[derive(Default)]
    struct Fixture {
        received: Mutex<Vec<Vec<u8>>>,
        pending: Mutex<VecDeque<Vec<u8>>>,
        terminated: Mutex<usize>,
        released: Mutex<usize>,
    }
    struct Mock {
        shared: Arc<Fixture>,
    }
    impl RestrictedLspProcess for Mock {
        fn write_bounded(&mut self, bytes: &[u8]) -> io::Result<()> {
            self.shared.received.lock().unwrap().push(bytes.to_vec());
            Ok(())
        }
        fn read_bounded(&mut self, _: usize, _: Duration) -> io::Result<Vec<u8>> {
            Ok(self
                .shared
                .pending
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_default())
        }
        fn terminate_tree_and_verify(&mut self) -> Result<bool, SupervisorError> {
            *self.shared.terminated.lock().unwrap() += 1;
            Ok(true)
        }
        fn release_after_verified_terminal(&mut self) -> Result<(), SupervisorError> {
            *self.shared.released.lock().unwrap() += 1;
            Ok(())
        }
    }
    #[test]
    fn refuses_to_attach_without_an_os_running_job() {
        let journal = std::env::temp_dir().join(format!("wo6-dap-no-{}", rand::random::<u128>()));
        let supervisor = Arc::new(Supervisor::open(journal.clone()).unwrap());
        let shared = Arc::new(Fixture::default());
        let job = supervisor.prepare(plan()).unwrap();
        assert_eq!(
            DapGateway::attach_native(
                Arc::clone(&supervisor),
                &job.job_id,
                &job.plan_sha256,
                &proof(),
                Box::new(Mock { shared }),
            )
            .err()
            .unwrap(),
            SupervisorError::NoContainment
        );
        drop(supervisor);
        let _ = std::fs::remove_file(journal);
    }
    #[test]
    fn read_only_dap_session_requires_durable_protected_release() {
        let journal = std::env::temp_dir().join(format!("wo6-dap-{}", rand::random::<u128>()));
        let supervisor = Arc::new(Supervisor::open(journal.clone()).unwrap());
        let plan = plan();
        let job = supervisor.prepare(plan.clone()).unwrap();
        supervisor
            .record_human_approval(&job.job_id, &job.plan_sha256, &plan.source_head, true)
            .unwrap();
        supervisor
            .consume_grant_after_attestation(&job.job_id, &plan, &proof())
            .unwrap();
        supervisor.native_mark_running(&job.job_id).unwrap();
        let shared = Arc::new(Fixture::default());
        let mut gateway = DapGateway::attach_native(
            Arc::clone(&supervisor),
            &job.job_id,
            &job.plan_sha256,
            &proof(),
            Box::new(Mock {
                shared: Arc::clone(&shared),
            }),
        )
        .unwrap();
        gateway.initialize().unwrap();
        // Valid DAP adapters answer initialize before their initialized event.
        // The read-only fixture needs no launch or configurationDone at all.
        shared.pending.lock().unwrap().push_back(frame(json!({
            "seq":2,"type":"response","request_seq":1,"command":"initialize","success":true
        })));
        assert_eq!(gateway.poll().unwrap(), vec![DapEvent::Initialized]);
        assert_eq!(gateway.phase(), DapPhase::Ready);
        gateway.threads().unwrap();
        shared.pending.lock().unwrap().push_back(frame(json!({
            "seq":3,"type":"response","request_seq":2,"command":"threads",
            "success":true,"body":{"threads":[]}
        })));
        assert_eq!(gateway.poll().unwrap(), vec![DapEvent::ThreadsReceived]);
        gateway.close().unwrap();
        let last = supervisor.inspect().unwrap().pop().unwrap();
        assert_eq!(last.phase, JobPhase::Completed);
        assert!(!last.requires_reconciliation);
        assert_eq!(*shared.terminated.lock().unwrap(), 1);
        assert_eq!(*shared.released.lock().unwrap(), 1);
        drop(supervisor);
        let _ = std::fs::remove_file(journal);
    }
}
