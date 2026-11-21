# Lu IDE C7 — registered source evidence linked to C6 development tasks

**Stack:** `ide/c7-task-source-evidence-20261108`, based on B11
`d49de2bda55f566b5ef8bc5f020be1d0cfa80863`. This is native +
editor **source implementation**, not installed-app acceptance.

## Purpose

C6 already provides the canonical app-private `development-ledger.jsonl`,
append-only human task revisions, bounded exact-head CI observations and
explicit unverified installation gates. C7 links real editor file identities
to those existing task IDs without creating another task database or
declaring that editing a file proves successful task completion.

In Explorer, choose **Load C6 tasks**, select an existing task, and choose
**Link saved file to task**. This is explicit and asks for confirmation.
If the live CodeMirror text differs from the saved base, the user sees a
warning: **unsaved draft bytes are not included**. The request carries only
project ID, task ID, validated project-relative path, expected saved file
SHA-256 and the exact task-update event ID originally selected.

The new `ide_link_task_source` command calls the original native
`IdeWorkspaceRegistry.preview_source_file` (registered cap-std
read-only browser), compares that independently observed SHA to the
submitted expected saved SHA and checks the C6 task event ID twice,
including inside the ledger append lock. It records a
`task.source.link` event with `TaskSourceLink` (relative path, saved
SHA, recorded task-update ID, observed timestamp and
`native_registered_source_read` provenance) in the **same app-data journal**.

A task can have at most 12 distinct source links; explicitly relinking an
existing file updates that source entry in the materialized view, while
all prior ledger events remain immutable. Task note edits retain the old
source links **as historical references to earlier task-update IDs**.
No source link is silently promoted to the new task revision.

In Continuity, each task shows its linked source paths, saved hash,
association age, whether a currently open tab's saved base SHA appears
to match (not proof the disk is unchanged), and whether unsaved text is
present. **Recheck disk** separately calls the same native source preview
and can distinguish an exact recorded hash from a later disk change,
missing file or invalid native response. It never changes source files.

## Trust and evidence

The C7 `task.sourceLink` typed catalog operation declares
`catalog_write` for the **private development journal only** and no
project file write or worker authorization. The existing task schema
continues to reject user-supplied CI verdicts or completion status.
The C6 `taskAcceptance=not_verified`, source-review,
installed-Windows and human-approval gates remain unchanged.

Every observation is time-sensitive:
- A saved source SHA proves only bytes observed in a registered file at
  the recorded time; it is not a complete source review or test result.
- A live tab's saved base SHA is not a fresh disk check.
- A changed task revision leaves the prior source link historically
  visible, not current task evidence.
- A successful GitHub CI job proves only exact-head CI as specified in C5;
  it does not prove private drafts, unsaved edits or the installed app.
- No project notes, code snippets, absolute filesystem roots, draft
  checkpoint bytes or source contents are persisted by C7.

The original C3 portable development checkpoint remains source-text-free,
and C7 does not add private filenames to its export automatically. The
user may link multiple source files and task IDs, but no automatic
inference or model tool action is invoked.

## Tests and open acceptance gates

- `ide/host/tests/task_source_link.rs`: 5 real native filesystem tests:
  restart durability, concurrent task-note edits, changed disk files,
  traversal and project closure, 12-link limit/relink.
- `ide/app/tests/task_source_evidence.test.mjs`: 9 frontend assertions:
  metadata-only requests, unsaved warning, project isolation,
  typed schema/capability parity, stale/historical evidence,
  exact native recheck, no implicit Save or CI promotion.
- Development-only V8 evaluation ran all nine committed frontend test
  bodies with assertion emulation: 9/9 passed. **This is not native Node,
  Rust, desktop compilation or installed acceptance.**
- Dedicated source workflow:
  `.github/workflows/ide-c7-task-source-evidence-native.yml`.
  Exact SHA Windows 2022 and Ubuntu 24.04 host Rust, Node tests and
  locked Tauri compile, plus authority-diff guard, must report real pass.
- Installed NSIS/GTK: open a registered project, create task, link
  edited file with and without unsaved draft, check no source writes,
  restart, verify task/file event replay, replace or remove source,
  recheck mismatch, revise task and check historical linkage.
- B2–B11 native and installed acceptance remains independently
  required; especially B4/B5 editor memory evidence, B6 GTK chooser,
  B9 restart navigation, B10 disk drift and B11 merge Undo.
- No release signing, redistribution review, protected worker
  execution, Lu Core bridge or task completion gate is changed.

Source-implemented C7 does not close outstanding acceptance gates.
GitHub jobs are allowed to run while independent development proceeds.
