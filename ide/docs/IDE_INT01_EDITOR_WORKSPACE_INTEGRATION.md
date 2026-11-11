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

## Remaining INT01 work (implementation/integration gaps, not CI tasks)

- **INT01-C — Multi-project/session reconciliation:** consolidate editor tab presence/identity across split panes, project switch, in-flight file opens, directory navigation, proposal drafts and native close. Prevent stale asynchronous results from selecting or resurrecting tabs from another project.
- **INT01-D — Durable workspace restart:** reconcile saved tab layout to current disk and private draft checkpoints. Restore navigation only by default; offer explicitly selected unsaved recovery, preserving unrelated original source and Undo ownership.
- **INT01-E — Working set and large projects:** bounded tree paging and search, quick-open navigation, async file admission, verified clean background tab reclamation and memory limits for thousands of files.
- **INT01-F — Unified editor UX:** keyboard/split-pane focus, external file-change comparison and conflict path, accessibility and consistent project switching.

## Deferred batch-regression plan

After the implementation work is complete, run the staged `int01_live_editor_write.test.mjs` and `int01_workspace_lifecycle.test.mjs` plus existing buffer, layout, disk-drift, source-search and draft tests in **one Windows/Linux task-group acceptance matrix**. No execution has been performed or represented as passing here. No physical Windows/NSIS, Linux Debian, LSP runtime or multi-project stress acceptance is claimed.

## Next task

**INT01-C**. Integrate the existing B-series cross-project/secondary-pane state paths and close stale async navigation/open outcomes before moving to INT01-D. Do not advance to IDE-INT-02 until INT01 integration source work is complete.
