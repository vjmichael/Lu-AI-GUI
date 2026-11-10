# IDE Group 4 — Protected Build/Test and LLDB (2026-11-10)

**Status:** Started, code-only first gate. **Not accepted.**
**Branch:** `ide/group04-protected-build-test-lldb-20261110`, based on
Group 3 exact parent `9dec923748bcdaa4df6f78aae104e07b653f6509`.
Standalone IDE only; Lu bridge deferred. Group 3 still has physical-human
and production containment gates open. No uncontained fallback.

## Existing implementation to preserve

- Protected fixed Cargo `rust_check`, `rust_build`, `rust_tests`,
  `lu_ide_host_tests` native supervisor: immutable snapshots, one-use
  per-job approval, bounded outputs and real OS custody in independent
  Linux cgroup/Bwrap and Windows AppContainer/SCM Job fixtures.
- Reviewed fixed-language adapters remain separately source/toolchain
  pinned; discovering a filename cannot grant execution.
- Source-pinned LLDB DAP, original Build receipt, immutable binary +
  symbols, native kernel PC/PDB/DWARF evidence and first-party Windows
  event ownership. Step/breakpoint are CI/developer-only experiments;
  breakpoint ACK is **uncertain** and variables/evaluate remain denied.

## Task 4A — Original build-to-debugger receipt binding (implemented, unverified)

Previous `NativeBuildRuntime::native_project_debug_target` accepted an
in-memory sealed target after checking only the D3 job's Completed
phase and job/plan identity. Its cached lineage held an original terminal
receipt SHA that was not compared at this call boundary. Group 4 now
rechecks the exact D3 original `rust_build` recipe, source HEAD, frozen
source/manifest/toolchain hashes, Completed phase, quarantine state, and
terminal receipt SHA against both the supervisor's source-plan review
and the last per-job fsynced journal entry. This is an additional
read-only preflight; it does not replace PE/PDB, kernel, one-use consent
or native output-tree verification. It explicitly rejects a completed
`rust_tests` receipt as a debugger build.

Hostile unit cases cover accepted exact match, substituted terminal
hash, source/manifest/toolchain/plan tamper, quarantined lineage and
wrong recipe. Tests are **source added, not executed**.

## Task 4B — Genuine OS-contained Build/Test acceptance (OPEN)

Use the existing WO6 native Windows SCM/AppContainer and Linux systemd-
delegated cgroup/Bwrap fixtures. Verify offline Cargo Build/Test normal,
source/manifest swap denial, no host-network access, bounded stdout/
stderr, cancellation/crash, actual process-tree zero and restart journal
against one exact head. Do not use mocks to satisfy native kernel gates.

## Task 4C — LLDB stepping/breakpoint acceptance (OPEN)

Run real pinned LLDB owner event-loop acceptance, independent stopped
OS PC and compiler PDB/DWARF line, one-use per-operation native human
consent where enabled, OS thread-bound step with a **new** independently
attested owner event. Keep a breakpoint ACK `ResultUncertain` until
actual native hit/memory witness; no arbitrary PID/attach/console.

## Task 4D — Variable inspection and installed acceptance (OPEN)

No variables, evaluate or setExpression until independent frame/scopes/
variable origin, length/redaction and native approval are implemented
and negatively tested. Then require actual installed Windows/Linux
Build/Test/LLDB cancel/restart and signed-artifact governance separately;
no source pass substitutes for physical human consent.

**CI policy:** one consolidated exact-SHA Windows/Linux native task-group
gate after cohesive fixes, not a CI workflow per subtask. Group 3 native
claims do not close Group 4. Preserve all existing default-off features.
