# TOOLS-08B — Durable progress reconstruction and fail-closed resumption

Status: implementation design, **not yet coded or accepted**. This branch is based on TOOLS-08 PR #19 at `62992c9ac20bed96e815ba732c94ebf186899a1e`. The accepted `0.1.0-alpha.2` baseline is unchanged.

## Purpose
TOOLS-08 removes the arbitrary eight-call limit and uses verified progress. On process restart, a fresh in-memory `LoopGuard` must **never** reset the stagnation history and silently permit a stuck sequence to resume, nor mistake a model-authored statement of success for actual progress. Incomplete/uncertain effects must never be replayed.

## Existing integration surfaces verified from source
- `src-tauri/lu-core/src/loop_guard.rs`: `check`, `observe_feedback`, `ProgressEvidence`, recent attempts and seen semantic fingerprints; currently in memory only.
- `src-tauri/lu-core/src/agent_tool_loop.rs`: verified model boundary, registry Protobuf codec, current-turn authority, pre-dispatch loop guard, durable effect dispatcher, decoded feedback.
- `src-tauri/lu-core/src/journal.rs`: `EffectJournal` states PREPARED/EXECUTING/APPLIED/FAILED/UNCERTAIN; `reconcile_interrupted` marks in-flight operations UNCERTAIN; `get` returns stored encoded receipt.
- `src-tauri/lu-core/src/model_turn_journal.rs`: conversation/task-scoped model audit; `ToolFeedback` is audit evidence, **not** effect authority or a trustworthy write receipt.

## Proposed protocol (next implementation gate)
1. Trusted supervisor binds a progress session to authenticated profile, conversation ID, task ID, registry/codec identity and a host-issued execution epoch. Never accept these identifiers from generated model JSON. Require a single live owner before resumption.
2. In the **same trusted dispatch path**, persist each admitted proposal and its result as a monotonic sequence with canonical native tool ID, canonical argument digest, effect class, decision, and a receipt reference. For progress classification, use only trusted, validated, host-decoded Protobuf output and (where applicable) a journaled applied receipt; never rely on the outer model-facing feedback JSON alone.
3. Store a bounded, versioned checkpoint containing the most recent window of observed calls, stalled count, blocked count, recovery-attempt flag, and the capped seen semantic-result digests. The checkpoint must be associated with the last committed progress event sequence and digest. Store in a profile-private durable database using transactionally consistent checkpoint + event writes (SQLite WAL/FULL), with schema migration and size validation. No executable user-controlled deserialization.
4. **Crash gaps:** if a proposal was admitted but the dispatch/result commit did not finish, mark it unresolved. For a read, the host may initiate a separately authorized fresh read; for a write/exec, inspect the effect journal and verified postconditions first. PREPARED, EXECUTING, UNCERTAIN or absent/malformed receipts never count as a successful change and never imply automatic retry. An APPLIED state is not sufficient without a matching, validated effect-specific receipt.
5. On restart, load events only for the exact authenticated session; verify version, order, monotonically increasing sequence, event/checkpoint digest linkage and correlated effect-journal receipts. Any mismatch, missing material evidence, unavailable codec, inconsistent state or incomplete transaction leads to **halt pending reconciliation**. Do not rebuild from mutable model transcript or treat a resumed turn as a new progress session.
6. Rehydrate the original no-progress history and seen-result ring. A valid previously stalled session resumes still stalled; a valid session with independently verified new state may continue. Host task-deadline and resource budgets remain effective across restart; restarting does not renew their elapsed budget.
7. All authorized edits of the same file remain legal if each edit has its own committed before/after SHA-256 witness; same path and tool name alone are neither proof of looping nor proof of progress. Continue to enforce per-turn grants and per-effect authorization on every resumed call.
8. Generation token-stream state is **not** resumable mid-stream. Restart creates a new model-generation attempt after explicitly closing the old stream and reconciling its worker. Existing per-generation token/output bounds and live `StreamLoopGuard` remain independent.

## Acceptance tests required
- 80+ independently authorized same-file edits with distinct committed hashes; restart after the 40th; no arbitrary total-call ceiling.
- Repeated unchanged output, repeated failure and A/B no-progress cycles preserve the same block/kill decisions across restart.
- Crash immediately before and after every transaction boundary: admitted, effect PREPARED, EXECUTING, APPLIED, progress observation recorded, checkpoint committed, final answer persisted.
- UNCERTAIN effect and missing/malformed/incorrect Protobuf receipt: fail closed; no blind duplicate write or fabricated `StateChanged`.
- Tampered task/conversation, stale epoch, missing event, out-of-order sequence, stale checkpoint and mismatched digest: fail closed.
- Concurrent owner attempts cannot both resume a session.
- SQLite WAL recovery and explicit migrations on Windows and Linux; no corruption of existing model/effect journal rows.
- Actual supervised tool dispatch and CI; subsequently real Windows installed GGUF stream cancellation and crash/restart acceptance.

## Work split
TOOLS-08B.1: precise durable progress-event schema and migration, source-locked tests.
TOOLS-08B.2: instrument authorized native execution with transaction-ordered progress receipt linking; no model feedback as authority.
TOOLS-08B.3: rehydration and fail-closed recovery state machine, including unfinished effects and ownership lease.
TOOLS-08B.4: Windows/Linux CI crash fault-injection and real Windows desktop backend acceptance.

**Non-goals:** model-facing `loop_guard` tool; fixed eight-call limit; treating changed tool parameters or call IDs as progress; automatic retries of uncertain writes; promising indefinite resources.
