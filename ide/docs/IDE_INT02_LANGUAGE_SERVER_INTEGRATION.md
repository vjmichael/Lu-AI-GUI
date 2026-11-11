# IDE-INT-02 — Language Server Integration

Status: **feature implementation in progress; native acceptance deferred**. Continue on the existing IDE implementation branch, not a separate PR/CI queue. GitHub is source storage only during the build-first phase. No tests, CI, PR verification, broader codebase review or runtime claims are authorized in this development phase.

## INT02-A — Exact live CodeMirror source binding (source implemented)

- `lsp_live_source.js` captures the same active project/file, immutable buffer instance, saved disk SHA-256, buffer revision, and current CodeMirror UTF-16 text as the reviewed edit workflow. Native LSP request text is capped at 512 KiB UTF-8, rejects NUL and unsupported language IDs, and validates UTF-16 cursor positions before dispatch.
- Start, document change, diagnostics, completion, hover, and same-file definition now bind requests to this exact live source, not a potentially delayed React `tab.draft`.
- Every native reply must echo the exact source revision **and actual native selected provider**. Changed projects, tabs, undo buffers, saved bases, and newer edits invalidate old LSP replies rather than rendering them. Editing immediately hides stale diagnostic/hover/completion responses.
- Completion apply uses the captured exact source text. The existing CodeMirror transaction path independently checks document bytes immediately before application and preserves native editor Undo.

## INT02-B — Session lifecycle, worker cleanup, and Problems integration (source implemented)

- One native session is admitted only after an explicit OS permission dialog using the existing protected worker. The UI uses a synchronous busy reservation and lifecycle epoch to prevent duplicate starts and stale async publication.
- Project switching, closing/reopening files, saved base changes and buffer replacement stop the prior session. A failed stop is not treated as an exited process: it retains a retryable native-worker cleanup state that blocks new sessions.
- Debounced document changes and separately requested features reject responses after source or session changes. Native host still owns OS containment, release pins, workspace snapshot, and per-session human authorization.
- Per-file CodeMirror selections now update exact line/UTF-16-character coordinates. Existing source-verified Problems navigation remains read-only and no LSP-provided URI is treated as a file capability.
- Language Services panel now shows actual active session and protected cleanup controls, including when the previous project was switched away.

## INT02-C — Semantic Rust/C/C++ and optionally pinned Python integration (source implemented)

- Rust uses native rust-analyzer and C/C++ uses native clangd. Their existing supervised, explicitly approved transport is retained.
- Python continues to support Ruff diagnostics when only reviewed Ruff exists. A new optional standalone, offline **Pyright** route is allowlisted in both protected Windows and Linux worker recipes, checked through the same root-owned/pinned binary-tree constraints, and selected by the Tauri native host only if its exact installer SHA-256 pin matches.
- Only an actual pinned Pyright session can perform Python completion, hover, and same-file definition. Both the global native semantic-language inventory and per-response `providerId` are checked before the UI enables semantic operations. No user PATH, Node/npm download, shell execution, or permissive fallback.
- **Important limitation:** adding the provider route does not install, build, approve, attest, or prove a genuine working Pyright executable bundle. The separately reviewed complete offline executable wrapper and its compiled-in SHA-256 remain required before Python semantic features can become available.

## Remaining INT02 implementation / integration

- **INT02-D — Native source-bound cross-file definitions**: current protocol intentionally denies untrusted server file:// links except same-file results. Implement native registered-project source identity and revision binding for a different file before offering editor navigation.
- **INT02-E — Multi-file language session ownership**: extend the current single-file native session to safely cover multiple currently open project buffers, without reusing old approval for another project and without concurrent writes or losing worker containment.
- **INT02-F — Language-server availability and lifecycle**: finish the end-to-end offline installed toolchain/bundle lifecycle for all three languages, diagnostics after worker interruption, restart and ownership reconciliation, and accessible source Problems/hover/completion presentation.
- **Deferred**: native Windows/Linux integration, exact-source compiler/protocol tests, installation/user approval, physical desktop accessibility, CI, PR review, whole-codebase repair and batch regression. All acceptance claims remain open until the implementation backlog has been built.

Next: **INT02-D**. Keep working through feature implementation rather than returning to GitHub Actions or PR operations. INT03 Build/Test follows after the INT02 implementation slices.
