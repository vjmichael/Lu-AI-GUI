# IDE Group 3 — Security and authorization (2026-11-10)

**Phase:** Implementation and native OS evidence collection. **NOT ACCEPTED as a whole.**
**Stack:** `ide/group03-security-authorization-20261110` on verified Group 2 head
`84a7c4acc72714b52e23eac9ebb62f39daea953b`.
[Group 2 installed Windows/Linux SUCCESS](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42851450696).

## Authoritative trust boundaries

Standalone Lu IDE must keep untrusted projects unexecuted by default.
A human approval binds one precise source/operation/worker Job. Windows
AppContainer/SCM and Linux delegated cgroup/Bwrap need actual kernel
evidence. An unsigned installed GUI/CI simulation cannot manufacture
physical OS dialog consent. Lu Core v1.0 integration remains deferred.

## Task 3A — Physical witness provenance (implemented; human evidence OPEN)

`Test-DEV046D-PhysicalHumanConsent.ps1` previously accepted an exact
visible IDE process name/session and action journal without independently
hash-checking the live process image against the reviewed binary. It now
requires reviewed Git commit SHA, exact executable path and SHA-256;
checks actual same-session visible process `MainModule.FileName` matches
the reviewed file and digest; and records those pins in the observation
receipt. Stale source, wrong executable, same-name spoof or unreadable
live process are denied BEFORE an operator is prompted. The script
never clicks Approve, and is forbidden on headless GitHub Actions.

**Do not count this as a human approval.** A real operator and independent
observer must run refusal, cancellation, timeout, stale-plan, one-use
approval, consumed-grant replay, crash/restart and wrong scope on a
physical interactive desktop with independently reviewed build/OS receipts.
Existing DEV-04.6D 4-scenario witness covers only a subset. Windows
production package worker `NoContainment` denial must stay in force.

## Task 3B — Windows AppContainer/ACL/MIC (native disposable gate)

Existing opt-in `ide056-ci-acl-positive` example makes an actual
AppContainer child under Job custody, checks its low-integrity token,
source read, denied write/rename/delete, native WRITE_DAC and
WRITE_OWNER denial, and private output success. Group 3 runs this
**without enabling it in the actual installed app**. The feature
does not cover explicit file mandatory integrity SACL, malicious
reparse/cross-project escape, or production worker permission.

## Task 3C — Windows real SCM custody and crash/loss (native disposable gate)

Reuse existing fixed-hash `Install-ProtectedRecoveryService.ps1`,
`Test-D7DProtectedServiceCrash.ps1` and
`Test-D7DProtectedServiceLoss.ps1`. Each scenario uses a separate
disposable Windows runner, exact compiled service+host hashes, real
SCM/Job owner identity and process-tree cleanup, explicit SCM teardown.
The scripts are genuine kernel observers, not mocked source checks.
Installed signed service upgrade/SCM ACL closure and app-level
interruption recovery remain separately OPEN.

## Task 3D — Linux fail-closed containment / no-replay

Run existing Linux supervisor and sandbox native tests. The cgroup-v2
probe is meaningful only when it reports true parent delegation;
hosted-runner `CONTROLLED DENIAL` is separately recorded as
`NOT_ACCEPTED`. It does not verify Bwrap filesystem/network
isolation, descendant escape, cancellation or restart custody.

## One consolidated exact-head Group 3 gate

`.github/workflows/ide-group03-security-native.yml` is limited to
this branch/workflow path. Its Windows AppContainer, real SCM crash,
real SCM service-loss, and Linux negative/kernel-probe jobs must all
finish SUCCESS for *native CI* acceptance. A green CI result is **not**
human approval or production release. Never promote a failed or
uncompleted job, and never turn on default-disabled protected execution.

## Remaining Group 3 open even if all native CI jobs pass

- Physical Windows/Linux explicit human native dialogs with witnessed
  refusal, timeout, cancel, stale-source, one-use, replay and restart.
- Production Windows AppContainer source/runtime/output exact effective
  ACL + explicit file MIC SACL/owner/reparse resistance.
- Production SCM custody and installed worker recovery across host and
  service crash, complete Linux delegated cgroup/Bwrap hostility and
  source/worker cleanup, and native human grants for package/debug ops.
- Independent signed/unsigned release-security and legal review deferred;
  Group 4 Build/Test/LLDB remains separate.

**Reporting:** SHA, run/job IDs and original kernel markers; distinguish
`SOURCE` vs `NATIVE_KERNEL` vs `INSTALLED` vs `HUMAN`. A
green test-only example can never authorize an installed user workflow.
Stop after Group 3 for owner direction.

## First consolidated native Group 3 CI finding

[Run 42855571804](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42855571804),
source `384772af2535ffc6450008034b362217c19f3945`, failed **before any kernel witness**:
Windows ACL and both Windows SCM scenarios, plus Linux native supervisor,
called Cargo `--locked` where the independently developed standalone
`ide/supervisor` / `ide/sandbox` crates have no committed local
`Cargo.lock`. Cargo refused to create those test-only locks. The
negative human/physical source contract already passed on Linux.
This is a CI invocation defect, not permission denial or proven
AppContainer/SCM failure.

The corrected Group 3 source-only/native-OS test steps follow the same
(non-`--locked`) convention as the existing consolidated native
supervisor workflows. This does **not** change, remove or regenerate the
pinned frontend/Tauri/NSIS/Debian release graph, and does not grant code
execution in the installed IDE. A separate audited and committed Cargo
lock will be required if these test crates become distributed products.
One combined exact-head native group is required after this correction.

## Windows AppContainer source-owner defect and narrow repair

[First real Group 3 Windows run 42855823301](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42855823301),
exact source `d304bb8cddb549ba8f950be68043c450ee465789`: 100 genuine Windows supervisor tests **PASS**,
but test-only AppContainer example returned
`SUPERVISOR_OS_CONTAINMENT_NOT_VERIFIED` before printing any positive
kernel ACL receipts. The existing `inspect_staged_readonly_tree`
requires the *exact host TokenUser SID* as owner and a protected DACL.
On hosted administrator Windows accounts, freshly staged test files may
inherit the Administrators group as owner. The fixture set only the DACL,
not the owner. This is a real mismatch between fixture setup and its
mandatory native attestation; the original single generic error does
not by itself prove the precise failing step.

Repair: disposable positive fixture now resolves that exact reviewed
host TokenUser SID through native Win32, sets OWNER + protected DACL
together with `SetNamedSecurityInfoW` on its staged source, executable
and private output; fails if exact owner cannot be applied. The
independent source ACL observer, low-integrity child, denied
WRITE_DAC/WRITE_OWNER/read-only source, private output and Job checks
are unchanged. Fixed-category stage markers distinguish profile
creation, exact owner+DACL sealing, independent audit, native child,
with **no paths/secrets** in logs. Production installed app never
enables this test feature and `NoContainment` denial stays intact.

Require a new native Windows run to prove the result; otherwise keep
the AppContainer gate OPEN. Windows SCM service loss native observer
already passed at the first run, but separate host-crash and original
Job proof must be inspected before positive acceptance.
