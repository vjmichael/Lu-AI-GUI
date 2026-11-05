# Lu IDE — standalone development application

**Status:** R3.3 Windows smoke accepted; R3.4 candidate adds accurate multi-hunk reviews and strict, data-only coding proposal envelopes. Package manager, executable workers and Lu bridge remain disabled.  
**Repo:** `vjmichael/Lu-AI-GUI` · **IDE-only branch:** `ide/workspace-development`

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

## Documents

- **Start here when resuming development:** [Engineering handoff and verified gates](ENGINEERING_HANDOFF.md).
- [R3.4 multi-hunk review and future proposal-envelope acceptance](docs/IDE_07_R34_MULTIHUNK_PROPOSALS.md).
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
