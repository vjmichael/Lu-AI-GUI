//! DEV-04.6C — guarded, one-use *native CI* project-debug control.
//! Not a renderer/AI tool; never linked in a default or release Lu build.
//! Real D5 owner-event custody and independently verified post-command
//! stop/PDB must be established BEFORE any successful outcome is recorded.
#![cfg(any(
    test,
    all(windows, debug_assertions, feature = "dev046c-ci-control"),
    all(windows, feature = "fin04-release-owner-evidence")
))]
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
        if cfg!(all(windows, feature = "fin04-release-owner-evidence")) && !cfg!(test) {
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

/// FIN-04R-1: a breakpoint adapter ACK, even 'verified=true', does
/// NOT prove executable breakpoint installation. Only a NEW first-party
/// Job-owned kernel exception event and independently decoded PE/PDB
/// source line can prove that this exact source location was HIT.
/// This verifier is evidence-only; the release action-slot admission
/// remains hard-denied until genuine OS and installed acceptance.
pub(crate) struct VerifiedCiBreakpointHit {
    action_id_sha256: String,
    consumed_sha256: String,
    receipt_sha256: String,
    previous_owner_sequence: u64,
    current_owner_sequence: u64,
}
pub(crate) fn verify_independent_breakpoint_hit(
    dispatched: &CiBreakpointDispatch,
    after: &NativeControlStop,
) -> Result<VerifiedCiBreakpointHit, ControlDenial> {
    if !valid_stop(after)
        || !same_fixed_job(&dispatched.from, after)
        || after.owner_sequence
            != dispatched
                .from
                .owner_sequence
                .checked_add(1)
                .ok_or(ControlDenial::NoIndependentStop)?
        || after.owner_receipt_sha256 == dispatched.from.owner_receipt_sha256
        || after.line != dispatched.from.line
        || after.source_snapshot_sha256 != dispatched.from.source_snapshot_sha256
    {
        return Err(ControlDenial::NoIndependentStop);
    }
    let fingerprint = format!(
        "lu-ci-breakpoint-hit-native-v1\\n{}\\n{}\\n{}\\n{}\\n{}\\n{}\\n{}\\n{}\\n{}",
        dispatched.claim.action_id_sha256,
        dispatched.claim.consumed_receipt_sha256,
        dispatched.from.job_id,
        dispatched.from.plan_sha256,
        dispatched.from.owner_receipt_sha256,
        after.owner_receipt_sha256,
        dispatched.from.owner_sequence,
        after.owner_sequence,
        after.rip,
    );
    Ok(VerifiedCiBreakpointHit {
        action_id_sha256: dispatched.claim.action_id_sha256.clone(),
        consumed_sha256: dispatched.claim.consumed_receipt_sha256.clone(),
        receipt_sha256: proof_sha(&fingerprint),
        previous_owner_sequence: dispatched.from.owner_sequence,
        current_owner_sequence: after.owner_sequence,
    })
}
impl VerifiedCiBreakpointHit {
    /// True only for a newer OS-owned exception at the reviewed line, not
    /// installation; no positive release claim can be minted from DAP ACK.
    pub(crate) fn matches_consumption(&self, action: &str, consumed: &str) -> bool {
        self.action_id_sha256 == action
            && self.consumed_sha256 == consumed
            && hash64(&self.receipt_sha256)
            && self.current_owner_sequence == self.previous_owner_sequence + 1
    }
    pub(crate) fn evidence_sha256(&self) -> &str {
        &self.receipt_sha256
    }
}
/// A bounded slice of the authenticated DebuggerThread sideband. Each row
/// represents a received exception whose exact matching ContinueDebugEvent
/// succeeded (or the final, still-held exception). No DAP event may supply it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct StepOwnerRecord {
    pub(crate) sequence: u64,
    pub(crate) pid: u32,
    pub(crate) tid: u32,
    pub(crate) owner_tid: u32,
    pub(crate) exception_code: u32,
    pub(crate) first_chance: u32,
    pub(crate) rip: u64,
    pub(crate) address: u64,
    pub(crate) continued: bool,
}

pub(crate) const MAX_STEP_OWNER_TRANSITIONS: usize = 32;

/// Private proof of every OS exception/continue between the two held stops.
/// A source-level step can require several internal single-step exceptions;
/// merely comparing increasing sequence numbers is NOT a substitute for this.
pub(crate) struct VerifiedStepOwnerAdvance {
    from: StepOwnerRecord,
    to: StepOwnerRecord,
    digest: String,
    transcript: String,
}
impl VerifiedStepOwnerAdvance {
    pub(crate) fn verify(records: &[StepOwnerRecord]) -> Result<Self, ControlDenial> {
        if records.len() < 2 || records.len() > MAX_STEP_OWNER_TRANSITIONS + 1 {
            return Err(ControlDenial::NoIndependentStop);
        }
        let from = &records[0];
        let to = records.last().unwrap();
        let mut fingerprint = String::from("lu-ci-step-owner-chain-v1\n");
        for (i, event) in records.iter().enumerate() {
            if from.sequence == 0
                || from.sequence.checked_add(i as u64) != Some(event.sequence)
                || event.pid == 0
                || event.pid != from.pid
                || event.tid == 0
                || event.tid != from.tid
                || event.owner_tid == 0
                || event.owner_tid != from.owner_tid
                || !matches!(event.exception_code, 0x8000_0003 | 0x8000_0004)
                || event.first_chance != 1
                || event.rip == 0
                || event.address == 0
                || event.continued != (i + 1 < records.len())
            {
                return Err(ControlDenial::NoIndependentStop);
            }
            fingerprint.push_str(&format!(
                "{}:{}:{}:{}:{}:{}:{}:{}:{}\n",
                event.sequence,
                event.pid,
                event.tid,
                event.owner_tid,
                event.exception_code,
                event.first_chance,
                event.rip,
                event.address,
                event.continued
            ));
        }
        Ok(Self {
            from: from.clone(),
            to: to.clone(),
            digest: proof_sha(&fingerprint),
            transcript: fingerprint,
        })
    }
    fn matches(&self, from: &NativeControlStop, to: &NativeControlStop) -> bool {
        fn rip_matches(record: &StepOwnerRecord, rip: u64) -> bool {
            record.rip == rip
                || (record.exception_code == 0x8000_0003
                    && record.first_chance == 1
                    && record.address == rip
                    && rip.checked_add(1) == Some(record.rip))
        }
        self.from.sequence == from.owner_sequence
            && self.to.sequence == to.owner_sequence
            && self.from.pid == from.process_id
            && self.to.pid == to.process_id
            && self.from.tid == from.thread_id
            && self.to.tid == to.thread_id
            && rip_matches(&self.from, from.rip)
            && rip_matches(&self.to, to.rip)
            && hash64(&self.digest)
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
    owner_chain_sha256: String,
    evidence: String,
}
pub(crate) fn verify_independent_step_result(
    dispatched: &CiStepDispatch,
    after: &NativeControlStop,
    ack: NativeStepAck,
    owner_chain: &VerifiedStepOwnerAdvance,
) -> Result<VerifiedCiStepResult, ControlDenial> {
    if !ack.correlated_response || !ack.new_dap_stop || ack.thread_id != dispatched.from.thread_id {
        return Err(ControlDenial::AckOnly);
    }
    if !valid_stop(after)
        || !same_fixed_job(&dispatched.from, after)
        || !owner_chain.matches(&dispatched.from, after)
        || after.owner_receipt_sha256 == dispatched.from.owner_receipt_sha256
        || after.rip == dispatched.from.rip
    {
        return Err(ControlDenial::NoIndependentStop);
    }
    let fingerprint = format!(
        "lu-ci-step-independent-v2\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}",
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
        after.line,
        owner_chain.digest,
        owner_chain.transcript
    );
    Ok(VerifiedCiStepResult {
        action_id_sha256: dispatched.claim.action_id_sha256.clone(),
        consumed_sha256: dispatched.claim.consumed_receipt_sha256.clone(),
        evidence_sha256: proof_sha(&fingerprint),
        from_seq: dispatched.from.owner_sequence,
        to_seq: after.owner_sequence,
        owner_chain_sha256: owner_chain.digest.clone(),
        evidence: fingerprint,
    })
}
impl VerifiedCiStepResult {
    pub(crate) fn matches_consumption(&self, action: &str, receipt: &str) -> bool {
        self.action_id_sha256 == action
            && self.consumed_sha256 == receipt
            && hash64(&self.evidence_sha256)
            && self.to_seq > self.from_seq
            && self.to_seq - self.from_seq <= MAX_STEP_OWNER_TRANSITIONS as u64
            && hash64(&self.owner_chain_sha256)
            && proof_sha(&self.evidence) == self.evidence_sha256
    }
    pub(crate) fn evidence_bytes(&self) -> &[u8] {
        self.evidence.as_bytes()
    }
    pub(crate) fn evidence_sha256(&self) -> &str {
        &self.evidence_sha256
    }
}

// Synthetic stop evidence is available only to portable unit tests; never
// compiled into a native worker or used as Windows acceptance evidence.
#[cfg(test)]
pub(crate) fn test_step_result_from_claim(claim: ConsumedControlClaim) -> VerifiedCiStepResult {
    let from = NativeControlStop {
        job_id: claim.job_id.clone(),
        plan_sha256: claim.plan_sha256.clone(),
        build_receipt_sha256: "f".repeat(64),
        running_receipt_sha256: "1".repeat(64),
        owner_receipt_sha256: claim.owner_event_sha256.clone(),
        source_snapshot_sha256: claim.source_snapshot_sha256.clone(),
        process_id: claim.process_id,
        thread_id: claim.thread_id,
        rip: claim.stopped_rip,
        line: claim.source_line,
        frame_id: 9,
        owner_sequence: claim.owner_sequence,
        has_independent_pdb: true,
        has_kernel_owner: true,
        has_durable_receipts: true,
    };
    let mut to = from.clone();
    to.owner_sequence += 2;
    to.rip += 4;
    to.line += 1;
    to.owner_receipt_sha256 = "d".repeat(64);
    let row = |sequence, rip, continued| StepOwnerRecord {
        sequence,
        pid: from.process_id,
        tid: from.thread_id,
        owner_tid: 99,
        exception_code: 0x8000_0004,
        first_chance: 1,
        rip,
        address: rip,
        continued,
    };
    let chain = VerifiedStepOwnerAdvance::verify(&[
        row(from.owner_sequence, from.rip, true),
        row(from.owner_sequence + 1, from.rip + 2, true),
        row(to.owner_sequence, to.rip, false),
    ])
    .unwrap();
    let ack = NativeStepAck {
        correlated_response: true,
        new_dap_stop: true,
        thread_id: from.thread_id,
    };
    let sent = CiControlSlot::admit(claim, from)
        .unwrap()
        .take_step()
        .unwrap();
    verify_independent_step_result(&sent, &to, ack, &chain).unwrap()
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
    fn owner_record(seq: u64, continued: bool) -> StepOwnerRecord {
        let stop = stop(seq);
        StepOwnerRecord {
            sequence: seq,
            pid: stop.process_id,
            tid: stop.thread_id,
            owner_tid: 99,
            exception_code: 0x8000_0004,
            first_chance: 1,
            rip: stop.rip,
            address: stop.rip,
            continued,
        }
    }
    fn tested_chain() -> VerifiedStepOwnerAdvance {
        VerifiedStepOwnerAdvance::verify(&[owner_record(1, true), owner_record(2, false)]).unwrap()
    }
    #[test]
    fn authentic_multi_exception_step_binds_every_continue_and_final_stop() {
        let mut slot = CiControlSlot::admit(claim(), stop(1)).unwrap();
        let sent = slot.take_step().unwrap();
        let mut middle = owner_record(2, true);
        middle.rip += 2;
        middle.address = middle.rip;
        let rows = [owner_record(1, true), middle, owner_record(3, false)];
        let chain = VerifiedStepOwnerAdvance::verify(&rows).unwrap();
        let proof = verify_independent_step_result(&sent, &stop(3), ack(), &chain).unwrap();
        assert!(
            proof.matches_consumption(sent.action_id_sha256(), &sent.claim.consumed_receipt_sha256)
        );
        assert!(verify_independent_step_result(&sent, &stop(3), ack(), &tested_chain()).is_err());
        assert!(verify_independent_step_result(&sent, &stop(2), ack(), &chain).is_err());
        let mut bad = rows.clone();
        bad[1].sequence = 4;
        assert!(VerifiedStepOwnerAdvance::verify(&bad).is_err());
        let mut bad = rows.clone();
        bad[1].continued = false;
        assert!(VerifiedStepOwnerAdvance::verify(&bad).is_err());
        let mut bad = rows.clone();
        bad[1].tid += 1;
        assert!(VerifiedStepOwnerAdvance::verify(&bad).is_err());
        let mut bad = rows.clone();
        bad[1].pid += 1;
        assert!(VerifiedStepOwnerAdvance::verify(&bad).is_err());
        let mut bad = rows.clone();
        bad[1].owner_tid += 1;
        assert!(VerifiedStepOwnerAdvance::verify(&bad).is_err());
        let mut bad = rows.clone();
        bad[1].exception_code = 0xc0000005;
        assert!(VerifiedStepOwnerAdvance::verify(&bad).is_err());
        let mut bad = rows.clone();
        bad[2].continued = true;
        assert!(VerifiedStepOwnerAdvance::verify(&bad).is_err());
        let mut bad = rows.clone();
        bad[2].first_chance = 0;
        assert!(VerifiedStepOwnerAdvance::verify(&bad).is_err());
        let mut bad = rows.clone();
        bad[2].rip += 1;
        let mismatched = VerifiedStepOwnerAdvance::verify(&bad).unwrap();
        assert!(verify_independent_step_result(&sent, &stop(3), ack(), &mismatched).is_err());
        let excessive = (1..=34)
            .map(|seq| owner_record(seq, seq != 34))
            .collect::<Vec<_>>();
        assert!(VerifiedStepOwnerAdvance::verify(&excessive).is_err());
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
            verify_independent_step_result(&dispatch, &stop(1), ack(), &tested_chain(),),
            Err(ControlDenial::NoIndependentStop)
        ));
        let mut b = stop(2);
        b.owner_receipt_sha256 = dispatch.from.owner_receipt_sha256.clone();
        assert!(matches!(
            verify_independent_step_result(&dispatch, &b, ack(), &tested_chain(),),
            Err(ControlDenial::NoIndependentStop)
        ));
    }
    #[test]
    fn independently_new_kernel_stop_and_pdb_bind_one_consumed_receipt() {
        let mut slot = CiControlSlot::admit(claim(), stop(1)).unwrap();
        let dispatched = slot.take_step().unwrap();
        let result =
            verify_independent_step_result(&dispatched, &stop(2), ack(), &tested_chain()).unwrap();
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
            verify_independent_step_result(&sent, &next, ack(), &tested_chain()),
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
            verify_independent_step_result(&sent, &changed, ack(), &tested_chain(),),
            Err(ControlDenial::NoIndependentStop)
        ));
        let mut changed = stop(2);
        changed.has_independent_pdb = false;
        assert!(verify_independent_step_result(&sent, &changed, ack(), &tested_chain()).is_err());
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
            },
            &tested_chain()
        )
        .is_err());
        assert!(verify_independent_step_result(
            &sent,
            &stop(2),
            NativeStepAck {
                correlated_response: true,
                new_dap_stop: true,
                thread_id: 23
            },
            &tested_chain()
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
    fn new_kernel_owned_pe_pdb_hit_is_distinct_from_dap_breakpoint_ack() {
        let mut c = claim();
        c.operation = DebugReviewIntent::Breakpoint {
            source: "src/main.rs".into(),
            line: 7,
        };
        c.operation_sha256 = op_digest(&c.operation);
        let mut slot = CiControlSlot::admit(c, stop(1)).unwrap();
        let sent = slot.take_breakpoint().unwrap();
        assert!(!sent.independently_installed());
        assert!(verify_independent_breakpoint_hit(&sent, &stop(1)).is_err());
        let hit = verify_independent_breakpoint_hit(&sent, &stop(2));
        assert!(
            hit.is_err(),
            "a new event at a different PDB line is not the reviewed hit"
        );
        let mut exact = stop(2);
        exact.line = 7;
        let verified = verify_independent_breakpoint_hit(&sent, &exact).unwrap();
        assert!(verified
            .matches_consumption(sent.action_id_sha256(), &sent.claim.consumed_receipt_sha256));
        assert_eq!(verified.evidence_sha256().len(), 64);
        assert!(
            !sent.independently_installed(),
            "hit evidence is not independently verified installation"
        );
        exact.owner_receipt_sha256 = sent.from.owner_receipt_sha256.clone();
        assert!(verify_independent_breakpoint_hit(&sent, &exact).is_err());
        let mut wrong = stop(2);
        wrong.line = 7;
        wrong.has_independent_pdb = false;
        assert!(verify_independent_breakpoint_hit(&sent, &wrong).is_err());
    }
    #[test]
    fn breakpoint_result_denies_changed_plan_source_thread_and_owner() {
        let mut c = claim();
        c.operation = DebugReviewIntent::Breakpoint {
            source: "src/main.rs".into(),
            line: 7,
        };
        c.operation_sha256 = op_digest(&c.operation);
        let sent = CiControlSlot::admit(c, stop(1))
            .unwrap()
            .take_breakpoint()
            .unwrap();
        let mut after = stop(2);
        after.line = 7;
        after.plan_sha256 = "9".repeat(64);
        assert!(verify_independent_breakpoint_hit(&sent, &after).is_err());
        let mut after = stop(2);
        after.line = 7;
        after.thread_id = 23;
        assert!(verify_independent_breakpoint_hit(&sent, &after).is_err());
        let mut after = stop(2);
        after.line = 7;
        after.source_snapshot_sha256 = "9".repeat(64);
        assert!(verify_independent_breakpoint_hit(&sent, &after).is_err());
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
