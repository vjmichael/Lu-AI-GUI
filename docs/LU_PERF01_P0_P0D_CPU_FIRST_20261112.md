# Lu PERF-01 P0/P0D — Host Timing and CPU-First Deterministic Dispatch

**Branch:** `lu/perf01-p0-p0d-cpu-first-20261112`
**Parent:** TOOLS-09 source `d27c31ad5592999fdbb0f7056009988cedcdc836`
**Scope:** Lu Core desktop/ANSF; no IDE files or new standalone search application.
**State:** Source implementation candidate. Native Windows/Linux compilation, real model/OS measurements and installed app acceptance are OPEN.

## P0 implementation: actual native-host instrumented turn path

- Added `perf01_telemetry.rs`, a bounded, 128-entry, in-memory per-chat host timing ledger. It retains a maximum of 24 same-chat entries per read and exposes only after a fresh ANSF chat existence check through `lu_perf01_recent`.
- Instrumented the real `generate_internal` admission/queue, context assembly, model dispatch, first emitted text chunk, terminal completion/failure/cancellation and streamed UTF-8 byte count. Host timing uses monotonic `Instant`; the timing receipt is recorded before the generation command completes.
- Existing no-GGUF host clock and new literal CPU chat lookup report host-observed duration and original-source scan scope without calling model load or requesting an OS tool grant. `modelStartedByThisRoute=false` is recorded for CPU calls only; inference routes leave this field unknown unless independently observed.
- Separate fields distinguish model-queue wait, host-context assembly, engine-dispatch to first text and end-to-end wall latency. Known selected model reference and backend family (`llama.cpp`) are recorded for inference; configured context limit is 4096 tokens.
- The receipt explicitly leaves prefill rate, decode rate, cache hit, CPU/GPU load, VRAM, quantization and verified task-throughput unknown. UTF-8 output bytes are NOT token counts. Missing timings are `null`, never zero or estimated.
- Model prompts, source contents, file paths and tokens are not recorded in this ledger. Original chat ID and generation ID are retained solely for per-chat scope; diagnostics do not mint tool authority, task status or verified effects.
- A small read-only current-chat receipt now displays route, host wall time, queue wait (if measured), first text (if observed) and terminal status in the normal desktop composer. No extra permission button is mounted.
- The existing engine `load_trace` and memory/VRAM probes remain separate original host observations. They are not misrepresented as newly measured GPU throughput.

## P0D implementation: CPU-first actual ANSF retrieval

- The original current-human Send accepts narrowly identified `Search this chat for: <literal>` or `Find in this conversation: <literal>` without a loaded GGUF model. A second exact phrase (`Show local model status`, `What model is loaded?`, `Is a model loaded?`) returns the current native supervisor phase/model reference without inference. Both extend the pre-existing no-model OS clock path.
- Classification occurs independently in Rust after the UI hint. Only text-only normal Send may use this path; no attachments, planning, reviewed specialist mode, ambiguous search syntax, embedded control tokens or large/unsafe phrase.
- Model session identity is still checked against the current supervisor. The active chat is authenticated from the current ANSF profile. Source is read from its original immutable ANSF book and original canonical source IDs, never an arbitrary model-supplied path or a different chat.
- The CPU scan is strictly bounded: up to 640 source entries, 2 MiB of content, 300 ms elapsed, eight matches, and 4096 characters from any one original. An oversized source, exceeded bound or missing cursor returns a specific PARTIAL status. Zero hits in an incomplete scan are NOT proof of absence.
- Original matches are presented with canonical ANSF source IDs and text snippets sanitized for Markdown/HTML display. They are historical evidence, not fresh file assertions, current instructions or execution permissions.
- Model-required requests still use the original inference flow. URL Browser, model-powered summarization and general semantic questions are not shortcut by this lexical route.
- PERF-01 P0D closeout extends the explicit CPU dispatcher to `Search selected workspace for: <literal>`: fresh native OS folder picker, same-session recheck immediately before I/O, original authenticated ANSF user-turn, and precisely the TOOLS-09 root-scoped no-follow, mount-aware, finite recursive search implementation. There is no model-chosen path or inherited approval. The result carries relative names, SHA-256 evidence, skipped entries, and a non-deceptive COMPLETE/PARTIAL status.
- `Show current chat counts` reads only the current chat's bounded ANSF visible index, with lower-bound counts when history or time limits are hit. No whole-history transcript or other chat is opened.
- Tests include direct actual ANSF SQLite current-chat isolation, cross-chat denial, source IDs, oversized-source partial outcome, strict intent negatives, model-session fence and fake native performance metric prevention.

## Repeatable local baseline procedure (not yet executed)

1. Build and verify the exact integrated Git HEAD and source-tree lock. Set `LU_PERF01_SOURCE_HEAD` to the checked-out original SHA at build time if the original verified build system provides it. Missing HEAD stays `null`, not a guessed identity.
2. On the installed host, capture the genuine host `lu_perf01_recent` JSON for at least five repetitions each of CPU literal lookup, warm model chat and cold model load as separate series, using fixed test corpora and identical model/context/temperature/hardware. CPU tests with unloaded model must prove model session/pid stays absent. Capture the existing separate engine load trace for cold load.
3. Run `node scripts/perf01-host-baseline.mjs --input original-host-receipts.json --head <verified-original-SHA>` to calculate the bounded count/p50/p95 and mean. The program refuses absent or changed source identity, missing evidence, insufficient repeats, non-host status and alleged model starts caused by CPU routing.
4. Independently inspect the real installed app, OS device/runtime logs, original input set, actual model/GGUF quantization, cache warm/cold state and CPU/GPU memory. Run before/after comparison on the SAME source-verified hardware/configuration. The baseline script alone never reports an optimization success or physical acceptance.

**Current measured baseline:** NONE. There is no installed Windows/Linux runtime or genuine loaded-GGUF execution in this coding environment, so there are no actual p50/p95 results, GPU starts, token rates or verified throughput figures.

## Still open

- P0: source-reproducible installed run, cold model load/observer correlation, UI Send-to-host timing, real TTFT/VRAM/CPU utilization, backend/token counters, cache observation, head-pinned original artifact export and genuine owner-operated baseline.
- P0D: the native-approved CPU workspace literal, ANSF exact lookup and current-chat structural count are now integrated. Still open are Windows native reparse/swap acceptance, same-device bind mounts, filesystem-index freshness/restart benchmarking, true indexed FTS5 current-chat lookup at scale, and deterministic structured task/Git status results. The model-mediated workspace inspection path remains separately authorized.
- Native Linux/Windows compiler and real fixture execution remain independently gated. Consolidated Rust/Node suites can run through the branch-scoped PERF-01 acceptance workflow; source-pattern checks are not substitutes for actual runtime behavior.
- No inference/cache/worker permissions are inherited, no old effect is replayed, no CI/PR/merge/release was authorized. The accepted alpha installer remains unchanged.

## Following task groups

Within PERF-01: complete P0/P0D native baseline then P1 warm model lifecycle, P2 prefix/KV caching, P3 first-token streaming, P4 interactive-priority scheduling, P5 tuning, P6 optional multi-GPU, P7 alternative backend evaluation.
Across Lu Core: finish P0-WIN-CHAT #204, TOOLS-10 browser/research, P0-Next/PLAN task-method integration, VIS-00B–05 rich responses, model library/imports, DLC-API/IPC later, and the deferred SEC-06/P0-GATE/NSIS/Debian/physical Radeon acceptance.


## PERF-01 P0/P0D exact-head closeout extension

The closeout source branch is `lu/perf01-p0-p0d-closeout-20261112`.
It builds on the earlier instrumentation and CPU ANSF path without touching IDE.
The native host's original llama.cpp engine callback now increments a bounded
atomic **observed engine_spawned process-start count**, including callbacks
outside a currently active model-load diagnostics operation. A generation
receipt holds the observed process-start event delta and host-process-lifetime
total. This is actual backend callback evidence when executed, not a claim
that a GPU was active or that an engine restart was caused by that turn.
Absent backend/PID observations remain absent.

For owner-operated physical Windows acceptance use
`scripts/perf01-windows-owner-metrics.ps1` with exact Lu and engine PIDs
from original process/engine receipts, the installed EXE identity and an
approved evidence directory. It records process CPU-time deltas and working
set while checking PID/start-time custody, and records optional Windows GPU
Engine PDH counters by engine PID. Those counters are **not** whole-GPU,
Radeon adapter load, inference tokens/sec, or Vulkan offload proof.
Output always remains **PENDING owner/independent acceptance**.

Consolidated exact-head tests now include `perf01-cpu-first.test.mjs`,
`perf01-p0d-closeout.test.mjs`, `perf01-baseline.test.mjs`, and
the original TOOLS-09 hostile scan fixture in the Lu Core test library.
The closeout workflow uses real Windows/Linux native Rust and Node runners,
not Docker, mock GGUF hardware or installed-NSIS acceptance.

**Outstanding physical acceptance:** an actual installed Lu Windows run
on the owner's Radeon machine; exact loaded GGUF/MMProj and model engine PID;
observed baseline CPU/GPU/VRAM and model-start counts on that machine;
repeatable before/after P50/P95 using the same model and input corpus;
genuine interactive Send/cancel/restart behavior; independent inspection of
original OS and ANSF receipts. None of these is established merely by source
commits or synthetic test values.
