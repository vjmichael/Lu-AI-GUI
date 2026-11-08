# Lu IDE DEV-02.5 — Genuine test-only installed language-server packages

**Status:** implementation committed, actual exact-head Windows 2022 NSIS / Ubuntu 24.04 Debian CI acceptance **OPEN**. This is a technical test inside disposable GitHub-hosted runners, **not** a signed or legally approved redistributable release and **not** native protected LSP session acceptance.

## What this adds

- Source-owned `stage_test_only_lsp_resources.py` accepts only original frozen GitHub SHA256 release archives from `lsp_upstream_archive_candidates_20261108.json` and original release-tag license notices from `lsp_original_upstream_notices_20261108.json`. It fetches only those exact hashes, refuses linked/oversized/path-traversing packages and expands a single expected native executable per provider.
- Three native resource trees: `language-servers/clangd`, `language-servers/rust-analyzer`, `language-servers/ruff`, each with `bin/` and original `NOTICES/`. Every resource tree's SHA matches the **real native Rust supervisor's algorithm**: Windows cap-std full path + size + binary SHA; Linux sorted relative path + binary SHA. Paths are always from a disposable checked-out CI source and frozen original upstream archives—not untrusted project scripts.
- Technical receipt holds exact Git HEAD, archive SHA, extracted executable SHA, full installed-tree pin, original notice count, and compile-time `LU_IDE_E2_*_BUNDLE_SHA256` environment names. It explicitly sets `releaseApproved:false`, `redistributionReviewed:false`, `signedServiceInstalled:false`, `runLanguageServers:false`, `installedProtectedSessionAccepted:false`.
- Test-only installer resource configurations for Windows NSIS and Linux Debian, including an **installed visible permanent notice** `DEVELOPMENT-ONLY-NOT-FOR-DISTRIBUTION.txt`, native provider trees and the existing original Cargo/npm dependency notice inventory. No signed LocalService broker is shipped or invented. The default IDE package configurations remain unchanged.

## Genuine installed platform CI

Dedicated `.github/workflows/ide-dev02-test-only-installed-lsp-ci.yml` uses actual `windows-2022` / `ubuntu-24.04` OS jobs. Both must:

1. Check the exact source SHA and approved immutable npm/Tauri locks, and verify release mode still **rejects** the current unapproved providers.
2. Run strict offline malicious-asset tests, download pinned upstream archives and license notes, verify extracted bytes and complete hashes, compile actual Tauri with the **opt-in native-lsp feature** and embedded real full-tree expected digests. Windows broker pin is deliberately absent, making Windows provider start fail closed.
3. Build a **real NSIS or Debian installer** with original library notices and no-release sentinel. Nothing is uploaded or released from this technical workflow.
4. Actually install the built package on the native OS. Check exactly three installed executable paths against original SHA256s and original per-provider NOTICE files, plus the warning marker. On Linux, prove root-owned non-world-writable installed executables.
5. Actually open the installed desktop: Windows uses the existing installed binary SHA-based UIAutomation `Shell` acceptance with a real OS chooser; Linux uses `dev02_test_only_installed_linux.py` and actual GTK/AT-SPI OS chooser. No DOM/JavaScript/Tauri IPC shortcuts.
6. Uninstall the package and verify the genuine installed executable is gone. A failed chooser, installer or cleanup **fails the test**.

These jobs may fail if the OS's native GTK chooser, NSIS GUI or architecture-specific packaging is still broken. Do not mark the acceptance complete unless **both exact final-head OS jobs succeed**.

## Limits

- This proves **technical packaging and installed resource integrity**, not the right to redistribute the provider binaries or original notices. No release assets or installers are uploaded. It also does not validate the complete transitive dynamic dependency graph.
- The user-deferred signed Windows LocalService broker and reviewed privileged Linux cgroup-v2/Bubblewrap protected runner are **not simulated**. The installed packages cannot be treated as proof of live protected LSP sessions; final GUI diagnostics/completion/hover and worker reaping remain independent gates. Windows has no signed service pin in this technical build.
- Python semantic BasedPyright/ty evaluation stays separate DEV-02.4, with Ruff diagnostics only in the package. No BasedPyright/ty production executable is added.
- This does not change the original `language-servers.manifest.json` release approval and does not merge to main or the user release candidate.

**Acceptance receipt:** exact Git HEAD, current Win/Linux source run, real installed NSIS+Debian jobs with completed success, observed installed executable SHA and original notices, GUI evidence and uninstall marker. No evidence may be inferred from the mere presence of this workflow.
