# IDE Task 3 — read-only project explorer acceptance

**Date:** 2026-11-04  
**Branch:** `ide/workspace-development`  
**Status:** **PARTIAL IMPLEMENTATION — project explorer and source preview complete; editing/saving and full IDE acceptance pending.**

## Implemented

- `ide/host/src/browser.rs`: capability-relative project tree listing and bounded source reading through `cap-std`, with no executable worker.
- `ide/app/src-tauri/src/lib.rs`: host-owned `ide_list_source_directory` and `ide_preview_source_file` commands. WebView supplies an opaque registered project ID plus a validated, relative path, never an absolute host root.
- `ide/app/web/main.jsx`: user-accessible project Explorer, directory navigation, back/up, file selection and escaped source text preview.
- `ide/app/web/styles.css`: separate file tree and source pane.
- `ide/host/tests/browser.rs`: nested folders, binary rejection, oversized text rejection, path traversal, external symlink rejection, restart/reopen state, changed root and bounded directory listing.
- `ide/app/tests/standalone.test.mjs`: UI command allowlist and source-rendering safety assertions.

## Boundaries and safety

- Opening a project still does not run package scripts, build steps, language servers, debugger adapters, extensions or an AI.
- Source listing is **non-recursive** and capped at **400 entries** per directory with a visible truncated notice. Source preview is **UTF-8 only**, at most **512 KiB**, and rejects embedded NULs.
- Explicitly reject empty/non-normal path segments, traversal (`..`), absolute paths, Windows drive prefixes, backslashes and control characters.
- Do not allow directory symlinks or file symlink preview. Source lookup occurs relative to a **capability directory**, not a host ambient path concatenation. There remains a project-root identity/swap verification gate before file mutations.
- Source text is rendered as a React text node inside `<code>`, **not** `innerHTML`, a browser URL or executable code.
- A currently open registered project is required. Closed/missing project identities are rejected.

## Verified evidence

- **Linux native Rust:** https://github.com/vjmichael/Lu-AI-GUI/actions/runs/41970834817 — **16 Rust tests passed**, including browser fixtures; committed `rustfmt --check` passed.
- **Frontend:** same run — **5 Node tests passed**, standalone React/ESBuild production bundle built.
- **Windows:** same run — `cargo test` for the IDE host and `cargo check` for the *independent* Tauri desktop passed.
- This is GitHub-hosted native validation, not the user's local Windows click-through acceptance.
- R2 Windows executable build: https://github.com/vjmichael/Lu-AI-GUI/actions/runs/41970937767; check its actual conclusion and artifact, do not assume success in advance.

## Still pending before Task 3 completion

- Actual editor integration (evaluate Monaco vs CodeMirror); multi-tab editing, undo/redo, search/replace, syntax support.
- Authorized atomic file saves, backups/recovery, dirty buffers, write conflict detection and safe rename/delete (none of these are present).
- Project-root capability pinned across rename/replacement races and Windows reparse-point adversarial acceptance.
- Very large trees, ignore rules, file watchers, indexing, binary/encoding UX and accessibility/load testing.
- A human's installed Windows run of the R2 Explorer: choose a disposable folder, navigate nested directories, open file preview, verify closed-project denial and restart persistence.
- **Task 2 sandbox remains separate and incomplete**. No executable worker is exposed by the explorer.

## Next engineering step

IDE Task 3: explicit user-owned editing with safe atomic saves and conflict prevention, followed by a production-grade editor component. Keep source and AI prompts separate; future coding chat is a distinct workspace surface (ADR-0002).
