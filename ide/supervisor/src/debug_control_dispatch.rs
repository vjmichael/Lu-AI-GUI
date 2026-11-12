//! DEV-04.6C — guarded, one-use *native CI* project-debug control.
//! Not a renderer/AI tool; never linked in a default or release Lu build.
//! Real D5 owner-event custody and independently verified post-command
//! stop/PDB must be established BEFORE any successful outcome is recorded.
#![cfg(any(test, all(windows, debug_assertions, feature = "dev046c-ci-control"), all(windows, feature = "fin04-release-owner-evidence")))]
use crate::{
    debug_action_receipts::{op_digest, ConsumedControlClaim},
    debug_command_review::{DebugReviewIntent, StepKind},
};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ControlDenial {
    Unreviewed,
    WrongJob,
    WrongStop,
    WrongSource,
    AlreadyConsumed,
    WrongOperation,
    AckOnly,
    NoIndependentStop,
    Replay,
}
fn hash64(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}
fn proof_sha(s: &str) -> String {
    format!("{:x}", Sha256::digest(s.as_bytes()))
}

/// Only the first-party native D5 protected Windows worker can create this
/// from actual OS Job/RIP, independently decoded PE/PDB, owner-side event
/// and immutable original build+debugger receipts. Test code may fabricate
/// it strictly for hostile state-machine tests.
#[derive(Debug, Clone)]
pub(crate) struct NativeControlStop {
    job_id: String,
    plan_sha256: String,
    build_receipt_sha256: String,
    running_receipt_sha256: String,
    owner_receipt_sha256: String,
    source_snapshot_sha256: String,
    process_id: u32,
    thread_id: u32,
    rip: u64,
    line: u32,
    frame_id: u64,
    owner_sequence: u64,
    has_independent_pdb: bool,
    has_kernel_owner: bool,
    has_durable_receipts: bool,
}
#[cfg(all(windows, debug_assertions, feature = "dev046c-ci-control"))]
pub(crate) fn native_control_stop_from_owner(
    value: &crate::windows_worker::lsp_native::WindowsOwnerEventAccepted,
) -> Result<NativeControlStop, ControlDenial> {
    if !value.lldb_event_loop_owner_attested
        || !value.protected_pe_pdb_verified
        || !value.durable_receipt_verified
        || value.debugger_commands_authorized
        || value.installed_app_accepted
    {
        return Err(ControlDenial::Unreviewed);
    }
    Ok(NativeControlStop {
        job_id: value.debugger_job_id.clone(),
        plan_sha256: value.debugger_plan_sha256.clone(),
        build_receipt_sha256: value.build_terminal_receipt_sha256.clone(),
        running_receipt_sha256: value.debug_running_receipt_sha256.clone(),
        owner_receipt_sha256: value.native_owner_receipt_sha256.clone(),
        source_snapshot_sha256: value.source_snapshot_sha256.clone(),
        process_id: value.debuggee_pid,
        thread_id: value.debuggee_thread_id,
        rip: value.stopped_rip,
        line: value.verified_source_line,
        frame_id: value.verified_frame_id,
        owner_sequence: value.owner_sequence,
        has_independent_pdb: true,
        has_kernel_owner: true,
        has_durable_receipts: true,
    })
}
fn valid_stop(s: &NativeControlStop) -> bool {
    s.job_id.starts_with("job-")
        && s.job_id.len() == 36
        && hash64(&s.plan_sha256)
        && hash64(&s.build_receipt_sha256)
        && hash64(&s.running_receipt_sha256)
        && hash64(&s.owner_receipt_sha256)
        && hash64(&s.source_snapshot_sha256)
        && s.process_id != 0
        && s.thread_id != 0
        && s.rip != 0
        && s.line > 0
        && s.frame_id > 0
        && s.frame_id <= 1_000_000
        && s.owner_sequence > 0
        && s.has_independent_pdb
        && s.has_kernel_owner
        && s.has_durable_receipts
}
fn same_fixed_job(a: &NativeControlStop, b: &NativeControlStop) -> bool {
    a.job_id == b.job_id
        && a.plan_sha256 == b.plan_sha256
        && a.build_receipt_sha256 == b.build_receipt_sha256
        && a.running_receipt_sha256 == b.running_receipt_sha256
        && a.source_snapshot_sha256 == b.source_snapshot_sha256
        && a.process_id == b.process_id
        && a.thread_id == b.thread_id
}

/// Explicitly CI-scoped one-use authority. The slot is moved out once
/// BEFORE sending the bounded native DAP request. It cannot be recreated
/// from a durable action receipt after worker crash or app restart.
pub(crate) struct CiControlSlot {
    current: NativeControlStop,
    claim: Option<ConsumedControlClaim>,
}
#[derive(Debug)]
pub(crate) struct CiStepDispatch {
    claim: ConsumedControlClaim,
    from: NativeControlStop,
    kind: StepKind,
}
#[derive(Debug)]
pub(crate) struct CiBreakpointDispatch {
    claim: ConsumedControlClaim,
    from: NativeControlStop,
}
/// A source-bound read-only VARIABLES request needs its OWN consumed action.
/// It cannot reuse a step/breakpoint claim or a DAP-only frame ID.
#[derive(Debug)]
pub(crate) struct CiVariablesDispatch {
    from: NativeControlStop,
    claim: ConsumedControlClaim,
}
impl CiControlSlot {
    pub(crate) fn admit(
        claim: ConsumedControlClaim,
        current: NativeControlStop,
    ) -> Result<Self, ControlDenial> {
        // D6D permits a *real native OS dialog* receipt in an explicit
        // debug-only Tauri build. This still never grants general executable
        // authority: the same one-shot Job/owner/DAP binding is mandatory.
        let trusted_native_dialog = cfg!(all(
            windows,
            debug_assertions,
            feature = "dev046d-native-dialog"
        ));
        // FIN-04R may compile the independent post-command *verifier* for
        // native review but MUST NOT admit a step/breakpoint/variables slot.
        // The release path lacks a protected independent PDB worker and its
        // own production-grade consent constructor. No CI/dev token, even
        // with matching SHA strings, can cross this boundary.
        if cfg!(all(windows, feature = "fin04-release-owner-evidence"))
            && !cfg!(test)
        {
            return Err(ControlDenial::Unreviewed);
        }
        if (!claim.ci_fixture_only && !trusted_native_dialog)
            || claim.execution_authorized
            || !hash64(&claim.approved_receipt_sha256)
            || !hash64(&claim.consumed_receipt_sha256)
            || claim.approved_receipt_sha256 == claim.consumed_receipt_sha256
            || !hash64(&claim.action_id_sha256)
            || !hash64(&claim.preview_sha256)
            || !hash64(&claim.operation_sha256)
            || claim.operation_sha256 != op_digest(&claim.operation)
            || !valid_stop(&current)
        {
            return Err(ControlDenial::Unreviewed);
        }
        if claim.job_id != current.job_id
            || claim.plan_sha256 != current.plan_sha256
            || claim.owner_event_sha256 != current.owner_receipt_sha256
            || claim.source_snapshot_sha256 != current.source_snapshot_sha256
            || claim.process_id != current.process_id
            || claim.thread_id != current.thread_id
            || claim.stopped_rip != current.rip
            || claim.source_line != current.line
            || claim.owner_sequence != current.owner_sequence
            || claim.relative_source != "src/main.rs"
        {
            return Err(ControlDenial::WrongStop);
        }
        Ok(Self {
            current,
            claim: Some(claim),
        })
    }
    pub(crate) fn take_step(&mut self) -> Result<CiStepDispatch, ControlDenial> {
        let claim = self.claim.take().ok_or(ControlDenial::AlreadyConsumed)?;
        let kind = match &claim.operation {
            DebugReviewIntent::Step { kind, thread_id } if *thread_id == self.current.thread_id => {
                *kind
            }
            _ => return Err(ControlDenial::WrongOperation),
        };
        Ok(CiStepDispatch {
            claim,
            from: self.current.clone(),
            kind,
        })
    }
    pub(crate) fn take_breakpoint(&mut self) -> Result<CiBreakpointDispatch, ControlDenial> {
        let claim = self.claim.take().ok_or(ControlDenial::AlreadyConsumed)?;
        match &claim.operation {
            DebugReviewIntent::Breakpoint { source, line }
                if source == &claim.relative_source && *line == self.current.line =>
            {
                Ok(CiBreakpointDispatch {
                    claim,
                    from: self.current.clone(),
                })
            }
            _ => Err(ControlDenial::WrongOperation),
        }
    }
    pub(crate) fn take_variables(&mut self) -> Result<CiVariablesDispatch, ControlDenial> {
        let claim = self.claim.take().ok_or(ControlDenial::AlreadyConsumed)?;
        match &claim.operation {
            DebugReviewIntent::InspectVariables { frame_id }
                if *frame_id == self.current.frame_id =>
            {
                Ok(CiVariablesDispatch {
                    from: self.current.clone(),
                    claim,
                })
            }
            _ => Err(ControlDenial::WrongOperation),
        }
    }
    pub(crate) fn has_unconsumed_slot(&self) -> bool {
        self.claim.is_some()
    }
}
impl CiVariablesDispatch {
    pub(crate) fn frame_id(&self) -> u64 {
        self.from.frame_id
    }
    pub(crate) fn action_id_sha256(&self) -> &str {
        &self.claim.action_id_sha256
    }
    /// Consumed only. Never an installed read, native OS result, grant
    /// to expand references or permission to evaluate program expressions.
    pub(crate) fn installed_accepted(&self) -> bool {
        false
    }
}
impl CiStepDispatch {
    pub(crate) fn kind(&self) -> StepKind {
        self.kind
    }
    pub(crate) fn thread_id(&self) -> u32 {
        self.from.thread_id
    }
    pub(crate) fn action_id_sha256(&self) -> &str {
        &self.claim.action_id_sha256
    }
}
impl CiBreakpointDispatch {
    pub(crate) fn source_line(&self) -> u32 {
        self.from.line
    }
    pub(crate) fn action_id_sha256(&self) -> &str {
        &self.claim.action_id_sha256
    }
    /// A breakpoint adapter ACK is not native evidence that memory contains
    /// an INT3 or that a protected process ever hit this source line.
    pub(crate) fn independently_installed(&self) -> bool {
        false
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct NativeStepAck {
    pub(crate) correlated_response: bool,
    pub(crate) new_dap_stop: bool,
    pub(crate) thread_id: u32,
}
/// Does NOT grant further control. A real native worker may create it only
/// by joining DAP correlation to a NEW D5 owner-event sequence, independently
/// checked stopped thread/RIP, compiler PDB line and original source seal.
pub(crate) struct VerifiedCiStepResult {
    action_id_sha256: String,
    consumed_sha256: String,
    evidence_sha256: String,
    from_seq: u64,
    to_seq: u64,
}
pub(crate) fn verify_independent_step_result(
    dispatched: &CiStepDispatch,
    after: &NativeControlStop,
    ack: NativeStepAck,
) -> Result<VerifiedCiStepResult, ControlDenial> {
    if !ack.correlated_response || !ack.new_dap_stop || ack.thread_id != dispatched.from.thread_id {
        return Err(ControlDenial::AckOnly);
    }
    if !valid_stop(after)
        || !same_fixed_job(&dispatched.from, after)
        || after.owner_sequence
            != dispatched
                .from
                .owner_sequence
                .checked_add(1)
                .ok_or(ControlDenial::NoIndependentStop)?
        || after.owner_receipt_sha256 == dispatched.from.owner_receipt_sha256
        || after.rip == dispatched.from.rip
    {
        return Err(ControlDenial::NoIndependentStop);
    }
    let fingerprint = format!(
        "lu-ci-step-independent-v1\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}",
        dispatched.claim.action_id_sha256,
        dispatched.claim.consumed_receipt_sha256,
        dispatched.from.job_id,
        dispatched.from.plan_sha256,
        dispatched.from.owner_receipt_sha256,
        after.owner_receipt_sha256,
        dispatched.from.rip,
        after.rip,
        dispatched.from.owner_sequence,
        after.owner_sequence,
        after.line
    );
    Ok(VerifiedCiStepResult {
        action_id_sha256: dispatched.claim.action_id_sha256.clone(),
        consumed_sha256: dispatched.claim.consumed_receipt_sha256.clone(),
        evidence_sha256: proof_sha(&fingerprint),
        from_seq: dispatched.from.owner_sequence,
        to_seq: after.owner_sequence,
    })
}
impl VerifiedCiStepResult {
    pub(crate) fn matches_consumption(&self, action: &str, receipt: &str) -> bool {
        self.action_id_sha256 == action
            && self.consumed_sha256 == receipt
            && hash64(&self.evidence_sha256)
            && self.to_seq == self.from_seq + 1
    }
    pub(crate) fn evidence_sha256(&self) -> &str {
        &self.evidence_sha256
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn stop(seq: u64) -> NativeControlStop {
        NativeControlStop {
            job_id: format!("job-{}", "a".repeat(32)),
            plan_sha256: "b".repeat(64),
            build_receipt_sha256: "c".repeat(64),
            running_receipt_sha256: "d".repeat(64),
            owner_receipt_sha256: format!("{:064x}", seq),
            source_snapshot_sha256: "e".repeat(64),
            process_id: 11,
            thread_id: 22,
            rip: if seq > 1 { 0x140001004 } else { 0x140001000 },
            line: if seq > 1 { 8 } else { 7 },
            frame_id: 9,
            owner_sequence: seq,
            has_independent_pdb: true,
            has_kernel_owner: true,
            has_durable_receipts: true,
        }
    }
    fn claim() -> ConsumedControlClaim {
        let operation = DebugReviewIntent::Step {
            kind: StepKind::Over,
            thread_id: 22,
        };
        ConsumedControlClaim {
            action_id_sha256: "f".repeat(64),
            approved_receipt_sha256: "1".repeat(64),
            consumed_receipt_sha256: "2".repeat(64),
            operation_sha256: op_digest(&operation),
            owner_sequence: 1,
            ci_fixture_only: true,
            execution_authorized: false,
            job_id: format!("job-{}", "a".repeat(32)),
            plan_sha256: "b".repeat(64),
            owner_event_sha256: format!("{:064x}", 1),
            source_snapshot_sha256: "e".repeat(64),
            preview_sha256: "4".repeat(64),
            stopped_rip: 0x140001000,
            process_id: 11,
            thread_id: 22,
            source_line: 7,
            relative_source: "src/main.rs".into(),
            operation,
        }
    }
    fn ack() -> NativeStepAck {
        NativeStepAck {
            correlated_response: true,
            new_dap_stop: true,
            thread_id: 22,
        }
    }
    #[test]
    fn tampered_consumed_operation_digest_cannot_dispatch() {
        let mut altered = claim();
        altered.operation_sha256 = "3".repeat(64);
        assert!(matches!(
            CiControlSlot::admit(altered, stop(1)),
            Err(ControlDenial::Unreviewed)
        ));
    }
    #[test]
    fn same_exact_native_step_claim_moves_only_once() {
        let mut slot = CiControlSlot::admit(claim(), stop(1)).unwrap();
        assert!(slot.has_unconsumed_slot());
        let sent = slot.take_step().unwrap();
        assert_eq!(sent.thread_id(), 22);
        assert!(matches!(
            slot.take_step(),
            Err(ControlDenial::AlreadyConsumed)
        ));
        assert!(!slot.has_unconsumed_slot());
    }
    #[test]
    fn daps_success_and_same_stop_cannot_prove_new_instruction() {
        let mut slot = CiControlSlot::admit(claim(), stop(1)).unwrap();
        let dispatch = slot.take_step().unwrap();
        assert!(matches!(
            verify_independent_step_result(&dispatch, &stop(1), ack(),),
            Err(ControlDenial::NoIndependentStop)
        ));
        let mut b = stop(2);
        b.owner_receipt_sha256 = dispatch.from.owner_receipt_sha256.clone();
        assert!(matches!(
            verify_independent_step_result(&dispatch, &b, ack(),),
            Err(ControlDenial::NoIndependentStop)
        ));
    }
    #[test]
    fn independently_new_kernel_stop_and_pdb_bind_one_consumed_receipt() {
        let mut slot = CiControlSlot::admit(claim(), stop(1)).unwrap();
        let dispatched = slot.take_step().unwrap();
        let result = verify_independent_step_result(&dispatched, &stop(2), ack()).unwrap();
        assert!(result.matches_consumption(
            dispatched.action_id_sha256(),
            &dispatched.claim.consumed_receipt_sha256,
        ));
        assert_eq!(result.evidence_sha256().len(), 64);
    }
    #[test]
    fn new_event_at_original_instruction_does_not_verify_forward_step() {
        let mut slot = CiControlSlot::admit(claim(), stop(1)).unwrap();
        let sent = slot.take_step().unwrap();
        let mut next = stop(2);
        next.rip = sent.from.rip;
        assert!(matches!(
            verify_independent_step_result(&sent, &next, ack()),
            Err(ControlDenial::NoIndependentStop)
        ));
    }
    #[test]
    fn foreign_job_thread_source_or_missing_kernel_evidence_denied() {
        let mut wrong = claim();
        wrong.thread_id = 99;
        assert!(matches!(
            CiControlSlot::admit(wrong, stop(1)),
            Err(ControlDenial::WrongStop)
        ));
        let mut slot = CiControlSlot::admit(claim(), stop(1)).unwrap();
        let sent = slot.take_step().unwrap();
        let mut changed = stop(2);
        changed.plan_sha256 = "9".repeat(64);
        assert!(matches!(
            verify_independent_step_result(&sent, &changed, ack(),),
            Err(ControlDenial::NoIndependentStop)
        ));
        let mut changed = stop(2);
        changed.has_independent_pdb = false;
        assert!(verify_independent_step_result(&sent, &changed, ack()).is_err());
    }
    #[test]
    fn no_reply_or_wrong_thread_cannot_be_accepted() {
        let mut slot = CiControlSlot::admit(claim(), stop(1)).unwrap();
        let sent = slot.take_step().unwrap();
        assert!(verify_independent_step_result(
            &sent,
            &stop(2),
            NativeStepAck {
                correlated_response: true,
                new_dap_stop: false,
                thread_id: 22
            }
        )
        .is_err());
        assert!(verify_independent_step_result(
            &sent,
            &stop(2),
            NativeStepAck {
                correlated_response: true,
                new_dap_stop: true,
                thread_id: 23
            }
        )
        .is_err());
    }
    #[test]
    fn g04_variable_scope_requires_original_native_frame_and_its_own_one_use_claim() {
        let mut exact = claim();
        exact.operation = DebugReviewIntent::InspectVariables { frame_id: 9 };
        exact.operation_sha256 = op_digest(&exact.operation);
        let mut slot = CiControlSlot::admit(exact, stop(1)).unwrap();
        let read = slot.take_variables().unwrap();
        assert_eq!(read.frame_id(), 9);
        assert_eq!(read.action_id_sha256().len(), 64);
        assert!(!read.installed_accepted());
        assert!(matches!(
            slot.take_variables(),
            Err(ControlDenial::AlreadyConsumed)
        ));
        let mut wrong = claim();
        wrong.operation = DebugReviewIntent::InspectVariables { frame_id: 10 };
        wrong.operation_sha256 = op_digest(&wrong.operation);
        let mut slot = CiControlSlot::admit(wrong, stop(1)).unwrap();
        assert!(matches!(
            slot.take_variables(),
            Err(ControlDenial::WrongOperation)
        ));
        assert!(!slot.has_unconsumed_slot());
    }
    #[test]
    fn breakpoint_adapter_approval_is_not_independent_memory_evidence() {
        let mut c = claim();
        c.operation = DebugReviewIntent::Breakpoint {
            source: "src/main.rs".into(),
            line: 7,
        };
        c.operation_sha256 = op_digest(&c.operation);
        let mut slot = CiControlSlot::admit(c, stop(1)).unwrap();
        let bp = slot.take_breakpoint().unwrap();
        assert_eq!(bp.source_line(), 7);
        assert!(!bp.independently_installed());
        assert!(matches!(
            slot.take_breakpoint(),
            Err(ControlDenial::AlreadyConsumed)
        ));
    }
}
