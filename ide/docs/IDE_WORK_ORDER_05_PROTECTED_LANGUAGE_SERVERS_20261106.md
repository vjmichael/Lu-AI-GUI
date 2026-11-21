# IDE WO5 — protected native language servers

**Status:** active implementation, not accepted or release-enabled. **Stacked on WO4 draft PR #68** (`ide/wo4-linux-delegated-containment-20261106`). Source order: WO1–3 native Windows gates accepted on `bd39d011`; WO4 Linux delegation implementation source CI green on `9113470` but real delegated Linux runtime evidence remains OPEN. WO5 does not close WO4 or automatically enable native LSP.

## Starting evidence

- Existing E2 protocol/editor on latest WO4 base: Windows and Linux native source compilation and LSP protocol tests PASS; Windows Tauri host compile PASS.
- Windows true restricted LocalService + AppContainer `clangd`: latest native job [128261182061](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42325948160), first normal acceptance yields `SUPERVISOR_RECONCILIATION_REQUIRED` after separately installed SCM service. No verified initialized `clangd`, hover, completion or cancellation E2E yet.
- Linux real delegated `clangd`: hosted Ubuntu job [128261181720](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42325948160) correctly denied `0::/system.slice/hosted-compute-agent.service` with no writable delegated cgroup. That denial is **not** a pass for native language server startup.
- Editor installation smoke is separately failing during genuine unsigned NSIS construction and remains WO7/WO8 release work, not proof of protected native LSP.

## WO5 work batches

1. **Diagnose actual Windows LSP refusal without weakening containment.** Add fixed, debug-only stage traces (no SIDs, paths, tokens, project data or source text); correlate exact phase of AppContainer creation, sealed input/bundle, SCM service-owned Job admission, grant consumption, suspended token attestation, durable Running, process resume, native pipe gateway, LSP initialize. Distinguish a service lease failure from child launch and gateway lifecycle failure. Do not promote broad ACLs, launch as user, run unsupervised native server or accept self-measured bundle as an installed release.
2. **Repair native protected execution and protocol lifecycle.** Require real restricted token, zero network capability, authenticated D7D service ownership, exact pinned file-system staged executable, bounded STDIO, monotonic one-use authorized job and process-tree kill. Verify real clangd `initialize` + diagnostics/completion/hover/definition and cancellation with journal restart receipt.
3. **Linux server acceptance** using WO4 native delegated hierarchy and actual root-owned pinned `clangd`, Bubblewrap and verified cgroup placement; refuse simulated kernel evidence. Use the Debian development container for source/unit/protocol tests when appropriate, but a read-only cgroup container cannot certify delegated enforcement.
4. **Release admission gates**: reviewed license + transitive dependencies and per-platform exact SHA manifest, installed release provenance, Windows and Linux hostile process/escape tests and actual installed Tauri editor UX. Keep `ide_lsp_worker_status` off until those tests pass. This work order does NOT enable arbitrary PATH servers or user-pinned binaries.

## Initial changes

- `windows_lsp_worker.rs` now reports whitelisted `PROTECTED_LSP_ADMISSION_STAGE=...` stages exclusively under debug+explicit `LU_IDE_E2_NATIVE_TRACE=1`. No new permissions or fallback paths.
- Native Windows CI enables that trace and the preexisting protected service admission trace for the true native `clangd` acceptance job.
- Subsequent source and native OS results must be recorded against exact commits.

**Remaining work orders:** 7 still unclosed in original WO1–10 inventory (WO4–10). WO4 stays open for native Linux acceptance while WO5 work proceeds.
