//! Streaming repetition detector for unproductive local-model generation.
//!
//! Independent from the native tool progress guard. A real GGUF backend has
//! per-request token/byte/time/cancellation bounds; this detector additionally
//! interrupts *large, repetitive text cycles while tokens are arriving*.
//! It is deliberately conservative: prose, code with repeating short labels,
//! and normal structured JSON must not be stopped on a few repeated words.
//! No model-generated command can disable this host-only guard.

const WINDOW_BYTES: usize = 8192;
const MIN_CYCLE_BYTES: usize = 1024;
const MIN_REPETITIONS: usize = 6;
const MAX_CYCLE_PERIOD: usize = 512;
const CHECK_EVERY_BYTES: usize = 64;
const MAX_CONSECUTIVE_EMPTY_CHUNKS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamLoopVerdict {
    Continue,
    RepeatedOutput,
}

pub struct StreamLoopGuard {
    tail: Vec<u8>,
    since_check: usize,
    total_bytes: u64,
    empty_chunks: usize,
}
impl Default for StreamLoopGuard {
    fn default() -> Self {
        Self { tail: Vec::new(), since_check: 0, total_bytes: 0, empty_chunks: 0 }
    }
}
impl StreamLoopGuard {
    pub fn total_bytes(&self) -> u64 { self.total_bytes }

    pub fn observe_chunk(&mut self, chunk: &str) -> StreamLoopVerdict {
        if chunk.is_empty() {
            self.empty_chunks = self.empty_chunks.saturating_add(1);
            return if self.empty_chunks >= MAX_CONSECUTIVE_EMPTY_CHUNKS {
                StreamLoopVerdict::RepeatedOutput
            } else {
                StreamLoopVerdict::Continue
            };
        }
        self.empty_chunks = 0;
        self.total_bytes = self.total_bytes.saturating_add(chunk.len() as u64);
        self.since_check = self.since_check.saturating_add(chunk.len());
        self.tail.extend_from_slice(chunk.as_bytes());
        if self.tail.len() > WINDOW_BYTES {
            let remove = self.tail.len() - WINDOW_BYTES;
            self.tail.drain(..remove);
        }
        if self.since_check < CHECK_EVERY_BYTES {
            return StreamLoopVerdict::Continue;
        }
        self.since_check = 0;
        if self.repeating_suffix() {
            StreamLoopVerdict::RepeatedOutput
        } else {
            StreamLoopVerdict::Continue
        }
    }

    fn repeating_suffix(&self) -> bool {
        let bytes = &self.tail;
        if bytes.len() < MIN_CYCLE_BYTES {
            return false;
        }
        for period in 1..=MAX_CYCLE_PERIOD {
            let reps = MIN_REPETITIONS.max(MIN_CYCLE_BYTES.div_ceil(period));
            let needed = reps * period;
            if needed > bytes.len() {
                continue;
            }
            let last = &bytes[bytes.len()-period..];
            let mut same = true;
            for repetition in 2..=reps {
                let start = bytes.len() - repetition*period;
                if &bytes[start..start+period] != last {
                    same = false;
                    break;
                }
            }
            if same { return true; }
        }
        false
    }
}
