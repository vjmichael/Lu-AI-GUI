# IDE Group 4 — Protected Build/Test and LLDB (2026-11-10)

## Group 4 task-boundary handoff (latest native evidence, Nov 10)

**Branch source:** work stacked on Group 3; no Lu Core bridge or release entitlement. Current source edits are NOT a native acceptance because the last four-lane run checked an earlier SHA.

| Gate | Observed original evidence | Status |
|---|---|---|
| G04B-Cargo check/test/build | Real systemd delegated/Bwrap `Completed` and exit zero for each mode at `bec4eb0`; independent artifact debug sections failed afterward | **Partial native, not accepted** |
| G04B-artifacts | Original executable alias readback passed; second DWARF hardlink reader denied, repaired in source via same all-links-inside-private-output check | **Rerun required** |
| G04B-cancel/crash/restart | Earlier genuine Linux fixed-LLDB crash/restart passed; Build/Test extended normal/cancel and Windows exact-head worker not fully observed on corrected source | **Open** |
| G04C-Windows LLDB | Owner-event/pinned LLVM branch compiled in existing gate; new four-lane Windows patched LLVM build from run [42868873025](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42868873025) was in progress at this checkpoint | **Open, not a success claim** |
| G04C-Linux LLDB | Genuine systemd-delegated adapter initialized; correlated launch returned failure, terminal quarantined; no ptrace/permission bypass applied | **Real native failure unresolved** |
| G04C-breakpoint | One-use native DAP setBreakpoints ACK is `ResultUncertain`; independently witnessed breakpoint *hit* and re-stop not yet completed | **Open** |
| G04C-step | Native isolated Windows step owner-event/PDB code and fixture exist, but physical human and installed evidence not obtained | **Open** |
| G04D-variable frame | Original OS RIP/PDB/owner receipt now independently binds correlated DAP frame ID; redacted one-scope/16-variable protocol and hostile tests source-implemented | **Source-only, native dispatch and consent open** |
| Installed protected Build/Test/LLDB | No signed/pinned protected worker bundle accepted in installed NSIS/Debian and no independent physical operator approval/restart | **Open** |

**Do not rerun superseded source, reuse a stale D3 permission, lower kernel scope, substitute a DAP acknowledgement for OS proof, or infer installed authorization from CI.** The next exact-source grouped run must validate the recent `read_attested` fix, native frame/one-use tests and pinned VS side-by-side selection, and must separately diagnose the Linux DAP launch failure. Preserve the still-running expensive LLVM build from prior SHA rather than prematurely cancelling it. Frontend remains nonexecuting by default.

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

### Linux LLDB launch failure localization

The same real native run `42866653321` confirmed that the OS-owned
LLDB-DAP adapter initializes inside a genuine delegated cgroup/Bwrap
worker, yet the next correlated DAP reply has `success=false`, originally
classified only as `other_adapter_error`. Before touching ptrace, AppArmor,
mounts, resources or process authority, the native diagnostic now emits
only the fixed reviewed request category and additional bounded error
classes (`sealed_target_launch`, `target_launch_failed`, etc.), never raw
adapter paths/contents. **No OS policy change or LLDB success is claimed.**

## Windows native runner VS installation-selection finding

[Group 4 native run 42866653321](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42866653321) reached the Windows offline toolchain stage, but the existing helper selected `vswhere -latest` and rejected the result as `DENIED: VS pin 17.14. not installed`. A newer side-by-side VS instance may shadow the required reviewed version. The helper now selects **exactly one** VS instance whose independently reported version starts `17.14.` and has the reviewed VC Tools component; it binds that instance's path and version in the same record, and still demands the exact MSVC `14.44.` and Windows SDK `10.0.26100.0`. No arbitrary version, unsigned release provenance, or machine-supplied command gains authorization. A new native run must determine whether the required exact pinned instance actually exists; if absent, the runner remains correctly blocked.

## Four-lane consolidated Group 4 native gate

One Group 4 branch/workflow-path-triggered native gate now contains: Windows fixed LLDB/AppContainer/SCM; Windows independently pinned LLVM18 D5 owner-event and D6 step/breakpoint script; genuine delegated Linux LLDB crash/normal/cancel; and genuine delegated Linux offline Cargo check/test/build/project-debug/cancel. Each first runs the exact-source hostile regression suite (original build receipt, enclosed ELF/DWARF alias and frame-bound consumed variable review). The Windows owner lane compiles native one-use journal, developer Tauri dialog and actual protected step/breakpoint fixtures. **No headless CI job may satisfy physical interactive approval or installed debugger acceptance**. A pending native job or a missing reviewed Windows VS installation remains OPEN.

## Four-lane first preflight outcome

[Run 42868873025](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42868873025), SHA `ca8c99f705a0ec914a21d593674d59b62ca20e98`, showed Linux Rustfmt differences in the shared enclosed-hardlink source before the real Linux kernel worker stages. Six exactly reported formatting differences were corrected; neither native OS runtime nor installed consent may be inferred from this preflight.

### Frame/thread identity negative regression

Feature-gated D6C/G04 variable and stepping reviews now refuse a
`stackTrace` for any thread other than the last correlated DAP stopped
thread. Even a numerically valid frame ID from another thread may not
be reused. No renderer input, grant, or installed debugger command was
added. This is protocol-only until native OS/consent and installed
acceptance pass.

## Exact native Windows LLVM generator failure — runner repair

[Four-lane run 42868873025](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42868873025) finished FAILURE. The pinned-LLVM source build obtained and overlaid LLVM18, but CMake's reviewed `Visual Studio 17 2022` generator reported `could not find any instance of Visual Studio` on hosted `windows-2025`. The separate fixed Windows lane had reached only the source-format gate. Both Group 4 Windows lanes now use `windows-2022` **without altering** LLVM 18.1.8, Rust 1.99.1, required VS17.14/MSVC14.44/SDK 10.0.26100.0, kernel containment, grant scope or checksum checks. The revised workflow must actually prove these pins exist; otherwise deny and keep native acceptance open.

## Fifth exact native preflight — older fixture missing synthetic stopped-thread identity

[Run 42870494783](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42870494783) Linux host feature-gated tests compiled and exercised 20/21 DAP cases successfully. The existing `dev045d_only_correlated_stack_can_supply_bounded_source_candidate` test fabricated `DapPhase::Stopped` without a stopped thread, then attempted a second `stackTrace`. The new Group 4 cross-thread protection correctly denied with `InvalidPhase`. That **synthetic fixture** now supplies the reviewed same-thread identity when opt-in D6/G04 controls are enabled. No production dispatch or thread check was relaxed; native worker acceptance must be rerun.

## Group 4D native Windows inspection authorization implementation (stacked branch)

The developer-only protected Windows DAP session now requires the exact
native D5 Job/PE/PDB/thread/owner receipt and correlated source frame,
then a *new* bounded OS-native human modal for one read-only variable
inspection. It rechecks source, OS owner and protected job after the modal;
consumes and fsyncs a single-use action before `scopes`; requests only
one non-expensive local scope and at most 16 redacted named variables;
then rechecks the original owner and immutable source. DAP names/types
remain **unverified adapter assertions**, so the result is marked
`ResultUncertain`, never `ResultVerified`. A cancelled, refused, stale,
foreign-frame or crashed action cannot recreate a grant. This code is
feature-gated and not an installed/default release capability; Windows
native build, genuine approval and restart acceptance are still REQUIRED.

