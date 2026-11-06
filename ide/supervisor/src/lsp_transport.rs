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
use std::{io, sync::Arc, time::{Duration, Instant}};

const MAX_CHUNK: usize = 16 * 1024;
const MAX_PENDING_OUTPUT: usize = MAX_MESSAGE + 256;
const READ_WAIT: Duration = Duration::from_millis(400);
const MAX_SESSION_LIFETIME: Duration = Duration::from_secs(10 * 60);
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
        let job = supervisor.inspect()?.into_iter()
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
        let session = LspSession::new(server_id)
            .map_err(|_| SupervisorError::InvalidPlan)?;
        Ok(Self {
            supervisor, job_id: job_id.to_owned(), session,
            process: Some(process), finished: false,
            opened_at: Instant::now(), io_bytes: 0,
            plan_sha256: expected_plan_sha256.to_owned(),
        })
    }
    fn send(&mut self, frame: &[u8]) -> Result<(), SupervisorError> {
        if frame.len() > MAX_PENDING_OUTPUT || self.opened_at.elapsed() > MAX_SESSION_LIFETIME
            || self.io_bytes.saturating_add(frame.len()) > MAX_TOTAL_IO {
            return self.deny_and_quarantine();
        }
        self.io_bytes += frame.len();
        let sent = self.process.as_mut().ok_or(SupervisorError::Quarantined)?
            .write_bounded(frame);
        if sent.is_err() { return self.deny_and_quarantine(); }
        Ok(())
    }
    pub(crate) fn initialize(&mut self) -> Result<(), SupervisorError> {
        let data = self.session.initialize().map_err(protocol_denied)?;
        self.send(&data)
    }
    pub(crate) fn open(
        &mut self, source: DiagnosticSource, trusted_uri: &str, text: &str,
    ) -> Result<(), SupervisorError> {
        let data = self.session.did_open(source, trusted_uri, text).map_err(protocol_denied)?;
        self.send(&data)
    }
    pub(crate) fn changed(
        &mut self, expected: &DiagnosticSource, next: DiagnosticSource, text: &str,
    ) -> Result<(), SupervisorError> {
        let data = self.session.did_change(expected, next, text).map_err(protocol_denied)?;
        self.send(&data)
    }
    pub(crate) fn query(
        &mut self, kind: &str, source: &DiagnosticSource, position: Position,
    ) -> Result<(), SupervisorError> {
        let data = self.session.request(kind, source, position).map_err(protocol_denied)?;
        self.send(&data)
    }
    pub(crate) fn poll(&mut self) -> Result<Vec<LspEvent>, SupervisorError> {
        if self.opened_at.elapsed() > MAX_SESSION_LIFETIME {
            return self.deny_and_quarantine();
        }
        let read = self.process.as_mut().ok_or(SupervisorError::Quarantined)?
            .read_bounded(MAX_CHUNK, READ_WAIT);
        let frame = match read {
            Ok(frame) => frame, Err(_) => return self.deny_and_quarantine(),
        };
        if frame.len() > MAX_CHUNK ||
            self.io_bytes.saturating_add(frame.len()) > MAX_TOTAL_IO {
            return self.deny_and_quarantine();
        }
        self.io_bytes += frame.len();
        if frame.is_empty() { return Ok(Vec::new()); }
        let (events, replies) = match self.session.receive(&frame) {
            Ok(result) => result,
            Err(_) => return self.deny_and_quarantine(),
        };
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
        let verified = self.process.as_mut().ok_or(SupervisorError::Quarantined)?
            .terminate_tree_and_verify()?;
        if !verified { return self.deny_and_quarantine(); }
        self.supervisor.native_finish(&self.job_id, true, true, true)?;
        self.finished = true; // Receipt is durable; do not append Cancelled.
        self.process.as_mut().ok_or(SupervisorError::Quarantined)?
            .release_after_verified_terminal()?;
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
        &mut self, job_id: &str, plan_sha256: &str,
    ) -> Result<bool, SupervisorError> {
        if job_id != self.job_id || plan_sha256 != self.plan_sha256 {
            return Err(SupervisorError::WrongProjectOrSource);
        }
        self.process.terminate_tree_and_verify()
    }
}
impl Drop for LspGateway {
    fn drop(&mut self) {
        if self.finished { return; }
        self.session.stop_uncertain();
        if let Some(process) = self.process.as_mut() {
            // A routine editor close, host drop or cancellation MUST fsync
            // CancelRequested before it kills an active contained process.
            let mut killer = GatewayTerminator {
                process: process.as_mut(), job_id: &self.job_id,
                plan_sha256: &self.plan_sha256,
            };
            if self.supervisor.cancel_running_with(&self.job_id, &mut killer).is_ok() {
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
        let root = std::env::temp_dir().join(format!(
            "lu-ide-e2-{}", std::process::id()));
        let sup = Arc::new(Supervisor::open(root.clone()).unwrap());
        struct Fake;
        impl RestrictedLspProcess for Fake {
            fn write_bounded(&mut self, _: &[u8]) -> io::Result<()> { Ok(()) }
            fn read_bounded(&mut self, _: usize, _: Duration) -> io::Result<Vec<u8>> {
                Ok(Vec::new())
            }
            fn terminate_tree_and_verify(&mut self) -> Result<bool, SupervisorError> { Ok(true) }
            fn release_after_verified_terminal(&mut self) -> Result<(), SupervisorError> { Ok(()) }
        }
        let p = OsContainmentProof {
            platform: "windows".into(), restricted_identity: true,
            filesystem_access_limited: true, network_access_limited: true,
            child_contained_before_resume: true, process_tree_kill_verified: true,
            memory_quota: true, cpu_quota: true, pid_quota: true,
            protected_journal_broker: true,
        };
        assert!(LspGateway::attach_native(
            Arc::clone(&sup), &format!("job-{}", "a".repeat(32)),
            &"b".repeat(64), &p, Box::new(Fake), "rust-analyzer",
        ).is_err());
        let _ = std::fs::remove_file(root);
    }
}
