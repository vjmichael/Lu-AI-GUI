# IDE-INT-01 — Editor and workspace integration

Status: **implementation underway, NOT feature-complete or accepted**.
Workstream: standalone Lu IDE (Tauri/React/Rust) in `ide/`; independent of Lu Core and NVP.
Development branch: `ide/int01-editor-workspace-integration-20261111`.
This branch deliberately has **no new PR, CI workflow, Actions dispatch, native/installed verification or regression test execution**. The owner has paused those activities until the product-build phase is complete. Test cases are source-only deferred fixtures.

## INT01-A — Live buffer write/review/close/reload correctness (source implemented)

A source analysis of `ide/app/web/main.jsx` found that Save/Review relied on `tab.draft` and Close/Reload relied on `tab.dirty`, both delayed React render snapshots that can lag the current CodeMirror transaction.

- `editor_live_write.js` centralizes immutable live edit intents: project ID, exact tab key/path, disk base revision, buffer ID/revision, saved bytes and current CodeMirror bytes. It does not write project files or issue native calls.
- Save reads `EditorState.sliceDoc()`, uses the existing native revision-checked `ide_save_source_file`, rejects a mismatched native receipt, prevents same-tick duplicate Save, and rebases only its original buffer session after native completion. If the operator keeps typing while Save runs, the later text remains a dirty draft; a replaced tab is never rebased.
- Review captures the same live source, rejects a stale result after an asynchronous native review, binds the review to the exact buffer revision, and rejects saving a reviewed draft when the buffer changed.
- Close checks live dirty state, requiring explicit discard confirmation even when a React render has not caught up. Reload similarly protects current content and rechecks identity after LSP shutdown and native source read.
- Existing cap-std/native write authorization, disk-SHA stale refusal, private draft vault and editor Undo ownership remain unchanged.

## INT01-B — Workspace lifecycle and saved-search isolation (source implemented)

- Project close/unregister uses **all live CodeMirror buffers** to decide whether it would discard unsaved data rather than outdated React flags.
- Before the native lifecycle IPC, it captures the live project tab identities. If any tab or edit changes during that asynchronous IPC, it retains editor buffers **in memory** and warns the operator to reopen/re-register before saving. The OS picker project directory is never deleted.
- Saved project-search matches are tied to persisted disk bytes. An existing tab with a live dirty draft or unavailable identity cannot receive saved-file search coordinates even if the React dirty flag has not updated. No automatic source rewrite occurs.

## INT01-C — Multi-project and split-pane reconciliation (source implemented)

- Added pure `editor_project_navigation.js`: selecting an already-open file is allowed only when that tab belongs to the active registered project; choosing the secondary tab swaps pane keys rather than mounting the same CodeMirror state in two panes.
- Directory-driven project switching uses a shared navigation epoch and registered-project check; the native response must match the exact requested project and directory. Old project navigation is saved in memory, and the destination restores only its own live tab keys.
- Added a synchronous workspace-transition reservation for directory navigation and explicit proposal staging. Project close/unregister, file opening and saved-source search refuse to compete with an in-flight transition.
- Proposal staging captures live CodeMirror identities, rereads native source revisions and directory identity, rejects duplicate edits and stale project/epoch responses, and refuses to replace source tabs modified while the asynchronous review ran. The chosen proposal remains an *unsaved editor draft*, not a file write.
- File previews, search hits and proposal responses cannot commit after a later project-navigation epoch supersedes them. Existing source tabs are selected through the same pane-ownership planner; source/Undo text and inactive-project tabs are not rewritten merely by switching.
- Added `int01_project_navigation.test.mjs` as a **deferred, unexecuted regression fixture**. No CI, compiler, package, PR verification, or test run was started.

**Implementation status:** INT01-C source assembled. Runtime correctness and any further repair remain deferred to the batch after the implementation backlog, per owner instruction.

## INT01-D — Durable workspace restart and private draft recovery (source implemented)

- Reused the already existing B9 app-private **source-free** tab layout and B2 private draft-vault APIs; no new project file format, privileged authority, background worker, or Lu Core connection was introduced.
- Added `editor_restart_reconciliation.js`: a bounded, source-free plan distinguishing current/changed/missing saved disk files, live editor buffers that must remain intact, prior unsaved flags, independently verified private checkpoint metadata, and unavailable checkpoint inspection. All relative paths remain scoped to one registered project.
- Explicit **Restore remembered** now reads every saved path through the native project capability and inspects only its own latest private checkpoint. Before opening new tabs, it checks the live editor witness, rechecks proposed new disk bytes/revisions, and requires confirmation if remembered disk revisions changed. Project-switch and proposal-staging transitions are excluded while reconciliation is in progress.
- Reopened tabs contain **current saved disk text only**. No historical unsaved source, private checkpoint text, language-server process, build or debugger session is automatically restored, started, executed or saved. Existing dirty tabs and their CodeMirror Undo histories are preserved.
- Added a per-file Restart Reconciliation view showing changed/unavailable files, editor-preservation status and checkpoint state. A separate user action opens the file, and **Recover checkpoint to unsaved editor** invokes the B2 native load and rechecks immutable checkpoint identity, current disk base and live CodeMirror identity before applying. External source drift or replacement of a dirty editor requires additional explicit confirmation. The original project file is never changed by Recover.
- Remember, Forget, Reload, Close, Save, Review and project navigation now refuse overlapping in-flight recovery transitions. Discarding a layout never discards private draft history.

**Implementation status:** INT01-D source is committed. This does **not** imply native Windows/Linux, installed restart, accessibility, regression, permission or security acceptance. No tests/CI were run and no PR-verification work was started. Deferred future batch cases include missing/renamed files, changed disk SHA, stale checkpoint, user refusal, edited tabs during asynchronous native reads, cross-project navigation, and restart with partial draft vault access.

## Remaining INT01 work (implementation/integration gaps, not CI tasks)

- **INT01-E — Working set and large projects:** bounded tree paging and search, quick-open navigation, async file admission, verified clean background tab reclamation and memory limits for thousands of files.
- **INT01-F — Unified editor UX:** keyboard/split-pane focus, external file-change comparison and conflict path, accessibility and consistent project switching.

## Deferred batch-regression plan

After the implementation work is complete, run the staged `int01_live_editor_write.test.mjs` and `int01_workspace_lifecycle.test.mjs` plus existing buffer, layout, disk-drift, source-search and draft tests in **one Windows/Linux task-group acceptance matrix**. No execution has been performed or represented as passing here. No physical Windows/NSIS, Linux Debian, LSP runtime or multi-project stress acceptance is claimed.

## Next task

**INT01-E — Large-project working set and navigation**. Improve bounded project-tree paging, quick-open, asynchronous file admission, clean background-tab reclamation and memory controls. INT01-F unified editor UX follows. Keep regression and CI gates paused until backlog implementation is finished.
