# IDE Architecture — standalone app, 2026-11-04

**Binding decision:** [ADR-0001 — standalone app and optional Lu bridge](ADR_0001_STANDALONE_APP_AND_LU_BRIDGE.md). The IDE's own Tauri host is the authority for IDE project permissions; Lu Core is **not** a required dependency.

## Guiding principles
1. Usable by humans without AI: responsive multi-file editor, project explorer, tabs/splits, undo/redo, search/replace, symbols, diagnostics, terminal, debugger, tests, run configurations, source-control, accessibility and keyboard commands.
2. The IDE runs independently. When available, Lu may add agent planning and diff proposals over a future authenticated out-of-process bridge; the IDE validates requested operations itself. Model proposals never confer authority.
3. Prefer established protocols: Language Server Protocol (LSP) for language intelligence; Debug Adapter Protocol (DAP) for debugging; structured compiler/task/test diagnostics. Reuse proven editor components (evaluate Monaco or CodeMirror) rather than custom syntax engine.
4. Host-owned workspace handles and grants: the standalone React WebView calls the IDE's versioned Rust commands with opaque workspace/file/job handles. Its native host checks trusted UI intent and scope before restricted worker execution. Lu-originated requests will need separate peer authentication and grants.
5. Separate the trusted IDE shell from project-supplied code and extensions. LSP servers, test runners, package managers, language runtimes and debug adapters can execute arbitrary code and must be classified as untrusted workers.
6. Keep snapshot/diff previews and Git worktrees for staged changes. Preserve user work before destructive cleanup; transactions and durable journal for IDE-owned effects.
7. Filesystem events, streaming logs, cancellation, resource quotas, recovery and progress should be first-class, not bolted on.
8. Maintain Linux and Windows backends behind common host contracts. Worker isolation differs by OS; do not claim parity from API-level tests.

## Workspace lifecycle
REGISTER (user selects directory through trusted OS picker) -> INDEX_READ_ONLY -> REVIEW_TRUST -> ACTIVATE_WORKERS (explicit approval) -> EDIT/BUILD/TEST -> CLOSE/RECOVER.
Unknown repositories default to restricted read-only *execution* mode; text editing can be allowed within a granted project root while automatic execution is denied.
Child processes inherit a minimal environment, no ambient credentials, sanitized environment variables, restricted mounts/network/process scope and disposable temp directories.
Persist per-project provenance, trusted revision, grants, toolchain selection, lockfile digests and worker receipts.

## User workflow
Open project -> inspect manifest/source tree -> choose per-project toolchain -> preview dependency plan -> approve install permissions -> edit/agent changes in reviewable diff -> format/lint/test in isolated worker -> inspect diagnostics -> commit through explicit advanced Git interface.

## Git responsibilities
Core: simple repository inspection and explicitly authorized basic operations.
IDE: worktrees, branch compare, stage hunks, commits, amend, cherry-pick, revert, interactive rebase, stash, bisect, conflict editor, blame/history, signed commits, PR review, GitHub/gh tools and CI inspection. Destructive and remote operations need destination-aware confirmation. Never parse arbitrary AI output into shell commands.

## Open decisions (not implemented)
Editor component choice, initial supported LSPs/DAPs, exact Rust/Protobuf integration points, isolation backend by distro, Windows AppContainer/low-privilege architecture, credential broker, manager toolchain provisioning and extension marketplace trust.

## Standalone launch and Lu relationship

The production process is `ide/app` (Lu IDE). `ide/host` compiles without `lu-core`. `lu-desktop` does not embed, load or manage this app. The UI must show disconnected state until a verified local IPC session is negotiated. An IPC handshake is **not yet implemented**; do not claim it exists because `ide/proto` has a local lifecycle schema.
