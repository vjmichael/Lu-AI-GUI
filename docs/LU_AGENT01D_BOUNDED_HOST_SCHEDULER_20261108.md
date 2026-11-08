# AGENT-01D — Bounded, host-supervised scheduling decisions

**Date:** November 8, 2026. **Base:** AGENT-01C PR #154. **Status:** Draft source/native test candidate. This work deliberately does not broaden unaccepted external-execution guarantees.

## User goal

An AI can help translate an idea into small, independently reviewable tasks. Lu should know which step is ready, why it was selected, how much work it is allowed to request, which human approved it, and why it must stop. It should also survive a crash without accidentally completing or repeating work that was never proven.

## Implemented source

`src-tauri/lu-core/src/agent_host_scheduler.rs` provides a native SQLite **decision and observation coordinator**, NOT a task or OS process launcher.

1. **One host per plan.** A host may claim only a reviewed, source-current AGENT-01B revision and authenticated AGENT-01A original saved human turn. An immutable `lu_agent_schedule_owners` row locks the exact plan, owner, revision SHA and epoch. A second owner is refused. There is no heartbeat expiry and no automatic new-owner takeover.
2. **Live-process ownership fence.** A persisted host ID is not sufficient on restart: a separate in-memory owner registration exists only in the original `AgentHostScheduler` instance that successfully created the session. A reopened process may read the ledger or quarantine an abandoned session using an independently verified stopped-host witness, but **cannot reclaim even the same owner ID** to schedule more steps.
3. **Fresh human consent per step.** Each proposed `q-<32hex>` step must be tied to a distinct, genuinely saved `g-<32hex>` human turn and a separate host-only `HostFreshStepConsent` witness of the exact plan, revision, step and budget. The original plan turn or review turn cannot double as new action consent. These verifiers have **no production desktop/WebView implementations yet**.
4. **Bounded resource reservations.** Max 8 proposals per plan, 8192 requested model tokens, 8 requested tool calls, 120 seconds of requested wall time and 8 MiB of requested output bytes **per lifetime plan reservation**. A step can reserve less, but never more than the session budget or remaining aggregate budget. At most one proposed step can be outstanding. These are **scheduler quotas only**, not actual CPU, kernel, process-memory, or OS job limits. Enforced runtime resource accounting is a separate E1/AGENT-01D later gate.
5. **Dependency safety.** A predecessor cannot be passed merely because AGENT-01A marked it `OBSERVED_UNVERIFIED` or a model said it finished. It needs `OBSERVED_VERIFIED_HISTORICAL` in the current scheduler and a **fresh** read of AGENT-01C's independently verified original task/effect/receipt evidence. If a previously verified source changes, the next proposal is rejected. All source/history checks must be repeated by a future actual worker admission broker.
6. **Durable proposal identities and no retries.** One step, one host-generated claim ID, one human consent turn; complete budgets and original revision are immutable SQLite fields. Unique constraints and database triggers block replay under another deliverable, rewrite of owner identity, partial source/budget mutation, deletion of history, and illegitimate state transitions. An identical proposal is idempotent only in the same live session.
7. **Historical outcome acknowledgement only.** `acknowledge_historical_receipt` must find an independently checked AGENT-01C deliverable for that exact revision and step and reverify it against the original source. The state `OBSERVED_VERIFIED_HISTORICAL` **does not mean an OS child is reaped or a future step may execute**. A new, separately consented step proposal can be considered only when every required dependency has this fresh historical evidence.
8. **Stop/recovery.** An explicit host cancellation records `CANCEL_REQUESTED`, marking pending proposals `CANCEL_INTENT_UNCERTAIN`, without claiming to kill or reap anything. A caller with an independent **host-process-stopped** witness can quarantine `RECOVERY_REQUIRED` after restart; pending proposals become `ABANDONED_UNCERTAIN`. This is not proof that descendants stopped. The old session is never rearmed, even by its former owner; a new plan and new authorization are required.
9. **Read-only progress and clean release.** Scoped `view` returns only host-owned fixed-format plan/step references and normalized states with `NOT_AUTHORIZED_FOR_EXECUTION`. Release is allowed only if no unresolved proposals remain; released plans are terminal. No execution token or shell command can be constructed from the view.

The module has no call to OS subprocess launch, `ToolRegistry`, `TaskStore::resume`, any model inference backend, arbitrary filesystem path tool, mandatory MCP or Docker. Lu Core itself is not sandboxed.

## Actual native regression requirements

The dedicated `.github/workflows/lu-agent01d-host-scheduler.yml` requires an exact, frozen source head on **Windows Server 2025 and Ubuntu 24.04**, and executes the actual production Rust SQLite modules:

- One original host claims a durable plan; other owners are denied, including after process reopen with the same persisted owner ID. SQLite ownership rows cannot be rewritten or deleted.
- A new human consent turn and exact budget are required. Incorrect user turn, no explicit approval, oversized single-step requests, changed step identity and unverified dependencies fail closed.
- Duplicate cancellation is idempotent on the original live host; cancellation and host-process-stop quarantine do not falsely claim old workers have ended. Process restart cannot silently recover scheduling authority.
- A superseded goal or changed plan revision invalidates the previous schedule.
- Absent or contradictory independent witness cannot complete or release a step; original real SQLite TaskStore/EffectJournal test **fixtures** exercise the join.
- Altering the original independent witness after historical acknowledgement blocks a dependent step even if the scheduler's cached past status said it was verified. Aggregate budget exhaustion rejects a second proposal even if each individual reservation is within per-step caps.

Native source/SQLite tests are genuine but the worker effects are **synthetic fixture receipts**. The workflow also reruns AGENT-01A/B/C native unit suites at this current stacked source head. A passing test never certifies process containment, actual Git/FFprobe, GGUF-selected tools, or a physical installed Windows release.

## Already confirmed OS and CI blockers (checked this task group)

| Earlier gate | Last observed actual failure | Current proof |
| --- | --- | --- |
| E1-P03 Windows worker | 5/6 failed `WorkerStartFailed` from original lower Job Object memory bound | Fixed in source; repaired exact-head Windows job queued |
| E1-P07 Windows/GGUF | Missing FFprobe executable during preflight; no actual real GGUF choice executed | Added prerequisite in source; current native GGUF acceptance queued |
| E1-P09 Windows audit | 3/3 Rust tests passed but workflow incorrectly demanded 4 | Workflow check adjusted; current native job queued |
| E1-P10 packed Git refs | 2/3 passed; direct packed Git HEAD oracle Conflict instead of Verified | Parser source changed; current Windows/Linux retest queued |
| E1-P08C delegated cgroup-v2 custody | No confirmed kernel-level delegated descendant-stop acceptance | Separate Linux gate OPEN |
| E1-P12/P13 independent source/revision | Exact-head Windows/Linux native oracle and source-mutation tests queued | Not accepted |
| E1-P15 conversation recovery | Exact-head Windows/Linux native and React evidence jobs queued | Not accepted |

## Required next integration gates

**AGENT-01D runtime admission phase:** only after E1-P03/P06/P07/P10/P12/P13 and P15 actual native checks have passed and reconciled, add a host-owned admission broker that rechecks the saved human request, source revision, per-turn authority, executable digest, OS Job Object/process-group/cgroup process custody, aggregate execution cost and immutable effect receipt. The broker—not this scheduler—must bind an approved claim to real per-turn privileges and report both application and descendant completion. No uncertain old effect ID may be reused.

**AGENT-01E:** desktop goal/step progress cards, approval/rejection UI, cancellation and recovery timelines, real multiple-conversation navigation, and physical NSIS Windows installed-app acceptance near v1.0. This candidate intentionally does not implement a production approval UI or active tool scheduling.

**Do not merge or claim OS execution acceptance from source-only work or fixture tests.**
