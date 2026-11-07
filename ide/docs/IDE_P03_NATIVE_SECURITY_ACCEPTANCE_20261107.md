# Lu IDE — P03 Native Security Acceptance (WO4–WO6)

**Opened:** 2026-11-07. **Owner-approved progression:** Move to P03 while P02 exact-head native/installed acceptance is still open. Do **not** reinterpret unfinished P02 checks as PASS.

**Stack:** `ide/release-closeout-p03-20261107` on P02 [draft PR #79](https://github.com/vjmichael/Lu-AI-GUI/pull/79), itself on P01 [draft PR #78](https://github.com/vjmichael/Lu-AI-GUI/pull/78). No edit to Lu Core, no WO10, no direct Lu model execution or fallback outside protected workers.

## Evidence established before this phase

- [WO6 P01 source-head Linux LLDB-DAP OS run 42355877315](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42355877315): real Linux cgroup membership, protected read/write and initial adapter response were observed. The adapter then produced repeated `no_stdout_yet` before a **20-second initialize wait failure**. The fixed `probe_lldb_dap_initialize.py` independently reported all three unprotected distribution/copy/Bubblewrap initialization preflights passed. Source inspection plus the [official DAP sequencing specification](https://microsoft.github.io/debug-adapter-protocol/overview) identified a client deadlock: Lu IDE awaited an `initialized` EVENT before the protected `launch` request, but a compliant adapter can send that event after `launch`. This is a source defect, not evidence that the kernel cgroup or privilege boundary failed.
- The same WO6 run: native Windows protected LLDB-DAP failed after AppContainer suspended-child attestation, live SCM lease, Running fsync, resume and gateway attachment; the process exit was **3221225781 / 0xC0000135** (Windows DLL dependency not found). Specific missing DLL/ACL is not yet identified; no permission or PATH bypass authorized.
- [P01 source-head E2 protected clangd run 42355877461](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42355877461): Windows native `clangd` reached restricted LocalService/SCM, AppContainer suspended attestation, Job, Running receipt, writer and gateway; then exited 1 with `permission_denied`, denied-resource class `unspecified`. Linux delegated clangd stopped before the bundle root owner/mode verification at `bundle_root_metadata_read` with `SUPERVISOR_OS_CONTAINMENT_NOT_VERIFIED`. These are open, not successful tool operation.
- This task's Debian container is cgroup-v2 **read-only**, with no Bubblewrap and no Cargo/Rust toolchain installed; it cannot provide an honest positive kernel-delegation, crash recovery, Windows AppContainer or installed desktop witness. Its clangd and LLDB binaries are not substitute release bundles.

## Implemented source changes

### P03-A — LLDB-DAP handshake and scoped configuration

- Changed `DapSession`: native `initialize` RESPONSE moves the protocol to Ready for the **already separately authorized fixed self-test launch**; `initialized` EVENT is now accepted after initialize and before/after launch. Removed accidental requirement for an early event before launch.
- Exactly one empty `configurationDone` request is permitted only after **both** a pinned native `launch` request and an adapter `initialized` event. Request/response correlation remains bounded; duplicate events, premature initialized, reverse `runInTerminal`, arbitrary target and unrequested commands remain rejected/quarantined.
- Wired `configuration_done` through only the existing supervised Windows/Linux DAP gateways and the fixed native acceptance fixture. This does not enable generic launch, attach, evaluate, shell, network, model or desktop authority.
- Added protocol regressions for early event denial, response-first launch, configurationDone ordering, duplicate rejection, strict fixed executable selection and exact response correlation.

### P03-B — Windows DLL/clangd failure classification

- Added fixed, nonidentifying Windows native exit categories for missing DLL, failed DLL initialization, kernel access denial and invalid PE format. This is debug-only evidence, not a grant.
- Added bounded, test-only Windows copied-bundle LLDB-DAP initialization probe and Python parser/unit regression. The probe runs **before** the separately restricted AppContainer positive test so a regular-token DLL dependency failure can be separated from a genuine restricted-AppContainer failure; it never launches a debuggee or counts as OS containment acceptance. Existing native SCM lease, exact Job, SID, token, SHA pinning and cleanup remain mandatory.
- Kept the original protected Windows `clangd` permission-denied root cause OPEN pending new exact-head stderr categorization and OS trace. No broad DACL/ACL, HOME/PATH, process token or AppContainer capability relaxation.

### P03-C — Linux bundle ownership and delegated security evidence

- The Linux root-owned native LSP bundle admission gate now reports fixed nonidentifying failure classes (non-directory, symlink, not root-owned, group/world writable) at the exact failing stage. Each still rejects the bundle. This allows targeted repair of hosted provisioning rather than bypassing the trusted ownership predicate.
- New `ide-p03-native-security-checks.yml` runs exact-head Win/Linux host DAP unit tests, full optional LSP/DAP supervisor compilation and tests, Rust formatting, Windows loader parser unit tests, and Linux native sandbox tests. The matrix distinguishes normal compilation from genuine independent OS witness. Existing WO4/WO5/WO6 native acceptance workflows remain enabled; no CI evidence has been promoted from a failed or prior head.

## Still outstanding — P03 cannot close until all observed and accepted

- [ ] WO4 native Linux cgroup-v2 delegation, **actual** Bwrap filesystem/network hostile tests, quotas, child/grandchild kill, no-replay restart and real kernel `populated 0` cleanup on a separately provisioned eligible host.
- [ ] WO5 Windows true protected `clangd` initialize/diagnostics/hover/definition/completion/cancel/restart with exact native OS receipts; resolve the observed exit-1 restricted permission denial without relaxing containment. Linux genuine delegated LSP repeat, bundle owner/mode/root identity, real live behavior/cancel and crash cleanup.
- [ ] WO6 Windows missing LLDB DLL/package dependency repaired with **real protected AppContainer success**; Linux fixed DAP initialize/launch/configurationDone, target stop/thread/stack, exact kernel cleanup and canceled run; protect executable/adapter pinned hashes and optional runtime.
- [ ] Independent cross-host crash, SCM service crash, durable journal replay/quarantine and child cleanup; install-app acceptance remains P04, not inferred from host tests.
- [ ] Resolve fresh P03 exact SHA native CI failures. Current source changes are **not yet accepted or signed**.

**Exit rule:** P03 remains **IN PROGRESS** until independent exact-commit Win/Linux positive and hostile negative OS results pass. Do not start P04, close P02 as accepted without separate evidence, retire old security PRs, ship unverified LSP/DAP or reopen deferred WO10.

## Additional P03 source closure work

- Connected the new DAP `ConfigurationRequested` event to **both** the dedicated native test fixture and the real Tauri `debug_backend.rs` worker loop. The installed application will not hang solely because the adapter emits `initialized` after its fixed `launch` request. Native tests and installed verification remain pending.
- Hardened the test-only Windows copied adapter probe to use two independently bounded (65,537-byte maximum) diagnostic pipe buffers; replaced unrestricted `communicate()` collection. It reports only fixed exit/handshake categories.
- Tightened Linux `ide-e2-native-lsp.yml` trusted bundle provisioning so the **root and bin directory** are explicitly installed as owner 0/group 0, mode 0755, and independently checked before native systemd execution. The worker's existing immutable-root, cgroup, syscall and Bwrap gates are unchanged.
- In the available Debian 13 container a fixed `clangd --log=error --background-index=false` responded to a direct LSP initialize request (`DEBIAN_CLANGD_DIRECT_PROTOCOL=INITIALIZE_RESPONSE_OK`). The same container's cgroup-v2 mount is read-only and Bubblewrap is not installed; **this demonstrates the protocol only, not native Linux containment**. Swift-provided LLDB-DAP is also missing `libpython3.11.so.1.0` there and is not the reviewed native CI bundle.
- Current P03 draft is [PR #80](https://github.com/vjmichael/Lu-AI-GUI/pull/80). The latest exact-head native CI was queued when checked, so P03 remains open. Do not infer success from earlier PR heads or from the unconstrained Debian tool probe.

### Late protocol and security regression closure

- The real native test and Tauri debugger now also check a read-only `configuration_needed()` state immediately after their fixed launch. This covers a valid `initialized` event arriving after the initialize response **but before** the launch, which could otherwise be consumed by the initialization poll loop and strand the debugger. The state is true only after the already-approved fixed launch and before one configurationDone. Added host regression. No new DAP executable authority.
- Windows test-only initialize preflight now rejects *all* unsolicited event/reverse-request frames ahead of the expected initialize response. Its output buffers are capped while the child runs. A preflight pass cannot satisfy signed/installed or AppContainer native acceptance.
