# IDE-01B — Standalone Rust host and desktop shell evidence

**Date:** 2026-11-04  
**Branch:** `ide/workspace-development`  
**Canonical Task:** [Task 1 in the design document](IDE_WORKSPACE_DESIGN.md)  
**Architecture decision:** [ADR-0001](ADR_0001_STANDALONE_APP_AND_LU_BRIDGE.md)

## Scope after the standalone pivot

IDE-01B is now an **independent IDE executable** foundation, not an embedded component of Lu Desktop.

- `ide/host`: Rust workspace metadata catalog, trusted native picker trait, host-generated project IDs, close/reopen/list lifecycle; **no link to `lu-core`**.
- `ide/proto/lu/ide/v1/workspace.proto`: local IDE lifecycle Protobuf request/reply; IDs and display names only; no filesystem paths, commands or grants.
- `ide/app/src-tauri`: independent Tauri binary `lu-ide-desktop`, own application-data catalog, native OS folder chooser and four project lifecycle commands.
- `ide/app/web`: standalone React project-management UI with disconnected Lu status and disabled not-yet-implemented IDE sections.
- `.github/workflows/ide-standalone.yml`: Linux Rust host tests and rustfmt, frontend build/tests, Windows Rust host tests and Tauri desktop `cargo check`.

## Evidence and status

- Earlier independent host Linux Rust native tests: https://github.com/vjmichael/Lu-AI-GUI/actions/runs/41963552544 (successful).
- First standalone front-end and Linux host test run: https://github.com/vjmichael/Lu-AI-GUI/actions/runs/41963904882 (those portions succeeded; Windows web build script initially failed when invoking Unix-style `esbuild`, corrected in commit `47c7e44674c77cdd545f0881daf909c93f0cb8e1`).
- Successful standalone Linux/React/Windows matrix: https://github.com/vjmichael/Lu-AI-GUI/actions/runs/41965250525 (Windows Rust/Tauri `cargo check`, Linux host native tests and standalone React build/tests all passed for source head `2e40a1826a57a03570c9e004f9e9730b11f30f83`).
- Successful standalone Windows **release executable** build: https://github.com/vjmichael/Lu-AI-GUI/actions/runs/41966041935. The ZIP and EXE checksums are recorded in [manual desktop acceptance](IDE_01B_MANUAL_DESKTOP_ACCEPTANCE.md).
- The IDE source remains confined to `ide/`; no Lu Core or Lu Desktop code is changed on this branch (verified by GitHub compare against `main`).
- No claim of real installed Windows launch, manual native picker acceptance, sandbox protection or working Lu connection. These remain open.

## What was superseded

The earlier experiments integrating `ide_workspace.rs`, a panel, and shared `WorkspaceHost` directly into `lu-desktop` have been rolled back on this branch. No IDE feature is injected into the Lu app. The `lu-core` native registry is not a dependency of the standalone host.

## Acceptance still required

1. **Verified:** Native Linux and Windows Rust host unit/integration tests for the cited source revision.
2. **Verified:** Standalone frontend build/tests, Windows Tauri `cargo check`, and unsigned Windows release binary creation for cited revisions.
3. **Pending:** Native installed-app launch and folder picker response, including cancel, close, reopen, and restart; independent checks on Windows and Linux.
4. Secure, versioned Lu bridge discovery/authentication, only when both sides have a stable contract. No implicit privilege sharing.
5. Before running any project code, separately pass Task 2 OS sandbox and hostile-project tests.

**Status:** Task 1 in progress; no executable tools enabled.
