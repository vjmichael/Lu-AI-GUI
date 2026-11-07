# Lu IDE — P02 Deterministic Native Build & Test Repair

**Date:** November 7, 2026. **Phase:** P02 only; no P03 implementation.
**Parent:** P01 RC branch `ide/release-closeout-p01-20261107` at `a8b0a8f0270856954acd1132e30477e387004c98`. **Working branch:** `ide/release-closeout-p02-20261107`, stacked on P01, preserving Lu Core `main` and excluding deferred WO10 bridge.

## Audited failures at the exact parent source

1. [WO9 native workflow run 42355877340](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42355877340): Windows job attempted `tauri-build` without `ide/app/src-tauri/icons/icon.ico`; Linux `tauri::generate_context!()` panicked because `../dist` had not been built. These were build-order defects, not evidence of protected OS feature rejection.
2. WO9 UI frontend job in the same run and WO8/WO6 frontend jobs: 65 of 67 Node tests passed. The two failures were `standalone.test.mjs` strict invoke/capability assertions not accounting for the already-implemented `ide_workflow_verify_native_receipt` command. That route is native evidence verification and **not** a general-purpose Lu capability; do not enlarge `capability_schemas.json`, IPC authority or tool grants to fix the test.
3. [WO4 source workflow run 42355877379](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42355877379) and the E1/E2 jobs: rustfmt rejected inherited WO9 host files and delegated Linux test binaries. Exact old/new formatting hunks were extracted from failed native CI logs and applied to the tracked files, with no behavioral source changes intended.
4. The actual Linux accessibility helper `wo8_linux_editor_atspi.py` declares `activate(..., timeout=...)`; two WO9 scripts called it with an invalid `seconds` argument. The helper's separate `accessible(..., seconds=...)` remains correct.
5. [WO8 earlier installed Linux run 42348625773](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42348625773) really built and installed the Debian package; its GTK native test then failed at `activate() got an unexpected keyword argument 'seconds'`. The WO8 helper's *own* native picker caller was also repaired in P02. A dedicated AST regression now validates all three WO8/WO9 scripts' activation keywords, but only a fresh installed test can prove functionality. [WO8 earlier installed Windows run 42348625788](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42348625788) passed 67/67 Node tests and reached real UI Automation, then failed `WO8_NATIVE_UI_DENIED:ui_element_not_exposed_Execution_requests`. The P02 harness now emits up to 100 real UIA node names and types on missing-control failure **before still denying acceptance**. Do not assume static JSX label matches guarantee an accessible installed WebView2 control.

## Source changes on the P02 branch

- WO9 native-feature CI generates the real first-party ICO/PNG and bundled frontend before Cargo, on Windows and Linux.
- Strict UI command tests recognize the existing privileged receipt command while deliberately excluding it from the ordinary human/AI capability catalog.
- The Linux WO8 native picker and WO9 accessibility calls use the declared `timeout` keyword, guarded by a Python AST signature contract.
- Seven Rust host/native fixture files received exact rustfmt repairs from observed CI diffs.
- New `.github/workflows/ide-p02-native-checks.yml` checks checked-out SHA, asset existence, Node tests, Rustfmt, native host tests, default Tauri and separately opted-in `native-build`, `native-debug`, `native-lsp` and their combined declaration on each OS. A Python AST helper signature check and Python/PowerShell syntax checks guard the GUI fixtures; **none is simulated installed acceptance**. Windows UIA now reports a bounded real accessibility tree on a missing-selector failure while remaining fail-closed.

## Verification and phase exit conditions

**Status: source repairs committed; pending exact-head native CI and installed OS interaction.** Obtain actual Win/Linux job results for the P02 PR SHA and fix every fresh failure. Run genuine installed Linux Debian/GTK/AT-SPI and Windows NSIS/WebView2 UI Automation, including the navigation and native picker, to verify what controls the OS really exposes. Error output/absence of access must fail closed; never substitute a mocked accessible label or browser-only test for OS evidence.

P02 stays OPEN until accepted. P03–P06 are not started by this phase; P07/WO10 remains explicitly deferred until Lu Core v1.0 architecture review.
