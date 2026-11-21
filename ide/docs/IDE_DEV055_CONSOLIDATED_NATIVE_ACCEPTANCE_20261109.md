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

## Task 4 — second actual native OS finding

At exact-head `c2426057dd28a68ec8b40120435085caa61f16f1`,
[run 42732291264](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42732291264)
passed both Windows/Ubuntu compiler matrices and delegated Linux offline-miss
and cancellation. It passed the prior Bubblewrap argv error, but the
CI-only positive wheel caused `output_bounded:false` and a fail-closed
`SUPERVISOR_RECONCILIATION_REQUIRED` before completion. Independent
local `uv` reproduction showed its normal wheel link-index cache
generates symlinks under `uv-cache`, whereas the strict scanner admits
only fixed virtualenv interpreter aliases.

The correction retains the source-pinned read-only wheelhouse and
network-denied namespace, and passes `--no-cache` **only** for the
`ide055-ci-wheelhouse` and exact synthetic-plan identity. Temporary
uv cache content stays inside the isolated `/tmp` mount, and is never
promoted into private output or a project. It does NOT broaden the
output scanner's symlink allowlist or alter production offline cache
semantics. The next exact-head native runner must still independently
prove positive install, tamper rejection, crash/restart reconciliation,
source preservation and proper final receipts before Task 4 acceptance.

## Task 4 — native formatter-only repair

At `a0330edef320fcca6991cf45449502744ca63bd4`,
[run 42733387421](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42733387421)
Ubuntu's native Node, host, supervisor and Tauri compile checks passed.
The grouped gate still correctly denied acceptance because `cargo fmt --check`
required multiline formatting for the new four-element fixed CI wheelhouse
args. The following commit applies the **exact formatter hunk** from the
native Ubuntu result, changing no command argument, containment, grant,
kernel cleanup, hash or installed package behavior. A new exact-head
Windows/Linux group remains mandatory; this prior run is not approval.

## Task 4 — real uv 0.9.9 bounded installer thread-pool failure

At exact-head `daae5b7cabf3382f168119921fbbf86b171e5036`,
[run 42733978070](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42733978070)
passed both Windows and Ubuntu native source matrices plus real delegated
Linux negative offline-miss/cancellation. Positive native execution confirmed
`tree_reaped:true`, `output_bounded:true`, and the real `uv` process
resolved and prepared the single CI synthetic wheel, but exit was `101`
due to `failed to initialize global rayon pool: ThreadPoolBuildError`
with OS code `11/EAGAIN` (`Resource temporarily unavailable`) under the
reviewed **8-PID cgroup limit**. The failure was independently observed in
the bounded CI-only diagnostic, not inferred from source.

The fix sets **`UV_CONCURRENT_INSTALLS=1`** as a trusted Bubblewrap
environment option *before* the command separator, and only for the exact
`ide055-ci-wheelhouse` synthetic positive plan. This is uv's supported
installer/unzip concurrency control (rather than increasing allowed PIDs,
relaxing containment, or using project-supplied config). Both `--offline`,
`--no-cache`, `--no-build`, `--no-install-workspace`, and the strict
installed-byte/source/journal checks remain unchanged. No production worker
inherits this CI-specific environment setting. The next exact-head native
matrix and delegated-process/hostile-restart group must still pass.

## Task 4 — Windows native formatting-only follow-up

At exact-head `4001538caeabbe59e898ab9787691341f988180b`,
[run 42735112718](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42735112718)
Windows native Node, host, supervisor, and Tauri jobs **passed their real
compiler/test phases**, but `cargo fmt --check` denied the consolidated
gate because rustfmt requires one line for the fixed
`UV_CONCURRENT_INSTALLS` Bubblewrap environment setting. This exact
formatter change does not modify any arguments or admission logic;
the previously queued Linux delegated process proof remains **OPEN** until
a complete post-change grouped exact-head run independently passes.

## Task 4 — genuine positive wheel PASS; hostile replay expectation corrected

At exact-head `9e62d7055b666178b484d70d2b6b29e7cc9a120e`,
[run 42735910714](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42735910714)
passed both Windows and Ubuntu exact-head native compiler/test/formatter matrices.
The real Linux systemd-delegated cgroup + Bubblewrap fixture also independently
passed **offline-miss**, **cancellation**, and **positive wheel install**:
the positive protected uv 0.9.9 child exited **0**, all descendants reaped,
private output bounded, synthetic archive/RECORD/license/module bytes
verified, tampered private module correctly denied, source unchanged, durable
terminal receipt produced, no restart execution grant.

The remaining `--hostile-recovery` case genuinely aborted via **SIGABRT**
after a durable native Running receipt but stopped with
`IDE055_RESTART_DID_NOT_REQUIRE_NATIVE_RECONCILIATION`. The original fixture
incorrectly expected reopened `phase == RecoveryRequired`. The actual
supervisor contract intentionally replays the durable phase **Running**
with **requires_reconciliation=true/quarantine**, and its explicit
`reconcile_interrupted_with` writes the `RecoveryRequired` receipt
*before* calling the kernel cgroup terminator, then a terminal
`Cancelled/os_orphan_cleanup_verified` receipt after success. Source
tests already enforce that no fresh grant survives reopening.

This repair changes only the **CI hostile-fixture assertion** to require
quarantined `Running` at reopening, invokes real native cgroup cleanup
unchanged, and additionally asserts both the recovery-before-OS and
kernel-cleanup terminal receipt codes in the independently reopened chain.
It does **not** bypass quarantine, auto-retry, skip native kernel evidence
or alter production logic. The complete group must be rerun at the new
SHA, including genuine SIGABRT and successful restart reconciliation.

## TASK 4 CI-only closeout — ACCEPTED (2026-11-10 UTC)

**Code evidence SHA:** `7c9075478f6428630e5980aad549da1b10a370d0`.
**One complete grouped native acceptance:** [run 42737200464](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42737200464),
**SUCCESS**, with the following exact-SHA native jobs:

| Gate | Actual job | Result |
| --- | --- | --- |
| Ubuntu 24.04 Node, Rust host/supervisor, Tauri feature, hostile fixtures, rustfmt and lockfiles | [129569214086](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42737200464/job/129569214086) | PASS |
| Windows 2022 same native task-group matrix | [129569214458](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42737200464/job/129569214458) | PASS |
| Real Linux `systemd-run` delegated cpu/memory/pids cgroup, AppArmor and Bubblewrap; fixed protected `uv==0.9.9` | [129571664578](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42737200464/job/129571664578) | PASS |

The Linux worker's four independent OS modes **all passed**, not merely compiled:
1. `--offline-miss`: genuine absent-wheel denial (native exit 1), bounded outputs, kernel process tree reaped, source intact and no replay authority;
2. `--cancel`: native Cancelled, real process tree gone and private journal reopened without an execution grant;
3. `--positive`: real `uv sync --offline --locked` **exit 0** under **8-PID** cgroup limit with no network or source writes; synthetic wheel archive SHA-256
   `4243c4b32ebff42e41e19d07152076028cb27c4991116be4f96ccfa67dc08d1b`;
   installed WHEEL/METADATA/LICENCE/RECORD/module bytes independently verified;
   modified installed module denied, restored original bytes reverified;
   true terminal receipt
   `cf3252797d5ca17cdbd2accddf8f04fce0f7b9421415cd5d7504b805e43fcfcc`;
4. `--hostile-recovery`: actual **SIGABRT after fsynced Running**, a **different native OS process** reopened the quarantined journal, appended durable
   `RecoveryRequired/recovery_before_os_reconcile` before `cgroup.kill`, verified kernel descendant cleanup, appended `Cancelled/os_orphan_cleanup_verified`, reopened the journal again, observed no durable execution grant, no auto-retry and unchanged original source. Exact log marker: `IDE055_HOSTILE_RESTART=PASS` with `kernel_reaped=true, replay_grant=false, auto_retry=false, source_preserved=true`.

**Retained GitHub Actions artifacts:** Ubuntu
[13728255778](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42737200464/artifacts/13728255778);
Windows
[13728016963](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42737200464/artifacts/13728016963);
Linux OS receipt
[13726944178](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42737200464/artifacts/13726944178).
The artifacts preserve exact-SHA audit coordinates; detailed kernel/hostile receipts are in the job log, not substituted by the small status files.

**Disposition:** Task 4's bounded **CI-only first-party synthetic-wheel acceptance is CLOSED**. This is not a release, merge, package-manager promotion, signed/installed app, real person approval, original upstream wheel/source-provenance or physical Windows AppContainer execution. Those independent Task 5/release/operator gates remain **OPEN**. Task 5 was not started in this group.

This section is a **documentation-only closeout appended after the verified code SHA**. It changes no Rust, Tauri, host, test fixture, workflow, dependency or runtime bytes. Do not misrepresent the doc-only follow-up SHA as a separate native compile/run: the native test is bound to the code SHA above.
