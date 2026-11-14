# E1-P08 — Linux group guardian and real crash proof

## Purpose

The desktop's E1-P08 SQLite recovery only marks a previously running task
INTERRUPTED and an executing effect UNCERTAIN. Those transitions alone
prove **nothing** about a surviving Linux worker or grandchild. A Linux
`PR_SET_PDEATHSIG(SIGKILL)` protects only its immediate recipient, not
children that it has already spawned.

The new first-party `lu-linux-group-guard` keeps a separate, host-created
process group alive as its **leader**, while Git, FFprobe and their ordinary
descendants run in that group. Unlike a group whose original worker leader
has already exited, the guard's living kernel process-group identity does
not depend on resurrecting/storing a potentially reused PID after restart.

## Actual OS mechanism

1. Before a reviewed native launch, the Lu host starts the tiny guard from
   a fixed sibling executable, not a user/model-specified command. It
   starts with `process_group(0)` and verifies `getpid()==getpgrp()`.
2. Guard writes a fixed READY line to the host and blocks on a private
   lifetime pipe. Only Lu retains the writer. The actual worker is
   launched into the guard's real PGID and still receives the existing
   Linux parent-death signal, bounds and SHA-based reviewed admission.
3. On normal completion, explicit cancellation, timeout or Lu shutdown,
   the host closes the guard pipe. The guard sends `SIGKILL` to its
   **own** group (including itself). The host requires the kernel to
   report SIGKILL termination of the guard before treating group-wide
   signal delivery as verified; it separately `wait()`-reaps the worker
   that it actually owns.
4. On abnormal Lu `SIGKILL`/crash the host pipe closes without invoking
   any Rust Drop handler. The independent guardian executes the same
   kill-group path. No previous journal PID is used to kill a process.
5. A restarted Lu reconciles the old effect/task journals and annotates
   the original ANSF chat. It still **does not** claim the operation
   completed or auto-reexecute uncertain tool effects.

The guardian does not sandbox Lu Core. It supervises only reviewed child
application operations. No Docker, MCP, model-generated argv, or
user-supplied PID is involved.

## Critical limitations and deployment boundary

- The guardian protects ordinary descendants **that remain in its PGID**.
  A hostile application that escapes with `setsid()` or `setpgid()` is
  **not** contained by this technique. Strict cgroup-v2 custody, where
  genuinely delegated by Linux, would be a stronger future OS-level
  containment gate. Linux adopted zombies may be reaped by init, not Lu;
  do not equate group-wide SIGKILL delivery with `wait()`-reaping every
  descendant.
- Linux **release builds fail closed** for reviewed Git/FFprobe launches
  if the first-party sibling `lu-linux-group-guard` is not installed.
  The future Linux single installer must package it next to Lu's
  executable; this installation/deployment is still OPEN.
- Debug builds may run existing older tests without the sibling guard.
  The debug-only `LU_E1_P08_TEST_GUARD_BINARY` override is exclusively
  an explicit native CI fixture. It is not a production application
  preference, a WebView input or a model-facing tool. The destructive
  `lu-e1-p08-linux-crash-parent` fixture requires its dedicated Cargo
  `e1-p08-crash-fixture` feature and MUST NEVER ship in an installer.
- Program provenance and permission flags are checked by the host, but
  release binary identity, complete package staging, and independent
  installed-Linux acceptance remain open.

## Measurable tests submitted

`.github/workflows/lu-e1-p08-reviewed-recovery.yml` schedules:

- Exact-head Ubuntu 24.04 / Windows 2025 production ANSF chat history
  recovery tests, SQLite task/effect journal recovery and Tauri build.
- Native Ubuntu crash proof: compile two real first-party binaries, spawn
  the watchdog and a shell with a live sleep grandchild, preserve an
  unrelated control process, SIGKILL the Lu-like host, then inspect
  `/proc` states and require the guard, worker and grandchild to be
  non-running while the unrelated control remains alive. The test pins
  the guardian, worker and grandchild via Linux `pidfd_open()` before
  crash injection; failure cleanup uses only `pidfd_send_signal()` against
  those kernel identities, never a stale numeric PGID. Merely checking
  SQLite statuses cannot pass this test.
- A separate real supervised reviewed Git/FFprobe test with the staged
  guardian actively in the trusted worker launch path. The recorded
  helper/worker source and binary hashes must match the runner's files.

The test is source-submitted, **not yet a verified passing OS result**.
All original E1-P07 GGUF model selection and physical installed Windows
acceptance gates remain separate.

## E1-P08C decision, Nov 8

An actual Linux `setsid()` descendant survived the death of its original
process group in an independent local kernel reproduction. A real OS
negative-control is now required in CI using
`benchmarks/e1/e1_p08_setsid_escape_probe.py`. This conclusively limits
what the PGID guardian promises: ordinary in-group descendants only.
**Strong all-descendant termination for an escape-capable app requires
a delegated cgroup-v2 subtree and a verified `cgroup.kill`/empty-state
receipt.** This next kernel custody gate is OPEN, not implemented.
See `docs/LU_E1_P08_CGROUP_V2_ESCAPE_DECISION_20261108.md`.
