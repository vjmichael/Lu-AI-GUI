//! TOOLS-08: host-owned, per-turn structural loop prevention.
//!
//! Implements the *intent* of original Jan-Mod-Design-Document §7.
//! Unlike the previous pattern breaker this watches *actual validated tool
//! proposals*, not generated prose. It is NEVER a model-facing tool or a
//! permission to dispatch. A blocked call cannot reach native authorization,
//! the effect journal, the registered native tool, or a subprocess.
//!
//! Original §7 has a five-call history and a separate 20-attempt counter;
//! mistakenly checking "recentCalls.length >= 20" with window=5 would never
//! fire. This implementation keeps a separate monotonically increasing
//! per-turn attempt counter. The existing Lu native turn's MAX_TOOL_STEPS=8
//! remains a stricter independent ceiling; this guard does not enlarge it.

use std::collections::{BTreeSet, VecDeque};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

pub const DEFAULT_LOOP_WINDOW: usize = 5;
pub const DEFAULT_TOTAL_WARN: u32 = 20;
pub const DEFAULT_EXACT_PREVIOUS: usize = 2;
pub const DEFAULT_KILL_BLOCKS: u32 = 3;
const MAX_SIGNATURE_TOKENS: usize = 512;
const MAX_SIGNATURE_DEPTH: usize = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopReason {
    ExactRepeat,
    PingPong,
    SimilarArguments,
    ExcessiveCalls,
}

impl LoopReason {
    /// Fixed host-owned language. Never reflect raw tool JSON, path, user
    /// content, previous model outputs or secret arguments in model feedback.
    pub const fn message(self) -> &'static str {
        match self {
            Self::ExactRepeat =>
                "Repeated identical tool request. Try a different approach.",
            Self::PingPong =>
                "Alternating tool calls form a loop. Change the approach.",
            Self::SimilarArguments =>
                "This tool was repeatedly called with similar arguments. Check progress.",
            Self::ExcessiveCalls =>
                "High number of tool calls in this turn. Consider finishing or changing strategy.",
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LoopPolicy {
    pub window_size: usize,
    pub warn_threshold_same_tool: usize,
    pub warn_threshold_total: u32,
    pub block_threshold_exact: usize,
    pub block_ping_pong: bool,
    pub kill_after_blocks: u32,
    pub similarity_threshold: f64,
}

impl Default for LoopPolicy {
    fn default() -> Self {
        Self {
            window_size: DEFAULT_LOOP_WINDOW,
            warn_threshold_same_tool: 3,
            warn_threshold_total: DEFAULT_TOTAL_WARN,
            block_threshold_exact: DEFAULT_EXACT_PREVIOUS,
            block_ping_pong: true,
            kill_after_blocks: DEFAULT_KILL_BLOCKS,
            similarity_threshold: 0.7,
        }
    }
}
impl LoopPolicy {
    pub fn validate(self) -> Result<Self, &'static str> {
        if !(4..=32).contains(&self.window_size) ||
            self.warn_threshold_same_tool == 0 ||
            self.warn_threshold_same_tool > self.window_size ||
            self.warn_threshold_total == 0 ||
            self.block_threshold_exact == 0 ||
            self.block_threshold_exact > self.window_size ||
            self.kill_after_blocks == 0 ||
            !self.similarity_threshold.is_finite() ||
            !(0.0..=1.0).contains(&self.similarity_threshold)
        {
            return Err("invalid trusted host loop protection policy");
        }
        Ok(self)
    }
}

struct RecentCall {
    tool: String,
    args_hash: [u8; 32],
    signature: BTreeSet<[u8; 32]>,
}

pub struct LoopGuard {
    policy: LoopPolicy,
    recent: VecDeque<RecentCall>,
    attempted: u32,
    consecutive_blocks: u32,
}

impl Default for LoopGuard {
    fn default() -> Self {
        Self::new(LoopPolicy::default()).expect("built-in loop policy valid")
    }
}

impl LoopGuard {
    /// Create per authenticated *turn*, not per conversation/AI model/tool.
    /// Only the trusted host supplies this policy. The model cannot tune it.
    pub fn new(policy: LoopPolicy) -> Result<Self, &'static str> {
        Ok(Self {
            policy: policy.validate()?,
            recent: VecDeque::new(),
            attempted: 0,
            consecutive_blocks: 0,
        })
    }

    pub fn attempted(&self) -> u32 { self.attempted }
    pub fn consecutive_blocks(&self) -> u32 { self.consecutive_blocks }

    /// Invoked only after the outer model JSON and native codec have strictly
    /// validated the tool name + arguments, immediately before native dispatch.
    /// Call counts include blocked requests. A blocked request is *not*
    /// added to the successful-call history, preserving the exact loop pattern
    /// for the next attempt and guaranteeing escalation.
    pub fn check(&mut self, tool: &str, args: &Map<String, Value>) -> LoopVerdict {
        self.attempted = self.attempted.saturating_add(1);
        let canonical = serde_json::to_vec(args)
            .expect("an already validated JSON object always serializes");
        let args_hash = digest(&canonical);
        let signature = signature(args);

        let exact = self.recent.iter()
            .filter(|call| call.tool == tool && call.args_hash == args_hash)
            .count();
        if exact >= self.policy.block_threshold_exact {
            return self.block(LoopReason::ExactRepeat);
        }

        // A B A B A is blocked on the 5th call. Merely calling two *different*
        // tools in sequence is not a loop.
        if self.policy.block_ping_pong && self.recent.len() >= 4 {
            let n = self.recent.len();
            let a = &self.recent[n-4].tool;
            let b = &self.recent[n-3].tool;
            let a2 = &self.recent[n-2].tool;
            let b2 = &self.recent[n-1].tool;
            if a == a2 && b == b2 && a != b && tool == a {
                return self.block(LoopReason::PingPong);
            }
        }

        let same_tool: Vec<_> = self.recent.iter()
            .filter(|call| call.tool == tool).collect();
        let near_repeat = same_tool.len() >= self.policy.warn_threshold_same_tool
            && same_tool.iter().any(|call|
                jaccard(&signature, &call.signature) > self.policy.similarity_threshold
            );

        let warning = if near_repeat {
            Some(LoopReason::SimilarArguments)
        } else if self.attempted >= self.policy.warn_threshold_total {
            Some(LoopReason::ExcessiveCalls)
        } else {
            None
        };
        self.consecutive_blocks = 0;
        self.recent.push_back(RecentCall {
            tool: tool.into(), args_hash, signature
        });
        while self.recent.len() > self.policy.window_size {
            self.recent.pop_front();
        }
        match warning {
            Some(reason) => LoopVerdict::Warn(reason),
            None => LoopVerdict::Allow,
        }
    }

    fn block(&mut self, reason: LoopReason) -> LoopVerdict {
        self.consecutive_blocks = self.consecutive_blocks.saturating_add(1);
        if self.consecutive_blocks >= self.policy.kill_after_blocks {
            LoopVerdict::Kill(reason)
        } else {
            LoopVerdict::Block(reason)
        }
    }
}

fn digest(bytes: &[u8]) -> [u8;32] {
    let hash = Sha256::digest(bytes);
    let mut result = [0_u8;32];
    result.copy_from_slice(&hash);
    result
}

fn signature(args: &Map<String, Value>) -> BTreeSet<[u8;32]> {
    fn visit(value: &Value, path: &str, depth: usize, tokens: &mut BTreeSet<[u8;32]>) {
        if depth > MAX_SIGNATURE_DEPTH || tokens.len() >= MAX_SIGNATURE_TOKENS {
            return;
        }
        match value {
            Value::Object(items) => for (key, child) in items {
                if tokens.len() >= MAX_SIGNATURE_TOKENS { break; }
                let next = format!("{path}/{}", key.replace('~',"~0").replace('/',"~1"));
                tokens.insert(digest(format!("key:{next}").as_bytes()));
                visit(child, &next, depth+1, tokens);
            },
            Value::Array(items) => for (index, child) in items.iter().enumerate() {
                if tokens.len() >= MAX_SIGNATURE_TOKENS { break; }
                visit(child, &format!("{path}/{index}"), depth+1, tokens);
            },
            _ => {
                let scalar = serde_json::to_string(value)
                    .expect("validated scalar JSON serializes");
                tokens.insert(digest(format!("val:{path}={scalar}").as_bytes()));
            }
        }
    }
    let mut tokens = BTreeSet::new();
    visit(&Value::Object(args.clone()), "", 0, &mut tokens);
    tokens
}

fn jaccard(a: &BTreeSet<[u8;32]>, b: &BTreeSet<[u8;32]>) -> f64 {
    if a.is_empty() && b.is_empty() { return 1.0; }
    let intersection = a.intersection(b).count();
    let union = a.len() + b.len() - intersection;
    if union == 0 { 1.0 } else { intersection as f64 / union as f64 }
}
