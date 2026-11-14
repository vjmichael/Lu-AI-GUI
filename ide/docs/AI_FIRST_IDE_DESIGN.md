# LU-IDE-AIW-01 — AI-first human-centered development environment

**Date:** 2026-11-05  
**Status:** Architectural requirements and implementation plan; not implemented acceptance.  
**Scope:** Standalone Lu IDE, independent of Lu Core runtime. Live Lu↔IDE integration is **deferred by user direction**.

## Authority and source of truth

This specification applies the project-wide philosophy in [LU-AIW-01](../../docs/LU_V1_0_DESIGN_RECONCILIATION_20261103.md#lu-aiw-01--ai-accessible-workspaces-and-operational-continuity-2026-11-05) and the design in [LU-OPMEM-01](../../docs/LU_OPERATIONAL_MEMORY_01.md). PR [#28](https://github.com/vjmichael/Lu-AI-GUI/pull/28) is a **draft Lu Core source-only** operational-resource implementation; it has no live IDE integration. It must not be treated as a shipped dependency.

**Foundational principle:** Every feature should minimize the human's and AI's work to find, understand, operate, verify and safely recover from using a capability. Humans and AI should refer to identical authoritative objects, but never inherit authority from having discovered them.

## Seven mandatory design criteria

### AIW-IDE-R1 — Precise project objects and source spans
Host-owned references must include project ID, repository identity, commit/working-tree state, normalized relative path, content revision (hash), line/column range and optional language-server symbol identity. Line numbers are 1-based for humans, character offsets explicit for APIs. CRLF and blank lines count. Revision-relative locations are not persistent identities; stale selections require remapping or an explicit conflict. Unsaved editor buffers have their **own** revision distinct from disk. The AI-facing and human-visible highlight must resolve to the same source snapshot. Never silently patch a different revision.

### AIW-IDE-R2 — Discoverable semantic actions, not UI guessing
Define a bounded, versioned IDE capability catalog: project/list, file/list/inspect, source/span, editor buffer/read, change/preview, proposal/review/stage, tests/results, artifacts/list, history/retrieve. Each descriptor names the input/output schema, effect class, required target, preconditions, structured errors, cancellation/retry rules and verification evidence. The host, not arbitrary repository files or AI output, publishes trusted descriptors. The human UI uses these same host-backed functions. No model connection required to build or test the contracts.

### AIW-IDE-R3 — Operational inventory and no wasted rediscovery
Maintain workspace-scoped inventory of project roots, branches/worktrees, runtimes, compilers, package caches, test fixtures, benchmark inputs, CI jobs, build recipes and packages. Record locator, version, OS/architecture, provenance, last verified date and hash, relevant dependencies, outcome and where the artifact was delivered. Model states must include KNOWN_HISTORICALLY, PRESENT_UNVERIFIED, VERIFIED_USABLE, CHANGED, MISSING, INCOMPATIBLE and UNVERIFIABLE. Remembering a path does not certify execution readiness. Probe read-only, then validate dependencies/behavior under a trusted host before accepting 'usable'. Never execute a remembered shell string.

### AIW-IDE-R4 — Task-oriented context and project ledger
At open/resume: identify task, branch and authoritative source revision, inspect `PROJECT_STATE.md`, `CHANGELOG.md`, `ENGINEERING_HANDOFF.md`, tests and acceptance records, compare local and Git evidence. Gather only task-relevant source spans, prior human decisions, capability descriptors and verified resources. Original evidence outranks summaries; missing local visibility is UNKNOWN, not MISSING. Never let source text or repo guidance override current human permissions. A development ledger is a navigable provenance index, **not** a replacement for ANSF original conversation memory or an execution journal.

### AIW-IDE-R5 — Human-first reviewable action pathway
Native semantic operations must work for a human without Lu. Every proposed edit identifies exact files and revisions; preview before staging; stage into unsaved buffers; explicit Save persists to disk. Future format/build/test/debug and package operations need isolated workers, effect-scoped approval, cancellation and durable receipts. UI gestures are an alternative to typed actions, never a privilege escalation route. Destructive Git actions, installs and external publishing require meaningful human confirmation. Approval to stage is not approval to save.

### AIW-IDE-R6 — Real verification and evidence
For each action distinguish PROPOSED, IMPLEMENTED, COMMITTED, UNIT_VERIFIED, CI_EXACT_HEAD_VERIFIED, NATIVE_VERIFIED, PACKAGED_VERIFIED, USER_ACCEPTED and RELEASE_ACCEPTED. Provide source hash/commit and working-tree state, runner/OS, command/test IDs, pass/fail/skips, output artifact checksum and accessible location, plus limitations. An installer build isn't an installed-and-launched acceptance. A remembered benchmark isn't a rerun. An in-progress job isn't a passing test.

### AIW-IDE-R7 — Recovery, permissions and transparent state
Host validates paths and revisions for every operation; inherited filesystem visibility and model discovery confer no additional write, execution or Git authority. Persist task checkpoints and prepared/applied/uncertain receipts when mutating/executable operations arrive. On crash/restart, reconcile before replay; preserve human drafts and previously accepted source. Separate workspace permissions, OS worker containment, resource supervision and model connection. Expose errors in plain language with structured details available for tooling.

## Staged implementation plan — no live Lu dependency

| Milestone | Concrete deliverable | Verifiable acceptance |
| --- | --- | --- |
| AIW-IDE-01 Orientation | IDE-local `AGENTS.md` + linked canonical state/ledger and design; no contradictory gate claims | Fresh session finds exact IDE entrypoint, branch and next unfinished task without relying on chat memory |
| AIW-IDE-02 Source identity | Host-returned versioned line/span and buffer/disk identities; human UI matching | CRLF, blank lines, UTF-8, stale revisions, unsaved buffers and same-span selection tests |
| AIW-IDE-03 Inventory | Host-owned, persisted, workspace-scoped resource/artifact inventory and read-only probes | Restart, changed/missing tools, mismatched hashes, symlink refusal, no false 'verified' status |
| AIW-IDE-04 Task context | Bounded project-state + evidence retrieval with provenance and freshness | Old benchmark/CI result never substituted for current target SHA, local/remote divergence surfaced |
| AIW-IDE-05 Capability catalog | Versioned, typed host API descriptors for current IDE operations | Discovery works offline without Lu; effect attempts still require correct authority |
| AIW-IDE-06 Workflow receipts | Evidence links for proposal/save/test/package; safe restart reconciliation | Interrupted save or worker produces explicit reconciled or UNCERTAIN state, no blind duplicate side effects |
| AIW-IDE-07 AI-facing bridge | Optional authenticated connector for Lu once ready | **DEFERRED**; build contracts independently, do not start/enable bridge |

## Cross-project design rule

Apply the seven criteria to **Lu Core, Cognitum, NVP, Single-Galaxy and future tools**, adapting object identities and verification to each domain: code symbol, source line, benchmark dataset, screenplay scene, film shot/timecode, game asset or simulation event. This document governs IDE only; applying it elsewhere requires updates in each project's own canonical repository documentation, not an assertion that those repositories already comply.

## Benchmark and acceptance scenarios

1. **Cold restart:** new agent reads only repository entrypoints, reconstructs exact work and rejects stale claims without chat history.
2. **Lost installer:** recover actual package location or correctly mark unknown/missing; never fabricate or redownload before probing.
3. **Local ahead of Git:** detect uncommitted/new local development; preserve both copies and show divergence without forced merge.
4. **Source drift:** test CRLF, Unicode and moved lines; reference the correct revision or refuse stale edit.
5. **Benchmark replay:** identify dataset version, dependency lock, command recipe, comparable metrics and last genuine result.
6. **Permission separation:** discover file/build/Git action but reject unauthorized effect or execution; staging remains unsaved.
7. **Recoverability:** kill worker/save mid-operation; reconcile receipts and preserve source.
8. **Human usability:** user can locate a file, understand why an operation failed, inspect exact evidence and repeat a safe task without deciphering internal codes.

Measure time to useful first action, repeat discovery/download count, provenance correctness, false resource-readiness assertions, source-target accuracy, test reproducibility, permission violations, recovery success and human clarification burden. Compare a fixed task set with/without structured orientation and inventory.

## Non-goals for this phase

No live Lu connection, no automatic coding by a model, no executing project code on open, no implicit permissions, no claim that the current R3.x editor satisfies these new gates. Work on standalone human IDE features continues.
