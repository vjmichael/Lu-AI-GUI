# Lu IDE DEV-01.5 — two-pane source editor

**Status:** Source-implemented only; native Windows/Linux frontend tests and installed GUI usability pending.
**Dependency:** DEV-01.4 draft recovery and DEV-01.3 guarded file operations; historical P05 merges remain separate.

## Scope

- Toggle **Split editor** from the active file's toolbar after opening at least two distinct source tabs. Left pane retains the selected tab; a scoped right-pane selector chooses a different open tab of the **same** current project. A single original CodeMirror `EditorState` and tab key own each file's draft; the split does not clone a draft or silently save files.
- Keyboard focus tracked per pane. `Ctrl+S` routes to the actual focused tab, while the right pane exposes a visible **Save secondary** action that still performs the original exact native revision-checked Save. The existing source conflict warnings and recovery checkpoint hooks remain intact.
- Closing the right tab invalidates the split's secondary identity. Switching projects resets the split to avoid showing a tab from another project. At narrow widths the two panes stack instead of clipping each other. No new native filesystem grants, model execution or external service.
- Static regressions guard source identity and focus-Save routing; dedicated Windows 2022/Ubuntu 24.04 Node checks run full `npm run check` at exact head.

## Open acceptance

Real installed Windows/Linux mouse, keyboard, accessibility, selection/undo, dual simultaneous edits, save-from-focus, source collision, restart/draft-recovery and memory performance are not yet evidenced. LSP presentation remains tied to primary pane until protected LSP provenance and feature routing are accepted. This is not a general multiwindow/detachable editor or per-pane navigation-history implementation. The core P03/P04/P05 and legal/signing gates remain unchanged.
