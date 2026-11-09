# MEMORY-LOOP-02 — independently verified outcome feedback to the next reasoning turn

**Started:** November 8, 2026. **Base:** draft MEMORY-LOOP-01 PR #177, stacked on AGENT-01L. **Status:** source candidate requiring native and loaded-GGUF acceptance.

## Goal and scope

Instead of requiring the AI to rediscover the result of each independently inspected Git/media operation, the **next authenticated human Lu conversation turn** can see a narrow, host-derived evidence summary. The model is an interpreter of historical execution facts; it never gets authority from that summary.

### Source-of-truth pipeline

1. **K / human origin:** the existing normal Send path durably appends the new human ANSF event and confirms it remains the latest authenticated original. Background auto-continuations, imported JSONL, Reader events, model text, screenshots and retrieved documents cannot select the source. No copied memory transcript is used to discover operation IDs.
2. **M / original ANSF index:** `AnsfStore::recent_authenticated_turns_before` selects only earlier `kind='user'`, `origin='host'` records with original source entry IDs, in descending event order, before the exact current human event. Limit 12 prior human turns; never scan a guessed filesystem journal directory or accept a model-selected path.
3. **I / reviewed native evidence:** for those turns, the already built read-only `reviewed_turn_continuity` reopens the **exact original** per-turn native TaskStore, effects, immutable receipt bytes and second independent outcome oracle, validating original conversation identity, task completion, outer model call link, allowed oracle method and matched receipt SHA. A verified semantic observation carries its independently observed 64-character SHA-256 digest; old raw worker output, stdout, prompts, arguments and local resource paths do **not** enter the feedback.
4. **F / OS custody distinct from outcome correctness:** check the original call's Win32 kernel Job empty witness or delegated Linux cgroup-v2 populated-zero receipt separately through `HostOriginalJobCustody`. `SOURCE_VERIFIED_OS_CUSTODY_UNPROVEN` is not promoted to full verified custody. A proven historic OS empty observation is **not proof that the Git repo/file is still unchanged**. Conflicted, unsupported, partial and uncertain statuses remain explicit.
5. **A / no inherited permission:** a 3,400-byte maximum JSON data-only source projection (two most recent reviewed turns; up to three effects each) enters the existing loaded GGUF's *next* normal human turn. It always says `NOT_AUTHORIZED_FOR_EXECUTION`, `priorGrantReusable=false`, `automaticRetryAllowed=false`, `historicalEffectMayBeReplayed=false`, `currentSourceFreshness=NOT_RECHECKED`. To execute, Lu still requires current ANSF goal and source rechecks, new native resource review, human authorization, host-owned scheduler claim, independent effect oracle, and OS custody.
6. **Crash/restart:** reconstruct the data from original ANSF and native journals, never an in-process grant or cached model response. If original evidence is unreadable or changes, emit a neutral `FEEDBACK_UNAVAILABLE_RECONCILE` boundary and ask for manual reconciliation. If no earlier reviewed operation exists in the bounded recent window, inject no feedback at all.

### Evidence distinction

- `HISTORICALLY_VERIFIED_WITH_OS_EMPTY_WITNESS`: previously **independent semantic and OS checks** match the original saved effect. Source freshness is still NOT_RECHECKED, never a current operation grant.
- `SOURCE_VERIFIED_OS_CUSTODY_UNPROVEN`: source outcome was independently checked at observation, but OS process-tree termination is NOT independently established; future execution stays blocked until a new decision.
- `INDEPENDENT_SOURCE_CONFLICT`, `INDEPENDENT_SOURCE_UNSUPPORTED`, `UNCERTAIN_OR_UNVERIFIED`, `PARTIAL_TASK_REQUIRES_RECONCILIATION`: no basis for declaring completed verified results.
- `FEEDBACK_UNAVAILABLE_RECONCILE`: journal/ANSF read failed; do not silently turn absent evidence into success.

### Acceptance tests (source added, not claimed passed)

- Real ANSF original-turn index and cross-chat controls.
- Real task/effect/independent verdict SQLite close/reopen before the next human model turn.
- Tamper the independent receipt digest and verify the follow-up source label downgrades; fail closed when OS custody witness is absent.
- Malformed, imported, cross-chat, unknown, missing and stale turns cannot create/borrow a worker permission.
- Eleven independent Node source architecture tests, targeted real Windows/Linux desktop Rust and React/Tauri compile gates.
- Actual loaded local Qwen observation of feedback and calibrated explanation, followed by installed Windows/Linux UI and cgroup/Job acceptance. All genuine native/GGUF/physical OS gates remain OPEN until independently inspected job logs.

**Boundaries:** This is foundational memory + feedback, not a new tool, auto-retry/replay runner, general document search, solved current-source freshness or permission to continue a plan. It does not weaken AGENT-01K/01L uncertainty and native Job/cgroup acceptance gates.
