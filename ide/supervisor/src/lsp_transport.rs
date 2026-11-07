//! Wave E2: supervisor-owned LSP transport bridge.
//!
//! This is NOT an alternate process launcher. An OS-specific D7 supervisor
//! must first start and retain the exact pinned language server *inside*
//! an independently protected Job/cgroup and provide its authenticated pipe
//! handles. The desktop and untrusted project cannot construct a gateway.
//! E2 does not yet implement those OS-specific constructors.
use crate::{JobPhase, NativeTreeTerminator, OsContainmentProof, Supervisor, SupervisorError};
use lu_ide_workspace::lsp::{DiagnosticSource, MAX_MESSAGE};
use lu_ide_workspace::lsp_session::{LspError, LspEvent, LspPhase, LspSession, Position};
use std::{
    io,
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};

/// Mint an LSP URI ONLY for a native-staged, absolute file from the worker's
/// own read-only/private workspace. The server cannot choose a URI to open.
pub(crate) fn trusted_staged_file_uri(path: &Path) -> Result<String, SupervisorError> {
    if !path.is_absolute() {
        return Err(SupervisorError::WrongProjectOrSource);
    }
    let raw = path
        .to_str()
        .ok_or(SupervisorError::WrongProjectOrSource)?
        .replace('\\', "/");
    if raw.len() > 1400 || raw.chars().any(char::is_control) {
        return Err(SupervisorError::WrongProjectOrSource);
    }
    let mut out = String::from("file:///");
    for b in raw.trim_start_matches('/').bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'/' | b'.' | b'-' | b'_' | b':' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    if out.len() > 2048 {
        return Err(SupervisorError::WrongProjectOrSource);
    }
    Ok(out)
}

// Debug/CI-only fixed fault classifications. Do not log protocol bytes,
// source text, client data, paths, credentials or arbitrary error strings.
fn trace_native_lsp_gateway(stage: &'static str) {
    if cfg!(debug_assertions)
        && std::env::var_os("LU_IDE_E2_NATIVE_TRACE").as_deref() == Some(std::ffi::OsStr::new("1"))
    {
        eprintln!("PROTECTED_LSP_GATEWAY_STAGE={stage}");
    }
}

const MAX_CHUNK: usize = 16 * 1024;
const MAX_PENDING_OUTPUT: usize = MAX_MESSAGE + 256;
const READ_WAIT: Duration = Duration::from_millis(400);
const MAX_SESSION_LIFETIME: Duration = Duration::from_secs(5 * 60);
const MAX_TOTAL_IO: usize = 32 * 1024 * 1024;

/// A trusted native worker adapter must own both ends of its OS-restricted
/// pipes and the real kernel containment handle. No blanket std::process::Child
/// or WebView-provided fd is accepted here.
pub(crate) trait RestrictedLspProcess: Send {
    fn write_bounded(&mut self, bytes: &[u8]) -> io::Result<()>;
    fn read_bounded(&mut self, limit: usize, timeout: Duration) -> io::Result<Vec<u8>>;
    /// Must terminate the entire kernel Job/cgroup tree and prove it empty.
    fn terminate_tree_and_verify(&mut self) -> Result<bool, SupervisorError>;
    /// Called only after a durable terminal receipt and verified empty tree.
    /// A D7D service must fsync its independent release witness first.
    fn release_after_verified_terminal(&mut self) -> Result<(), SupervisorError>;
}

pub(crate) struct LspGateway {
    supervisor: Arc<Supervisor>,
    job_id: String,
    session: LspSession,
    process: Option<Box<dyn RestrictedLspProcess>>,
    finished: bool,
    opened_at: Instant,
    io_bytes: usize,
    plan_sha256: String,
}
impl LspGateway {
    /// Only an already-authorized OS worker implementation can call this
    /// internal factory. A struct of bools is not OS proof on its own;
    /// the kernel Job/cgroup handle remains a separate requirement.
    pub(crate) fn attach_native(
        supervisor: Arc<Supervisor>,
        job_id: &str,
        expected_plan_sha256: &str,
        native_proof: &OsContainmentProof,
        process: Box<dyn RestrictedLspProcess>,
        server_id: &str,
    ) -> Result<Self, SupervisorError> {
        let job = supervisor
            .inspect()?
            .into_iter()
            .find(|entry| entry.job_id == job_id)
            .ok_or(SupervisorError::NoSuchJob)?;
        if job.plan_sha256 != expected_plan_sha256
            || job.phase != JobPhase::Running
            || job.requires_reconciliation
            || !native_proof.complete()
            || !matches!(native_proof.platform.as_str(), "windows" | "linux")
        {
            return Err(SupervisorError::NoContainment);
        }
        let session = LspSession::new(server_id).map_err(|_| SupervisorError::InvalidPlan)?;
        Ok(Self {
            supervisor,
            job_id: job_id.to_owned(),
            session,
            process: Some(process),
            finished: false,
            opened_at: Instant::now(),
            io_bytes: 0,
            plan_sha256: expected_plan_sha256.to_owned(),
        })
    }
    fn send(&mut self, frame: &[u8]) -> Result<(), SupervisorError> {
        if frame.len() > MAX_PENDING_OUTPUT
            || self.opened_at.elapsed() > MAX_SESSION_LIFETIME
            || self.io_bytes.saturating_add(frame.len()) > MAX_TOTAL_IO
        {
            return self.deny_and_quarantine();
        }
        self.io_bytes += frame.len();
        let sent = self
            .process
            .as_mut()
            .ok_or(SupervisorError::Quarantined)?
            .write_bounded(frame);
        if sent.is_err() {
            trace_native_lsp_gateway("send_pipe_failed");
            return self.deny_and_quarantine();
        }
        trace_native_lsp_gateway("send_pipe_completed");
        Ok(())
    }
    pub(crate) fn phase(&self) -> LspPhase {
        self.session.phase()
    }
    pub(crate) fn initialize(&mut self) -> Result<(), SupervisorError> {
        let data = self.session.initialize().map_err(protocol_denied)?;
        self.send(&data)
    }
    pub(crate) fn open(
        &mut self,
        source: DiagnosticSource,
        trusted_uri: &str,
        text: &str,
    ) -> Result<(), SupervisorError> {
        let data = self
            .session
            .did_open(source, trusted_uri, text)
            .map_err(protocol_denied)?;
        self.send(&data)
    }
    pub(crate) fn changed(
        &mut self,
        expected: &DiagnosticSource,
        next: DiagnosticSource,
        text: &str,
    ) -> Result<(), SupervisorError> {
        let data = self
            .session
            .did_change(expected, next, text)
            .map_err(protocol_denied)?;
        self.send(&data)
    }
    pub(crate) fn query(
        &mut self,
        kind: &str,
        source: &DiagnosticSource,
        position: Position,
    ) -> Result<(), SupervisorError> {
        let data = self
            .session
            .request(kind, source, position)
            .map_err(protocol_denied)?;
        self.send(&data)
    }
    pub(crate) fn poll(&mut self) -> Result<Vec<LspEvent>, SupervisorError> {
        if self.opened_at.elapsed() > MAX_SESSION_LIFETIME {
            return self.deny_and_quarantine();
        }
        let read = self
            .process
            .as_mut()
            .ok_or(SupervisorError::Quarantined)?
            .read_bounded(MAX_CHUNK, READ_WAIT);
        let frame = match read {
            Ok(frame) => frame,
            Err(_) => {
                trace_native_lsp_gateway("read_pipe_failed");
                return self.deny_and_quarantine();
            }
        };
        if frame.len() > MAX_CHUNK || self.io_bytes.saturating_add(frame.len()) > MAX_TOTAL_IO {
            return self.deny_and_quarantine();
        }
        self.io_bytes += frame.len();
        if frame.is_empty() {
            return Ok(Vec::new());
        }
        trace_native_lsp_gateway("read_pipe_data");
        let (events, replies) = match self.session.receive(&frame) {
            Ok(result) => result,
            Err(_) => {
                trace_native_lsp_gateway("parse_or_protocol_denied");
                return self.deny_and_quarantine();
            }
        };
        trace_native_lsp_gateway("protocol_received");
        for outbound in replies {
            self.send(&outbound)?;
        }
        Ok(events)
    }
    /// On an untrusted protocol fault or process communication failure, do
    /// not keep working. Quarantine and independently kill the whole tree.
    fn deny_and_quarantine<T>(&mut self) -> Result<T, SupervisorError> {
        let _ = self.supervisor.native_quarantine_after_error(&self.job_id);
        self.session.stop_uncertain();
        if let Some(process) = self.process.as_mut() {
            let _ = process.terminate_tree_and_verify();
        }
        Err(SupervisorError::Quarantined)
    }
    /// An orderly session ends only after the entire OS process tree is empty,
    /// a durable D3 terminal receipt exists, and the protected D7D independent
    /// service acknowledges its release. A graceful LSP shutdown alone is
    /// never evidence of process-tree cleanup.
    pub(crate) fn close(mut self) -> Result<(), SupervisorError> {
        if self.session.phase() == LspPhase::Ready {
            if let Ok(frame) = self.session.shutdown() {
                let _ = self.send(&frame);
            }
        }
        let verified = self
            .process
            .as_mut()
            .ok_or(SupervisorError::Quarantined)?
            .terminate_tree_and_verify()?;
        if !verified {
            return self.deny_and_quarantine();
        }
        // Mark the Job as pending protected-service release first. If either
        // process or host crashes before ACK and final receipt, replay must
        // quarantine this Job rather than inventing a successful shutdown.
        self.supervisor.native_mark_release_pending(&self.job_id)?;
        self.process
            .as_mut()
            .ok_or(SupervisorError::Quarantined)?
            .release_after_verified_terminal()?;
        self.supervisor
            .native_finish(&self.job_id, true, true, true)?;
        self.finished = true; // Durable final receipt follows independent ACK.
        Ok(())
    }
}
struct GatewayTerminator<'a> {
    process: &'a mut dyn RestrictedLspProcess,
    job_id: &'a str,
    plan_sha256: &'a str,
}
impl NativeTreeTerminator for GatewayTerminator<'_> {
    fn terminate_and_verify_tree(
        &mut self,
        job_id: &str,
        plan_sha256: &str,
    ) -> Result<bool, SupervisorError> {
        if job_id != self.job_id || plan_sha256 != self.plan_sha256 {
            return Err(SupervisorError::WrongProjectOrSource);
        }
        self.process.terminate_tree_and_verify()
    }
}
impl Drop for LspGateway {
    fn drop(&mut self) {
        if self.finished {
            return;
        }
        self.session.stop_uncertain();
        if let Some(process) = self.process.as_mut() {
            // A routine editor close, host drop or cancellation MUST fsync
            // CancelRequested before it kills an active contained process.
            let mut killer = GatewayTerminator {
                process: process.as_mut(),
                job_id: &self.job_id,
                plan_sha256: &self.plan_sha256,
            };
            if self
                .supervisor
                .cancel_running_with(&self.job_id, &mut killer)
                .is_ok()
            {
                let _ = process.release_after_verified_terminal();
            } else {
                let _ = process.terminate_tree_and_verify();
                let _ = self.supervisor.native_quarantine_after_error(&self.job_id);
            }
        }
    }
}
fn protocol_denied(_: LspError) -> SupervisorError {
    SupervisorError::NoContainment
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn non_native_approval_cannot_satisfy_the_supervisor_worker_boundary() {
        let root = std::env::temp_dir().join(format!("lu-ide-e2-{}", std::process::id()));
        let sup = Arc::new(Supervisor::open(root.clone()).unwrap());
        struct Fake;
        impl RestrictedLspProcess for Fake {
            fn write_bounded(&mut self, _: &[u8]) -> io::Result<()> {
                Ok(())
            }
            fn read_bounded(&mut self, _: usize, _: Duration) -> io::Result<Vec<u8>> {
                Ok(Vec::new())
            }
            fn terminate_tree_and_verify(&mut self) -> Result<bool, SupervisorError> {
                Ok(true)
            }
            fn release_after_verified_terminal(&mut self) -> Result<(), SupervisorError> {
                Ok(())
            }
        }
        let p = OsContainmentProof {
            platform: "windows".into(),
            restricted_identity: true,
            filesystem_access_limited: true,
            network_access_limited: true,
            child_contained_before_resume: true,
            process_tree_kill_verified: true,
            memory_quota: true,
            cpu_quota: true,
            pid_quota: true,
            protected_journal_broker: true,
        };
        assert!(LspGateway::attach_native(
            Arc::clone(&sup),
            &format!("job-{}", "a".repeat(32)),
            &"b".repeat(64),
            &p,
            Box::new(Fake),
            "rust-analyzer",
        )
        .is_err());
        let _ = std::fs::remove_file(root);
    }
    #[derive(Default)]
    struct MockState {
        killed: std::sync::atomic::AtomicUsize,
        released: std::sync::atomic::AtomicUsize,
    }
    struct MockProcess {
        state: Arc<MockState>,
        fail_release: bool,
    }
    impl RestrictedLspProcess for MockProcess {
        fn write_bounded(&mut self, _: &[u8]) -> io::Result<()> {
            Ok(())
        }
        fn read_bounded(&mut self, _: usize, _: Duration) -> io::Result<Vec<u8>> {
            Ok(Vec::new())
        }
        fn terminate_tree_and_verify(&mut self) -> Result<bool, SupervisorError> {
            self.state
                .killed
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(true)
        }
        fn release_after_verified_terminal(&mut self) -> Result<(), SupervisorError> {
            if self.fail_release {
                return Err(SupervisorError::NoContainment);
            }
            self.state
                .released
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }
    }
    fn admitted_fixture() -> (Arc<Supervisor>, String, String, std::path::PathBuf) {
        let nonce = format!(
            "{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let journal = std::env::temp_dir().join(format!("lu-e2-receipt-{nonce}.jsonl"));
        let supervisor = Arc::new(Supervisor::open(journal.clone()).unwrap());
        let plan = crate::JobPlan {
            project_id: format!("ide-{}", "a".repeat(32)),
            recipe_id: "lsp_clangd".to_owned(),
            source_head: "b".repeat(40),
            input_snapshot_sha256: "c".repeat(64),
            manifest_sha256: "d".repeat(64),
            toolchain_sha256: "e".repeat(64),
            runtime_limit_ms: 60_000,
            memory_limit_bytes: 128 * 1024 * 1024,
            process_limit: 2,
            log_limit_bytes: 32 * 1024,
            effect_class: "language_analysis_isolated".to_owned(),
        };
        let summary = supervisor.prepare(plan.clone()).unwrap();
        supervisor
            .record_human_approval(
                &summary.job_id,
                &summary.plan_sha256,
                &plan.source_head,
                true,
            )
            .unwrap();
        let proof = OsContainmentProof {
            platform: "windows".into(),
            restricted_identity: true,
            filesystem_access_limited: true,
            network_access_limited: true,
            child_contained_before_resume: true,
            process_tree_kill_verified: true,
            memory_quota: true,
            cpu_quota: true,
            pid_quota: true,
            protected_journal_broker: true,
        };
        supervisor
            .consume_grant_after_attestation(&summary.job_id, &plan, &proof)
            .unwrap();
        supervisor.native_mark_running(&summary.job_id).unwrap();
        (supervisor, summary.job_id, summary.plan_sha256, journal)
    }
    fn proof_fixture() -> OsContainmentProof {
        OsContainmentProof {
            platform: "windows".into(),
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
    #[test]
    fn protected_lsp_normal_close_persists_terminal_receipt_before_restart() {
        let (supervisor, id, sha, journal) = admitted_fixture();
        let evidence = Arc::new(MockState::default());
        let gateway = LspGateway::attach_native(
            Arc::clone(&supervisor),
            &id,
            &sha,
            &proof_fixture(),
            Box::new(MockProcess {
                state: Arc::clone(&evidence),
                fail_release: false,
            }),
            "clangd",
        )
        .unwrap();
        gateway.close().unwrap();
        assert!(evidence.killed.load(std::sync::atomic::Ordering::SeqCst) >= 1);
        assert_eq!(
            evidence.released.load(std::sync::atomic::Ordering::SeqCst),
            1
        );
        drop(supervisor);
        let read = Supervisor::open(journal.clone())
            .unwrap()
            .inspect()
            .unwrap();
        assert_eq!(read.len(), 1);
        assert_eq!(read[0].phase, JobPhase::Completed);
        assert!(!read[0].requires_reconciliation);
        let _ = std::fs::remove_file(journal);
    }
    #[test]
    fn dropped_lsp_actor_is_durably_cancelled_before_os_tree_kill() {
        let (supervisor, id, sha, journal) = admitted_fixture();
        let evidence = Arc::new(MockState::default());
        let gateway = LspGateway::attach_native(
            Arc::clone(&supervisor),
            &id,
            &sha,
            &proof_fixture(),
            Box::new(MockProcess {
                state: Arc::clone(&evidence),
                fail_release: false,
            }),
            "clangd",
        )
        .unwrap();
        drop(gateway);
        assert!(evidence.killed.load(std::sync::atomic::Ordering::SeqCst) >= 1);
        assert_eq!(
            evidence.released.load(std::sync::atomic::Ordering::SeqCst),
            1
        );
        drop(supervisor);
        let read = Supervisor::open(journal.clone())
            .unwrap()
            .inspect()
            .unwrap();
        assert_eq!(read.len(), 1);
        assert_eq!(read[0].phase, JobPhase::Cancelled);
        assert!(!read[0].requires_reconciliation);
        let _ = std::fs::remove_file(journal);
    }
    #[test]
    fn failed_protected_service_release_quarantines_even_when_tree_is_empty() {
        let (supervisor, id, sha, journal) = admitted_fixture();
        let gateway = LspGateway::attach_native(
            Arc::clone(&supervisor),
            &id,
            &sha,
            &proof_fixture(),
            Box::new(MockProcess {
                state: Arc::new(MockState::default()),
                fail_release: true,
            }),
            "clangd",
        )
        .unwrap();
        assert!(gateway.close().is_err());
        drop(supervisor);
        let read = Supervisor::open(journal.clone())
            .unwrap()
            .inspect()
            .unwrap();
        assert_eq!(read.len(), 1);
        assert_eq!(read[0].phase, JobPhase::RecoveryRequired);
        assert!(read[0].requires_reconciliation);
        let _ = std::fs::remove_file(journal);
    }
}
