# Lu IDE Workspace — dedicated development branch

Status: **architecture and security design**, not an integrated release.
Branch: `ide/workspace-development` in `vjmichael/Lu-AI-GUI`.
This branch contains the IDE DLC only; it must not silently change core runtime or accepted alpha artifacts.

## Mission
A native, Lu-assisted IDE for opening software projects; reading/editing code; navigating symbols; reviewing diffs; running builds and tests; debugging; managing project toolchains/dependencies; and using advanced local/remote Git. Long term: develop and validate Lu itself in disposable development instances.

## Repository and ownership
- Keep IDE under `ide/` in this same repository; do not create another GitHub repository.
- Lu Core owns model inference, memory, trusted human-turn identity, authority grants, durable journal, worker supervision and basic Git primitives.
- IDE owns source/editor UI, language adapters, debugging, package-manager orchestration, project build/test recipes, advanced Git operations and GitHub/gh integration.
- Reuse existing Rust/Protobuf host-worker contracts with versioned explicit extension API; **do not** create an alternate authorization or execution bypass.
- IDE branch is not a process-security boundary. Future merges into main require compatibility and acceptance evidence.
- Linux is the primary development/test environment for the initial IDE; Windows parity is required before release. GitHub is repository storage and optional CI, not the only test environment.

## Planned modules
`ide/ui`: file tree, editor, panes, diagnostics, symbol search, source-control, testing, package center, task console.
`ide/host`: Rust workspace registry, path-safe file operations, authenticated tool dispatch, workspace trust, manager adapter registry, git operations.
`ide/proto`: versioned IDE service and event contracts (host-minted IDs, no privileged raw paths from WebView).
`ide/workers`: LSP/DAP/build/test/package/git worker lifecycle under host supervision and OS isolation.
`ide/tests`: unit, integration, adversarial security and Windows/Linux native acceptance.

## Scope boundaries
- Prototype IDE-01A was described as a standalone Python Git read/inspection prototype; it is **not claimed to be integrated** into Lu or uploaded to this branch.
- Core basic Git stays in Lu Core. Interactive rebase, cherry-pick, conflict resolution, bisect, worktrees, PR/CI review and credential-aware remote Git stay in IDE.
- No automatic running of repository-defined scripts, extensions, package lifecycle hooks, debug adapters or AI-written commands upon opening.
- No Docker prerequisite or multi-installer requirement for end users. Host tools may be detected; provisioning needs explicit consent. Optional higher-isolation VM can be used for risky code.

See [architecture](docs/ARCHITECTURE.md), [package manager](docs/PACKAGE_ORCHESTRATOR.md), [security](docs/SECURITY_MODEL.md), and [gates](docs/IMPLEMENTATION_GATES.md).
