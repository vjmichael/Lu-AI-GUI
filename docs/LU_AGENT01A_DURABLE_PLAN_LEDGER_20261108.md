# AGENT-01A — Durable multi-step plan ledger (source candidate)

Date: November 8, 2026. Owner priority: main Lu Core AGENT-01 workstream. This is **not an execution grant, an autonomous agent scheduler, or final v1.0 acceptance**.

## Why now

The E1 external execution gates are still unproven in exact-head native Windows/Linux CI (especially P03 Windows Job Object, P07 actual GGUF tool choice, independent P10/P12/P13 sources and P08C Linux cgroup-v2). E1-P15 conversation recovery is also a separate draft PR #151. Rather than building an active agent runtime on their unaccepted guarantees, this candidate supplies the **independent, non-executing** AGENT-01 planning and checkpoint substrate.

## Implemented

- Added Lu Core module `agent_plan_ledger.rs`, with native SQLite `lu_agent_plans`, `lu_agent_plan_steps` and `lu_agent_plan_events`.
- `register` accepts a bounded 1–32-step typed plan, exact host-generated `p-...`, `c-...` and `g-...` IDs, and a live `TrustedUserTurnVerifier` over the **original saved human** conversation turn. Steps have one optional earlier dependency, which prevents cycles. A duplicate ID with different origin, step kind or dependency fails closed.
- Plans and steps have no executable command, model-supplied path, shell, operating-system PID, authority grant or credentials. Step labels are bounded enum values: collect source, inspect, analyze, independent check, human review, present.
- A host-only observation has its own 128-bit opaque event identity for idempotence and cannot be confused with a verified outcome. Exact event replays are idempotent; cross-plan or different-step reuse fails.
- An observation cannot proceed past an unmet dependency. Any explicit reconciliation warning blocks further observations; a separate cancellation intent also blocks progression **without pretending to terminate processes**.
- `reconcile_abandoned_observations` is explicitly callable only after exclusive old-host shutdown. It marks formerly observed steps as needing review. It does **not** establish old OS child termination or authorize replay.
- `snapshot` is scoped to the exact original conversation and human turn. It returns fixed-value `NOT_AUTHORIZED_FOR_EXECUTION`, never a model-facing "next runnable tool" command.
- Five production native Rust tests exercise authorization, idempotent durable planning, invalid/cyclic dependencies, crash/restart reconciliation, and cancellation intent. A sixth cross-module test uses the actual native ChatStore production source with a genuine stored human turn, then closes/reopens it.

## Required acceptance

1. Exact-head frozen-source Git validation and native Lu Core agent plan tests on Ubuntu 24.04 and Windows 2025.
2. Actual production ChatStore plus actual Lu Core plan ledger in the portable native-chat harness on both systems, including after restart, cross-chat denial and wrong-turn refusal.
3. Independent negative validation of duplicate IDs, step cycles, 33-step rejection, misplaced authorization, cancellation intent, and no auto-worker reruns.
4. Only after the genuine OS E1 execution and P15 recovery gates pass: connect verified worker receipts to the plan and separately review an explicit user-authorized **new turn** continuation protocol. No old call IDs may be replayed.
5. Installed Windows NSIS / AMD Vulkan / real GGUF / OS Job Object tests remain deferred until near v1.0 according to the owner's release schedule; they are **not waived**.

## Remaining AGENT-01 work orders

- AGENT-01B: canonical goal and assumption references, immutable step revision graph, dependency and expected-deliverable metadata; do not mistake model-generated plans for human authority.
- AGENT-01C: verified external-effect receipt join and host-exclusive plan ownership after native gates, monotonic checkpoints and independent outcome witness before step completion.
- AGENT-01D: actual safe conditional scheduler with explicit per-turn authority, resource ceilings, bounded iteration, human interrupts, and no automatic replay of uncertain effects.
- AGENT-01E: conversation progress cards, pause/interrupt/reconcile UX and installed Windows/Linux E2E, preserving all error/recovery history.

**Acceptance boundary:** a passing AGENT-01A SQLite suite proves only durable non-executing plan bookkeeping. It cannot certify external process lifetime, autonomous GGUF decisions, semantics of worker output or resumption safety.
