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
6. The native host logs **counts only** for interrupted, uncertain and
   partial-turn recovery, never user text, file paths, tool stdout or
   model secrets. Repeating recovery is idempotent.

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
- Test/compile results must reference the **exact candidate commit** and
  frozen `source-tree.lock.json`; green PR merge-check status or a
  scripted model response is not sufficient evidence.

## Remaining separate gates

- Source and native CI results are **OPEN** until real Windows/Linux
  runners complete. This phase does not claim installed-app acceptance.
- An OS child surviving an abnormal Linux parent failure is **not proven
  dead or reaped** by SQLite reconciliation. Stronger descendant lifecycle
  reconciliation requires independent OS-specific PID/process-group
  identity evidence. Do not advertise this as crash-time process cleanup.
- Profile-private old per-turn journal retention and user-facing recovery
  messages are not in this narrowly scoped source change. Do not delete
  an `APPLIED` receipt merely to save disk space.
- E1-P07's actual GGUF-selected tool conversation, full installed
  Windows/WebView acceptance and any pending earlier P03/P04/P05/P06
  native evidence remain independently open.
