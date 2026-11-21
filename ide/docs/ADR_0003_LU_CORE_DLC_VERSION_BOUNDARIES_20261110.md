# ADR-0003 — Lu Core / DLC boundaries and phased integration

**Decision date:** 2026-11-10 UTC (owner-directed documentation).  
**Status:** Accepted *roadmap boundary*; detailed v2 transport/permission contract remains **proposed / NOT authorized**.  
**Applies to:** Lu Core and independently built IDE, Film/NVP, Research, Writing and other DLCs.  
**Related:** [ADR-0001](ADR_0001_STANDALONE_APP_AND_LU_BRIDGE.md), [IDE architecture](ARCHITECTURE.md), [release phase ledger](IDE_RELEASE_CLOSEOUT_P01_P07_20261107.md).

## Why

Lu Core and the specialist DLCs were intentionally developed concurrently but independently because neither their eventual integration surface nor their shared capability ownership was known. Asking what makes an IDE professional did not require replacing a lightweight editor with an independent platform. The developed professional-grade IDE capabilities should be finished and assessed before proposing later scope reductions. This record prevents accidental pre-v2 coupling while preserving valuable native work.

## Product versions — release intent, not retroactive acceptance

| Product boundary | Intended result | IDE / DLC integration rule |
|---|---|---|
| **Lu v1.0** | Lu Core complete against its own planned acceptance criteria: desktop shell, local model/runtime, reasoning/planning, native tools, memory, authorization, verification and recovery | **DLCs remain standalone.** No live Lu-to-DLC bridge is a v1.0 dependency |
| **Lu v2.0** | Independently reviewed, versioned interfaces integrate the planned DLCs into Lu as optional specialist workspaces | Allow connection only after separately accepted peer identification, capability discovery, user consent, OS custody, failure isolation and feature-by-feature installed acceptance. Every DLC remains operable alone |
| **Lu v3.0** | Finished, cohesive Lu with all planned DLCs fully integrated and polished | Establish end-to-end workflows, performance budgets, crash/restart/upgrade compatibility, cross-DLC verification and recovery. No claim that v3 is already specified feature-by-feature or implemented |

The user explicitly deferred the current WO10/P07 Lu bridge until Core v1.0. The v2/v3 roadmap does not authorize work on WO10, changing `lu-core`, merging IDE into `lu-desktop`, changing model authority or shipping a release.

## Ownership and provisional interface boundary

| Capability | Long-term architectural owner | Independent IDE v1 authority until integration |
|---|---|---|
| Human-editable source, tabs, splits, project explorer, LSP UI, code navigation | IDE specialist | IDE native host and editor |
| Programming-specific run configurations, compiler diagnostics, DAP debugging interface, package-manager semantics | IDE specialist | IDE host / protected workers |
| General filesystem, deterministic search, OS process supervision, tool registry, policy and per-turn authorization | Lu Core for Lu-originated operations | IDE **retains its own** trusted registry, one-use OS grants and native custody |
| Durable task journal, independently verifiable receipts, cancellation and OS recovery | Core shared *capability candidate* after v1, not an in-process implementation swap | IDE retains its proven supervisor/journal without weakening gates |
| Planning, memory, knowledge/belief updates, agent orchestration | Lu Core | No model/agent runtime required inside IDE |
| Specialist media/film tools, timeline semantics, render decisions | NVP/Film DLC | Outside IDE scope |
| Future cross-DLC authenticated Protobuf/IPC transport | A separately versioned, joint integration contract | **Not implemented, not authorized, not required to run IDE** |

This is an allocation of *responsibility*, not a decision to copy IDE security code into Core, delete standalone workers, share databases, forward secrets, or reuse historical approvals. Before any v2 implementation, perform a real capability-by-capability source and installed-behavior comparison. Prefer stable, minimum surfaces and adapters rather than duplicate full applications. Preserve the IDE's isolated trust domain and default-deny behavior.

## Required future v2 integration design review (P07/WO10 deferred)

1. Inventory every DLC's actual source, native entry points, capability registry, resource identities, IPC contracts, process supervision and user-visible operations. Mark planned vs source-implemented vs native-accepted vs installed-accepted.
2. Assign owners for each operation; prefer one authoritative implementation only **after** demonstrated behavior equivalence. Never remove an independent standalone safety mechanism before replacement is accepted.
3. Propose a versioned Protobuf interface out-of-process with local authenticated peer identity, explicit schema/version negotiation, read-only discovery, opaque handles, bounded inputs and typed outcomes. OS transport differs by platform.
4. Every Lu-originated action requires a fresh Lu-side per-turn grant **and** the IDE's own independent exact-project/operation approval. Refusal, stale-plan, timeouts, cancellation and restart revoke authority. The bridge itself cannot mint privileges.
5. Model/AI-proposed edits remain proposals until reviewed. Preserve source revision identity, original user files, undo/recovery and independently verified output receipts. No network/ambient credential inheritance or executable action just because a project opens.
6. Validate Windows and Linux installed packages separately for offline, incompatible, disconnected and malicious peers. Always allow the IDE to run without Lu.
7. Obtain the owner's explicit authorization after Lu Core v1.0 for implementation, staged pilot and release promotion; do not imply that this roadmap grants that authorization.

## Delivery and regression rules

- The user chose to **finish** the developed standalone IDE first; optional later simplification is a separate decision after accepted daily-driver pilots.
- Separate source, compiler, native OS worker, installed GUI, human approval and production release results. All claims use exact SHA, OS and real receipts.
- Do not let CI become a per-edit feature queue: one consolidated exact-head task-group gate, inspect genuine failures, repair and record remaining gates.
- No Docker requirement, no CLI-first dependency; user should operate the installed IDE graphically.
- No signing/publisher/legal approval implied. Unsigned Windows development pilots remain distinct from eventual release review.

## Current task-group boundary (as of the initial architecture record)

IDE Task Group 1 repairs are implemented in code commit `17e4d320f25009e185e3c8a7757089ab4b5df36e`, followed by exact-checkout grouped workflow dispatch `c827a94def0a936177e2c6aa16dc565739dfe61a`.
- Native Windows/Linux compiler and protected Linux worker group [42766269202](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42766269202) **SUCCESS** at code/witness SHA `c827a94...`.
- Installed [42766269459](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42766269459): Windows NSIS job **SUCCESS**; Linux Debian/GTK **OPEN while in progress at time of this record**. Never infer GTK success from the native matrix or a source fix.
- This document is a **documentation-only successor**, not proof that a newer source SHA was rerun. No Lu Core source, standalone execution authority or previous security gate is changed.

## Group 6 / Group 7 distinction

Final IDE branch/source reconciliation, four-language everyday developer pilots, release artifact provenance and legal/package review remain **Group 6** work unless separately completed with real evidence. This ADR closes the **architecture policy-writing part** of Group 7 only; it does not implement or accept v2/v3 integration. No historical PR or workflow is silently merged or closed.
