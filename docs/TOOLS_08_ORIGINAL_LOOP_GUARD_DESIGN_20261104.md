# TOOLS-08 — Progress-aware tool and streaming generation loop prevention

**Owner revision: 2026-11-04.** This is the authoritative review of TOOLS-08 on its development branch. **Design authority:** Owner's later explicit instruction supersedes the numeric-call and pattern-only heuristics in [original Jan §7](reference/Jan-Mod-Design-Document.md). Preserve Jan's **internal pre-dispatch guard** and Allow/Warn/Block/Kill concept, but **do not stop productive long tasks for calling a tool eight times, editing the same file repeatedly, or merely alternating tools**.

## 1. Two loops, two detectors

**Tool loops:** A validated tool request is compared with **host-observed results and verified mutation receipts** from previous calls. Lu must distinguish success/changed data from unchanged results, repeated errors, no-op edits and unverified effects. The model cannot declare its own progress.

**Generation/text loops:** The token stream needs its own host-side repetition guard. A stream can loop on the same words without making any tool call. The detector accumulates text across arbitrary chunk boundaries and interrupts a **large exact repeated suffix** (at least 1 KiB and six copies of a period of at most 512 bytes), or **64 consecutive empty text chunks**; ordinary short repeated words are permitted. Existing per-generation max-new-token, bounded output, backend request deadline and cancellation remain separate protections. In Lu's current supervised GGUF adapter the detector runs **while chunks are emitted**, not after a giant response is assembled.

## 2. Unlimited *productive* native calls, bounded resource exposure

- **Removed** `MAX_TOOL_STEPS=8`, the old `ToolTurnError::StepLimit`, and `MAX_TOTAL_MODEL_GENERATIONS=(MAX_TOOL_STEPS+1)*3`. There is **no fixed count of allowed tool invocations** or supervised generations.
- Tool calls still require exactly the same current-turn authority, effect-specific approval and durable journal. **Repetition is not authorization**; a different write/edit still must be individually permitted.
- Host retains **30-minute foreground task deadline**, existing per-model-generation `max_new_tokens=512` default in this adapter, **64 KiB model-response and tool-feedback limits**, **128 KiB cumulative prompt limit**, bounded workers, per-operation cancellation and journal durability. The 30-minute duration is **not** a tool-call count; long background jobs need a deliberate checkpoint/continuation route instead of an infinite foreground request.
- Progress should allow repeated operations on the **same file** if the *actual content hash changes*, including multiple independently approved append, replace, patch and edit transactions. The file path itself, operation name or fresh call ID is not a progress witness.
- The original §7 setting `warn_threshold_total=20` is now obsolete **by explicit owner instruction**; call count is telemetry, not a stopping/warning condition. Original `window_size=5` is expanded to **8 recent observations** because the guard now checks results as well as call patterns.

## 3. Actual progress evidence

| Native result/effect | Progress classification |
| --- | --- |
| Read or network read, successful and materially different stable result | `NewInformation` |
| Repeated same result (ignoring transient timestamps/call IDs/duration fields) | `NoProgress` |
| Any failed/denied operation | `NoProgress` (does not imply safe retry) |
| Authorized write with a **committed typed before/after SHA-256 receipt** whose hashes differ | `StateChanged` |
| Committed edit whose original and replacement hashes are identical | `NoProgress` |
| Successful write without a verifiable state-change receipt | `Unverified` (never automatically assume progress) |
| Executable, elevated, and other unsupported effects | `Unverified` pending effect-specific host receipts |

Read result fingerprints only include **reviewed data-bearing fields from host-decoded typed results**, not echo-only request paths/pages/queries or unstable metadata, and are tracked in a capped 512-entry ring. Unrecognized result structures are `Unverified`, not progress. That catches duplicate content even when the model changes a search query or reads a different file. A read result's novelty is not proof that the overall user goal is finished; it is evidence the task learned something new.

A write receipt **must** come from the native trusted worker and existing durable effect authorization; model text, output from retrieved files or a fabricated JSON field cannot mint a state change. Current `native_edit_existing` already has typed before/after hashes; create-new has a separate create-only postcondition. **More effect-specific receipt adapters** are still needed for every model-facing write and DLC tool.

## 4. Revised escalation

The default policy counts **consecutive calls without verified progress**, not total calls: warn at **3 unproductive results** and block at **8**. One novel operation is allowed to attempt recovery at the stagnant threshold; if it also fails to demonstrate progress, subsequent calls block. Exact repeated failures can block earlier after two observed unsuccessful duplicates. A clear unchanged-output A/B cycle also blocks. Three consecutive blocks terminate the turn.

**After every verified advance the stall counter and block escalation reset.** This means 100 successful meaningful edits are not penalized. The model is told to change strategy for stagnation; merely tweaking a JSON argument or incrementing a page number cannot fabricate a new result. Block/Kill never dispatches a native tool or creates an effect-journal entry.

Existing JSON MBRL (one deterministic normalization and up to two **syntax** regenerations) remains distinct; `LOOP_DETECTED` is **not** a trigger to repeat an uncertain write.

## 5. Implementation

- `src-tauri/lu-core/src/loop_guard.rs`: `LoopGuard::check` before native dispatch and `observe_feedback` afterward; `ProgressEvidence` from **typed, host-decoded result** and effect class; verified progress resets stagnation.
- `src-tauri/lu-core/src/agent_tool_loop.rs`: remove fixed eight-call check; retain monotonic call IDs, separate numeric-overflow failure and all existing grants; attach fixed bounded warning inside result; do not reveal raw args.
- `src-tauri/lu-core/src/generation_loop_guard.rs`: independent streaming token/text-cycle detector; ignores short benign repetition and checks across chunk boundaries.
- `src-tauri/lu-core/src/supervised_tool_generation.rs`: no fixed generation count; bounded wall clock, output/prompt sizes, JSON repair stages and current-turn authority; immediately interrupt repetitive streaming output with terminal `token_loop` classification.
- Tests: numerous changing-read results in one turn; **80 distinct edits with hash-changing postconditions on the same file** (structural host feedback test); a real trusted native read repeated **24 times while content changes**; a real journaled supervised model sequence executing **25 useful native reads**; failures, unchanged edits, stale metadata, no-progress A/B cycling, escalation, bounded feedback, token output periodicity, Windows handle cleanup and regression of original authority checks.

## 6. Release acceptance gaps — do not overclaim

- **Durable progress reconstruction:** in-memory `LoopGuard` history is not yet persisted as a dedicated signed progress record; on crash/restart the host must conservatively halt or rebuild only from trusted journal evidence before resuming.
- **Write progress verification:** the first source implementation recognizes committed before/after state hashes. Expand strong effect-specific receipts and a **task-goal progress model** before calling arbitrary native tool/DLC outcomes genuine progress. A changing timestamp/return value is not enough.
- **Streaming cancellation:** Lu backend must actually honor an interrupted stream callback and kill/reap workers; prove locally with real GGUF/llama-server and installed Windows GUI, not only Rust mock tests.
- **Long context:** the existing cumulative 128 KiB prompt limit is not an eight-call cap but will eventually constrain long workflows. Host-owned summarization/checkpointing must preserve tool receipts and grants; it cannot erase progress history to evade the guard.
- **Time budget:** host foreground timeout currently 30 minutes, model backends must separately enforce per-generation deadlines. No user-visible promise of indefinite unbounded autonomous execution.
- **CI:** Linux/Windows native + supervised tests on the **exact source-locked revision**, source-tree verification, desktop CI. Accepted `0.1.0-alpha.2` untouched; draft PR #19 stays unmerged until accepted.
