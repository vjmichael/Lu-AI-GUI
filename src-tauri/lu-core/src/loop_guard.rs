//! TOOLS-08 — progress-aware, host-owned prevention of stalled native-tool loops.
//!
//! A useful task may make hundreds of individually authorized file edits;
//! a tool name, target path or raw call count is not evidence of looping.
//! Examine the *result of each admitted call* before judging later calls.
//!
//! Read results: new semantic content is progress; repeating the same answer
//! is not. Writes: only committed, typed before/after state witnesses or a
//! verified create-new receipt are proof of mutation. An "ok" flag alone,
//! a new call ID, timestamp or changed model arguments are NOT proof.
//! Unverified writes require additional host evidence for long workflows.
//! Block/Kill never reaches the effect journal or native registry.

use std::collections::{BTreeSet, VecDeque};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::contracts::ToolEffect;

pub const DEFAULT_LOOP_WINDOW: usize = 8;
pub const DEFAULT_STALLED_WARNING: u32 = 3;
pub const DEFAULT_STALLED_BLOCK: u32 = 8;
pub const DEFAULT_KILL_BLOCKS: u32 = 3;
const MAX_SEEN_RESULTS: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopReason {
    NoProgress,
    RepeatedFailure,
    OutputCycle,
    UnverifiedEffect,
}
impl LoopReason {
    pub const fn message(self) -> &'static str {
        match self {
            Self::NoProgress =>
                "Recent tool calls have not established new information or state. Change approach.",
            Self::RepeatedFailure =>
                "The same failing operation was repeated without progress. Change approach.",
            Self::OutputCycle =>
                "Tool responses are cycling without new information. Change approach.",
            Self::UnverifiedEffect =>
                "Further actions need an independently verified state-change receipt.",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopVerdict {
    Allow,
    Warn(LoopReason),
    Block(LoopReason),
    Kill(LoopReason),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressEvidence {
    /// Bounded, verified native result supplied genuinely new information.
    NewInformation,
    /// Host-applied state change evidenced by a typed before/after receipt.
    StateChanged,
    /// Execution returned the same observation, a failure, or a no-op.
    NoProgress,
    /// Tool claimed success but omitted a strong change witness.
    Unverified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoopPolicy {
    pub window_size: usize,
    pub warn_after_stalled: u32,
    pub block_after_stalled: u32,
    pub kill_after_blocks: u32,
}
impl Default for LoopPolicy {
    fn default() -> Self {
        Self {
            window_size: DEFAULT_LOOP_WINDOW,
            warn_after_stalled: DEFAULT_STALLED_WARNING,
            block_after_stalled: DEFAULT_STALLED_BLOCK,
            kill_after_blocks: DEFAULT_KILL_BLOCKS,
        }
    }
}
impl LoopPolicy {
    pub fn validate(self) -> Result<Self, &'static str> {
        if !(4..=32).contains(&self.window_size)
            || self.warn_after_stalled == 0
            || self.block_after_stalled < self.warn_after_stalled + 1
            || self.block_after_stalled > 128
            || self.kill_after_blocks == 0
            || self.kill_after_blocks > 10
        {
            return Err("invalid host progress-guard policy");
        }
        Ok(self)
    }
}
#[derive(Clone)]
struct ObservedCall {
    tool: String,
    args_hash: [u8; 32],
    progress: ProgressEvidence,
}
pub struct LoopGuard {
    policy: LoopPolicy,
    recent: VecDeque<ObservedCall>,
    seen: VecDeque<[u8; 32]>,
    attempts: u64,
    no_progress: u32,
    consecutive_blocks: u32,
    recovery_attempted: bool,
    blocked_last: bool,
}

impl Default for LoopGuard {
    fn default() -> Self {
        Self::new(LoopPolicy::default()).expect("built-in policy is valid")
    }
}
impl LoopGuard {
    pub fn new(policy: LoopPolicy) -> Result<Self, &'static str> {
        Ok(Self {
            policy: policy.validate()?,
            recent: VecDeque::new(),
            seen: VecDeque::new(),
            attempts: 0,
            no_progress: 0,
            consecutive_blocks: 0,
            recovery_attempted: false,
            blocked_last: false,
        })
    }
    pub fn attempted(&self) -> u64 { self.attempts }
    pub fn no_progress_streak(&self) -> u32 { self.no_progress }
    pub fn consecutive_blocks(&self) -> u32 { self.consecutive_blocks }

    /// Called AFTER model JSON and typed codec validation and BEFORE
    /// ConversationAuthority/durable native dispatch. It sees no effects.
    /// The next validated result MUST be observed (or turn halted) before
    /// another model response is admitted by this sequential native turn.
    pub fn check(&mut self, tool: &str, args: &Map<String, Value>) -> LoopVerdict {
        self.attempts = self.attempts.saturating_add(1);
        let args_hash = digest(&serde_json::to_vec(args)
            .expect("validated JSON map serializes"));
        let identical_stalled = self.recent.iter()
            .filter(|r| r.tool == tool && r.args_hash == args_hash
                && r.progress == ProgressEvidence::NoProgress)
            .count();
        let recent_all_stalled = self.recent.iter().rev().take(4)
            .all(|r| r.progress == ProgressEvidence::NoProgress)
            && self.recent.len() >= 4;
        let pingpong = self.recent.len() >= 4 && recent_all_stalled && {
            let n = self.recent.len();
            let a = &self.recent[n-4].tool;
            let b = &self.recent[n-3].tool;
            a != b && a == &self.recent[n-2].tool
                && b == &self.recent[n-1].tool && tool == a
        };
        // If the stalled threshold is reached, give one *different* call
        // a recovery chance. It must demonstrate progress or the next tool
        // proposal is blocked. Repeating the stalled action never qualifies.
        let is_new_approach = !self.recent.iter().any(|r|
            r.tool == tool && r.args_hash == args_hash);
        let exhausted = self.no_progress >= self.policy.block_after_stalled
            && (!is_new_approach || self.recovery_attempted);
        let exact_failed = identical_stalled >= 2 && self.no_progress >= 2;
        let reason = if exhausted { Some(LoopReason::NoProgress) }
            else if exact_failed { Some(LoopReason::RepeatedFailure) }
            else if pingpong { Some(LoopReason::OutputCycle) }
            else { None };
        if let Some(reason) = reason {
            self.blocked_last = true;
            self.consecutive_blocks = self.consecutive_blocks.saturating_add(1);
            if self.consecutive_blocks >= self.policy.kill_after_blocks {
                return LoopVerdict::Kill(reason);
            }
            return LoopVerdict::Block(reason);
        }
        self.blocked_last = false;
        if self.no_progress >= self.policy.block_after_stalled && is_new_approach {
            self.recovery_attempted = true;
        }
        if self.no_progress >= self.policy.warn_after_stalled {
            LoopVerdict::Warn(if self.recent.back()
                .is_some_and(|r|r.progress == ProgressEvidence::Unverified) {
                LoopReason::UnverifiedEffect
            } else { LoopReason::NoProgress })
        } else {
            LoopVerdict::Allow
        }
    }

    /// Call exactly once for every successfully decoded native *feedback*
    /// after dispatch; never trust a model's assertion of completion.
    pub fn observe_feedback(
        &mut self, tool: &str, args: &Map<String, Value>,
        feedback: &Value, effect: ToolEffect,
    ) -> ProgressEvidence {
        let observation = self.classify(feedback, effect, tool);
        self.observe(tool, args, observation);
        observation
    }

    /// Trusted effect supervisor may report a verified receipt; this API is
    /// not model-facing and MUST NOT be called with untrusted success claims.
    pub fn observe(
        &mut self, tool: &str, args: &Map<String, Value>, result: ProgressEvidence,
    ) {
        let args_hash = digest(&serde_json::to_vec(args).expect("validated JSON serializes"));
        self.recent.push_back(ObservedCall {
            tool: tool.into(), args_hash, progress: result,
        });
        while self.recent.len() > self.policy.window_size {
            self.recent.pop_front();
        }
        self.blocked_last = false;
        if matches!(result, ProgressEvidence::NewInformation | ProgressEvidence::StateChanged) {
            self.no_progress = 0;
            self.consecutive_blocks = 0;
            self.recovery_attempted = false;
        } else {
            self.no_progress = self.no_progress.saturating_add(1);
            // An unverified operation is not the same as a verified no-op:
            // preserve the signal and require a host witness for long work.
            if result == ProgressEvidence::Unverified && self.no_progress < self.policy.warn_after_stalled {
                self.consecutive_blocks = 0;
            }
        }
    }

    fn classify(
        &mut self, feedback: &Value, effect: ToolEffect, tool: &str,
    ) -> ProgressEvidence {
        if feedback.get("ok").and_then(Value::as_bool) != Some(true) {
            return ProgressEvidence::NoProgress;
        }
        let Some(result) = feedback.get("result") else {
            return ProgressEvidence::Unverified;
        };
        match effect {
            ToolEffect::ReadWorkspace | ToolEffect::NetworkRead => {
                // Remove unstable metadata, not actual content; only new
                // semantic results count as progress. A changed call ID or
                // timing field is never evidence of new information.
                // Do not count a different echoed filename, page number or
                // operation request as genuine information. Only known
                // data-bearing result fields can attest new observations.
                let Some(semantic) = semantic_read_result(result) else {
                    return ProgressEvidence::Unverified;
                };
                let normalized = without_volatiles(&semantic);
                let fingerprint = digest(&serde_json::to_vec(&normalized)
                    .expect("trusted JSON serializes"));
                if self.seen.contains(&fingerprint) {
                    ProgressEvidence::NoProgress
                } else {
                    self.seen.push_back(fingerprint);
                    while self.seen.len() > MAX_SEEN_RESULTS { self.seen.pop_front(); }
                    ProgressEvidence::NewInformation
                }
            }
            ToolEffect::WriteWorkspace => {
                // A verified edit witness is the only general-purpose
                // postcondition we currently understand. The *native* typed
                // result must say it committed AND the after-hash changed.
                let obj = result.as_object();
                let committed = obj.and_then(|m|m.get("committed"))
                    .and_then(Value::as_bool) == Some(true);
                let before = obj.and_then(|m|m.get("original_sha256"))
                    .and_then(Value::as_str);
                let after = obj.and_then(|m|m.get("replacement_sha256"))
                    .and_then(Value::as_str);
                if committed && matches!((before, after), (Some(a), Some(b))
                    if valid_sha(a) && valid_sha(b) && a != b)
                {
                    return ProgressEvidence::StateChanged;
                }
                if committed && matches!((before, after), (Some(a), Some(b))
                    if valid_sha(a) && valid_sha(b) && a == b)
                {
                    return ProgressEvidence::NoProgress;
                }
                // Create-new cannot succeed twice for the same target under
                // existing native create-only semantics.
                if committed && tool == "lu_create_new_file" {
                    return ProgressEvidence::StateChanged;
                }
                ProgressEvidence::Unverified
            }
            _ => ProgressEvidence::Unverified,
        }
    }
}
fn digest(bytes: &[u8]) -> [u8;32] {
    let sum = Sha256::digest(bytes);
    let mut out = [0u8;32];
    out.copy_from_slice(&sum);
    out
}
fn valid_sha(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b|b.is_ascii_hexdigit())
}
fn without_volatiles(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut stable=Map::new();
            for (key, v) in map {
                if matches!(key.as_str(),
                    "call_id" | "request_id" | "duration_ms" |
                    "elapsed_ms" | "timestamp" | "created_at" |
                    "updated_at" | "execution_time" | "_lu_loop_warning"
                ) {
                    continue;
                }
                stable.insert(key.clone(),without_volatiles(v));
            }
            Value::Object(stable)
        }
        Value::Array(items) => Value::Array(items.iter().map(without_volatiles).collect()),
        _ => value.clone(),
    }
}

/// Recognize actual information fields from trusted typed native decoders.
/// Tool-specific decoders can add fields to this allowlist during review.
/// Unrecognized metadata alone is not proof of progress.
fn semantic_read_result(result: &Value) -> Option<Value> {
    let object = result.as_object()?;
    const EVIDENCE_FIELDS: &[&str] = &[
        "content", "text", "entries", "matches", "hits", "files",
        "diff", "commits", "items", "rows", "data", "results",
        "kind", "size_bytes", "read_only", "exists",
        "valid", "issues", "truncated", "bytes_read",
        "candidate_present", "evidence", "word_count",
    ];
    let mut evidence = Map::new();
    for field in EVIDENCE_FIELDS {
        if let Some(value) = object.get(*field) {
            evidence.insert((*field).to_owned(), value.clone());
        }
    }
    if evidence.is_empty() { None } else { Some(Value::Object(evidence)) }
}
