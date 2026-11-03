# WIN-LOAD-01 — Model loading diagnostics, increment 1
Date: 2026-11-03. Branch: `lu/win-load-01-host-trace-20261103`. Alpha `0.1.0-alpha.2` remains immutable.

## Purpose and verified boundaries
The accepted alpha intermittently displays a Windows "Not Responding" message while llama.cpp loads a GGUF. The root cause is **unknown**. We must distinguish UI/WebView event-loop delay, native Tauri command responsiveness, inference supervisor waiting, and actual engine/GPU behavior instead of guessing from elapsed time.

This increment adds:
- A host-owned, 96-entry bounded load trace with monotonic sequence number, random operation ID, opaque model reference, phase, and measured elapsed milliseconds.
- Stages `accepted`, `supervisor_enter`, and `finished` with categorical outcome (`ok`, `error`, `worker_join_error`). No estimated completion percentages.
- `lu_load_diagnostics`, a synchronous snapshot that takes only the small trace mutex, not the model catalog or supervisor lock.
- React polling every 1.5 seconds *during load only*, with a single outstanding request and cleanup on state change/unmount. UI displays elapsed wall time separately from host response status and latest observed stage.
- Three Rust unit tests: bounded history/ordering and no fabricated progress, overlapping load rejection, safe no-op completion.

## Interpretation rules
- A changing elapsed timer means the React scheduling loop is still running, but says nothing about Tauri or GPU responsiveness.
- A recent successful diagnostics response shows native command transport responded to that poll; an absent/delayed response does **not** prove a GPU or model defect.
- `supervisor_enter` proves the host attempted a supervisor load; it does not prove the engine has begun reading model bytes.
- `finished: ok` means the supervisor reported successful load, not that GPU memory/offload was independently measured.
- Engine logs and OS GPU performance counters are still required to identify stalls. Never show loaded bytes or progress percent without measured engine evidence.

## Security and privacy
No selected model filesystem path, prompt, API key, stdout/stderr, image content, or user folder names are written into the trace. No WebView-controlled executable selection. Diagnostics are in-memory and bounded; do not log raw error messages.

## Test ledger
| Test | Automated | Deferred platform acceptance |
| --- | --- | --- |
| Trace ordering, eviction, overlapping-load refusal | Rust `load_trace::tests`, Windows GitHub Actions | Repeat after source manifest gate |
| Host diagnostics during long model load | Rust command and WebView wiring; integration not yet tested | Load large Qwen GGUF; verify UI timer advances, host responds independently; capture process responsiveness |
| Engine vs GPU vs WebView stall classification | Not implemented | Correlate OS performance counters and bounded engine logs |
| Cancellation during load | Not implemented | Defined cancellation semantics, process teardown, no orphan |
| Alpha attachment/chat persistence regression | Not part of this increment | Full v1.0 acceptance nearer release |

## Status / blockers
This increment is **development code, not a completed WIN-LOAD-01 gate**. The repository's historical `main` still differs from the locally staged development source. REPO-01 full-source manifest synchronization remains mandatory before whole-application CI can be accepted. This increment's GitHub Actions workflow checks Windows Rust tests of its own branch and must not be represented as a complete Lu build/test.
