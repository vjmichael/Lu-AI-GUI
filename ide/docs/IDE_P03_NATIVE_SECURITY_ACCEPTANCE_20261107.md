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

## P03 continuation — real OS results and targeted repairs

### Native results reviewed, November 7 (earlier P03 SHA `2799b5de`; NOT latest-head PASS)

- [P03 exact-head source/OS matrix 42358925392](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42358925392): Windows/Linux protected supervisor tests reached **54/55 and 36/37 passed**, respectively, then failed one stale mock: the DAP supervisor test injected `initialized` BEFORE `initialize` response even though real fixed client correctly rejects that order. Updated the first-party mock to use response-first semantics. Separate rustfmt hunks repaired; complete latest-head matrix still outstanding.
- [Real native delegated Linux clangd 42358925376](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42358925376): root-owned bundle audit now **passes**, as do live systemd kernel delegation/cgroup PID placement, protected Bwrap stdio, Running receipt and actual **clangd initialize**. It then loses the read pipe during hover/diagnostics/next editor operations. Native session and cancellation still **NOT PASSED**. A debug-build-only bounded stderr and kernel OOM/PID-limit witness was wired for the failing LSP session; no raw project/error output is logged.
- [Real native protected LLDB-DAP 42358925401](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42358925401): Linux Bwrap + cgroup + initialize response **PASSED**; fixed launch then received an adapter-error response and the session was quarantined. Windows independent copied adapter preflight failed **0xC0000135 / DLL dependency missing**, proving the error occurs even outside the AppContainer: stage/import closure must be fixed before native Windows acceptance.
- Added safe fixed-vocabulary DAP launch-error classification, plus Windows test-only direct PE import audit via LLVM's `llvm-readobj` to identify missing dependencies without changing process rights, installing arbitrary software or claiming that direct-import closure proves native launch.
- **Potential Linux LLDB launch fix:** LLVM upstream documents `lldb-dap` default `disableASLR=true` and confirms `personality set failed: Operation not permitted` in sandboxed runtimes. The first-party fixed debuggee launch now explicitly sets `disableASLR=false`, **keeping ASLR ON**, and has a protocol regression. This is a grounded repair hypothesis; native Linux launch/stop/stack/restart must still pass at a subsequent exact SHA. Upstream: [LLVM LLDB-DAP test patch](https://lists.llvm.org/pipermail/all-commits/Week-of-Mon-20241021/185878.html) and [current launch defaults](https://llvm.googlesource.com/llvm-project/lldb/+/refs/heads/main/tools/lldb-dap/README.md).
- **Independent WO4 actual kernel test:** compiled-in-only `linux_delegated_ci_wo4_host` and new P03 CI job run the first-party live cgroup witness via exact `lu-ide-delegated` systemd service host leaf, demanding enabled controllers, real child AND grandchild PID membership, quotas, `cgroup.kill` and kernel `populated 0`. An OS/runner rejection is a fail, never a simulated pass.
- **Independent hostile Bwrap test:** first-party `wo4_bwrap_hostile_native.py` and new P03 CI job require different actual PID/network namespace identities, inaccessible host files and a readable but **not writable** approved workspace, while retaining the system AppArmor unprivileged namespace restriction. This checks only the fixed OS sandbox shape, not release packaging or installed-app acceptance.

### Exact next outcomes required

1. Inspect newest **exact P03 HEAD** jobs after last source commit; format/compile/protocol check cannot be promoted until passed. Do not inherit prior-run green jobs.
2. For Windows: identify and package only reviewed fixed LLVM/Python/CRT dependencies with provenance into the measured DAP bundle, then prove actual AppContainer/Job/SCM debugger session and child-tree kill; independently repair clangd exit 1 permission denial.
3. For Linux: use fixed failure classifiers to repair protected clangd post-initialize read-pipe failure and DAP fixed-target launch; verify DAP `initialized` -> `configurationDone` -> stop/thread/stack and actual cancellation/restart. Require native WO4 cgroup child/grandchild and hostile Bwrap positive jobs.
4. Independently verify host/service crash quarantine/recovery and no replay of stale grants or debuggee process after restart, with durable exact-source receipts.

**P03 state remains: SOURCE REPAIRS ACTIVE / REAL NATIVE OS ACCEPTANCE OPEN.** No P04, no WO10, no broad filesystem/ACL or process-rights relaxations.

## November 7 — real native CI conclusions on P03 SHA `1884c531`

- **Linux protected clangd — genuine acceptance PASS:** [E2 native run 42391111181](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42391111181) executed real systemd delegated cgroup/Bwrap clangd initialize, hover, definition, changed-source diagnostics, completion, normal/cancel cleanup and durable reopened receipts. The recipe-specific 16-task cap and single indexing thread fixed the four-task cgroup PID limit; other LSP recipes remain limited to four. Source rustfmt at this SHA remained failing independently.
- **Linux debugger host crash/replay — genuine acceptance PASS:** [WO6 OS run 42391111210](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42391111210) deliberately crashed the DAP test host after restricted initialize, then independently proved real worker cgroup `populated 0` and supervisor restart quarantine. **Native fixed debuggee launch still FAILS** with LLDB-DAP adapter error; kernel PID maximum and memory OOM counters both reported `no`. No stop, thread, stack or normal/cancel acceptance claim.
- **Windows debugger loader diagnosed:** copied LLVM runtime imported `python310.dll` transitively from its actual seven staged PE binaries, but file was absent; unprotected adapter failed `0xC0000135`. Pinned test-only Python 3.10.11 runtime copy and SHA witness added to the measured bundle for a new CI run. This is not a reviewed production Python/LLVM license package, installer or signing approval.
- **Windows clangd — DENIED:** [E2 native run 42391111181](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42391111181) proved authentic SCM, AppContainer/Job admission, exact NUL lease, fsynced Running, but clangd immediately exited 1 with fixed category `permission_denied`; source of permission denial still unverified. No blanket AppContainer ACL/capability grant or fallback.
- **Linux WO4 real cgroup and hostile Bwrap** were positive on [P03 run 42387150080](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42387150080) and repeat native cgroup on [42391111159](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42391111159). **Windows D7D** real protected service crash and recovery, protected Cargo normal/cancel were positive on [42387149941](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42387149941). Final exact-head CI remains mandatory.

**P03 remains OPEN**: Windows clangd, Windows LLDB-DAP dependency-repaired AppContainer runtime, Linux native LLDB-DAP launch/stop/thread/stack/cancel, exact-head source formatting and OS recovery. P04 not started.

### Fixed LLDB-DAP headless launch refinement (latest P03 source, native verification required)

The LLVM 18 LLDB-DAP launch handler explicitly accepts `disableSTDIO` and otherwise defaults to debuggee standard-I/O/pseudo-terminal setup. This first-party, stop/thread/stack-only debuggee requires no terminal I/O. The fixed request now sets `disableSTDIO=true` as well as `disableASLR=false`; neither adds capabilities, unpins a target, permits reverse terminal requests, nor relaxes Bwrap/cgroups/AppContainer. This is a bounded repair hypothesis for the protected Linux launch failure, **not yet real native acceptance**. LLVM source: https://llvm.googlesource.com/llvm-project/+/refs/tags/llvmorg-18.1.2/lldb/tools/lldb-dap/lldb-dap.cpp .

### E2 native Windows/Linux test scheduler repair (November 7 follow-up)

The P03 head `a96560dca4c942bf5fc2a91830566ea5214d12af` lacked any E2 native clangd CI run because `.github/workflows/ide-e2-native-lsp.yml` contained a corrupted concatenated PowerShell line in the Windows clangd PE import check, followed by a duplicated non-YAML fragment after the Linux delegated systemd job. The completed prior E2 native Linux success and protected Windows failure therefore could not be revalidated at that head. The exact source workflow has now been structurally repaired and its Windows/Linux native tests made schedulable again, **not yet proven to pass**. A green source-only matrix does not satisfy this missing E2 native evidence.

### Exact native Rust format repair (November 7)

Older branch-head Windows/Linux native probe and P02 runs failed solely at `cargo fmt --check` due two exact formatter diffs in `ide/sandbox/src/bin/linux_delegated_ci_wo4_host.rs` and `linux_delegated_ci_wo6_dap_host.rs`; Linux/Windows native kernel tests themselves had separately passed. Both expressions are now identical to the Rust 1.99 CI formatter output, without changing security behavior. **Actual final exact-head formatter result remains pending**, distinct from real debugger/clangd acceptance.


### IDE 10 continuation — P03 native Windows script corruption, exact-head revalidation

[Previous exact-head protected E2 run 42403027786](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42403027786) had an actual Windows PowerShell **ParserError** in the self-measured clangd import-audit inline `run:` block. It was truncated inside `if ($dll -notmatch ...` before an AppContainer probe ran. The protected Linux clangd job from that same run passed, but the Windows failure provides **no native permission verdict** at that SHA.

On continuation branch `ide/p03-p04-native-closeout-20261107`, restored syntactically complete direct PE import parsing and closure of the foreach/if block. Same-distribution LLVM DLL staging is unchanged. The parser reports only the count of direct imports absent from the pinned private directory; those may be Windows OS-supplied DLLs and this is **not** full transitive or dynamic-dependency proof. Added `ide/supervisor/fixtures/Test-P03ClangdCI.ps1` to parse the exact native `pwsh` step using the native PowerShell AST before expensive compilation; wired into both E2 Windows and P03 exact-head Windows source jobs. No startup capability, broad PATH, ACL, AppContainer token, service permission, or signing-policy change.

**Open:** exact continuation-head E2 Windows parser/native clangd result; historical restricted clangd exit-1 permission failure, fixed Windows LLDB-DAP AppContainer normal/cancel, Linux LLDB-DAP launch/stack/stop, and final actual OS kernel/host restart witnesses. Do not infer any of these passes from source syntax repair.


### IDE continuation: bounded LLDB response-shape classification

Previous genuine Linux cgroup/Bwrap LLDB native debuggee launch reached a correlated adapter `success:false` response but only `other_adapter_error` was reported, with no `body.error.format`. The debug-only fixed-vocabulary classifier now examines bounded alternative JSON response slots (`body.error.description`, `body.error.message`, plain `body.error`, `body.message`) before the generic response message, never printing adapter-supplied strings or project paths. Host regressions cover permission, ptrace, missing target and launch classes. A future **exact-head native Linux DAP launch/stop/stack/cancel result** is required; this is diagnostic development, not proof or a relaxed cgroup/Bwrap/ptrace grant.
