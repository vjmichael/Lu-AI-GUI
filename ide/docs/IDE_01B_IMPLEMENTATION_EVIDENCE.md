# LU IDE — IDE-01B Native Host Implementation Evidence

**Date:** 2026-11-04  
**Canonical roadmap:** [Design Document, Section 6, Task 1](IDE_WORKSPACE_DESIGN.md)  
**Branch:** `ide/workspace-development`  
**Status:** **PARTIAL IMPLEMENTATION — native crate compiled and Linux tests passed; Tauri host and authenticated user-origin integration still pending.**

## Implemented in branch

- `ide/host/Cargo.toml`, `ide/host/build.rs` — independent Rust 1.88 IDE host crate depending on *existing* `lu-core`, with generated prost IPC.
- `ide/proto/lu/ide/v1/workspace.proto` — versioned and path-free UI lifecycle operations; host-minted project identifiers and bounded metadata replies. No tool execution, filesystem paths, grants, credentials or shell commands in protocol.
- `ide/host/src/lib.rs` — Lu Core `WorkspaceHost` registration for `lu.ide` with zero executable tools; native directory picker trait; canonical project root admission; bounded persistent metadata catalog; close/reopen/list; rollback on catalog write failure.
- `ide/host/src/wire.rs` — canonical Protobuf decoding/encoding, version checks, size limits, malformed-frame denial and stable error codes.
- `ide/host/tests/lifecycle.rs` — project lifecycle, restart, cancel, missing/changed root, metadata rollback, symlink and malformed UI request tests.
- `.github/workflows/ide-workspace-linux.yml` — Rust 1.88 Linux native compilation, tests and rustfmt gate. Read-only contents token; superseded workflow runs are cancelled.

## Verified evidence

- **Linux run:** https://github.com/vjmichael/Lu-AI-GUI/actions/runs/41961237114
- Source revision checked: `8de251a0d2cb0bbe82e37caac254ab50c14726c1`.
- Runner: GitHub-hosted Ubuntu Linux using Rust 1.88.
- **8 of 8 Rust tests passed** (2 protobuf unit tests, 6 lifecycle integration tests).
- **Rust formatting check passed** on committed source.
- This is a real Linux CI runner, **not** the user's separate local Linux development machine.
- Native tests verify registration and protocol behavior; they are **not** proof of runtime sandboxing.

## What this does not yet prove

1. No Tauri command is connected to a real OS directory picker; `NativeDirectoryPicker` currently has test implementations only.
2. No trusted conversation/UI origin is attached to an executable IDE action. The protocol is not an authority grant, and Task 1's full origin integration is still open.
3. Catalog root metadata is not a durable filesystem capability; any future file access requires Lu host-brokered root handles and revalidation.
4. No LSP, debugger, shell, build, tests, Git mutations, package installer or worker execution is enabled.
5. No Windows compile or physical Windows acceptance has been run for this IDE crate.
6. The standalone IDE crate has no checked-in lockfile yet; dependencies should be locked or brought under a common workspace lock before release.
7. No integration into the running Lu Tauri/React app or installed build has been tested.

## Next engineering work within Task 1

- Implement a trusted Tauri-native folder-selection adapter and IDE registry state with no path supplied by WebView.
- Bind lifecycle requests to authenticated UI/session origin and, where model-originated actions are introduced later, to Lu's persisted user-turn verifier and capability policy.
- Establish backward-compatible registration/recovery and tests against real Lu app-data storage.
- Add Windows compile check and record local Linux acceptance where available; verify Lu Core and IDE workspace version compatibility.
- Only after Task 1 acceptance proceed to Task 2's real Linux sandbox, restricted workers and adversarial process tests.

**Security note:** Passing lifecycle tests does not certify arbitrary package managers, code execution or native extension loading. Execution remains disabled in this component.
