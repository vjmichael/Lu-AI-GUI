# IDE-01B — Installed desktop acceptance procedure

**Status:** **pending actual human desktop validation**, as of 2026-11-04.
This checklist is for the independent **Lu IDE** executable, not the Lu chat application.

## Verified unsigned alpha artifact

- Windows release build: https://github.com/vjmichael/Lu-AI-GUI/actions/runs/41966041935
- Packaged ZIP: `Lu-IDE-Windows-Alpha.zip` (single `lu-ide-desktop.exe`).
- ZIP SHA-256: `4fd520c093d85cbeaab4f8527f8ab555cecdd4b80abbd9aaabb9121b60966cd4`.
- EXE SHA-256: `0562fc3dab1c4b4d5ae0a242d43656c539cd7cf2822945b066ad11050cf8fbdb`.
- The ZIP was inspected: the executable is a Windows PE file with `MZ` header, size 9,974,272 bytes.
- **Unsigned**, not an installer, not manually launched in Windows desktop acceptance; Windows may display publisher warnings. Do not treat this as a production-security claim.

## Setup

1. Obtain the unsigned Windows alpha executable from the [standalone executable CI workflow](https://github.com/vjmichael/Lu-AI-GUI/actions/workflows/ide-alpha-windows.yml) after a successful run, or compile `ide/app` locally with Rust 1.90 and Node 22.
2. Work in a disposable test folder (e.g. `Desktop/LuIDE-Test-Project`); do not open credentials directories or a sensitive production repository during the first acceptance run.
3. Place a harmless `README.md`, `test.py` and `package.json` in that directory. Do not execute anything. The current UI should not run package scripts or language servers.
4. Confirm Lu Desktop is not required to be running.

## Required manual checks

- [ ] Launch `lu-ide-desktop.exe` normally, without Lu Desktop.
- [ ] Distinct **Lu IDE** window and project UI appear; no chat model, login, or Lu inference initialization.
- [ ] The footer states **Lu connection: Not connected**.
- [ ] Select **+ Open folder**, observe the *native Windows folder picker*, select disposable project.
- [ ] Project appears by display name with opaque project ID; no absolute project path appears in visible project cards.
- [ ] Click **Close**, then **Reopen**, and observe the state change.
- [ ] Close the application, relaunch, and verify the registered project persists but is not marked active until reopened.
- [ ] Open picker and cancel: no new project is registered.
- [ ] Rename/move the selected directory externally, refresh: project becomes unavailable and cannot reopen silently.
- [ ] Verify no editor, terminal, dependency install, LSP, build or model worker starts just by opening the project.
- [ ] Confirm existing Lu Desktop launches and behaves independently; project selection in IDE does not write Lu model/chat state.
- [ ] Inspect for UI errors and record the exact executable SHA-256, Windows version, test result, and any crash/reopen behavior.

## Technical checks remaining beyond manual UI

- Actual OS dialog cancellation/reopen races and filesystem-path swap attacks.
- Directory identity across rename/replacement; durable scoped file handles.
- Real packaged installer (NSIS) with install/uninstall and an OS-supported signed/untrusted-binary flow.
- Authenticated, versioned out-of-process Lu bridge, when contracts are agreed.
- Stage 2 kernel-verified Linux/Windows execution containment. **There is currently no project-code execution in this alpha.**

## Acceptance recording

Mark IDE-01B **desktop-validated** only after results are collected for a specific build SHA and OS. A successful Cargo compilation or synthetic project-registry test is not a real desktop/picker acceptance.
