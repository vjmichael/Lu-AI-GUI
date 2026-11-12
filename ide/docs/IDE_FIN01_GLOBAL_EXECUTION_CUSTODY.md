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
