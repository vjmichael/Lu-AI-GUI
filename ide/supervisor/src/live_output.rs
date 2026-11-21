//! DEV-03.3B: bounded, ephemeral worker-to-host output witness.
//! This is untrusted process text, NEVER an OS result, grant, journal or
//! recovered log. Kept only in the host's active job, not on disk.
use serde::Serialize;
use std::collections::VecDeque;
use std::sync::Mutex;

const MAX_TEXT_BYTES: usize = 64 * 1024;
const MAX_EVENTS: usize = 256;
const CHUNK_BYTES: usize = 2048;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputChunk {
    pub sequence: u64,
    pub stream: &'static str,
    pub text: String,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputPage {
    pub chunks: Vec<OutputChunk>,
    pub last_sequence: u64,
    pub truncated: bool,
    pub live_unverified: bool,
}
#[derive(Default)]
struct OutputState {
    chunks: VecDeque<OutputChunk>,
    bytes: usize,
    last_sequence: u64,
    truncated: bool,
}
#[derive(Default)]
pub struct LiveOutput {
    state: Mutex<OutputState>,
}
impl LiveOutput {
    /// Only the native worker calls append, for already-authorized child
    /// stdout/stderr or the Windows merged diagnostics HANDLE. Never UI text.
    pub fn append(&self, stream: &'static str, source: &[u8]) {
        if !matches!(stream, "stdout" | "stderr" | "combined") || source.is_empty() {
            return;
        }
        if let Ok(mut state) = self.state.lock() {
            for part in source.chunks(CHUNK_BYTES) {
                let mut text = String::from_utf8_lossy(part).into_owned();
                // Prevent terminal escape/control sequences affecting UI or logs.
                text = text
                    .chars()
                    .map(|c| {
                        if c == '\n' || c == '\r' || c == '\t' || !c.is_control() {
                            c
                        } else {
                            '�'
                        }
                    })
                    .collect();
                let size = text.len();
                state.last_sequence = state.last_sequence.saturating_add(1);
                let seq = state.last_sequence;
                if size > MAX_TEXT_BYTES {
                    state.truncated = true;
                    continue;
                }
                while state.bytes.saturating_add(size) > MAX_TEXT_BYTES
                    || state.chunks.len() >= MAX_EVENTS
                {
                    if let Some(old) = state.chunks.pop_front() {
                        state.bytes -= old.text.len();
                        state.truncated = true;
                    } else {
                        break;
                    }
                }
                state.bytes += size;
                state.chunks.push_back(OutputChunk {
                    sequence: seq,
                    stream,
                    text,
                });
            }
        }
    }
    /// Native host-only bounded capture AFTER the original process has
    /// returned its terminal OS receipt. Does not persist output or reconstitute
    /// any result after restart. Never exposes raw source or a worker grant.
    pub fn bounded_native_snapshot(&self) -> OutputPage {
        match self.state.lock() {
            Ok(state) => OutputPage {
                chunks: state.chunks.iter().cloned().collect(),
                last_sequence: state.last_sequence,
                truncated: state.truncated,
                live_unverified: true,
            },
            Err(_) => OutputPage {
                chunks: Vec::new(),
                last_sequence: 0,
                truncated: true,
                live_unverified: true,
            },
        }
    }
    /// A cursor only pages existing untrusted text, never authorizes work.
    pub fn page(&self, after: u64) -> OutputPage {
        match self.state.lock() {
            Ok(state) => OutputPage {
                chunks: state
                    .chunks
                    .iter()
                    .filter(|item| item.sequence > after)
                    .take(64)
                    .cloned()
                    .collect(),
                last_sequence: state.last_sequence,
                truncated: state.truncated
                    || state
                        .chunks
                        .front()
                        .is_some_and(|first| after > 0 && after.saturating_add(1) < first.sequence),
                live_unverified: true,
            },
            Err(_) => OutputPage {
                chunks: Vec::new(),
                last_sequence: after,
                truncated: true,
                live_unverified: true,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn distinct_streams_and_incremental_cursor() {
        let sink = LiveOutput::default();
        sink.append("stdout", b"one\n");
        sink.append("stderr", b"two\n");
        let page = sink.page(0);
        assert_eq!(page.chunks.len(), 2);
        assert_eq!(page.chunks[0].stream, "stdout");
        assert_eq!(page.chunks[1].stream, "stderr");
        assert_eq!(sink.page(1).chunks.len(), 1);
        assert_eq!(sink.page(2).chunks.len(), 0);
        assert!(page.live_unverified);
    }
    #[test]
    fn huge_output_is_bounded_and_truncated() {
        let sink = LiveOutput::default();
        sink.append("stdout", &vec![b'x'; 100_000]);
        let page = sink.page(0);
        assert!(page.truncated);
        assert!(page.chunks.len() <= MAX_EVENTS);
        assert!(page.chunks.iter().map(|x| x.text.len()).sum::<usize>() <= MAX_TEXT_BYTES);
    }
    #[test]
    fn malformed_stream_and_controls_do_not_create_authority() {
        let sink = LiveOutput::default();
        sink.append("shell", b"malicious");
        assert!(sink.page(0).chunks.is_empty());
        sink.append("combined", b"abc\x1b[31m");
        assert!(sink.page(0).chunks[0].text.contains('�'));
    }
}
