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

**Successful R3 matrix:** https://github.com/vjmichael/Lu-AI-GUI/actions/runs/41973884203 — **23/23 Linux Rust tests passed**, `cargo fmt --check` passed, **7/7 frontend tests passed**, and **Windows Rust host tests plus standalone Tauri `cargo check` passed**. Tested source SHA: `81dc6651f6d1050f7c78ee61f5bef86b93ee28be`.

**Successful R3 optimized Windows release:** https://github.com/vjmichael/Lu-AI-GUI/actions/runs/41973964084 — unsigned 10,408,448-byte Windows PE executable (original artifact `lu-ide-code-editor-windows-alpha-r3`). EXE SHA-256: `2725961ebaa8936ba595e2b0b0af15ad93f6d59b037c5efd1f0a2e8df3197390`.

**User-facing R3 package:** `Lu-IDE-Standalone-R3-Code-Editor.zip`, containing the executable, `README-R3.txt` and disposable `R3TestProject`. ZIP SHA-256: `4c7ef595d5d0d119faceaebc65410ffc8a3489d327286fb080e661ec948e0816`. Archive integrity test passed.

**Windows R3 manual smoke acceptance — partially passed (November 4, 2026, America/Phoenix):** The user supplied two real Windows R3 screenshots with both `src/hello.py` and `src/app.rs` open as tabs, displaying user-edited content with **Saved** indicators. The user explicitly confirmed editing and saving, then closing the project folder and reopening the files with changes retained. This is sufficient to record **PASS** for the basic multi-tab open/edit/save/close/reopen persistence workflow.

**New visual finding — OPEN:** In both screenshots the CodeMirror gutter's line numbers appear stacked above the text and syntax highlighting appears absent; the earlier `style-src 'self'` CSP likely blocked CodeMirror's dynamically injected styles. CodeMirror provides `EditorView.cspNonce`; a Tauri-injected nonce carrier and matching facet have been committed as a security-preserving fix. The fix requires its own CI and Windows UI verification; **do not mark gutter/highlighting accepted yet**.

**Remaining manual gates:** Undo/redo persistence on tab switch, external-file conflict rejection, close/reload confirmation, binary/oversized rejection, Windows reparse point and metadata edge cases, and crash recovery remain unverified.

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


## R3.1 — CodeMirror CSP styling repair (Windows visual acceptance pending)

**Reported by user (November 4, 2026 America/Phoenix):** The R3 editor screenshots showed line numbers stacked above the source, with little or no syntax coloration. Saving and project reopen persistence worked, but editor layout was not accepted.

**Root cause analysis:** The standalone desktop configured `style-src 'self'`. CodeMirror 6 dynamically inserts style modules to lay out its gutter and syntax highlighting; a strict CSP blocks those inline style nodes without a nonce. Official CodeMirror provides `EditorView.cspNonce`; Tauri 2 generates nonces for static style tags and includes them in the packaged CSP. A static, empty style nonce carrier is added to `ide/app/web/index.html`; `ide/app/web/code_editor.jsx` passes its runtime nonce to CodeMirror. The CSP remains restrictive—no `unsafe-inline` or `unsafe-eval` added.

**Verification:**
- https://github.com/vjmichael/Lu-AI-GUI/actions/runs/41975718712 — all three standalone CI jobs passed: Linux Rust tests, frontend tests including new nonce regression check, and Windows native Tauri compile.
- https://github.com/vjmichael/Lu-AI-GUI/actions/runs/41975814653 — optimized unsigned Windows release succeeded; artifact `lu-ide-code-editor-windows-alpha-r3-1`.
- Release EXE SHA-256: `604abcc9470745ad9f623489c6c905d119aad6f5ba9adcacf84b52249acfab26`, size 10,408,448 bytes, valid PE header.
- User-facing package `Lu-IDE-Standalone-R3.1-Editor.zip` SHA-256: `6c0cc6cac54a9880e972c32934fbcb5de09e8aa74bc2348f502068719b46697b`; ZIP integrity passed, includes `StylingTestProject` fixture and instructions.

**Windows R3.1 user smoke acceptance — PASSED (November 4, 2026 America/Phoenix):** The user provided four screenshots of the actual Windows R3.1 application. CodeMirror line numbers now align with the associated text in the left gutter; Python (`src/hello.py`), Rust (`src/app.rs`), and Markdown (`README.md`) show syntax coloration. Three open editor tabs and the persisted project catalog (four registered projects) are visible. The user confirmed: **"The smoke test pass. The lines are in their proper place."** This closes the R3.1 **visual gutter/highlighting** gate. It does **not** close the separate undo/redo, external file conflict, file permission, or crash-recovery gates.

**R3.2 project-registration usability request:** The user requested a way to remove a project from the registered list without deleting its real directory. Implemented as **Remove from IDE**: visible confirmation, explicit guarantee that on-disk folders/files remain, warning if dirty tabs are open, and native catalog-only `unregister` operation. The host never removes project files. Linux/Windows and manual removal acceptance should be tracked separately from this R3.1 visual success.

Reference: https://codemirror.net/docs/ref/#view.EditorView%5EcspNonce and https://v2.tauri.app/security/csp/.
