# Lu IDE DEV-02.1 — project Problems, exact-source navigation

**Status:** source-implemented only. Native Windows 2022 and Ubuntu 24.04 source tests and actual protected installed language-server acceptance **OPEN**.
**Base:** independent standalone `ide/p03-p04-native-closeout-20261107` at `dde157e82fa09376a0262e75bd70387c25f0cad2`. Not stacked on unaccepted DEV-01 candidate, not merged to `main`.

## Scope delivered

- The existing native-confirmed LSP diagnostic records are projected across **currently open tabs of the current project**, not across arbitrary unregistered filesystem paths or untrusted server-supplied URIs. The model helper `ide/app/web/lsp_problems.js` calls the existing `matchesSource` and `visibleDiagnostics` source guards: matching native project ID, registered relative path, saved disk SHA, editor buffer revision and language ID; invalid UTF-16 positions and excessively long diagnostic messages are refused.
- The editor toolbar adds **All problems**, an accessible project-scoped read-only panel with severity and message/filename filters, source-confirmed error/warning/info counts, explicit stale-source count, and a distinct **no verified diagnostics** state. Zero entries must **not** be presented as zero source errors when no LSP worker has returned verified data.
- Clicking a problem checks exact current source identity a **second time** and navigates only to an **already open** editor tab. It cannot open arbitrary server-provided file URLs, paths, command links, workspace edits or inferred source locations; the existing CodeMirror jump request receives the original tab's identity and a fresh event nonce.
- Read-only model bounded to 64 open tabs, 200 displayed diagnostics and 128-character filters. It does not spawn, find, start, approve or implicitly connect a language-server worker. No additional source-edit command, protected application grant, path resolver or network permission is exposed by this change.
- `ide/app/tests/dev02_project_problems.test.mjs` covers multi-file severity ordering, source revision mismatch, buffer edit, generation mismatch, cross-project isolation, malformed UTF-16 offsets, caps and filter inputs. UI contract test checks real editor wiring and absence of JS/IPC bypass. A dedicated exact-head Windows2022/Ubuntu24 source workflow runs all frontend Node tests and original native Rust LSP protocol tests with the immutable reviewed npm lock.

## Remaining DEV-02 requirements

1. Inspect exact-head native source jobs for real failure logs and repair any actual compiler/React/test issues. Test submissions do not imply passes.
2. After protected-worker BLK-06 acceptance, demonstrate **real Windows and Linux installed app** launches of reviewed clangd/rust-analyzer/Ruff from a registered project using independent OS consent and containment, language intelligence after real editing, stale-result cancellation and safe worker reaping.
3. Extend native host protocol to reviewed project-specific code navigation (definitions/references/symbols) only with host-validated registered relative paths, never blindly follow server `file://` URIs. Add robust per-language acceptance fixtures and UI action tests.
4. Complete automatic diagnostics on save/change, hover/completion, symbol navigation and source-following without implicit execution or invented available language servers. Current native LSP backend is feature-gated and Windows-only; this patch **does not** make it Linux-capable.
5. Resolve B2/B3 P05 and GTK/protected debugger and signed-release/legal provenance separately. Lu Core P07/WO10 integration stays deferred until user approval.

## Acceptance wording

Source-implemented is not live LSP or installed language-server acceptance. An actual real OS native result must be inspected at its exact head, followed by installed Windows/Linux protected-worker evidence. No unreviewed executable is copied, invoked, or granted new permissions by this task.
