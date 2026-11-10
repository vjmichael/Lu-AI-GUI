# IDE-05.5 — One consolidated Windows/Linux native acceptance gate

**Initiated November 9, 2026.** Workstream:
`ide/dev05-1-package-discovery-20261109`, stacked on IDE-05.4
`6379bc112cf871ae00c960f1b3ca01816552c30d`.
All results described below must be tied to their **actual exact-head SHA**
and GitHub Actions job/log/artifact ID, not source-only assertions.

## Acceptance policy

One **group** workflow, `.github/workflows/ide-dev05-consolidated-native.yml`,
is triggered on this named IDE-05 branch at the group boundary and can be
re-dispatched manually when the native runner queue is healthy. The
two older push-triggered standalone micro-workflows
`ide-supervisor-core.yml` and `ide-d7-windows-worker.yml` now keep
only PR/manual triggers. Other existing jobs may continue queuing and
the previous runs are not implicitly cancelled by changing those YAMLs.

The consolidated matrix checks the genuine Windows 2022 and Ubuntu
24.04 runner compilers for the exact repo head:
- locked npm install with scripts disabled; IDE-05.1–.4 Node source-boundary
  tests and frontend build;
- real native Rust host tests for registered project and revision-bound
  preview/revalidation;
- real native supervisor compilation and all-target unit/hostile tests
  with **package-uv-worker** enabled and correct Windows/Linux OS features;
- opt-in **native Tauri** compilation with `native-package-uv`, using the
  exact committed `Cargo.lock` (not installed GUI proof);
- Cargo rustfmt and source lockfile immutability; retain per-OS SHA/run IDs
  in CI artifacts even on failures.

A separate **genuine Linux OS child** job depends on both compiler jobs.
The runner constructs a **CI-only self-measured candidate**, `uv==0.9.9`
plus system Python and observed license-source files in a fixed root-owned
tree under `/opt/lu-ide/runtimes/uv_sync_offline`. It embeds the
observed tree hash at compile time. This is *not* an upstream-attested
or production-approved runtime pin. The test verifies that the real
Bubblewrap binary can create namespace isolation under the
distribution-reviewed AppArmor profile. It then uses **systemd's exact
delegated cpu/memory/pids cgroup**, the original D3 supervisor,
the fixed `uv_sync_offline` recipe and a real `uv` binary:

1. `--offline-miss`: Require an unavailable locked registry wheel to
   terminate **Failed**, with genuine stderr showing offline/cache
   failure, independently observed native terminal/child cleanup,
   unchanged frozen source and restart replay that grants no authority.
2. `--cancel`: Pre-request real native cancellation for the exact
   fixed recipe, require actual Cancelled and non-quarantined kernel
   process-tree cleanup, then independently reopen the durable journal.
   This is a **CI synthetic approval**, never human dialog evidence;
   it is not a proof that a real operator canceled a running uv job.

The OS worker still cannot run arbitrary commands, use networking,
build sdists, load project scripts or write into the original workspace.
If user namespaces, real cgroups, exact runtime hashes, trusted Python,
the actual uv invocation, the real failure cause or native OS cleanup
cannot be proven, **FAIL** the corresponding gate. No fake success
marker or uncontained process substitutes for the native job.

## Installed application and real-dependency gates remain OPEN

The Windows feature-gated package worker still **rejects** execution
`NoContainment` before child launch because its AppContainer staged
workspace lacks independently approved read-only ACLs. Thus no CI can
truthfully show a successful Windows protected package Sync. The
`ide/app/fixtures/Test-IDE055-PhysicalWindowsPackageDenial.ps1`
physical interactive operator witness tests only **Windows fail-closed
behavior**, requiring one real visible installed executable and unchanged
D3/package journal hashes. It forbids GitHub Actions and never fabricates
an approval or an installed dependency result. It requires actual human
physical-PC execution to generate the evidence JSON; its existence is
not acceptance.

A successful **offline wheel** install is also NOT established by the
Linux negative fixtures. The current worker's output cache is initially
empty, and the conservative installed-artifact verifier rejects many
normal interpreter symlinks and metadata formats. These are not bypassed.
To promote IDE-05 fully, add and independently verify an exact
release-approved offline wheel bundle/source manifest, demonstrate
native OS-contained positive installs with correct local
`.dist-info/RECORD`, original license and wheel-origin archive hashes,
and prove no untrusted links/writes/credentials escape.

The installed desktop must then be run by a real human on the actual
Windows and Linux hosts: fresh approve, decline, timeout, cancellation,
worker crash, restart, OS cleanup, private rollback/discard and source
preservation with exact pinned binaries and durable receipts. The Linux
CI service's synthetic `record_human_approval(true)` fixture may **not**
satisfy the human acceptance criterion. A signed/NSIS release remains
a separate release/installation gate.

## Queue and confidence at implementation checkpoint

At initial inspection on Nov 9 the repository had **104 queued workflow
runs**, including exact-head runs 42677505364 and 42677505507 for
`6379bc1`. Those jobs were `queued` with no steps/logs and
therefore could not prove Windows/Linux native compilation or runtime.
A newly created task-group workflow will also remain **unaccepted**
until GitHub actually provides a runner and its real job logs/artifacts
are inspected and any genuine failures repaired.

**Do not mark IDE-05.5 accepted from the workflow YAML itself.**
No native Windows/Linux compiler, actual protected package process,
human dialog, installed application or real-wheel acceptance is claimed
without genuine post-dispatch evidence. The separate DEV-04.6D debugger
human-modal acceptance stays open.


## Actual GitHub native run inspection and demonstrated repair (2026-11-09)

Exact head `0819e880ba184db98d95b3c6af4994128c7033f6`:
- IDE-05 group run **42704670757** executed on genuine Windows 2022
  (job **129458221541**) and Ubuntu 24.04 (job **129458222099**).
  BOTH failed at the Node regression step. Windows logged **26 tests,
  24 passed, 2 failed**: the IDE-05.3 source test still expected
  `package-uv-worker=["multilang-worker"]` after IDE-05.4 added
  optional pinned parser/hash dependencies; an IDE-05.4 source test
  expected a bare quoted `direct_url.json` instead of an interpolated
  fixed `{dist}/direct_url.json`. These **test patterns** were repaired
  without weakening their assertions. The real native protected Linux
  dependent job was **skipped** because the compile matrix had not
  passed: NEVER label uv OS acceptance as executed.
- Both CI platforms then failed receipt artifact upload because
  `.ide055-receipts/` is a hidden folder and upload-artifact@v4
  excludes hidden files by default. The group workflow now explicitly
  sets `include-hidden-files: true` for its two private receipt
  archives only.
- Previous exact-head `6379bc1` production supervisor run
  **42677505364** (Linux job **129362776917**, Windows job
  **129362777687**) and Windows D7 run **42677505507**
  (job **129362775735**) did compile/test the original supervisor
  features successfully, but each failed **cargo fmt --check**.
  The Linux formatting log had 512 formatter hunks over 22 source and
  example files, much of it inherited DEV-04/D6D formatting debt.
  Formatter-proposed original→new source edits were applied only
  after exact original context matching; no debugger behavior changes
  were intended. IDE-05.5 requires a **new exact-head rerun** for
  confirmation, and Rust/Tauri compilation with package features
  still has no passing native result at this checkpoint.

**Do not elevate these negative/failure results to IDE-05 acceptance.**
The consolidated current group is the only new intended acceptance
batch; all still-pending positive protected `uv`, original package
provenance, human consent, installed-app and Windows ACL gates remain open.

## Native exact-head evidence from run 42709673086 (repair checkpoint)

The first IDE-05.5 task-group repair `ad47cd1` was actually compiled and
tested on Windows 2022 and Ubuntu 24.04 native GitHub runners in
https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42709673086 .
Both runners completed the `native-package-uv` Tauri Cargo compile
**successfully** after the frontend build prerequisite was fixed.
The Node focused IDE-05 suite passed; the complete existing frontend
suite surfaced four stale baseline assertions for recently added
IDE-05/DEV-04.6D commands and formatting. Host tests uncovered one
incorrect assumption that an optional `ide/host/Cargo.toml` must
exist in every project. The protected supervisor unit suite passed 85
and failed one snapshot fixture expectation: when multilang is
enabled, a fixed Python recipe is recognized but a project lacking
pyproject produces `InvalidSnapshot`, not the old assertion's `Io`.
Cargo fmt's other remaining findings were preexisting host formatting
drift, not native build failures.

Those concrete issues are repaired in the next exact-head checkpoint;
the source profile fixture explicitly distinguishes `not_found` and
no SHA for an absent optional manifest, the supervisor test asserts
the actual missing-manifest error, the full frontend command catalog
reflects feature-gated package/LLDB routes **outside Lu capability
authority**, and the host receives rustfmt-only corrections derived
from the real native runner diff.

**No Linux protected uv process was run in 42709673086:** its gate
was skipped because other prerequisite jobs failed. Therefore physical
human approval, installed package acceptance, recovery, wheel-origin
attestation and Windows workspace ACLs remain open.

## Task 4 — synthetic first-party positive wheel and hostile restart

The Task 4 native CI gate extends the same protected Ubuntu systemd-delegated
cgroup, Bubblewrap network namespace and exact root-owned runtime pin.
The dedicated ide055-ci-wheelhouse build feature is absent from both installed
desktop bundles and the normal native-package-uv worker.

The runtime setup constructs a deterministic first-party synthetic MIT wheel,
pins its archive SHA in the exact runtime tree and in the native CI executable,
and serves it from a read-only local wheelhouse. Only the CI fixture may use
this exact local registry identity. User-provided path, Git, editable, sdist
and arbitrary wheel sources remain denied.

The --positive native fixture requires a real successful uv sync --offline
--locked to a private output directory, correct wheel bytes, license text,
installed RECORD hashes, absence of unlisted site-packages files, original
source immutability and denial after installed-module tampering. The resulting
environment is NOT approved for project promotion. In particular, this
self-generated wheel proves neither independent upstream supply-chain origin
nor external publisher/license approval.

For the --hostile-recovery fixture, an opt-in CI-only fault oracle aborts the
native runner after kernel cgroup membership and a durable Running receipt.
A separate native process must replay the journal, find RecoveryRequired,
perform kernel-backed cgroup cleanup, record Cancelled, preserve source data,
and demonstrate no automatic retry or restored execution grant. This is not
a physical desktop/human approval test.

Task 4's CI-only acceptance requires exact-head Windows/Linux native source
jobs plus genuine positive/offline-miss/cancel/hostile-recovery OS fixture
logs and their receipts. The separate production provenance and installed-app
operator gates remain open.

## Task 4 — runner-verified positive-mode argument-order failure

At exact-head `33870ac23456bdb5ac28d2876e327d35f24e045b`,
[run 42731353632](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42731353632)
passed the full Windows/Ubuntu native-source matrix and the real delegated
Linux offline-miss and cancellation cases. The positive case failed with
`bwrap: execvp --setenv: No such file or directory` after native exit 1.
The previous fixture appended CI-only `--setenv UV_NO_INDEX 1` **after**
Bubblewrap's `--` executable separator; it was parsed as the executable,
not as a Bubblewrap setting. The reviewed correction hoists the
synthetic-plan-specific environment flag **before** the separator, keeps
`/runtime/wheelhouse` read-only, keeps production plans without that
flag, and retains the same real cgroup/namespace/approval/receipt checks.

**Do not accept Task 4 from the source patch.** Require one exact-new-head
native Windows/Linux group and independent logs showing positive wheel
installation, installed-byte tampering rejection, kernel-backed hostile
SIGABRT/restart reconciliation, offline miss, and cancellation.
