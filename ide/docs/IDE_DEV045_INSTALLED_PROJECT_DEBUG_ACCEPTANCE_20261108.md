# DEV-04.5 — Installed protected project debugger acceptance and native repairs

**Status: PARTIAL IMPLEMENTATION / NOT ACCEPTED.** This task is stacked on DEV-04.4
PR #167 (exact head inspected: `111fd3cd80b1dae9d2270db83c07f0f18856c193`).
The current DEV-04.5 branch is
`ide/dev04-5-installed-debugger-repair-20261108`.
Do not describe the unsigned default installer, structural debug symbols, or
source-only tests as a working installed debugger.

## Findings at the inspected DEV-04.4 exact head

- Windows worker-containment workflow #42554629921: native Job Object
  probes passed, then `cargo fmt --check` failed.
- Windows native production worker workflow #42554630096: build and supervisor
  tests passed, then supervisor `cargo fmt --check` failed. This is not
  acceptance for the project-debugger code path.
- Windows AppContainer workflow #42554630112: native AppContainer zero-rights
  probe passed, then sandbox `cargo fmt --check` failed.
- Linux 26.04 installed GTK supplemental workflow #42554630143: stopped
  before Debian packaging at eight source-contract test failures. Causes
  include stale 46-operation expectations (actual typed capability schema: 49),
  an outdated separate `rust_build` arm assertion (Cargo uses a reviewed
  shared dispatch), a test scope accidentally including the B11 merge apply,
  a comment text accidentally matching `await`, and incomplete explicit
  WebView command and parameter-schema assertions.
- The dedicated DEV-04.4 source matrix, delegated Linux project-debugging
  and Windows protected project-debugging workflows were still queued at
  inspection. No full end-to-end native OS verdict may be inferred.

## Repairs and new installed denial test

1. Updated source-contract tests, preserving the explicit reviewed-command
   allowlist and fixed offline Rust build command. Corrected editor read-only
   audit function scoping and Rust Tauri argument parsing in the schema test.
2. Added `Dev045ProjectDebuggerDenied` to the **real installed Windows
   UI Automation** fixture and the existing unsigned NSIS installed smoke
   workflow. It opens the actual Jobs UI, checks that the new project-debugger
   control exists but is disabled and reports absent release support, verifies
   no Lu recovery service is present, observes protected worker count before
   and after, and independently hashes the project source.
3. This *negative* test must run in the real installed default desktop; it
   does not grant the unsigned build native debugger rights or synthesize
   a success from a JSX contract. A test source/CI pass does not count as
   physical desktop acceptance until the exact-head Windows result completes.
4. Supervisor and sandbox formatting failures must be repaired and all
   affected exact-head Windows/Linux jobs rerun. A green source-only matrix
   does not bypass the OS jobs.

## Positive project-debugger installed workflow — OPEN

Acceptance requires the following **real** scenario on each supported OS:

- Build an offline first-party Cargo project through the existing native
  protected worker; observe kernel/Job custody, durable Completed zero-exit
  original build receipt, source/manifest/toolchain/output SHA identity,
  independently verified one-and-only executable and DWARF or PE/PDB witness.
- Restart or reopen the installed IDE and prove that a stale build, copied
  witness, or expired owner *cannot* create an executable target or replay
  a prior approval.
- From a fresh project/source/review, stage the exact hashed executable and
  its exact symbols in an exclusive, separately owned native-only directory.
  Detect modification, symlink/reparse-point and hard-link substitution,
  duplicate and cross-project receipts, and ambiguous binaries.
- Show the **native OS dialog** with a new one-use approval distinct from
  build authorization, then run real LLDB-DAP only under the existing
  AppContainer/Job/SCM or delegated cgroup-v2/Bubblewrap supervisor.
  Verify a real stop, thread and stack; do not infer from a green UI label.
- Require source-relative line/address evidence before enabling source-line
  breakpoints, stepping or variables. Those features remain disabled here.
- Cancel during a live native debug session; verify bounded stdout/stderr,
  kernel-descendant tree reaping, terminal durable receipt, no old token
  reuse, no project source change, and no orphan target stage.
- Crash/restart the host and its delegated service independently; verify the
  journal and real process custody, quarantine unknown cleanup, reconcile
  only with appropriate OS authority, and never auto-replay execution.
- Install, run real UI Automation, then uninstall and audit leftover
  processes, service credentials, private stages, bundle bytes, receipts,
  and license/signature provenance. Test-only unsigned fixtures do not
  establish a distributable or signed release.

## Technical release boundary

`native-project-debug` remains opt-in and requires `native-build`
and `native-debug`, reviewed offline LLDB/runtime hashes, Windows SCM
registration or Linux delegated custody, human approval and production
license/signing review. The default installer must remain non-executing.
NEVER remove these gates to green a CI run or silently switch to an
uncontained local debugger. Release/legal approvals may be completed later;
technical acceptance and legal approval remain different decisions.

## Remaining task order

**DEV-04.5A** — exact-head Rust formatting repair and source/test rerun.

**DEV-04.5B** — completed, independently inspected Linux delegated real Cargo
build → project LLDB and Windows SCM/AppContainer project-debug cases.

**DEV-04.5C** — reviewed pinned debug bundle, technically test-only installed
native flow with live stop/cancel/crash/restart/uninstall receipts.

**DEV-04.5D** — source-line/DWARF/PDB semantic debugger compatibility tests,
followed later by source breakpoints and stepping under the same custody.

Do not close DEV-04.5 until real exact-head OS evidence, not only checked-in
assertions or queued CI, supports each technical claim.
