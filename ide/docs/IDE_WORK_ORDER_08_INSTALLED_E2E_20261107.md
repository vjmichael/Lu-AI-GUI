# IDE WO8 — Real Installed Application End-to-End Acceptance

**Started:** 2026-11-07 UTC. **Implementation in progress; no production acceptance claimed.** Branch `ide/wo8-installed-e2e-20261107` is stacked on WO7 draft PR #73; WO6 native debugger and WO7 signed protected-feature packaging/release remain separate open gates.

## Mission

Verify **what the installed standalone IDE actually does**, not just whether its source compiles or an NSIS setup runs. Require real native GUI interaction, native filesystem effects, permission refusals, persistence after process restart, cancellation and post-crash recovery, and eventual signed protected Run/Test/Debug on actual installed Windows. Linux installed-app acceptance is independent. No Docker; no mandatory Lu Core connection; no CLI-only substitute for a human UI.

## Work breakdown

| Task | Required installed evidence | State |
| --- | --- | --- |
| **WO8-A Native acceptance contract** | Exact installed binary+installer hashes, native UI process/window identity, OS+SHA, witnessed actions vs expected effects; no false release pass from source/fixture. | Started — this ledger, default-deny matrix |
| **WO8-B Windows unsigned standalone GUI** | Actual NSIS installation; live WebView UI Automation navigation and native folder picker; register fixture folder; open file; prove no automatic project-code execution; uninstall and verify cleanup. | Source harness/workflow added; native CI pending |
| **WO8-C Human editing and restart** | Type into real CodeMirror, verify file unchanged until explicit Save, inspect review and save, verify file SHA/text, close/reopen registered project, terminate/relaunch app, persistence and conflict rejection. | OPEN |
| **WO8-D Linux installed desktop** | Genuine .deb creation, installation, real graphical launch in OS session, native picker and project/file save/restart, uninstallation and no hidden worker. | OPEN |
| **WO8-E Installed protected feature** | From a signed exact reviewed WO7 package, actual per-turn native approval, fixed protected Cargo check/test, LLDB-DAP inspected stop/threads/stack, cancellation, host/service crash/restart, kernel Job/cgroup cleanup; negative denied paths before/after. | OPEN; WO4–WO7 OS and signature prerequisites |
| **WO8-F Acceptance and release evidence** | Machine-readable independent native witness for every gate: exact commit, package file digests, OS, process, operations, outcome, cleanup, CI job and evidence provenance; reject skipped/unknown/old receipts, no automatic shipping. | OPEN |

## Boundaries

- Existing default unsigned NSIS smoke proved only install, two launches and uninstall. It does not prove human UI use, project picker, editor, native Run/Test/Debug, signed service or release.
- WO8-B uses **Windows UI Automation against the installed WebView** with real OS button invocations; neither React unit rendering, hidden web debug ports nor a simulated Tauri host qualifies.
- Never use a synthetic UI success if the automation tree, folder chooser or saved file cannot be inspected; report failure and repair. The installed app's system folder picker must remain the only source of trusted workspace roots.
- Source presence, unsigned NSIS, or green hosted CI cannot imply arbitrary untrusted project execution is safe. No Lu IPC bridge, unrestricted terminal or silent installation.
- W07's source-complete signed candidate pipeline remains **release unaccepted** pending actual signed approved binaries, licenses, service/installer OS lifecycle and protected-feature tests.
- Linux .deb and Windows NSIS may have distinct native OS acceptance; do not generalize across platforms.

## Implemented WO8 files

- `ide/app/fixtures/Test-Wo8NativeDesktopUI.ps1`: real native Windows UI Automation, installed binary SHA, WebView navigation, OS picker, fixture project and source open, verified absence of unexpected automatic workers, bounded cleanup. `Shell` and `ProjectReadOnly` are separate native scenarios.
- `.github/workflows/ide-wo8-installed-windows.yml`: builds and installs actual unsigned NSIS, runs native Windows UI scenarios against an ephemeral fixture, uninstalls, and labels exactly what passed. The default secure unsigned build remains without bundled LSP/debugger/worker.
- Follow-on UI save/restart and Linux native acceptance workflows are WO8-C/D, not assumed.

## Current release posture

WO8 may advance in parallel with WO7 release-approval operations. Real native installed-app acceptance must be observed independently. Keep WO8 PR draft until corresponding OS and signed-feature gates are met.
