# IDE B9 — private restart-safe workspace navigation

**Stack:** B9 `ide/b9-durable-workspace-navigation-20261108` on B8
`bf1f7fc238281dd125b25cb6f3a04509d6b490b6`. Earlier B4/B5/B6
acceptance evidence is unaffected. Standalone Lu IDE only; no Lu Core changes.

## User-visible behavior

The editor offers three **explicit** actions:

- **Remember tabs** — save the active registered project's list of relative
  open source paths, last saved SHA-256 revision for each file, whether it
  had unsaved edits, selected primary/secondary pane, split state, and
  bounded tab strip position to private app data.
- **Restore remembered** — after the user reopens a registered project and
  enters its editor following a restart, *revalidate* each saved relative
  file against the native browser, reopen its **current saved disk bytes**
  in CodeMirror if not already open, and restore the surviving pane selection.
  Missing, moved, no-longer-text, or inaccessible files are skipped with a
  counted warning; changed disk revisions are reported. Already-open dirty
  tabs are never overwritten. Only the original registered-project
  capability may read those files.
- **Forget layout** — delete that project's private navigation metadata.
  The original project files, live CodeMirror edits and distinct B2 private
  draft checkpoints remain untouched.

This deliberately uses **manual Remember** rather than background capture,
and **manual Restore** rather than automatic file reopening on launch. On
startup the desktop reads a bounded private layout *only after* the project
has been reopened through the existing native registry; the read never
opens a source file. This avoids surprise filesystem activity, unexpected
retention of filenames and silent replay of stale code.

**No unsaved draft bytes are in B9.** If a remembered tab had a dirty editor,
the user sees an explicit warning before saving/restoring navigation. Only
B2's separately consented private draft checkpoint can preserve or recover
that unsaved content, and its recovery is always a separate user decision.
Plain CodeMirror Undo/selection content from prior process memory is not
reconstructed by B9.

## Native custody

New `ide/host/src/workspace_layout.rs` implements
`WorkspaceLayoutStore` at
`<native app_data_dir>/private-workspace-navigation/<ide-project-id>.json`.
The **host**, not JavaScript, supplies the app-data location.

The typed `WorkspaceLayout` schema 1 is restricted to 1–48 source paths,
their saved SHA revisions, a dirty indicator, two optional selected paths
which must be members of the list, a split boolean and 0–47 tab offset.
Extra JSON fields, path traversal, absolute/drive paths, duplicates, oversized
records, malformed project IDs and SHA values are rejected. There is **no**
source text, native filesystem root, worker authorization, executable,
private checkpoint ID or network resource in the record.

Each Remember operation first uses
`IdeWorkspaceRegistry.preview_source_file` — the preexisting cap-std
read-only registered source capability — to verify every path and its exact
saved disk SHA. An externally changed source produces
`B9_SOURCE_CHANGED` rather than quietly accepting stale metadata.
The serialized record is bounded to 96 KiB, written via a private temp file,
`fsync`, atomic same-directory rename and Unix parent sync; Unix private
directory mode is 0700, temporary file 0600. Symlinked/reparse or
group/world-accessible private storage fails closed. The storage carries no
project-file write authority.

Three native Tauri commands:
`ide_workspace_layout_read`, `ide_workspace_layout_save`,
`ide_workspace_layout_clear`. Read/clear require the project to be active
through the native registry. The existing native project-unregister path
removes the private layout first, including for an inactive registered
project; it does not delete source or B2 checkpoints.

## Frontend revision and recovery boundaries

`ide/app/web/workspace_layout.js` only serializes navigation fields. It
independently checks the live CodeMirror buffer during Remember so a recent
edit not yet reflected in React's `tabs` state is marked dirty without
persisting that source text.

On Restore, each file goes through
`ide_preview_source_file(projectId,relativePath)` and
`revalidatedResumeDecision`. The synchronous B6 explorer ticket excludes
conflicting ordinary file opens, the current project ID is checked after
each native read, and a tab removed while the restore ran causes the entire
restore to be rejected before mutating editor memory. A whole restored batch
is capacity-checked under B5's 48/project, 96/global limits. Existing tabs
retain their EditorState and Undo. New tabs receive **fresh** buffer identity
and contain *only* verified current disk text. A separate B2 latest-draft
inspection can display a recovery choice but never applies one automatically.

Project close/unregister, tab close, Save, Reload and explicit clean-tab
reclamation are guarded while a B9 navigation operation holds its ticket.

## Test evidence (separate from acceptance)

- `ide/app/tests/workspace_layout.test.mjs`: nine Node tests for source
  exclusion, fresh dirty-state observation, foreign/traversal/duplicate
  path denial, 48-file cap, extra-field stripping, exact response identity,
  stale disk revision and frontend/native wiring.
- `ide/host/src/workspace_layout.rs`: three Rust pure validation tests.
- `ide/host/tests/workspace_layout.rs`: six **genuine filesystem** native
  integration scenarios covering restart and registration gating, disk
  revision drift, explicit clear preserving project and B2 vault, inactive
  project unregistration, unsafe Unix storage and deleted source rejection.
- During implementation nine V8 checks against fetched exact JavaScript
  helper sources passed. That is **not** a Node 22/npm acceptance, real
  Windows/Linux Rust execution, or installed app UIA/AT-SPI result.

## Gates still OPEN

1. Full frontend `npm ci && npm run check`, including the new Node tests.
2. Actual `cargo test --manifest-path ide/host/Cargo.toml --all-targets`
   on Windows and Linux, with native symlink/ACL testing.
3. Locked Tauri Windows/Linux compile, power-loss/corrupt-record handling
   and installed NSIS/.deb restart → Reopen project → Restore tabs →
   unsaved source preservation → Uninstall acceptance.
4. B4/B5 physical memory and latency receipts, B6 race/GTK acceptance,
   legal/signing and protected LSP/worker gates remain independent.

No success at a prior SHA or source-only V8 test is a release acceptance.
