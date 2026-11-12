# IDE FIN-01–07 implementation register — source audit, not closure

**Branch:** ide/fin01-07-implementation-20261111, stacked after INT06.
**Status:** The product is NOT feature-frozen or accepted; no merge/release.

| Group | Implemented in this branch | Still missing from intended feature |
|---|---|---|
| FIN-01 | Build-supervisor global busy/recovery booleans; fail-closed profile selection; bounded Workbench reads | Independent shared cross-supervisor native race/restart demonstration; true OS-level custody |
| FIN-02 | Opt-in protected Cargo profile, remeasured fixed Cargo/Python/Node/CMake/LLDB/service pins, independent staging utility and explicit Windows NSIS/Linux Debian runtime placement | Approved installed binary payloads; source-exact Linux delegated production host, Windows SCM/AppContainer consent, native/installed isolation tests, version/license closure |
| FIN-03 | Exact-HEAD public PR reviews, partial public branch rules, review/CI correlation and read-only local stash refs | 6E-05 authenticated publication; complete branch/ruleset verification and local revert, stash mutation, unstage and worktree changes |
| FIN-04 | Test Explorer, original native case reports and bounded source-revisioned test-symbol hints | Genuine per-test protected execution, comprehensive framework enumeration, native production debugger action consent, independently proven breakpoint-hit/step |
| FIN-05 | Private package lifecycle, global worker custody and fresh native one-use confirmed vcpkg.json direct declaration edit | General package Add/Remove/Upgrade resolution/installation, vcpkg/Conan protected executors, wheel provenance, Windows uv ACL proof |
| FIN-06 | Guided human-only editor/LSP/build/test/debug/Git/acceptance panel navigation | Optional protected PTY; real installed accessibility and owner workflow evidence |
| FIN-07 | Explicit source-scope register in Workbench and this ledger | Feature freeze requires owner approval and sufficient P0 implementation evidence |

**Separate consolidated acceptance (deferred by owner):** exact-head compiler/Node/Rust, real Windows AppContainer SCM/Job Object and Linux cgroup-v2/Bubblewrap, original immutable supervisor receipts and cancellation/restart, installed NSIS/Debian upgrade/uninstall/preservation, physical approve/refuse/timeout, accessibility and distribution/licensing review.

**Working policy:** Continue source implementation first, retain CI/PR/installed acceptance gates as OPEN, do not rebrand source tests as kernel/installed passes. Lu Core/Lu DLC bridge and self-development IDE remain explicitly post-Lu-Core-v1.0 optional work, not blockers for standalone alpha. No release or main merge authorized.

## FIN-02 source gates vs real OS acceptance

FIN-02B/C/D source scripts and configurations are not evidence of
a native install. The checked-in WO7 input inventory still has
`approved:false` and no actual runtime binaries; the protected-alpha
build must refuse absent pins. The existing Linux systemd 254+
`lu-ide-delegated.service.in` remains an **unexpanded installer
template** whose pinned native production host and cgroup service
installation have not been demonstrated. The existing Windows D7D
recovery-service binary and AppContainer/Job Object witnesses also
require original installed OS execution. No executable or manifest
was falsely promoted to release approval.

FIN-03–FIN-05 add read-only GitHub/Local Git observations, native
test-symbol hints and one reviewed vcpkg *source declaration edit*,
not authenticated push/merge, production LLDB stepping, per-case
execution or package installation. FIN-07 feature freeze remains OPEN.

## FIN 02–05 continuation (same source branch)

- **FIN-02E:** read-only Linux kernel process/cgroup admission readiness
  now gates Cargo and multilang UI status as well as the protected
  worker's pre-existing per-job cgroup proof. A normal uncontained
  desktop process stays unavailable. No resident delegated production
  host/IPC service has been packaged or installed; the existing systemd
  host entry remains a template awaiting a verified first-party
  production native binary, reviewed package install, ownership,
  delegation and genuine hostile-process evidence.
- **FIN-03:** one-file index-only reviewed native unstaging and separate
  no-replay recovery classification, plus scoped public review and
  stash read-only observations. No authenticated remote publication,
  automated stash/revert or worktree mutation.
- **FIN-04:** native test-symbol selection can now request a fixed
  exact Rust libtest recipe through the existing protected
  Cargo worker, with frozen source SHA, original test case report and
  no implied test assertion or human task acceptance. The new parser
  path was corrected to preserve original case events. Production
  debugger controls remain developer-only behind the old
  intentionally unshippable feature; framework-true discovery and
  complete per-case test runner matrix remain open.
- **FIN-05:** separately opted-in Linux uv Sync uses a reviewed staged
  and installed uv/Python bundle hash, no-network Bwrap, existing
  native approval, durable journal and private output. This does not
  add Windows uv ACL permissions or general vcpkg/Conan resolvers.

**All claims are source-level only.** No actual approved native release
binaries were obtained; no Windows AppContainer or Linux cgroup-v2
installed child/process witness was executed; no Rust compiler,
Node test, CI or PR acceptance has run in this continuation.
No main merge, release or feature freeze is authorized.

## FIN-03–07 continuation — source-only, no closure

The following follow-on work is implemented on this existing IDE FIN branch:
- FIN-03: local stash reflog read-only inventory now rejects records which are not two/three-parent stash-shaped commits or have an identical base/index parent identity. **This does not implement** stash save/apply/drop, local revert, remote push, worktree mutation, or native one-use consent for those operations.
- FIN-04: the Test Explorer rejects hostile source-hint navigation, forged job identities and duplicate case identifiers. The protected exact Rust case path pre-exists. **Still open:** genuine multi-framework protected per-case execution; shippable native LLDB breakpoint/step/variable authority; installed proof.
- FIN-05: reviewed vcpkg direct source-declaration edits reject malformed root special fields. Existing offline uv Sync remains separate. **Still open:** vcpkg/Conan protected installed operations, resolver/cache integrity, Windows uv ACL, dependency provenance.
- FIN-06: recovery-required Workbench observations cannot promote build/debug evidence or navigate the guided workflow. **Still open:** installed human workflow, accessibility, optional PTY, proof of cross-supervisor continuity.
- FIN-07: read-only exact-source evidence reconciliation independently tracks compiler, Windows/Linux native, Windows/Linux installed, owner, security, and license gates. The view is an advisory projection, not connected to release authority or native receipt verification. The feature freeze is not authorized.

FIN-02 remains explicitly deferred as directed; FIN-01 cross-supervisor production custody is also open. All source changes in this pass are uncompiled and without execution receipts; regression fixtures have been added but not run. Full FIN-03/04/05 functionality and FIN-06/07 production integration are not complete. No merge to main, no release and no feature freeze.

## FIN-03, FIN-04, FIN-05 continued implementation — 2026-11-11

- **FIN-03 actual Git worktree mutation candidate:** added `git_reviewed_restore.rs`, Tauri one-use OS-reviewed single-file restore, a GUI review/execute/recovery panel, real disposable `git2` fixture tests and hostile UI contract tests. It refuses other statuses, changed HEAD/bytes, unsupported types and unsafe paths, and saves original modified bytes under the exact local Git directory before replacement. The interruption marker never authorizes automatic retry. Its destructive executor is **default-disabled** behind `fin03-reviewed-restore` until native Windows/Linux correctness, permission/ACL preservation, race and interruption evidence are accepted. Stash apply/drop, broad revert and authenticated publication remain open.
- **FIN-04 production debugger:** developer-only Windows D6D step/breakpoint/variable action code remains explicitly unshippable, and independent breakpoint/variable results remain uncertain. The desktop admission projection now rejects forged/unknown capability codes and keeps developer-only control separate from future production admission, including global OS recovery denial. No production debugger privilege was enabled. The Windows owner-event/LLDB pipeline is still CI/developer-only, therefore production authorization/installed native tests remain a required implementation group.
- **FIN-05 offline vcpkg worker source:** introduced `package-vcpkg-worker` as an opt-in, Linux-only supervisor feature; exact runtime tree/licensing requirements; independent reviewed package JobPlan hash, max 10-minute runtime, 12 processes, 128KiB logs; a fixed Bubblewrap command with no-download and private output paths; and regression source fixtures. **Not a callable desktop operation:** no accepted vcpkg binaries/pins, human-approved native desktop execution entrypoint, post-run package graph/provenance, or original installed acceptance exists. Conan and Windows remain untouched. Existing Linux uv Sync remains separate.
- **FIN-02 deferred:** source work must not be relabeled production custody. FIN-04 and FIN-05 cannot be closed as operational release functionality until exact protected native runtimes and OS authorization/containment are demonstrated.
- **Tests and closure:** New Rust/Node fixtures were committed but not run, and no CI workflow, native OS execution, physical-human acceptance, package installation, feature freeze, PR merge or release was performed.

The next source engineering tasks are (1) reconcile/repair the FIN-03 restore through exact-head native execution with genuine interruption, (2) move independently verified Windows protected LLDB owner-event operations from developer test feature to a genuinely shippable native authorization path without weakening release policy, and (3) wire Linux vcpkg package preview into separate one-use consent, worker receipt and source/installed provenance verification; then complete Conan/Windows. These are implementation tasks, not acceptance-only labels.

## FIN-06B/FIN-07B consolidated workflow and scope reconciliation

The existing Workbench now displays two explicit, separately sourced projections:
FIN-06B project-scoped Git/build/debug/package/recovery observations, and
FIN-07B conservative unresolved implementation boundaries derived from the
registered FIN group scope. These are **advisory observations**, not a mutable
task register or an independent OS-evidence verifier.

- FIN-01: cross-supervisor/OS custody and interrupted-worker reconciliation open.
- FIN-02: intentionally deferred; trusted production runtimes and Windows/Linux
  installed service/worker admission are not accepted.
- FIN-03: one-file restore remains opt-in and native-unverified; remaining
  stash/revert/worktree/remote publication features open.
- FIN-04: developer debugger control cannot masquerade as installed/production
  stop/step/breakpoint authority; framework coverage and real OS acceptance open.
- FIN-05: opt-in Linux vcpkg worker source is not a callable approved desktop
  package operation; provenance, Conan and Windows execution open.
- FIN-06: source-facing human workflow now correlates native observations
  and recovery. Installed accessibility, restart, optional PTY and owner operation
  acceptance remain open.
- FIN-07: scope register and conservative reconciliation exist in source,
  but human feature-scope decision, dependency-license closure, native exact-head
  compiler/tests and installed OS acceptance remain open.

The reconciliation always reports featureFrozen=false and release eligibility
false. New Node regression fixtures verify foreign-project rejection, recovery
denial and the inability to promote observations into release authority.
No native tests, CI, installer verification, human acceptance, merge or release
were performed in this development slice.

## FIN-04H source continuation — 2026-11-12

The existing FIN branch now also contains a **source-only** protected
single-case route for saved Python/pytest and Node test candidates
alongside the prior exact Rust libtest path. A fixed native framework
recipe must pass bounded symbol/path grammar, saved source SHA,
manifest SHA, original clean Git HEAD and immutable native input checks.
The previously admitted, pinned AppContainer/Job or cgroup/Bubblewrap
worker and original fresh OS confirmation remain the sole launch route.
The Test Explorer calls the dedicated command and accepts original
job/plan/receipt-bound results; test-runner output is **reported**, not
independently verified assertion evidence.

A source-level regression contract has been added but has not been
run in native Node; compiler and installed OS verification are open.
One pre-existing malformed Linux vcpkg manifest match which referenced
an uninitialized command was corrected; this does not enable the
vcpkg executor. CTest built-test enumeration and exact-case
execution are still missing.

The production debugger is **not promoted**: D5 live LLVM event
owner, D6B fresh one-use human grant, D6C independent post-command
stop/PDB verification and OS-only action recovery are still gated
behind debug-only native paths or unimplemented release integration.
The original action journal inspection remains immutable and
global unresolved actions still deny new protected admission.
Neither a terminal DAP ACK nor a CI-only receipt may become release
authority. Full boundaries and required native acceptance are in
[`IDE_FIN04H_EXACT_FRAMEWORK_AND_LLDB_RELEASE_GATES_20261112.md`](IDE_FIN04H_EXACT_FRAMEWORK_AND_LLDB_RELEASE_GATES_20261112.md).

FIN-02 remains deferred. No CI, physical Windows/Linux test,
installer acceptance, feature freeze, release or merge to main was
authorized here.
