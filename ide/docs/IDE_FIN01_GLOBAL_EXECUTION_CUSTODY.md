# FIN-01 — Global native execution state, project isolation and bounded observations

**Source implemented**, not native/installed accepted. Actual native admission continues to run only in existing OS-supervised workers. FIN-01 is about accurate human reporting, not a new grant.

The native BuildStatus now publishes three booleans computed from the complete original supervisor state: `globalRecoveryRequired` (all durable journal entries), `activeInAnotherProject`, and `globalWorkerBusy` (process-local current job). It never publishes foreign project details/paths/output. Selected-project journals remain bounded; the global recovery decision is not.

Protected profile selection now requires all three native fields to be explicitly false, and INT05 inventory/INT06 Workbench refuse missing status. Recovery or another project's busy worker cannot appear available in UI even if native runtime pins match. The original native supervisor still independently enforces actual OS custody and refusal.

INT06's seven read-only probes receive a 12-second per-read presentation deadline and `Promise.allSettled` partial results. Timeout is **not** cancellation of an OS read, nor proof that its host operation finished. Any unavailable field stays unverified. A project switch invalidates all pending results.

Acceptance pending: native source compilation, Windows/Linux OS supervisor/cgroup/Job isolation, concurrent project switching, restart/recovery and installed GUI. No CI/PR/physical tests executed.
