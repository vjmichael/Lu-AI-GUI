# IDE batch 01 — work orders 1–3 (2026-11-06)

**Branch:** `ide/wave-e2-supervised-lsp-editor-20261106` · [draft PR #65](https://github.com/vjmichael/Lu-AI-GUI/pull/65).
**Source repair heads:** `2076532`, `0e941c7`, `3c38f5e`.
**Acceptance:** no native OS or installer pass inferred from source commits. These three work orders remain open until the exact-head native gates pass.

## WO1 — Windows native compilation and existing test failures

Previously observed failures on CI head `4653894`:
- Native Windows E2 source compile: `error[E0277]`, raw HANDLE (`*mut c_void`) captured by LSP writer thread; job 128173461983.
- Windows E2 and supervisor-core `--all-targets`: `error[E0432]`, Windows acceptance example references the `windows-worker` module without that feature; jobs 128173462089 and 128173463300.
- Windows Tauri host: macro `frontendDist="../dist"` absent at compile; job 128173461721.
- Protected recovery in-memory test panicked at line 1259 with `Quarantined` because it invoked **post-crash** preflight on a still-live job instead of reopening journal; job 128173502837.

**Committed repairs:** move an owned pipe handle across thread boundary through a narrow one-writer Send wrapper, without globally making arbitrary raw handles Send; gate non-executable Windows acceptance example behind `windows-worker`; perform real npm frontend build prior to Tauri native check; reopen interrupted journal in test before protected recovery. **Pending:** exact-head Windows compiler, Rustfmt and test evidence.

## WO2 — Windows AppContainer containment

- The D7B real pinned Cargo fixture (job 128173510848) staged input and toolchain, consumed the grant, entered a kernel Job, and attested a suspended AppContainer. Cargo exited 101 **before launching rustc** because Rust's Windows child process machinery could not open `\\.\NUL` for child stdin (error 5). A parent-supplied inherited stdio handle does not resolve the *nested* Cargo subprocess's own null-device open.
- Microsoft's AppContainer access model requires explicit access for restricted resources; current upstream Rust opens the NUL device for `Stdio::Null`. Microsoft MXC host prep documents an opt-in machine-scoped `\Device\Null` security descriptor change, but this is **NOT** authorization for Lu IDE to install a broad global ACE. No automatic global device ACL patch, elevated fallback, relaxed token or false pass added.
- The D6 AppContainer live-loopback test (job 128173455685) returned exit 1399 (socket timeout without direct WSAEACCES). The test correctly refuses to call that explicit kernel denial.
- **Committed strengthening:** actual child now inspects `TokenCapabilities` and rejects *any* unexpected capability or failed kernel token query, in addition to its existing token/secret/network checks. The WSAEACCES-only gate is not weakened.
- **Remaining engineering:** determine a reviewed least-privilege, profile-SID-specific, reversible administrative NUL provisioning path (if feasible) and re-run genuine Cargo/rustc under kernel restrictions; prove zero-network denial using native, endpoint-positive evidence. No release should auto-mutate global OS object ACLs.

## WO3 — Protected SCM service and crash recovery

- The protected service installation/start worked in prior CI, but its IPC admission fails. On the D7D protected-Cargo job 128173503358, the last prior trace was `PROTECTED_SERVICE_ADMISSION_STAGE=connect_start` before `SUPERVISOR_OS_CONTAINMENT_NOT_VERIFIED`. Crash/service-loss fixtures also failed admission, and D7C legacy Job observation remained unaccepted.
- **Committed repair:** the protected preflight regression test now models a journal-reopened interrupted job. `Quarantined` remains mandatory for live/unknown jobs.
- **Committed diagnostic:** protected client reports only fixed debug stage markers for pipe-open access denial versus absent/busy timeout versus kernel service-image/token attestation denial. D7D CI also records whether its native worker token carries INTERACTIVE SID (not the SID itself); this distinguishes CI-only logon topology from production pipe ACL mistakes. Service identity, protected ACL, hash, ownership, replay nonce and denial behavior unchanged.
- **Remaining engineering:** inspect the new *actual* pipe stage and repair precisely that access/identity issue without weakening release IPC; then prove separate-service named Job ownership, owner crash kill, service-loss cleanup, durable fsync ACK, quarantine and restart reconciliation on real Windows.

## Batch accounting

| Order | Source | Native acceptance | Decision |
| --- | --- | --- | --- |
| 1. Windows compile/fixture | Targeted repairs committed | Exact-head Windows CI pending | OPEN until compiler + tests pass |
| 2. AppContainer security | Kernel capability attestation added | NUL and network explicit denial **failed** | BLOCKED; no unsafe workaround |
| 3. Protected service/recovery | Recovery test + debug-stage triage committed | SCM IPC and hostile crash recovery **failed** | OPEN; investigate new stage |

**Next orders, after closing 1–3:** 4 Linux delegated containment; 5 activate restricted native LSP; 6 supervised build/test/DAP; 7 signed license-audited NSIS; 8 actual installed desktop E2E; 9 Wave F packaging/Git/multi-project; 10 Wave G optional authorized Lu bridge/self-development (intentionally deferred).

**Evidence policy:** Only exact-head CI native runs and installed application tests can advance accepted gates. Do not merge draft PRs or enable protected LSP from these source commits.
