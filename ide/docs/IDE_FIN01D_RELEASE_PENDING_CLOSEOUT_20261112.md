# FIN-01D protected SCM release and cancellation reconciliation — source closeout register

**Branch:** `ide/fin01d-custody-closeout-20261112`, stacked on original IDE FIN branch at `32f3b6d9`.

**Scope:** FIN-01D hardening only. This is NOT approval to start a new worker,
release a debugger action, clear an original stale marker, or install a
runtime. FIN-02 and physical installed-human acceptance remain deferred.

## Protected original Job terminal protocol

- **Normal build/debug/LSP completion:** independently verify empty original
  Job and bounded output; fsync original `ReleasePending`; require original
  protected SCM service empty-Job/revoked-lease ACK; only then append a
  `Completed` or `Failed` terminal receipt. The Cargo path previously
  committed its terminal before receiving that release ACK.
- **Protected cancellation / dropped LSP or DAP:** fsync
  `CancelRequested` before kill. A verified full descendant reap moves
  only to `ReleasePending`. Independent SCM ACK is required before
  a distinct `Cancelled` receipt. Unverified kill remains `Uncertain`.
- **Failure / crash:** interrupted `ReleasePending` replays as quarantine;
  failed service release moves to `RecoveryRequired`; terminal journal
  write failure cannot be converted into success; no stopped session or
  original worker can be replayed on restart.
- **Trust boundaries:** these transitions are private to the original native
  supervisor and already-owned worker, not callable by WebView/model input.
  The existing global FIN-01D marker, durable journal, kernel custody,
  exact service identity, and human approval requirements are unchanged.

## Tests

- Rust supervisor tests cover interrupted release quarantine across restart,
  distinct terminal receipt, cancelled release still pending, and
  cancellation terminal intent/phase enforcement.
- LSP mock tests cover dropped actor with failed service release.
- Exact-head CI workflow: `.github/workflows/ide-fin01d-custody.yml`.
  It runs a complete Windows release-observer Rust suite and Linux
  DAP-enabled Rust suite. Passing source tests do **not** prove actual
  Win32 AppContainer/SCM or delegated Linux cgroup-v2 execution.

## Unresolved gates

1. Independently complete the original FIN-04R1 pinned LLVM18 native kernel
   workflow, obtain operation-specific failure diagnostics from the actual
   `--project-debug-protected-pdb` fixture, and repair demonstrated OS
   defects without removing quarantine.
2. At the exact new code head, run original Windows Job/SCM positive/negative
   release and cancellation, worker/guardian death and process descendants,
   service ACK loss/restart, terminal journal failure, and global-marker
   recovery; verify original hashes and OS provenance.
3. Run original Linux cgroup-v2/worker restart and fail-closed recovery on a
   genuinely delegated native host.
4. Consolidate with FIN-04 debugger, FIN-03/Group 6 protected Git and FIN-05
   package gates before FIN-06/FIN-07 acceptance.
5. Preserve deferred FIN-02 runtime packaging, physical installed-human
   NSIS/Debian approval, production debugger action disablement, unsigned
   release/signing review, and no merge to main.

This register describes source behavior and outstanding acceptance, never a
native kernel or human-consent success.
