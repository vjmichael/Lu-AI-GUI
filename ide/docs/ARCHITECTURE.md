# IDE Architecture — 2026-11-04

## Guiding principles
1. Usable by humans without AI: responsive multi-file editor, project explorer, tabs/splits, undo/redo, search/replace, symbols, diagnostics, terminal, debugger, tests, run configurations, source-control, accessibility and keyboard commands.
2. Lu adds transparent agent planning, contextual tool use, reviewable diffs, verified build/test feedback and durable task recovery. Model proposals never confer authority.
3. Prefer established protocols: Language Server Protocol (LSP) for language intelligence; Debug Adapter Protocol (DAP) for debugging; structured compiler/task/test diagnostics. Reuse proven editor components (evaluate Monaco or CodeMirror) rather than custom syntax engine.
4. Host-owned workspace handles and grants: React/WebView calls versioned Rust commands with opaque workspace/file/job handles. Host authenticates turn, checks requested operation and target, then invokes restricted workers through Protobuf IPC.
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
