# Lu IDE — Current project state (authoritative handoff)

**WO2 production NUL authority bootstrap added for native review:** fixed-service SID, elevated-only, per-boot READ_CONTROL/WRITE_DAC helper; per-job service-owned grant/revoke and boot reconciliation still open. See [batch 02](docs/IDE_WORK_BATCH_02_20261106.md). No production worker launch.

**WO2 pinned Windows native Cargo success (D7B exact commit 1090800):** real isolated Cargo Completed exit 0, output sealed and durable restart; real cancellation Job reaped and no output. CI-only exact-SID NUL permission does NOT finish production privilege lifecycle. **WO3 SCM service still denied at pipe open despite exact user SID match.** Native token diagnostic and nonblocking disposable pipe test added, protected service ACL preserved. See Batch 02 ledger.

### WO2/WO3 active Windows repairs — no release acceptance (2026-11-06)

See [batch 02](docs/IDE_WORK_BATCH_02_20261106.md): live compiler output now uses capability-rooted metadata quota scan at 250 ms, while final full content seal requires a reaped kernel Job; fixed native cap-fs Windows metadata `E0624`, applied actual Rustfmt deltas. D7D installed restricted SCM still requires native pipe admission: exact host SID/pipe-rights diagnostics and a disposable identical-DACL control were added. Service now caps independent handler threads at 12 so pipe instance exhaustion cannot terminate its protected Job keeper. Prior native AppContainer/Job admission passed; real pinned Cargo success, protected service crash/restart and production NUL permission lifecycle remain **OPEN**. No enabled release worker.


### WO2/WO3 active closeout — live native Cargo quota and exact SCM pipe denial (2026-11-06)

The [batch 02 ledger](docs/IDE_WORK_BATCH_02_20261106.md) records new Windows source work: capability-rooted live **metadata** quota scans during mutable Cargo writes (250 ms) while immutable content seal remains post-Job exit, Windows cap-fs `E0624` compiler repair, bounded native cancellation diagnostics, and confined in-root rename test. D7D protected SCM pipe denial now has exact host TokenUser diagnostics and a disposable same-ACL pipe control; production pipe DACL, host binary/hash and restricted LocalService identity were **not** loosened. Earlier exact native AppContainer and worker-containment probes pass; pinned Cargo success and D7D service host-crash/restart **do not**. Production NUL permission lifecycle is unimplemented. Draft PR #65, no release activation.


### D7D recovery ACK retry and Windows Cargo release gate (2026-11-06)

**Work orders 2 and 3 are still open.** On draft [PR #65](https://github.com/vjmichael/Lu-AI-GUI/pull/65), native AppContainer admission and Windows Job worker-containment passed on exact historical head `c6d288e`; this is **not** proof that isolated Cargo compiles or that the separate recovery service passes. The real pinned Cargo still needs complete kernel/AppContainer/NUL acceptance and release-safe privilege handling.

- WO2: `windows_null_device` now has a **compile-time refusal** of a release build carrying disposable `ci-native-null-lease`; the native Cargo host fails *before* an AppContainer, protected service lease, or one-shot grant when a safe NUL-access path is unavailable. A separate privileged, crash-safe per-job production device-access lifecycle and real rustc execution remain unimplemented/unaccepted. No broad ALL-APPLICATION-PACKAGES ACE is deployed.
- WO3: `windows_service_security` queries the created named-pipe DACL as the correct `SE_FILE_OBJECT` Windows security object type; it retains exact installer-approved host TokenUser, hash, kernel image and client PID checks. After recording and fsyncing `recovered_release`, the service retains the **original empty kernel Job** for a 60-second ACK retry window. The retry checks real Job emptiness, protected evidence and freshly authenticated client/nonce, and cannot start in live/unreaped/pending states. The independent native D7D crash fixture now verifies the second ACK and eventual bounded Job release.
- Commits: `220d4a4` (pipe object type), `57bd66f` (fsynced ACK retry and compile-time CI safety), `c6d288e` (fail-closed native Cargo before approval), `2b336e3` (native ACK/release test). **Exact-head D7D SCM service, host-crash recovery, service-loss refusal, pinned offline Cargo and installed NSIS are pending native CI/physical acceptance.** Never mark those passed from source code alone.
- The original batch remains held on WO2 and WO3: subsequent WO4–WO10 are not activated merely because GitHub jobs are queued.


### WO3 actual named-pipe ACL attestation added (2026-11-06)

`d65c202` inspects the *kernel-created* protected SCM pipe DACL prior to reporting running status: protected, exactly SYSTEM/service/exact-host SID; no unknown ACE; no client FILE_CREATE_PIPE_INSTANCE right. This follows native job `128199760472` still returning pipe access denied despite exact-user SDDL. Debug-only `3539186` will classify read/write/control access denial to guide the next targeted repair. D7D authentication, owner-crash and restart acceptance are NOT yet proven. [Batch 02 ledger](docs/IDE_WORK_BATCH_02_20261106.md).

### Confirmed D6 Windows isolation and D7C/D7D ABI repair — WO2/WO3 STILL OPEN (2026-11-06)

Native D6 AppContainer job `128199718599` actually passed zero-capability, host secret denial, live loopback network isolation and Job reaping; its workflow failed only on Rustfmt, now repaired by `6fc0b36`. The independent D7C witness returned Win32 24 because its C# Job accounting ABI was 32 instead of 48 bytes; repaired in both D7C/D7D native crash witnesses by `958609d`. Protected SCM service client job `128199760472` still got pipe access denied after the exact-user DACL and v2 config; CI/debug-only read/write/control denial classification was added in `3539186`. **Neither production Cargo null-device provisioning nor protected service authenticated host-crash/restart is accepted.** See [batch 02](docs/IDE_WORK_BATCH_02_20261106.md). GitHub native CI is verification, not a development stop.

**WO2/WO3 continuing closeout:** CI-only native NUL DACL work now additionally compile-time gated with `ci-native-null-lease` (not in release/default features). Independent D7C/D7D crash observers now record native Win32 accounting query failure codes; no failed query can certify an empty Job. The release-only NUL capability/installer approach remains an unimplemented security gate; protected SCM crash and restart proof remains open. See [batch 02](docs/IDE_WORK_BATCH_02_20261106.md). Native exact-head acceptance still required.


### Work orders 2–3: resumed implementation; native security closeout remains open (2026-11-06)

[Batch 02 code/evidence ledger](docs/IDE_WORK_BATCH_02_20261106.md): WO2 AppContainer now checks real token zero capabilities, controls before/after live network probe and child listener absence, bounds real kernel Job reaping, and has an explicitly CI-only exact temporary NUL SID grant with verified normal-exit revocation after sealed staging and protected broker approval. This is **not** production-safe across host crash, so production NUL lifecycle and real confined Cargo pass remain open. WO3 service uses installer-approved exact host account SID (not broad IU) for pipe ACL/TokenUser and v2 config, starts SCM RUNNING only after creating the protected endpoint, fixes installer PowerShell parse and bounds independent kernel Job observation. Real SCM IPC, owner-crash/service-loss restart recovery remain unaccepted. Related Windows/Linux core supervisor unit CI on historical `8e5aa5a` passed; not native OS execution acceptance. PR draft, no release worker. Details in linked ledger.


### First three-work-order batch — source repairs committed; native gates OPEN (2026-11-06)

See [IDE batch 01](docs/IDE_WORK_BATCH_01_20261106.md). Work order 1: fixed Windows LSP writer HANDLE thread transfer (`E0277`), acceptance example feature gating (`E0432`), genuine Tauri frontend assets and post-crash preflight fixture. Work order 2: actual AppContainer child kernel token capabilities inspected; Cargo still cannot open `\\.\NUL` and the D6 network denial still lacks WSAEACCES evidence. Work order 3: added fixed-stage pipe admission triage and CI logon classification; protected SCM actual IPC and restart/recovery are **not** accepted. Commits `2076532`, `0e941c7`, `3c38f5e`. Fresh exact-head CI pending. Next unblocked work remains closing orders 1–3, then 4–6; orders 7–10 later. No unsafe worker or LSP activation.


### E2 Windows CI repairs — native evidence, not release acceptance (2026-11-06)

PR #65 native Windows logs identify two independent defects: AppContainer Cargo cannot open Windows `\\.\NUL` for rustc child stdin (Cargo 101, compiler never started); a hostile rename test incorrectly required an unchanged *within-root directory name* while simulating an attacker. The latter test is corrected to require copied-file integrity, matching output digest and no external secret. Native Rustfmt 1.99 failures in service security and two probe fixtures were repaired. The device access problem **remains blocked** without explicit independent least-privilege OS authorization/proof. Cancelled checkout jobs had misleading manifest-not-found messages; they are not Cargo path bugs. See `ide/docs/E2_NATIVE_CI_FAILURE_TRIAGE_20261106.md` for exact jobs, commits and continuing gates. No protected LSP, signed installer or installed-app acceptance is claimed.


### E2B native integration continuation — 2026-11-06

- **Language routing:** the feature-gated Windows Tauri backend now selects clangd for C/C++, rust-analyzer for Rust, and Ruff for Python diagnostics only. The host independently validates filename extension and language ID, checks each server against its own compile-time approved installer hash, and publishes only installed, verified language IDs to the editor. JS cannot start unverified catalog-only servers.
- **Native unit-level lifecycle coverage:** protected gateway mocks now exercise Running → ReleasePending → Completed with durable replay, Running → CancelRequested → Cancelled on drop, and service-ACK failure quarantine after supervisor restart. These are authored source tests, **not kernel proofs**.
- **Installer staging:** ide/packaging/stage_language_server_release.py requires a reviewed two-platform manifest, exact service SHA-256, OS Authenticode check, native binary hashes and license notices before staging a per-machine NSIS resource tree. ide/app/src-tauri/tauri.e2.nsis.conf.json keeps this separate from the default non-executable alpha. Restricted service installation, signed packaged binaries and real installed-app acceptance remain open gates.
- **CI queue hygiene:** E1/E2 workflows now coalesce superseded branch jobs; exact-head native results remain queued. This is not released or installed E2 execution.

### E2 native Windows acceptance follow-up — observed failures, no release promotion

Confirmed from exact Windows CI: D7B securely launches Cargo under Job/AppContainer, but Cargo cannot open Windows `NUL` for compiler child stdin (OS error 5); nested `rustc -vV` is never executed and Cargo exits 101. **Do not widen device ACLs** or claim a compiler acceptance pass. D7D service startup status 1066/substage 103 diagnosed an incorrect primary-token argument to `CheckTokenMembership`; corrected with impersonation-token duplication at both service self-check and host service authentication, with Windows regression. Legacy unprotected-only reconciliation test now properly feature-gated and Tauri CI generates its deterministic missing Windows icon before `cargo check`. The exact CI run/job evidence and commits are recorded in `ide/docs/E2_NATIVE_CI_FAILURE_TRIAGE_20261106.md`. All changes pending new independent exact-head OS acceptance; E2 release, LSP execution and signed NSIS remain **disabled**.

### E2 follow-up native CI repair and installed desktop smoke (2026-11-06)

On PR #65: sealed Linux LSP snapshot tamper fixture repaired without weakening file permissions, exact CI rustfmt hunks applied across host/supervisor, real Windows AppContainer Cargo `Failed` stage given bounded metadata and D7D protected broker admission given debug-only stage codes. **Windows D7B/D7D kernel acceptance remains red**; no mocked receipts count as acceptance. Hosted Linux has no writable delegated cgroup; OS containment must be proven on a suitable independent runner. Added an **unsigned default-alpha NSIS** install/two launches/uninstall test path on a disposable Windows Actions host, without native LSP, service or release claims. No signed E2 binaries/installer, installed LSP editor E2E or host-crash/service restart acceptance yet. Full exact-head job list and commits: `ide/docs/E2_NATIVE_CI_FAILURE_TRIAGE_20261106.md`.

### E2 D7D/D7B native failure repair update

Actual newer Windows CI advanced beyond previous parser and VS pin failures: VS2022 Rust 1.99.1 / MSVC 14.44 and Windows SDK 10.0.26100 built the protected service/worker; sc.exe 1639 was repaired; subsequent test created/started `LuIdeRecoveryBroker` with restricted ACL evidence. The D7D fixture's legacy broker path requirement was repaired. The next protected Cargo acceptance **still failed `SUPERVISOR_OS_CONTAINMENT_NOT_VERIFIED`** (run 42286298966) and requires native stage tracing. D7B long stage returned **`SUPERVISOR_GRANT_EXPIRED`** (42286299058); source now permits a fresh host approval only *after* sealed staging, without extending the fixed 60-second grant. These repairs are **not** Windows OS acceptance passes. E2 installed NSIS and release remain explicitly disabled and unverified. Exact logs, job IDs, commits and open gates: `ide/docs/E2_NATIVE_CI_FAILURE_TRIAGE_20261106.md`.

### E2 native verification and exact-head repair ledger (2026-11-06)

Actual failed CI logs, their job IDs, and the corrective commits are recorded in `ide/docs/E2_NATIVE_CI_FAILURE_TRIAGE_20261106.md`. Repaired confirmed Windows compiler failures (Job Object unsafe-expression parser and `windows-sys` token import), absent VS 17.14 on Windows 2025 (pin offline fixture to Windows 2022), missing `rustfmt` on pinned Rust 1.99.1 jobs, and five native E1 LSP formatting hunks. D6 AppContainer exit `1099` is now classified but **does not pass** zero-network acceptance. Updated PR #65 native jobs are queued, not accepted. Do not enable/default-release the E2 worker.

## Wave E2 — Supervised language services and editor presentation (2026-11-06)

- **Independent real-server protocol smoke:** local `clangd 17.0.0` stdio fixture successfully exchanged initialize, versioned diagnostics, hover, same-file definition and completion messages. Sanitized wire frames in `ide/host/fixtures/clangd17_real_wire.json` are replayed by Rust E2 tests. This is **not** D7D contained server execution or installed IDE acceptance. Restricted clangd completion text edits are now accepted only for the current identifier prefix/cursor after human selection.
- **Branch:** \`ide/wave-e2-supervised-lsp-editor-20261106\` stacked on Wave E1 PR #64; source-only integration, not a release-ready real server session.
- **Native host protocol:** \`lsp_session.rs\` implements LSP initialize/initialized, revision-bound didOpen/didChange, hover/completion/definition requests, exact-version diagnostics, explicit workspace/configuration nulls, and denial of server-initiated edits and commands. Tests use bounded actual LSP JSON-RPC wire frames.
- **Protected supervisor bridge:** optional \`lsp-transport\` internally binds a running D3 job and plan hash, complete OS-proof contract, per-chunk quotas, and process-tree kill. It cannot launch an executable itself; Windows protected Job/SCM and Linux delegated cgroup pipe adapters remain missing and MUST be implemented before activation.
- **Human editor:** CodeMirror now supports external, exact-source diagnostic underlines, click-to-navigate problems, plain-text hover, suggestion preview and explicitly clicked completion insertion, same-file definition navigation, and source-revision invalidation. Language Services view shows reviewed descriptive providers. Actual native \`ide_lsp_worker_status\` returns \`LSP_PROTECTED_WORKER_UNVERIFIED\`; live actions stay disabled.
- **Installer:** \`ide/packaging/language-servers.manifest.json\` candidates: native rust-analyzer, clangd, Ruff; Node-dependent Pyright/TypeScript deferred. \`verify_language_servers.py\` refuses unreviewed assets, wrong digest, missing notices, path escapes, or release without explicit approval. **No server is downloaded or bundled.**
- **Status:** source changes committed with cross-platform CI/hostile checks, not native-tested or runtime-accepted. Wave D6/D7D OS security tests, true LSP restricted child admission and installed Windows/Linux editor tests still open. See \`ide/docs/IDE_WAVE_E2_SUPERVISED_LANGUAGE_SERVICES.md\`.

## Wave E1 — Free language-server catalog and framed protocol foundation (2026-11-06)

- **Source:** `ide/wave-e1-lsp-catalog-transport-20261106`, stacked on D7D PR #63. E1 is a new source-only slice and intentionally does **not** close Wave D.
- **Open-source language servers:** native host describes rust-analyzer, clangd, Pyright, Ruff, TypeScript Language Server, and VS Code-derived HTML/CSS/JSON servers. No automatic download, installation, PATH search, process launch, Lu IPC or project execution.
- **Protocol:** bounded incremental LSP JSON-RPC 2.0 stdio frame parser and serializer, strict Content-Length, typed disk+unsaved-buffer diagnostic identity, and non-executable `lsp.catalog` native capability exposed read-only through `ide_list_language_servers`.
- **Tests/CI:** Rust parser/identity regressions, JS command-boundary assertions, cross-platform E1 workflow added. Source commits only; actual native CI, installed desktop LSP features and server lifecycle acceptance **not yet passed**.
- **Next:** E2 authenticated supervisor-controlled LSP lifecycle; completion/diagnostics/hover in the human editor; eventually LSP navigation/refactoring, build/test and DAP. Block worker launch on Wave D security acceptance. See `ide/docs/IDE_WAVE_E1_LSP_FOUNDATION.md`.

## Wave D verification extension — real protected Cargo and hostile service-loss (2026-11-06)

- Added a third D7D native Windows CI job exercising the **actual** \`d7_windows_acceptance\` offline Cargo check and cancellation with \`protected-recovery-service\` and an independently installed SCM service. The app itself still exposes no Run button.
- Added a fourth Windows job forcibly killing the separate protected service while the approved fixture owner and fixed contained child are alive, requiring host-side liveness detection and Job kill, and denying restart recovery when the old named Job is absent.
- Corrected restricted-service pipe SDDL to use a protected DACL without forcing SYSTEM ownership from the LocalService process; pre-acceptance source repair, not a Windows OS pass.
- **Wave D cannot be marked accepted:** current exact-head CI remains queued; prior D6 live-loopback probe returned \`1099\` (no raw network error, not WSAEACCES), Linux delegated cgroup acceptance, signed installer and actual installed Windows E2E remain separate gates. No worker is enabled in desktop. See \`ide/supervisor/D7D_PROTECTED_SERVICE_ACCEPTANCE.md\` and PR #63.

## Wave D7D — Restricted Windows recovery service (2026-11-06)

- **Source branch / PR:** \`ide/wave-d7d-protected-windows-recovery-service-20261106\`, draft PR #63 stacked on D7C PR #62. This is NOT native Windows or installed-desktop acceptance.
- **Trust boundary:** restricted LocalService / service SID principal; protected direct ProgramData evidence root and verified ACL; installed SHA-pinned binaries; mutual named-pipe authentication by kernel client/server PID, token and image; noninteractive request scopes cannot execute commands.
- **Execution admission:** \`protected-recovery-service\` feature switches worker away from D7C same-user child keeper to independent SCM service. Explicit Global Job DACL permits restricted service to hold real kernel handle; service fsyncs READY before child resume and enforces normal empty release. Broken IPC kills and retains the Job for reconciliation, not premature release.
- **Crash recovery:** durable RecoveryRequired preflight retains quarantine, independent original Job inspection, protected service-recovery fsynced ACK, final live Job re-verification and Cancelled receipt. A missing Job or ACK remains quarantined.
- **CI:** native Windows SCM smoke and independently observed host-crash/child-kill/restart recovery jobs added in \`.github/workflows/ide-d7d-protected-service.yml\`; exact-head CI remained queued at this documentation update. Do NOT claim tests passed. See \`ide/supervisor/D7D_PROTECTED_SERVICE_ACCEPTANCE.md\`.
- **Remaining:** Windows native compile and repair; real full D3 supervisor + AppContainer worker restart; service-crash and malicious IPC fault injection; signed installed binary and earlier D6/D7B acceptance. Desktop Run and Lu Core bridge **remain disabled**.

## Wave D7C — Independent native Windows recovery keeper (2026-11-06)

- **Implementation:** `ide/supervisor/src/windows_broker.rs` plus fixed `src/bin/windows_recovery_broker.rs` now implement a second native process that independently holds the named Job before worker grant consumption / resume. Exact installed binary SHA is required by native `TrustedBroker`, and the supervisor PID + kernel creation time are authenticated against its live process handle.
- **Crash protocol:** fixed broker confirms its Job handle, fsyncs `ready` to an independent bounded hash-linked journal and acknowledges via a private parent-child pipe; the worker refuses launch or quarantines if the broker is not alive. On supervisor death the broker durably records kill intent, calls `TerminateJobObject`, verifies zero active descendants, fsyncs the observation, and retains the original named Job for up to 15 minutes for independently verifiable restart reconciliation. No automatic relaunch.
- **Private release:** normal terminal release uses a one-use nonce on an inherited anonymous stdin pipe **after** the D3 terminal receipt and empty-Job verification; no publicly signalable normal-release event. A separate named event is accepted only after parent death plus Job cleanup, and pre-signals are reset.
- **Adversarial fixtures:** native `d7c_broker_crash_fixture.rs` and PowerShell `Test-D7CProductionBrokerCrash.ps1` force a real parent crash from an external observer, prove child kernel exit and a still-live empty named Job, validate durable broker-event order, try a forged early release, and cleanly release the recovered keeper. Windows D7C workflow now builds/runs these fixtures; D7B offline Cargo fixture also requires an actual broker binary.
- **Release hold:** all latest-head Windows CI and installed Windows tests still pending. The new broker is an **independent process under the host identity**, not a separately protected/attested Windows service principal, and its same-user journal hash chain cannot authenticate itself. A signed separate-identity broker service with protected ACLs / trusted IPC is still required for malicious same-user recovery authority. No desktop/Tauri Run command activated. See `ide/supervisor/D7C_WINDOWS_CAPABILITY_ACCEPTANCE.md`.

## Wave D7C — Handle-rooted Windows worker staging and publication (2026-11-06)

- **Source:** `ide/wave-d7c-race-safe-native-worker-20261106`, stacked on D7B PR #60. No Tauri Run command or Lu Core bridge. Native worker remains an explicit `windows-worker` feature.
- **Security refactor:** new `ide/supervisor/src/windows_cap_tree.rs` pins `cap-std` and `cap-fs-ext` 4.0.3 and uses Windows directory handles without FILE_SHARE_DELETE, root FILE_FLAG_OPEN_REPARSE_POINT, handle-relative child enumeration/opens, explicit no-follow files, hardlink/reparse rejection, bounded recursive SHA-256 scanning, and native-parent-relative create-new publication. Removed the old ambient `tree_transfer`.
- **Authoritative worker integration:** the same rooted publisher now stages the verified source snapshot and pinned offline Rust/MSVC/SDK bundle, then copies and reseals outputs after Job tree reap. Exact input and bundle digests are refreshed before the one-use native grant. Reused D7/D7B receipts, Job cancellation and missing-Job quarantine without loosening denial.
- **Hostile tests/CI:** new race test attempts repeated concurrent rename/junction swaps while requiring every accepted copy to match the pinned input; native output exclusive publication, traversal rejection, symlinked parent refusal, staged tampering. `.github/workflows/ide-d7c-windows-capability.yml` runs Windows compile/tests, repeat race, formatting, desktop isolation.
- **Pending:** latest-head Windows CI, prior D6 network deny and D7B offline Cargo fixture, secure authority for root selection/AppContainer profile ACLs against hostile same-user modification, independent authenticated durable witness for a named Job absent after crash, and installed Windows app acceptance. D7C is **source-implemented, not production-accepted**. Details in `ide/supervisor/D7C_WINDOWS_CAPABILITY_ACCEPTANCE.md`.

## Wave D7B — Pinned Windows SDK and hostile acceptance (2026-11-06)

- **Source / PR:** `ide/wave-d7b-windows-acceptance-20261106`, draft [PR #60](https://github.com/vjmichael/Lu-AI-GUI/pull/60), stacked on D7 #58; preserve D6 → D7 → D7B merge order.
- **Build fixture:** `ide/supervisor/fixtures/New-PinnedWindowsToolchain.ps1` packages Rust 1.99.1, native MSVC 14.44.*, VS 17.14.*, Windows SDK 10.0.26100.0 and the actual UCRT/UM headers/libs and compiler tools into an offline bundle with bounds, no reparse points and full canonical per-file SHA-256 lock outside the runnable payload. **Release pin remains unapproved until an independently reviewed expected bundle digest is supplied.**
- **Worker integration:** D7 native Windows worker environment now uses sealed Cargo/rustc, linker, SDK binaries, `LIB`, `INCLUDE`, fixed offline Cargo target; Windows GetFileInformationByHandle rejects hardlinked files and tracks directory identity across path walks. The latter does NOT prove full handle-relative race immunity.
- **Hostile native acceptance:** Windows tests for hardlinks, directory symlinks/junctions, source parent swap, post-seal tampering, output aliasing, bundle digest drift, and missing-named-Job restart quarantine; `d7_windows_acceptance --cancel-native-worker` requires genuine OS Job cancellation, no published outputs, durable CancelRequested → Cancelled replay.
- **Independent crash observer:** opt-in D2 fixed native Job parent-crash mode plus `Test-IndependentJobCrash.ps1` separately witnesses two live descendant process handles before forcibly killing owner and confirms both OS handles signaled. This is **fixture-only**, not an authenticated D7 crash-recovery receipt.
- **CI:** `.github/workflows/ide-d7b-native-acceptance.yml` targets Windows 2025 for separate hostile, crash and full offline Cargo checks. Latest-head CI and real SDK AppContainer execution **pending**; D7B remains draft, worker remains off by default, no desktop Run or Lu Core execution bridge.
- **Native D6 blocker:** exact-source [Windows AppContainer run 42228397045](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42228397045) compiled/formatted but failed with child exit `14` (live-loopback WinSock error other than verified `WSAEACCES`). The follow-up D6/D7/D7B probe now encodes `1000 + raw WinSock error` on this failure to identify the OS condition; it remains a **failed gate**, not a network-isolation pass.
- **Remaining risk:** same-user concurrent swap race and independent durable witness for a missing Job after host crash/reboot are unresolved. See `ide/supervisor/WINDOWS_WORKER_ACCEPTANCE.md`.

## Wave D7 — Integrated Windows native worker (2026-11-06)

- **Draft PR #58:** `ide/wave-d7-windows-native-worker-20261106`, stacked on Wave D6 PR #56 source `62986780b752f7add62f93c818a8cbac796660c8`. Do not merge in reverse order. No installed app or Lu Core worker permission added.
- **Native Rust implementation:** `ide/supervisor/src/windows_worker.rs` is an explicit **off-by-default** `windows-worker` Cargo feature. It connects a reverified `PublishedInput` and host-pinned offline toolchain SHA-256 to a fresh profile-owned AppContainer staging tree, fixed Cargo check/test commands, a private named Windows Job with CPU/memory/PID quotas, and zero-capability `SECURITY_CAPABILITIES` process launch.
- **Crash-window hardening:** `PROC_THREAD_ATTRIBUTE_JOB_LIST` attaches the child to the Job *during* `CreateProcessW`, prior to any instruction. Suspended child token + SID are verified. Parent Job handle is kill-on-close; launch has no unrestricted fallback.
- **Durable outcomes:** existing D3 one-use native approval, admission, fsynced running/cancel/uncertain terminal receipts are reused; native output copying is content- and budget-checked and terminal receipt links published SHA-256. Faults immediately quarantine new work. Restart never auto-relaunches; the only completed Windows recovery path requires a still-existing kernel Job that can be killed and queried empty.
- **Open gates:** D6's `LOCALAPPDATA` Windows error-203 repair, D7 exact-head compile/format, full pinned offline toolchain+SDK, real native Cargo fixture, hostile race/cancellation/crash matrix, durable missing-Job witness and physically installed Windows acceptance. Full production worker **not verified or activated**. See `ide/supervisor/WINDOWS_WORKER_ACCEPTANCE.md`.

## Wave D6 — Windows AppContainer + Job Object native admission (2026-11-05)

- **Branch:** `ide/wave-d6-appcontainer-worker-20261105`, stacked on D5 production-worker source `341b3ab5ea268770bb01fdcf32cbb8d06452b095`. D5 Rust supervisor Windows and Linux native CI successful after formatter repair; desktop installed acceptance still open.
- **Windows fixed fixture:** `ide/sandbox/src/bin/windows_appcontainer_probe.rs` now creates a fresh *zero-capability* AppContainer profile, stages only its own fixed executable inside the profile-owned LocalState directory, uses `SECURITY_CAPABILITIES` in `STARTUPINFOEXW` with suspended `CreateProcessW`, creates/configures Job Object, assigns suspended child before resuming, and checks child AppContainer token / denied host secret read / exit / zero active Job processes. Any OS admission failure denies execution; no arbitrary app or project build.
- **Verification:** dedicated `ide-appcontainer-windows.yml` native Windows CI is required; fixed runtime test and formatting pending until exact-head result. D4's two unformatted OS rights probes are fixed in this stacked branch from the actual prior CI output.
- **D6 native CI investigation (latest code head before this documentation update: `56b0dfa15ed71e2979af9b53b7220f287b72e72b`):** initial real Windows AppContainer run [42163105747](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42163105747) compiled and formatted but failed at `CreateProcessW` (`ERROR_ENVVAR_NOT_FOUND 203`). Repaired minimal, correctly NUL-terminated Unicode `SystemRoot/TEMP/TMP` bootstrap environment; secret must preflight readable in the host then be denied with `ERROR_ACCESS_DENIED` in AppContainer; added a live localhost listener and required `WSAEACCES` instead of interpreting any network failure as isolation; reject uncontained `CREATE_SUSPENDED` orphan on Job assignment failure with direct terminate/wait. **Exact-head native runtime acceptance is pending.** These fixed hostile tests do not authorize project code, Windows Cargo execution or an installed IDE Run command.
- **Remaining:** AppContainer network-denial hostile test, secure Cargo/toolchain staging into per-job container, real Windows job/cancel/timeout worker adapter integrated with D3 grants, crash recovery, race-safe snapshot traversal and installed Windows UI acceptance. The desktop `job.run` remains deliberately absent; do not call full production worker complete.


## Wave D5 — Native production-worker implementation, gated (2026-11-05)

- **Source branch:** `ide/wave-d5-production-worker-20261105`, stacked on Wave D4 PR #54 head `32b208a76ed17910554b8ca6b7e8647577e6657a`. D3 and D4 remain separate draft PRs; do not merge D5 before them.
- **Real input staging:** `ide/supervisor/src/snapshot.rs` now copies bounded regular files from the registered source into a unique private staged directory, excluding generated caches and rejecting symlink/reparse/device files, unexpected names and budget overruns. A deterministic SHA-256 tree seal and *recipe-specific* Cargo manifest digest are reverified on staged bytes before execution. Private native source/privileged toolchain identity must still come from a trusted host component.
- **Linux native worker code:** `ide/supervisor/src/linux_worker.rs` (opt-in `linux-worker` compile feature) implements real fixed offline Cargo check/test execution via root-owned unprivileged `/usr/bin/bwrap`, isolated user/PID/network/mount namespaces, read-only `/workspace`, writable private `/output`, cleared environment, and resource-limited cgroup-v2 admission performed in `pre_exec` **before** the launcher runs. Requires delegated CPU/memory/PID controllers and actual cgroup.kill; fail closed without them.
- **Cancellation and recovery:** native cancellation requests are journaled before `cgroup.kill`. Process tree must report empty via `cgroup.events` before cancellation is completed; failure becomes uncertain/quarantined. On restart, the worker can attempt *explicit* cleanup of a matching existing per-job cgroup, never relaunch the job. Log SHA-256/size bounds, output scanner, per-file kernel limit and short runtime watchdog are present.
- **Authorization integration:** D3's native-only one-shot grant is consumed after scoped input/toolchain/OS admission evidence and before `spawn`; durable admission/running/terminal records are appended with native host state. The separate D1 review journal still cannot mint an executable grant.
- **Not yet ready for user execution:** Linux actual worker integration with installed Tauri, Windows restricted/AppContainer+Job worker, race-safe cap-std source traversal, OS crash/kill hostile acceptance and physical installed Windows acceptance are **not complete**. No production `job.run` command is advertised. CI compile/tests and actual delegated cgroup runtime validation pending; do not claim Wave D complete.


## Wave D4 — Native restricted-rights admission probes (2026-11-05)

- **Branch:** `ide/wave-d4-native-rights-admission-20261105`, stacked on D3 source commit `080b15dbfbc231dc2b0b691cb65a03b20d1c5794`. Production supervisor remains independently gated and disconnected from Tauri.
- **Windows:** new `ide/sandbox/src/bin/windows_restricted_token_probe.rs` validates OS `CreateRestrictedToken` using a fixed Restricted Code SID plus disabled maximum privileges, requiring `IsTokenRestricted`. No process launches. Restricted token creation does NOT by itself prove low integrity/AppContainer, filesystem or network access, or safe worker activation.
- **Linux:** new `ide/sandbox/src/bin/linux_cgroup_probe.rs` derives current cgroup-v2 scope, refuses unsafe paths, and attempts to configure/read back CPU/memory/PID limits on a unique empty delegated child. No process launches. Missing delegation reports **DENIED** even if the workflow itself finishes successfully. This is not evidence of pre-exec process placement or subtree kill.
- **CI:** dedicated Linux and Windows native rights-probe workflow created, exact-head results pending. No Tauri command, no general build/test execution, and no Lu Core bridge.
- **Still open:** Windows low-integrity/restricted child and ACL/network tests, Linux cgroup atomic placement/hostile load, real worker cancellation and crash/recovery, secure native input snapshot, protected OS broker and installed Windows acceptance.


## Wave D3 — Production supervisor control-plane implementation (2026-11-05)

- **Branch:** `ide/wave-d3-supervisor-core-20261105`, stacked on corrected D2 Windows Job Object probe commit `c091fbe174b85a0c0f90c67933fdb2069000cd24`. D2's [Windows native runtime+format workflow](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42158228849) and continuity gate passed on that source SHA. Installed Windows acceptance is still separate and open.
- **Implemented:** Independent `ide/supervisor` native control-plane crate, not connected to Tauri, Lu Core or the IDE's Run button. It models allowlisted build/test jobs with exact Git commit, host input-snapshot/toolchain/manifest digests, effect class and bounded runtime/memory/PID/log quotas. A fresh one-job, one-session, 60-second in-memory grant is bound to the plan hash and current source. Durable receipt state transitions and SHA-256 hash linkage detect corrupted/truncated history (not cryptographically authenticated external receipts).
- **Cancellation and recovery:** native-private broker interface records cancellation request before invoking process-tree termination; only explicitly verified descendant kill can produce a cancelled receipt. Failed kill becomes `uncertain`, blocks further work. Interrupted admission/running/cancel attempts are quarantined on journal reopen; cleanup requires native broker proof and no automatic retry.
- **Execution remains disabled:** `production_worker_admission` always fails closed until OS-specific restricted rights, filesystem/network access policy, CPU/memory/PID quotas, real worker-snapshot publication, crash-orphan resolution and platform hostile tests are complete. No release-build spawn API, worker bridge or frontend Run command added. D1 review entries cannot mint grants. Unit tests use **simulated** OS broker only.
- **Verification status:** dedicated Windows/Linux core test workflow, plus Node static desktop boundary test added; latest-head CI pending. D3 is the supervisor foundation, NOT complete actual worker execution, installed desktop acceptance, or closed Wave D.


## Wave D2-W1 — Windows fixed-fixture Job Object containment probe (2026-11-05)

- **Source branch:** `ide/wave-d2-windows-job-probe-20261105`, stacked on D1 PR #51 SHA `ef623bfd22f8dfc8b1674a44911e98825be8ab53`. This is a standalone OS diagnostic, **not** a production worker, and does not alter the Lu IDE app or Core.
- Reused the existing `ide/sandbox` experiment (Linux Bubblewrap native probes already documented). Added isolated `src/bin/windows_probe.rs` with a fixed self-executing Windows fixture, Win32 Job Object limits and **suspended-create → assign-to-job → resume** ordering. Any OS API admission failure kills/denies the child, no fallback.
- The probe tests a successful contained child, a timed-out fixed sleeper terminated via `TerminateJobObject`, and a fixed grandchild killed with the parent Job. Uses `KILL_ON_JOB_CLOSE`, a 128 MiB job memory quota, active process cap 4, no inherited handles, and minimized environment (SystemRoot/TEMP/TMP). All modes are compiled in; no user/model arbitrary commands or arguments.
- A dedicated `.github/workflows/ide-sandbox-windows.yml` runs the **actual Windows native** probe; docs at `ide/sandbox/WINDOWS_JOB_PROBE.md`. No test result is claimed before its CI runs. Linux existing 5/5 Bubblewrap probes remain historical reference, not a new D2 pass.
- **Still open:** Windows restricted token/file/network confinement, worker exact input snapshot, credential broker, cancellation races, crash-restart orphan reconciliation, Linux process quota/descendant admission, actual build/test worker, physical installed Windows acceptance. Do not wire a Run button until separately proven.


### D1 first-head CI repairs
- Initial D1 frontend workflow had 55 passing tests, one failing contract test: an existing static parser mistook the Tauri-managed `intents` state handle for an external request field. Filtered `intents` as host state; the JSON schema still accepts only `projectId` and `recipeId` for prepare.
- Initial Linux native host suite compiled and tested successfully, then failed formatting only in the new `execution_intents.rs`. Applied **all 34 actual rustfmt hunks** from the exact CI job log and made the fixed recipe array `static`. Exact-head rerun required; not yet proven on revised source.
- Windows desktop compilation and continuity gate on the prior head are not a substitute for latest-head acceptance; installed physical Windows UI remains unverified.


## Wave D1 — Host-authorized execution-intent protocol (2026-11-05)

- **Implementation branch:** `ide/wave-d1-execution-intent-20261105`, stacked on C6 PR #50 exact SHA `c403a8ba47d6ac1662b8627ce5ab34a9a16b591e`. C6 source CI passed Linux, frontend, Windows Tauri, and continuity on that SHA; installed Windows app acceptance remains separately unverified. Do not merge D1 before C6 and its C3/C4/C5 ancestors.
- **D1 scope:** Native `execution_intents.rs` defines a host-owned, append-only, private `execution-intents.jsonl` journal. Three typed operations: `job.prepare`, `job.review`, and `job.list`. The human-facing **Job requests** view records and reviews intentions, not execution. Fixed allowlisted Rust check/test recipes only; no model-supplied commands, file paths, arguments, environment variables or executable launch path.
- **Preconditions:** a registered active Git project, clean observed HEAD and no tracked/untracked modifications, known fixed `Cargo.toml` or `ide/host/Cargo.toml` manifest with bounded SHA-256 digest and symlink/reparse checks. Preparation expires in 120 seconds, is scoped to one IDE session, and records opaque native IDs. Recording review re-reads source SHA and manifest digest; changing project/source/session or expiration blocks approval. A previous review can be withdrawn; no reusable grant is minted.
- **Effects and recovery:** Journal appends are mutex-serialized, bounded, synced, and checked for valid monotonic transitions. Corrupt/truncated journals fail closed; on restart, unreviewed preparations require fresh review. There is **no process spawn**, no worker, no OS containment assertion, no execution token, no Git mutation and no Lu Core bridge. Approval notes are not execution authority. Required Windows Job Object/Linux cgroup containment, worker budgets, kill/recovery tests, and signed effect receipts remain **Wave D2+ gates**, not complete.
- **Verification:** Native Rust journal tests and frontend type/schema/UI checks added. Exact-head D1 Linux, Windows, frontend and continuity acceptance are PENDING; physical installed Windows UI checks remain unverified. The project root/Delivered folder on the user's physical machine is not inspected.


## Wave C6 — Task evidence and development ledger integration (2026-11-05)

- **Source branch:** `ide/wave-c6-task-ledger-20261105`, stacked on exact C5 source `1dd4a58d84d6bc5f383f75e463b15f82baea6de8` (C5 PR #49). Earlier C3/C4 are passing native/frontend/continuity GitHub CI on their own heads, but no physical installed Windows acceptance. The standalone IDE and Lu Core remain independent.
- **One private ledger:** C6 extends existing `development-ledger.jsonl` events with an optional, strictly typed task payload and host-owned task materialization. Legacy events remain readable; a task change appends a new event and never rewrites old history. Snapshot of local Git HEAD and dirty state is captured by the host at task update time.
- **Human task claims:** native `task.update`/`ide_record_task_update` supports bounded task IDs/titles/next actions, status `planned | in_progress | blocked | ready_for_review`, and user-selected gates `source_review | exact_head_ci | installed_windows | human_approval`. No `completed` input is allowed. Maximum 64 tasks per registered project.
- **CI-linked evidence:** `task.ciObserve`/`ide_observe_task_ci` independently invokes the trusted exact-HEAD public GitHub inspector, binds its bounded summary to the *latest* human task event ID and current source commit, and appends a separate observation. The frontend never supplies the CI verdict, workflow IDs, credentials, Git SHA or any acceptance claim. An intervening task edit or changed HEAD invalidates the old CI link.
- **Evidence gates:** host projects per-task gate statuses. Successful commit CI can produce `commit_ci_passed_not_task_accepted`; it never proves unsaved buffers, installed Windows UI, a human source review, or final task acceptance. The Continuity UI supports task updates and explicit CI observation, while the portable checkpoint exports only task IDs, statuses, commit/evidence coordinates and explicit unknowns (no task title or private next-action text).
- **Security:** app-profile-only append under one mutex, no project file mutation, no arbitrary process execution, no Lu runtime bridge. Native test fixtures and Node frontend contract/privacy tests added. **Verification:** exact-head C6 Linux, Windows, frontend and continuity CI not yet accepted; physical installed Windows UI and actual live public CI lookup in an installed app remain open.


## Wave C5 — Exact-head CI receipts (2026-11-05)

- **Implementation branch:** `ide/wave-c5-exact-ci-evidence-20261105`, stacked on C4 commit `ca606dfb378ce31945a768a23e9ab6a31cbe60e8`. C3 PR #47 and C4 PR #48 remain independent/open pending acceptance; do not merge C5 before dependencies.
- **Source implementation:** `host/src/github_ci.rs` read-only public GitHub Actions inspector and typed `git.ciExactHead` host capability/`ide_read_exact_head_ci` Tauri command. An explicit user click queries only `https://api.github.com` for the active registered Git project's validated origin and exact local HEAD SHA. No cookies, tokens, shell, Git fetch/push, source writes or background polling.
- Workflow receipts check the *specific* `ide-standalone.yml` (required: host-linux, frontend, desktop-windows) and `ide-continuity.yml` (required: continuity) workflow paths, matching run head SHA, matching job head SHA when present, completion and success. Missing/duplicate jobs, partial responses, failure/pending runs or local HEAD changes never count as passing. Unknown origin, private repo/rate limits, absence of runs and incomplete evidence remain explicit.
- Human Source control view has **Check exact HEAD CI**. C4 evidence matrix and C3 Copy development checkpoint accept only cached, explicit, current SHA-bound CI receipt metadata; copying has **no** additional network request. Dirty worktree/unsaved drafts, installed Windows application, toolchain usability and individual task completion **remain unverified** by CI.
- **Implementation verification status:** Rust pure fixture tests and Node regression tests added; exact-head Linux/frontend/Windows CI and installed Windows UI acceptance **PENDING**, not passed at authoring. No new installer claimed; local machine's Deliverables/resources uninspected.

### C4 frontend CI regression repair (2026-11-05)
- Earlier C3 exact-head run 42145804942: Linux native and Windows desktop succeeded, frontend failed a **test-only** capability-registry regex expecting same-line Rust macro arguments. Updated the contract assertion to support multiline rustfmt and require 24 nonempty commands. Direct catalog/Tauri parity: 24=24, no missing or duplicate commands.
- Wave C3 source was separately repaired at `efe731d874e11309f4646d150827a723ed9932f7`. This C4 stacked branch carries the same test repair independently. Latest C4 exact-head CI, installed Windows UI acceptance and toolchain execution remain open.

## Wave C4 — Development evidence reconciliation (2026-11-05)

- **Implemented in source on independent branch** `ide/wave-c4-evidence-reconciliation-20261105`, based on exact Wave C3 SHA `2572fa445d31d265b0ef27ef52a1dd4a09156326`. No Lu bridge or executable worker.
- A pure `development_reconciliation.js` derives ten human-readable evidence classifications from the existing native project context + inventory comparison. The Continuity screen labels local Git, stored upstream, live remote (not queried), bounded artifacts, historical baseline, project task claims (unverified), host events, native CI, installed app, and toolchain usability separately.
- A clean observed Git tree or matching SHA never implies source build success or installer acceptance. Unknown/mixed project IDs fail closed; incomplete scans and truncated paths remain explicit. Project-authored document text is never used as authoritative task completion.
- The human Copy development checkpoint export embeds the conservative matrix; no new Tauri capabilities/permissions or project writes. JS tests cover contradictory Git state, changed/missing bytes, incomplete scan, absence of CI evidence, privacy and UI wiring.
- **Verification:** source committed but full latest-head native Linux/frontend/Windows CI and installed Windows GUI acceptance pending. No package or CI receipt claimed for C4 at authoring. Worktree/Delivered on the user's Windows device remains uninspected.


## Wave C3 — Portable development checkpoint (2026-11-05)

- **Implemented in source on draft PR #47:** Human **Copy development checkpoint** in Continuity performs fresh parallel native context and inventory comparison reads for the exact currently selected registered project and copies bounded versioned JSON. Project IDs must match both host responses; switching projects invalidates pending requests and any stale continuity UI. Clipboard failure is surfaced, not treated as success.
- Captures local Git HEAD/branch/dirty counts and stored tracking evidence, bounded artifact SHA/size/status, baseline changes, recent event IDs/saved revisions, and orientation document revision IDs. Omits source and note excerpts, event summaries, private absolute roots, credentials, executable work and live GitHub content. Limits and unknown states are explicit. **Two reads are not an atomic transaction**; copy time is not evidence time.
- New pure helper and Node tests: `app/web/development_checkpoint.js`, `app/tests/development_checkpoint.test.mjs`. Local isolated Node helper tests passed (3) before repo integration; full frontend/Windows CI and installed Windows copy UI still pending at PR head.
- Latest verified predecessor commit before this checkpoint: `26040e8331f5a5b8053f4af8248a88d9d551f04f`; new source SHA must be taken from GitHub after publication, not inferred. Local user's checkout/Delivered still not inspected. Lu bridge remains deferred.

**Observed date:** 2026-11-05 (America/Phoenix)  
**Repo:** `vjmichael/Lu-AI-GUI` · **Working branch:** `ide/wave-c4-evidence-reconciliation-20261105` (stacked on Wave C3 PR #47; unmerged). Wave C3 remains on `ide/wave-c-development-truth-20261105`.  
**Last inspected implementation source SHA:** Prior C3 source commit `d6bdca75953ab63579be5d071ec706effe4c6771`; inspect latest [draft PR #47](https://github.com/vjmichael/Lu-AI-GUI/pull/47) HEAD for the current source and exact-head validation.
**Current source validation:** Wave C3 source and tests committed; exact-head Linux/frontend/Windows checks and installed Windows clipboard acceptance **pending**. First C3 matrix [42145655561](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42145655561) queued after prior source. A subsequent race-state cleanup supersedes it; inspect the final PR HEAD, not prior runs.
**Local checkout, `Delivered` folder and untracked work:** **UNKNOWN / NOT INSPECTED** in this session. Never assume GitHub is ahead of inaccessible local work.  
**Live Lu↔IDE connection:** **DEFERRED by user**. The IDE remains standalone.

## Current status by capability

| Milestone | Source | Verification | User acceptance |
| --- | --- | --- | --- |
| R1–R3 project registration, explorer, CodeMirror, explicit save | Implemented | Earlier CI and Windows user smoke; see [handoff](ENGINEERING_HANDOFF.md) | Earlier core editor milestones accepted |
| R3.4 multi-hunk review and revision conflicts | Implemented | Earlier native CI accepted; see [R3.4](docs/IDE_07_R34_MULTIHUNK_PROPOSALS.md) | Earlier Windows smoke accepted |
| R3.5 proposal inbox and editor-only staging | Implemented | Earlier native matrix passed; see [R3.5](docs/IDE_07_R35_PROPOSAL_INBOX.md) | User reported working staging flow; full hostile gates open |
| R3.6 success banner, R3.7 Find in File | Implemented | [Source CI 42069988384](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42069988384) passed for R3.7 SHA `eb0aefd` | New GUI acceptance still open |
| A1 saved-file source spans, Copy Source Reference | Implemented | [CI 42075298457](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42075298457) Linux/frontend/Windows passed for SHA `467775e` | New clipboard UX still open |
| **A2 typed Semantic Capability Registry** | **Implemented**: 21 real host operations, JSON Schema v2, self-contained nested definitions, native-argument parity tests, Capabilities UI | **Exact-source CI pending** on latest code | Windows installed UX open |
| **A3 Development Ledger and Context Read Model** | **Initial runtime implemented**: host app-profile JSONL event history, bounded project context and Continuity UI | **Exact-source CI pending** on latest code | Windows installed UX open |
| A3 bounded local artifact and toolchain-candidate inventory | **Implemented in source** | CI on latest source pending | Installed acceptance open |
| A3 persistent snapshot and compare | **Implemented in source** | Native regression fixtures added; CI pending | Installed acceptance open |
| A3 native local Git dirty/untracked inspection | **Implemented in source** | Exact-head CI pending | Windows UI open |
| Wave C1 local staged/unstaged Git diff/history | **Implemented on isolated branch** | CI pending | Installed UI open |
| Wave C2 targeted resource SHA probe | **Implemented on isolated branch** | CI pending | Installed UI open |
| A3 full live GitHub and external toolchain reconciliation | **Not implemented** | Not tested | Open |
| Live Lu connector, worker execution, LSP/build/test/package | **Deferred/not implemented as IDE runtime** | Not tested | Open |

## A2 — Typed contracts actually implemented

- **Source:** `host/src/capabilities.rs`, `host/src/capability_schemas.json`, standalone Tauri `ide_list_capabilities`, React Capabilities screen.
- Host-created, versioned descriptors for **21 existing** project, source, review, save, proposal, continuity, inventory and Git/GitHub operations. Request/result JSON schemas are typed, with nested structures and self-contained JSON Schema 2020-12 definitions. Response includes typed error schema. **Discovery is not permission, execution or a Lu bridge.**
- UI shows concise operation input/result signatures, with full schema expansion on demand. Rust validation and frontend structural/command parity tests added.

## Wave C — Development truth (independent source implementation)

- [Draft PR #47](https://github.com/vjmichael/Lu-AI-GUI/pull/47), source repair SHA `dbaa04ff`. First-run [42143131954](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42143131954) frontend passed, Linux failed E0308 in resource timestamp conversion. Converted cap-std time to std time before Unix offset; new exact-source [42143391542](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42143391542) pending. Native Rust/Windows launch and user acceptance remain open.


- Isolated feature branch `ide/wave-c-development-truth-20261105` while Wave B PRs remain in their own GitHub acceptance queues. **Do not mark the completed Wave B source work as Windows-accepted yet.**
- Implemented `host/src/git_evidence.rs` to inspect staged/unstaged local Git patch text and 30 local commits, bounded to 64 KiB of patch and 80 changed files. It refuses parent repository inspection, excludes untracked *contents* from patch, performs no Git command or mutation, and labels remote state as `not_queried`.
- Added typed `git.diff` and `git.history` and an explicit human **Inspect diff and history** action in Source control.
- Implemented `host/src/resource_probe.rs` for exact relative-path SHA-256 and metadata under the registered project. Missing/oversized/not-regular/changed files are not mislabeled as verified. **Executable readiness remains not tested**; no shell, package install or Lua/Lu bridge.
- Added typed `resource.probe` and a **Recheck file bytes** button in Continuity. The registry now describes **24 native operations** with typed schema parity verified structurally (24 descriptors / 24 schemas / 25 registered handlers including discovery).
- Native fixture tests cover Git patch separation, bounded history, root isolation, symlink and traversal, resource SHA and missing/over-limit cases. Frontend contract tests and isolated PR CI trigger added. Full semantics: [Wave C development truth](docs/IDE_WAVE_C_DEVELOPMENT_TRUTH.md).
- **Acceptance status:** source implemented; exact-head Linux/frontend/Windows CI and installed Windows UI remain open. No global executable toolchain inspection, authenticated private remote comparison, or real worker execution claimed.

## Wave B1 — Human Find and Replace (source implemented, exact-head CI open)

- `app/web/find_replace.js` is a pure bounded literal match-and-replace planner for the **active unsaved CodeMirror draft only**. Matches preserve UTF-16 offsets, escape regex metacharacters and optionally match case. All-or-nothing replacement refuses scans above 10,000 matches; no partial bulk edit or direct disk write.
- `app/web/main.jsx` adds Ctrl+H, human Find/Replace controls, Replace One and confirmation-gated Replace All. All edits enter the existing draft/dirty state and still require explicit Save and revision-checked host writes.
- `app/web/code_editor.jsx` dispatches a planned set of changes as **one undoable CodeMirror transaction**, checks the expected draft before applying and acknowledges/consumes the nonce to prevent replay when tabs switch. No Lu model or file/host tool added.
- New `app/tests/find_replace.test.mjs` includes UTF-16 Unicode offsets, literal metacharacter search, case-sensitivity, literal replacement of dollar strings, match selection, over-limit refusal and empty query. Updated existing frontend regression and editor styles.
- **Verification:** implementation source committed; CI pending. The previous native Git/GitHub CI run for `b39c6267` is no longer an exact-head check for this new source. CI concurrency may cancel superseded branch runs. No new Windows installer or physical acceptance claimed.

## A3 live GitHub evidence and native Git CI repair

- `host/src/github_live.rs` provides an explicit read-only check against public GitHub origin HEAD, exposed through typed `git.compareLive`, Tauri `ide_compare_live_github`, and a human **Check GitHub** button. The catalog contains **21 actual native operations**.
- Origin is syntactically restricted to `github.com`, with fixed `https://api.github.com` REST, no redirects, credentials, shell commands or Git mutation. TLS query has short timeouts and a 48 KiB response bound.
- Separately reports time-stamped GitHub branch SHA, local HEAD, stored tracking SHA, local dirty/untracked status and **SHA equality or difference without assuming ancestry**. Private/rate-limited/unavailable GitHub remains unverified; no automatic request or Lu connection.
- CI [42128252074](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42128252074) exposed E0505 dropping a Git repository before a borrowed Git tree during test teardown. Fixed by releasing the tree first, in source `b39c6267`.
- CI [42128928605](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42128928605): **Windows host and Tauri checks passed**; Linux/frontend queued at last inspection. No physical installed Windows UI acceptance or local user filesystem inspection.

## A3 native Git worktree reader — implementation committed, CI pending

- Native Git dirty-status regression now alters file byte length to avoid a same-size timestamp/stat-cache race. Latest source commit `27e6dde8cf6bd12e6ea2fbe19b27c2fb8f74c89f`; [CI run 42128252074](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42128252074) queued; no recent compile or physical Windows acceptance result for this SHA.


- Activated the **Source control** sidebar page (previously 'Soon'), backed by new read-only `ide_read_git_status` Tauri command and typed `git.status` capability. UI shows exact local branch, modified/staged/untracked counts, local commit and upstream difference, limited change list and explicit no-live-GitHub disclaimer.
- New native operation increases capability registry to **20** descriptors, matching 20 implemented non-discovery Tauri commands. Structural audit found no argument/schema mismatch. Source revision `4d1a559d2f37afb062524827a6d23aff6e62f9b5`; full Linux/frontend/Windows acceptance remains open.


- Latest implementation SHA `a37be15b808b2d238ac2457aee9e357d056be13f`, source-specific verification [run 42127486856](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42127486856) queued. Tests include native Git fixture + baseline transition; Linux build and Windows installed acceptance remain open.

- `host/src/git_status.rs` uses native `git2` / libgit2 to inspect the **exact registered project root** without launching `git` or executing project code. Refuses accidental parent-repository discovery.
- Returns branch, local commit ID, locally configured upstream tracking commit, locally computed ahead/behind counts, true modified/staged/untracked file counts and at most 120 changed-path summaries. The Continuity screen displays these separately from older loose-ref evidence.
- **Not a live GitHub comparison**: no fetch, push, network access, remote PR inspection or authority to mutate Git. Worktree inspection can be unverifiable, and none of its observations grant permission to write.
- Added native Git fixture tests for clean -> dirty/untracked and parent-repo refusal, plus frontend/schema checks. A recorded inventory baseline now includes the observed clean/dirty worktree state, and comparison reports a transition such as `became_dirty` rather than assuming the same commit means no local changes. `git2` now a standalone host dependency; native Windows/Linux CI must compile and run before marking this accepted.
- CI: [run 42127100529](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42127100529) pending at last check; previous build `42125275258` had Windows success, but its Linux/frontend jobs were cancelled as source evolved.

## A3 — Bounded local evidence inventory and saved baselines

- **Implemented in source:** `host/src/inventory.rs` performs a bounded, nonexecuting scan of known folders such as `Delivered`, direct root-level installers, build output, benchmark/result folders, and selected project-local tool/bin directories. Files receive observed byte size, SHA-256 if at most 64 MiB per file and within a 128 MiB total scan budget, category, and explicit `present_unverified` status. No shell, package install, app launch, network request, or ambient filesystem search.
- **Local Git:** reads only `.git/HEAD` and loose local/remote-tracking refs through the project capability. It can report equal/different *locally stored* refs, but does **not** verify a working-tree diff, packed refs, linked Git worktrees or current GitHub HEAD. Those are marked `not_verified`/`not_queried`.
- **Durable user-triggered baseline:** `host/src/inventory_history.rs` preserves a bounded append-only history (up to 64 snapshots total, 1 MiB storage limit) of observed artifact paths, sizes and SHA-256 digests in application-profile `evidence-baselines.json`. Later scans compare with the latest snapshot and classify known files as `matched_bytes`, `changed_bytes`, `missing_in_scanned_scope`, `new_in_scanned_scope` or `unverifiable`. An incomplete scan never establishes missing evidence. Histories persist across restart, but the snapshots prove byte observations, **not executable readiness or benchmark comparability**.
- **UI:** Continuity now shows evidence files and their digests, observed Git refs, a human **Record evidence snapshot** control and baseline comparisons. The capability catalog includes two typed host operations, `inventory.capture` and `inventory.compare`. Native fixture tests and frontend parity tests added.
- **Scope limits:** only registered project roots are scanned (including direct root-level installer candidates so a manually selected `Delivered` folder can be inspected). Toolchains installed elsewhere on the user's machine, older `Delivered` folders outside the chosen root, live remote GitHub, actual dirty/untracked diffs, native installer launch and benchmark reruns remain **not inspected** without separate explicit access and worker authorization.
- **Verification:** source committed, pending native/Windows CI [run 42125275258](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42125275258). No physical Windows acceptance and no new installer claimed.

## A3 — Current, narrowly scoped first slice

- **Source:** `host/src/continuity.rs`, `host/tests/continuity.rs`, standalone Tauri `ide_read_project_context`, React Continuity screen.
- Private app-profile `development-ledger.jsonl` receives **best-effort host observations after successful actions**: project register/open/close/unregister and explicit revision-checked source save. Saved events include **validated relative source path** and saved SHA-256 revision. No project root or absolute local path stored in event.
- The read model requires an **active registered project**, returns at most **40 recent events** and twelve bounded root/`ide/`/benchmark orientation document slots with exact saved revisions and excerpts. Project-authored documents are labeled **untrusted_project_text**.
- Remote Git state: `not_queried`. Local artifacts: `bounded_scan`. Toolchain readiness: `not_verified` or `present_unverified` for scoped candidates. These describe exactly what was and was not inspected, not false missing/ready claims.
- **Limitations:** Observations are NOT crash-safe effect receipts, verified CI results, operational resource probes, Git reconciliation or authoritative build history. Failed ledger appends are logged separately and do not convert a successful file write into a retryable failure. A corrupt ledger fails closed on read. True effect journaling, verified external resources and complete local/remote reconciliation remain open. The bounded artifact inventory and persisted byte snapshots are implemented as separate services.

## Next unblocked tasks (order matters)

1. **Close exact-source CI:** Linux Rust unit tests, `cargo fmt --check`, frontend build/tests and Windows Tauri/native checks for A2/A3; fix actual failures before claiming acceptance. Then test Capabilities and Continuity screens on a Windows build. No new installer is verified.
2. **A3 follow-up:** verify exact-source native CI, fix formatting/build errors, then expand beyond bounded project roots only through explicitly registered locations. Real executable version/OS checks require supervised workers and permission; benchmark comparability and signed build/test receipts remain open.
3. **Project reconciliation:** inspect *real* local worktrees (including dirty/untracked state), accessible out-of-project `Delivered` folders, live remote HEAD/PRs, benchmarks and package hashes. Classify `LOCAL_ONLY`, `REMOTE_ONLY`, `MATCHED`, `DIVERGED` or `UNVERIFIED`; no destructive merges without human consent.
4. **Standalone editor hardening:** Find/Replace and unsaved-buffer revision identities; retain approved proposal staging, explicit Save and host revision checks.
5. **Execution security before LSP/build/test/package workers:** OS containment and user authorization gates remain mandatory; future Lu integration remains deferred.

## Handoff and artifact truth

- [Chronological changelog](CHANGELOG.md), [continuity protocol](CONTINUITY_PROTOCOL.md), [engineering handoff](ENGINEERING_HANDOFF.md), [capability-first architecture](docs/CAPABILITY_FIRST_ARCHITECTURE.md).
- Repository branch source was inspected using the GitHub connector. No local checkout or user Windows filesystem was inspected. **No R3.6–A3 Windows installer or local NSIS package has been verified in this session.**
- Do not infer that a Git commit is a tested build, that a CI success is an installed launch, or that historical resource memory authorizes execution.
