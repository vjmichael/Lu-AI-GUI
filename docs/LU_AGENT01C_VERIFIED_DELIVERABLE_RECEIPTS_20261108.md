# AGENT-01C — Independently checked historical deliverables (draft)

Date: November 8, 2026. Parent: AGENT-01B draft PR #153. **Source/test candidate only.** This is not an autonomous scheduler or a production-accepted OS worker.

## Contract

The purpose is to tie an expected output from a **human-reviewed, immutable AGENT-01B revision** to the exact, already-performed, host-approved application effect that actually produced it. An independently recorded semantic witness must agree with the original retained OS receipt. No model reply, unchecked process exit code, claimed PDF/MP4 filename, old plan approval, or merely APPLIED SQLite state can assert that a deliverable has succeeded.

The host registers a receipt through `AgentDeliverableReceipts::record_observed`; the user interface may later use `history` for a **read-only** overview. The API deliberately contains no execute, run, spawn, retry, resume, scheduler, or auto-continue call.

## Admission rules

1. The host verifies the original `p-...` plan, `c-...` conversation and `g-...` saved human turn using the real AGENT-01A scoped SQLite plan. Cancel-requested / needs-review plans cannot receive new witness links.
2. The expected revision SHA must exactly match the **latest AGENT-01B revision**, whose separate human review status is `APPROVED_PLAN_ONLY`. The referenced ANSF goal event must still be current and all assumptions still accessible through a host-only source verifier. Any change requires a fresh review.
3. The target step must declare `INDEPENDENT_CHECK` and `INDEPENDENT_WITNESS`; it cannot be a random planned artifact, text answer or tool permission. Each independently checked effect can back at most one deliverable for one original turn.
4. The host selects the profile directory, not a model-provided path. The module derives the exact task and effect SQLite filenames from the original human turn. Only actual ordinary files within a bounded size are accepted; symlink/reparse journals and linked sidecars are refused. Both journals are opened **read-only** so a lookup never creates fake evidence.
5. The **outer model call** must exist in the original effect journal with the exact typed reviewed-tool ID and `APPLIED` state; the **inner reviewed worker call** (using one of the reviewed suffixes `-gh`, `-gs`, `-gb`, `-gc`, `-fm`) must also be `APPLIED` with a retained bounded receipt.
6. The inner worker's real `lu_tasks` row must have `COMPLETED` state and be scoped to the same original human turn. This is a **journal observation**, not proof that all worker descendants have been terminated.
7. The independent observation for the same inner worker call must have `VERIFIED` verdict, a method compatible with that exact Git/FFprobe operation, a well-formed independent source digest, and a receipt SHA-256 that matches the actual retained inner receipt.
8. The ledger writes a unique, immutable delivery link and complete exact hashes/method IDs into the original AGENT-01 plan database. SQLite UPDATE and DELETE triggers refuse replacement; identical duplicate delivery is idempotent, while changed hashes or reuse of one effect for another output is rejected.
9. After restart the historical view recomputes that match by reopening original journals. If the source has been superseded the old witness is marked `SUPERSEDED_HISTORICAL_WITNESS`; if the original receipt, independent observation or live ANSF source check is missing/changed it is `NEEDS_RECONCILIATION`. A historical, still-matching witness is `VERIFIED_AT_OBSERVATION` **only**.
10. Every view returns `NOT_AUTHORIZED_FOR_EXECUTION`. No writable task progress, “step complete” state, new process, or valid current OS reaping/cgroup protection is inferred from the witness.

## Native acceptance

Dedicated `.github/workflows/lu-agent01c-deliverable-receipts.yml` requests exact-head Git frozen source verification and real production Lu Core SQLite tests on Ubuntu 24.04 and Windows 2025. Test fixtures are **not actual Git/FFprobe launched processes**; they exercise the genuine `TaskStore`, `EffectJournal`, `AgentPlanLedger` and `AgentPlanRevisionStore` APIs with deliberately synthetic receipts.

Tests include honest historical receipt reading across SQLite close/reopen; idempotence; cross-chat refusal; missing independent outcome; right-hash/wrong-method refusal; independent-source digest tamper and refusal to silently change an old witness; cancellation intent; stale ANSF goal check and a linked journal refusal on Unix. Each OS runner must publish actual named-test evidence and source SHA before this limited scoped candidate is accepted.

## CI status reviewed November 8, 2026, approximately 13:20 Phoenix

Older **completed** failures were inspected directly in GitHub Actions logs:

| Gate | Actual older outcome | Failure / remediation |
| --- | --- | --- |
| E1-P03 real Windows worker, run 42442233765 | 1/6 passed, **5 failed**, Linux 6/6 passed | All five failing Windows tests returned `WorkerStartFailed` before reliable worker execution. The old 512 MiB Job Object minimum conflicted with the 256 MiB request. Current PR #105 changed the minimum; exact-head native Windows run 42505918911 is queued, **not accepted**. |
| E1-P07 Windows reviewed apps, older job 128797679598 | **FAILED before app tests** | Missing `ffprobe` on PATH. Current workflow added CI provisioning; exact-head P07 real-run 42508911198 and genuinely loaded GGUF choice 42508911271 are queued. |
| E1-P07 genuine GGUF choice, older job 128792548671 | **FAILED before model test** | Preflight `command -v ffprobe` exited after Git succeeded; no genuine loaded model chose a tool. Later native provisioning is source-only until queued CI passes. |
| E1-P09 Windows native review-audit, run 42500441633 | **3/3 Rust tests actually passed, workflow FAILED** | Workflow incorrectly required at least 4 tests on Windows (fourth was Unix-only), so follow-on Windows desktop job skipped. Updated expected count is 3 Windows / 4 Linux on newer heads; exact-head still queued. |
| E1-P10 Windows independent Git/WAV oracle, run 42500688360 | **2/3 passed, 1 failed** | `direct_git_head_ref_and_packed_ref_independent_of_executable` incorrectly returned Conflict rather than Verified on legal multi-line packed refs. Reader fixed to handle lines in later source; exact-head Windows/Linux run 42512258155 and P13 run 42512518555 queued. |

Other P06B/C/D/E native execution, E1-P08C delegated cgroup-v2, P11 history, P12 MP4/status, P13 source revisions, E1-P15 installed continuity and AGENT-01A/01B exact-head CI are likewise **queued or pending**, not accepted. A green REPO-01 source lock on an ancestor only proves Git blob identity for that ancestor, not process safety or GGUF behavior.

## Unresolved limitations and follow-up

- The plan, task and effect SQLite files are queried separately, not in one atomic cross-database transaction. The host must provide exclusive session ownership/fencing before invoking the future execution scheduler. Existing immutable observations are revalidated on every read; no execution permission results.
- Current source digests describe the **observation time**, not a persistent filesystem snapshot. Supported Git/MP4/WAV validation methods are limited; every other app or output remains outside AGENT-01C until it has its own reviewed independent semantic validator.
- This is a **host API and Rust SQLite candidate**, not a wired desktop approval workflow, real GGUF decision, native third-party OS admission acceptance, installed NSIS acceptance, or Linux escape-capable cgroup-v2 result. Do not promote it to v1.0 success solely on source tests.
- AGENT-01D: bounded, owner-fenced scheduler, fresh human authorization, verified OS process cleanup, precise resource/revision rechecks, and no replay of uncertain call IDs. Start active worker control only after P03/P06/P07/P10/P12/P13 and P15 native gates close. E1-P08C strong Linux descendant custody is a separate gate for escape-capable tools.
