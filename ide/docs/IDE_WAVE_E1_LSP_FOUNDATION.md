# Lu IDE Wave E1 — Open-source Language Server Catalog and Safe LSP Boundary

**Status:** source implemented in a draft E1 branch, not native CI-accepted; no language server is launched. Wave D platform isolation remains unresolved. Lu IDE desktop Run and Lu Core bridge are still disabled.

## Why use LSP?

The Language Server Protocol is a local, language-agnostic JSON-RPC transport for IDE operations. It lets the existing standalone human editor and a future authorized Lu client share the **same native source objects, revision identifiers, diagnostics and results** instead of implementing separate code analyzers. LSP does not grant filesystem rights or worker execution authority.

## Fixed descriptive providers

| Provider | Language IDs | Supported focus | Open-source license | Upstream |
| --- | --- | --- | --- | --- |
| rust-analyzer | rust | completion, diagnostics, symbols, navigation | MIT OR Apache-2.0 | https://github.com/rust-lang/rust-analyzer |
| clangd | c, cpp | C/C++ completion, compile diagnostics and navigation | Apache-2.0 WITH LLVM-exception | https://clangd.llvm.org/ |
| Pyright | python | Python types, navigation, completion | MIT | https://github.com/microsoft/pyright |
| Ruff | python | Python lint, formatting and fixes; **not a Pyright replacement** | MIT | https://github.com/astral-sh/ruff |
| TypeScript Language Server | javascript, typescript and JSX/TSX | TS/JS language intelligence | MIT AND Apache-2.0 | https://github.com/typescript-language-server/typescript-language-server |
| VS Code extracted HTML / CSS / JSON | html, css/scss/less, json/jsonc | web markup and config language service | MIT | https://github.com/hrsh7th/vscode-langservers-extracted |

Free-to-use does not imply bundled, safe-to-execute, or no dependencies. Many servers are locally installed binaries; Pyright/TypeScript and extracted VS Code servers require a Node environment. Lu IDE must detect availability with the host-owned resource inventory, pin executable provenance, and **never automatically install or execute a server when merely opening a project**.

## E1 code

- `ide/host/src/lsp.rs`: a versioned, static, non-executable provider catalog; no dynamic shell args, PATH-based executable launching, downloads, working directory selection or runtime endpoint.
- `ide_list_language_servers`: read-only Tauri host command; `lsp.catalog` typed semantic capability plus JSON Schema v2 result shape, explicit `executableFromDesktop=false`.
- `FrameParser` and `encode_frame`: bounded in-memory `Content-Length` JSON-RPC 2.0 parsing/serialization (512 KiB body, 256-byte header). Incremental reads, multiple messages, rejected invalid/duplicate lengths, oversized headers, invalid JSON and arbitrary top-level data. These are pure source functions, not a process transport.
- `DiagnosticSource`: authoritative project/file ID, disk hash, editor-buffer revision and language ID. A diagnostic is stale if any coordinate changes; a URI supplied by a language server cannot authorize a file edit.
- Rust tests and a Node source-boundary test confirm known providers, non-executable discovery, framing, malformed input rejection and stale source identities.

## E2 — Required before starting any real server

1. Close Wave D's actual OS worker and network/secret-leak acceptance gates; do not create direct Tauri `Command::spawn` calls.
2. Host-only executable provenance with approved hashes and known configuration, verified worker privileges and containment before executing any downloaded or locally selected server.
3. Broker an LSP session with a bounded initialization, request/response IDs, LSP document synchronization, cancellation, shutdown, crash recovery, logs and resource caps. Only within the exact registered workspace ID and current source revision. Limit workspace/configuration requests and remote URLs.
4. Validate all server messages and edits. No server request can write files, execute commands, grant project access or modify buffers without separate IDE-native permission and revision checks.
5. Verify real rust-analyzer + Pyright + clangd + TypeScript LSP transcripts and Windows/Linux fixture tests with cancellation, restart, malicious/oversize replies, mixed UTF-8/UTF-16 positions, stale diagnostics and prompt-injection strings.
6. Provide human editor diagnostics, hover and completion **only after** native acceptance; future Lu access requires independent cross-app auth and approvals.

**This slice is catalog + wire contract, not a complete LSP client, installed server, or live language intelligence.**
