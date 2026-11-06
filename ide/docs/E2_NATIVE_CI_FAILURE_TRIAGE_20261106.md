# Lu IDE E2 native CI failure triage — 2026-11-06

This is a **commit-scoped** repair ledger. It records observed CI failures separately from source-level proposed repairs and actual later acceptance. No new success is inferred from a pushed fix.

## Confirmed failed runs (historical heads)

| Workflow and run | Native job | Direct log evidence | Source repair on PR #65 |
| --- | --- | --- | --- |
| D7D protected service 42263037290 | crash-recovery-windows 128047652708, native-windows-service 128047652803, service-loss-windows 128047652858 | Rust E0308 / parser failure in `windows_broker.rs:132`: `unsafe { TerminateJobObject(handle, 98) } != 0` was not parsed as intended | Parenthesized the unsafe result before comparing; commit `b9d12b3` |
| D7D protected service 42263037290 | same three Windows jobs | Rust E0432: `OpenProcessToken` and `OpenThreadToken` do not live under `windows_sys::Win32::Security` in version 0.61.2 | Moved both imports into `Win32::System::Threading`; commit `65b13ace` |
| D7D protected service 42263037290 | pinned-offline-protected-cargo 128047652179 | `DENIED: VS pin 17.14. not installed` from `New-PinnedWindowsToolchain.ps1:78` on the Windows 2025 runner | Pin the specifically offline VS 17.14/14.44 job to `windows-2022`; commit `02dcf220`. Keep exact compiler/version checks |
| D7B pinned toolchain 42263037601 | pinned-offline-cargo 128047661944 | Same VS 17.14 unavailable failure | Switch only the pinned offline D7B job to `windows-2022`; commit `7624f44a` |
| D7C capability staging 42263037514 | capability-worker-windows 128047660771 | `cargo-fmt.exe is not installed for toolchain 1.99.1` | Add `components: rustfmt` explicitly to D7C / D7B / E2 pinned-1.99.1 workflow actions; commits `d721f117`, `c089d1b0`, `8c6ac15d` |
| E1 LSP catalog 42264428558 | e1-host Linux 128052509472 and Windows 128052510020 | Native Rust E1 catalog and host tests **passed**, but `cargo fmt --check` failed on five precise `ide/host/src/lsp.rs` hunks | Applied exact native Linux CI rustfmt hunks without changing semantics; commit `9e484f74` |
| D6 AppContainer 42263037282 | appcontainer 128047652757 | Isolated child exited `1099`; that corresponds to a socket failure without raw OS error, **not** verified WSAEACCES | D6 probe now distinguishes std I/O timeout / refusal / unknown missing raw OS error; commit `4a6ee4ec`. **No network isolation pass granted.** |

## New Windows native failures and repairs (exact run evidence)

| Workflow / run | Native evidence | Repair / status |
| --- | --- | --- |
| D7D pinned-offline 42284316584, job 128121085896 | Exact Rust and VS2022/MSVC/SDK fixture constructed; native Windows service + supervisor **compiled successfully**. `Install-ProtectedRecoveryService.ps1:75` failed with generic `SCM service creation failed`. | Preserve sc.exe exit code/output and correct previously uninitialized protected config ACL allowlist (`c4f23fd`). |
| D7D pinned-offline 42285403347, job 128124805286 | With diagnostics, native `sc.exe create` returned **1639**, printed usage (malformed argv), not access denial. | Use explicit `$scArgs` and minimal well-delimited native arguments, avoiding automatic `$args` (`a155355`). |
| D7D pinned-offline 42285830462, job 128126270722 | **`SERVICE=LuIdeRecoveryBroker`** proves installation/start step succeeded. Acceptance fixture then rejected `DENIED: fixed Windows recovery broker must be provisioned`, despite the separately protected service. | Protected build now references the service executable with `LU_IDE_D7_PROTECTED_SERVICE_EXE` instead of the legacy D7C broker, and CI provides it (`19ff31d`, `ef68a8f`). |
| D7B pinned-offline 42286299058, job 128127910718 | Real native AppContainer acceptance ran for ~4 minutes, then returned **`SUPERVISOR_GRANT_EXPIRED`**. Grant TTL is 60,000ms and staged MSVC/SDK bundle preparation occurs after early approval. | Added native post-stage *fresh* approval callback, with CI fixture issuing its test-only grant after sealed toolchain/input checks; 60s TTL unchanged (`7d6136c`, `542ef5f`). The older public entrypoint still denies expired grants. |
| D7D pinned-offline 42286298966, job 128127959372 | Restricted SCM service created and started with ACLs; real protected Cargo test returned **`SUPERVISOR_OS_CONTAINMENT_NOT_VERIFIED`** after staging. The broad fail-closed error is insufficient to isolate profile/Job/pipe/admission stage. | Added debug-CI-only non-sensitive stage markers (never security decisions) around workspace, toolchain, kernel Job, broker fsync, grant consumption, suspended AppContainer (`3dee288`, workflow `2c2b2c7`, `6f3d35b`). **Cause remains open**, not marked repaired or accepted. |

The D7D native compile and Windows SCM service installation are now **separately observed successful** in later runs, but the protected Cargo execution and cancellation remain failed/unaccepted. D7B staging-grant repair is source-only pending new native verification. No accepted E2 clangd end-to-end installed-app evidence exists.

The VS runner mismatch is not fixed by accepting an unpinned compiler: GitHub's Windows 2025 image moved to Visual Studio 2026, while the Windows 2022 image retains the Visual Studio 2022 generation. The test retains Rust 1.99.1, MSVC 14.44, Visual Studio 17.14 and Windows SDK 10.0.26100 as exact required versions. Image updates may still require an independently observed SDK inventory before acceptance.

## Current branch verification boundary

- Draft: [PR #65](https://github.com/vjmichael/Lu-AI-GUI/pull/65), branch `ide/wave-e2-supervised-lsp-editor-20261106`.
- GitHub jobs for this latest series remain queued at time of repair; repair commits are **not yet independently compiler/OS verified**.
- No installed NSIS E2 image with signed, license-audited clangd/rust-analyzer/Ruff and D7D restricted service has been accepted.
- The D6 strict AppContainer zero-capability network/secret denial gate remains **open**.
- E2 protocol/host/supervisor source **needs new native compilation results**. Its formatting may also need changes after newer Rustfmt reports; do not mask rustfmt checks.
- Continue in dependency order: 1) exact-head native compile; 2) repair fresh errors; 3) hostile Job/service/cgroup and cleanup acceptance; 4) signed installed desktop UI acceptance. Keep the unaccepted default-alpha worker disabled.

This ledger is for source and CI truth: no fabricated test pass or simulated service witness.
