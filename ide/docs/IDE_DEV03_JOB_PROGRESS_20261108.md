# DEV-03.3 — bounded native job progress (source slice, November 8 2026)

## Implemented
The Jobs pane now projects native `ide_build_status` observations into a
bounded, read-only phase timeline on its existing 1-second polling cycle.
The host is still authoritative. No log lines, arbitrary UI output, source
text, environment, worker handles, grant tokens, or absolute paths appear
in this timeline.

`ide/app/web/job_progress.js` enforces the selected native project identity,
known fixed Cargo recipe names, bounded token IDs, known phases, a maximum
of eight recent jobs per poll, and at most 96 retained phase observations.
Repeated polls do not manufacture new events. An active job with an actual
native job ID is shown as running/cancel-requested; a job without an ID is
not assigned a fake one. Historical quarantine records do not contain
recipe identities and are therefore left in the existing separate
quarantine UI, without inferring or inventing a recipe.

The existing Tauri `ide_build_status` endpoint, protected workers,
admission dialogs, cancellation and terminal receipts were not altered.
This UI makes no assertion that execution succeeded or that descendant
process cleanup occurred.

## Still OPEN — DEV-03.3 real stdout/stderr

This first slice is **phase progress, not command-output streaming**.
Subsequent work must introduce a host-only incremental output transport
from the already-authorized bounded native worker. Requirements:
- worker-owned, bounded stdout/stderr collection with independent
  sequence/offset, stream identity, UTF-8 replacement policy and truncation;
- project/job/plan binding enforced at native readback (not UI filtering);
- no WebView-controlled file paths, subprocess commands or new permissions;
- distinct live-unverified versus sealed-terminal output, preserving
  cancellation and recovery boundaries;
- backpressure, no unbounded buffering, no secret-bearing unfiltered stderr
  assumed safe; restart must report interrupted and not replay invented bytes;
- real Windows AppContainer/Job/SCM and Linux Bubblewrap/cgroup-v2 process
  tests, race/cancel tests, and installed GUI clipboard/accessibility checks.

## Verification
`node --test ide/app/tests/job_progress.test.mjs` is the source-level
regression command. The GitHub workflow at
`.github/workflows/ide-dev03-job-progress-native.yml` checks the exact
branch head and the native host/Tauri test build on Windows/Linux.
Passing source checks alone must **not** close streaming output or installed
acceptance.
