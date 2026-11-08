# Lu IDE — standalone development application

> **Current coordination (Nov 7 Phoenix / Nov 8 UTC):** Start with the [Product Readiness & Blocker Ledger](docs/IDE_PRODUCT_READINESS_BLOCKER_LEDGER_20261108.md), then the [P01–P07 Release Closeout Phase Ledger](docs/IDE_RELEASE_CLOSEOUT_P01_P07_20261107.md) and [Project State](PROJECT_STATE.md). P01 source consolidation is complete; P02–P06 native/release verification, P05 B2/B3, and the P06-R2 source matrix still have open blockers. P07/WO10 Lu Core bridge is deferred until after Core v1.0. The older R3/stage narrative below remains **historical**, not evidence of currently accepted installed capabilities.


**Current status:** Standalone advanced alpha; installed and protected Windows/Linux acceptance **OPEN**. Current IDE source is distributed across retained draft PR branches; a fully accepted single release candidate is not yet merged into `main`. Normal reviewed file edits are implemented; general project toolchain management and project debugging remain incomplete.  
**Repo:** `vjmichael/Lu-AI-GUI` · **Blocker plan branch:** `ide/product-readiness-blocker-ledger-20261108` (documentation-only, stacked on P06-R2 #111).

Lu IDE is its **own desktop application**, with its own process, project registry and native UI. It belongs in the same Git repository as Lu Core for convenience, but it **does not load inside Lu Desktop**, does not link against `lu-core`, and does not require Lu to run.

The future interface to Lu will be an optional, authenticated, versioned **out-of-process Protobuf bridge**. There is **no active bridge yet**; the IDE clearly displays the disconnected state.

## Workspace ownership

- `ide/app`: standalone Tauri 2 and React desktop, project UI and trusted OS directory picker.
- `ide/host`: standalone Rust project registry, scoped source browsing and explicit revision-checked atomic saves; no code execution.
- `ide/proto`: path-free, size-bounded versioned IDE lifecycle contracts.
- `ide/docs`: consolidated design, stage list, acceptance and decision records.
- `ide/workers`: future OS-restricted build, package, language-server and debugger workers.

`lu-desktop` and `src-tauri/lu-core` are **not dependencies** of the standalone IDE. Basic Git stays in Lu Core; advanced Git is an IDE responsibility. The IDE will later coordinate Python uv, C/C++ vcpkg/Conan, Cargo and other native managers behind one controlled interface.

## Development / tests

From the repository root:

```sh
cargo test --manifest-path ide/host/Cargo.toml --all-targets
cd ide/app
npm install
npm run check
# Once the platform's Tauri development prerequisites are available:
npm run tauri:dev
```

`ide/app` is designed to run on Linux and Windows. GitHub CI is a portable build check; real installed-app launch, native picker tests and hostile-code sandbox acceptance are separately required. Opening a project runs **no scripts**. R3 adds CodeMirror editing and Ctrl+S, guarded by source revision hashes and bounded native writes. Unsaved buffers remain memory-only; see [R3 editor acceptance](docs/IDE_03_R3_EDITOR_ACCEPTANCE.md). There are no executable project workers in this phase.

## Development continuity (required)

Start new work at [PROJECT_STATE.md](PROJECT_STATE.md), then [CHANGELOG.md](CHANGELOG.md) and [CONTINUITY_PROTOCOL.md](CONTINUITY_PROTOCOL.md). Do not assume GitHub is ahead of an uninspected local folder. IDE implementation PRs are checked by `.github/workflows/ide-continuity.yml` for accompanying changelog/current-state updates. A repository rule requiring that check is still needed for unbypassable enforcement. The latest R3.6/R3.7 source is not yet a verified Windows package; live Lu integration is deferred.

## Documents

- **Start here when resuming development:** [Engineering handoff and verified gates](ENGINEERING_HANDOFF.md).
- [R3.4 multi-hunk review and future proposal-envelope acceptance](docs/IDE_07_R34_MULTIHUNK_PROPOSALS.md).
- [R3.5 IDE-owned coding proposal inbox and manual editor staging](docs/IDE_07_R35_PROPOSAL_INBOX.md).
- [R3.3 reviewed changes / Lu coding foundation](docs/IDE_07_PROPOSAL_REVIEW_FOUNDATION.md).
- **Canonical design and eight-task list:** [IDE Workspace design, Section 6](docs/IDE_WORKSPACE_DESIGN.md).
- **Architecture decision:** [Standalone executable + optional Lu bridge](docs/ADR_0001_STANDALONE_APP_AND_LU_BRIDGE.md).
- **Workspace conversation distinction:** [Native Lu chat vs IDE coding chat vs NVP film chat](docs/ADR_0002_WORKSPACE_CHAT_BOUNDARIES.md).
- [System architecture](docs/ARCHITECTURE.md)
- [Master package manager](docs/PACKAGE_ORCHESTRATOR.md)
- [Security model](docs/SECURITY_MODEL.md)
- [Task acceptance subgates](docs/IMPLEMENTATION_GATES.md)
- [IDE-01B evidence](docs/IDE_01B_IMPLEMENTATION_EVIDENCE.md)
- [R3 code editor and save acceptance](docs/IDE_03_R3_EDITOR_ACCEPTANCE.md)

This branch contains experimental code only. No Lu Core integration or production security guarantee is implied.
