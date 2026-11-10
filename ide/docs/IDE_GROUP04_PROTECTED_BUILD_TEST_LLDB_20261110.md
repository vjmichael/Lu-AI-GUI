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

## Group 4B — Parent exact-head native compiler diagnostics and narrow repair

Observed genuine parent SHA `9dec923748bcdaa4df6f78aae104e07b653f6509`:
- [WO6 source run 42860263579](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42860263579) failed Windows/Linux test compilation with Rust E0618: `lib.rs` unit test's local `plan` variable shadowed the fixture function when constructing `fixed`. Qualified `self::plan()` without changing authority or product code.
- [WO6 native OS run 42860263865](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42860263865) and [opt-in Windows desktop run 42860263589](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42860263589) failed Windows Rust E0425: `start_windows_lsp` referenced `project_target`, which only exists in the separate debugger launcher. Removed this unreachable/invalid LSP branch; the Windows DAP project-target validation remains separate, strict and unchanged.
- Native Linux Cargo got a genuinely Completed/exit-zero contained check, but then **denied** postrun with `DENIED: real completed Cargo binary readback changed`. Cause and fix remain open; do not call this native Build/Test a success.
- Real Linux LLDB delegated stop/cancel also failed a later native gate, not yet reconciled.

These compiler repairs are committed source changes; no subsequent exact-head Windows/Linux acceptance has yet passed. Do not rerun unchanged failed prior SHA as evidence of the new code.

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

## Group 4B contained executable alias accounting — source repair

Linux Cargo can hard-link the primary executable under `target/debug/`
to its same-inode compiled copy under `target/debug/deps/`. The native
readback previously required `nlink == 1`, which rejects both even when
the entire compiled output is private and post-termination SHA-checked.
The auditor now requires that **every OS link** be accounted for in the
bounded native output inventory, with the same device/inode, byte length
and SHA-256. Any link outside the inventoried output is still denied.
Hostile Unix tests create a genuine same-inode Cargo-style alias and then
an external alias that must fail. The earlier native refusal does not
identify which `checked_file` predicate failed, so this is a targeted
hypothesis requiring an actual exact-head Linux rerun, not proven success.


## Group 4C / 4D — stale-frame denial and developer-only redacted inspection

The genuine Windows project DAP launcher now re-verifies the privately
sealed Cargo target **after staging/SCM admission and immediately before
one-use grant consumption**. Ordinary LSP has no project debugger target.
The DAP parser now invalidates its previous source/frame and any variable
probe on each new stop, continue, step, stack revision, termination or
quarantine. It prevents an old source coordinate being reused across stops.

A narrowly feature-gated `dev046c-ci-control` protocol path can issue one
correlated `scopes` query for the currently returned source frame and one
bounded `variables` query for one non-expensive Locals/Arguments scope.
Up to 16 conservative identifier/type records are returned **with all
values, addresses and child references excluded**. No recursive expansion,
`evaluate`, `setVariable`, arbitrary DAP payload, Tauri command or installed
feature is added. Hostile tests check stale frame, duplicate/expensive scopes,
one-use consumption and a secret value that must never surface.

**NOT CLOSED:** the DAP frame ID is adapter-supplied; its independent native
OS/PE/PDB binding and per-inspection human action receipt are not yet
wired to this protocol. The added code is private, non-installed test
scaffolding, NOT real accepted variable inspection. Breakpoint ACK remains
`ResultUncertain`; native breakpoint hit and Windows physical approval
remain OPEN.

## Consolidated native acceptance submitted (not a passing result)

One new branch-path-only workflow, `.github/workflows/ide-group04-native.yml`,
reuses the existing real Windows SCM/AppContainer and two Linux delegated
Cargo/LLDB jobs, with exact-head test/format preflight for Group 4's
original-build lineage, enclosed ELF hardlinks and redacted DAP parser.
It is triggered only by this workflow file on the Group 4 branch and does
not automatically open an independent CI fanout. All three actual OS
jobs must pass at one SHA. A queued/failed/absent job is NOT ACCEPTED.


## First exact-head Group 4 CI finding

[Consolidated run 42865814046](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42865814046) at `ae6c9a41ddf833526eb8b8e58a7c4b0cb843fa73` reached the source preflight and found native `rustfmt` differences in the new redacted-variable DAP processor. Linux kernel worker tests were not reached; this is not containment failure or a native PASS. The exact formatter output was applied without changing authority. A *new* final-head run is necessary before any native claim.

### Second consolidated native preflight

[Run 42866301600](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42866301600), SHA `ef3365b913b351970b585e0794e24f2c73ebca70`, independently reached the supervisor rustfmt gate after the host redaction-path formatting was corrected. It found four exact formatter differences in the new Unix hardlink and D3 receipt regression tests; the native jobs did not reach OS execution. Only those exact formatting changes were applied. Native Build/Test and LLDB runtime acceptance remain OPEN pending a later exact-SHA rerun.

## Group 4D — independent Windows frame-id binding before scope review

The existing real D5 native OS stop acceptance already rechecks the actual
protected thread RIP, compiler PE/PDB/source line, first-party LLDB event
owner and both durable receipt chains. It now also captures the **exact
correlated top DAP stack frame ID** from that same accepted stopped OS RIP
and source line, rejects missing/out-of-range frame IDs, and carries the
bound frame ID in the private nonserialized owner witness. Only that
witness can make `InspectVariables` reviewable in a native developer
session. The consumed one-use variables claim must match this exact
frame, plan/source/native event and cannot be replayed or borrowed from
a step/breakpoint consent.

This is **source-only authorization plumbing**, not yet an accepted
variable read: the consumed claim is not wired to installed Rust/OS
scopes dispatch, and the redacted protocol's bounds do not establish
verified variable values. No WebView path or release execution is enabled.

## Group 4B — genuine completed build exposed second auditor hardlink defect

[Native run 42866653321](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42866653321) at `bec4eb037493550891ee8ba0bf2e20dec1ab240e`:
Linux Cargo check/test/build each reached genuine sandbox `Completed`
results. The original readback is now accepted, but the independent
DWARF verifier rejected the same Cargo-linked executable because its
`read_attested` still required `nlink == 1`. This was a second actual
same-inode policy conflict, not altered source evidence. Both auditors
now share the exact enclosed-hardlink proof and reject any foreign alias;
private debug target sealing passes the original complete inventory.
A new cross-layer ELF/DWARF alias-vs-external-alias regression was added.
**Native rerun required**; no symbol/LLDB success asserted yet.
