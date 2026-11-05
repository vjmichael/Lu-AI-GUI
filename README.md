# Lu — Independent Desktop (Windows alpha development)

Lu is a standalone, locally hosted AI desktop application built with Tauri 2, React, Rust, and a pinned llama.cpp engine. This source tree is distinct from Jan's desktop, Cowork, plugin and inference execution paths. The optional legacy-format import helpers in Lu Core are data migration adapters only; they are not Jan runtime dependencies.

**Status (2026-11-03):** Lu `0.1.0-alpha.2` accepted by the project owner as the **functional Windows alpha development baseline**, following physical GGUF/MMProj, streaming, chat history, image/doc persistence, FIFO cancellation and Retry tests. It is **not** a production-stable or fully secured v1.0 release. Windows model-load `Not Responding`, missing taskbar icon, import feedback, signing and thumbnail timing remain tracked defects.

**Development status: start here.** [Central development ledger](docs/LU_DEVELOPMENT_LEDGER.md) (what exists on main versus draft PRs, exact source/CI evidence, next gates) · [CHANGELOG](CHANGELOG.md) (accepted milestones only) · [Alpha baseline and observed defects](docs/LU_ALPHA_01_ACCEPTED_BASELINE_20261103.md) · [v1.0 design requirements](docs/LU_V1_0_DESIGN_RECONCILIATION_20261103.md). Do not infer that draft code is installed or accepted. The historical Jan-mod design is subordinate to later Lu architecture and security decisions.

## Code map

- `lu-desktop/` — independent Tauri/React executable, model manager, chat UI, recovery and native host commands
- `src-tauri/lu-core/` — independent Lu Core APIs (ANSF, model admission, inference, authorization, etc.)
- `src-tauri/lu-win-worker-job/` — separate Windows process containment FFI boundary
- `docs/` — design, migration and acceptance notes

## Offline build

The desktop crate depends on the Lu Core crate through a **local relative path**, not the Jan application. Build frontend from `lu-desktop` with `npm run build`, then compile Windows `lu-desktop/src-tauri` using an installed MSVC toolchain and genuine Windows resource compiler; stage the verified pinned `llama-server.exe` engine as described in `lu-desktop/README.md`.

No model loads on startup merely because its GGUF was discovered. An explicit Load action is mandatory. New GGUF imports receive dedicated `lu-models/m-<32 lowercase hex>/` folders with their paired MMProj. Existing flat-layout models remain accessible without moving large files. The current development installer may require WebView2 to be installed externally; Windows acceptance remains mandatory.

## Repository note

This is a **clean working tree** branch, not a new Git repository. Its Git commit history still descends from the earlier Jan fork. For fully independent Git history, create a new empty `lu` repository and import this tree as its initial commit; keep the original Jan fork as a historical archive. GitHub is a source repository and optional Windows CI/package builder, not the primary development environment.

## Third-party provenance

The project retains appropriate Apache-2.0 licensing and notices for any code or documentation derived from the Jan fork. See `LICENSE` and `NOTICE`. Clean source layout does not erase upstream attribution.
