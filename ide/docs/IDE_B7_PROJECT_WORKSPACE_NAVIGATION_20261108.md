# IDE B7 — project-scoped editor navigation continuity

## Purpose

B5 kept up to 48 open source files per project (96 overall), including private
CodeMirror EditorState/Undo history, but switching to another already-registered
project reset split-pane state and used the most recently added tab as the active
file. This broke the user's working context while their unsaved drafts remained
alive in RAM.

B7 keeps the **in-process** view choices for each currently registered project:
primary selected source, separate secondary source, whether the editor was split,
and the bounded visible tab-window offset.

## Implementation

- `ide/app/web/editor_project_view.js` contains pure `captureProjectView` and
  `restoreProjectView`; both use only live registered project tab identities.
- Before successful native project switch, `main.jsx` captures the departing
  project's view, then restores only valid, still-open tab identities for the
  destination. Removed tabs cannot be resurrected by stale view data.
- The project-change effect no longer erases the restored split view. Rendering
  an active tab from another project during a transition is explicitly denied.
- Proposals staged in a different project preserve the old project's view but
  focus the newly staged draft with a fresh single-pane view.
- Closing or unregistering a project invalidates its saved in-memory view;
  removing an inactive project does not collapse the currently displayed split.
- No source bytes, private draft text, OS absolute roots, CodeMirror EditorState,
  task authority or LSP worker information is stored in a saved view.
- The current B2 private draft vault is independent. **B7 is not cross-process
  session persistence**, does not auto-open files on startup, and never reads or
  writes project source while restoring navigation.

## Verification and open acceptance

`ide/app/tests/editor_project_view.test.mjs` includes six Node cases:
per-project primary/secondary restoration, cross-project isolation, removed
source fallback, bounded 48-tab window, untrusted/empty view denial, and
frontend wiring/source-guard contracts.

Five focused V8 smoke checks were executed against the exact GitHub JavaScript
files during implementation, and all five passed. Full Node 22 `npm run check`,
actual Windows/Linux Tauri compilation and installed two-project UIA/AT-SPI
acceptance remain separately OPEN. No B4/B5/B6 acceptance status is inferred.

## Next B work

B8: explicit, conservative cleanup of **only provably clean, non-visible,
non-busy** background editor tabs to free capacity and memory. No automatic
dirty-buffer eviction, no implicit Save, and no reopening from navigation
metadata.
