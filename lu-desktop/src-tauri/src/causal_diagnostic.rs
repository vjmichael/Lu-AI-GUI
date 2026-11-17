//! CP-01E: per-invocation host observations for the existing DX v2 protocol.
//! This is an adapter, not an authorization or recovery journal. No content,
//! paths, conversation IDs, model output, or exception strings are collected.
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub(crate) enum Checkpoint {
    NativeReceived = 4,
    SessionReady = 19,
    UserSaved = 2,
    WorkerEntered = 7,
    SavedAttested = 5,
    ModelEntered = 8,
    ModelReturned = 9,
    AssistantSaved = 12,
    ReplyObserved = 10,
}

const ORDER: &[Checkpoint] = &[
    Checkpoint::NativeReceived, Checkpoint::SessionReady, Checkpoint::UserSaved,
    Checkpoint::WorkerEntered, Checkpoint::SavedAttested, Checkpoint::ModelEntered,
    Checkpoint::ModelReturned, Checkpoint::AssistantSaved, Checkpoint::ReplyObserved,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Observation {
    pub stage: i32,
    pub outcome: i32,
    pub failure_code: i32,
    pub elapsed_ms: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct CausalProgress {
    trace_ref: String,
    started: Instant,
    observations: Vec<Observation>,
    dropped: u32,
    failed: bool,
}

impl CausalProgress {
    pub fn new(trace_ref: String, started: Instant) -> Self {
        let mut trace = Self {
            trace_ref, started, observations: Vec::with_capacity(ORDER.len()),
            dropped: 0, failed: false,
        };
        // The trusted native wrapper minted the reference at its entry.
        trace.observations.push(Observation {
            stage: Checkpoint::NativeReceived as i32, outcome: 1,
            failure_code: 0, elapsed_ms: 0,
        });
        trace
    }

    pub fn trace_ref(&self) -> &str { &self.trace_ref }
    pub fn observations(&self) -> &[Observation] { &self.observations }
    pub fn dropped(&self) -> u32 { self.dropped }

    fn append(&mut self, stage: i32, outcome: i32, failure_code: i32) {
        let elapsed_ms = self.started.elapsed().as_millis().min(u64::MAX as u128) as u64;
        self.observations.push(Observation { stage, outcome, failure_code, elapsed_ms });
    }

    pub fn observe(&mut self, stage: Checkpoint) -> bool {
        // A duplicate renderer report is idempotent. A report before the
        // exact original host commit is not evidence and cannot be promoted.
        if stage == Checkpoint::ReplyObserved && !self.failed &&
            self.observations.last().is_some_and(|e| e.stage == stage as i32) {
            return true;
        }
        if self.failed || ORDER.get(self.observations.len()) != Some(&stage) {
            self.dropped = self.dropped.saturating_add(1);
            return false;
        }
        let outcome = if matches!(stage, Checkpoint::WorkerEntered | Checkpoint::ModelEntered) { 1 } else { 2 };
        self.append(stage as i32, outcome, 0);
        true
    }

    /// Only call at an actual error return from the original model dispatch.
    /// The cause remains unknown; this is not proof a worker exited.
    pub fn model_failed(&mut self) {
        if self.failed || ORDER.get(self.observations.len()) != Some(&Checkpoint::ModelReturned) {
            self.dropped = self.dropped.saturating_add(1);
            return;
        }
        self.append(Checkpoint::ModelReturned as i32, 3, 1);
        self.failed = true;
    }

    /// A canonical refusal proves that the original nonce was fenced, but
    /// not which earlier native admission check failed.
    pub fn refused(&mut self) {
        if self.failed { return; }
        self.append(20, 3, 1);
        self.failed = true;
    }

    /// Generic failure/cancellation with no independently observed cause.
    /// Mark evidence incomplete; never infer worker termination or rollback.
    pub fn finish_incomplete(&mut self, cancelled: bool) {
        if self.failed { return; }
        self.dropped = self.dropped.saturating_add(1);
        self.append(21, if cancelled { 4 } else { 3 }, if cancelled { 9 } else { 1 });
        self.failed = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn progress() -> CausalProgress {
        CausalProgress::new("0123456789abcdef0123456789abcdef".into(), Instant::now())
    }
    fn advance(t: &mut CausalProgress, stop: usize) {
        for stage in ORDER.iter().take(stop).skip(1) { assert!(t.observe(*stage)); }
    }
    #[test]
    fn host_completion_is_not_renderer_completion() {
        let mut t = progress(); advance(&mut t, 8);
        assert_eq!(t.observations().last().unwrap().stage, 12);
        assert!(t.observe(Checkpoint::ReplyObserved));
        assert_eq!(t.observations().len(), 9);
        assert!(t.observe(Checkpoint::ReplyObserved));
        assert_eq!(t.observations().len(), 9);
        assert_eq!(t.dropped(), 0);
    }
    #[test]
    fn premature_or_out_of_order_reply_cannot_complete_trace() {
        let mut t = progress();
        assert!(!t.observe(Checkpoint::ReplyObserved));
        assert_eq!(t.observations().len(), 1);
        advance(&mut t, 8);
        assert_eq!(t.dropped(), 1);
    }
    #[test]
    fn model_failure_preserves_exact_boundary_without_worker_exit() {
        let mut t = progress(); advance(&mut t, 6); t.model_failed();
        assert_eq!(t.observations().last().unwrap().stage, 9);
        assert_eq!(t.observations().last().unwrap().outcome, 3);
        assert!(!t.observe(Checkpoint::AssistantSaved));
        assert_eq!(t.dropped(), 1);
        assert!(!t.observations().iter().any(|e| e.stage == 17));
    }
    #[test]
    fn refusal_and_cancellation_are_distinct_and_bounded() {
        let mut t = progress(); t.refused();
        let mut c = progress(); c.finish_incomplete(true);
        assert_eq!(t.observations().last().unwrap().stage, 20);
        assert_eq!(c.observations().last().unwrap().failure_code, 9);
        assert_eq!(c.dropped(), 1);
        for _ in 0..1000 { c.finish_incomplete(true); }
        assert_eq!(c.observations().len(), 2);
    }
    #[test]
    fn independent_invocations_do_not_merge_or_share_checkpoint_state() {
        let mut a = progress(); advance(&mut a, 5);
        let b = CausalProgress::new("fedcba9876543210fedcba9876543210".into(), Instant::now());
        assert_ne!(a.trace_ref(), b.trace_ref());
        assert_eq!(b.observations().len(), 1);
        assert!(a.observations().windows(2).all(|w| w[0].elapsed_ms <= w[1].elapsed_ms));
    }
}
