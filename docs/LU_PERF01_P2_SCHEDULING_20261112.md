# PERF-01 — Foreground inference scheduling and latency (2026-11-12)

**Scope:** Lu Core + desktop host; explicitly excludes Lu IDE, NVP, alternate runtimes, new tool grants, and model-triggered OS action. **Source-only implementation: not native/installed accepted, not merged, not released.**

**Branch:** `lu/perf01-p2-inference-latency-20261112`, stacked on `lu/perf01-p1-warm-model-20261112`.

## Naming reconciliation with issue #201

This conversation calls the next work group **P2 — Inference Scheduling and Latency**. GitHub [PERF-01 issue #201](https://github.com/vjmichael/Lu-AI-GUI/issues/201) labels **P2 as prefix/KV cache reuse**, **P3 as direct streaming**, and **P4 as interactive priority**. These source changes implement the *requested scheduling/latency scope* (parts of issue P3/P4), not prefix/KV cache reuse. Cache reuse and invalidation remain explicitly OPEN. Do not mark issue #201 P2/P3/P4 fully accepted.

## Source implementation

1. **Trusted two-class generation queue** (`lu-desktop/src-tauri/src/generation_queue.rs`): only the native host selects Foreground (ordinary human Send) versus Background (opted-in task-terminal continuation). New foreground turns overtake already-queued background model replies, but never preempt a running inference. Background gets a slot after four foreground admissions, bounded by two queued/running background jobs against eight total, so at least six slots are available to human turns. Per-chat uniqueness, same-session binding, queue cancellation and RAII ticket cleanup remain intact. Cancelled queued jobs are removed from priority wait lanes immediately rather than blocking another turn.
2. **Desktop host origin mapping and background gate** (`lu-desktop/src-tauri/src/lib.rs`): `generate_internal` binds the trusted origin to the queue class. Existing 500 ms watchdog still observes/reconciles running worker outcomes and notices, but calls the new foreground-guarded scheduler path to defer **new** native background launches while a human generation is pending or running. No worker is killed, no authorization extended, and no GPU/VRAM claims invented. The separate P1 warm eviction guard remains.
3. **First-token path** (`lu-desktop/src-tauri/src/lib.rs`): send each decoded token chunk to the Tauri UI event stream before synchronous ANSF/SQLite progress-checkpoint writes. Subsequent bounded progress checkpoints and durable completion remain mandatory.
4. **Original timing evidence** (`lu-desktop/src-tauri/src/perf01_telemetry.rs`): distinguish human versus terminal-continuation traces; measure enqueue-to-slot, context assembly, dispatch-to-first-chunk, user-turn-to-first-chunk, and user-turn-to-first successfully forwarded Tauri WebView event. Last field is *host event-forwarding time*, **not proof of browser render or GPU prefill/decode rates**. Preserve 128-receipt host-bound bounded memory and chat-scoped retrieval.
5. **Native process guardian** (`src-tauri/lu-core/src/native_background_scheduler.rs`): `tick_foreground_guarded` always polls and reconciles existing child processes and TaskStore receipts, while suppressing new starts during foreground model work. Existing `tick` / `tick_at` remain unchanged for other native callers and tests.

## Source regression coverage (added, NOT executed in this task)

- Human overtakes queued auto continuation; running turn is not preempted.
- Background fairness after four consecutive human admissions.
- Two background requests cannot exhaust eight-slot queue; six simultaneous human tickets can still enter.
- Cancelling foreground/background waiters does not cancel an unrelated active turn or block queue progress.
- Guarded native scheduler leaves a due worker queued under human activity and applies original worker hash/admission after the gate clears.
- First model chunk / successful WebView forwarding reported separately; a failed UI forward must not be reported as visible.

## Known limitations / acceptance

- **No actual Rust/Windows/Linux native compile or installed UI run performed in this source iteration.** The build/test toolchain is not present in the current execution environment; no CI batch was requested/dispatched.
- Foreground admission gate samples the current native queue; races with concurrently starting workers require stress testing. Already-started workers are not preempted; resource budgets limit additional starts but cannot reclaim their CPU/RAM instantaneously.
- Queued-position events are a snapshot and may change when later foreground turns arrive. UI should not treat the position as immutable.
- No physical Radeon GGUF TTFT improvement, latency p50/p95/p99, VRAM occupancy, cache hit rate, or full WebView-paint measurement is claimed. No prefix/KV caching was implemented.
- Inference produced through other protected subpaths (e.g. browser proposal) needs consolidated scheduling/authority audit in the grouped acceptance gate.
- Grouped acceptance must run exact-source-head Rust unit/integration tests and Windows/Linux builds; mixed cold/warm genuine GGUF foreground/background tasks; verified event/partial-history cancellation and restart; resource-pressure and checked-binary worker tests; and before/after TTFT distributions on the same hardware. Keep native security and local Windows owner-operated acceptance open.

**Disposition:** Source implementation assembled; proceed to the next PERF group provisionally while retaining this native/installed performance acceptance and issue #201 prefix/KV reuse as separate open items. No merge/release authorization.
