# FIN-01 — Global native execution state, project isolation and bounded observations

**Source implemented**, not native/installed accepted. Actual native admission continues to run only in existing OS-supervised workers. FIN-01 is about accurate human reporting, not a new grant.

The native BuildStatus now publishes three booleans computed from the complete original supervisor state: `globalRecoveryRequired` (all durable journal entries), `activeInAnotherProject`, and `globalWorkerBusy` (process-local current job). It never publishes foreign project details/paths/output. Selected-project journals remain bounded; the global recovery decision is not.

Protected profile selection now requires all three native fields to be explicitly false, and INT05 inventory/INT06 Workbench refuse missing status. Recovery or another project's busy worker cannot appear available in UI even if native runtime pins match. The original native supervisor still independently enforces actual OS custody and refusal.

INT06's seven read-only probes receive a 12-second per-read presentation deadline and `Promise.allSettled` partial results. Timeout is **not** cancellation of an OS read, nor proof that its host operation finished. Any unavailable field stays unverified. A project switch invalidates all pending results.

Acceptance pending: native source compilation, Windows/Linux OS supervisor/cgroup/Job isolation, concurrent project switching, restart/recovery and installed GUI. No CI/PR/physical tests executed.

## FIN-01C — Cross-supervisor and cross-process admission fencing

The desktop's *build/package* and *debugger* hosts now use a single,
profile-rooted `fin01-global-native-custody-v1.lock`. The owning native
host creates the marker with atomic `create_new` before reserving an
in-memory worker slot, preparing an original JobPlan, requesting OS
approval or launching an OS-contained worker. The marker is written
and synced; on Linux its parent directory is synced as well. The
marker and OS processes remain separate trust domains.

Admission replays **both** durable supervisor journals and denies any
unreconciled, nonterminal or quarantined job even when the desktop
has no surviving active worker handle. Both BuildStatus and
DebugStatus read the global marker and both journals and fail closed
on missing/corrupt evidence. Project-scoped views do not disclose
foreign job metadata.

The owner releases the marker only after replaying both original
journals and finding every job terminal with no pending reconciliation.
The marker is not unlinked in Drop or automatically after a crash,
timeout, cancellation, failed journal read, interrupted commit or
unknown OS release. An unexplained marker requires manual OS custody
and original-journal reconciliation. This intentionally may prevent
new jobs after a crash; deleting the marker on PID age alone would
introduce a second unauthorized supervisor owner.

Native source tests cover contention, stale marker, malformed owner,
foreign journal custody, and negative terminal release. **Not yet
completed:** a separate operator-approved, independently OS-attested
restart recovery route for the shared marker; native Windows/Linux
concurrent-process races; exact-head compilation; actual installed
service/cgroup process custody; native owner acceptance. No CI or
installed acceptance was performed. FIN-02 remains deferred and no
destructive Git or package executor was promoted.

## FIN-01D — Unified build/debug/LSP admission and recovery (source increment)

- The shared `fin01-global-native-custody-v1.lock` now serializes the
  **build/package, debugger and LSP** native supervisors. All admission
  paths independently replay the other two original journals, not just
  their own current-process state. An active LSP session retains its
  original marker until independently supervised close and terminal
  journal validation. This deliberately prohibits a concurrent build
  while a protected LSP is live, pending a separately reviewed safe
  multi-lease design; it is not a background resource scheduler.
- An otherwise clean-looking previous LSP job that remains nonterminal,
  uncertain or quarantined blocks build/debug re-admission. Global
  recovery assessment now counts all three original journals and never
  discloses foreign project/job details.
- Denied build/debug/LSP admissions preflight original journals
  **before creating** the global marker. After acquiring, a refusal
  before `Supervisor::prepare` may release its own marker only when the
  complete original owner-job inventory and a bounded SHA-256 of the
  underlying owner-journal bytes remain unchanged. An admission that
  reached preparation, a changed journal, a missing owner or unknown OS
  cleanup remains quarantined. There is no automatic `Drop` unlink.
- A current LSP session must revalidate its exact private marker
  identity before further diagnostics, document updates, hover or
  completion. Replacement/loss is treated as uncertain custody, not
  continued permission. `stop` and OS-only reconciliation remain
  available for cleanup.
- Added a project-scoped LSP recovery inventory, a separately
  approved native OS-only exact-job cleanup command through the
  original Windows protected recovery service or Linux delegated
  cgroup, and a visible human entrypoint in the IDE. No project code
  or language server is restarted by this command. The global
  marker is **not** cleared after restart merely because journals
  appear terminal; an independently OS-attested stale-owner clearance
  mechanism is still required.

**Validation limits:** New Rust and Node hostile fixtures are checked
in but not executed. No exact-head Windows/Linux compilation,
physical OS admission race, kernel-descendant probe, native dialog
or installed application acceptance was run in this pass. FIN-02
runtime provisioning/packaging remains deferred; no new destructive
Git or package execution was enabled. FIN-01D remains open for
stale-marker OS proof and native acceptance, not a completed
production authorization contract.

### FIN-01D launch refusal / one-use admission repair

Native build, debugger and language-server hosts now reconcile an error from
*their own current launch* against its original job ID. Only a job whose
record remains `Prepared` or `Authorized` can transition through the
existing `Supervisor::cancel_before_start` API, with a durable
`no_worker_was_started` receipt. `AdmissionPending`, `Running`,
`ReleasePending`, `RecoveryRequired` and `Uncertain` never use that
path: they remain under original OS-proven termination/quarantine. LSP
tracks whether the marker and job were created **by the current
invocation** so a rejected second Start cannot cancel or release a
previous approved session.

The no-worker marker-release path additionally compares complete
job identities and the bounded original-journal SHA-256 captured
during admission. A changed journal, stale marker, lost owner, or
unverified OS release always denies marker deletion. A native
Windows/Linux interruption matrix and a separately operator-approved
restart marker clearance with independently observed kernel custody
are still outstanding. Source tests are committed but unexecuted.

## FIN-01D kernel-attested stale-marker clearance (source integrated)

The separate native Tauri command `ide_fin01_clear_stale_marker` requires
a new OS-level dialog. The WebView sends **no PID, source path, asserted
OS witness, release flag or approval boolean**. The host acquires a
cross-process recovery sidecar; reads the exact private global marker;
replays all three original supervisor journals; refuses any nonterminal
or quarantined original job; and computes full journal byte digests.

A native host-only OS function then checks the original marker owner's
actual kernel process state and all known protected worker identities:

- **Windows:** `OpenProcess` / `WaitForSingleObject` must show the old
  owner exited. For every original job, `OpenJobObjectW` must expose a
  Job Object with zero active processes or an absent exact named object.
  The protected workers use Job kill-on-last-handle semantics. Access
  denied, a live process, a corrupt/duplicate job identity or a busy
  object fails closed.
- **Linux:** the native host must actually belong to the restricted
  `lu-ide-host` leaf of a delegated kernel cgroup-v2 hierarchy.
  The former owner must be absent from `/proc`. The delegated parent
  must contain no directly attached processes and no unknown child
  cgroups. Every original `lu-ide-job-...` group still present must
  report `populated 0`, empty `cgroup.procs` and `pids.current=0`.
  Ordinary desktop/container hosts without genuine delegation cannot
  pass as protected native hosts.

The host rechecks the original journal digests and exact marker identity,
fsyncs a host-produced `fin01-global-clearance-evidence-v1.jsonl`
entry, repeats the OS kernel and journal checks, then removes the marker
and fsyncs the containing directory. This is a clearance of **stale
serialization custody only**, not an execution grant or automatic retry.

A separate recovery sidecar is mutually exclusive across processes.
An abandoned sidecar may be reaped only after native kernel proof that
its own recorded process exited while the original marker still exists.
A corrupt sidecar or unavailable kernel is a quarantine, not a timer
based cleanup.

### Required native acceptance evidence — not yet obtained

- **Windows:** original exact-head compiled/installed protected service;
  actual named Job handles, active-process and zero-descendant observations;
  owner crash with genuine Job cleanup, refused live-owner clear, native
  physical approval/refusal, stale-marker clearance readback and no new
  worker without a fresh permission.
- **Linux:** an actually delegated systemd/cgroup-v2 host, real worker
  descendants and `cgroup.events` / `cgroup.procs` / `pids.current`
  witnesses; denial for populated and unknown worker cgroups, confirmed
  owner death, then original-journal and global-marker recovery.
- **Both:** after-approval journal append, corrupted/audit write refusal,
  two competing recovery hosts, marker replacement, PID reuse/conservative
  denial, interruption during receipt fsync and after unlink, restart,
  cancellation races, foreign project isolation and native accessibility.

No CI, native Windows/Linux process test, actual installed application
test, or physical-human witness has been run for this source increment.
The new OS-verifier functions are a way to **obtain** genuine evidence on
the correct hosts, not evidence that those hosts were exercised.

**Release safety:** FIN-01D native acceptance remains open. FIN-02
provisioning is still deferred. FIN-03 destructive stash/restore and
FIN-05 vcpkg/Conan production execution remain disabled.
