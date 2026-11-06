//! Wave E2: supervisor-owned LSP transport bridge.
//!
//! This is NOT an alternate process launcher. An OS-specific D7 supervisor
//! must first start and retain the exact pinned language server *inside*
//! an independently protected Job/cgroup and provide its authenticated pipe
//! handles. The desktop and untrusted project cannot construct a gateway.
//! E2 does not yet implement those OS-specific constructors.
use crate::{JobPhase, OsContainmentProof, Supervisor, SupervisorError};
use lu_ide_workspace::lsp::{DiagnosticSource, MAX_MESSAGE};
use lu_ide_workspace::lsp_session::{LspError, LspEvent, LspPhase, LspSession, Position};
use std::{io, time::Duration};

const MAX_CHUNK: usize = 16 * 1024;
const MAX_PENDING_OUTPUT: usize = MAX_MESSAGE + 256;
const READ_WAIT: Duration = Duration::from_millis(400);

/// A trusted native worker adapter must own both ends of its OS-restricted
/// pipes and the real kernel containment handle. No blanket std::process::Child
/// or WebView-provided fd is accepted here.
pub(crate) trait RestrictedLspProcess: Send {
    fn write_bounded(&mut self, bytes: &[u8]) -> io::Result<()>;
    fn read_bounded(&mut self, limit: usize, timeout: Duration) -> io::Result<Vec<u8>>;
    /// Must terminate the entire kernel Job/cgroup tree and prove it empty.
    fn terminate_tree_and_verify(&mut self) -> Result<bool, SupervisorError>;
}

pub(crate) struct LspGateway<'a> {
    supervisor: &'a Supervisor,
    job_id: String,
    session: LspSession,
    process: Option<Box<dyn RestrictedLspProcess>>,
    finished: bool,
}
impl<'a> LspGateway<'a> {
    /// Only an already-authorized OS worker implementation can call this
    /// internal factory. A struct of bools is not OS proof on its own;
    /// the kernel Job/cgroup handle remains a separate requirement.
    pub(crate) fn attach_native(
        supervisor: &'a Supervisor,
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
        })
    }
    fn send(&mut self, frame: &[u8]) -> Result<(), SupervisorError> {
        if frame.len() > MAX_PENDING_OUTPUT { return Err(SupervisorError::NoContainment); }
        self.process.as_mut().ok_or(SupervisorError::Quarantined)?
            .write_bounded(frame).map_err(|_| SupervisorError::Quarantined)
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
        let frame = self.process.as_mut().ok_or(SupervisorError::Quarantined)?
            .read_bounded(MAX_CHUNK, READ_WAIT).map_err(|_| SupervisorError::Quarantined)?;
        if frame.len() > MAX_CHUNK { return Err(SupervisorError::NoContainment); }
        if frame.is_empty() { return Ok(Vec::new()); }
        let (events, replies) = self.session.receive(&frame).map_err(protocol_denied)?;
        for outbound in replies {
            self.send(&outbound)?;
        }
        Ok(events)
    }
    pub(crate) fn close(mut self) -> Result<(), SupervisorError> {
        if self.session.phase() == LspPhase::Ready {
            // Best-effort graceful shutdown, but kernel process-tree cleanup
            // remains required even when the LSP process acknowledges exit.
            if let Ok(frame) = self.session.shutdown() {
                let _ = self.send(&frame);
            }
        }
        let verified = self.process.as_mut().ok_or(SupervisorError::Quarantined)?
            .terminate_tree_and_verify()?;
        if !verified {
            let _ = self.supervisor.native_quarantine_after_error(&self.job_id);
            return Err(SupervisorError::Quarantined);
        }
        self.finished = true;
        Ok(())
    }
}
impl Drop for LspGateway<'_> {
    fn drop(&mut self) {
        if self.finished { return; }
        if let Some(process) = self.process.as_mut() {
            if !matches!(process.terminate_tree_and_verify(), Ok(true)) {
                let _ = self.supervisor.native_quarantine_after_error(&self.job_id);
            }
        }
        // A process-tree kill alone must not produce a finished receipt;
        // the native supervisor is responsible for terminal journal evidence.
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
        let sup = Supervisor::open(root.clone()).unwrap();
        struct Fake;
        impl RestrictedLspProcess for Fake {
            fn write_bounded(&mut self, _: &[u8]) -> io::Result<()> { Ok(()) }
            fn read_bounded(&mut self, _: usize, _: Duration) -> io::Result<Vec<u8>> {
                Ok(Vec::new())
            }
            fn terminate_tree_and_verify(&mut self) -> Result<bool, SupervisorError> { Ok(true) }
        }
        let p = OsContainmentProof {
            platform: "windows".into(), restricted_identity: true,
            filesystem_access_limited: true, network_access_limited: true,
            child_contained_before_resume: true, process_tree_kill_verified: true,
            memory_quota: true, cpu_quota: true, pid_quota: true,
            protected_journal_broker: true,
        };
        assert!(LspGateway::attach_native(
            &sup, &format!("job-{}", "a".repeat(32)),
            &"b".repeat(64), &p, Box::new(Fake), "rust-analyzer",
        ).is_err());
        let _ = std::fs::remove_file(root);
    }
}
