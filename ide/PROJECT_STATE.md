## G04D two-lane source gate verified — 2026-11-10

[Run 42916253635](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42916253635), source SHA `f1b0b9d462d620b2e499a349df461ac650317189`, completed **SUCCESS** in both `windows-native-source` and `linux-native-source`. Windows exact-source/consent-boundary, protected host/supervisor tests and developer-only Tauri compilation steps all succeeded, including the CI-only icon and genuine frontendDist build repair. Linux formatting, protocol, supervisor and release-source checks succeeded. This is a **source/compilation gate only**, not pinned LLVM18 rebuild, protected kernel Build/Test/cancel/crash/restart, breakpoint/step OS owner evidence, physical human consent, installed app or authoritative `545a7a3` exact-head acceptance. Original [run 42870494783](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42870494783) is terminal FAILED at historical `e8fce1f`; the pinned MSVC chrono overlay has NOT been rerun. The authoritative Group 4 branch remains unchanged at `545a7a3`. No branch reconciliation or expensive native CI dispatched. G04B/C/D remain OPEN.

## G04D genuine Windows frontendDist source-gate repair — 2026-11-10

[Temporary source run 42877709275](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42877709275) at `d01c4b0` finished Linux SUCCESS, Windows FAILURE: Windows host/supervisor native tests and protected supervisor compilation progressed, but Tauri `generate_context!()` panicked because configured `frontendDist: ../dist` was absent. The earlier missing ICO was already addressed with a CI-only diagnostic resource. The temporary source workflow now runs `npm ci --ignore-scripts` against the checked-in lockfile and genuine `node scripts/build.mjs`, checks `ide/app/dist/index.html`, then compiles Tauri; this does not simulate human consent or grant installed release. This is **source repair, native rerun pending**; the workflow path push triggers only its two-lane source gate, not expensive pinned LLVM. Authoritative Group 4 remains at `545a7a3`, no exact-head OS acceptance, no branch reconciliation, no Group 4B/C/D closure.


## Group 4C demonstrated MSVC defect repair — 2026-11-10

Historical native run [42870494783](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42870494783), pinned owner job `130020178608`, failed MSVC C2039/C3083/C3861 because pinned LLVM18 `lldb/tools/lldb-dap/ProgressEvent.h` uses `std::chrono` without its direct header. The temporary inspection branch now adds a **fail-closed pinned-source overlay** in `ide/llvm18_owner/apply_llvm18_owner_hook.py`: after verifying exact upstream LLVM commit `3b5b5c1`, it requires the unique `#include <atomic>` anchor and adds `#include <chrono>` before building. This is a **source repair only**, not a native compiler pass. The original Group 4 branch remains at `545a7a3bc40361926acb5ae3803b84645929b06c`; no CI dispatched, no owner-event or installed acceptance. The prior `InvalidPhase` correlated-stack fixture correction and temporary diagnostic Tauri icon staging already exist on the inspection branch; they require fresh native verification. Group 4B/C/D remain OPEN. Do not reconcile temporary work until the authoritative acceptance decision.

## Group 4 native terminal evidence (2026-11-10)

Original historical run 42870494783 at e8fce1f finished FAILED: pinned Windows LLVM18 owner job 130020178608 reached MSVC but failed missing direct chrono include in LLDB-DAP ProgressEvent.h/ProgressEvent.cpp; no owner-event/lock acceptance. Windows/Linux other lanes failed dap_session correlated-stack test InvalidPhase at line 1799. Temporary run 42876264987 at a617977: Linux source PASS, Windows parser 23/23 and control 9/9 PASS, desktop compiler FAIL because icons/icon.ico missing. These are historical source failures, NOT exact-head native acceptance at authoritative 545a7a3. No Group 4 branch rewrite, CI dispatch, OS human approval or installed acceptance. See ide/docs/IDE_GROUP04_PROTECTED_BUILD_TEST_LLDB_20261110.md. Group 4B/C/D remain OPEN.

# Lu IDE — Current project state (authoritative handoff)

## Group 4D native inspection staging (Nov 10; NOT PROMOTED)

The temporary `ide/group04d-native-inspection-20261110` branch is
based on exact Gate 4 SHA `545a7a3` without changing that base ref
while long-running pinned Windows LLVM18 job #130020178608 executes.
It contains developer-only protected Windows scope/variable inspection,
physical OS modal and one-use fsynced control receipt, redacted
names/types only, stale-stop revocation, collapsed advanced controls,
strict source tests and a real interactive Windows consent/restart
operator fixture. As of source gate #42876264987, Linux exact-head
format+Rust tests passed; Windows native dev compilation was still
in progress. No native LLVM/installed physical result is inferred.
Release/default remains disabled. Promote only after original Group 4
exact-sha acceptance and independently checked Windows/Linux compiler,
OS custody and physical human evidence. No source-check bypass.

## Group 4 implementation/evidence boundary (Nov 10)

The Group4 branch now contains original Cargo terminal binding, all-links-
accounted ELF/DWARF reader, Windows exact VS instance selection, strict
stale DAP source/frame invalidation and a feature-gated redacted variable
protocol bound to a native owner/PDB verified frame. Actual Linux Cargo
check/test/build Completed at historical `bec4eb0`; a second DWARF
readback failure was repaired in later source but not yet retested.
Actual Linux LLDB launch is still rejected after successful initialize.
The grouped Windows pinned LLVM owner job at older source is not
finished. Group 4 installed Build/Test/LLDB and real human per-action
approvals are **NOT ACCEPTED**. Use
`ide/docs/IDE_GROUP04_PROTECTED_BUILD_TEST_LLDB_20261110.md` for the
full evidence matrix; do not count pending CI or source changes as PASS.

## Group 4B — independent DWARF reader enclosed alias repair (2026-11-10)

Native run 42866653321 proved contained Cargo check/test/build completed;
the ELF/DWARF `read_attested` still denied Cargo's legitimate same-inode
internal alias. Both binary and symbol readers now share all-links-
accounted sealed-output verification, denying external aliases. Native
rerun, independent LLDB and installed acceptance remain OPEN.

## Group 4D — OS-bound frame ID for one-use variable review (2026-11-10)

Extended genuine Windows D5 kernel/PDB/owner evidence with top correlated
DAP frame ID, matching OS RIP and source line. Variable review requires
this native frame, and its consumed claim cannot be reused across
operations or wrong frame. No installed scope dispatch or release-enabled
variable inspection. Native exact-head acceptance remains OPEN.

## Group 4C/4D — guarded DAP and redacted variable protocol (2026-11-10)

Revalidated protected Windows project target before one-use DAP job grant.
New stop/continue invalidates old source/frame. Added developer-only
correlated one-scope/16-variable names-and-types readback with values
redacted and no recursively dereferenced references. It is not yet
independently OS-frame verified or operator-consented in an installed IDE;
actual 4C breakpoint hit and 4D release acceptance remain OPEN.

## Group 4B — contained Cargo executable link closure (2026-11-10)

Added all-links-accounted postrun verification for legitimate Cargo
same-inode executable copies within the private build output. External
hard-links remain denied; no containment, approval or release permission
changed. Real Linux readback rerun is OPEN pending native acceptance.

## Group 4B — demonstrated parent compiler failures (2026-11-10)

At the original Group 3 parent head, real WO6 Windows/Linux unit builds
failed with Rust E0618 (a `plan` test variable shadowed `plan()`), and
Windows protected worker compile failed E0425 (ordinary LSP launcher
used debugger-only `project_target`). Both source defects are repaired
within the same Group 4 branch without a containment or release change.
Real delegated Linux Cargo also failed after native success because
its protected binary readback changed, and native LLDB later failed;
those are unresolved. Group 4 exact-head CI is NOT ACCEPTED.

## Group 4A — Protected Build/Test and LLDB (2026-11-10)

Working branch `ide/group04-protected-build-test-lldb-20261110` is
stacked on Group 3 draft at `9dec923748bcdaa4df6f78aae104e07b653f6509`.
The existing protected Cargo Build/Test and owner-verified LLDB work is
being reconciled, not replaced. The first hardening change makes the
build-to-project-debugger handoff compare its cached terminal receipt
hash and original source/manifest/toolchain identity against the
supervisor's actual Completed `rust_build` receipt. A phase/plan-only
match or completed test recipe can no longer supply project-debugger
provenance. This change never creates an OS grant or approves execution.

Native Rust regression was added but **not yet run** on exact-head
Windows/Linux. Group 3's real physical approval and production OS gates
remain OPEN. Group 4 still requires real OS-contained Build/Test
normal/cancel/crash/restart, LLDB owner-event breakpoint and step proof,
variable provenance/redaction before enabling inspection, and installed
human authorization. Do not promote debugger functions from disposable
CI/developer-only builds or make the default executable.
See `ide/docs/IDE_GROUP04_PROTECTED_BUILD_TEST_LLDB_20261110.md`.

## DEV-04.6C — protected CI debugger command and independent result (Nov 9, 2026)

Working branch `ide/dev04-6a-debug-control-admission-20261109` stacked
on draft D5 PR #192. Added explicitly opt-in `dev046c-ci-control` for
sealed native Windows test use only; no release or Tauri route. The
protected gateway can now send an operation-specific, fsynced-before-
dispatch one-use `next`/step request for the exact D5-owned OS thread
or one single-line `setBreakpoints` request for the reviewed compiler
source, never arbitrary DAP. The step can receive `ResultVerified`
**only** after a correlated DAP step stop plus a fresh first-party
owner-event sequence, independently advanced OS RIP/PE/PDB/source line,
same Job/plan/original build/Running/source receipts and new durable
owner receipt. LLDB response alone never proves success.

Single-source breakpoint ACK remains `ResultUncertain`; variables
remain denied; all action claims consumed once with no restart replay.
New native fixture modes `--project-debug-step` and
`--project-debug-breakpoint` are added to the *existing* consolidated
Windows SCM/Job batch. No new PR/CI run. **No exact-head D6C native
compilation, OS step or installed consent has been accepted.**
See `ide/docs/IDE_DEV046C_NATIVE_CONTROL_DISPATCH_20261109.md`.
D5 Windows event-loop sequencing and all D6 installed human-approval
gates remain open.


## DEV-04.6B — one-use debugger control receipts and operator-consent contract (Nov 9, 2026)

On working branch `ide/dev04-6a-debug-control-admission-20261109`,
stacked on D5 draft PR #192. Added a native-test-only,
operation-specific `DebugActionApproval` journal: exact D5 owner-event
and independent Job/PE/PDB/line identity, one pending D6A review,
CSPRNG challenge, approval/denial, 30-second expiry, consumption
**fsynced before any future command**, revocation, and restart-only
read/verification of the chained receipts. A consumed claim cannot
be replayed and no journal file restores permission. The only current
terminal outcome is **ResultUncertain**; caller-asserted success was
explicitly prohibited.

The existing real protected D5 acceptance fixture now exercises
CI-simulated operator confirmation, one-use consumption, repeat denial
and audit after restart; `ci_only_not_installed_consent` is preserved.
**Actual OS human-approval UI, actual breakpoint/step/variable DAP
commands, independently verified operation results and signed installed
acceptance remain NOT IMPLEMENTED / OPEN.** Nine added adversarial
source tests, native compiler and OS tests not yet executed. By owner
policy, no tiny CI or draft PR was created; retain this work for one
consolidated native acceptance batch after the cohesive D6 group.
See `ide/docs/IDE_DEV046B_ACTION_APPROVAL_ONE_USE_20261109.md`.


## DEV-04.6A debugger operation review and CI batching — Nov 9, 2026

Working branch ide/dev04-6a-debug-control-admission-20261109 stacked on
D5 PR #192. Private, non-executing debug operation previews now bind
Job/plan/build/Running/owner-event/source SHA/OS stopped RIP and sequence.
Same-line breakpoint and same-thread stepping can be reviewed; unverified
variables and arbitrary source locations are denied. Revocation on event
change/cancel/restart, one pending review and max eight previews. No
native DAP request or new execution authority. D5 OS and installed gates
remain OPEN. Owner decision: consolidate related CI suites into ONE batch
at a coherent task-group boundary, no tiny workflows per commit. See
ide/docs/IDE_DEV046A_DEBUG_COMMAND_REVIEW_AND_CI_BATCH_20261109.md.


## DEV-04.5D5 — First-party LLVM Windows event-loop owner and durable sideband (2026-11-09 UTC)

Draft PR #192 stacked on D4D #190. Source-only/native-CI implementation:
the actual pinned LLVM 18.1.8 DebuggerThread event loop has a reviewed
overlay that emits nonce-bound, monotonic \`WaitForDebugEvent\` owner and
\`ContinueDebugEvent\` revocation records through a fourth inherited
anonymous pipe **only** for explicit debug CI. The pipe is made
non-inheritable before launching the inferior. Normal Windows LSP/DAP and
release binaries retain the exact original three-handle list.

The host's bounded native reader validates 76-byte records, one-use
nonce, Job-owned LLDB owner-thread PID, real stopped debuggee PID/TID/RIP,
original sealed PE/PDB/immutable source and both fsynced build/debug
journal chains. It writes a separately fsynced event receipt under the
trusted supervisor journal parent and rechecks it after restart. CI-only
\`--project-debug-owner\`, \`--project-debug-owner-cancel\` and
\`--project-debug-owner-crash\` modes run against the real restricted
Windows Job/SCM and a source-built, hash-recorded patched LLDB, not the
system LLDB. The crash mode intentionally kills the genuine owned LLDB
after an owner receipt, denies stale proof and requires Job cleanup and
Cancelled journal reconciliation.

**Acceptance not closed:** all exact-head Windows jobs were still queued
when inspected; no actual Windows compilation/stop/continue/crash logs or
installed release artifacts yet. The \`ContinueDebugEvent\` revoke path
has parser/contract tests but needs a completed real native stop/continue
sequence. The source-built LLVM Windows build and full native crash run
must pass, then installed-app acceptance must be independently obtained.
Do not expand breakpoint, stepping, variable, evaluate, attach, or
general execution permissions. See
\`ide/docs/IDE_DEV045D5_FIRST_PARTY_LLDB_OWNER_EVENT_20261109.md\`.


## DEV-04.5D4D — Windows debug-port and LLDB owner verification (2026-11-09)

Draft work stacked on PR #189. Read-only native Windows OS corroboration
now checks the kernel's CheckRemoteDebuggerPresent on the actual protected
debuggee, verifies the debuggee's process-table parent matches the real
SCM/Job-owned LLDB process handle, and rechecks the Job membership,
debug port and parent around independent PE/PDB lookup. It retains
`actual_debug_event_owner_attested=false` and adds no debugger actions,
attach, writable process handle or reusable permission.

**Strict stop-event owner identity is NOT proved by this corroboration.**
Microsoft documents that only the original debugger event loop can receive
the actual Win32 DEBUG_EVENT. A reviewed, pinned LLDB DebuggerThread
owner-side hook plus a separately admitted, one-use authenticated IPC
and durable event/continue receipt must be developed and independently
tested in real protected Windows Job/SCM acceptance. A DAP stop, matching
RIP/PDB, kernel debug-port flag, ancestry, or standalone event observer
cannot substitute. These checks have no native exact-head job result yet;
D4C Windows CI is queued (real WO6 run 42584964922, source check run
42584965121). Existing GTK and other source/installed gates stay open.
No breakpoints, stepping, variables or default installed authority.
See `ide/docs/IDE_DEV045D4D_WINDOWS_EVENT_OWNER_AND_NATIVE_GATES_20261109.md`.



## DEV-04.5D4C — Real Lu LLDB-owned stop → OS thread RIP → PDB → receipts (2026-11-09)

Stacked on D4B draft PR #186. **Committed test implementation, native OS acceptance
not yet established.** The D4B read-only Windows Job thread sampler now verifies
the opened Win32 thread handle belongs to the expected process, preventing a
ToolHelp thread-ID recycling substitution.

A new CI-only native Windows DAP method runs during the **real separately
approved Cargo `rust_build` → immutable PE/PDB → SCM/AppContainer Job
`dap_lldb_project` → actual LLDB-DAP `Stopped` + `threads` +
correlated `stackTrace`** acceptance, not the disposable debug-event fixture.
It must recheck the Job-owned thread's OS RIP against DAP, independently
decode that relocated address from the original sealed PDB with a fixed,
SHA/plan-bound test-only LLVM symbolizer, match the compiler source path
against LLDB's source path and the exact approved `src/main.rs` source
snapshot/digest, and revalidate both distinct durable job journals before
and after the lookup. `--project-debug-line` requires another fsynced terminal
receipt retaining the previous Running receipt and original Cargo Completed
receipt, with one-use grant replay denial after reopening.

Only the explicit `dev045d4c-ci-oracle` + debug assertions can run this
fixed-purpose native reader. No renderer/AI launch path, new tool permission,
general shell/attach, breakpoint, stepping, variable or evaluate authority.
Release/default builds omit it and the stable IDE remains unchanged.
**Crucial limit:** independent Windows thread context and PDB equality still
do not separately authenticate LLDB's own internal `DEBUG_EVENT` ownership.
The result deliberately records `lldb_debug_event_owner_attested=false`.
Windows exact-head native results, protected installed workflow and signed
publisher acceptance remain open. See
`ide/docs/IDE_DEV045D4C_REAL_LLDB_STOP_PDB_20261109.md`.

## DEV-04.5D4B Windows stopped RIP and independent PDB line — November 9, 2026

Draft PR #186 stacked on D4 #184. The Windows protected SCM/Job
debugger now supports query-only x64 GetThreadContext sampling of its
real Job-member threads after a correlated LLDB stop; it rechecks
the thread IDs, RIP and job membership, without suspending, attaching
or authorizing debugger commands. **This sample alone is not proof of
a Windows debug event or PDB source line**, because GetThreadContext
may not be valid on running threads. Both authority bits stay false.

A new native fixed-fixture test creates a real Windows DEBUG_ONLY_THIS_PROCESS
child, owns WaitForDebugEvent(EXCEPTION_BREAKPOINT), reads that stopped
event thread's RIP, and independently verifies the loaded PE relocation,
MSF7 PE/PDB RSDS GUID/age, original source hash and LLVM CodeView source
line. Tests must reject a wrong line, wrong RIP, changed source and
rehashed mismatched PDB. Existing SCM/AppContainer protected acceptance
adds --project-debug-thread, and exact-head Windows compiler/fixture
and Linux regression workflows were added. These source changes **do not
prove the real Lu LLDB-owned stop is independently verified**.

All D4B exact-head OS jobs, real protected Lu thread context and PDB
binding, default-installed debugger, UX GTK and signed release gates
remain OPEN until completed native evidence passes. Keep source
breakpoints, stepping, scopes/variables and evaluate disabled. See
ide/docs/IDE_DEV045D4B_WINDOWS_STOP_EVENT_PDB_20261109.md.


## DEV-04.5D4 Windows Job module witness — November 9, 2026 (UTC)

Draft [PR #184](https://github.com/vjmichael/Lu-AI-GUI/pull/184)
stacked on DEV-04.5D3 #181. New native Windows read-only verifier obtains
job members from the existing restricted AppContainer Job handle, verifies
one unique process executable and loaded PE32+ module via Windows process/
ToolHelp APIs, independently checks sealed PE SHA-256, observed loaded
base/SizeOfImage and derives the ASLR-correct preferred instruction
address from the DAP claim. The protected DAP gateway also verifies
the distinct debugger Authorized/Running and original Cargo Completed
fsynced journal receipts plus exact native target/snapshot bindings.
The real SCM/service-backed Windows acceptance fixture now has fixed
`--project-debug-module` mode under the same protected worker; a separate
Windows native CI matrix checks the new Win32 code and Linux regression.
**Implementation only; exact-head native result OPEN.** A process/module
mapping is NOT an independently observed stopped-thread RIP or PDB source
line: both remain explicitly false and breakpoint/step/variables/evaluate
remain disabled. Windows OS thread context + independent PDB line is the
next D4B gate; installed signed-app acceptance remains independent.
See `ide/docs/IDE_DEV045D4_WINDOWS_JOB_MODULE_20261108.md`.


## DEV-04.5D3 protected stop-to-source-line kernel and receipt binding — November 9, 2026 (UTC)

Draft [PR #181](https://github.com/vjmichael/Lu-AI-GUI/pull/181) stacks on D2 #180.
**Source implementation exists, real exact-head native acceptance remains OPEN.**
Found that the Linux cgroup/Bubblewrap debugger tried to launch an OS-private
`/debug-target` project executable without actually mounting the sealed
binary; the associated bind mount was mistakenly placed in the language-server
launcher and referenced a nonexistent `project_target`. D3 moved that
readonly mount to the real native LLDB launcher without changing scope,
cgroup quotas, user authorization or non-executing default installs.

Native LinuxDapSession can now verify a real LLDB-correlated stopped PC
against its own delegated cgroup-v2, kernel ptrace-stop state,
`/proc/PID/exe` device/inode/verified SHA-256, same-process `r-x` VMA,
ELF PT_LOAD relocated image bias, and pinned `/workspace` source snapshot.
It ties all of this to the original **Completed Cargo build terminal
receipt**, exact target binding and distinct **Authorized→Running**
debugger job/plan. A dedicated default-disabled debug CI-only
`dev045d3-ci-oracle` feature verifies the independent DWARF file/line
through a compile-pinned original root-owned LLVM 18 symbolizer; it is
not a production or WebView launch facility. Native project DAP test
acceptance now demands the full witness and a separate fsynced
Completed/Cancelled debugger terminal receipt, with restart replay denial.
No breakpoints, stepping, evaluation, scopes/variables or attach authority.

At latest inspection all exact-head D3 native Windows/Linux jobs were
queued/pending, not successful. The Windows SCM/Job **live** PE/PDB
module-base proof, actual Linux source-line OS receipt acceptance, signed
installed app, older B/C native gates and GTK chooser remain OPEN.
See `ide/docs/IDE_DEV045D3_PROTECTED_STOP_KERNEL_LINE_20261108.md`.
Treat static DWARF/PDB and standalone ptrace trials as partial evidence only.


## B6 real Ubuntu26 native result and EWMH repair — November 8, 2026

B6 native Ubuntu26 run #42518812684 completed 117/117 Node
tests, built the real licensed Debian and installed/launched the GUI. GTK
folder selection still failed: bare Xvfb lacked EWMH _NET_ACTIVE_WINDOW,
so the OS chooser keyboard activation could not run; bounded OS clicks
also failed. B6 now launches native Openbox and witnesses
_NET_SUPPORTING_WM_CHECK in the same installed GTK X11 session.
**Retest OPEN; installed GTK acceptance not obtained.** Windows2022 and
Ubuntu24 native B6 jobs remain queued; genuine B4/B5 memory JSON absent.

Lu Core queue cleanup maintenance run #42518984700:
34 requests accepted and 34 terminal cancelled (0 pending/unknown),
independently verified by individual GitHub run fetches; 267 further
eligible candidates deferred by conservative batch cap. No current-head
checks were in the accepted set. See B6 source doc and the GitHub
maintenance audit artifact for provenance.

## IDE B6 async native read / tab-admission safety — November 8, 2026

On branch `ide/b6-async-editor-admission-20261108` stacked on B5 PR #147
(commit `029f5b13`), source changes guard late native directory/file/search
responses with a synchronous read ticket and current project identity;
revalidate 48/project and 96/global tab capacity AFTER native preview,
preserving a concurrently opened dirty draft; refuse stale reload when
CodeMirror state, buffer revision or registered project changed in flight.
Added five Node regression cases. This fixes a source-level race identified
during B5 review without changing native authorization, file writes or LSP
worker execution. **Native/frontend tests and installed OS race acceptance
remain OPEN.** No B4/B5 CI, OS memory or UI performance gate is considered
closed; B6 is on an isolated branch so existing exact-head PR #147 required
checks remain scheduled. See
`ide/docs/IDE_B6_NATIVE_EDITOR_ASYNC_ADMISSION_20261108.md`.

**Observed native incident unrelated to B5 memory acceptance:** the Ubuntu
26.04 supplemental installed WO8 run #42514897905 installed/launched the
desktop and passed prior failed-upgrade recovery but failed when the GTK
folder chooser remained visible after four real OS accessibility confirmation
attempts. This is a genuine chooser automation failure; the exact-head B4/B5
Windows and Ubuntu 24.04 installed performance JSON artifacts are still
unverified and must not be inferred from the supplemental failure.

## Installed B4/B5 performance evidence follow-through — November 8, 2026

B4 and B5 exact-head native/installed Windows and Linux jobs were still queued or pending at last GitHub inspection, not actual runtime failures. The only completed earlier B3 source matrix #42504529949 failed two stale contract assertions, whose later B2 source fixes are unproven until native CI actually executes. To establish real-world B5 results without creating another redundant build matrix, the B5 branch extends its inherited **genuine B4 unsigned installed NSIS and Debian** OS fixtures. After B4 two-pane work, the same installed binary exercises a 48-source project (including >128KiB and <512KiB files), visible native file-input, 49th-tab denial, 12 first/last tab round trips, typing/Undo/explicit Save, and original file integrity through WebView2 UI Automation/GTK AT-SPI and actual system clipboard. Windows sums installed PID + current Win32_Process descendant working/private bytes; Linux sums /proc descendant RSS/virtual bytes. JSON receipt artifacts report observed P95 open/switch round trips and process-memory samples; they are NOT synthetic Node figures, frame time or total machine RAM. **Neither the measurements nor B4/B5 acceptance has been obtained until exact-head installed OS jobs execute and both receipts are reviewed.** See docs/IDE_B5_LARGE_PROJECT_WORKING_SET_20261108.md.


## IDE B5: large-project bounded editor working set — November 8, 2026

B5 branch ide/b5-large-project-editor-working-set-20261108 is stacked on B4 PR #146 exact head 93e8fad1fa48478e31ffebb62c89d6f1d0c3a2ed. Native cap-std source reader already limits source to 512KiB and folder listings to 400 entries. Desktop source now caps 48 tabs/project and 96 tabs globally without evicting drafts or undo; only 12 tab buttons and first 120 directory rows are rendered, with paged/jump controls, progressive additional directory rows and native capability-scoped direct relative-file open for paths beyond the first 400 entries. Files larger than 128Ki UTF-16 units use lighter CodeMirror syntax/line-wrap setup; secondary protected diagnostics are memoized. Pure Node 10,000-tab synthetic stress benchmark and exact-head native Windows/Linux frontend+locked Tauri workflow committed. **Real installed B4 two-pane acceptance and real OS large-project memory/latency measurements remain OPEN; no synthetic speed claims.** Source doc: ide/docs/IDE_B5_LARGE_PROJECT_WORKING_SET_20261108.md.


## IDE B4: split editor with independent, project-scoped source buffers — November 8, 2026

**B4 installed acceptance source added:** Real Windows unsigned NSIS + WebView2 UIA and Linux Debian + GTK/AT-SPI two-pane editing, per-pane explicit Save and original source SHA isolation workflows are now committed on B4, reusing actual protected-source release notice packaging requirements. Their scheduled GitHub OS runs are **not passing evidence until exact-head jobs complete successfully**. The B4 native workflow now diffs actual reviewed execution/file-write source against its fixed B2 parent rather than checking that a checkout has no unstaged modifications. Original B2/B3 feature gates remain OPEN.


B4 branch ide/b4-split-editor-current-stack-20261108 is stacked on B2 draft PR #145. The visible current IDE editor now supports two different open files in side-by-side CodeMirror panes (or stacked at smaller display widths), with independent source tab state/undo, no duplicated buffer identities, a secondary-file selector, Make primary and explicit Save second file. The global Ctrl+S chooses the focused pane rather than saving the wrong source. Source/project closure and project switching invalidate a stale secondary pane. Pure B4 JavaScript tests and actual Windows/Linux native host+frontend+locked Tauri compile CI are committed, but exact-head CI and genuinely installed Windows NSIS/Linux Debian two-pane editing acceptance remain OPEN. No new native tool permission, LSP release, Lu Core integration or disk auto-save. Details: docs/IDE_B4_SPLIT_EDITOR_20261108.md.


## IDE B2: revisioned editor buffers and private durable draft references — November 8, 2026

**Installed acceptance workflow implementation:** Both real unsigned installed test-only Windows NSIS/UIA and Ubuntu Debian/GTK/AT-SPI workflows are committed to the B2 branch, using native OS chooser/editor/system clipboard and process restart, verifying original disk SHA unchanged until explicit Save. These are source/workflow implementations **not accepted installed OS results**. The recovery vault now denies preexisting group/world-readable Unix private directories and pauses opt-in backups on the 24-checkpoint immutable history cap, preserving copied older references. Exact-head native build/UI results remain OPEN.


Current-stack B2 branch ide/b2-persistent-draft-recovery-20261108, stacked on B3 #143, ports original standalone PR #43 session-only CodeMirror buffer identity and tests (UUID, revision including Undo, exact Unicode/CRLF selection and bytes, Save-rebase rules). It adds an IDE-private fsynced immutable SHA-256-addressed draft recovery vault with CAS latest head, bounded source and 24-checkpoint history, exact restart load, explicit delete and read-only saved-source root prechecks. Four typed Tauri operations + frontend buttons preserve/copy durable reference, detect recovery on reopen, explicitly recover an old checkpoint (including by pasted reference), and never silently overwrite project files. Private snapshots are plaintext local app-data, not a secure encrypted store or network sync; opt-in continuous checkpointing starts only after human Preserve. New native Win/Linux tests and source CI are committed, but native exact-head and actual unsigned installed NSIS/.deb UI/clipboard/restart acceptance remain OPEN until real results. Original B2 #43 and installed PR #45 preserved. See docs/IDE_B2_DURABLE_DRAFTS_20261108.md.


## IDE B3: current-stack read-only Find in Project port — November 8, 2026

A new development branch ide/b3-saved-project-search-port-20261108 is stacked on DEV-02.6B PR #141. The original B3 project-wide saved-source bounded search and native tests from PR #46 are now ported without removing the original PR. The active current Tauri host registers ide_find_in_project and a new source.findProject read-only typed capability; the capability catalog grows from 41 to 42 and preserves all previous operations. The current React/CodeMirror application has a Find in project pane with literal search, match case, skipped/truncated reporting, verified saved SHA-256 hit navigation, unsaved-draft isolation and project-change stale response denial. New exact-head native Windows/Linux CI tests host, UI and Tauri source. **Source committed; CI and genuine installed-app UI acceptance are NOT yet passed.** Original independent B2 unsaved buffer identity PR #43, its installed Windows PR #45, and B3 PR #46 remain open for independent source diff and evidence review. Docs: ide/docs/IDE_B3_PROJECT_SEARCH_CURRENT_STACK_PORT_20261108.md.


## DEV-02.6B original resource closure and native protected three-provider protocol (November 8, 2026)

**DEV-02.6B production-stager correction:** The approved-release verifier now rejects binary-only clangd packages and requires independent cross-OS native resource-file source/SHA inventories; Windows NSIS and Linux Debian staging both copy the reviewed DLL/Clang resource tree and rehash it into the whole-bundle Rust pin. Existing source tests were extended for exact preservation/tamper/path denials. The actual release manifest remains disabled. Exact-head native source/protocol verification and original archive dynamic closure are still OPEN.

Draft PR #141 stacked on #140 creates a fail-closed candidate assembler for *all original release regular-file bytes*, not only the executable, for clangd, rust-analyzer and Ruff on Windows/Linux; maps the original DLL and Clang lib/clang resource paths into per-server fixed worker layout, validates every file SHA+quota and four original notice bytes. A separate Rust fixture uses the real Windows protected SCM/AppContainer/Job or Linux Bwrap/delegated cgroup worker for live initialize, diagnostics, clangd/Rust semantic queries, cancellation, process reaping, and fsynced restart receipts. Manual independent protected runner workflows are committed, plus hosted full-resource and source-compile CI, **but genuine protected execution is NOT established unless these jobs pass at the exact SHA**. ReleaseApproved, redistribution/legal/signing, full dynamic loader closure, installed NSIS/Debian GUI and real user OS consent remain OPEN. See docs/IDE_DEV02_6B_FULL_NATIVE_RESOURCES_AND_PROTECTED_PROTOCOL_20261108.md.

## DEV-02.6A actual native LSP dependency audit and version smoke (November 8, 2026)

Draft PR #140 stacked on DEV-02.5 #138 adds a genuine Windows/Linux CI test-only static PE-import/delay-import and ELF NEEDED/RPATH/GLIBC audit of original pinned clangd, rust-analyzer and Ruff archives. It stages original native library siblings safely in disposable CI and runs only bounded exact `--version` commands, uploading JSON-only receipts. Native exact-head OS results are pending and **no version smoke is accepted until CI passes**. This does not validate full LLVM resource/DLL closure, installed NSIS/.deb app/editor, actual protected Lu worker, LSP protocol, user-consent, legal notice distribution, or signing. Production language-server manifest remains releaseApproved=false. See docs/IDE_DEV02_6_NATIVE_DEPENDENCY_SMOKE_20261108.md.

## DEV-02.5 disposable native LSP candidate install (November 8, 2026)

Draft PR #138 stacked on DEV-02.4 #134 adds Windows/Linux CI-only original SHA256-pinned archive + four actual original-notice verification for clangd 23.1.0, rust-analyzer 2026-11-02 and Ruff 0.17.4. It creates a seven-file sealed test ZIP and hash-installs the files in disposable RUNNER_TEMP with tamper/foreign path denial; only JSON receipts may be uploaded. **Exact-head native CI is queued, not accepted.** This is not an NSIS/Debian installed application, a dependency-closure proof (especially LLVM DLLs), protected-process execution or Pyright semantic release. The existing production manifest remains releaseApproved=false and final signing/legal review deferred. Details: docs/IDE_DEV02_5_DISPOSABLE_NATIVE_LSP_TEST_PAYLOAD_20261108.md.

## IDE P02–P06 native continuation update (November 7)

P05 source integrity now independently pins all 38 required workflow blobs in both preserved audit files; no original checks or PRs retired. On prior exact PR #97 head the genuine Linux protected clangd native normal/cancellation tests and unsigned installed Windows WO8 passed, but Windows protected clangd/LLDB-DAP, Linux fixed LLDB-DAP, Linux installed GTK and cross-platform P02/P06 queued jobs remain OPEN. Current source adds bounded LLDB-DAP alternate response error classification, GTK native chooser caption coverage and owned Windows consent regression guards without broadening tool authority. Actual signed two-version installer upgrades and legal redistribution approval remain intentionally unaccepted; P07/WO10 is deferred.


## IDE 10 — P02 Linux/P06 notice evidence, P03/P04 native repairs (November 7)

The active additional draft development branch `ide/p03-p04-native-closeout-20261107` starts from P06 #95 exact source `55f70b66bdbdfded57ee416011881eb9965904f6`; P01–P06 stacked originals remain draft and none were merged/retired. P02 Windows native source and WO8 installed tests passed at that parent exact SHA, while Ubuntu source, genuine Debian installed GTK, and P06 Ubuntu notice jobs were **queued** at inspection; no Linux acceptance inferred. P06 archived Windows report at that parent SHA records 521 resolved inventory entries and zero missing package-specific notices but the seven special MPL/r-efi and separate embedded-compiler, Apple/native SDK and WebView2 runtime obligations remain unapproved; legal/signing/release approval FALSE.

P03 continuation repaired the malformed Windows clangd native CI inline PowerShell PE-import script (prior native ParserError) and added early exact-script AST parser regression; the PE check is diagnostic only, **not** an AppContainer/SCM/Job/LLDB positive witness. P04 continuation made the real installed native-dialog witness accept owned Window/Pane UIA fragments only with exact button/title and OS InvokePattern (prior Windows WO9 correctly denied missing dialog); and made fsynced disposable upgrade/rollback worker-quiescence admission reject boolean/float/string/null/non-integer counts, with hostile subprocess-based tests. **These new source changes await exact new-head CI and actual Win/Linux installed native results.** Signed two-version installer transitions, production worker reconciliation/rollback, final license clearance and protected native Run/Test/Debug remain OPEN. P05 native-gated PR/workflow retirement deferred; P07/WO10 Lu integration deferred until Lu Core v1.0.


## P06 now has source-committed, OS-reviewed dependency lockfiles (November 7)

Both original CI-approved immutable Cargo.lock and npm package-lock.json bytes have been committed directly as source blobs; reviewed SHA-256 and original Git blob identities are enforced on both OSs. The P06 temporary read-only lockfile exporter is removed; source CI now runs npm ci and Cargo --locked. Native builds/GUI, 48 historic missing license notices (new evidence pending), signed release, independent license review, and protected OS acceptance remain separate release gates.

## P06 governance and dependency reproducibility starts (November 7)

A separate draft P06 branch stacked on P05 now adds dual-OS actual npm/Cargo lock resolution, hashing and saved CI receipts, mandatory original upstream license fetch and complete notice archive, security tests rejecting transitive Tauri drift and release approval substitution, and a default-deny native acceptance matrix. GitHub repository rulesets query yielded [], but main branch protection GET was forbidden to the connector (403); do not infer protection disabled. Exact-head P06 CI and actual lockfile review, native/Signed/upgrade rollback, and legal approval remain open. P07/WO10 still deferred.

## P05 continuation — exact upstream license retrieval and Tauri graph repair (November 7)

Native package runs revealed incompatible old/new transitive Tauri APIs after the initial patch pin. Candidate now uses exact Tauri 2.12 core/API/CLI, validates resolved runtime versions, and runs real upstream text retrieval for all six canonical notices on each E2 verification submission. The source SHA, upstream URLs/revisions/text SHA-256 and npm/Cargo notice artifacts accompany each unsigned test package and the verified hash receipt. Actual OS/installed tests remain needed. All 38 original native/signed CI workflows and original PR branches are retained; P06 not started and P07/WO10 deferred.

## P05 final native-gated operations decision — November 7, 2026

P05 has accounted for every earlier IDE PR (30 original-snapshot heads) and every original IDE CI workflow (38 source-pinned checks), reviewed the seven late divergent patches and preserved unique parallel work. The result is an explicit **retain all** decision, not arbitrary inactivity: no old PR may be closed or retargeted and no security/installed/release CI trigger may be removed before verified P02–P04 native-equivalent coverage. New machine-readable CI owner/disposition matrix and hostile source tests prevent such premature operations. P05 audit/source development is complete; exact-head P05 Windows/Linux CI still required, and physical GitHub cleanup is a **native-acceptance dependency**. P06 not started.

## P05 historical PR reconciliation — November 7, 2026

Source audit complete: 15 genuine WO9 ancestors, 7 late-diverged patches reviewed, 3 B2/B3 unported feature PRs retained, 1 WO10 deferred, and 4 P01–P04 stacked drafts. PR #70 original native Cargo/LLDB receipts are archived in WO6 documentation with exact run IDs and prior-head caveat. Semantic-source tests prevent false PR closure/native approval; operational GitHub PR closure/CI reduction remains gated on P03/P04 installed and security acceptance. [Semantic record](packaging/p05-semantic-reconciliation.json).

## P05 E2 native workflow compatibility repair — November 7, 2026

Two older E2 unsigned installed Windows NSIS workflows were missing required P04 legal resource staging and failed before running installed GUI smoke. P05 now builds exact-source resolved license evidence, includes it in the installer, emits verified package/notice hashes and preserves native installed test steps. Original and reviewed replacement Git blob SHAs are tracked, not silently rewritten. P05's Windows/Linux source audit passed on prior head; native installed E2 revalidation is still outstanding. No security or installed test was disabled.

## P05 exact GitHub source and CI consolidation — November 7, 2026

**P05 in progress, P03/P04 independently open.** A dedicated release-closeout P05 branch stacked on P04 exact head `bc4fd78685c342675e4a80556970e934125a0e20` has inventoried **30 open IDE PRs among 78 open repository PRs**, recording exact branch SHA, base, historical source path, different Git blobs and candidate omissions. Parallel #43/#45 buffer identity/installed proof and #46 Find in Project remain genuinely unported; #76 WO10 contract remains excluded until Lu Core v1.0. All **38 native/source/signed IDE workflows** at P04 source are Git-blob-pinned and retained. Windows/Linux P05 source audit CI refuses unsupported PR closures, dropped native workflow evidence, false signed acceptance and accidental WO10 import. Source checks are not native acceptance, and semantic comparison plus actual PR/CI reduction cannot be marked complete before prior P02–P04 gate acceptance. **No PR closed, retargeted or merged; no protected CI disabled.** [P05 full audit](docs/IDE_P05_PR_CI_CONSOLIDATION_20261107.md).


## P04 verification-package licenses and transactional rollback — November 7, 2026

Full upstream redistribution license texts and actual resolved npm/Cargo per-package legal bytes are now collected and SHA-bound to Windows NSIS/Linux Debian unsigned verification packages. Source CI runs notice integrity and missing-license denial; native installed tests explicitly check embedded Tauri `THIRD_PARTY_LICENSES`. The independently reviewed production signing/licenses gate stays false pending original binary notices and deferred signing identity. P04's new fsynced same-volume transaction retains the previous version and runs real subprocess-crash recovery tests with unchanged user data; it is **disposable-fixture-only**, not an authorized NSIS/dpkg protected-service upgrade. Actual installed GUI, OS migration and genuine mid-installer failure rollback remain open. [P04 report](docs/IDE_P04_INSTALLED_RELEASE_CLOSEOUT_20261107.md).

## P04 installed release and migration checkpoint — November 7, 2026

**Draft PR #84 is OPEN and stacked on P03 PR #80.** Windows native unsigned NSIS installed app now reaches real Jobs UI navigation, but subsequent project scenario had a faulty disposable path guard; source corrected, exact-head installed acceptance still needed. Linux native .deb/GTK display and real picker interaction passed, but `SELECT_FOLDER` remained open with no project persisted; source now selects the exact child folder in a genuine OS chooser, native acceptance pending. WO9 installed Git/ZIP remains dependent on these installed UI repairs. Windows signed-only clean reinstall, two signed-version sequential replacement and injected postinstall restoration plus separate GPG-signed Debian transition test code exist **but have no reviewed signing material, actual signed executable, live OS result or genuine failed-installer rollback**. Never promote a source test to protected native Run/Test/Debug, a signed release or an in-place upgrade. Provenance: [P04 evidence](docs/IDE_P04_INSTALLED_RELEASE_CLOSEOUT_20261107.md), [phase ledger](docs/IDE_RELEASE_CLOSEOUT_P01_P07_20261107.md). P05 source/CI audit is now in progress on a new stacked branch; older P04-native gates remain open.

## P03 native acceptance checkpoint — November 7, 2026

**Draft PR #80 is OPEN.** Real prior-head Linux cgroup-v2/Bwrap and protected clangd editor operations, Windows protected offline Cargo/SCM service-crash reconciliation, and Linux DAP host-crash process cleanup/replay passed. **Windows LLDB-DAP inside AppContainer**, **Linux fixed LLDB debuggee launch/stack/cancellation**, and **exact-head protected clangd/WO6 CI** remain open. The E2 CI workflow was structurally invalid and therefore skipped recent commits; the YAML/PowerShell has now been repaired. Two Rust 1.99 formatter violations in native probe fixtures were also corrected, and bounded DAP failure metadata was added. Do not loosen AppContainer, ptrace, service SID, kernel quotas, or release defaults to claim success. [P03 ledger](docs/IDE_P03_NATIVE_SECURITY_ACCEPTANCE_20261107.md). P04 [draft PR #84](https://github.com/vjmichael/Lu-AI-GUI/pull/84) remains independently unapproved.

## Release Closeout P01–P07 — canonical current phase ledger (November 7, 2026)

**P01 source consolidation complete as a DRAFT integration candidate:** [PR #78](https://github.com/vjmichael/Lu-AI-GUI/pull/78). **P02 is implemented in source but still requires complete native/installed acceptance** ([PR #79](https://github.com/vjmichael/Lu-AI-GUI/pull/79)); **P03 is actively under native security verification** ([PR #80](https://github.com/vjmichael/Lu-AI-GUI/pull/80)); **P04 release/installed workflows are separately in progress, not signed-accepted** ([PR #84](https://github.com/vjmichael/Lu-AI-GUI/pull/84)). The user expressly authorized proceeding to P03/P04 while older CI gates remain open. **P05 source inventory/CI reduction work is now in progress; P06 has not started, and P07/WO10 remains DEFERRED until after Lu Core v1.0.** The authoritative phase/subtask list, exact-source and native test acceptance, outstanding dependency audits and no-autopromotion policy are in [IDE Release Closeout P01–P07](docs/IDE_RELEASE_CLOSEOUT_P01_P07_20261107.md). Phases P02–P06 are sequenced; do not start the next phase until reporting a completed phase and obtaining further user direction.

**P01 review:** The combined candidate adds only `ide/` and IDE GitHub workflows and preserves all original Lu main source and CI. Parallel PRs #43, #45, #46 are **not integrated** and must remain open until source feature review; additional late branch deltas were documented without closure. [Detailed P01 source audit](docs/IDE_P01_INTEGRATION_AUDIT_20261107.md). Release evidence remains default-deny and native CI is red; this is not signed acceptance.

P01 combined current `main` and the last nondeferred IDE source from WO9; preserve Lu Core and original Lu workflows/docs, exclude experimental WO10 bridge branch, and keep all source branches and older PRs open until independent comparison. Earlier independent buffer-identity and project-search PRs have missing/nonancestral source; source coverage and exact-head native acceptance are **NOT** assumed complete. WO9 current installed and protected feature CI is red. No installed signed Lu IDE release is approved.


### WO9 exact-head native repair and restart ledger — November 7, 2026 UTC

**PR #75 draft / WO9-D–F:** Real Linux Tauri installed compile exposed `ReviewedStageReceipt: Deserialize` missing (Rust E0277); fixed with strict deserialize. Native frontend schema inspection incorrectly counted injected `coordinator` as a WebView key; repaired. WO9 Win/Linux host Rust and React tests previously passed on exact head `16fb0ad`; newer source changes require fresh runs. Added **opt-in native-build Tauri compilation** on Windows and Ubuntu to close the prior host-only test gap. WO9-E replay security now demotes persisted completed DAG steps to `native_receipt_unverified` after host restart and requires an explicit read-only exact project/job/plan match with the protected WO6 supervisor journal to unlock dependent work. A running/crashed step is quarantined and never auto-runs. Manual signed Windows installed workflow now exercises a slow native Cargo cancellation and installed-host crash/restart with no dependent auto-run; **not real accepted yet** without reviewed signing and independent OS receipts. WO9-F real installed Win/Linux GUI tests and all signed-release gates remain OPEN. [WO9 work ledger](docs/IDE_WORK_ORDER_09_ADVANCED_WORKFLOWS_20261107.md).


### WO9-D/E/F advanced Git, durable dependency and installed release acceptance implementation — 2026-11-07 UTC

**Draft PR #75** (`ide/wo9-advanced-workflows-20261107`, stacked on WO8 PR #74) now includes all WO9-A–F **source areas**. WO9-D: one-file human-reviewed local Git stage/commit using libgit2 in-memory SHA-pinned bytes, conditional branch-ref update, separate native OS confirmations, no hooks, no push, no untracked/symlink/gitlink staging. WO9-E: fsynced bounded multi-project DAG, dependency-only manual native protected WO6 job dispatch, fail-closed recovery after host restart, terminal native receipt and quarantine; never automatically reruns or grants authority. WO9-F: native Windows unsigned NSIS installed portfolio/Git UIA and Linux .deb GTK/AT-SPI actual ZIP export/readback/uninstall workflow; separate manual genuine signed Windows Authenticode / Linux GPG package plus protected two-step OS job acceptance, and a default-deny independent source/OS receipt verifier. Exact native Windows/Linux compilation, installed GUI acceptance and signed independent release gates remain **OPEN** until evidence on the same commit. Inherited WO8 Windows mistaken executable-name glob and Linux Xvfb missing WM are repaired on WO8 and WO9 branches, not yet accepted until rerun.

**WO9 evidence and work-order ledger:** [docs/IDE_WORK_ORDER_09_ADVANCED_WORKFLOWS_20261107.md](docs/IDE_WORK_ORDER_09_ADVANCED_WORKFLOWS_20261107.md). Do not confuse implementation completion with protected installed Windows/Linux release acceptance. The actual approved signing/certificates, Linux release public key, WO6/WO7 reviewed dependencies and verified signed candidate are absent. Existing native DAG execution uses only the already authorized WO6 supervisor; no second executor and no WO10 Lu bridge. Keep WO9 draft, source code test results separately from release approval.


### WO9 advanced Git/packaging/multi-project workflows started — 2026-11-07 UTC

**Roadmap remains exactly ten work orders:** historical WO1–WO3 acceptance; WO4–WO8 still have native/release acceptance to close; **WO9** advanced packaging, Git, multi-project workflows in development; WO10 optional Lu Core self-development bridge later. WO9 begins in `ide/wo9-advanced-workflows-20261107` **stacked on WO8 PR #74**. WO8's signed installed native cancellation, Linux editor and WO7 release gates are not reclassified as passed or cancelled by this.

**Source implemented (WO9-A/B/C):** `host/src/advanced_workflows.rs` accepts only registered active project IDs (up to eight), uses bounded native libgit2 and `cap_std` artifact inventory to build an ordered multi-project Git/packaging evidence digest and explicit blockers. Native Tauri and the newly enabled **Packages & Workflows** UI provide preview, revalidation, and OS-picker-selected no-overwrite fsynced JSON review manifest. A second explicit action writes a portable stored ZIP of only independently hashed known artifact files, with exact source revision recheck, byte limits, native folder destination, no project-source writes, and no arbitrary command or Git execution. Added real Rust local Git fixture and ZIP readback/source-drift negative tests and a Windows/Linux native host + React UI CI workflow. Native installed acceptance **not yet established**. Details: [WO9 ledger](docs/IDE_WORK_ORDER_09_ADVANCED_WORKFLOWS_20261107.md).

**WO9 open:** reviewed file-specific Git stage/commit without hooks/push, durable multi-project dependent-job coordinator using existing native supervisor and per-turn authority, actual installed Windows/Linux ZIP/GUI acceptance, signed packaging/license closures and final release gate. WO10 bridge is not enabled. No hidden release promotion from green unit tests.


### WO8 durable build/LLDB crash journal repair — 2026-11-07 UTC

Native build and debug status now expose exact supervisor-replayed job journals across IDE restart (source: `app/src-tauri/src/build_backend.rs`, `debug_backend.rs`). The installed Jobs UI shows bounded job ID, phase, plan digest and required OS reconciliation. Full native security quarantine is **not truncated**, even though the display journal is limited to 32. Windows signed installed acceptance now covers real Cargo cancel/desktop crash/service crash/source swap and real DAP cancellation, each requiring observed OS worker, durable exact job receipt and native OS-only cleanup if quarantined. Manual workflow: `.github/workflows/ide-wo8-protected-recovery.yml`, still unexecuted without approved signed WO7 candidate. [WO8 evidence ledger](docs/IDE_WORK_ORDER_08_INSTALLED_E2E_20261107.md). Native Win/Linux installed CI and release acceptance still OPEN.


### WO8 installed native failures, Linux editor and protected crash scenarios — 2026-11-07 UTC

**Current draft:** [WO8 PR #74](https://github.com/vjmichael/Lu-AI-GUI/pull/74) over [WO7 PR #73](https://github.com/vjmichael/Lu-AI-GUI/pull/73). **Native source acceptance pending, final signed protected-feature release unapproved.** Repair made to parent and child: WO7 Windows/Linux preflight test omitted `desktop.exe` fixture after signature validation expanded to three binaries. Restored fixture without bypassing verifier. WO8 GUI native Windows `ExternalConflict` now tests real unsaved draft plus external disk mutation, refusing stale save and preserving original external edit. Linux installed .deb test now uses native AT-SPI through GTK WebKit/X11 to test folder picker, editor, explicit Save, restart persisted source, and external conflict. Native signed candidate workflow `ide-wo8-protected-recovery.yml` attempts real Cargo cancellation, host-crash, restricted SCM crash/restart, and post-plan source-tamper denial; each requires observable OS processes and exact durable job ID/receipt/reconciliation, with no unprotected fallback. Jobs UI now exposes bounded recent receipts to inspect recovery. Updated per-OS per-gate evidence matrix. Documentation: [WO8 ledger](docs/IDE_WORK_ORDER_08_INSTALLED_E2E_20261107.md).

**Remaining separate acceptance:** exact-head GitHub Windows/Linux installed test runs may still be pending/failing, with runtime repairs needed; actual signed WO7 candidate and Windows service/worker credentials not provided; full real signed protected harness cannot be certified; WO6 LLDB native and earlier WO4/WO5 dependencies independently open. Avoid counting source syntax green or unsigned NSIS as signed installed OS acceptance.


### WO8 C–F source implementation expansion — 2026-11-07 UTC

**Draft branch/PR:** `ide/wo8-installed-e2e-20261107`, [PR #74](https://github.com/vjmichael/Lu-AI-GUI/pull/74) stacked on WO7 PR #73. Added installed **CodeMirror accessible editor**, OS-driven real keyboard review/save with pre-save file hash, explicit process-restart project reopen, real Linux .deb install/X11 window/uninstall, and an **explicit manual** signed WO7-candidate Windows UI native confirmation+Cargo/LLDB acceptance workflow. WO8 source-only syntax and native installed workflows are now separate. A fail-closed `verify_wo8_installed_evidence.py` rejects unsupported releases and skipped/stale/digestless/native-wrong-OS claims; all checked-in evidence is unapproved. See [WO8 ledger](docs/IDE_WORK_ORDER_08_INSTALLED_E2E_20261107.md).

**Evidence status:** source committed, native exact-head acceptance not yet credited. Real signed candidate not available, Linux full editor and protected features not tested, and Windows installed external-change conflict/cancel/host/service crash recovery remain open. WO7 and WO6 native prerequisites carried forward. Do not mark WO8 release accepted based on parser tests, a running GUI or unsigned NSIS. Physical Windows/user local data not inspected.


### WO8 started — native installed E2E acceptance (2026-11-07 UTC)

**Stack:** `ide/wo8-installed-e2e-20261107` from WO7 PR #73. The next work order is **real installed application behavior**, separate from source-only protocols and WO7 signing. WO7 package, signing, service and evidence source exists but the released installed protected functionality is **not** accepted. WO6 Linux/Windows LLDB native acceptance and reviewed signing material remain OPEN.

**New WO8-A/B source:** `ide/docs/IDE_WORK_ORDER_08_INSTALLED_E2E_20261107.md`, `ide/app/fixtures/Test-Wo8NativeDesktopUI.ps1`, `.github/workflows/ide-wo8-installed-windows.yml`. The workflow builds actual unsigned NSIS, installs in runner-local disposable directory, drives real native Windows UI Automation against the installed Tauri WebView, navigates Projects/Capabilities/Language services/Job requests, exercises native directory picker, attempts fixture registration/source open, audits no auto-started workers, checks original source SHA, and uninstalls. It does NOT claim signed release or protected Run/Test/Debug. **Native evidence not yet accepted** until latest exact-head Windows workflow passes.

**WO8 pending:** GUI editor save/review, external drift and restart persistence; actual Linux .deb installed GUI; signed installed protected Cargo and LLDB stop/threads/stack/cancel, service/host crash/reconciliation; independent complete native acceptance provenance. Local physical Windows is NOT INSPECTED. No user-owned app or source files modified.


### WO7-B/C/D/E implementation checkpoint — 2026-11-07 UTC

**Branch:** `ide/wo7-signed-nsis-release-20261106`; draft [PR #73](https://github.com/vjmichael/Lu-AI-GUI/pull/73) stacked on WO6 PR #70. The planned **source** for B–E is now present: reviewed, SHA-pinned staged payload/notice tree; PE regular+delay import closure scan; exact Tauri resource handoff; protected per-machine NSIS install/uninstall hooks and bounded LocalService lifecycle; explicitly pinned real signing utility and signed NSIS assembler; Windows-only signed candidate preflight; a real NSIS/SCM installed smoke harness triggered only from a reviewed artifact run; and exact-head per-native-gate evidence consistency checks. **No certificate is supplied; no installer is signed or installed by this code commit; release and protected Run/Test/Debug still not accepted.**

The default unsigned NSIS workflow is deliberately unchanged. `packaging/wo7-release.manifest.json`, `wo7-bundle.inputs.json`, and `wo7-evidence.inputs.json` remain default-deny. Latest source regression run is `.github/workflows/ide-wo7-signed-release-preflight.yml`, tracking the current exact PR head; source-negative CI cannot certify release packaging. Manual signed candidate fixture workflow: `.github/workflows/ide-wo7-installed-acceptance.yml`. Project documents, source paths and remaining acceptance: [WO7 work order](docs/IDE_WORK_ORDER_07_SIGNED_NSIS_RELEASE_20261106.md). **Still pending:** real reviewed binaries, notices and dynamic-dependency closure; signer/certificate; Windows NSIS hook compilation and installed service identity/rollback test; protected installed Cargo Run/Test/LLDB-DAP, cancel/crash/reconcile; actual verified signed artifact and release approval. Work may continue independently of GH CI. Physical Windows source/build tree is NOT inspected.


## WO7 started — signed NSIS preflight (2026-11-06 local / 2026-11-07 UTC)

**Branch:** `ide/wo7-signed-nsis-release-20261106`, stacked on WO6 PR #70. **Status:** WO7-A source implemented, CI and real signing/installed acceptance pending. WO6 coding is complete, but protected native LLDB-DAP initialize/cancel/inspect and Windows installed debugger remain outstanding. No such native test was declared accepted.

- New `packaging/verify_wo7_nsis_release.py` and default-deny `packaging/wo7-release.manifest.json`: signed installer and SCM-service actual binary SHA-256 and Authenticode signer/validity preflight on **real Windows**, exact source head, independently completed license/acceptance records and WO6 debugger approval. No auto sign, fetch, install or worker authorization. Test injection is isolated and cannot make the release CLI accept Linux.
- New hostile source tests on both OSes and source-negative workflow `.github/workflows/ide-wo7-signed-release-preflight.yml`. Neither a green fixture suite nor a passing preflight establishes an installed signed NSIS release.
- Scope/remaining gates: [WO7 ledger](docs/IDE_WORK_ORDER_07_SIGNED_NSIS_RELEASE_20261106.md) — reproducible reviewed dependency/package staging; actual certificate-backed signing; SCM privileged install/uninstall rollback; exact signed installed Run/Test/Debug and cancellation/recovery; independent WO4–WO6 native OS acceptance. **Release disabled; no signing certificate/binary supplied; no installation accepted.**
- Local user machine/source trees: **NOT INSPECTED** in this GitHub-only source change. GitHub branch commits are not installed artifacts or proof of local status.


**Current work-order count: 7 open of 10 (WO4–WO10).** WO1 native Windows compiler/test repair gates passed on the accepted Windows baseline; WO2 protected AppContainer Cargo and WO3 restricted SCM/cancellation/host-and-service-crash acceptance passed together at `bd39d01129f8e4a894edaf3e4a1734fb3dbd1867` ([D7D run 42322328821](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42322328821), [D7B run 42322328831](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42322328831)). This is *native source/OS acceptance*, not installed NSIS release or live LSP enablement.

**WO4 STARTED** on branch `ide/wo4-linux-delegated-containment-20261106`, draft [PR #68](https://github.com/vjmichael/Lu-AI-GUI/pull/68) stacked on the accepted Windows PR #65. Linux worker now checks a separately provisioned, empty `lu-ide-delegated` cgroup parent with host in `lu-ide-host`, requires active CPU/memory/PID controllers, uniquely trustworthy kernel `cgroup.events` emptiness and retains cleanup obligation after `rmdir` failure. Linux admission probe follows the same gate. **Linux actual delegated execution/kill/recovery remains unaccepted** because hosted CI has no writable delegation. See [WO4 ledger](docs/IDE_WORK_ORDER_04_LINUX_DELEGATION_20261106.md). WO5 native LSP, WO6 build/test/DAP, WO7 signed NSIS, WO8 installed E2E, WO9 workflows, WO10 optional Lu bridge remain open.


**Native Windows WO2/WO3 historical behavioral evidence:** at `750638e`, real restricted SCM two-way authentication + READY and release **PASS**; two simultaneous exact-AppContainer-SID NUL leases granted/revoked independently **PASS**; forced host-crash original kernel Job cleanup with fsynced recovery ACK/retry/release **PASS**; forced service-loss cleanup and rejection of missing Job recovery **PASS**; genuine offline Cargo check under protected SCM/AppContainer with output digest and durable restart **PASS**. Workflows nevertheless red due cleanup/negative-probe exit accounting, and protected Cargo **cancellation** was canceled before proving. Repaired SYSTEM/Admin SCM DELETE-only rights, hardened explicit `sc delete` teardown and corrected fixture result accounting; fixed separate D7B reserved `$PID` witness bug. **Final exact-head native all-job acceptance remains pending.** See [Batch 02](docs/IDE_WORK_BATCH_02_20261106.md). No installed worker/LSP promotion.


**IDE 07 WO2/WO3 native admission progress (Nov 6):** Windows CI confirmed exact-host pipe open after minimal no-pipe-create client ACL adjustment, then proved fully authenticated **service server** token/image identity with a query-only token inspection. The service still refused owner admission before READY. Source now impersonates only the kernel-attested connecting client for narrow owner process verification, reverting before all protected Job and permission effects. D7B native owner-crash test dispatch repaired; six Rustfmt hunks applied. Protected Cargo, host/service crash and two simultaneous NUL leases remain **UNVERIFIED at the new exact head**. See [Batch 02](docs/IDE_WORK_BATCH_02_20261106.md). No installed/release worker or LSP enablement.


**WO2 native multi-job acceptance added:** GitHub Windows D7D now has a disposable `--nul-overlap` protected SCM fixture proving two independently authenticated Job-specific NUL grants can coexist and be revoked separately, after the original service admission passes. Fixed-service bootstrap grants only restricted SCM identity the NUL DACL-management rights and attempts cleanup in `finally`. This is source/CI-fixture work, **not a passing OS result**. WO3 service named-pipe authentication and crash/restart remain blocking. [Batch 02 ledger](docs/IDE_WORK_BATCH_02_20261106.md).


**WO2/WO3 native repair continuation:** plain `windows-worker` no longer compiles protected SCM-only recovery symbols (fix for observed Windows E0433/E0425). Applied 31 native Rustfmt 1.99 hunks across NUL bootstrap, device lease, recovery service and client. D7D crash/service-loss CI both stopped at pre-child service admission; CI observers now emit whitelisted native denial stages for precise diagnosis. Corrected source is committed, exact-head OS acceptance pending; release worker and live LSP remain disabled. [Batch 02 ledger](docs/IDE_WORK_BATCH_02_20261106.md).


**IDE 07 / WO3 latest native repair:** restricted SCM listener no longer terminates an existing protected-Job holder solely because a *later* named pipe instance fails ACL readback; first-instance failure still blocks service startup. CI-only host token integrity classification added for exact-user pipe denial, without a fallback IPC channel or broader ACL. Protected SCM handshake and hostile recovery remain unaccepted. See [batch 02](docs/IDE_WORK_BATCH_02_20261106.md).


**IDE 07 / WO2–WO3 active (2026-11-06):** In draft [PR #65](https://github.com/vjmichael/Lu-AI-GUI/pull/65), protected LocalService now has source for per-job OS-derived NUL SID grants, fsynced intent, verified revocation and service-start reconciliation. **New:** concurrent NUL ACL transactions are serialized, and an unexpectedly changed exact-SID ACE cannot be silently revoked; a lock regression test is committed. This is unverified latest-source code, not installed worker acceptance. Real D7B contained Cargo + cancellation already passed at historical `1090800` with disposable CI-only NUL permissions. D7D exact-user SCM named-pipe open remains denied; authenticated crash/restart/service-loss proofs remain open. No release worker, LSP process or installed NSIS feature is enabled. [Detailed WO2/WO3 ledger](docs/IDE_WORK_BATCH_02_20261106.md).


**WO2 production NUL authority bootstrap added for native review:** fixed-service SID, elevated-only, per-boot READ_CONTROL/WRITE_DAC helper; per-job service-owned grant/revoke and boot reconciliation still open. See [batch 02](docs/IDE_WORK_BATCH_02_20261106.md). No production worker launch.

**WO2 pinned Windows native Cargo success (D7B exact commit 1090800):** real isolated Cargo Completed exit 0, output sealed and durable restart; real cancellation Job reaped and no output. CI-only exact-SID NUL permission does NOT finish production privilege lifecycle. **WO3 SCM service still denied at pipe open despite exact user SID match.** Native token diagnostic and nonblocking disposable pipe test added, protected service ACL preserved. See Batch 02 ledger.

### WO2/WO3 active Windows repairs — no release acceptance (2026-11-06)

See [batch 02](docs/IDE_WORK_BATCH_02_20261106.md): live compiler output now uses capability-rooted metadata quota scan at 250 ms, while final full content seal requires a reaped kernel Job; fixed native cap-fs Windows metadata `E0624`, applied actual Rustfmt deltas. D7D installed restricted SCM still requires native pipe admission: exact host SID/pipe-rights diagnostics and a disposable identical-DACL control were added. Service now caps independent handler threads at 12 so pipe instance exhaustion cannot terminate its protected Job keeper. Prior native AppContainer/Job admission passed; real pinned Cargo success, protected service crash/restart and production NUL permission lifecycle remain **OPEN**. No enabled release worker.


### WO2/WO3 active closeout — live native Cargo quota and exact SCM pipe denial (2026-11-06)

The [batch 02 ledger](docs/IDE_WORK_BATCH_02_20261106.md) records new Windows source work: capability-rooted live **metadata** quota scans during mutable Cargo writes (250 ms) while immutable content seal remains post-Job exit, Windows cap-fs `E0624` compiler repair, bounded native cancellation diagnostics, and confined in-root rename test. D7D protected SCM pipe denial now has exact host TokenUser diagnostics and a disposable same-ACL pipe control; production pipe DACL, host binary/hash and restricted LocalService identity were **not** loosened. Earlier exact native AppContainer and worker-containment probes pass; pinned Cargo success and D7D service host-crash/restart **do not**. Production NUL permission lifecycle is unimplemented. Draft PR #65, no release activation.


### D7D recovery ACK retry and Windows Cargo release gate (2026-11-06)

**Work orders 2 and 3 are still open.** On draft [PR #65](https://github.com/vjmichael/Lu-AI-GUI/pull/65), native AppContainer admission and Windows Job worker-containment passed on exact historical head `c6d288e`; this is **not** proof that isolated Cargo compiles or that the separate recovery service passes. The real pinned Cargo still needs complete kernel/AppContainer/NUL acceptance and release-safe privilege handling.

- WO2: `windows_null_device` now has a **compile-time refusal** of a release build carrying disposable `ci-native-null-lease`; the native Cargo host fails *before* an AppContainer, protected service lease, or one-shot grant when a safe NUL-access path is unavailable. A separate privileged, crash-safe per-job production device-access lifecycle and real rustc execution remain unimplemented/unaccepted. No broad ALL-APPLICATION-PACKAGES ACE is deployed.
- WO3: `windows_service_security` queries the created named-pipe DACL as the correct `SE_FILE_OBJECT` Windows security object type; it retains exact installer-approved host TokenUser, hash, kernel image and client PID checks. After recording and fsyncing `recovered_release`, the service retains the **original empty kernel Job** for a 60-second ACK retry window. The retry checks real Job emptiness, protected evidence and freshly authenticated client/nonce, and cannot start in live/unreaped/pending states. The independent native D7D crash fixture now verifies the second ACK and eventual bounded Job release.
- Commits: `220d4a4` (pipe object type), `57bd66f` (fsynced ACK retry and compile-time CI safety), `c6d288e` (fail-closed native Cargo before approval), `2b336e3` (native ACK/release test). **Exact-head D7D SCM service, host-crash recovery, service-loss refusal, pinned offline Cargo and installed NSIS are pending native CI/physical acceptance.** Never mark those passed from source code alone.
- The original batch remains held on WO2 and WO3: subsequent WO4–WO10 are not activated merely because GitHub jobs are queued.


### WO3 actual named-pipe ACL attestation added (2026-11-06)

`d65c202` inspects the *kernel-created* protected SCM pipe DACL prior to reporting running status: protected, exactly SYSTEM/service/exact-host SID; no unknown ACE; no client FILE_CREATE_PIPE_INSTANCE right. This follows native job `128199760472` still returning pipe access denied despite exact-user SDDL. Debug-only `3539186` will classify read/write/control access denial to guide the next targeted repair. D7D authentication, owner-crash and restart acceptance are NOT yet proven. [Batch 02 ledger](docs/IDE_WORK_BATCH_02_20261106.md).

### Confirmed D6 Windows isolation and D7C/D7D ABI repair — WO2/WO3 STILL OPEN (2026-11-06)

Native D6 AppContainer job `128199718599` actually passed zero-capability, host secret denial, live loopback network isolation and Job reaping; its workflow failed only on Rustfmt, now repaired by `6fc0b36`. The independent D7C witness returned Win32 24 because its C# Job accounting ABI was 32 instead of 48 bytes; repaired in both D7C/D7D native crash witnesses by `958609d`. Protected SCM service client job `128199760472` still got pipe access denied after the exact-user DACL and v2 config; CI/debug-only read/write/control denial classification was added in `3539186`. **Neither production Cargo null-device provisioning nor protected service authenticated host-crash/restart is accepted.** See [batch 02](docs/IDE_WORK_BATCH_02_20261106.md). GitHub native CI is verification, not a development stop.

**WO2/WO3 continuing closeout:** CI-only native NUL DACL work now additionally compile-time gated with `ci-native-null-lease` (not in release/default features). Independent D7C/D7D crash observers now record native Win32 accounting query failure codes; no failed query can certify an empty Job. The release-only NUL capability/installer approach remains an unimplemented security gate; protected SCM crash and restart proof remains open. See [batch 02](docs/IDE_WORK_BATCH_02_20261106.md). Native exact-head acceptance still required.


### Work orders 2–3: resumed implementation; native security closeout remains open (2026-11-06)

[Batch 02 code/evidence ledger](docs/IDE_WORK_BATCH_02_20261106.md): WO2 AppContainer now checks real token zero capabilities, controls before/after live network probe and child listener absence, bounds real kernel Job reaping, and has an explicitly CI-only exact temporary NUL SID grant with verified normal-exit revocation after sealed staging and protected broker approval. This is **not** production-safe across host crash, so production NUL lifecycle and real confined Cargo pass remain open. WO3 service uses installer-approved exact host account SID (not broad IU) for pipe ACL/TokenUser and v2 config, starts SCM RUNNING only after creating the protected endpoint, fixes installer PowerShell parse and bounds independent kernel Job observation. Real SCM IPC, owner-crash/service-loss restart recovery remain unaccepted. Related Windows/Linux core supervisor unit CI on historical `8e5aa5a` passed; not native OS execution acceptance. PR draft, no release worker. Details in linked ledger.


### First three-work-order batch — source repairs committed; native gates OPEN (2026-11-06)

See [IDE batch 01](docs/IDE_WORK_BATCH_01_20261106.md). Work order 1: fixed Windows LSP writer HANDLE thread transfer (`E0277`), acceptance example feature gating (`E0432`), genuine Tauri frontend assets and post-crash preflight fixture. Work order 2: actual AppContainer child kernel token capabilities inspected; Cargo still cannot open `\\.\NUL` and the D6 network denial still lacks WSAEACCES evidence. Work order 3: added fixed-stage pipe admission triage and CI logon classification; protected SCM actual IPC and restart/recovery are **not** accepted. Commits `2076532`, `0e941c7`, `3c38f5e`. Fresh exact-head CI pending. Next unblocked work remains closing orders 1–3, then 4–6; orders 7–10 later. No unsafe worker or LSP activation.


### E2 Windows CI repairs — native evidence, not release acceptance (2026-11-06)

PR #65 native Windows logs identify two independent defects: AppContainer Cargo cannot open Windows `\\.\NUL` for rustc child stdin (Cargo 101, compiler never started); a hostile rename test incorrectly required an unchanged *within-root directory name* while simulating an attacker. The latter test is corrected to require copied-file integrity, matching output digest and no external secret. Native Rustfmt 1.99 failures in service security and two probe fixtures were repaired. The device access problem **remains blocked** without explicit independent least-privilege OS authorization/proof. Cancelled checkout jobs had misleading manifest-not-found messages; they are not Cargo path bugs. See `ide/docs/E2_NATIVE_CI_FAILURE_TRIAGE_20261106.md` for exact jobs, commits and continuing gates. No protected LSP, signed installer or installed-app acceptance is claimed.


### E2B native integration continuation — 2026-11-06

- **Language routing:** the feature-gated Windows Tauri backend now selects clangd for C/C++, rust-analyzer for Rust, and Ruff for Python diagnostics only. The host independently validates filename extension and language ID, checks each server against its own compile-time approved installer hash, and publishes only installed, verified language IDs to the editor. JS cannot start unverified catalog-only servers.
- **Native unit-level lifecycle coverage:** protected gateway mocks now exercise Running → ReleasePending → Completed with durable replay, Running → CancelRequested → Cancelled on drop, and service-ACK failure quarantine after supervisor restart. These are authored source tests, **not kernel proofs**.
- **Installer staging:** ide/packaging/stage_language_server_release.py requires a reviewed two-platform manifest, exact service SHA-256, OS Authenticode check, native binary hashes and license notices before staging a per-machine NSIS resource tree. ide/app/src-tauri/tauri.e2.nsis.conf.json keeps this separate from the default non-executable alpha. Restricted service installation, signed packaged binaries and real installed-app acceptance remain open gates.
- **CI queue hygiene:** E1/E2 workflows now coalesce superseded branch jobs; exact-head native results remain queued. This is not released or installed E2 execution.

### E2 native Windows acceptance follow-up — observed failures, no release promotion

Confirmed from exact Windows CI: D7B securely launches Cargo under Job/AppContainer, but Cargo cannot open Windows `NUL` for compiler child stdin (OS error 5); nested `rustc -vV` is never executed and Cargo exits 101. **Do not widen device ACLs** or claim a compiler acceptance pass. D7D service startup status 1066/substage 103 diagnosed an incorrect primary-token argument to `CheckTokenMembership`; corrected with impersonation-token duplication at both service self-check and host service authentication, with Windows regression. Legacy unprotected-only reconciliation test now properly feature-gated and Tauri CI generates its deterministic missing Windows icon before `cargo check`. The exact CI run/job evidence and commits are recorded in `ide/docs/E2_NATIVE_CI_FAILURE_TRIAGE_20261106.md`. All changes pending new independent exact-head OS acceptance; E2 release, LSP execution and signed NSIS remain **disabled**.

### E2 follow-up native CI repair and installed desktop smoke (2026-11-06)

On PR #65: sealed Linux LSP snapshot tamper fixture repaired without weakening file permissions, exact CI rustfmt hunks applied across host/supervisor, real Windows AppContainer Cargo `Failed` stage given bounded metadata and D7D protected broker admission given debug-only stage codes. **Windows D7B/D7D kernel acceptance remains red**; no mocked receipts count as acceptance. Hosted Linux has no writable delegated cgroup; OS containment must be proven on a suitable independent runner. Added an **unsigned default-alpha NSIS** install/two launches/uninstall test path on a disposable Windows Actions host, without native LSP, service or release claims. No signed E2 binaries/installer, installed LSP editor E2E or host-crash/service restart acceptance yet. Full exact-head job list and commits: `ide/docs/E2_NATIVE_CI_FAILURE_TRIAGE_20261106.md`.

### E2 D7D/D7B native failure repair update

Actual newer Windows CI advanced beyond previous parser and VS pin failures: VS2022 Rust 1.99.1 / MSVC 14.44 and Windows SDK 10.0.26100 built the protected service/worker; sc.exe 1639 was repaired; subsequent test created/started `LuIdeRecoveryBroker` with restricted ACL evidence. The D7D fixture's legacy broker path requirement was repaired. The next protected Cargo acceptance **still failed `SUPERVISOR_OS_CONTAINMENT_NOT_VERIFIED`** (run 42286298966) and requires native stage tracing. D7B long stage returned **`SUPERVISOR_GRANT_EXPIRED`** (42286299058); source now permits a fresh host approval only *after* sealed staging, without extending the fixed 60-second grant. These repairs are **not** Windows OS acceptance passes. E2 installed NSIS and release remain explicitly disabled and unverified. Exact logs, job IDs, commits and open gates: `ide/docs/E2_NATIVE_CI_FAILURE_TRIAGE_20261106.md`.

### E2 native verification and exact-head repair ledger (2026-11-06)

Actual failed CI logs, their job IDs, and the corrective commits are recorded in `ide/docs/E2_NATIVE_CI_FAILURE_TRIAGE_20261106.md`. Repaired confirmed Windows compiler failures (Job Object unsafe-expression parser and `windows-sys` token import), absent VS 17.14 on Windows 2025 (pin offline fixture to Windows 2022), missing `rustfmt` on pinned Rust 1.99.1 jobs, and five native E1 LSP formatting hunks. D6 AppContainer exit `1099` is now classified but **does not pass** zero-network acceptance. Updated PR #65 native jobs are queued, not accepted. Do not enable/default-release the E2 worker.

## Wave E2 — Supervised language services and editor presentation (2026-11-06)

- **Independent real-server protocol smoke:** local `clangd 17.0.0` stdio fixture successfully exchanged initialize, versioned diagnostics, hover, same-file definition and completion messages. Sanitized wire frames in `ide/host/fixtures/clangd17_real_wire.json` are replayed by Rust E2 tests. This is **not** D7D contained server execution or installed IDE acceptance. Restricted clangd completion text edits are now accepted only for the current identifier prefix/cursor after human selection.
- **Branch:** \`ide/wave-e2-supervised-lsp-editor-20261106\` stacked on Wave E1 PR #64; source-only integration, not a release-ready real server session.
- **Native host protocol:** \`lsp_session.rs\` implements LSP initialize/initialized, revision-bound didOpen/didChange, hover/completion/definition requests, exact-version diagnostics, explicit workspace/configuration nulls, and denial of server-initiated edits and commands. Tests use bounded actual LSP JSON-RPC wire frames.
- **Protected supervisor bridge:** optional \`lsp-transport\` internally binds a running D3 job and plan hash, complete OS-proof contract, per-chunk quotas, and process-tree kill. It cannot launch an executable itself; Windows protected Job/SCM and Linux delegated cgroup pipe adapters remain missing and MUST be implemented before activation.
- **Human editor:** CodeMirror now supports external, exact-source diagnostic underlines, click-to-navigate problems, plain-text hover, suggestion preview and explicitly clicked completion insertion, same-file definition navigation, and source-revision invalidation. Language Services view shows reviewed descriptive providers. Actual native \`ide_lsp_worker_status\` returns \`LSP_PROTECTED_WORKER_UNVERIFIED\`; live actions stay disabled.
- **Installer:** \`ide/packaging/language-servers.manifest.json\` candidates: native rust-analyzer, clangd, Ruff; Node-dependent Pyright/TypeScript deferred. \`verify_language_servers.py\` refuses unreviewed assets, wrong digest, missing notices, path escapes, or release without explicit approval. **No server is downloaded or bundled.**
- **Status:** source changes committed with cross-platform CI/hostile checks, not native-tested or runtime-accepted. Wave D6/D7D OS security tests, true LSP restricted child admission and installed Windows/Linux editor tests still open. See \`ide/docs/IDE_WAVE_E2_SUPERVISED_LANGUAGE_SERVICES.md\`.

## Wave E1 — Free language-server catalog and framed protocol foundation (2026-11-06)

- **Source:** `ide/wave-e1-lsp-catalog-transport-20261106`, stacked on D7D PR #63. E1 is a new source-only slice and intentionally does **not** close Wave D.
- **Open-source language servers:** native host describes rust-analyzer, clangd, Pyright, Ruff, TypeScript Language Server, and VS Code-derived HTML/CSS/JSON servers. No automatic download, installation, PATH search, process launch, Lu IPC or project execution.
- **Protocol:** bounded incremental LSP JSON-RPC 2.0 stdio frame parser and serializer, strict Content-Length, typed disk+unsaved-buffer diagnostic identity, and non-executable `lsp.catalog` native capability exposed read-only through `ide_list_language_servers`.
- **Tests/CI:** Rust parser/identity regressions, JS command-boundary assertions, cross-platform E1 workflow added. Source commits only; actual native CI, installed desktop LSP features and server lifecycle acceptance **not yet passed**.
- **Next:** E2 authenticated supervisor-controlled LSP lifecycle; completion/diagnostics/hover in the human editor; eventually LSP navigation/refactoring, build/test and DAP. Block worker launch on Wave D security acceptance. See `ide/docs/IDE_WAVE_E1_LSP_FOUNDATION.md`.

## Wave D verification extension — real protected Cargo and hostile service-loss (2026-11-06)

- Added a third D7D native Windows CI job exercising the **actual** \`d7_windows_acceptance\` offline Cargo check and cancellation with \`protected-recovery-service\` and an independently installed SCM service. The app itself still exposes no Run button.
- Added a fourth Windows job forcibly killing the separate protected service while the approved fixture owner and fixed contained child are alive, requiring host-side liveness detection and Job kill, and denying restart recovery when the old named Job is absent.
- Corrected restricted-service pipe SDDL to use a protected DACL without forcing SYSTEM ownership from the LocalService process; pre-acceptance source repair, not a Windows OS pass.
- **Wave D cannot be marked accepted:** current exact-head CI remains queued; prior D6 live-loopback probe returned \`1099\` (no raw network error, not WSAEACCES), Linux delegated cgroup acceptance, signed installer and actual installed Windows E2E remain separate gates. No worker is enabled in desktop. See \`ide/supervisor/D7D_PROTECTED_SERVICE_ACCEPTANCE.md\` and PR #63.

## Wave D7D — Restricted Windows recovery service (2026-11-06)

- **Source branch / PR:** \`ide/wave-d7d-protected-windows-recovery-service-20261106\`, draft PR #63 stacked on D7C PR #62. This is NOT native Windows or installed-desktop acceptance.
- **Trust boundary:** restricted LocalService / service SID principal; protected direct ProgramData evidence root and verified ACL; installed SHA-pinned binaries; mutual named-pipe authentication by kernel client/server PID, token and image; noninteractive request scopes cannot execute commands.
- **Execution admission:** \`protected-recovery-service\` feature switches worker away from D7C same-user child keeper to independent SCM service. Explicit Global Job DACL permits restricted service to hold real kernel handle; service fsyncs READY before child resume and enforces normal empty release. Broken IPC kills and retains the Job for reconciliation, not premature release.
- **Crash recovery:** durable RecoveryRequired preflight retains quarantine, independent original Job inspection, protected service-recovery fsynced ACK, final live Job re-verification and Cancelled receipt. A missing Job or ACK remains quarantined.
- **CI:** native Windows SCM smoke and independently observed host-crash/child-kill/restart recovery jobs added in \`.github/workflows/ide-d7d-protected-service.yml\`; exact-head CI remained queued at this documentation update. Do NOT claim tests passed. See \`ide/supervisor/D7D_PROTECTED_SERVICE_ACCEPTANCE.md\`.
- **Remaining:** Windows native compile and repair; real full D3 supervisor + AppContainer worker restart; service-crash and malicious IPC fault injection; signed installed binary and earlier D6/D7B acceptance. Desktop Run and Lu Core bridge **remain disabled**.

## Wave D7C — Independent native Windows recovery keeper (2026-11-06)

- **Implementation:** `ide/supervisor/src/windows_broker.rs` plus fixed `src/bin/windows_recovery_broker.rs` now implement a second native process that independently holds the named Job before worker grant consumption / resume. Exact installed binary SHA is required by native `TrustedBroker`, and the supervisor PID + kernel creation time are authenticated against its live process handle.
- **Crash protocol:** fixed broker confirms its Job handle, fsyncs `ready` to an independent bounded hash-linked journal and acknowledges via a private parent-child pipe; the worker refuses launch or quarantines if the broker is not alive. On supervisor death the broker durably records kill intent, calls `TerminateJobObject`, verifies zero active descendants, fsyncs the observation, and retains the original named Job for up to 15 minutes for independently verifiable restart reconciliation. No automatic relaunch.
- **Private release:** normal terminal release uses a one-use nonce on an inherited anonymous stdin pipe **after** the D3 terminal receipt and empty-Job verification; no publicly signalable normal-release event. A separate named event is accepted only after parent death plus Job cleanup, and pre-signals are reset.
- **Adversarial fixtures:** native `d7c_broker_crash_fixture.rs` and PowerShell `Test-D7CProductionBrokerCrash.ps1` force a real parent crash from an external observer, prove child kernel exit and a still-live empty named Job, validate durable broker-event order, try a forged early release, and cleanly release the recovered keeper. Windows D7C workflow now builds/runs these fixtures; D7B offline Cargo fixture also requires an actual broker binary.
- **Release hold:** all latest-head Windows CI and installed Windows tests still pending. The new broker is an **independent process under the host identity**, not a separately protected/attested Windows service principal, and its same-user journal hash chain cannot authenticate itself. A signed separate-identity broker service with protected ACLs / trusted IPC is still required for malicious same-user recovery authority. No desktop/Tauri Run command activated. See `ide/supervisor/D7C_WINDOWS_CAPABILITY_ACCEPTANCE.md`.

## Wave D7C — Handle-rooted Windows worker staging and publication (2026-11-06)

- **Source:** `ide/wave-d7c-race-safe-native-worker-20261106`, stacked on D7B PR #60. No Tauri Run command or Lu Core bridge. Native worker remains an explicit `windows-worker` feature.
- **Security refactor:** new `ide/supervisor/src/windows_cap_tree.rs` pins `cap-std` and `cap-fs-ext` 4.0.3 and uses Windows directory handles without FILE_SHARE_DELETE, root FILE_FLAG_OPEN_REPARSE_POINT, handle-relative child enumeration/opens, explicit no-follow files, hardlink/reparse rejection, bounded recursive SHA-256 scanning, and native-parent-relative create-new publication. Removed the old ambient `tree_transfer`.
- **Authoritative worker integration:** the same rooted publisher now stages the verified source snapshot and pinned offline Rust/MSVC/SDK bundle, then copies and reseals outputs after Job tree reap. Exact input and bundle digests are refreshed before the one-use native grant. Reused D7/D7B receipts, Job cancellation and missing-Job quarantine without loosening denial.
- **Hostile tests/CI:** new race test attempts repeated concurrent rename/junction swaps while requiring every accepted copy to match the pinned input; native output exclusive publication, traversal rejection, symlinked parent refusal, staged tampering. `.github/workflows/ide-d7c-windows-capability.yml` runs Windows compile/tests, repeat race, formatting, desktop isolation.
- **Pending:** latest-head Windows CI, prior D6 network deny and D7B offline Cargo fixture, secure authority for root selection/AppContainer profile ACLs against hostile same-user modification, independent authenticated durable witness for a named Job absent after crash, and installed Windows app acceptance. D7C is **source-implemented, not production-accepted**. Details in `ide/supervisor/D7C_WINDOWS_CAPABILITY_ACCEPTANCE.md`.

## Wave D7B — Pinned Windows SDK and hostile acceptance (2026-11-06)

- **Source / PR:** `ide/wave-d7b-windows-acceptance-20261106`, draft [PR #60](https://github.com/vjmichael/Lu-AI-GUI/pull/60), stacked on D7 #58; preserve D6 → D7 → D7B merge order.
- **Build fixture:** `ide/supervisor/fixtures/New-PinnedWindowsToolchain.ps1` packages Rust 1.99.1, native MSVC 14.44.*, VS 17.14.*, Windows SDK 10.0.26100.0 and the actual UCRT/UM headers/libs and compiler tools into an offline bundle with bounds, no reparse points and full canonical per-file SHA-256 lock outside the runnable payload. **Release pin remains unapproved until an independently reviewed expected bundle digest is supplied.**
- **Worker integration:** D7 native Windows worker environment now uses sealed Cargo/rustc, linker, SDK binaries, `LIB`, `INCLUDE`, fixed offline Cargo target; Windows GetFileInformationByHandle rejects hardlinked files and tracks directory identity across path walks. The latter does NOT prove full handle-relative race immunity.
- **Hostile native acceptance:** Windows tests for hardlinks, directory symlinks/junctions, source parent swap, post-seal tampering, output aliasing, bundle digest drift, and missing-named-Job restart quarantine; `d7_windows_acceptance --cancel-native-worker` requires genuine OS Job cancellation, no published outputs, durable CancelRequested → Cancelled replay.
- **Independent crash observer:** opt-in D2 fixed native Job parent-crash mode plus `Test-IndependentJobCrash.ps1` separately witnesses two live descendant process handles before forcibly killing owner and confirms both OS handles signaled. This is **fixture-only**, not an authenticated D7 crash-recovery receipt.
- **CI:** `.github/workflows/ide-d7b-native-acceptance.yml` targets Windows 2025 for separate hostile, crash and full offline Cargo checks. Latest-head CI and real SDK AppContainer execution **pending**; D7B remains draft, worker remains off by default, no desktop Run or Lu Core execution bridge.
- **Native D6 blocker:** exact-source [Windows AppContainer run 42228397045](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42228397045) compiled/formatted but failed with child exit `14` (live-loopback WinSock error other than verified `WSAEACCES`). The follow-up D6/D7/D7B probe now encodes `1000 + raw WinSock error` on this failure to identify the OS condition; it remains a **failed gate**, not a network-isolation pass.
- **Remaining risk:** same-user concurrent swap race and independent durable witness for a missing Job after host crash/reboot are unresolved. See `ide/supervisor/WINDOWS_WORKER_ACCEPTANCE.md`.

## Wave D7 — Integrated Windows native worker (2026-11-06)

- **Draft PR #58:** `ide/wave-d7-windows-native-worker-20261106`, stacked on Wave D6 PR #56 source `62986780b752f7add62f93c818a8cbac796660c8`. Do not merge in reverse order. No installed app or Lu Core worker permission added.
- **Native Rust implementation:** `ide/supervisor/src/windows_worker.rs` is an explicit **off-by-default** `windows-worker` Cargo feature. It connects a reverified `PublishedInput` and host-pinned offline toolchain SHA-256 to a fresh profile-owned AppContainer staging tree, fixed Cargo check/test commands, a private named Windows Job with CPU/memory/PID quotas, and zero-capability `SECURITY_CAPABILITIES` process launch.
- **Crash-window hardening:** `PROC_THREAD_ATTRIBUTE_JOB_LIST` attaches the child to the Job *during* `CreateProcessW`, prior to any instruction. Suspended child token + SID are verified. Parent Job handle is kill-on-close; launch has no unrestricted fallback.
- **Durable outcomes:** existing D3 one-use native approval, admission, fsynced running/cancel/uncertain terminal receipts are reused; native output copying is content- and budget-checked and terminal receipt links published SHA-256. Faults immediately quarantine new work. Restart never auto-relaunches; the only completed Windows recovery path requires a still-existing kernel Job that can be killed and queried empty.
- **Open gates:** D6's `LOCALAPPDATA` Windows error-203 repair, D7 exact-head compile/format, full pinned offline toolchain+SDK, real native Cargo fixture, hostile race/cancellation/crash matrix, durable missing-Job witness and physically installed Windows acceptance. Full production worker **not verified or activated**. See `ide/supervisor/WINDOWS_WORKER_ACCEPTANCE.md`.

## Wave D6 — Windows AppContainer + Job Object native admission (2026-11-05)

- **Branch:** `ide/wave-d6-appcontainer-worker-20261105`, stacked on D5 production-worker source `341b3ab5ea268770bb01fdcf32cbb8d06452b095`. D5 Rust supervisor Windows and Linux native CI successful after formatter repair; desktop installed acceptance still open.
- **Windows fixed fixture:** `ide/sandbox/src/bin/windows_appcontainer_probe.rs` now creates a fresh *zero-capability* AppContainer profile, stages only its own fixed executable inside the profile-owned LocalState directory, uses `SECURITY_CAPABILITIES` in `STARTUPINFOEXW` with suspended `CreateProcessW`, creates/configures Job Object, assigns suspended child before resuming, and checks child AppContainer token / denied host secret read / exit / zero active Job processes. Any OS admission failure denies execution; no arbitrary app or project build.
- **Verification:** dedicated `ide-appcontainer-windows.yml` native Windows CI is required; fixed runtime test and formatting pending until exact-head result. D4's two unformatted OS rights probes are fixed in this stacked branch from the actual prior CI output.
- **D6 native CI investigation (latest code head before this documentation update: `56b0dfa15ed71e2979af9b53b7220f287b72e72b`):** initial real Windows AppContainer run [42163105747](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42163105747) compiled and formatted but failed at `CreateProcessW` (`ERROR_ENVVAR_NOT_FOUND 203`). Repaired minimal, correctly NUL-terminated Unicode `SystemRoot/TEMP/TMP` bootstrap environment; secret must preflight readable in the host then be denied with `ERROR_ACCESS_DENIED` in AppContainer; added a live localhost listener and required `WSAEACCES` instead of interpreting any network failure as isolation; reject uncontained `CREATE_SUSPENDED` orphan on Job assignment failure with direct terminate/wait. **Exact-head native runtime acceptance is pending.** These fixed hostile tests do not authorize project code, Windows Cargo execution or an installed IDE Run command.
- **Remaining:** AppContainer network-denial hostile test, secure Cargo/toolchain staging into per-job container, real Windows job/cancel/timeout worker adapter integrated with D3 grants, crash recovery, race-safe snapshot traversal and installed Windows UI acceptance. The desktop `job.run` remains deliberately absent; do not call full production worker complete.


## Wave D5 — Native production-worker implementation, gated (2026-11-05)

- **Source branch:** `ide/wave-d5-production-worker-20261105`, stacked on Wave D4 PR #54 head `32b208a76ed17910554b8ca6b7e8647577e6657a`. D3 and D4 remain separate draft PRs; do not merge D5 before them.
- **Real input staging:** `ide/supervisor/src/snapshot.rs` now copies bounded regular files from the registered source into a unique private staged directory, excluding generated caches and rejecting symlink/reparse/device files, unexpected names and budget overruns. A deterministic SHA-256 tree seal and *recipe-specific* Cargo manifest digest are reverified on staged bytes before execution. Private native source/privileged toolchain identity must still come from a trusted host component.
- **Linux native worker code:** `ide/supervisor/src/linux_worker.rs` (opt-in `linux-worker` compile feature) implements real fixed offline Cargo check/test execution via root-owned unprivileged `/usr/bin/bwrap`, isolated user/PID/network/mount namespaces, read-only `/workspace`, writable private `/output`, cleared environment, and resource-limited cgroup-v2 admission performed in `pre_exec` **before** the launcher runs. Requires delegated CPU/memory/PID controllers and actual cgroup.kill; fail closed without them.
- **Cancellation and recovery:** native cancellation requests are journaled before `cgroup.kill`. Process tree must report empty via `cgroup.events` before cancellation is completed; failure becomes uncertain/quarantined. On restart, the worker can attempt *explicit* cleanup of a matching existing per-job cgroup, never relaunch the job. Log SHA-256/size bounds, output scanner, per-file kernel limit and short runtime watchdog are present.
- **Authorization integration:** D3's native-only one-shot grant is consumed after scoped input/toolchain/OS admission evidence and before `spawn`; durable admission/running/terminal records are appended with native host state. The separate D1 review journal still cannot mint an executable grant.
- **Not yet ready for user execution:** Linux actual worker integration with installed Tauri, Windows restricted/AppContainer+Job worker, race-safe cap-std source traversal, OS crash/kill hostile acceptance and physical installed Windows acceptance are **not complete**. No production `job.run` command is advertised. CI compile/tests and actual delegated cgroup runtime validation pending; do not claim Wave D complete.


## Wave D4 — Native restricted-rights admission probes (2026-11-05)

- **Branch:** `ide/wave-d4-native-rights-admission-20261105`, stacked on D3 source commit `080b15dbfbc231dc2b0b691cb65a03b20d1c5794`. Production supervisor remains independently gated and disconnected from Tauri.
- **Windows:** new `ide/sandbox/src/bin/windows_restricted_token_probe.rs` validates OS `CreateRestrictedToken` using a fixed Restricted Code SID plus disabled maximum privileges, requiring `IsTokenRestricted`. No process launches. Restricted token creation does NOT by itself prove low integrity/AppContainer, filesystem or network access, or safe worker activation.
- **Linux:** new `ide/sandbox/src/bin/linux_cgroup_probe.rs` derives current cgroup-v2 scope, refuses unsafe paths, and attempts to configure/read back CPU/memory/PID limits on a unique empty delegated child. No process launches. Missing delegation reports **DENIED** even if the workflow itself finishes successfully. This is not evidence of pre-exec process placement or subtree kill.
- **CI:** dedicated Linux and Windows native rights-probe workflow created, exact-head results pending. No Tauri command, no general build/test execution, and no Lu Core bridge.
- **Still open:** Windows low-integrity/restricted child and ACL/network tests, Linux cgroup atomic placement/hostile load, real worker cancellation and crash/recovery, secure native input snapshot, protected OS broker and installed Windows acceptance.


## Wave D3 — Production supervisor control-plane implementation (2026-11-05)

- **Branch:** `ide/wave-d3-supervisor-core-20261105`, stacked on corrected D2 Windows Job Object probe commit `c091fbe174b85a0c0f90c67933fdb2069000cd24`. D2's [Windows native runtime+format workflow](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42158228849) and continuity gate passed on that source SHA. Installed Windows acceptance is still separate and open.
- **Implemented:** Independent `ide/supervisor` native control-plane crate, not connected to Tauri, Lu Core or the IDE's Run button. It models allowlisted build/test jobs with exact Git commit, host input-snapshot/toolchain/manifest digests, effect class and bounded runtime/memory/PID/log quotas. A fresh one-job, one-session, 60-second in-memory grant is bound to the plan hash and current source. Durable receipt state transitions and SHA-256 hash linkage detect corrupted/truncated history (not cryptographically authenticated external receipts).
- **Cancellation and recovery:** native-private broker interface records cancellation request before invoking process-tree termination; only explicitly verified descendant kill can produce a cancelled receipt. Failed kill becomes `uncertain`, blocks further work. Interrupted admission/running/cancel attempts are quarantined on journal reopen; cleanup requires native broker proof and no automatic retry.
- **Execution remains disabled:** `production_worker_admission` always fails closed until OS-specific restricted rights, filesystem/network access policy, CPU/memory/PID quotas, real worker-snapshot publication, crash-orphan resolution and platform hostile tests are complete. No release-build spawn API, worker bridge or frontend Run command added. D1 review entries cannot mint grants. Unit tests use **simulated** OS broker only.
- **Verification status:** dedicated Windows/Linux core test workflow, plus Node static desktop boundary test added; latest-head CI pending. D3 is the supervisor foundation, NOT complete actual worker execution, installed desktop acceptance, or closed Wave D.


## Wave D2-W1 — Windows fixed-fixture Job Object containment probe (2026-11-05)

- **Source branch:** `ide/wave-d2-windows-job-probe-20261105`, stacked on D1 PR #51 SHA `ef623bfd22f8dfc8b1674a44911e98825be8ab53`. This is a standalone OS diagnostic, **not** a production worker, and does not alter the Lu IDE app or Core.
- Reused the existing `ide/sandbox` experiment (Linux Bubblewrap native probes already documented). Added isolated `src/bin/windows_probe.rs` with a fixed self-executing Windows fixture, Win32 Job Object limits and **suspended-create → assign-to-job → resume** ordering. Any OS API admission failure kills/denies the child, no fallback.
- The probe tests a successful contained child, a timed-out fixed sleeper terminated via `TerminateJobObject`, and a fixed grandchild killed with the parent Job. Uses `KILL_ON_JOB_CLOSE`, a 128 MiB job memory quota, active process cap 4, no inherited handles, and minimized environment (SystemRoot/TEMP/TMP). All modes are compiled in; no user/model arbitrary commands or arguments.
- A dedicated `.github/workflows/ide-sandbox-windows.yml` runs the **actual Windows native** probe; docs at `ide/sandbox/WINDOWS_JOB_PROBE.md`. No test result is claimed before its CI runs. Linux existing 5/5 Bubblewrap probes remain historical reference, not a new D2 pass.
- **Still open:** Windows restricted token/file/network confinement, worker exact input snapshot, credential broker, cancellation races, crash-restart orphan reconciliation, Linux process quota/descendant admission, actual build/test worker, physical installed Windows acceptance. Do not wire a Run button until separately proven.


### D1 first-head CI repairs
- Initial D1 frontend workflow had 55 passing tests, one failing contract test: an existing static parser mistook the Tauri-managed `intents` state handle for an external request field. Filtered `intents` as host state; the JSON schema still accepts only `projectId` and `recipeId` for prepare.
- Initial Linux native host suite compiled and tested successfully, then failed formatting only in the new `execution_intents.rs`. Applied **all 34 actual rustfmt hunks** from the exact CI job log and made the fixed recipe array `static`. Exact-head rerun required; not yet proven on revised source.
- Windows desktop compilation and continuity gate on the prior head are not a substitute for latest-head acceptance; installed physical Windows UI remains unverified.


## Wave D1 — Host-authorized execution-intent protocol (2026-11-05)

- **Implementation branch:** `ide/wave-d1-execution-intent-20261105`, stacked on C6 PR #50 exact SHA `c403a8ba47d6ac1662b8627ce5ab34a9a16b591e`. C6 source CI passed Linux, frontend, Windows Tauri, and continuity on that SHA; installed Windows app acceptance remains separately unverified. Do not merge D1 before C6 and its C3/C4/C5 ancestors.
- **D1 scope:** Native `execution_intents.rs` defines a host-owned, append-only, private `execution-intents.jsonl` journal. Three typed operations: `job.prepare`, `job.review`, and `job.list`. The human-facing **Job requests** view records and reviews intentions, not execution. Fixed allowlisted Rust check/test recipes only; no model-supplied commands, file paths, arguments, environment variables or executable launch path.
- **Preconditions:** a registered active Git project, clean observed HEAD and no tracked/untracked modifications, known fixed `Cargo.toml` or `ide/host/Cargo.toml` manifest with bounded SHA-256 digest and symlink/reparse checks. Preparation expires in 120 seconds, is scoped to one IDE session, and records opaque native IDs. Recording review re-reads source SHA and manifest digest; changing project/source/session or expiration blocks approval. A previous review can be withdrawn; no reusable grant is minted.
- **Effects and recovery:** Journal appends are mutex-serialized, bounded, synced, and checked for valid monotonic transitions. Corrupt/truncated journals fail closed; on restart, unreviewed preparations require fresh review. There is **no process spawn**, no worker, no OS containment assertion, no execution token, no Git mutation and no Lu Core bridge. Approval notes are not execution authority. Required Windows Job Object/Linux cgroup containment, worker budgets, kill/recovery tests, and signed effect receipts remain **Wave D2+ gates**, not complete.
- **Verification:** Native Rust journal tests and frontend type/schema/UI checks added. Exact-head D1 Linux, Windows, frontend and continuity acceptance are PENDING; physical installed Windows UI checks remain unverified. The project root/Delivered folder on the user's physical machine is not inspected.


## Wave C6 — Task evidence and development ledger integration (2026-11-05)

- **Source branch:** `ide/wave-c6-task-ledger-20261105`, stacked on exact C5 source `1dd4a58d84d6bc5f383f75e463b15f82baea6de8` (C5 PR #49). Earlier C3/C4 are passing native/frontend/continuity GitHub CI on their own heads, but no physical installed Windows acceptance. The standalone IDE and Lu Core remain independent.
- **One private ledger:** C6 extends existing `development-ledger.jsonl` events with an optional, strictly typed task payload and host-owned task materialization. Legacy events remain readable; a task change appends a new event and never rewrites old history. Snapshot of local Git HEAD and dirty state is captured by the host at task update time.
- **Human task claims:** native `task.update`/`ide_record_task_update` supports bounded task IDs/titles/next actions, status `planned | in_progress | blocked | ready_for_review`, and user-selected gates `source_review | exact_head_ci | installed_windows | human_approval`. No `completed` input is allowed. Maximum 64 tasks per registered project.
- **CI-linked evidence:** `task.ciObserve`/`ide_observe_task_ci` independently invokes the trusted exact-HEAD public GitHub inspector, binds its bounded summary to the *latest* human task event ID and current source commit, and appends a separate observation. The frontend never supplies the CI verdict, workflow IDs, credentials, Git SHA or any acceptance claim. An intervening task edit or changed HEAD invalidates the old CI link.
- **Evidence gates:** host projects per-task gate statuses. Successful commit CI can produce `commit_ci_passed_not_task_accepted`; it never proves unsaved buffers, installed Windows UI, a human source review, or final task acceptance. The Continuity UI supports task updates and explicit CI observation, while the portable checkpoint exports only task IDs, statuses, commit/evidence coordinates and explicit unknowns (no task title or private next-action text).
- **Security:** app-profile-only append under one mutex, no project file mutation, no arbitrary process execution, no Lu runtime bridge. Native test fixtures and Node frontend contract/privacy tests added. **Verification:** exact-head C6 Linux, Windows, frontend and continuity CI not yet accepted; physical installed Windows UI and actual live public CI lookup in an installed app remain open.


## Wave C5 — Exact-head CI receipts (2026-11-05)

- **Implementation branch:** `ide/wave-c5-exact-ci-evidence-20261105`, stacked on C4 commit `ca606dfb378ce31945a768a23e9ab6a31cbe60e8`. C3 PR #47 and C4 PR #48 remain independent/open pending acceptance; do not merge C5 before dependencies.
- **Source implementation:** `host/src/github_ci.rs` read-only public GitHub Actions inspector and typed `git.ciExactHead` host capability/`ide_read_exact_head_ci` Tauri command. An explicit user click queries only `https://api.github.com` for the active registered Git project's validated origin and exact local HEAD SHA. No cookies, tokens, shell, Git fetch/push, source writes or background polling.
- Workflow receipts check the *specific* `ide-standalone.yml` (required: host-linux, frontend, desktop-windows) and `ide-continuity.yml` (required: continuity) workflow paths, matching run head SHA, matching job head SHA when present, completion and success. Missing/duplicate jobs, partial responses, failure/pending runs or local HEAD changes never count as passing. Unknown origin, private repo/rate limits, absence of runs and incomplete evidence remain explicit.
- Human Source control view has **Check exact HEAD CI**. C4 evidence matrix and C3 Copy development checkpoint accept only cached, explicit, current SHA-bound CI receipt metadata; copying has **no** additional network request. Dirty worktree/unsaved drafts, installed Windows application, toolchain usability and individual task completion **remain unverified** by CI.
- **Implementation verification status:** Rust pure fixture tests and Node regression tests added; exact-head Linux/frontend/Windows CI and installed Windows UI acceptance **PENDING**, not passed at authoring. No new installer claimed; local machine's Deliverables/resources uninspected.

### C4 frontend CI regression repair (2026-11-05)
- Earlier C3 exact-head run 42145804942: Linux native and Windows desktop succeeded, frontend failed a **test-only** capability-registry regex expecting same-line Rust macro arguments. Updated the contract assertion to support multiline rustfmt and require 24 nonempty commands. Direct catalog/Tauri parity: 24=24, no missing or duplicate commands.
- Wave C3 source was separately repaired at `efe731d874e11309f4646d150827a723ed9932f7`. This C4 stacked branch carries the same test repair independently. Latest C4 exact-head CI, installed Windows UI acceptance and toolchain execution remain open.

## Wave C4 — Development evidence reconciliation (2026-11-05)

- **Implemented in source on independent branch** `ide/wave-c4-evidence-reconciliation-20261105`, based on exact Wave C3 SHA `2572fa445d31d265b0ef27ef52a1dd4a09156326`. No Lu bridge or executable worker.
- A pure `development_reconciliation.js` derives ten human-readable evidence classifications from the existing native project context + inventory comparison. The Continuity screen labels local Git, stored upstream, live remote (not queried), bounded artifacts, historical baseline, project task claims (unverified), host events, native CI, installed app, and toolchain usability separately.
- A clean observed Git tree or matching SHA never implies source build success or installer acceptance. Unknown/mixed project IDs fail closed; incomplete scans and truncated paths remain explicit. Project-authored document text is never used as authoritative task completion.
- The human Copy development checkpoint export embeds the conservative matrix; no new Tauri capabilities/permissions or project writes. JS tests cover contradictory Git state, changed/missing bytes, incomplete scan, absence of CI evidence, privacy and UI wiring.
- **Verification:** source committed but full latest-head native Linux/frontend/Windows CI and installed Windows GUI acceptance pending. No package or CI receipt claimed for C4 at authoring. Worktree/Delivered on the user's Windows device remains uninspected.


## Wave C3 — Portable development checkpoint (2026-11-05)

- **Implemented in source on draft PR #47:** Human **Copy development checkpoint** in Continuity performs fresh parallel native context and inventory comparison reads for the exact currently selected registered project and copies bounded versioned JSON. Project IDs must match both host responses; switching projects invalidates pending requests and any stale continuity UI. Clipboard failure is surfaced, not treated as success.
- Captures local Git HEAD/branch/dirty counts and stored tracking evidence, bounded artifact SHA/size/status, baseline changes, recent event IDs/saved revisions, and orientation document revision IDs. Omits source and note excerpts, event summaries, private absolute roots, credentials, executable work and live GitHub content. Limits and unknown states are explicit. **Two reads are not an atomic transaction**; copy time is not evidence time.
- New pure helper and Node tests: `app/web/development_checkpoint.js`, `app/tests/development_checkpoint.test.mjs`. Local isolated Node helper tests passed (3) before repo integration; full frontend/Windows CI and installed Windows copy UI still pending at PR head.
- Latest verified predecessor commit before this checkpoint: `26040e8331f5a5b8053f4af8248a88d9d551f04f`; new source SHA must be taken from GitHub after publication, not inferred. Local user's checkout/Delivered still not inspected. Lu bridge remains deferred.

**Observed date:** 2026-11-05 (America/Phoenix)  
**Repo:** `vjmichael/Lu-AI-GUI` · **Working branch:** `ide/wave-c4-evidence-reconciliation-20261105` (stacked on Wave C3 PR #47; unmerged). Wave C3 remains on `ide/wave-c-development-truth-20261105`.  
**Last inspected implementation source SHA:** Prior C3 source commit `d6bdca75953ab63579be5d071ec706effe4c6771`; inspect latest [draft PR #47](https://github.com/vjmichael/Lu-AI-GUI/pull/47) HEAD for the current source and exact-head validation.
**Current source validation:** Wave C3 source and tests committed; exact-head Linux/frontend/Windows checks and installed Windows clipboard acceptance **pending**. First C3 matrix [42145655561](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42145655561) queued after prior source. A subsequent race-state cleanup supersedes it; inspect the final PR HEAD, not prior runs.
**Local checkout, `Delivered` folder and untracked work:** **UNKNOWN / NOT INSPECTED** in this session. Never assume GitHub is ahead of inaccessible local work.  
**Live Lu↔IDE connection:** **DEFERRED by user**. The IDE remains standalone.

## Current status by capability

| Milestone | Source | Verification | User acceptance |
| --- | --- | --- | --- |
| R1–R3 project registration, explorer, CodeMirror, explicit save | Implemented | Earlier CI and Windows user smoke; see [handoff](ENGINEERING_HANDOFF.md) | Earlier core editor milestones accepted |
| R3.4 multi-hunk review and revision conflicts | Implemented | Earlier native CI accepted; see [R3.4](docs/IDE_07_R34_MULTIHUNK_PROPOSALS.md) | Earlier Windows smoke accepted |
| R3.5 proposal inbox and editor-only staging | Implemented | Earlier native matrix passed; see [R3.5](docs/IDE_07_R35_PROPOSAL_INBOX.md) | User reported working staging flow; full hostile gates open |
| R3.6 success banner, R3.7 Find in File | Implemented | [Source CI 42069988384](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42069988384) passed for R3.7 SHA `eb0aefd` | New GUI acceptance still open |
| A1 saved-file source spans, Copy Source Reference | Implemented | [CI 42075298457](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42075298457) Linux/frontend/Windows passed for SHA `467775e` | New clipboard UX still open |
| **A2 typed Semantic Capability Registry** | **Implemented**: 21 real host operations, JSON Schema v2, self-contained nested definitions, native-argument parity tests, Capabilities UI | **Exact-source CI pending** on latest code | Windows installed UX open |
| **A3 Development Ledger and Context Read Model** | **Initial runtime implemented**: host app-profile JSONL event history, bounded project context and Continuity UI | **Exact-source CI pending** on latest code | Windows installed UX open |
| A3 bounded local artifact and toolchain-candidate inventory | **Implemented in source** | CI on latest source pending | Installed acceptance open |
| A3 persistent snapshot and compare | **Implemented in source** | Native regression fixtures added; CI pending | Installed acceptance open |
| A3 native local Git dirty/untracked inspection | **Implemented in source** | Exact-head CI pending | Windows UI open |
| Wave C1 local staged/unstaged Git diff/history | **Implemented on isolated branch** | CI pending | Installed UI open |
| Wave C2 targeted resource SHA probe | **Implemented on isolated branch** | CI pending | Installed UI open |
| A3 full live GitHub and external toolchain reconciliation | **Not implemented** | Not tested | Open |
| Live Lu connector, worker execution, LSP/build/test/package | **Deferred/not implemented as IDE runtime** | Not tested | Open |

## A2 — Typed contracts actually implemented

- **Source:** `host/src/capabilities.rs`, `host/src/capability_schemas.json`, standalone Tauri `ide_list_capabilities`, React Capabilities screen.
- Host-created, versioned descriptors for **21 existing** project, source, review, save, proposal, continuity, inventory and Git/GitHub operations. Request/result JSON schemas are typed, with nested structures and self-contained JSON Schema 2020-12 definitions. Response includes typed error schema. **Discovery is not permission, execution or a Lu bridge.**
- UI shows concise operation input/result signatures, with full schema expansion on demand. Rust validation and frontend structural/command parity tests added.

## Wave C — Development truth (independent source implementation)

- [Draft PR #47](https://github.com/vjmichael/Lu-AI-GUI/pull/47), source repair SHA `dbaa04ff`. First-run [42143131954](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42143131954) frontend passed, Linux failed E0308 in resource timestamp conversion. Converted cap-std time to std time before Unix offset; new exact-source [42143391542](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42143391542) pending. Native Rust/Windows launch and user acceptance remain open.


- Isolated feature branch `ide/wave-c-development-truth-20261105` while Wave B PRs remain in their own GitHub acceptance queues. **Do not mark the completed Wave B source work as Windows-accepted yet.**
- Implemented `host/src/git_evidence.rs` to inspect staged/unstaged local Git patch text and 30 local commits, bounded to 64 KiB of patch and 80 changed files. It refuses parent repository inspection, excludes untracked *contents* from patch, performs no Git command or mutation, and labels remote state as `not_queried`.
- Added typed `git.diff` and `git.history` and an explicit human **Inspect diff and history** action in Source control.
- Implemented `host/src/resource_probe.rs` for exact relative-path SHA-256 and metadata under the registered project. Missing/oversized/not-regular/changed files are not mislabeled as verified. **Executable readiness remains not tested**; no shell, package install or Lua/Lu bridge.
- Added typed `resource.probe` and a **Recheck file bytes** button in Continuity. The registry now describes **24 native operations** with typed schema parity verified structurally (24 descriptors / 24 schemas / 25 registered handlers including discovery).
- Native fixture tests cover Git patch separation, bounded history, root isolation, symlink and traversal, resource SHA and missing/over-limit cases. Frontend contract tests and isolated PR CI trigger added. Full semantics: [Wave C development truth](docs/IDE_WAVE_C_DEVELOPMENT_TRUTH.md).
- **Acceptance status:** source implemented; exact-head Linux/frontend/Windows CI and installed Windows UI remain open. No global executable toolchain inspection, authenticated private remote comparison, or real worker execution claimed.

## Wave B1 — Human Find and Replace (source implemented, exact-head CI open)

- `app/web/find_replace.js` is a pure bounded literal match-and-replace planner for the **active unsaved CodeMirror draft only**. Matches preserve UTF-16 offsets, escape regex metacharacters and optionally match case. All-or-nothing replacement refuses scans above 10,000 matches; no partial bulk edit or direct disk write.
- `app/web/main.jsx` adds Ctrl+H, human Find/Replace controls, Replace One and confirmation-gated Replace All. All edits enter the existing draft/dirty state and still require explicit Save and revision-checked host writes.
- `app/web/code_editor.jsx` dispatches a planned set of changes as **one undoable CodeMirror transaction**, checks the expected draft before applying and acknowledges/consumes the nonce to prevent replay when tabs switch. No Lu model or file/host tool added.
- New `app/tests/find_replace.test.mjs` includes UTF-16 Unicode offsets, literal metacharacter search, case-sensitivity, literal replacement of dollar strings, match selection, over-limit refusal and empty query. Updated existing frontend regression and editor styles.
- **Verification:** implementation source committed; CI pending. The previous native Git/GitHub CI run for `b39c6267` is no longer an exact-head check for this new source. CI concurrency may cancel superseded branch runs. No new Windows installer or physical acceptance claimed.

## A3 live GitHub evidence and native Git CI repair

- `host/src/github_live.rs` provides an explicit read-only check against public GitHub origin HEAD, exposed through typed `git.compareLive`, Tauri `ide_compare_live_github`, and a human **Check GitHub** button. The catalog contains **21 actual native operations**.
- Origin is syntactically restricted to `github.com`, with fixed `https://api.github.com` REST, no redirects, credentials, shell commands or Git mutation. TLS query has short timeouts and a 48 KiB response bound.
- Separately reports time-stamped GitHub branch SHA, local HEAD, stored tracking SHA, local dirty/untracked status and **SHA equality or difference without assuming ancestry**. Private/rate-limited/unavailable GitHub remains unverified; no automatic request or Lu connection.
- CI [42128252074](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42128252074) exposed E0505 dropping a Git repository before a borrowed Git tree during test teardown. Fixed by releasing the tree first, in source `b39c6267`.
- CI [42128928605](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42128928605): **Windows host and Tauri checks passed**; Linux/frontend queued at last inspection. No physical installed Windows UI acceptance or local user filesystem inspection.

## A3 native Git worktree reader — implementation committed, CI pending

- Native Git dirty-status regression now alters file byte length to avoid a same-size timestamp/stat-cache race. Latest source commit `27e6dde8cf6bd12e6ea2fbe19b27c2fb8f74c89f`; [CI run 42128252074](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42128252074) queued; no recent compile or physical Windows acceptance result for this SHA.


- Activated the **Source control** sidebar page (previously 'Soon'), backed by new read-only `ide_read_git_status` Tauri command and typed `git.status` capability. UI shows exact local branch, modified/staged/untracked counts, local commit and upstream difference, limited change list and explicit no-live-GitHub disclaimer.
- New native operation increases capability registry to **20** descriptors, matching 20 implemented non-discovery Tauri commands. Structural audit found no argument/schema mismatch. Source revision `4d1a559d2f37afb062524827a6d23aff6e62f9b5`; full Linux/frontend/Windows acceptance remains open.


- Latest implementation SHA `a37be15b808b2d238ac2457aee9e357d056be13f`, source-specific verification [run 42127486856](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42127486856) queued. Tests include native Git fixture + baseline transition; Linux build and Windows installed acceptance remain open.

- `host/src/git_status.rs` uses native `git2` / libgit2 to inspect the **exact registered project root** without launching `git` or executing project code. Refuses accidental parent-repository discovery.
- Returns branch, local commit ID, locally configured upstream tracking commit, locally computed ahead/behind counts, true modified/staged/untracked file counts and at most 120 changed-path summaries. The Continuity screen displays these separately from older loose-ref evidence.
- **Not a live GitHub comparison**: no fetch, push, network access, remote PR inspection or authority to mutate Git. Worktree inspection can be unverifiable, and none of its observations grant permission to write.
- Added native Git fixture tests for clean -> dirty/untracked and parent-repo refusal, plus frontend/schema checks. A recorded inventory baseline now includes the observed clean/dirty worktree state, and comparison reports a transition such as `became_dirty` rather than assuming the same commit means no local changes. `git2` now a standalone host dependency; native Windows/Linux CI must compile and run before marking this accepted.
- CI: [run 42127100529](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42127100529) pending at last check; previous build `42125275258` had Windows success, but its Linux/frontend jobs were cancelled as source evolved.

## A3 — Bounded local evidence inventory and saved baselines

- **Implemented in source:** `host/src/inventory.rs` performs a bounded, nonexecuting scan of known folders such as `Delivered`, direct root-level installers, build output, benchmark/result folders, and selected project-local tool/bin directories. Files receive observed byte size, SHA-256 if at most 64 MiB per file and within a 128 MiB total scan budget, category, and explicit `present_unverified` status. No shell, package install, app launch, network request, or ambient filesystem search.
- **Local Git:** reads only `.git/HEAD` and loose local/remote-tracking refs through the project capability. It can report equal/different *locally stored* refs, but does **not** verify a working-tree diff, packed refs, linked Git worktrees or current GitHub HEAD. Those are marked `not_verified`/`not_queried`.
- **Durable user-triggered baseline:** `host/src/inventory_history.rs` preserves a bounded append-only history (up to 64 snapshots total, 1 MiB storage limit) of observed artifact paths, sizes and SHA-256 digests in application-profile `evidence-baselines.json`. Later scans compare with the latest snapshot and classify known files as `matched_bytes`, `changed_bytes`, `missing_in_scanned_scope`, `new_in_scanned_scope` or `unverifiable`. An incomplete scan never establishes missing evidence. Histories persist across restart, but the snapshots prove byte observations, **not executable readiness or benchmark comparability**.
- **UI:** Continuity now shows evidence files and their digests, observed Git refs, a human **Record evidence snapshot** control and baseline comparisons. The capability catalog includes two typed host operations, `inventory.capture` and `inventory.compare`. Native fixture tests and frontend parity tests added.
- **Scope limits:** only registered project roots are scanned (including direct root-level installer candidates so a manually selected `Delivered` folder can be inspected). Toolchains installed elsewhere on the user's machine, older `Delivered` folders outside the chosen root, live remote GitHub, actual dirty/untracked diffs, native installer launch and benchmark reruns remain **not inspected** without separate explicit access and worker authorization.
- **Verification:** source committed, pending native/Windows CI [run 42125275258](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42125275258). No physical Windows acceptance and no new installer claimed.

## A3 — Current, narrowly scoped first slice

- **Source:** `host/src/continuity.rs`, `host/tests/continuity.rs`, standalone Tauri `ide_read_project_context`, React Continuity screen.
- Private app-profile `development-ledger.jsonl` receives **best-effort host observations after successful actions**: project register/open/close/unregister and explicit revision-checked source save. Saved events include **validated relative source path** and saved SHA-256 revision. No project root or absolute local path stored in event.
- The read model requires an **active registered project**, returns at most **40 recent events** and twelve bounded root/`ide/`/benchmark orientation document slots with exact saved revisions and excerpts. Project-authored documents are labeled **untrusted_project_text**.
- Remote Git state: `not_queried`. Local artifacts: `bounded_scan`. Toolchain readiness: `not_verified` or `present_unverified` for scoped candidates. These describe exactly what was and was not inspected, not false missing/ready claims.
- **Limitations:** Observations are NOT crash-safe effect receipts, verified CI results, operational resource probes, Git reconciliation or authoritative build history. Failed ledger appends are logged separately and do not convert a successful file write into a retryable failure. A corrupt ledger fails closed on read. True effect journaling, verified external resources and complete local/remote reconciliation remain open. The bounded artifact inventory and persisted byte snapshots are implemented as separate services.

## Next unblocked tasks (order matters)

1. **Close exact-source CI:** Linux Rust unit tests, `cargo fmt --check`, frontend build/tests and Windows Tauri/native checks for A2/A3; fix actual failures before claiming acceptance. Then test Capabilities and Continuity screens on a Windows build. No new installer is verified.
2. **A3 follow-up:** verify exact-source native CI, fix formatting/build errors, then expand beyond bounded project roots only through explicitly registered locations. Real executable version/OS checks require supervised workers and permission; benchmark comparability and signed build/test receipts remain open.
3. **Project reconciliation:** inspect *real* local worktrees (including dirty/untracked state), accessible out-of-project `Delivered` folders, live remote HEAD/PRs, benchmarks and package hashes. Classify `LOCAL_ONLY`, `REMOTE_ONLY`, `MATCHED`, `DIVERGED` or `UNVERIFIED`; no destructive merges without human consent.
4. **Standalone editor hardening:** Find/Replace and unsaved-buffer revision identities; retain approved proposal staging, explicit Save and host revision checks.
5. **Execution security before LSP/build/test/package workers:** OS containment and user authorization gates remain mandatory; future Lu integration remains deferred.

## Handoff and artifact truth

- [Chronological changelog](CHANGELOG.md), [continuity protocol](CONTINUITY_PROTOCOL.md), [engineering handoff](ENGINEERING_HANDOFF.md), [capability-first architecture](docs/CAPABILITY_FIRST_ARCHITECTURE.md).
- Repository branch source was inspected using the GitHub connector. No local checkout or user Windows filesystem was inspected. **No R3.6–A3 Windows installer or local NSIS package has been verified in this session.**
- Do not infer that a Git commit is a tested build, that a CI success is an installed launch, or that historical resource memory authorizes execution.


## IDE native release and external review — November 7/8, 2026

The active draft PR #97 continues P02–P06. Windows WO9 real OS popup caption is UIA Text/Pane rather than Button; added exact app-owned separate HWND physical bounded click and regression, pending installed CI. P03 Windows AppContainer LLDB-DAP still fails permission-denied before handshake, Linux fixed LLDB-DAP and installed GTK remain native pending. Signing tool now requires a private-key certificate with valid Code Signing EKU, dates and chain; no real publisher cert/secret is available, release still denied. A technical P06 review packet records prior unsigned Windows artifact run 42430581152, its installer/inventory digests, 521 resolved notices, 0 unresolved notice sources, 6 upstream originals and outstanding independent legal/target/binary/GPG review. P05 retains all 38 workflows and original historical PRs with nine unported B2/B3 source blobs. Signed two-version installed rollback and P07/WO10 integration are not accepted.

## Native CI and release scope — Nov 7/8 continuation
Owner defers Windows publisher certificate, Linux signing key and external legal sign-off; no release waiver. Native WO9 Windows both consent dialogs and exact Git stage/commit passed before CI fixture process-baseline variable collision; corrected. Added independent Ubuntu 26.04 unsigned GTK editor/rollback/ZIP diagnostic without replacing still-queued Ubuntu 24.04 required gates. P05 original PRs, nine unported blobs and 38 original workflow gates remain intact; P03 protected LLDB-DAP OS tests still open.

### Native test-failure diagnostics and bounded GUI recovery
Nov 7/8 exact-head Ubuntu26 installed app: real rollback, notice bytes and GUI window PASS; GTK folder picker remained open after three legitimate AT-SPI actions, so editor remained open. New exact candidate ranking, bottom/right dialog control selection and native Alt+Up exact folder selection fallback await repeat. Win WO9: native installed app's renderer existed but UIA subtree was empty at startup; added exact foreground OS Tab-based accessibility warmup with hard failure if actual UIA missing. P03 protected debugger remains open; signing/independent legal review deferred.

## Exact native UI diagnostic follow-up
Windows WO9 real consent x2 and project review PASS, but Git HEAD did not change at instantaneous readback; native test now polls actual commit SHA for 24s (no synthetic success). Ubuntu26 real Debian rollback PASS, but only one generic GTK Open control appeared and project chooser never satisfied OS registration; active visible chooser and exact non-dialog GUI project required, otherwise denied. Source tests added; signing/independent legal review remain explicitly deferred and release false.

### Linux GTK chooser active-window fallback
Latest native Ubuntu 26.04 `.deb` build/failed-upgrade recovery passed, but GTK chooser showed no native action buttons; implemented bounded OS X11 chooser-identified keyboard fallback gated on actual AT-SPI visible chooser and real app registration. Native OS retest is pending. Signing/legal publisher identity remain deferred, release remains default deny.

### P04 GTK CI native spatial witness
The previous Ubuntu26 exact installed test [42436702598](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42436702598) proved dpkg rollback but found neither an X11 active window nor an AT-SPI action button for the **visible** native GTK chooser. A new bounded physical X11 mouse attempt is strictly derived from the chooser's accessible rectangle and independently checked against display bounds. The test still must observe a changed, registered project in the real installed app and remains FAIL on no registration. It is diagnostic native GUI repair, not a process/security containment downgrade or a signed-release claim. Ubuntu24 actual native acceptance remains required.

## DEV-02.2 native protected Rust/C++/Python integration — November 8, 2026

Draft [PR #131](https://github.com/vjmichael/Lu-AI-GUI/pull/131) branches from DEV-02.1 #129. Windows opt-in `native-lsp` retains independent signed LocalService, AppContainer and Job Object authority; Linux opt-in `native-lsp` now connects the already-existing root-owned Bubblewrap + delegated kernel cgroup-v2 `LinuxLspSession`. Default installed shell remains `default=[]` and cannot auto-launch language servers. Native project identity, exact source file SHA, unsaved buffer revision, human OS approval and stage-sealed one-shot grant are retained; no trust in server URIs or PATH. Rust/rust-analyzer and C/C++/clangd source interfaces expose diagnostics, completion, hover, same-file definition; Python/Ruff exposes diagnostics only; Pyright-compatible semantics deferred pending reviewed provider and complete runtime/licensing.

Exact-head native Win2022/Ubuntu24 verification is **OPEN**. Run [42454644809](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42454644809) failed a real JSX closing-brace error, repaired at `affb128`; next run [42489917103](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42489917103) passed frontend build but revealed two stale Windows-only source-contract guards, repaired at `e77acaa`; next run [42490040190](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42490040190) passed all 86 frontend tests but Windows host source job used `--locked` without a host Cargo.lock, repaired at `4d1e406`. The subsequent workflow YAML failed to parse due to literal backslash-n in added comments; repaired at `a5b1bcc`. **These repairs are not passing final-head native evidence**. Never report a Tauri or actual protected worker pass until the current exact-head OS Rust tests do run and finish successfully.

Linux/offline bundle staging and Windows signed-service stager now compute full-tree reviewed binary+original notice hashes, but `ide/packaging/language-servers.manifest.json` still has `releaseApproved=false`, no approved hashes or redistribution/copyright signoff. Two manual installed workflows require dedicated protected OS runner enrollment, root-owned assets and kernel cgroup delegation on Linux, or independently reviewed Authenticode-signed LocalService on Windows, plus actual installed NSIS/Debian GUI user consent, diagnostics/completion/hover/definition, worker cleanup and uninstall. No self-hosted runner/approved assets/signed service are assumed to exist. This is not an approved release. PR #131 detail: `ide/docs/IDE_DEV02_PROTECTED_MULTILANG_20261108.md`. DEV-01 installed, B2/B3, P03/P04 and Core P07 remain separate.

## DEV-02.3 LSP review intake and Python semantic decision (November 8, 2026)

Draft PR #132 atop DEV-02.2 PR #131 records an offline **NO APPROVAL** intake path for operator-supplied actual clangd/rust-analyzer/Ruff Windows/Linux provider files, original license notices and exact candidate versions. `ide/packaging/prepare_lsp_candidate_review.py` emits SHA256 and sizes with `releaseApproved:false`, `copyrightReviewed:false`, `dependenciesAudited:false`, no implicit runtime permission. It refuses traversal, symlink, unexpected providers, missing platforms/notices and bad binaries. Dedicated Windows2022/Ubuntu24 source tests are required. This **does not** provide assets, license approval, signer identity, real protected self-hosted runner or installed app acceptance. Python semantic alternatives are Microsoft Pyright or BasedPyright with an independently audited Node runtime/dependency graph, and Astral ty native standalone beta with its own comparative capability/security benchmarks; neither is selected/approved. Existing Ruff remains diagnostics-only. See `ide/docs/IDE_DEV02_3_OFFLINE_REVIEW_AND_PYTHON_SEMANTIC_20261108.md`. Upstream release approval and technical protected installed OS evidence remain OPEN.

### DEV-02.3 original release notices, technical approval boundary and Windows SCM correction

Official release-tag notice pointers are now fixed in `ide/packaging/lsp_original_upstream_notices_20261108.json`: LLVM tag `llvmorg-23.1.0` `llvm/LICENSE.TXT` original blob `fa6ac540...`, rust-analyzer tag `2026-11-02` original MIT/Apache blobs `31aa7938...`/`1b5ec8b7...`, Ruff tag `0.17.4` original license blob `655a0c76...`. `collect_original_lsp_notices.py` verifies Git blob bytes and SHA256, rejects unapproved/missing/linked source, and retains exact originals and a **redistribution false** receipt in a dedicated native CI artifact. This is provenance preparation, not completed copyright/transitive-dependency review.

A critical installed workflow risk was removed: the Windows `ide-dev02-windows-installed-protected-lsp.yml` manual gate no longer uses a bare `sc.exe create/start`, which omitted service SIDs and private ACL setup. It requires the installed service **and desktop host** to have valid Authenticode, puts the host under Program Files, invokes the already reviewed `Install-ProtectedRecoveryService.ps1` elevated service installer, verifies Running and explicitly checks service teardown. Real installed NSIS acceptance and legal identity signing remain unproven; currently blocked pending approved signed binaries, approved three-provider release manifest and protected runner.

### DEV-02.4 — independent Python semantic candidate real-protocol comparison

Draft test-only candidate work uses the same 20 deterministic Python project files against BasedPyright v1.40.2 and native ty v0.0.99 on genuine Windows 2022 and Ubuntu24 GitHub runners. `python_semantic_native_benchmark.py` speaks bounded JSON-RPC `initialize`/`didOpen`/`hover`/`definition`/`completion` and shutdown, records independent response counts and latency, checks same-file URI identity, and verifies process exit/reaping; it is NOT an installed Lu IDE worker or product permission. `stage_ty_semantic_candidate_ci.py` checks actual official ty zip/tar compressed SHA; BasedPyright npm exact version is staged with install lifecycle scripts disabled in runner temp, Node 22 comes from CI. Test-only artifact JSON does not include binary distribution. Ruff retains Python diagnostics-only native policy. Real exact-head OS workflow needs to finish; only after metrics and reviewed Node/native runtime, legal/dependency graph, protected worker and installed-app approval may a semantic provider be chosen and enabled. See `ide/docs/IDE_DEV02_4_PYTHON_SEMANTIC_NATIVE_BENCHMARK_20261108.md`.


### DEV-02.4 first-head native OS Python provider failures and remediation (Nov 8)

[Real Windows/Linux run 42494557323](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42494557323) failed, notwithstanding actual semantic responses: Ubuntu BasedPyright and ty 20/20 hover, definition and completion each; Windows BasedPyright 20 hover/20 completion/0 validated definition; Windows ty 20/20 all semantic methods. Both ty results had ambiguous `terminalReaped=false` from the first benchmark's conflation of nonzero exit code and actual process reaping. The revised `ide/packaging/python_semantic_native_benchmark.py` now uses strict platform-native local `file:` URI identity with cross-file denial for definitions, distinguishes OS reaped vs clean shutdown from actual handshake, forced tree kill and exact exit code, and preserves failures with nonidentifying mismatch classifications. Real exact-head repeat on Windows/Linux is **OPEN**; no provider selected, released or deployed into Lu. User-deferred commercial release legal/signing and physical installed GUI LSP acceptance remain separate.


### DEV-02.4 ty actual upstream server shutdown evidence

Review of official `astral-sh/ty/scripts/build_ty_pgo.py` confirms a necessary terminal sequence: `shutdown` acknowledged, `exit` notified, then `stdin.close()` and process `wait`. Lu's first candidate comparator omitted EOF. At PR #134 current source this was repaired without relaxing `cleanExit`: zero exit after acknowledged LSP shutdown, explicit exit notification, no forced OS kill and actual reaping are independently required. CI source unit regression models shutdown→exit→EOF→wait and distinguishes nonzero exit (reaped, NOT clean). Exact-head OS results needed to determine whether ty’s first failure was a missed EOF or a genuine provider defect; do not claim an installed Lu LSP pass.


## B7 project workspace navigation continuity — Nov 8

Source branch `ide/b7-project-workspace-continuity-20261108` is stacked after B6 exact source `a562ed08ee511fa28faf9aff1c25b89a95d86eaf`. Project switch captures/restores the active source tab, secondary pane and tab-window position strictly from current registered tabs; it cannot revive a closed file or copy private drafts. Unregistering an inactive project does not tear down the active split. No persistence of raw editor data or expansion of native authority. Documentation: `ide/docs/IDE_B7_PROJECT_WORKSPACE_NAVIGATION_20261108.md`. Full Node / installed Windows/Linux tests remain open; older B4/B5/B6 acceptance gates are preserved for later review.


## B8 safe clean background working-set reclamation — Nov 8

Source branch `ide/b8-safe-clean-tab-reclamation-20261108` stacks on B7 (`dbe9bd026a2949e678892815fce36d6d8a07b7b9`). User-visible `Close clean tabs` keeps selected/secondary panes, any dirty editor, any previously edited buffer with Undo history, in-flight private draft checkpoint/Save and LSP session tabs. A second live CodeMirror text check prevents stale React state from losing newly typed text. No project file Save, native read/write, network access, background worker or register/unregister operation. Source tests in `ide/app/tests/editor_clean_tab_reclamation.test.mjs`, documentation in `ide/docs/IDE_B8_EXPLICIT_CLEAN_TAB_RECLAMATION_20261108.md`. Full npm/native Windows/Linux and installed app performance evidence remains OPEN; user requested deferring acceptance review while B-series development continues.

 
## B9 private restart-safe workspace navigation — November 8, 2026

B9 source branch `ide/b9-durable-workspace-navigation-20261108`, stacked on B8 exact source `bf1f7fc238281dd125b25cb6f3a04509d6b490b6`. Introduced a private app-data JSON layout record limited to 48 project-relative files and SHA revisions; no disk root, code text, draft, Undo or worker authority is persisted. Native `ide_workspace_layout_read/save/clear` use the existing registered project browser and bounded atomic storage; native project unregister purges that layout. Opt-in editor Remember/Restore/Forget operations require explicit action; revalidation opens only saved current source, does not overwrite existing dirty editors and never auto-recovers B2 private draft checkpoints. Rust unit and real native filesystem tests plus ten frontend regressions committed; the exact-head real Windows/Linux native and installed-app gates remain **OPEN**. Design/security and handoff: `ide/docs/IDE_B9_DURABLE_WORKSPACE_NAVIGATION_20261108.md`. Existing B4/B5/B6 acceptance gates are not changed or accepted by B9.

 
## B10 external source drift and non-destructive comparison — November 8, 2026

Branch `ide/b10-external-source-reconciliation-20261108` stacked on B9 exact `6e0a05a2b6085acbcc907f08715ed7f5ee1935d4`. Users can manually check disk revisions for up to 48 registered open tabs and compare saved base, live unsaved CodeMirror draft and current native disk text in a bounded, read-only first-difference excerpt; mismatch and missing files remain visible while no tab is automatically overwritten. Native preview path and cap-std authority are unchanged. UI snapshots are invalidated by editing, project changes, changed tab generations and live EditorState identity; no disk text is retained in audit reports. `ide/app/tests/editor_disk_drift.test.mjs` includes ten new source regressions, and B10 source logic plus test bodies passed V8 execution with Node assertion emulation (not Node/native CI). Design/handoff: `ide/docs/IDE_B10_EXTERNAL_SOURCE_DRIFT_20261108.md`. Full Node, native Windows/Linux and physical installed UI gates, along with B4/B5 memory metrics and B6/B9 installed acceptance, remain OPEN as requested.

 
## B11 reviewed external-source conflict resolution — November 8, 2026

B11 branch `ide/b11-reviewed-conflict-merge-20261108`, stacked on B10 exact commit `ab20deade3974f14770fa94f1f6c3687f36dcc13`. Implements human-controlled 3-way base/draft/current-disk line merge for an open primary CodeMirror source tab. Disjoint changes combine; every overlapping conflict requires explicit draft/disk/base choice before applying. Native `ide_preview_source_file` re-reads latest disk on user approval; project, file, saved revision, current disk SHA/text, buffer revision/generation, and live EditorState must match. CodeMirror receives only one Undo-able editor transaction, never a project source Save, worker or new permission; saved source base rebases to freshly verified disk SHA after successful callback. Mixed newline formats fail closed before transaction. Original B2 private checkpoints are not deleted. 18 pure tests passed via V8/Node assertion emulation and 1,488 generated merge cases passed; native npm/Rust/Tauri and installed Win/Linux UI acceptance are **OPEN**. Documentation: `ide/docs/IDE_B11_REVIEWED_EXTERNAL_MERGE_20261108.md`. B2–B10 independent GitHub acceptance gates remain running/pending and will be evaluated when results arrive, rather than deferring their importance.

 
## C7 task-to-source evidence integrated with C6 ledger — November 8, 2026

Branch `ide/c7-task-source-evidence-20261108` stacked on B11 exact head `d49de2bda55f566b5ef8bc5f020be1d0cfa80863`. Reuses canonical C6 `development-ledger.jsonl` with new `task.source.link` events, max 12 registered source associations per task, native re-read of relative saved UTF-8 source and SHA, exact task-update event identity, current project and optional Git SHA/worktree snapshots. Earlier links remain historical when a task is edited; existing C6 CI observations, status and acceptance gates remain independent. Explorer offers explicit Load C6 tasks / Link saved file to task. Continuity shows source links and separate native disk recheck, with no source text or unsaved draft bytes persisted, no new task database, and no automatic Save or worker permission. Added 5 native filesystem tests and 9 frontend regressions; 9/9 exact-source V8 test-body checks pass, but real Node/Rust/Tauri/installed Windows/Linux results remain OPEN. Dedicated matrix `.github/workflows/ide-c7-task-source-evidence-native.yml` added; installed acceptance will be reviewed when GitHub completes, not treated as deferred. Handoff `ide/docs/IDE_C7_TASK_SOURCE_EVIDENCE_20261108.md`.


## DEV-03.1 fixed project build/test discovery — November 8, 2026

Branch `ide/dev03-project-run-profile-discovery-20261108`, stacked on C7 exact commit `07f0a889bd14c3d587e26678307a5c8d32f9fe6e`. Starts Group D DEV-03. New native `IdeWorkspaceRegistry.discover_project_run_profiles` and Tauri `ide_discover_project_run_profiles` inspect only six built-in project manifest conventions, recording current saved SHA or missing/unreadable, and at most 20 conventional test file paths from bounded read-only `tests/` listings. Source text, scripts, workers, commands and effect authority are not accepted from UI or exports. React Jobs page offers explicit Inspect, showing true unavailable Python/JS/CMake project adapters; it can select an existing Cargo recipe ID only, still requiring the unchanged protected WO6 snapshot/pins/OS approval before any actual Run. Typed catalog now 48 operations and 45 definitions. Six native filesystem tests committed; nine exact-source frontend test bodies passed in local V8 with Node assertion emulation (NOT native Node). Workflow `.github/workflows/ide-dev03-project-profiles-native.yml` queues real Windows2022/Ubuntu24 Rust/Node/Tauri source checks. This is **DEV-03.1 source-implemented only**; DEV-03.2 separately approved execution adapters, DEV-03.3 streaming output, DEV-03.4 full test discovery, DEV-03.5 kernel cancellation/installed E2E remain open. Original Group A/B OS failures and Group C candidate consolidation remain independent parallel gates. Handoff: `ide/docs/IDE_DEV03_PROJECT_PROFILES_20261108.md`.

## DEV-03.2A protected Rust project Cargo Build — November 8, 2026

DEV-03.2A on branch ide/dev03-reviewed-rust-build-20261108, stacked on
DEV-03.1 exact 80486012e817de2f6c64892695e66ebb99292286, extends the
existing WO6 frozen-source and reviewed native Cargo runner with a fourth
fixed recipe: rust_build. Exactly Cargo build --locked --offline --manifest-path
Cargo.toml under original Linux cgroup/Bubblewrap and Windows
AppContainer/Job/SCM pinned toolchain routes. The original snapshot,
clean Git HEAD, fresh native OS confirmation, one-use supervisor admission,
kernel descendant process cancellation and durable journal remain intact.
Jobs UI exposes Cargo Build and the read-only discoverer now has seven
profiles; Python pytest, JS/TS tests and CMake are still nonexecuting
until their own reviewed native adapters exist. Typed profile schemas now
have seven bounded entries. Added native Rust plan/snapshot/intent hostile
tests and seven new cross-layer JS contracts; all sixteen JS test bodies
passed under exact-source V8/Node-assert emulation (NOT native Node).
Native real OS workflow: .github/workflows/ide-dev03-reviewed-rust-build-native.yml.
Exact-head Windows/Linux Rust/Node/Tauri and installed protected build,
cancel/restart and worker source-immunity acceptance remain OPEN.
Engineering handoff: ide/docs/IDE_DEV03_REVIEWED_RUST_BUILD_20261108.md.



## DEV-03.2B.1 multi-language adapter review contracts — November 8, 2026

Branch ide/dev03-multilang-native-adapter-review-20261108 stacked on exact
DEV-03.2A 1d6b1325967cfa20e8be7bba395e33ac2e506ff4. The native host now
defines three immutable non-Rust adapter contracts: Python pytest, Node builtin
test (JS/TS compatibility separately unverified), and CMake configure/build/
CTest logical pipeline. An explicit native registered-project review checks
clean Git HEAD, cap-std source reads, and exact saved manifest SHA-256 with a
second preflight recheck. Review fingerprints are NOT credentials. There is
NO toolchain bundle, frozen source snapshot, actual native OS worker, issued
grant, source execution, task acceptance or installed acceptance for these
languages. Frontend Jobs screen displays fixed steps and eight required
future admission gates but deliberately offers no Run option. Added Tauri
ide_review_multilang_adapter, capability job.reviewAdapter, now 49 catalog
operations and 47 typed definitions, six genuine native Git/filesystem
regression fixtures and 11 frontend hostile contract tests. Exact eleven JS
test bodies passed under development-only V8 assertion emulation; real
Node/Rust/Tauri/installed Windows/Linux results remain OPEN. Dedicated CI
.github/workflows/ide-dev03-multilang-adapter-review-native.yml requires
exact-head Windows 2022 and Ubuntu 24.04 tests and verifies original
supervisor, protected workers and native build backend were unchanged.
Engineering handoff: ide/docs/IDE_DEV03_MULTILANG_ADAPTER_REVIEW_20261108.md.
Outstanding DEV-03.2B.2/.3/.4: pinned offline runtime bundles, reviewed
platform-specific contained worker execution, real cancellation/recovery,
artifacts and installed OS acceptance. The original product readiness
BLK-05/06 and Group C integration gates remain separate open failures.


## DEV-03.2B.2 portable offline bundle hardening — November 8, 2026

On `ide/dev03-multilang-offline-runtime-bundles-20261108`, packaged runtime trees now fail closed on Windows reserved device names (including qualified names such as CON.txt) and on Unicode-normalized/case-folded relative-path aliases that could turn distinct Linux archive entries into the same Windows destination. Tests include device-name, case and Unicode normalization fixtures, with filesystem-specific skips only where the host cannot create the invalid fixture. Existing tree-SHA, file count/size limits, symlink/reparse/hard-link rejection, expected-pin admission and candidate/approved separation remain unchanged. This is a source guard, **not** evidence of successful Windows/Linux native tests or authorized runtime execution. Exact-head candidate workflows and installed AppContainer/Job/SCM or Bubblewrap/cgroup-v2 acceptance are still OPEN. Python/Node/CMake execution remains opt-in and fail-closed until verified worker bindings, cancellation, durable recovery and genuine OS evidence pass.


## DEV-03.3 protected Jobs phase progress — November 8, 2026

Branch `ide/dev03-streamed-job-progress-20261108` stacked on DEV-03.2B offline runtime candidate exact `76f35dab32670b36c4faa55777a4178f5c169ab7`. Adds bounded, selected-project-only, read-only native `ide_build_status` phase history in the Jobs view. Known Cargo recipe IDs, native job IDs, valid phase codes, up to eight recent entries per poll, and at most 96 historical observations; repeated polling is deduplicated. Active running/cancel-requested phase is represented only once a real native job ID exists. Separate quarantined records remain in the existing recovery UI because their native schema lacks a recipe identity. Source regressions and Windows/Linux exact-head workflow added. **This is live broker phase reporting, not live stdout/stderr**; true bounded native output transport, protected worker integration, terminal sealing, restart semantics, native CI, and installed GUI acceptance remain OPEN. See `ide/docs/IDE_DEV03_JOB_PROGRESS_20261108.md`.


## DEV-03.3B protected worker output — November 8, 2026

On `ide/dev03b-protected-output-streams-20261108` stacked on DEV-03.3 exact `f8590b5ee735d7d62cc7b027565395879012164b`, the existing authorized build worker gains an optional native-only `LiveOutput` recorder (bounded rolling 64 KiB/256 chunks, 64 chunks per read), with cursor and stream identity, in-memory only. Linux captures real stdout and stderr separately from existing bounded pipes after native cgroup admission; Windows now inherits distinct, explicit stdout (`cargo-stdout.log`) and stderr (`cargo-diagnostics.log`) handles in the same protected Job Object worker and reads both incrementally, preserving their identities but not claiming cross-stream total ordering. Project-registered exact native current/last-finished job scoping is enforced in Tauri command `ide_build_read_output`; no path, executable, command, grant or journal replay is accepted from UI. Jobs view polls and performs a bounded final drain, rendering advisory untrusted text distinctly from durable result and recovery status. Existing journal and kernel termination contract were not broadened. Regression tests and exact-head native Windows/Linux workflow added. Native CI and real installed protected worker E2E with cancellation/crash/stream races remain **OPEN**, so **do not mark DEV-03.3B accepted for shipping** until actual evidence passes. See `ide/docs/IDE_DEV03B_PROTECTED_OUTPUT_20261108.md`.


## DEV-03.4 bounded conventional test discovery — November 8, 2026

Branch `ide/dev03-4-bounded-test-discovery-20261108` stacks on draft DEV-03.3B PR #160 head `b0d714d58424169aade5b1a798b0eab4c138c914`. Registered project host capability now scans seven fixed conventional folders (nonrecursive, metadata only, max 96 files), classifying filename hints for Rust, Python, JavaScript/TypeScript and C/C++ without reading project scripts, executing commands or issuing grants. Rust filenames may map to existing Cargo recipe names as **hints**, but `runnable=false` for every record, and framework discovery is explicitly incomplete. Tauri `ide_discover_conventional_tests` and Jobs inspection UI added with native/JS hostile tests. Exact-head Windows/Linux Node, Rust host and Tauri checks are queued. DEV-03.3B real protected streaming, DEV-03.2B worker admission, comprehensive framework enumeration, DEV-03.5 cancellation/installed acceptance and genuine installed app acceptance all remain independent open gates. Handoff: `ide/docs/IDE_DEV034_TEST_DISCOVERY_20261108.md`.


## DEV-03.5 protected job lifecycle + installed workflow acceptance — November 8, 2026

On branch `ide/dev03-5-protected-job-lifecycle-20261108` stacked on DEV-03.4 PR #161, the native build backend fixes multiword supervisor phase serialization to canonical snake_case (e.g., `cancel_requested`, `recovery_required`) and requires exact JobID for cancellation once assigned, with duplicate request denial and no new execution authority. Jobs now displays a project/plan-bound joined lifecycle projection from genuine native journal, ephemeral streams, and matching in-process result, without claiming OS cleanup or installed acceptance from output, nor replaying missing terminal evidence after restart. Windows NSIS actual UI Automation and Linux Debian AT-SPI installed fixtures were extended to verify the read-only Jobs discovery/lifecycle panels without starting an unpinned worker. Exact-head Rust/Node/Tauri regression workflow, genuine cgroup/AppContainer stream/cancel verification, and installed signed/pinned full-chain run/recover/restart acceptance remain OPEN until independent Windows/Linux evidence passes. Details in `ide/docs/IDE_DEV035_PROTECTED_LIFECYCLE_20261108.md`.


**DEV-03.5 follow-up:** Native status now exposes last in-memory job ID only to the matching project; Jobs may inspect its bounded stdout/stderr after failure or cancellation exclusively when the same JobID is visible in the durable native journal. No output paths or authorization travel to the frontend, and none survive app restart. Both protected real-OS workflows are now triggered on DEV-03.5 app-layer changes for exact-head evidence.


## DEV-04.1 project debugger target provenance preparation — November 8, 2026

After DEV-03.5 draft PR #162, new branch `ide/dev04-1-debug-target-provenance-20261108` starts the next Group D debugger work without waiving the pending DEV-03 acceptance gates. Native registered-project-only review of the fixed `Cargo.toml` requires exact saved SHA-256, clean Git HEAD, capability-bound manifest access and a second read/worktree check. It returns only a domain-separated review fingerprint and declared missing prerequisites: no sealed source→binary manifest, no independently attested executable/debug symbols, no project debugger adapter, no grant, no process, no installed acceptance. Tauri command `ide_review_project_debug_target` and Jobs UI display source review. Host Git/dirty/symlink tests, JS forged-target tests, and exact-head Windows/Linux Rust/Node/Tauri workflow added. Remaining DEV-04.2–04.5: native build receipt→binary hash binding, debug-symbol attestation, protected project LLDB-DAP admission and installed OS acceptance. All actual Windows/Linux CI, signed/pinned worker and installed acceptance gates remain OPEN. Handoff: `ide/docs/IDE_DEV041_DEBUG_TARGET_PROVENANCE_20261108.md`.


## DEV-04.2 native build-to-executable witness — November 8, 2026

Branch `ide/dev04-2-build-binary-provenance-20261108` stacks on DEV-04.1 draft #163. Fixed supervisor build-test evidence review now includes `rust_build` (previously excluded). After an actual zero-exit OS-contained Completed receipt with exact source/plan/toolchain and no quarantine, the host can independently rehash the unique direct `target/debug` ELF/PE candidate from a bounded post-termination output witness and bind its SHA, output tree and terminal receipt to the immutable worker result. UI verifies exact hash/receipt/manifest identity and never permits debugger launch. Linux delegated WO6 `--build` and Windows D7B/D7D `--build-native-worker` genuine protected fixtures now require this binary check, alongside native source/hostile frontend/desktop compilation matrix. This is neither signed binary provenance nor symbol attestation. Actual exact-head OS CI, installed signed/pinned app debugging, binary format/symbol closure, output-tamper and debugger authorization remain OPEN. See `ide/docs/IDE_DEV042_PROTECTED_BUILD_BINARY_20261108.md`.


## DEV-04.3 executable format and debug-symbol identity — November 8, 2026

Branch `ide/dev04-3-executable-debug-symbols-20261108` stacked on DEV-04.2 draft PR #164. The protected build backend can now independently rehash the exact native completed Cargo executable and, after that binding, structurally verify x86-64 ELF64 plus bounded uncompressed DWARF info/abbrev/line sections on Linux, or PE32+ executable/CodeView RSDS plus the output-manifest PDB MSF7 Info GUID/age and DBI age/x64 on Windows. The native `DebugSymbolWitness` binds symbol digest, binary digest and output tree to the same host-only result. The UI validates all exact receipt-to-symbol identities, with `sourceLineMappingVerified=false` and `debuggerExecutionAuthorized=false`. Real Linux cgroup/Bubblewrap and Windows AppContainer/Job/SCM fixed Cargo build acceptance fixtures now require this witness; format/hostile frontend tests and exact-head Windows/Linux native CI added. All CI, deep semantic debug record verification, handle custody against hostile swaps, genuine signed/pinned installed debugger execution and OS recovery remain OPEN. Handoff: `ide/docs/IDE_DEV043_DEBUG_SYMBOLS_20261108.md`.


## DEV-04.4 protected project LLDB-DAP integration — November 8, 2026

On branch `ide/dev04-4-protected-project-debug-20261108` stacked on DEV-04.3 draft PR #165, opt-in feature `native-project-debug` requires both `native-build` and `native-debug`. New supervisor job recipe `dap_lldb_project` accepts only a full native target binding digest (source/head/snapshot, original zero-exit protected Cargo build job/plan and durable terminal receipt, toolchain, independently rehashed binary and verified DWARF/PE-PDB hashes), incorporated into the new D3 plan hash and one-use approval. NativeBuildRuntime keeps only the latest private, doubly rehashed executable/symbol seal under in-process project scope; nothing can silently reauthorize after restart. Fresh native OS approval uses the exact target and original build receipt. Existing Linux cgroup/Bubblewrap LLDB-DAP worker ro-binds its staged project target; Windows AppContainer/SCM worker independently stages/rechecks exact binary/PDB inside LocalState. Host-owned DAP protocol accepts only native staged no-args/stdio-disabled stop-on-entry launch; no renderer paths, PIDs, arbitrary terminal or scripts. GUI project Debug button submits project ID only and remains unavailable in default/release-unpinned builds; frontend rejects mismatched DAP/build receipts or unverified cleanup. Debug cancellation requires exact JobID; debug phase labels corrected. Native hostile source, private target/tamper, DAP and JS tests and Linux genuine protected build→LLDB stop/stack→journal-reopen acceptance fixture added; dedicated Windows/Linux `native-project-debug` source matrix added. **All OS results and installed acceptance remain OPEN**. Windows full real project debugger D7D and signed installed E2E are not yet tested; no broad debugger release claim. See `ide/docs/IDE_DEV044_PROTECTED_PROJECT_DEBUG_20261108.md`.
