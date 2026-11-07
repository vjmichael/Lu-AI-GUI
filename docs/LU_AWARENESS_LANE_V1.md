# Lu v1.0 — AWARENESS-06 lightweight active observer and diagnosis handoff

**Status 2026-11-06:** Native policy, task-scoped notation and UI review/handoff **source implemented** on Observe & Learn PR #61. This is NOT a running second model, self-repairing agent or independent GPU worker. Exact-head Windows/Linux CI and installed desktop acceptance remain OPEN.

## Purpose

Lu may become **attentive and proactive**, within a user's approved scope, without keeping a 27B–35B transformer generating continuously. The smaller optional awareness model has ONLY three jobs: (1) notice a meaningful deviation from expected app behavior, (2) annotate a narrowly verified observation, and (3) prepare a structured handoff to the primary model and Lu IDE. It does not write source code, decide that a renderer hint is a verified bug, assign itself OS privileges or train itself.

### Runtime inside the existing Lu host

1. **Native event sensor (cheap)** — existing ObserveHost, window/app scope, future UI Automation adapter, app logs and independently verified test receipts. No LLM tokens for ordinary focus or layout changes. A user-approved temporary capture is distinct from a live model visual stream.
2. **Awareness decision lane (native)** — new \`native_awareness_lane\` bounded state machine, directly in Lu Core. It ignores ordinary state changes, deduplicates noisy hints, distinguishes untrusted React hints from OS accessibility evidence and trusted native test receipts, assigns \`observe\` or \`investigate\` priority, and suggests at most three brief small-model consultations and one main-model review per minute. Those are **recommendations only**, not executed inference.
3. **Notation (durable)** — new \`lu_awareness_notes\` table in the existing \`TaskStore\` SQLite database. Each row binds an \`obs-*\` task ID, exact conversation, monotonic event sequence, enum code/source/priority and timestamp; no screenshots, raw on-screen text, pixel array, window titles, file paths, arbitrary model output or prompt text. Writes require a RUNNING task with that exact ownership. A duplicate or cross-chat write is rejected.
4. **Small model (future supervised lane)** — when an actually installed small model/backend supports independent execution, the existing host supervisor may offer a bounded, optional, separately authorized prompt. It must not block an interactive user turn, require automatic loading of the main model, or silently consume its context. A dedicated small GPU/NPU is OPTIONAL; on a CPU, use lower frequency and measured thread/memory ceilings. This must be added by extending the **existing** inference supervisor and scheduler—not a second agent runtime.
5. **Escalation to large model** — a short typed \`Handoff\` exposes source session, note count, number of independently verified failures, number of unverified indicators and one fixed recommendation. It cannot include model-authored instructions or executable actions. In the current desktop pilot, \`Prepare large-model review handoff\` shows this summary for human review; it does **not** invoke the primary model. Actual low-priority dispatch through the main model queue requires user authorization and another gate.
6. **Repair through Lu IDE** — the main model may investigate source, tests and trusted native receipts through the already-authorized IDE. Every build/change/test/effect goes through its existing host broker and development ledger; there is no new shell or desktop-control grant.
7. **Verified learning** — only a separately approved and independently verified outcome may be proposed to existing ANSF memory. A persistent TaskStore awareness note is NOT automatically memory, training data, a defect verdict or permission.

## Critical privilege boundaries

- The default state remains OFF. A user must explicitly start Observe for the selected application; this does not grant screenshots, microphone, network, training, model execution, or computer control.
- React's \`error_indicator\` is a *hint*, never a test failure. A \`VerifiedTestFailure\` code is admitted only from a trusted native test receipt; the current renderer interface cannot emit it.
- The awareness lane never calls a model by itself; the host must affirm a small model was separately approved, loaded, idle and not competing with interactive work. Main-model reviews also need explicit permission and separately scoped tools.
- Native notations are read-only in current UX and bounded to the active chat/session. Stopping or switching chats revokes the in-memory decision lane. Note rows remain in the task ledger for review, not automatically in ANSF.
- Per-minute rate limits, deduplication and a 24-item volatile note cap limit attention spam. Notations cannot allocate unbounded memory or wake the GPU by virtue of an event.
- If a model reviews an observation and suggests a change, that remains a **proposal**. A screenshot or LLM confidence is not a valid PASS/FAIL receipt.

## Implementation in the current branch

- \`src-tauri/lu-core/src/native_awareness_lane.rs\`: pure Rust control with tests for default-no-inference, opt-in model recommendations, provenance denial, repeated hints, deduplication, chat isolation and revocation.
- \`src-tauri/lu-core/src/tasks.rs\`: additive \`lu_awareness_notes\` table plus scoped append/read APIs and persistence tests.
- \`lu-desktop/src-tauri/src/observe_desktop.rs\`: hooks into the current Lu-only observer, writes an enum-only note when the UI emits an error indicator, and prepares a fixed low-trust or verified handoff.
- \`lu-desktop/src-tauri/src/lib.rs\` and \`lu-desktop/web/main.jsx\`: read-only notes/handoff Tauri commands and a visible pilot review pane.
- Dedicated Linux/Windows native/desktop CI tests and source inventory, with results to be checked on the exact head.

## Next v1.0 gates

AWARENESS-07 — independently supervised small model runtime and device selection with real CPU/iGPU/NPU/dGPU benchmarks, VRAM isolation and source-attested model readiness. Existing single-model queue cannot yet host two concurrent inference sessions.

AWARENESS-08 — trust-bounded, short-input/short-output mini-prompt schema, no model-created authority; bounded output to fixed diagnostic categories, failure-injection and restart/cancel tests.

AWARENESS-09 — prioritized dispatch to main-model supervisor and Lu IDE context, user-visible review and intervention, cost/load limits, no unapproved background code execution.

AWARENESS-10 — independently verified GUI test receipts and actual Windows captured, redacted visual evidence. Distinguish intended UI appearance from actual operation and artifact hash. No default continuous video stream.

AWARENESS-11 — reviewed ANSF publication, correction/retraction, retention and deletion. No model-weight training from observation unless separately opted into an explicit export/training flow.

**Release status: NOT READY** until actual second model scheduling, native perception, Windows and Linux CI, and installed desktop acceptance are verified.
