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
