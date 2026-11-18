# MEMORY-LOOP-03A — Current human goal and historical recovery preflight

**Task family:** IMP-02 MEMORY-LOOP (P0).  
**Status:** source implementation candidate, **native / installed / real loaded-GGUF acceptance OPEN**.  
**Parent:** MEMORY-LOOP-02C draft PR #185, with its own still-open OS/installed-app acceptance.  
**Canonical design:** [K/I/A/F active execution roadmap](LU_KIAF_ACTIVE_EXECUTION_LEDGER_20261108.md), [memory-first policy](LU_HUMAN_CENTERED_AI_MEMORY_FIRST_20261107.md).

## Missing link being addressed

Lu's ordinary `lu_generate` already appends:
- M2 saved original ANSF goal and resolved/open clarification context;
- MEMORY-LOOP-02 independent historical effect/oracle and OS-custody outcomes;
- M3 bounded original-source lexical/semantic recall;
- optional model-created **inert** AGENT-01F plan draft and AGENT-01G one separately approved source operation.

Those elements are independently useful, but a smaller loaded model can misinterpret a deferred human answer as consent, a stored plan as a permission, a historical independent source verdict as *current* source freshness, or a verified source as proof the original OS descendants ended. The actual tiny local Qwen2.5 diagnostic scored zero of eighteen strict memory/recovery comprehension cases, motivating host-owned classification rather than more tool breadth.

## 03A implementation

`memory_loop03_goal_gate.rs` is a **pure** host policy. It receives only booleans/counts and fixed statuses derived by the native host, never raw recalled instructions or model-proposed paths:

1. Verify the currently enqueued generation ID is the original latest **host-authenticated human** ANSF turn and its trusted text exactly equals the current prompt.
2. Reopen the real M2 projected GoalSnapshot for this chat. Inspect whether a saved goal and revision both exist; count **pending** and **deferred** (not-now) questions separately. Superseded and answered questions do not block the current goal. Inconsistent, unknown or unreadable goal events fail closed; a conversation lacking a saved goal remains available for *general chat*, not authenticated plan review.
3. Reopen the independently host-selected 02C bounded history (real original reviewed TaskStore/EffectJournal, source-specific independent oracle, original OS Job/delegated-cgroup descendant witness). Use only the **host's** recovery classification and coverage flag. Unknown status or uninspected older human-turn history cannot be promoted to successful recovery.
4. Produce a small typed status object with distinct `goalStatus`, `historicalRecoveryStatus`, `conversationNextStep`, pending/deferred counts and `previousEffectsNeedReconciliation`. **No effect is dispatched.**
5. Add the JSON to the real next human `lu_generate` model input within a 2,300-byte envelope and existing 16 KiB model-prompt cap. A context-overflow fallback states that memory/history give no authority; the model can still converse normally. The model cannot change the host-classified statuses.

The bounded host fields use:
- `executionAuthority: NOT_AUTHORIZED_FOR_EXECUTION`;
- `currentSourceFreshness: NOT_RECHECKED`;
- `priorGrantReusable: false`;
- `automaticRetryAllowed: false`;
- `modelMayOverrideHostDecision: false`.

`draftReviewContextReady` is only a **nonexecuting model-context suggestion**: it does not create or approve a plan. Actual `planOnly` admission still independently checks the saved human goal, active clarification and final goal revision; real step execution still requires AGENT-01G's new native dialog, exact source and fresh per-turn authority. Zero generic tools or mini-apps are introduced.

## Submitted tests / evidence boundaries

- Nine pure Rust policy tests: authenticated current turn, absent goal/general chat, pending vs deferred, inconsistent goal, independent source conflict, OS unknown, missing original, bounded no-history and unknown host status. Rust tests are designed to compile **standalone with rustc**, without Tauri or engine mocks.
- Eight Node source-contract tests: actual `lu_generate` ingress, ANSF goal origin, real 02C original-journal retrieval, OS source distinction, closed no-replay status, context bound and negative cases.
- `.github/workflows/lu-memory-loop03a-goal-recovery.yml`: exact-head Windows 2025 + Ubuntu 24.04 standalone native Rust tests, Node, React bundle/check, Tauri Cargo tests and actual desktop compilation, with raw log artifacts.
- The Node contract assertion bodies were exercised within an isolated JavaScript verification harness and all eight passed before the exact-source freeze. This is **not** a native Rust pass or installed app observation.

## Open tests and following task groups

1. Independently inspect **actual** source SHA Windows/Linux policy compile, Tauri/SQLite/React jobs, resolve genuine errors and collect artifacts. No queued/pending job is proof.
2. Measure actual loaded Qwen3/Qwen 3.8 27B responses for missing goal, answered/pending/deferred, OS-unknown vs independent source conflict and recovery after restart. The host must still enforce authorization if the model misreads its capsule.
3. Demonstrate 03A with a genuinely installed Lu app and a real reviewable original ANSF human turn; observe that it is not falsely promoted to execution authority.
4. Follow-on **03B** should integrate independently verified *procedural* memory and context-change/belief-revision validity into the same source-traceable reasoning turn, without duplicating ANSF or granting old permissions. This is a planned distinct scope, not a completed feature.
5. Preserve independent 02C and AGENT-01L native kernel custody, current source recheck and physical NSIS/AMD Vulkan gates; defer Cognitum until Lu 2.0/3.0.

This is execution-layer correctness and better feedback for weak local models, not a new reasoning model or a new tool library.
