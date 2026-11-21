# DEV-02.6A — Native language-server dynamic imports and actual version probes

**State: implemented test-only audit; exact-head native Windows/Linux CI evidence required.**
Stacked on DEV-02.5 draft PR #138, itself stacked on DEV-02.4 #134.
No language-server release, redistribution or execution inside Lu IDE is approved.

## Why this is a new gate

DEV-02.5 verifies the authentic release archives, four original license texts and
the integrity of a disposable test ZIP containing three native server
executables. That is insufficient for a single-click installer: a native
binary may reference DLLs or ELF libraries not copied into the ZIP, and
clangd may require original LLVM/Clang resource files. A file hash alone
does not demonstrate that the program can actually start.

DEV-02.6A advances this using fixed upstream *versions already approved for
technical inspection, NOT approved for redistribution*: clangd 23.1.0,
rust-analyzer 2026-11-02 and Ruff 0.17.4.

## Exact candidate test

1. On a genuine Windows 2022 or Ubuntu 24.04 hosted runner, check out the
   triggering SHA and assert Lu's release manifest remains denied.
2. Download the original x86_64 archive for each of the three providers using
   its current immutable original GitHub SHA256 pin. Reject drift, unsafe
   archive paths, linked/special members, duplicate names and excessive
   decompression. No arbitrary archive extraction.
3. Stage only the main executable and original native DLL/.so entries using
   their original relative archive paths under RUNNER_TEMP. Record the exact
   SHA256 of every staged byte. Count known Clang resource entries but do NOT
   claim to have staged or reviewed a resource closure.
4. On Windows, parse PE import and delay-import tables without loading the
   DLL as code. Report imports found in the same original directory,
   imports found elsewhere in the original archive, candidate Windows
   system imports and unresolved external DLL names. Even a nominal
   Windows system DLL is classed as OS-provided UNVERIFIED, not as a new
   redistributable native asset.
5. On Linux, use the trusted static readelf tool to inspect original ELF
   NEEDED libraries, RPATH/RUNPATH and GLIBC symbol version requirements.
   Do NOT use ldd on arbitrary uploaded/unchecked project files.
6. Launch only the three fixed original release commands with literal
   --version, no shell, no project scripts, a bounded timeout and process
   reaping. Require zero exit and the expected provider marker. This is a
   technical smoke probe inside disposable CI, NOT a Lu supervised worker
   or a customer program execution permission.
7. Upload JSON-only receipts (seven-day retention). Failed provider probes
   remain visibly failed; no candidate executable or archive is uploaded.

Files:
- ide/packaging/audit_native_lsp_dependency_smoke.py
- ide/packaging/test_audit_native_lsp_dependency_smoke.py
- .github/workflows/ide-dev02-native-closure-smoke.yml

## Exact acceptance boundaries

The research report must retain these FALSE flags, even when native
version probes pass: releaseApproved, redistributionApproved,
protectedLuWorkerTested, genuineInstalledNsisOrDeb,
fullDependencyClosureApproved, fullLLVMResourcesReviewed.

Success of a --version process does NOT show that full LSP initialize,
diagnostics, completion, hover, definition or cancellation works; nor
does it show that the protected Windows AppContainer/LocalService/Job Object
or Linux root-owned Bubblewrap/delegated cgroup launcher has accepted
the provider. It cannot prove all proprietary DLL obligations, minimum
supported Windows build, all transitive notices, or LLVM's resource
header/config folder layout.

DEV-02.6B (next gate): confirm original complete dependency/resources
and legal byte graph from the authentic archive; correctly stage
dependency-complete frozen server packages; run real protocol requests
and hostile URI/revision/provenance tests *inside* independently
provisioned protected OS workers. No default enablement of native-lsp.

DEV-02.7 (later): genuine installed Windows NSIS and Ubuntu/Debian
GTK desktop use, human-owned consent, version/restart recovery,
cancellation/job reaping, diagnostics/hover/definition/completion,
rollback and uninstall. Legal publisher/signing review remains a
separate user-deferred release gate.
