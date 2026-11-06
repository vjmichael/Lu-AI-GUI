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

The VS runner mismatch is not fixed by accepting an unpinned compiler: GitHub's Windows 2025 image moved to Visual Studio 2026, while the Windows 2022 image retains the Visual Studio 2022 generation. The test retains Rust 1.99.1, MSVC 14.44, Visual Studio 17.14 and Windows SDK 10.0.26100 as exact required versions. Image updates may still require an independently observed SDK inventory before acceptance.

## Current branch verification boundary

- Draft: [PR #65](https://github.com/vjmichael/Lu-AI-GUI/pull/65), branch `ide/wave-e2-supervised-lsp-editor-20261106`.
- GitHub jobs for this latest series remain queued at time of repair; repair commits are **not yet independently compiler/OS verified**.
- No installed NSIS E2 image with signed, license-audited clangd/rust-analyzer/Ruff and D7D restricted service has been accepted.
- The D6 strict AppContainer zero-capability network/secret denial gate remains **open**.
- E2 protocol/host/supervisor source **needs new native compilation results**. Its formatting may also need changes after newer Rustfmt reports; do not mask rustfmt checks.
- Continue in dependency order: 1) exact-head native compile; 2) repair fresh errors; 3) hostile Job/service/cgroup and cleanup acceptance; 4) signed installed desktop UI acceptance. Keep the unaccepted default-alpha worker disabled.

This ledger is for source and CI truth: no fabricated test pass or simulated service witness.
