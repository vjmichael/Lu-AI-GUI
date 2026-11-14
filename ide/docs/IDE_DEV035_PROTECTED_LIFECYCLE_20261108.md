# DEV-03.5 — Protected job lifecycle and installed-workflow evidence

## Implementation and scope (November 8, 2026)

Stacked on draft DEV-03.4 PR #161, itself stacked on DEV-03.3B
protected streaming PR #160. This is host-supervised workflow integration
and OS acceptance instrumentation, not a new process executor.

### Corrected native contract

`NativeBuildRuntime` previously emitted `format!("{:?}", phase).to_lowercase()`
for durable and terminal JobPhase labels, silently producing
`cancelrequested`, `recoveryrequired`, `admissionpending`, etc.
instead of the supervisor's actual snake_case receipt labels. DEV-03.5
now explicitly maps all twelve enum variants to their correct public
names. Native unit tests assert cancellation and recovery names.

Cancellation now requires the **exact current JobID** once native
preparation assigned one. Before that, `None` only cancels the same
in-memory preflight. An omitted ID cannot cancel a newly admitted job
between WebView polls. Duplicate requests are rejected. A request is
still NOT cancellation completion: the existing native supervisor must
durably fsync the request and verify the OS-owned process tree gone.

Recovery is only available for matching native journal entries with
`requires_reconciliation`; its existing reviewed Windows SCM or Linux
cgroup cleanup operation never resumes or reruns a job.

### Joined Jobs view

A pure, bounded UI projection reads only current native worker identity,
project-matched durable journal and quarantine entries, and exact
plan-matched in-process native terminal results. It separately reports
whether job-specific ephemeral untrusted output was observed or truncated.
It never reads untrusted project files as receipts, treats generated
output as kernel cleanup, restores lost live logs on restart, upgrades a
Completed journal entry into a fresh BuildRunView, or accepts an installed
test on the basis of UI evidence. A recovery_required entry always takes
priority over apparent process progress.

Tests cover requested-vs-verified cancellation; quarantine; durable
completed record after restart; exact current job/plan binding; forged
native result; output injection; closed/cross-project identity and
preflight without a durable receipt.

### Real installed application UX acceptance instrumentation

- **Windows NSIS**: The already-existing actual unsigned installation
  workflow runs an additional Windows UI Automation scenario. It opens
  the registered project, Jobs view, DEV-03.3B streaming panel,
  DEV-03.4 conventional file discovery and DEV-03.5 lifecycle panel;
  invokes read-only native discovery; checks the source file's SHA-256;
  rejects a worker appearing or the protected Run button becoming enabled
  in an unpinned unsigned installation. It does not simulate approval.
- **Linux Debian**: The existing actual installed `.deb` GTK/X11
  workflow runs a new AT-SPI scenario *before* the installation is
  removed. The scenario uses registered project reopen, real
  accessibility actions, read-only discovery, source SHA and disabled
  protected execution. It never uses internal Tauri IPC or WebView JS.

The separate **real protected worker** OS lanes remain authoritative:
WO6 Linux delegated cgroup-v2/Bubblewrap Cargo check/test/cancel
fixtures and D7B/D7D Windows AppContainer/Job/SCM protected fixtures
are already wired to assert actual worker stdout/stderr under the
previous DEV-03.3B work. Their exact-head final results MUST be
inspected rather than inferred from this UI source work.

### Acceptance matrix (all independently required)

| Gate | Scope | Current acceptance |
|---|---|---|
| DEV035-SOURCE | Windows/Linux native Rust/Node/Tauri regression | OPEN until exact-head CI |
| DEV035-LINUX-WORKER | Real cgroup/Bubblewrap Cargo test/stream/cancel/restart receipt | OPEN pending OS run |
| DEV035-WIN-WORKER | Real AppContainer handles, streaming, Job Object cancellation, SCM recovery | OPEN pending OS run |
| DEV035-WIN-UI | Actual installed NSIS Jobs UI/read-only discovery and disabled unpinned Run | OPEN pending installed workflow |
| DEV035-LINUX-UI | Actual installed Debian GTK Jobs/read-only discovery and disabled unpinned Run | OPEN pending installed workflow |
| DEV035-FULL-CHAIN | Signed/pinned installed app with fresh approval, actual streamed build, cancellation, kernel recovery and restart | OPEN; cannot be substituted by unsigned smoke |
| DEV035-DISCOVERY | Comprehensive dynamic framework-specific test enumeration | OUT OF SCOPE; DEV03.4 returns bounded filename hints |

No Docker, uncontained fallback, new grant authority, log-derived success
receipt or silent protected-job restart is allowed.

### Failed-job output visibility
The native build status now includes only `lastOutputJobId`, restricted to
its registered project and existing in-memory last-job window. The Jobs UI
selects that ID only when an exact project + job + plan digest exists in the
native durable journal. The native read command still rechecks active/last
runtime identity and has no path argument. Thus a denied, failed, cancelled
or quarantined worker's bounded advisory text can remain inspectable in the
same process after the active worker disappears. After restart this pointer
and all buffered text are gone; only durable receipts survive. Neither output
nor its presence permits rerun or implies successful cleanup.
