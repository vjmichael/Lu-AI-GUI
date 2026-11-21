# DEV-04.6A — Host-only debugger command review and consolidated CI

Status: source implementation only, NOT native or installed acceptance. This
work follows draft PR #192 (D5 real Windows LLVM event ownership).

The host can construct a private review session only through the original
native D5 owner-event witness under its explicit CI-only feature. Each
review binds Job ID, debugger plan SHA, original Completed Cargo build SHA,
debugger Running receipt, separate fsynced owner-event SHA, approved source
snapshot, exact source line, Job-owned debuggee PID/TID/RIP and the current
owner event sequence.

Reviewable intents are limited to the same independently checked source
line breakpoint and steps Over, Into or Out on the exact stopped thread.
Unverified frames/variables, arbitrary source lines, expressions and
other threads are explicitly denied. One review at a time, eight maximum
per stop. An event change, cancellation, crash, source mutation, restart
or ownership loss revokes the in-memory state.

The review is a hash-bound PREVIEW ONLY: executable_authority=false,
installed_app_accepted=false. No DAP setBreakpoints, next, stepIn, stepOut,
scopes, variables or evaluate are dispatched; no new Tauri/AI action, human
grant, or worker capability is added. Later D6B must add a separate
operation-specific native human consent and verified result receipt only
after D5 exact-head native OS and installed acceptance passes.

## CI batching policy adopted November 9, 2026

Develop a coherent task group on a working branch without opening a PR
for each minor implementation task. At the task-group boundary, submit
ONE consolidated exact-head Windows/Linux native verification batch
covering related compilation, hostile-unit and real OS custody gates,
plus the required installed acceptance artifacts. Avoid redundant,
per-commit CI matrices and rerunning queued older heads. Source reviews
cannot close OS security gates. Pending GitHub runs are not passes.
This D6A work remains on its unreviewed working branch pending the
broader D6 debugger admission group before that consolidated batch.

All D5 native Windows compilation, stop/continue, crash/cancel and signed
installed acceptance remain OPEN, with no expansion of control authority.

## Real D5 worker integration for the grouped acceptance batch

The already existing real protected Windows owner-event test now also
invokes the D6A review gate after it authenticates the native event and
before normal continuation/cancellation/crash. It confirms a same-line
breakpoint and same-thread step can be previewed, their hashes differ,
both remain non-executing, frame variables are denied, and simulated
owner-event sequence change invalidates outstanding review state.

This is a test integration only; it does NOT dispatch a breakpoint,
step, scopes, variables or evaluate request. The existing exact-head
native D5 OS workflow will exercise this when it is included in the
later consolidated Windows/Linux batch; no small CI workflow was added.

## FIN-04F — Release-safe read-only native debugger-control evidence (November 12, 2026)

This source increment **does not enable** production LLDB control. It extracts
an independent *read-only* `debug_control_evidence` verifier from the
developer-only D6B/D6C action-admission path. The original
`debug_action_receipts` and `debug_control_dispatch` remain feature-gated;
the new module cannot create `NativeConsentMarker`, `CiControlSlot`,
one-use claims, OS workers, DAP frames or live owner events.

The module replays the original `debug-fixture-receipts.jsonl` supervisor
state, hashes its original bytes before and after the scan, and verifies
each private `lu-debug-action-<sha>.ndjson` action journal:
- 32 KiB max per action, 12 events, 4,096 action files, 32 visible
  selected-project results; reject tampered/replaced, overlarge and
  symlinked records, invalid ordinal or hash-chain links, unknown phase
  transitions and changed origin Job/plan/operation/owner identity
- correlate **every** action to the original debugger Job/plan before
  disclosing only the selected project's records (foreign corrupt data
  blocks the scan without leaking foreign job identifiers or counts)
- report `Prepared`, `Approved`, `Consumed` as requiring independent
  OS-only reconciliation; `ResultVerified` remains a historical recorded
  claim, **never** a fresh kernel proof or available execution grant
- refuse original job-journal changes during evidence inspection

The native Tauri endpoint `ide_debug_control_evidence` accepts only
the registered project ID. `ide_debug_review_control_candidate` accepts
an exact previously completed debug Job and plan hash, plus one of five
bounded intent labels: `step_over`, `step_into`, `step_out`,
`breakpoint_at_reviewed_line`, `inspect_reviewed_frame`. The native
backend invokes the existing independently rechecked saved-source
review and original action-audit reader. It binds the original terminal
receipt, source SHA, reviewed line and adapter frame ID to a digest,
while clearly marking missing live owner, independent instruction/source
mapping, per-operation consent, installed control acceptance, and
FIN-01D kernel admission as **blockers**.

The IDE shows two read-only panels. No arbitrary source path, debugger
PID, expression, command, memory reference, DAP token, app-supplied
kernel witness, or executable action is passed to the native host.

New Rust fixtures cover original action hash chains, foreign-project
redaction/corruption, historical verified results as non-authority,
and no replay. JavaScript contract fixtures verify that release-safe
review cannot import or call the developer-only control dispatcher.
These fixtures are **committed but have not yet been executed**.

**Still open:** FIN-01D's actual native Windows/Linux kernel-custody
acceptance; release-grade independently held LLDB event-owner stop,
fresh native approval/denial and one-shot claim consumption; genuine
step/breakpoint/variables results with post-command OS/PDB verification;
interrupted owner-event recovery; installed UI, accessibility and
restart acceptance; multi-framework exact-case execution.

**FIN-02 remains deferred. FIN-03 destructive mutations and FIN-05
package execution remain disabled. No release/merge or CI batch.**

## FIN-04G — Global original-action recovery admission and evidence identity

This source increment fixes an admission defect: a release/native debugger
must not infer clean custody from terminal worker receipts while a separate
D6 native debugger-action receipt remains `Prepared`, `Approved`,
`Consumed`, or `ResultUncertain`. Previously this check was reachable
only under the debug-only D6D feature gate.

The **release-safe**, read-only `debug_control_evidence` inspector now:
- Replays the original on-disk `debug-fixture-receipts.jsonl` supervisor
  independently of its in-memory owner and rejects stale/desynchronized
  supervisor state, with a special bound pristine-empty profile case.
- Hashes the original job journal and the **full** bounded private action
  journal directory both before and after validation; changed, replaced,
  newly inserted, removed, corrupt or foreign action files refuse the scan.
- Replays all original action journal chains and correlations before
  selected-project filtering. No foreign project identity/count leaks to
  the renderer. Returns only a privacy-safe global unresolved-action flag.
- Separates `global_os_cleanup_required` (native process/worker uncertainty)
  from `global_unresolved_action_required` (unfinished control or uncertain
  result). A `ResultUncertain` row requires independent result reconciliation,
  not a claim that another kernel process is necessarily still running.
- Distinguishes a legitimate active native Job from an orphaned action in
  the operator display. Both block unsafe parallel admission, but recovery
  messaging is not conflated.

Every native-debug build, including production profiles, checks this
independent action evidence in `status()` and `run_bound()`. The new
debugger launch checks it *again under the profile-wide FIN-01 global
marker*, before `Supervisor::prepare`. Refusal before preparation
releases only the current invocation's unchanged original journal
marker; it does not touch earlier stale markers or assert OS cleanup.

Read-only debugger-control candidates now bind the original supervisor
journal SHA-256 and complete action manifest SHA-256 alongside the source,
terminal debugger receipt and operation intent. A second exact original
stopped-source review after scanning denies changed source/terminal evidence.
Review digest changes cannot mint an execution grant or resurrect an old stop.

Added native Rust fixtures for cross-project unfinished action blocking,
original on-disk journal advancing past in-memory state, byte-level action
mutation, unchanged manifest, terminal no-action distinction and uncertain
result classification. Updated JavaScript source-contract tests and the
operator's profile-wide recovery messaging.

**Still blocked:** genuine FIN-01D native Windows/Linux kernel-custody
acceptance, one-use release operator action approval, independently
verified OS event-owner/PDB/RIP source mappings and post-command results,
OS-only recovery for interrupted native control actions, installed
Windows/Linux debugging UX, and multi-framework exact-case execution.

**No production DAP step/breakpoint/variable executor was enabled.**
FIN-02 remains deferred; FIN-03 destructive and FIN-05 package executors
remain gated. Rust/Node source fixtures were committed but not run,
and no compiler, CI or installed-app acceptance is claimed.
