# IDE-05.6 — Independent physical human approval/refusal acceptance

**Status:** OPEN — requires an actual operator and an independent observer on a
physical OS desktop. Source-level approval checks and synthetic CI approval
cannot satisfy this gate. No background automation or model-produced consent
is admissible.

## Preconditions

- Capture exact tested Git SHA, installed NSIS or Debian package digest,
  OS build, application PID, original license and runtime-bundle pins.
- Use a disposable registered project, a clean source worktree, a separately
  verified offline bundle, a known locked package snapshot and private
  output directory. No live user project may be used as the hostile fixture.
- The installed default-disabled Windows package worker must **remain
  disabled** until an independent native kernel ACL/MIC/SCM test passes.
  Do not bypass `SUPERVISOR_OS_CONTAINMENT_NOT_VERIFIED` to reach the dialog.
- Human approval must be performed through the genuine Tauri OS dialog, not
  React/WebView, a mocked callback, a CI `record_human_approval(true)` call,
  screen-coordinate automation or a synthetic programmatic accept event.

## Required independent operator/observer sequence

| Attempt | Real action | Required independently observed result |
| --- | --- | --- |
| 1: refusal | Request one exact prepared package job; independently record its Job ID, full JobPlan SHA, source HEAD, manifest/lock digest and bundled uv digest in the OS dialog; click Deny | No child process; no output; durable rejected/terminal receipt; no usable grant |
| 2: cancel | Request a **new** prepared job; cancel while the genuine native modal remains open | No native child; no authorization; journal reopens without replay authority |
| 3: timeout | Leave a new OS dialog untouched beyond its fixed 45-second timeout | No child, no grant; no silent yes/default approval |
| 4: stale source | Request a new plan, modify the disposable source/lock from a separate trusted OS process *after display but before clicking Approve*, then click Approve | Native revalidation denies; no worker, no stale grant or source change from Lu |
| 5: allowed one-shot | Restore a clean new exact plan, manually click Approve on that plan only | One durable fresh grant bound to Job ID, full plan digest and source; actual OS-contained child; verified installed output and terminal process-tree cleanup |
| 6: replay | Try to reuse the **same** Job ID and approved plan after terminal result | `SUPERVISOR_GRANT_CONSUMED` or equivalent fail-closed disposition; no second worker |
| 7: interruption | Authorize a fresh job, interrupt the installed app/worker, reopen and reconcile | Quarantine, OS cleanup before durable terminal, no automatic rerun or historical grant |
| 8: wrong scope | Attempt a valid-looking approval for another disposable project/source/recipe | Rejected by exact project/source/recipe binding, no worker |

## Evidence to retain and verify

The independent observer must correlate a genuine screen/session recording of
the installed app and OS dialog to a real process PID/Job/SCM or Linux cgroup,
the native supervisor receipt history, frozen source manifest hashes,
terminal output digests, and uninstall/restart evidence. Include an external
witness statement identifying each attempt, actual OS, and source SHA;
a model or CI cannot attest that a person really clicked the permission dialog.

The observer must redact real user paths, code and package secrets. Do not
print private project paths or source content in logs. The witness must record
why any case was skipped or failed, not count skipped cases as passed.

## Related independent Windows OS security closure

The disposable native ACL probe in
`ide/supervisor/examples/ide056_windows_acl_positive.rs` exercises fresh
AppContainer source read versus prohibited write/rename/delete and private
output write under Job custody. Even a successful probe does **not** prove
all kernel owner-rights, mandatory-integrity labels, reparse point attacks,
actual SCM escrow/recovery or installed Tauri consent. Those remain separate
positive hostile Windows acceptance prerequisites before removing the
production `NoContainment` early denial.

**Disposition:** This document is an acceptance contract, not a human witness.
Never mark Task 5 release-grade authorization complete until the physical
observations and independent receipts exist.
