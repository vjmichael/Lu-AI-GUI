# DEV-04.5 — Installed protected project debugger acceptance and native repairs

Status: **IN PROGRESS**, stacked on DEV-04.4 draft PR #167. Neither Linux nor Windows project LLDB installed acceptance is closed.

## Actual CI failures inspected

At exact DEV-04.4 head `111fd3cd80b1dae9d2270db83c07f0f18856c193`:

- Windows worker containment, Windows D7 native worker and Windows AppContainer admission reached source formatting gates and failed `cargo fmt --check`. The fixed Windows kernel-probe stage had first reported successful suspended Job assignment, descendant cleanup and AppContainer denial. Extracted all **88 exact rustfmt hunks** from real job logs, applied to **10 specific Rust files**. This source repair does **not** assert later D7 runtime success.
- Supplemental Linux installed editor workflow reported **8 failing Node tests / 239**, largely outdated source contract checks. Refreshed tests for the current fixed Cargo builder route, 49 schema operations, 47 definitions, exact WebView/native commands including private-only debug/workspace operations, digit-bearing native argument names, read-only disk check versus separate editor merge, and sync cleanup handler with comments excluded.
- Independently found a substantive DEV-04.4 wiring error: project-target mount in ordinary Linux LSP worker and project-target PE/PDB stage and grant recheck in ordinary Windows LSP worker. Moved both to the **actual protected LLDB-DAP start** route. General LSP does not accept project targets. Compilation and OS acceptance on repaired SHA remain required.

## Genuine signed Windows installed gate

Manual workflow `.github/workflows/ide-dev04-5-installed-project-debug-signed.yml` is **not triggered automatically by a source PR**. Requires:

- Exact 40-byte source commit, existing signed WO7 release-candidate artifact, independent Authenticode/license verification, and `nativeProjectDebugger` manifest metadata explicitly pinning `native-project-debug`, the signed candidate's reviewed LLDB tree, and `installedAcceptance=false`. Current legacy WO7 candidates without these fields fail closed.
- Clean Windows 2022 installation of the actual signed NSIS, exact installed desktop SHA-256 check, and installed **LocalService** protected broker identity/signature.
- Native Windows UI Automation (never hidden Tauri invoke): register a fixed disposable clean-Git Rust project; select genuine `rust_build`; approve native build; verify zero exit, terminal OS receipt and visible binary + PE/PDB evidence; obtain **separate native project DAP consent**; prove stop/thread/stack, exact build receipt/target SHA, worker teardown and original source hash.
- Close desktop, reopen from the installed NSIS and prove previous sealed target and D3 grant cannot be reused. Uninstall NSIS and verify disappearance of desktop/service.
- A failed/native-unsupported/missing-signed-candidate run is not installation acceptance. Separate cancellation/crash and hardlink/race acceptance remain open.

`ide/app/fixtures/Test-Wo8NativeDesktopUI.ps1` now has `Dev045ProjectDebug` and `Dev045ProjectRestartDenied` modes. They **must not** be confused with WO8 `ProtectedBuildDebug`, which only exercises the fixed LLDB fixture.

The exact-head DEV-04.4 Windows/Linux source CI parses the Windows PowerShell UI acceptance script, runs a bounded Node harness contract, supervisor DAP and binary witness tests, and compiles the native opt-in Tauri executable. It does **not** install signed packages or prove native-kernel behavior.

## Open requirements

1. Obtain exact-head passed Windows/Linux Rust/Node/Tauri native jobs; repair any source or runtime faults in the actual logs.
2. Linux: actual kernel delegated cgroup-v2/Bubblewrap Cargo `--project-debug`, real ELF/DWARF and private-stage SHA evidence, fresh D3 LLDB stop/stack, kill/empty descendants and restart journal (workflow WO6). Then **separately** install a pinned Debian project-debugger package and exercise native GTK consent, cancel/recovery and uninstall. The default unpinned Debian build is not accepted for debugger release.
3. Windows: independently authenticated SCM/AppContainer/Job project debugger normal/cancel/crash, native PE/PDB identity and source-map interaction. Then signed/pinned **installed** manual candidate and UI acceptance above.
4. Complete executable/data handle custody hardening, Windows hardlink/reparse race tests, symbol/source line mapping, LLDB reverse-request denial and actual breakpoint/step evidence before broad project debugger release.
5. Final legal/license, signing and redistribution approval remains a separate decision.

**No production or installed debugger acceptance is implied by this file or a source-only workflow.**
