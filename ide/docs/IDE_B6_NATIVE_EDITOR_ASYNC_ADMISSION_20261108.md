# IDE B6 — asynchronous native editor admission and draft identity safety

**Branch:** `ide/b6-async-editor-admission-20261108`, stacked on B5 draft
PR #147 exact starting source `029f5b131f5855494ed9e38af5d6956dc48abf8d`.
Independent source branch (no new PR/CI fanout requested during current GitHub
queue congestion). B4/B5 exact-head CI and installed OS measurements remain OPEN.

## Defect and correction

The existing B5 48-file admission and dirty-draft preservation were tested
only as synchronous decisions, but source previews use asynchronous native
Tauri IPC. Two clicks in a single React render can each see the same tab
count before `explorerPending` is published, and an in-flight disk read can
return after the project, tab or draft changed. A 47-to-48 tab race must not
create a 49th tab, reset a private unsaved draft identity, discard Undo, or
reopen a file for a project that is no longer selected.

- Added a synchronous ref-backed per-explorer read reservation, shared by
  directory navigation, ordinary file open, saved project-search hit and
  source reload. React busy UI remains present, but is not the lock.
- The native preview response is checked for the requested project ID and
  exact relative path and compared with the *current* registered project.
  After the `await`, tab admission is computed again using `tabsRef.current`.
  A concurrently opened file is selected without resetting its draft.
- Saved search hits now use the live tab set rather than a stale React closure,
  and re-check the capacity after the source revision has been verified.
- Reload refuses to replace a tab when it has been closed, its project has
  switched, or its revision, draft, dirty state, generation, buffer revision
  or CodeMirror EditorState object changed while native I/O was outstanding.
  The dirty draft is retained and the user sees a refusal, rather than an
  implicit Save or a silent discard.
- Directory read responses validate project/path and are dropped when the
  initiating project selection is no longer current. The implementation
  does not widen cap-std, native permissions, allowed text sizes, or worker
  execution authority.

## Source verification added

`ide/app/tests/editor_async_admission.test.mjs` checks:
1. Request admitted at 47 tabs, refused on completion if the 48th was opened;
2. Concurrent duplicate open must select, not replace, a dirty draft;
3. Foreign project, wrong path and malformed response discarded;
4. Late reload denied after typing, generation change, close or project switch;
5. UI contract for the synchronous ticket, post-I/O admission and release.

`npm test`, the locked frontend build, both native Windows/Linux Tauri
compiles, and installed real GUI multi-click/race acceptance have **not**
run in this isolated source workspace. A committed test is not a passing test.
No B5 performance or memory threshold can be claimed before genuine JSON
measurements from installed Windows and Ubuntu.

## Reconciliation and pending acceptance

- Keep B5 PR #147 and its exact-head required Windows/Linux checks intact.
- Run B6 tests on both native OSs when capacity is available, including:
  two source opens in the same UI frame; tab #48 vs #49 contention; closing
  and changing projects during slow read; typing during an explicitly
  confirmed reload; and verifying original file hashes and private drafts.
- Review current-head installed B4/B5 receipts *before* considering their
  release/GUI gates complete. B6 remains source-implemented, unaccepted.
- No Lu Core source or runtime isolation policy is changed by this branch.
