# IDE WO8 — Real Installed Application End-to-End Acceptance

**Started:** 2026-11-07 UTC. **Implementation in progress; no production acceptance claimed.** Branch `ide/wo8-installed-e2e-20261107` is stacked on WO7 draft PR #73; WO6 native debugger and WO7 signed protected-feature packaging/release remain separate open gates.

## Mission

Verify **what the installed standalone IDE actually does**, not just whether its source compiles or an NSIS setup runs. Require real native GUI interaction, native filesystem effects, permission refusals, persistence after process restart, cancellation and post-crash recovery, and eventual signed protected Run/Test/Debug on actual installed Windows. Linux installed-app acceptance is independent. No Docker; no mandatory Lu Core connection; no CLI-only substitute for a human UI.

## Work breakdown

| Task | Required installed evidence | State |
| --- | --- | --- |
| **WO8-A Native acceptance contract** | Exact installed binary+installer hashes, native UI process/window identity, OS+SHA, witnessed actions vs expected effects; no false release pass from source/fixture. | Started — this ledger, default-deny matrix |
| **WO8-B Windows unsigned standalone GUI** | Actual NSIS installation; live WebView UI Automation navigation and native folder picker; register fixture folder; open file; prove no automatic project-code execution; uninstall and verify cleanup. | Native UIAutomation harness + real installed NSIS CI implemented; OS job pending |
| **WO8-C Human editing and restart** | Type into real CodeMirror, verify file unchanged until explicit Save, inspect review and save, verify file SHA/text, close/reopen registered project, terminate/relaunch app, persistence and conflict rejection. | Editor review/save/restart scenarios implemented in native UI harness; external conflict and verified OS acceptance OPEN |
| **WO8-D Linux installed desktop** | Genuine .deb creation, installation, real graphical launch in OS session, native picker and project/file save/restart, uninstallation and no hidden worker. | True .deb + Xvfb/DBus/GTK window test and uninstall source implemented; GUI editing/picker and real OS acceptance OPEN |
| **WO8-E Installed protected feature** | From a signed exact reviewed WO7 package, actual per-turn native approval, fixed protected Cargo check/test, LLDB-DAP inspected stop/threads/stack, cancellation, host/service crash/restart, kernel Job/cgroup cleanup; negative denied paths before/after. | Manual exact signed candidate OS workflow + real UI approval/host workers source added; native acceptance, cancellations, crash/recovery OPEN; WO4–WO7 prerequisites |
| **WO8-F Acceptance and release evidence** | Machine-readable independent native witness for every gate: exact commit, package file digests, OS, process, operations, outcome, cleanup, CI job and evidence provenance; reject skipped/unknown/old receipts, no automatic shipping. | Fail-closed coverage/provenance checker + default deny inventory and hostile unit tests implemented; independent evidence retrieval/approval OPEN |

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

## Latest development checkpoint — 2026-11-07 UTC

The following source is committed on draft PR #74, but native OS and installed approvals remain independently OPEN:

- **Windows:** `Test-Wo8NativeDesktopUI.ps1` now provides four independently scoped test scenarios plus manual protected build/debug: Shell, ProjectReadOnly, EditorPersistence, PersistedRestart, ProtectedBuildDebug. It finds the actual OS-level accessible CodeMirror textbox, types with keyboard input, compares source hash before an explicit Save, reviews a real change, saves via UI, restarts, and reopens the registered source. `ide-wo8-installed-windows.yml` uses a real unsigned NSIS, never a mocked Tauri frontend.
- **Linux:** `tauri.wo8.linux.conf.json`, `Test-Wo8InstalledDeb.sh`, `wo8_linux_window_acceptance.sh` and `ide-wo8-installed-linux.yml` build/install genuine Linux .deb, verify Debian package identity and exact commit, launch installed GTK under DBus/Xvfb with a visible X11 window, reject unexpected language/debug workers, and uninstall. This does **not** yet prove a Linux editor save, folder picker or native protected worker.
- **Protected signed candidate:** `ide-wo8-signed-protected-e2e.yml` is **manual and gated** on an independently signed WO7 candidate run with exact SHA. Its native UI tests click the actual fixed recipe and one-use consent dialogs, then require real installed protected Cargo output and LLDB stopped thread/stack/cleanup UI observations. It cannot run without a real package; cancellation/crash recovery and negative hostile source-swap remain extra outstanding tasks.
- **Provenance:** `verify_wo8_installed_evidence.py` and `wo8-installed-evidence.json` require exact-head, OS/workflow-specific, digest-pinned native witnesses for the entire WO8 matrix. Even complete local JSON only proves **metadata consistency**, not that those GitHub jobs or runtime witnesses were fetched and trusted. `test_verify_wo8_installed_evidence.py` covers stale/skipped/wrong-OS/unsigned claims; `ide-wo8-source-safety.yml` checks Python fixtures, Bash syntax and native PowerShell UIA syntax.

**Known verification boundary:** native Windows and Linux jobs have been scheduled, but passing output is not yet credited. WO7 signed package, native WO6 LLDB-DAP and actual installed protected Run/Test/Debug are not declared production accepted. There is no release artifact or installer hash in this ledger.

## WO8 native failure repair / installed recovery extension (2026-11-07 UTC)

**Concrete inherited CI defect repaired:** WO7's final preflight expanded from two to three signed executables (installer, service and desktop), but the unit-test setup only created two fixtures. Both Windows/Linux WO7 CI aborted with `FileNotFoundError: desktop.exe` before exercising signature rejection. Added a correctly independent desktop test fixture on **WO7 PR #73** and carried the exact correction onto **WO8 PR #74**. This fixes test setup without bypassing signature verification or approval.

**Windows installed editing and collisions:** `Test-Wo8NativeDesktopUI.ps1` adds `ExternalConflict`: enter an unsaved editor draft, mutate the real source file from an external OS writer, click the installed IDE Save button, require the revision-conflict alert, prove the external file SHA is preserved and the editor draft remains unsaved. `ide-wo8-installed-windows.yml` now runs this after the real Save/restart scenarios and validates final disk contents. The failure is never treated as a successful Save.

**Linux installed editor-level E2E:** `wo8_linux_editor_atspi.py` uses native AT-SPI accessibility and X11 keyboard events under a real installed Tauri GTK WebKit app; it registers a fixture through the OS folder picker, edits CodeMirror, verifies the file is untouched until explicit review and Save, terminates/relaunches the installed app with the persisted project catalog, then proves an external writer is not overwritten by a stale IDE draft. It denies missing native controls; no source-only mock can pass. `Test-Wo8InstalledDeb.sh` and `ide-wo8-installed-linux.yml` now install `python3-pyatspi`/accessibility dependencies and execute these against the installed Debian package before uninstalling.

**Signed installed protected cancellation/crash/recovery:** The real OS UIA fixture now includes `ProtectedCancellation`, `ProtectedHostCrash`, `ProtectedServiceCrash`, `ProtectedTamperDenial`. It requires a genuinely Authenticode-valid installed app, pinned LocalService image and identity, native one-use user confirmation, an **observed real Cargo process**, and an exact active job ID. It cancels through the UI or kills the actual installed desktop/verified recovery service; it demands bounded OS-process cleanup and an exact persisted job receipt/reconciliation before claiming success. It also modifies committed Rust source **while the native approval dialog is held** and requires refusal before worker admission. The manual `ide-wo8-protected-recovery.yml` stages a genuinely signed WO7 candidate, uses a finite disposable 90-second offline Rust integration test to exercise the interruption, invokes all four scenarios, and requires verified service uninstall. The existing unsigned default NSIS is explicitly excluded. If the signed candidate, service, protected worker, consent or durable OS recovery is unavailable, this workflow **fails or cannot start**; no fake green.

**Inspectability and coverage:** The installed Jobs view now displays bounded recent native receipts including job ID/phase/source snapshot and native error, allowing crash-restart reconciliation to refer to the exact job. `verify_wo8_installed_evidence.py` now requires Windows and Linux actual external edit/restart observations and **four individually named** protected native interruption gates, all bound to the correct native runner/workflow and commit. Checked-in evidence remains empty and release denied.

**Not yet accepted:** The new exact-head native Windows and Linux installed runs must finish and any genuine compilation, accessibility, packaging, service or runtime failures must be repaired. The manual signed protected workflow has not been run because an approved signed WO7 candidate and native debugger/worker prerequisites have not been established. WO8 acceptance is not a release approval.
