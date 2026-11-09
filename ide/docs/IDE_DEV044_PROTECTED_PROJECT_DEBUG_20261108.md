# DEV-04.4 — Exact project build → protected LLDB-DAP

**Status:** Source implementation and real Linux native OS acceptance fixture
committed on `ide/dev04-4-protected-project-debug-20261108`, stacked on
draft DEV-04.3 PR #165. Native Windows/Linux and installed acceptance are
**OPEN until the exact-head jobs pass**.

## Existing trusted components retained

This does **not** add a second process launcher or an unconfined "debugger
shell." It extends the existing WO6 debugger supervisor, D3 one-use grants,
native Job Object/AppContainer/SCM and Linux delegated cgroup-v2/Bubblewrap
process custody. Fixed LLDB self-test remains independently available. The
default desktop has no project debugger authority.

A **separate opt-in** `native-project-debug` requires **both**
`native-build` and `native-debug`. It is absent from default NSIS and Debian
installer builds. The existing independently reviewed LLDB adapter/service
pins and native human OS approval are still mandatory; the feature flag
alone never starts a child.

## Exact binary custody and D3 authorization

1. The registered project is clean, has a valid committed Git HEAD, and has
   completed a real native, zero-exit, unquarantined Cargo `rust_build`
   under its independently recorded native OS custody.
2. The host observes the exact build job ID/plan hash/terminal receipt,
   immutable frozen source and manifest hashes, toolchain digest, fully
   hashed output tree, the unique binary SHA and structurally matched
   DWARF or PE/PDB symbol SHA. No WebView path or artifact can be selected.
3. The host copies the exact independently rehashed executable (and PDB
   if required) into a **private, exclusive target directory**, rehashes
   the new bytes, and keeps that target only under native in-process
   custody. No persisted GUI record can revive it after restart.
4. A domain-separated target binding SHA-256 includes source, original
   build job/plan, terminal receipt, toolchain, output, binary and symbol
   evidence. New D3 job recipe `dap_lldb_project` requires the binding
   digest. It is included in the D3 hash-linked plan and single-use grant.
   Other recipes reject any target binding; historical plans serialize
   their absent optional field identically.
5. The host re-publishes and verifies the current source snapshot,
   checks Git identity and the sealed target before presenting a **fresh
   native OS approval dialog** identifying the exact build receipt and
   target hashes. Previous Cargo approval and fixed debugger approval
   do not authorize the new debugger job.
6. The new native DAP worker verifies the matching plan/target binding
   and rehashes the target *before grant consumption*. Linux mounts the
   private target read-only at `/debug-target` within the same
   unshared PID/mount/network namespace and pre-exec attached cgroup.
   Windows copies/rechecks the bounded target tree inside the
   separately staged AppContainer LocalState, after its independent
   SCM/Job Object custody and resource quota checks.
7. Only that native staged target can form a no-args, no-env,
   `runInTerminal:false`, `stopOnEntry:true` LLDB-DAP launch request.
   No arbitrary path/attach/PID, evaluate, expression, terminal,
   breakpoint scripting or detached user child is exposed.
8. Existing DAP gateway verifies native initialized/stop/threads/stack,
   then requires process-tree kill, Job/SCM or cgroup release and
   fsynced terminal receipt. Failure or host crash remains quarantined
   until separately approved OS-only reconciliation; it is never
   silently rerun. Debugger cancellation is now exact-JobID, one-use,
   and visible with canonical durable snake_case phase labels.

The resulting native `DebugWitness` carries a **different** project debugger
JobID/plan SHA, originating build JobID/plan/receipt, binary SHA, symbol SHA
and target binding SHA. UI validation rejects mismatched, stale,
cross-project, incomplete or fixed-fixture results. This witness is never a
signing or installed-application certificate.

## Native tests

- D3 hostile test: only `dap_lldb_project` accepts a 64-hex target
  binding; changing a binding changes the actual one-use hashed plan;
  Cargo and the fixed debugger reject an injected binding.
- DAP hostile test: only the native isolated staged target basename
  can be sent in a project `launch` frame, with no user args,
  environment or terminal. Path traversal and duplicate launch denied.
- Native ELF/DWARF and PE/PDB stage tests: exact private copy succeeds
  only with matching native output/receipt/format/symbol evidence.
  Later byte/PDB tampering denies and private stage is retired on drop.
- JS hostile test: only exact build receipt and binary/symbol lineage
  with a distinct fresh DAP job and verified OS cleanup is shown.
- Genuine Linux WO6 CI: fixed `--project-debug` now runs **actual protected
  Cargo build**, checks its real ELF/DWARF, copies and rehashes its
  target, obtains an independent D3 debugger approval, performs native
  LLDB initialization, stop/threads/stack, verifies cgroup cleanup and
  reopens both native journal entries. It runs under the same
  systemd-delegated, root-owned LLDB and Bubblewrap policy.
- Exact-head source matrix:
  `.github/workflows/ide-dev04-4-native-project-dap.yml`, testing
  frontend, DAP host, supervisor and `native-project-debug` Windows/Linux
  Tauri compilation. Source checks are **not** runtime acceptance.

## Remaining blockers — no release claim

- **Linux actual OS**: require completed, passing delegated genuine Cargo
  build → project LLDB stop/stack/cancel/restart evidence and rechecked
  cgroup/kernel receipts; repair failures as they arise.
- **Windows actual OS**: require a full D7D authenticated SCM/AppContainer
  *project* build → PE/PDB → real project LLDB launch normal/cancel/crash
  acceptance. The native Windows stage/launch path and compilation source
  tests alone do not satisfy this. Test-only measured binaries are not
  accepted release signatures.
- **Installed application**: signed/pinned reviewed offline LLDB runtime,
  separately approved Windows LocalService and Linux root-owned bundle,
  installation and uninstall, native consent via installed GUI, source
  editor→protected build→debugger workflow, cancellation and restart.
- **Debug features**: exact source line-to-address mapping, verified
  conditional-free breakpoints, stepping, variable visibility and
  symbol/loader compatibility beyond structural DWARF/PDB identity.
- **Custody hardening**: local-account same-user stage mutation,
  Windows hardlink/reparse races, source/output handle-based publication,
  stale private-stage cleanup and hostile reverse DAP requests require
  independently reviewed OS tests; never weaken confinement to pass.
- **Legal/release**: signing identity and third-party adapter license
  approvals remain independently open.

Next follow-on group **DEV-04.5**: genuine installed project-debugger workflow
acceptance and repairs, followed by breakpoint/step/source mapping only
after protected worker execution is established.
