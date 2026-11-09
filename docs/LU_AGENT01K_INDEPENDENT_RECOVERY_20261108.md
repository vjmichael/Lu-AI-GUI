# AGENT-01K — Independently reconstructed recovery decisions (draft)

Stack: AGENT-01K on AGENT-01J PR #170, with NSIS and Qwen CI heads on earlier PRs preserved. **This is an audit/decision layer, not an auto-resume capability.**

## Problem and explicit boundary

AGENT-01I/J can show historical parent/child approval, model execution and ancestry, but an old SQLite status or a model's answer cannot establish that an operation succeeded, a child process tree ended, or a grant remains valid. We must reconstruct the original authority and native evidence independently, and never mistake a database reconciliation for process termination. Cross-database observations are **not an atomic OS snapshot**; they cannot mint future execution authority.

## Production source in this candidate

- `lu-core::agent_recovery_decision::reconcile_recovery` verifies authenticated original plan origin and approved immutable revision, current ANSF goal/source references, and read-only AGENT-01J ancestry. It revalidates each linked ancestor through AGENT-01I's existing current parent source/claim/oracle gate (bounded 32 generations), not merely the first parent.
- It queries the existing AGENT-01D scheduler to distinguish no owner, still-owned, cancelled, quarantined, stale revision, and cleanly released records. It compares each released claim to exactly one historical `AgentDeliverableReceipts::history` record from the exact fresh human execution turn. It refuses mismatched turns, duplicates, missing receipts, stale or changed independent oracle evidence and unsafe owner states.
- Historical receipts are independently recomputed by AGENT-01C from original real task/effect SQLite and the original worker receipt SHA-256/independent outcome; no model narrative or `APPLIED` effect alone is considered proof. `HistoricalDeliverable.execution_turn_id` now exposes its actual journal turn for matching against the claimed fresh owner.
- OS descendant termination is an **independent additional requirement**. A new host-only `IndependentProcessCustody` witness interface is reserved for independently checked Windows Job Object and Linux guardian/cgroup process evidence. **The installed host deliberately passes `NoIndependentCustodyProof`, always false.** This prevents reporting complete process custody before a real verified provider is connected. Even a trusted custody proof yields `REVIEW_NEW_STEP_WITH_FRESH_AUTHORIZATION_ONLY`, never a worker/tool/model grant.
- New `open_existing_readonly` constructors for ledger, revision, scheduler and receipts avoid journal mutations, DDL and owner acquisition during a recovery inspection.
- Native `lu_agent_reconcile_recovery` selects the host profile and authenticates the chat. The React activity pane adds a user-initiated **Review independent recovery evidence** button; shows each independently observed step, descendant-custody uncertainty and next human action; clears a prior observation before inspecting again. No WebView-supplied paths, permissions or success claims are accepted.

## Status semantics

- `REVIEW_FIRST_STEP_ONLY`: approved plan with no owner/receipt history; still needs a fresh human operation review.
- `STOP_HOST_STILL_OWNED_OR_UNKNOWN`, `STOP_CANCEL_UNCERTAIN`, `STOP_ABANDONED_HOST_CUSTODY`: worker ownership uncertain; inspect real process custody before any next step.
- `STOP_UNVERIFIED_WORKER_ORACLE`, `STOP_MISSING_OR_EXTRA_WORKER_RECEIPTS`: original native result could not be independently corroborated.
- `STOP_ANCESTOR_ORACLE_UNVERIFIED`, `STOP_LINEAGE_RECONCILIATION`, `STOP_GOAL_OR_SOURCE_CHANGED`: source-backed continuation context no longer agrees.
- `STOP_ANCESTOR_CUSTODY_UNPROVEN`: even if the child plan has no claims or a verified result, any parent/grandparent lacking independently attested descendant termination blocks safe continuation review. This is the installed-host default for linked child plans until AGENT-01L connects real OS custody.
- `STOP_DESCENDANT_CUSTODY_UNPROVEN`: historical worker evidence rechecked, but no independently verified full descendant termination (current production default).
- `REVIEW_NEW_STEP_WITH_FRESH_AUTHORIZATION_ONLY`: only when every historical result and every required host OS custody witness pass. This **still returns `NOT_AUTHORIZED_FOR_EXECUTION`** and cannot be used to replay an old task.

## Test plan / current acceptance

- Four Lu Core native recovery-state tests exercise missing custody, independently asserted custody with *review only*, rebinding/withholding an effect, and active/cancelled/recovery-required owners. A separate actual SQLite integration fixture reopens independent task and effect records, withholds the effect file after restart, verifies that missing evidence invalidates the decision, and tests changed goals and released owner replay denial. The fixture's worker receipts are synthetic; it proves SQLite joining, not OS execution.
- Nine Node source-contract checks validate production wiring, truly read-only file openings, no-bypass properties, UI and the genuine model test extension.
- The existing **real pinned-Qwen3 E1-P07 AGENT-01I two-tool-turn** acceptance test now checks AGENT-01K recovery results after the actual worker/effect/oracle reopen: both steps are historically verified, OS descendant custody is unproven, hiding the grandparent worker's effect journal blocks the child as `STOP_ANCESTOR_ORACLE_UNVERIFIED`, and superseding ANSF blocks continuation.
- Windows 2025 / Ubuntu 24.04 exact-head workflow requires the native Rust/SQLite tests. Windows additionally compiles the real desktop/React host, with explicitly marked engine build placeholders. The existing pinned actual-Qwen workflow must run separately. GitHub queued tests are NOT evidence of a native pass.

## Next engineering work

AGENT-01L must integrate an independently tested, durable OS custody witness into the actual installed Windows/Linux protected reviewed-worker termination path; a mere process ID disappearance, timeout, TaskStore terminal state or supervisor reinitialization does not prove descendants ended. Only then may Lu classify a fully verified historical run as eligible to **ask the human** for another distinct authorization. Physical NSIS acceptance and genuine loaded Qwen must still be independently accepted. **Automatic multi-step/replay remains disabled.**
