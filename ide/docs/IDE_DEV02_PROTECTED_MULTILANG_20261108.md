# Lu IDE DEV-02.2 — protected project-specific Rust, Python and C/C++ language services

**Status:** SOURCE-IMPLEMENTED; native feature-flag Windows/Linux compilation, actual installed Windows NSIS and Ubuntu Linux Debian LSP testing **OPEN**. An implemented workflow or manually queued job does not equal a native OS pass.

**Parent:** DEV-02.1 project Problems [PR #129](https://github.com/vjmichael/Lu-AI-GUI/pull/129). Standalone IDE source only; no Lu Core/P07 bridge or release-candidate reconciliation. The independent BLK-06 protected worker, BLK-05 GTK chooser, DEV-01 installed tests and P05 B2/B3 source reconciliation retain their own gates.

## Architecture and concrete implementation

Previously the Tauri `lsp_backend` was compiled only for Windows, despite the supervisor already having a fully separate feature-gated `linux_worker::lsp_native` protected LSP gateway. DEV-02.2 connects that existing Linux gateway to the *same native source/approval runtime* as Windows:

- `native-lsp` remains **off in default builds**. On explicit protected builds, Windows uses pinned LocalService + AppContainer/Job Object `start_windows_lsp`; Linux uses **root-owned reviewed executable bundle, root-owned /usr/bin/bwrap, delegated cgroup-v2, network-unshared read-only project snapshot** through `start_linux_lsp`. Neither path executes an unreviewed PATH program or falls back to a same-user unconstrained worker.
- Each start is scoped to an OS-picker-registered active project, valid relative source file, exact on-disk SHA and monotonically versioned editor buffer. The native host shows a separate OS human approval dialog, checks identity/revision **again after approval**, stages and seals a bounded project snapshot and has the supervisor consume the one-shot grant only after native OS attestation. Native in-flight responses are correlated to project, source, disk SHA, buffer version and language ID. Stop closes the supervised gateway; kill/recovery must be verified on real OS.
- Existing protected Tauri commands for `ide_lsp_worker_status`, `ide_lsp_start_session`, `ide_lsp_change_document`, `ide_lsp_feature`, `ide_lsp_read_diagnostics` and `ide_lsp_stop_session` now compile for Linux as well as Windows **only behind `native-lsp`**.
- Existing UI synchronizes active editor changes after a 400-ms debounce, requests real diagnostics, completion, hover or same-file definition from the native worker and accepts only exact-source replies. DEV-02.1 Problems aggregates *only* source-matched diagnostics from already-open current-project tabs. No LSP reply becomes filesystem, process or URI authority.

## Language capability truth table

| Language | Native engine | Diagnostics | Completion | Hover | Navigation |
| --- | --- | --- | --- | --- | --- |
| Rust | rust-analyzer | Implemented gateway | Implemented gateway | Implemented gateway | Same-file definition only |
| C/C++ | clangd | Implemented gateway | Implemented gateway | Implemented gateway | Same-file definition only |
| Python | Ruff `server` | Implemented gateway | **Not approved** | **Not approved** | **Not approved** |

UI and native host both deny Ruff semantic operations. The deferred Pyright-compatible provider remains in the manifest as **deferred**, with no approved executable, node runtime, transitive npm inventory or protected launch recipe. Do not advertise Python semantic completion/navigation yet. Multi-file definition, references, workspace symbols, LSP server code actions, and untrusted server `file://` opening are not implemented.

## Reviewable packaging & provenance

- Original `ide/packaging/language-servers.manifest.json` retains `releaseApproved: false`, no asset bytes or pinned SHA. **No new release approval or signer identity was fabricated.**
- Offline `stage_linux_language_server_release.py` demands reviewed full Windows **and** Linux executable digests, original notice bytes, explicit license/dependency approval and `releaseApproved: true`, then stages native Linux `rust-analyzer`, `clangd` and `ruff` and computes full-tree hashes matching the Linux Rust worker. The receipt explicitly records `installedRootOwned: false` and `nativeWorkerVerified: false` because only genuine installation/runtime can establish those.
- Original signed-Windows-service stager now includes full-tree Windows cap-std digest pins of executable + notices, not just a single binary SHA. The host still re-hashes each installed bundle, requires independently valid Authenticode on the LocalService broker and consumes OS-specific grants.
- Dedicated Tauri resource-only package configurations `tauri.dev02.linux.conf.json` and `tauri.dev02.windows.conf.json` map precisely vetted providers and original Cargo/npm legal notices into real Debian/NSIS packages. They do not install or approve a service by merely mapping resources.

## Real source and installed workflows

1. `.github/workflows/ide-dev02-protected-multilang-native-ci.yml`: **normal push/PR** exact-head Windows2022+Ubuntu24 `npm run check`, host+supervisor tests, `cargo check`/unit tests with opt-in `native-lsp`, reviewed immutable npm/Cargo locks, and Linux offline staging hostile tests. It explicitly checks that release mode rejects the current unapproved manifest.
2. `.github/workflows/ide-dev02-linux-installed-protected-lsp.yml`: **manual dedicated runner** `[self-hosted, Linux, lu-ide-delegated]`. Requires real delegated cgroup and root-owned Bubblewrap and reviewed assets, builds a real `native-lsp` Debian package and installs it as root, verifies original upstream licenses and installed file ownership, and runs OS-only GTK/AT-SPI/X11 `dev02_linux_installed_lsp_atspi.py` against real Rust/C++/Python files. Every provider requires a real native session dialog; Rust/C++ hover/completion/definition and all diagnostics are asserted; Ruff semantic controls must be disabled. A failed GTK picker fails the job. Genuine package removal is required.
3. `.github/workflows/ide-dev02-windows-installed-protected-lsp.yml`: **manual dedicated runner** `[self-hosted, Windows, lu-ide-protected]`. Requires an independently reviewed SHA and Authenticode-signed Windows recovery service plus release-approved LSP assets, packages and installs actual `native-lsp` NSIS, creates the exact LocalService broker, and runs `Dev02ProtectedLsp` through existing native Windows UIAutomation owned-popup witness. Tests real OS process spawn/reaping, diagnostics, completion, hover and same-file definition and Python Ruff diagnostics, then removes service and uninstalls. No mock invocation or user-facing web automation is accepted.

These installed jobs are **not scheduled to run automatically** because their privileged, signed, approved prerequisites are not yet provided. They must be manually dispatched only when dedicated reviewed runners and assets exist. A self-hosted label, a debug build, or a checked-in test fixture is **not** evidence of containment or a permitted binary license.

## Native acceptance and remaining work

- Inspect exact-head normal push/PR source CI; fix any real Windows/Linux compile, protocol or UI-contract failures without relaxing kernel grants, default-off feature flags or release provenance. Keep evidence per exact SHA.
- Then stage and independently verify actual reviewed rust-analyzer/clangd/Ruff binaries and notices, signed Windows broker, Linux root ownership/real cgroup delegation, installer payload SHA and exact installed paths. Do not use the user's project directory or ambient PATH for executables.
- Obtain **passing real installed app** native GTK chooser, Windows UI Automation provider consent, actual diagnostics/completion/hover/definition and worker reaping across project switches, edits and cancellation; test deleted/replaced source and stale replies. Complete source->package->installed resource inventory and uninstall. The default unsigned alpha remains nonexecuting for LSP.
- For Python semantic completion/hover/definition, separately review a Pyright-compatible approved distribution, its Node or native runtime and transitive licenses; implement new strictly pinned native sandbox recipe and native method validation before enabling the controls. No blanket Ruff fallback.
- Extend definition/reference navigation outside the currently open file **only after** native host resolves provider-supplied URIs to a registered project-relative path and a verified file SHA, and the front end checks the destination revision.
- Real p95 latency, memory/energy, large multi-crate workspaces, cancellation/quota violations, process-tree crash recovery, and installed physical Windows acceptance remain independent gates.

**Acceptance:** No native source green, actual protected language-server launch, installed OS success, Pyright licensing, distribution signing or legal review is implied merely by this source branch.
