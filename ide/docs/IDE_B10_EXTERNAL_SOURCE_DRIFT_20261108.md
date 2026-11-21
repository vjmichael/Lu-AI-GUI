# IDE B10 — external source revision awareness and safe comparison

**Development branch:** `ide/b10-external-source-reconciliation-20261108`
stacked on B9 `6e0a05a2b6085acbcc907f08715ed7f5ee1935d4`.
No Lu Core or native worker/tool permission changes.

## User need and implementation

Lu IDE's native Save already rejects stale optimistic revision hashes when
another program changes the source on disk. Before B10 the editor displayed
only a broad conflict message. A user could not check multiple open files
for drift, review the currently saved disk version against their unsaved
CodeMirror draft, or distinguish a missing file from a stale response.

B10 provides two explicit **read-only** editor actions:

1. **Check disk changes** scans up to the B5 bound of 48 currently open tabs
   in the selected registered project. For each, it calls the original
   `ide_preview_source_file` native command; records only relative tab
   identity, previous saved SHA, current disk SHA and classification:
   `current`, `changed`, `unavailable`, `stale` or
   `invalid_native_result`. The UI marks independently observed unsaved
   drafts. No source text is retained in the report.
2. **Compare with disk** explicitly re-reads one selected file through the
   same native registered cap-std browser. It compares (a) the earlier saved
   text with the current disk contents, and (b) the **live CodeMirror draft**
   with the same current disk contents. It shows only the first differing
   logical line region, at most six lines per side, at most 240 characters
   per line. Excess lines and clipping are disclosed; it is NOT a full diff,
   three-way automatic merge or patch.

The comparison identifies newline-only changes (LF versus CRLF) separately.
It never mutates a CodeMirror document, alters its private B2 draft identity,
revises a Save base SHA, restores history, invokes Git, or writes project
source. Existing **Reload** (which requires user confirmation for dirty
drafts), **Review changes** and **Save** remain separate actions with their
previous revision and permission guards.

## Trust and concurrency boundaries

- The native original picked-folder capability remains the only project
  read path. No ambient path, symlink traversal, shell, watcher service,
  new native command, model call, network or tool authority is introduced.
- `editor_disk_drift.js` validates project ID, exact project-relative tab
  key, saved digest, uniqueness and 48-tab bound before making an audit
  plan. No source or draft bytes enter the plan.
- `classifyDiskAudit` accepts results only for the original project/file,
  valid revision and a matching in-memory tab generation, saved digest,
  buffer revision, and **same live CodeMirror EditorState instance**.
  A project change, tab close, Save/reload, typing or stale native
  response invalidates the result. Unavailable files are reported, never
  interpreted as an empty source.
- The existing B6 synchronous explorer I/O ticket serializes native reads.
  The inspected project ID and ticket are checked again after every await.
  Same-render Save/Review now check the synchronous ticket too.
- On editor changes, the old comparison is cleared and prior audit rows
  for that tab are marked stale. A previous disk report is an observation
  only and can never authorize Save or Reload.
- The comparison refuses source strings over 512 Ki UTF-16 code units;
  the original native read also denies files above 512 KiB UTF-8.
  UI snippets are plain React text (no HTML injection).

## Code / tests

- `ide/app/web/editor_disk_drift.js`: pure audit target, live-state
  validation, bounded first-difference display plan.
- `ide/app/web/main.jsx`: manually triggered scan/compare, status report,
  source identity checks, editor-safe lifecycle guards.
- `ide/app/web/styles.css`: bounded accessible report and comparison view.
- `ide/app/tests/editor_disk_drift.test.mjs`: ten Node regressions for
  project/scoped path validation, 48-file bound, stale editor/race responses,
  native response validation, source/draft/disk differences, line endings,
  oversized excerpts and UI read-only contracts.

During source development, **12 focused checks** evaluated exact committed
GitHub JavaScript with a V8 evaluator and all passed. The committed Node
suite, full `npm run check`, native Tauri builds, and installed OS editor
acceptance remain **OPEN** until executed, not source-accepted.

## Physical installed acceptance to revisit

1. Genuine installed Windows NSIS and Ubuntu Debian/GTK: select a project,
   edit a file without saving, then modify the original through an
   independently external process.
2. Scan: changed SHA must be reported with no project write and the live
   draft/Undo still intact; rename/delete a second file and confirm it is
   classified unavailable.
3. Compare: view independent base-versus-disk and draft-versus-disk bounded
   excerpts, including Unicode, CRLF/LF and large lines. No JS/mock bypass.
4. Attempt to Save the external-conflict tab: native optimistic revision
   guard must reject it; preserve the draft and private recovery option.
5. Change projects, close a tab or continue typing during a delayed disk
   read: the stale response must not be shown as authoritative.
6. Existing B2–B9 technical gates, source-bound B4/B5 performance receipts,
   protected execution and release signing/legal review remain independent.

No automatic file watcher, background polling, unattended merge or source
overwrite is claimed by B10.
