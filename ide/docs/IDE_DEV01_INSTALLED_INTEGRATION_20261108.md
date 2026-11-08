# Lu IDE DEV-01.3–01.6 — actual installed Windows/Linux integration acceptance

**Status:** Genuine installed-app acceptance workflow IMPLEMENTED but **NOT ACCEPTED**. Windows/Linux jobs must finish successfully at the exact integration head. This is a stacked draft on DEV-01.6 PR #126; no release-candidate merge, protected worker gate, signing or legal release approval.

## Strict test design

The dedicated `.github/workflows/ide-dev01-installed-integration-ci.yml` builds one real unsigned Windows 2022 NSIS package and one Ubuntu 24.04 Debian package from their exact source SHA, verifying the existing immutable Cargo/npm lock evidence and upstream original third-party dependency license inventory. It installs each package into a disposable runner, launches its **actual installed** Tauri/WebView desktop and selects a temporary project through the real OS picker.

**Windows** extends the existing proven UIAutomation `Test-Wo8NativeDesktopUI.ps1` harness with two separate scenarios: `Dev01Integrated` and `Dev01RecoveryRestart`. Both require the installed binary SHA256 and the real native WebView control tree. **Linux** uses `dev01_linux_installed_atspi.py` with the real installed `/usr/bin/lu-ide-desktop`, AT-SPI accessibility and X11 keyboard/pointer actions, reusing the existing native GTK picker witness. No harness directly invokes Tauri commands, uses JS injection, browser storage, or synthetically updates editor state.

### Per-OS full acceptance sequence

1. From the native file picker, register a disposable project containing real `main.rs`, `side.rs`, `move-me.rs` and **864** source files across 24 folders (**867 indexed files total**). Open **Ctrl+P Quick Open** through OS UI and require the actual native index to report 867 files.
2. Open and explicitly close `move-me.rs`, use the installed Files pane **Rename / move**, provide `renamed.rs` as an existing-parent project-relative path, and confirm physical source disappearance, destination creation and exactly preserved SHA256 bytes. No direct filesystem write is used as an action shortcut.
3. Select and close the renamed file; invoke native **Trash file**, confirm through a real OS-owned browser confirmation with exact text, require the actual project-local durable trash record and payload, click **Project trash → Restore**, confirm and require original bytes restored with no stale payload.
4. Open `main.rs` and `side.rs`, enable **Split editor**, prove both primary and secondary native accessible editor panes, type in the secondary CodeMirror, choose **Save secondary** and verify real disk contents changed only after this explicit action.
5. Type a second, distinct **unsaved** draft in the secondary pane, require no disk mutation and a visible current-session native recovery record, then forcibly terminate the installed desktop process.
6. Start a genuine **new** desktop application process, reopen the registered project through the app's own UI and require a **previous-session** draft warning. Select the saved checkpoint into an unsaved editor tab; verify on-disk source still untouched, then explicitly Save and prove new draft contents appeared. Finally uninstall the package and verify installed artifacts have been removed.

All stages fail closed. Intermediate log markers do not constitute full acceptance. Only the final exact-head Windows `DEV01_INSTALLED_WINDOWS_ALL_FOUR_AND_UNINSTALL=PASS` or Linux `DEV01_LINUX_REAL_DEBIAN_ALL_FOUR=PASS` markers, together with actual GitHub OS job SUCCESS, can support that platform's integrated GUI acceptance.

## Outstanding additional gates and known limitations

- **As of workflow creation, no current-head DEV-01.3–01.6 source jobs had finished**; Windows/Linux runners were queued. A concrete cap-std Unix import mismatch in DEV-01.4 source was corrected on #124 and carried forward through #125/#126 and this integration branch without discarding stacked ancestry. This is a source repair, *not* an observed CI pass.
- The Linux integration will **fail** if the current native GTK folder chooser still does not close, just as the outstanding BLK-05 gate should. No fixture catches/skips this or invokes an internal project-register command.
- Unexpected WebView browser confirmation accessibility failures, editor selection, OS package build failures and platform-specific hard-link semantics remain open until actual job logs are inspected and repaired.
- The instrumented process kill verifies app-session draft restoration after a checkpoint; additional **crash-time stage-specific** native transaction fault injection (journal created, hard-link created, original unlinked), simultaneous external writers, disk-full testing and private-vault ACL review are separate gates.
- Performance acceptance requires a repeated same-machine comparison (original 24-dir sequential UI loop versus bounded native index), cold/warm memory and p50/p95 timings, 10k+ project partial-index user experience, typing responsiveness and OS cancellation. A single run with 867 files is a functional integration test, not a performance victory.
- Trash and rename remain **file only**. Directory-tree no-clobber move and rollback are not implemented; no claim is made about signed commercial redistributable installers or legal publisher identities. Lu Core/P07 integration deferred.

## Ownership

- DEV-01.3 PR #123: native file-only no-clobber transactions and recoverable project trash
- DEV-01.4 PR #124: app-data-only checkpointed unsaved buffers, prior-session authorization
- DEV-01.5 PR #125: split editor and original draft ownership
- DEV-01.6 PR #126: one native bounded filename index and measured OS timing
- Current stacked installed acceptance: `ide/dev01-installed-integration-20261108`.

Never merge to a release candidate merely because an acceptance workflow exists or a job starts. Require completed exact-head Windows/Linux OS jobs and repair their *specific* actual failures first.
