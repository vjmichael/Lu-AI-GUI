# Lu IDE — Engineering Acceptance Subgates

**Status:** design only as of 2026-11-04.  
**Canonical task list:** [IDE Workspace design, Section 6 — Recommended Implementations](IDE_WORKSPACE_DESIGN.md).

The project tracks **eight implementation tasks**, not ten competing roadmap tasks. The earlier IDE-01B ... IDE-10 identifiers below are subordinate engineering gates and final acceptance checks. Progress on a gate must not be described as a completed implementation task until all stage acceptance requirements are met.

| Canonical task | Related engineering gates | Required verification |
| --- | --- | --- |
| 1. Standalone Rust host + Protobuf lifecycle | IDE-01B | Independent Tauri app and host; OS-native picker, opaque IDs, no Lu dependency, open/close/reopen and persistence, Linux/Windows compile and native lifecycle tests |
| 2. Linux sandbox and real execution tests | IDE-07 (Linux first; Windows parity later) | Actual hostile-worker file/secret/network/resource tests; fail-closed; no automatic execution |
| 3. Project explorer and code editor | IDE-02 | Tabs/splits, undo/redo, search, diff, reliable save/recovery, accessibility |
| 4. LSP, build/test and debugger integration | IDE-03, IDE-06 | Sandboxed language server, build/test worker and DAP lifecycle; stream/cancel/recover |
| 5. Master package manager | IDE-05 | Detect/preview/approve/apply/verify for uv and C/C++ adapters first; lockfiles, hostile package hook tests; additional manager adapters in stages |
| 6. Advanced Git and GitHub workflows | IDE-04 | Worktrees, rebase/merge conflict resolution, staging, typed operations, remote confirmations, credentials boundary |
| 7. Lu AI coding integration | IDE-08 | Reviewable patches, real tests, authenticated per-turn grants, durable task recovery, prompt-injection adversarial cases |
| 8. Lu self-development environment | IDE-09 | Disposable Lu Dev instance, separate worktree/processes, no stable-core changes, rollback and merge review |

**Cross-cutting final acceptance:** IDE-10 — native Linux and Windows security, performance, installer/upgrade, backward compatibility, end-to-end and crash-injection evidence. This is required to release the IDE but **not** a ninth task.

**Wave A1 progress (2026-11-05):** A bounded saved-disk source-span API with SHA revision precondition, exact one-based lines and UTF-8 byte offsets is implemented in source and has native tests added. Not yet CI-verified; unsaved buffer snapshots and human UI references are pending. This is **partial progress toward task 3**, not task 3 completion.

**Build-order reconciliation (2026-11-05):** [IDE-ARCH-02](CAPABILITY_FIRST_ARCHITECTURE.md) changes the internal delivery sequence to capability-first Waves A–G. The stage IDs and their objective acceptance criteria above remain unchanged. Read-only source identity, project provenance, Git inspection and toolchain discovery can proceed before executable worker containment. Live Lu integration is deferred; no runtime bridge dependency is allowed.

**Dependency rule:** Do not accept executable package installs, build/test sessions, or executable LSP/DAP adapters before task 2's native isolation gates. Windows release claims remain blocked until real Windows security parity tests are complete.

**Progress discipline:** Track designed, prototype, compiled, unit-tested, native-tested, and accepted separately. A passing mock is not a passing sandbox; branch isolation is not process isolation.

**Architectural change:** [ADR-0001](ADR_0001_STANDALONE_APP_AND_LU_BRIDGE.md) makes Lu IDE a standalone application. Lu's optional bridge must not be treated as a required startup dependency or a shortcut around IDE security.
