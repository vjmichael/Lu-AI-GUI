# IDE WO6 — Protected Build/Test and Debugger Work Order

**Current state: implementation complete for the planned guarded operations; independent native OS and release acceptance NOT fully closed.** Draft PR #70 stacks on WO5 PR #69. This document distinguishes code from independently observed execution; neither source CI nor fixture hashes are a release signature. WO4 delegated cgroup and WO5 native language-server admission remain separate open dependencies.

## Immutable execution contract

No WebView/AI/process may supply a command, executable path, runtime environment, token, project script, debugger expression, arbitrary PID, conditional breakpoint expression, or attach target as a worker instruction. The trusted native host admits only fixed Cargo recipes `rust_check`, `rust_tests`, `lu_ide_host_tests` and one `dap_lldb_fixed` debug **self-test**. Every run requires clean source-head verification, a fresh verified snapshot, exact installer-reviewed toolchain/adapter+service pin, an independent native OS approval, one-use D3 authorization, a real constrained Job/cgroup, bounded outputs and time, and terminal process-tree evidence. A historical D1 review is never an execution grant.

## Implemented WO6 code

| Area | Source implementation | Evidence boundary |
| --- | --- | --- |
| Desktop Run/Test | `app/src-tauri/src/build_backend.rs` and `app/web/main.jsx`: opt-in Tauri commands, fixed recipes, pinned-offline Windows SCM/AppContainer or Linux cgroup/Bubblewrap native workers, clean Git head, frozen snapshot and fresh OS dialog | Default desktop remains nonexecuting if the feature or exact release pins are absent |
| Build cancellation/recovery | Native cancellation handle, status view, exact job scope, no false cancelled state, project-scoped OS-only `ide_build_reconcile` with separate approval | Kernel cleanup and durable receipt remain mandatory before result acceptance |
| Provenance and artifacts | `supervisor/src/wo6_build_test_evidence.rs` plus `wo6_output_evidence.rs`: exact project/head/manifest/plan/toolchain/receipt witness, relative artifact names/sizes/SHA; no arbitrary file download | Output witness advisory after terminal worker; not an installed or security certificate |
| DAP protocol | `host/src/dap_session.rs` incremental bounded framing, response correlation, fixed read-only threads/stack/stop; rejects reverse `runInTerminal`, arbitrary requests, invalid and oversized messages | Only a fixed `launch` accepted from trusted native code with no user path |
| Native debugger worker | `supervisor/src/dap_transport.rs`, `windows_lsp_worker.rs`, `linux_lsp_worker.rs`: restricted LLDB-DAP inside same authenticated Job/cgroup, fixed bundled debuggee, verified process-tree teardown and durable receipt | Does not attach to user-selected PID or arbitrary project binary |
| Debugger UI and recovery | `app/src-tauri/src/debug_backend.rs`, desktop Run Fixed Self-Test, cancellation and OS-only `ide_debug_reconcile`, quarantine carried across restart | No project debugger release enabled; interruption cannot silently reauthorize |
| Adapter licensing | `ide/packaging/debug-adapters.manifest.json`, `verify_debug_adapters.py`, tests and first-party `wo6_selftest.c` | `releaseApproved=false`. Unreviewed transitive dependencies, missing signers/hash and installed acceptance block distribution |

## Actual CI acceptance

1. **Source gates:** `.github/workflows/ide-wo6-build-test-dap.yml` executes hostile protocol, D3 supervisor, and LLDB packaging-deny tests on native Windows and Linux. `.github/workflows/ide-wo6-native-desktop.yml` compiles default and opt-in native build/debug Tauri paths on both OSes. Source compilation is necessary but *not* native worker success.
2. **Default installed desktop:** `.github/workflows/ide-e2-unsigned-installed-smoke.yml` performs a real unsigned NSIS build, install, two launches and uninstall and requires no unauthorized native LSP/service; the standalone default-IDE workflow passed at previous WO6 head `2530407`. It does not prove a protected debugger was bundled.
3. **Independent native Windows Cargo:** existing D7D protected-service acceptance `ide-d7d-protected-service.yml` executes genuine pinned offline Cargo and cancellation in AppContainer+Job with authenticated separate SCM, revoked NUL lease, and recovery. Reuse this; do not create an alternate uncontained process path.
4. **Independent native Linux Cargo:** `supervisor/examples/wo6_linux_native_build_acceptance.rs` with fixed systemd host `sandbox/src/bin/linux_delegated_ci_wo6_build_host.rs` and new `ide-wo6-native-os-acceptance.yml` attempts real cgroup-v2/Bubblewrap `cargo check`, `cargo test`, cancellation, kernel process-tree cleanup and reopened durable receipts. No fake cgroup, root worker or unconfined fallback.
5. **Independent native debugger:** `supervisor/examples/wo6_native_dap_acceptance.rs` requires actual protocol `initialize`, fixed self-test `launch`, stopped thread, thread/stack result, normal and cancel kernel termination and reopened D3 receipt. Windows native CI provisions only a test-only self-measured LLDB-DAP bundle and separate restricted SCM; Linux uses root-owned binaries under a real systemd-delegated unit. Fixture binary hashing does not license/sign a release.
6. **Fail closed when unavailable:** Host inability to create cgroup, missing exact SCM, missing compatible LLDB runtime dependencies, unsupported reverse DAP, hostile debugger, or stale receipts must fail rather than mark the job green.

## Remaining independently verifiable gates before closing WO6

- Obtain a successful exact-head **native Windows protected Cargo** normal/cancel and **native Linux delegated Cargo** check/test/cancel run, including durable OS cleanup and restart evidence; repair genuine OS failures while preserving containment.
- Obtain genuine Windows and Linux LLDB-DAP fixed-debuggee initialization, stop, threads/stack, cancellation, worker-crash recovery and service/cgroup process cleanup. Protocol-mock unit tests are **not enough**.
- Review adapter license notices, third-party dependency closure, exact release binary SHA and authenticode/signing identity, Linux immutable root-owned bundle, and installed/debugger feature acceptance. Do not flip `releaseApproved` or invent signatures; these require actual independently approved artifacts and native evidence.
- Prove protected Run/Test and fixed debugger through the **installed** desktop (not just the default disabled NSIS candidate), including Run/Cancel, recovery and uninstall. Project-specific debugging beyond the fixed self-test requires a separately authorized source-to-binary mapping, target attestation and native worker acceptance.
- Hostile OS tests of source/manifest path swaps, network and credential isolation, build-script process descendants, resource/output caps, debugger reverse requests and stale race handling before release enablement.

**Release default remains disabled** until the above are accepted. WO7 signed packaging and WO8 broader installed acceptance remain separate WOs. Remaining original open orders at this document revision: WO4–WO10, seven open. Implemented source is not closed WO6 acceptance.

---

## Historical WO6 evidence from PR #70 (historical heads only)

This section is copied from [PR #70](https://github.com/vjmichael/Lu-AI-GUI/pull/70) at `c0b83a304566d803e19082591d36debffe501698` to preserve original CI receipts. It **does not** establish current-head Windows/Linux debugger acceptance or release approval.

## WO6 closeout evidence checkpoint — 2026-11-06 (CI running 2026-11-07 UTC)

**WO6 remains OPEN and draft, not release enabled.** This section records actual kernel/installer evidence, not a future claim.

### Native acceptance obtained

- **Linux protected Cargo normal check:** completed with OS exit 0 inside a real systemd-delegated cgroup-v2 and Bubblewrap sandbox; process tree was verified empty and D3 journal was replayed.
- **Linux protected Cargo tests:** completed with OS exit 0 under the same real controls and durable terminal receipt.
- **Linux protected Cargo cancellation:** recorded `CancelRequested`, kernel process-tree kill and `Cancelled`; surviving D3 journal replay matched. [Native job run 42340446866](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42340446866) proved all three modes, reconfirmed by [42341239874](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42341239874).
- Missing `/etc/alternatives/cc` inside the restricted namespace was resolved using a versioned, root-owned GNU linker chosen from a fixed allowlist, added to the native toolchain SHA fingerprint; no writable `/etc`, network, PATH search or unconfined fallback.
- The default **unsigned** Windows NSIS installer was actually installed, launched twice and uninstalled in prior acceptance [42339662353](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42339662353). This does **not** demonstrate positive native debugger installation.

### Native LLDB-DAP repair and verification still open

- Linux three-way genuine LLDB-DAP `initialize` protocol probe passed from: (1) distro original, (2) root-owned copied bundle and (3) copied bundle within the real read-only Bubblewrap namespace. No project/debuggee execution was used for the preflight.
- Under the original kernel `pids.max=4`, the actual contained debugger exited early. Cgroup PIDs count threads; `dap_lldb_fixed` was increased to **16 kernel tasks**, within the supervisor's existing maximum (no change to CPU/memory/network/FS controls or native cleanup). This resulted in actual protocol output from the kernel-contained LLDB-DAP, proving its launch and pipe boundary.
- The strict DAP parser then rejected a native message; bounded adapter-assigned `seq=0` is now accepted while **positive, one-use, exact `request_seq` response correlation remains mandatory**, and only standard nonexecuting informational events are ignored without passing their contents to the editor. Reverse terminal, evaluate, path/target input, arbitrary launch and shell requests remain denied. Hostile unit cases cover server sequence zero and forged uncorrelated replies.
- The current genuine Windows/Linux normal+cancel+restart retest is [42341808568](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42341808568), queued at this documentation checkpoint. No stopped thread, stackTrace, release receipt or Windows native acceptance is credited until it passes. Native Linux quota diagnostics expose only boolean kernel PID/OOM hits, not raw process data.

### Release blockers and WO boundaries

- **Not done:** exact-head Windows restricted SCM/AppContainer LLDB-DAP execution, real native Linux stopped-thread/stack and cancellation, installed debugger feature Run/Cancel/host-crash reconciliation, and hostile native OS probes involving the debugger.
- **Not done:** independently reviewed signed LLDB-DAP distributions with transitive licenses, package identity/signatures and verifiable published SHA pins. The independent release manifest intentionally has `releaseApproved=false`; it must not be edited to fake an approval. Signed redistribution belongs to WO7, broader installed-app release acceptance to WO8, but native WO6 worker evidence is still this work order's gate.
- **Never waive:** native one-use approval, exact source snapshot, cgroup/Job kernel confinement, strict resource limits, cancellation-before-kill receipt, process-tree verification and quarantine on failure.

---
