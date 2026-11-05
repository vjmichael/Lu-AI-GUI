# IDE Task 3 — R3 native tabbed code editor acceptance

**Date:** 2026-11-04 (America/Phoenix)  
**Branch:** `ide/workspace-development`  
**Status:** **IMPLEMENTED CANDIDATE** — Linux/Windows code verification and installed-app acceptance tracked separately. Not production-ready.

## R3 functionality

- Standalone, independently launchable `ide/app` Tauri desktop; **no Lu Core dependency** and no model connection.
- CodeMirror 6 editor instead of the read-only preview. Supports syntax highlighting for Rust, Python, JavaScript/JSX, TypeScript/TSX, JSON and Markdown. Unknown UTF-8 file types can still be edited as plain text.
- Multiple source tabs with separate CodeMirror `EditorState` objects, preserving individual undo/redo histories and cursor/selection through tab switches.
- Dirty-state indicators, explicit **Save** button and **Ctrl+S**, reload with confirmation before discarding unsaved edits, confirmation before closing a dirty tab or project.
- Editor writes use a host-issued project ID and validated relative path, and cannot be invoked without the explicit Tauri command.
- Native host rejects a write when the original SHA-256 revision differs from the revision received when the tab opened: `FILE_CHANGED_ON_DISK`. The original draft stays in memory on conflict.
- Native host serializes saves from the IDE process; it validates UTF-8, rejects NULs, oversized text (over 512 KiB), path traversal, symlinks and closed projects.
- Write creates a randomized sibling file with `create_new`, writes and flushes data, sets permissions via the open file handle, syncs the file, rechecks the source revision, replaces the source using capability-relative rename, then syncs the parent directory on Unix.
- Source scripts are **never executed** during open, edit or save. No package manager, LSP, terminal, debugger or AI coding agent is enabled.

## Evidence sources

- `ide/host/src/editor.rs`, `ide/host/src/browser.rs` — capability-relative save and SHA-256 content revision.
- `ide/host/tests/editor.rs` — normal write, revision update, stale external-edit conflict, invalid paths/ID, symlink denial, permission mode preservation, persistence and competing writers.
- `ide/app/web/code_editor.jsx` — CodeMirror 6 state, syntax extensions and edit updates.
- `ide/app/web/main.jsx` — tab lifecycle, dirty/save/reload/conflict prompts and keyboard shortcuts.
- `ide/app/tests/standalone.test.mjs` — frontend command and editor source assertions.
- `.github/workflows/ide-standalone.yml` — real Linux Rust tests, Windows Rust/Tauri checks and frontend build/tests.

**First native save test result:** https://github.com/vjmichael/Lu-AI-GUI/actions/runs/41973185867 — the Linux Rust tests detected a post-replacement directory `fsync` failure (`EBADF`) despite a successful rename. The code was corrected to obtain a read-only directory file handle *before* the rename. Do not treat that first run as accepted.

**Later check:** https://github.com/vjmichael/Lu-AI-GUI/actions/runs/41973439474 — Rust tests passed after the fix, with only formatting failures. The Rust 1.88 formatting output was committed separately.

**Latest exact revision:** Check https://github.com/vjmichael/Lu-AI-GUI/actions/workflows/ide-standalone.yml for final R3 green matrix. Do not inherit a success from an earlier SHA.

## Remaining safety and correctness limitations

1. The **compare-before-rename** algorithm is optimistic. An uncooperative external writer can race between the last revision comparison and replacement. Cross-process identity pinning / lock coordination and Windows reparse-point attack tests are separate release gates.
2. Unix file mode bits are preserved. Complete Windows ACL/owner/alternate-stream and special-file metadata preservation is not yet guaranteed. Avoid editing sensitive production repositories until this passes.
3. Unsaved edit buffers are **memory-only**. The UI warns on normal close/reload; automatic recovery from process/power crash is **not yet implemented**.
4. The replacement file is synced before rename. Parent-directory durability is verified on Unix. Windows storage-failure and antivirus/locked-file failure injection remain open.
5. No language server, extension, tool, build, code runner or model may execute code from a project. Task 2 sandbox remains incomplete and separate.
6. Files larger than 512 KiB, binary/invalid-UTF8 files, recursive search/replace, multiple selection diff/recovery, encoding transformations and large-project performance require later work.
7. **Source-file writes are human UI operations only.** There is no Lu bridge; sharing a model later does not confer file-edit permissions.

## Windows R3 manual smoke test

Use an expendable copy of `DemoProject`, not a live production project. R2 already demonstrated opening and browsing source.

- [ ] Launch R3 as a standalone app without Lu running.
- [ ] Open the test project, select `src/hello.py`, then `src/app.rs`. Both create tabs; syntax highlighting works.
- [ ] Add text to `hello.py`, switch tabs, return and **Undo/Redo**. Verify the draft and history survived.
- [ ] Click **Save** or **Ctrl+S**. Close/reopen the file and verify disk bytes changed.
- [ ] Modify the same file externally after opening it in the editor, then attempt to save a draft. Verify **FILE_CHANGED_ON_DISK** and that the external file was **not overwritten**.
- [ ] Confirm **Reload** warns before discarding an unsaved draft; confirm closing a dirty tab warns.
- [ ] Confirm project close does not silently discard dirty tabs.
- [ ] Attempt to open a non-UTF8/binary or oversized file. Expect an error, not execution.
- [ ] Observe the UI remains **Lu connection: Not connected**; editing works without Lu.

**Pass criteria:** Record exact build SHA and OS, actual file bytes before/after, successful conflict rejection, undo/redo after tab switch and dialog behavior. A CI build is not human desktop acceptance.
