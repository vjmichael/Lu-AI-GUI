# E1-P08 — reviewed external tool turn restart reconciliation

## Why this phase exists

E1-P07 delegates reviewed Git and FFprobe execution through the real Lu
native model-tool bridge. Each authenticated turn creates host-controlled
`lu-reviewed-task-g-<32 lowercase hex>.sqlite` and
`lu-reviewed-effects-g-<32 lowercase hex>.sqlite` stores in Lu's private
application profile. Before E1-P08, desktop startup only reconciled the
background worker database and a separate global effect journal, not these
per-turn stores. A dead Lu process could therefore leave durable
`RUNNING` and `EXECUTING` entries that appeared current after restart.

## Implemented behavior (source candidate, not acceptance proof)

1. On startup, **after** acquiring the existing exclusive
   `DesktopOwner` SQLite writer lease and **before** making tools available,
   the host scans only the trusted application profile directory.
2. A bounded scan (maximum 4,096 entries) admits only exact host-minted
   turn IDs. It refuses malformed reviewed-journal names, symlinked files,
   and Windows reparse-point journal files; SQLite WAL/SHM/journal sidecars
   are examined for aliases but never independently opened.
3. Existing task journals call `TaskStore::reconcile_interrupted()`.
   Only `RUNNING` entries become durable `INTERRUPTED` records.
4. Existing effect journals call `EffectJournal::reconcile_interrupted()`.
   Only `EXECUTING` effects become durable `UNCERTAIN`. Verified
   `APPLIED` receipts remain untouched. No effect or model request is
   retried automatically.
5. If Lu died between creating task and effect databases, the surviving
   journal is recovered without fabricating the missing database.
6. The native host carries exact, host-minted turn identities to the
   authoritative ANSF ChatStore. It verifies the original authenticated
   user-turn ownership, consults the indexed terminal/notice event state,
   and appends a durable *interrupted* assistant display checkpoint to the
   correct conversation only. Completed/cancelled turns are not rewritten.
   Multiple restarts never produce duplicate interruption notices.
7. The Send path also creates a small indexed queued marker before its
   per-turn worker SQLite journals. Startup recovers orphaned queued
   markers without reading an entire transcript into memory, covering a
   crash before the first tool journal exists. This minimizes, but cannot
   entirely eliminate, the narrow crash window between saving the human
   message and writing its queued marker.
8. Normal reviewed-tool runtime errors are also rendered as non-successful
   assistant history checkpoints. User-visible text comes from the host,
   never raw worker stderr/argv. The native host logs **counts only** for
   interrupted, uncertain, partial and unlinked-turn recovery.

Lu Core remains a normal non-sandboxed native application. The worker
lifecycle supervisor is separate from Lu's own user-level authority.

## Verification matrix

- Portable native harness compiles the **real production desktop recovery
  module** together with real Lu Core TaskStore and EffectJournal, without
  GUI/GTK/WebView stubs.
- Actual SQLite fixtures: running versus completed tasks; executing versus
  applied effects; preserved applied bytes; repeated startup; two one-sided
  journal creation interruptions; malformed turn ID; Unix symlink refusal.
- Dedicated `.github/workflows/lu-e1-p08-reviewed-recovery.yml` schedules
  Ubuntu 24.04 and Windows 2025 portable-native tests and a Windows
  frontend + Tauri desktop compilation/test pass.
- Native-chat portable tests verify genuine ANSF message replay, clear
  interruption labels, cross-chat isolation, no duplicate markers, completed
  response preservation, and recovery of queued turns with no worker journal.
- Linux-only real kernel crash proof uses a separate guardian leader PGID:
  SIGKILL the Lu-like parent process, verify its real worker and grandchild
  are no longer alive, and verify an unrelated control process survived.
  A second native test executes actual Git and FFprobe through the
  guardian-enabled reviewed supervisor.
- Test/compile results must reference the **exact candidate commit** and
  frozen `source-tree.lock.json`; green PR merge-check status or a
  scripted model response is not sufficient evidence.

## Remaining separate gates

- Source and native CI results are **OPEN** until real Windows/Linux
  runners complete. This phase does not claim installed-app acceptance.
- Source now includes a Linux-only first-party process-group guardian,
  with actual SIGKILL crash-injection tests. On Linux, a release build
  requires the sibling guardian binary for reviewed operations; a missing
  helper fails closed. The entire installer/package deployment remains
  open. The guardian signals its inherited process group but cannot kill
  workers that deliberately escape via `setsid()`/`setpgid()`, or
  `wait()`-reap descendants adopted by another parent.
  See docs/LU_E1_P08_LINUX_GROUP_GUARD_20261108.md.
- Profile-private old per-turn journal retention and user-facing recovery
  messages are not in this narrowly scoped source change. Do not delete
  an `APPLIED` receipt merely to save disk space.
- E1-P07's actual GGUF-selected tool conversation, full installed
  Windows/WebView acceptance and any pending earlier P03/P04/P05/P06
  native evidence remain independently open.
