# IDE-INT-02 — Language Server Integration

Status: **feature implementation in progress; native acceptance deferred**. Continue on the existing IDE implementation branch, not a separate PR/CI queue. GitHub is source storage only during the build-first phase. No tests, CI, PR verification, broader codebase review or runtime claims are authorized in this development phase.

## INT02-A — Exact live CodeMirror source binding (source implemented)

- `lsp_live_source.js` captures the same active project/file, immutable buffer instance, saved disk SHA-256, buffer revision, and current CodeMirror UTF-16 text as the reviewed edit workflow. Native LSP request text is capped at 512 KiB UTF-8, rejects NUL and unsupported language IDs, and validates UTF-16 cursor positions before dispatch.
- Start, document change, diagnostics, completion, hover, and same-file definition now bind requests to this exact live source, not a potentially delayed React `tab.draft`.
- Every native reply must echo the exact source revision **and actual native selected provider**. Changed projects, tabs, undo buffers, saved bases, and newer edits invalidate old LSP replies rather than rendering them. Editing immediately hides stale diagnostic/hover/completion responses.
- Completion apply uses the captured exact source text. The existing CodeMirror transaction path independently checks document bytes immediately before application and preserves native editor Undo.

## INT02-B — Session lifecycle, worker cleanup, and Problems integration (source implemented)

- One native session is admitted only after an explicit OS permission dialog using the existing protected worker. The UI uses a synchronous busy reservation and lifecycle epoch to prevent duplicate starts and stale async publication.
- Project switching and native worker cleanup stop the session. Individual file close/reload or saved-base changes detach only that file once INT02-E is present. A failed stop or document cleanup is not treated as a verified exited process; additional language operations remain blocked until cleanup.
- Debounced document changes and separately requested features reject responses after source or session changes. Native host still owns OS containment, release pins, workspace snapshot, and per-session human authorization.
- Per-file CodeMirror selections now update exact line/UTF-16-character coordinates. Existing source-verified Problems navigation remains read-only and no LSP-provided URI is treated as a file capability.
- Language Services panel now shows actual active session and protected cleanup controls, including when the previous project was switched away.

## INT02-C — Semantic Rust/C/C++ and optionally pinned Python integration (source implemented)

- Rust uses native rust-analyzer and C/C++ uses native clangd. Their existing supervised, explicitly approved transport is retained.
- Python continues to support Ruff diagnostics when only reviewed Ruff exists. A new optional standalone, offline **Pyright** route is allowlisted in both protected Windows and Linux worker recipes, checked through the same root-owned/pinned binary-tree constraints, and selected by the Tauri native host only if its exact installer SHA-256 pin matches.
- Only an actual pinned Pyright session can perform Python completion, hover, and same-file definition. Both the global native semantic-language inventory and per-response `providerId` are checked before the UI enables semantic operations. No user PATH, Node/npm download, shell execution, or permissive fallback.
- **Important limitation:** adding the provider route does not install, build, approve, attest, or prove a genuine working Pyright executable bundle. The separately reviewed complete offline executable wrapper and its compiled-in SHA-256 remain required before Python semantic features can become available.

## Remaining INT02 implementation / integration

- **INT02-D — Native source-bound cross-file definitions (source implemented)**: protocol now carries an opaque candidate URI to the native host, which reconstructs the exact protected-worker staging prefix, rejects noncanonical escapes, traversal, foreign roots, unsupported file types and unregistered targets, and compares its sealed snapshot bytes with the live registry's saved SHA-256. Only a project-relative filename, verified target revision and checked UTF-16 span reach the UI. Go to Definition re-previews the target, refuses changed disk bytes or unsaved target tabs, retains Undo, and navigates by an exact CodeMirror selection. No server URI becomes a path capability. Native execution/acceptance is deferred.
- **INT02-E — Multi-file language session ownership (source implemented)**: one native OS-approved, pinned and supervised session owns up to 16 explicitly attached, live CodeMirror buffers within the same registered project and provider, bounded to 512 KiB per file and 2 MiB aggregate. The LSP protocol maintains per-file URI, version, pending request, diagnostics and didOpen/didChange/didClose ownership. Native IPC serializes admission, changes, queries and cleanup under a single mutex; additional files must match the original sealed project snapshot SHA-256. The editor offers **Analyze this file** within the approved scope, preserves sibling buffers on tab changes, detaches per-file analysis for close/reload/Save, stops the worker when the project changes, and blocks activity on uncertain cleanup. No authority expands to another project, provider, executable, or filesystem path; native runtime/installed acceptance remains deferred.
- **INT02-F — Language-server availability and lifecycle**: finish the end-to-end offline installed toolchain/bundle lifecycle for all three languages, diagnostics after worker interruption, restart and ownership reconciliation, and accessible source Problems/hover/completion presentation.
- **Deferred**: native Windows/Linux integration, exact-source compiler/protocol tests, installation/user approval, physical desktop accessibility, CI, PR review, whole-codebase repair and batch regression. All acceptance claims remain open until the implementation backlog has been built.

Next: **INT02-F — Language-server availability and lifecycle**. Continue feature implementation without GitHub Actions or PR operations. INT03 Build/Test follows after INT02 implementation slices.
