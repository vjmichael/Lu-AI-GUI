# MEMORY-LOOP-03B — Verified procedural memory, belief revision and CCM in real Lu reasoning turns

**Status:** implemented source candidate; exact-head Windows/Linux Rust/React native CI, installed Lu and loaded-Qwen acceptance **OPEN**.  
**Parent:** MEMORY-LOOP-03A draft PR #187, itself stacked on 02C #185 and 02B #182.  
**Priority:** IMP-02 (P0) of [Lu's K/I/A/F active execution ledger](LU_KIAF_ACTIVE_EXECUTION_LEDGER_20261108.md).

## Motivation and current boundary

Lu already stores M4 verified procedures in the original ANSF SQLite book, reported/verified/stale/contradicted/retracted claims, and immutable M3 task checkpoints with a Context Change Monitor (CCM). The normal GGUF Send also has the new 03A current-human goal/clarification/recovery gate.

But the original M4 desktop belief read always passed `|_|false` as its second-witness predicate. That was safely conservative yet prevented actual independently witnessed claims from ever being recognized as historical verified evidence in the normal model-turn reader. The original M4 procedure recall was core-only; its version-specific records could not reach the same GGUF reasoning turn.

03B closes the **source integration gap**, without turning remembered text into native instructions or approval.

## Implementation: original evidence, not invented authority

1. **Historical second-witness validator:** New `EffectJournal::independent_verified_receipt` joins the original `lu_effects` and `lu_independent_outcomes`, rehashes the actual APPLIED receipt, compares exact call/tool identity and the checkpoint's stored receipt SHA-256, checks the method-specific independent outcome against a narrow reviewed Git/media mapping, and requires a properly formatted independent observed-source digest. A model, WebView, old prose, mere APPLIED state, unsupported method, contradicted verdict or missing receipt cannot satisfy this check. This is a **historical** witness; it does not recheck a live Git repository or assert Win32 Job/Linux cgroup-v2 process custody.
2. **Exact-version verified procedures:** New `ChatStore::m4_procedure_context` reuses `AnsfStore::reusable_verified_procedures` (original source-current revision + checksum, latest matching source-linked task checkpoint, absence of blockers, identical journal evidence, immutable invalidation and successor filters, exact tool + tool-version + application-version). It attaches a second independent host witness, not a Boolean supplied by the model. It returns only approved original source IDs, hash identifiers, exact versions, bounded prerequisites and historical steps/failure conditions; no process handles or executable tool invocations.
3. **Belief revision:** Existing `ChatStore::m4_belief_context` now invokes the same independent receipt verifier rather than always returning false. The original ANSF belief engine still distinguishes `reported_unverified`, `independently_verified`, `contradicted_human_review_required`, `stale_not_current`, `retracted_not_current`, `superseded_not_current`. Conflicting live values do not get ranked as a winner; the host labels them for human reconciliation. Future corrections do not delete original claims.
4. **CCM:** 03B reopens the original source-bound `ContinuityStore` checkpoint and existing `monitor_project_context`. Both original conversations must belong to the current host profile; changed source IDs, changed content/next action, added/cleared blockers, no comparable record or an unread original source require explicit review. A historically unchanged comparison still does not certify the current world state.
5. **Same actual GGUF turn:** New `memory_loop03b_context.rs` consumes only the just-enqueued host-authenticated current human turn, calling the existing source readers and the new pure seven-case `memory_loop03b_policy.rs`. It adds a fixed source-scoped status capsule to the same `generate_internal` model prompt as 03A's goal/recovery gate, carrying `currentGoalGate`, `historicalRecoveryGate`, `status`, original source/checkpoint IDs, uncertainty/truncation and non-executable bounded details. The loaded model can discuss evidence or propose a fresh plan, **never** authorize/retry the old effect.

## Exact human selectors, not model-generated tools

The original human can request context in Lu's existing normal chat:

```text
Recall procedure project <project> task <task> tool <tool_id> tool-version <version> application-version <version>
Recall belief project <project> task <task> subject <subject> predicate <predicate> version <version>
Resume project <project> task <task>
```

The procedure selector is new and requires exact versions (no automatic guesses). The belief and resume selectors already existed; 03B now adds a common host-triaged read-only context alongside their existing M3/M4 historical detail. The host compares the **actual latest original human turn and exact text**, not any archived passage or model-supplied identifier. A missing or invalid selector results in ordinary conversation, not arbitrary ANSF scanning or permission.

Each selector has explicit strict bounded identifiers; returned original data is capped, and omitted steps/source IDs are labeled. Historical procedure source hashes, receipt hashes and verified observation are not live source freshness, current authority, or installed-app proof.

## Security invariants

- Lu Core's three capability classes remain **Foundational / Utility / Specialist**. No fourth tool class, new mini-app, background automation, second memory store, model-selected path, or mandatory MCP.
- ANSF remains the authoritative original; derived M4/CCM records may be *read* but no observed historical instruction creates a new user event or effect.
- `executionAuthority="NOT_AUTHORIZED_FOR_EXECUTION"`; `currentSourceFreshness="NOT_RECHECKED"`; `priorGrantReusable=false`; `automaticRetryAllowed=false`; `modelMayOverrideHostDecision=false`.
- A source has to be correct at observation **and** pass the separate independent second witness to be called historical verified. The OS descendant witness is a **different** gate, not inferred from a valid SHA.
- Invalid/missing verifier, conflicting beliefs, omitted evidence or changed/unknown original source never becomes a success claim. The underlying AGENT-01F/G per-turn native approval and worker/oracle execution paths remain unchanged.
- **03A dominates 03B:** even if an old procedure or claim was independently verified, 03B sets `currentTaskReviewBlockedByPreflight=true` and `humanReviewRequired=true` whenever the current saved-goal/clarification gate or historical original source/OS-custody gate is not reviewable. Missing or unknown 03A labels fail closed; a historical memory snapshot never overrides host recovery.

## Source and native acceptance ledger

**Submitted:**
- Genuine Lu Core SQLite journal test: verifies absent witness, APPLIED-only refusal, valid method+receipt+observed digest, reopen, tampered receipt digest, wrong tool, and wrong method.
- Reused genuine source M4 procedure and belief-revision native tests; plus new 7 pure native Rust host policy and 6 desktop context unit tests.
- 11 Node source-contract cases covering integration in the normal `lu_generate` path, source binding, closed policy, exact versions, contradiction/CCM/truncation and non-grant.
- Targeted Windows 2025/Ubuntu 24.04 workflow runs pure Rust, real Core ANSF/journal/receipt tests, desktop Rust, React UI build/check, and frozen source verification. Source-contract assertion bodies evaluated locally before freeze: 11/11 passed. **These do not substitute for actual OS job logs.**

**Still OPEN (do not assert success):**
- Real exact-head Linux/Windows rustc + Cargo test + Tauri/React compilation and frozen manifest.
- Genuine loaded model reading exact verified procedures, changes to application/tool versions, corrected/retracted claims, contradictory values, CCM and missing independent witnesses; benchmark it against model-only cases without declaring the model's answer verified.
- Real installed Lu with user interaction, restart, model swap and provenance UI evidence.
- Current external source freshness, positive Windows Job/SCM/Linux delegated-cgroup descendant cleanup, full AGENT-01G fresh authority and safe sequential effects.
- Performance of source recall and context budgets on the intended Qwen3.8 27B configuration.

**Next:** 03C genuine model comprehension and error-attribution benchmarks with independent host policy comparison; 02C/03A native and installed acceptance should continue independently. Cognitum refactoring remains deferred to Lu 2.0/3.0.
