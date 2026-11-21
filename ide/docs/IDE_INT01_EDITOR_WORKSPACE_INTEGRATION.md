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

## INT01-E — Large-project working set and navigation (source implemented)

- Added `ide/host/src/quick_open.rs` with bounded cap-std native filename-only discovery: on-demand breadth-first traversal from the registered project, no symlink-follow, generated-dependency directory exclusions, maximum 30,000 entries / 3,000 directories / 24 levels, and up to 60 ranked path hits. The service never reads source text, executes a process, modifies the worktree or accesses a remote.
- Added sorted native directory paging with a 120-name page, a validated filename-only next cursor and bounded enumeration. The cursor is a transient observation, not a persistent directory snapshot or any authority. When a folder exceeds the scan cap the user is warned that the listing is partial rather than assured of completeness.
- Added Tauri read-only commands `ide_quick_open_source_paths` and `ide_list_source_directory_page`, plus `quick_open_view.js` to check project scope, bounded responses, filenames and page cursor before UI presentation.
- Integrated the Explorer Quick Open form, explicit Find files, ranked filename results, separate handling for files larger than the existing 512-KiB editor admission limit, Ctrl/Cmd+P keyboard focus, and progressive Load next 120 names. All selected files use the existing native source preview, late response guard and per-project tab admission, not an implicit file grant.
- Preserved bounded CodeMirror working sets (48 tabs per project, 96 globally) and existing light-weight display for large admitted files. Added visible working-set counts; `Close clean tabs` remains a direct user action and uses the existing live-text proof to preserve both visible panes, dirty/unknown buffers, private draft activity and active language service buffers. No automatic eviction or source changes.
- Directory first-load and proposal-staging folder navigation now use the paginated host source. Project-switch epochs, explicit draft recovery, and protected native tasks retain their earlier custody boundaries.

**Implementation status:** INT01-E source committed without running tests, CI, PR verification or installed acceptance. The deferred future batch should cover 30k-entry cutoff, huge folders, Unicode ordering, deep generated folders, symlink/permission refusal, stale page cursor, cross-project results, Quick Open at full tab budget, and live Undo preservation on clean-tab reclamation.

## INT01-F — Unified editor UX and accessibility (source implemented)

- Added separate first/second editor labels and focus tracking to the existing CodeMirror view. Explicit Ctrl/Cmd+1 and Ctrl/Cmd+2 actions focus only the registered project's matching current editor pane. The editor accepts a requested focus only when project, pane and tab generation all match; opening a different project resets transient focus.
- Ctrl/Cmd+S now targets the last genuinely focused editor pane even when keyboard focus has moved to a toolbar, rather than falling back silently to primary. Saving continues through the existing reviewed native source-revision gate and never grants broader execution authority.
- Added direct editor-toolbar navigation to source-verified Problems and project-scoped Jobs; the Jobs screen now provides direct return navigation to either source Problems or the first editor. These buttons navigate existing UI and only request read-only project job/intent inspection, never initiate execution.
- The Problems panel is programmatically focusable following an explicit user navigation action. Added roving source-tab keyboard control with ArrowLeft/ArrowRight/Home/End, accessible tab selected state and focus transfer to the newly selected tab even when tab paging changes.
- Added per-project readable status/busy messages for project switching, tab counts, layout/draft recovery, and editor pane count. Native directory selection announces successful project switching while retaining other projects' unsaved buffers.
- External-file-change inspection now shows separate counts of changed/current/unavailable/stale files, with clear warnings for simultaneous disk drift and unsaved local editor changes. Comparison, reload, and Save remain separately explicit operations; no automatic merge/write or fabricated verified status.
- Updated responsive CSS for visible pane keyboard-focus boundaries, status messages, Problems/Jobs return controls and high-contrast focus outlines. No additional source-code storage, Lu bridge or native worker was introduced.

**Implementation status:** INT01-F source committed. The combined INT01-A–F implementation pass is **source-complete**, not accepted: no source-wide repair, compiler/test execution, CI/PR verification, installed Windows/Linux acceptance or human physical interaction matrix was run. Deferred regression scenarios include keyboard focus across project switches and split-pane swaps, Ctrl/Cmd+S after toolbar focus, roving tab paging, stale diagnostic navigation, job progress isolation, external disk drift and assistive-technology accessibility.

## Remaining INT01 work (implementation/integration gaps, not CI tasks)


## Deferred batch-regression plan

After the implementation work is complete, run the staged `int01_live_editor_write.test.mjs` and `int01_workspace_lifecycle.test.mjs` plus existing buffer, layout, disk-drift, source-search and draft tests in **one Windows/Linux task-group acceptance matrix**. No execution has been performed or represented as passing here. No physical Windows/NSIS, Linux Debian, LSP runtime or multi-project stress acceptance is claimed.

## Next task

**IDE-INT-02 — Language Server Integration**. Continue the feature backlog by integrating protected Rust, C/C++ and Python language servers against current source buffers, completion, diagnostics and restart lifecycle. All source analysis/repair, CI, PR checks and regression/installed acceptance are postponed until backlog implementation is complete.
