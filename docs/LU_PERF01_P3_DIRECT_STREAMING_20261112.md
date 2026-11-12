# PERF-01 P3 — Direct streaming completion (2026-11-12)

**Status: source implementation assembled, NOT native/installed accepted.**
Repo `vjmichael/Lu-AI-GUI`, branch `lu/perf01-p3-direct-streaming-20261112`, stacked on `lu/perf01-p2-kv-cache-20261112`. Scope is Lu Core and original Tauri desktop. No Lu IDE, NVP, model runtime duplication, new effect permission, merge, release or per-edit CI.

## Original sources and retained invariants

- `src-tauri/lu-core/src/inference_bridge.rs`: authenticated Protobuf event sequence and original cancellation token unchanged.
- `src-tauri/lu-core/src/ansf_live_chat.rs`: append-only ANSF original messages; `assistant_progress` is an interrupted view until an actual `assistant_completed` or `assistant_cancelled` native journal event. Future model history excludes unfinished assistant output.
- `lu-desktop/src-tauri/src/ansf_chat_store.rs`: original chat-scoped SQLite book, bounded visible history window, terminal state and restart recovery. No synthetic final answer.
- Existing P1 warm model, P2 same-scope cache gate, and priority inference queue remain; completion cannot bypass the original one-generation reservation or operator Stop.

## P3 source changes

1. **Bounded native delivery:** New `lu-desktop/src-tauri/src/stream_delivery.rs` creates consecutive host-owned event cursors, forwards the first visible chunk immediately, coalesces subsequent small chunks at 2 KiB/40 ms, flushes pending output on every terminal return, splits around a 64 KiB event maximum, and rejects cumulative streams above 1 MiB or NUL text. The original model output is accumulated only up to this cap; no unbounded worker-to-WebView channel is introduced. These are conservative host limits, not llama.cpp latency or GPU guarantees.
2. **Per-session in-memory stream custody:** `generation_queue.rs` stores each active generation's bounded output and next event index. `record_chunk` requires the exact active generation/session and monotonic index; wrong session, duplicated/corrupt cursor, cancelled/completed generation, excessive chunk or total output is rejected. The queue is limited to eight generations, with existing foreground priority and maximum two background continuations. No text is shared between chats.
3. **WebView reconnect:** Host-only Tauri command `lu_stream_reconnect` supplies the current exact-session and chat `LiveStreamSnapshot` (phase, generation ID, text, next index, cancelling). `generation_jobs.mjs` accepts contiguous events and marks gaps as requiring authoritative recovery; older/duplicate events cannot splice text. The React `main.jsx` reattaches when opening a chat, after event registration, on focus and on a detected gap. An in-flight snapshot cannot overwrite a new generation that started while the RPC was pending. It never resubmits user prompts or fabricates a worker/tool approval.
4. **WebView transport loss:** An emit failure does not terminate the host-supervised GGUF inference. The original bounded native queue and ANSF checkpoints remain authoritative; `PerfReceipt` only reports successful event enqueue, not actual screen paint.
5. **Durable partial output:** After first UI forwarding the first text chunk is immediately checkpointed in the original ANSF SQLite book. Later cumulative checkpoints are at 16 KiB increments or 750 ms; completion and failure paths flush the full bounded text. A crash after the checkpoint leaves the exact partial marked `interrupted` in the indexed history, not a completed assistant response. The UI explicitly labels interrupted partials. If the host restarted there is no active stream snapshot, and the WebView shows the durable original record instead; automatic inference replay is prohibited.
6. **Native cancellation/completion ordering and cache revocation:** `Ticket::commit_completed` serializes final ANSF progress and terminal `assistant_completed` publication against the same trusted generation queue Stop mutex. If cancellation was recorded first, no final completion is published; once committed, a stale Stop cannot retroactively cancel it. Already-running model cancellation still uses the original supervisor token and the original generation ID.

## Source regression coverage added

- Rust queue: exact-chat/session reconnect, monotonic cursor, wrong-session denial, UTF-8, oversized chunks, cancellation after partial output, final journal commit ordered against Stop and denial of post-commit Stop.
- Rust delivery: immediate first chunk, coalescing, host event index continuity, Unicode preservation, oversized/invalid input refusal, forced final tail and 64 KiB boundary.
- Original ANSF desktop store: reopen SQLite after a simulated host death; recover last exact partial via history/history-window; exclude incomplete output from model context.
- Node UI reducers: start event without queued event, gap detection and native snapshot repair, stale snapshot and duplicate rejection, cross-chat protection, upper-bound output and no live backend after restart.
- Existing R18 source contract updated to accept P2's `enqueue_with_class` API without changing original queue-before-supervisor safety.

**Execution evidence this source turn:** the exact fetched `generation_jobs.mjs` reducer was run in an isolated JavaScript check with ten passing behavior assertions before later integrations; these are NOT comprehensive Node/Rust tests. All added source test suites remain **unexecuted** against an integrated checkout.

## Known limitations / acceptance still required

- No original exact-head Rust/Cargo compile or Windows/Linux installed run was performed. The environment lacks Cargo and external GitHub network access; no CI batch was requested or dispatched. All reported capabilities require native regression.
- The coalescer bounds bytes and event emission count, but cannot prove the Tauri/WebView delivery queue, graphical paint frame rate or native OS-level memory/cgroup consumption. Its 40 ms flush threshold is checked **when another model chunk arrives**; it is not an independent timer that wakes a blocked llama-server stream. The first chunk is forwarded immediately, and any tail is flushed at termination. Large output is rejected rather than silently truncated or reported complete.
- A host process crash can lose the uncheckpointed tail after the last successful checkpoint; the nominal checkpoint threshold is 16 KiB or 750 ms **evaluated only on a forwarded chunk**, with an immediate first-chunk checkpoint. It is not a real-time flush guarantee during a model stall. Full zero-loss recovery would require synchronous per-chunk persistence before every display, which conflicts with first-token latency. The output must remain marked *partial*.
- Renderer reconnect depends on the live same-process in-memory snapshot. After host restart only the original ANSF partial remains; no hidden generation restart or tool replay occurs. Mid-RPC cancellation/host restart needs real concurrency acceptance.
- Native acceptance should stress multiple concurrent chats, late events, slow/frozen WebView, cancel exactly during final journal commit, short/long Unicode streams, malformed/out-of-order backend SSE, subprocess death, repeat restarts, 1 MiB cap, resource pressure and prompt-queue fairness. Check source SHA, genuine llama.cpp/Windows Vulkan path, first-token p50/p95 and precise original ANSF history against real installed UI.
- A UI emit success only means Tauri accepted an event, not proof a renderer painted it. It must not be represented as actual visual TTFT.

**Disposition:** P3 implementation complete in source *provisionally*. Native, security race, performance, physical owner-run Windows, mixed-model/vision and sustained-production acceptance remain OPEN. Proceed to further implementation groups without falsely closing these gates.
