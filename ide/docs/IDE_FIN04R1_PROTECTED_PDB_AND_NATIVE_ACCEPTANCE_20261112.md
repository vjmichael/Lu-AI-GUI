# FIN-04R-1 — protected production PE/PDB reader and native acceptance

**2026-11-12 | Existing FIN branch | source integration, NOT native/installed acceptance**

## Source implemented

- The original DEV-04.5D4C `run_measured_reader` which starts `llvm-symbolizer.exe` using host `std::process::Command` now compiles **only** for the CI/debug oracle feature. A common strict `independently_verify_pdb_line_with_reader` accepts an independently owned native reader, still checks exact PE/PDB GUID/age/DBI and SHA, current loaded image base, OS-owned thread RIP, compiler source path, DAP candidate line and SHA of original immutable `src/main.rs`.
- The release `WindowsLspProcess::read_protected_pe_pdb` uses the already-running debugger's **same kernel Job Object and AppContainer SID** with the existing restricted SCM lease. It admits only the exact fixed binary `language-server/bin/llvm-symbolizer.exe`, remeasures the staged release bundle/target PE/PDB and inherits exactly stdin/stdout/NUL stderr (no owner-event pipe). The helper cannot use the shell, networking, a caller path or a model command. Its output is bounded to 8192 bytes; it must terminally exit 0, produce exactly one JSON mapping accepted by the shared validator, and leave all original stage hashes unchanged. The LLDB actor's OS stop and owner's original event must still exist after the helper exits. Any uncertain containment or output fails closed and is quarantined on debugger session unwind.
- `WindowsDapSession::observe_release_owned_stop` now joins that independent PDB line to its live first-party `DEBUG_EVENT`, target Job, independently sampled thread RIP, protected loader module, completed build and running-debugger receipts, preserved source snapshot and original PDB digest. It rechecks the original event, source bytes, and journals after reading. The returned session-local witness has `source_line_independently_verified=true` **only on these checks**; `debugger_control_authorized=false`, `recovery_authorized=false`, `installed_app_accepted=false`.
- `close_with_protected_release_owner_continue` requires the original protected LLVM debugger thread to signal a real `ContinueDebugEvent` for that exact stopped sequence before Job terminal, rather than treating a DAP `disconnect` ACK as OS event revocation.
- A fresh one-use native Windows modal can review the exact originally mapped **step-over** or **verified-line breakpoint** intent. The original action journal writes `Prepared → Approved → Revoked` (or `Denied/Revoked` on refusal, expiry or cancellation). It never creates a `Consumed` executable command, calls DAP step/setBreakpoints or restores authority from a receipt. A UI can request a **review only** using a fixed operation kind and independently sealed build; a model or renderer cannot provide PID, RIP, path, authority, reviewed line, process handle or consent bit.
- A bounded native-source verifier `verify_independent_step_result` continues to demand a new original kernel owner stop and independently checked PDB. New `verify_independent_breakpoint_hit` logic distinguishes a genuine new PDB-mapped OS breakpoint *hit* from an adapter `verified=true` acknowledgement; neither code path is a released dispatch right. The release `CiControlSlot::admit` hard-rejects non-test use.
- Original OS-only interrupted-action recovery still requires a separate native human dialog, exact original journals, kernel Job/cgroup double quiescence, fsynced uncertainty receipt and **no action replay or global marker clearance**.

## Requested requirement status

| Requirement | Source | Actual native/installed result |
| --- | --- | --- |
| Replace host symbolizer with independently contained reader | Protected AppContainer / original Job / SCM fixed-purpose reader implemented | **Not tested on Windows** |
| Fresh owner-event + independent PDB + one-use human authorization | Native approved review identity, challenge, journal and immediate revocation implemented | **Human review not physically accepted; no production command grant** |
| Independently verified post-command step and breakpoint | Native evidence/verifier + negative fixtures; breakpoint acknowledges only a verified *hit*, not installation | **Positive Windows post-command dispatch not performed; production controls hard-disabled** |
| OS-only interrupted action recovery | Native double kernel quiescence and no-replay original-journal receipt path implemented | **Windows kernel/restart negative and positive fixtures not performed** |
| Installed app physical-human/security acceptance | Exact-head Windows native fixture plus separate operator-only physical consent/restart script authored | **Unrun; not accepted** |

## Runnable native/physical acceptance assets (do not infer results)

- `ide/supervisor/examples/wo6_native_dap_acceptance.rs --project-debug-protected-pdb`: starts real pinned protected Cargo build and LLDB project worker, requires actual first-party kernel owner, contained PDB result, owner-event continuation, original SCM Job terminal and independently reopened owner receipt. It is **not** a positive step/breakpoint production test.
- `ide/llvm18_owner/Test-FIN04R1-ProtectedPdbWindows.ps1`: exact-source native Windows test/compile and real OS acceptance runner. Requires original installed protected recovery service and exact pinned first-party bundle; never run unattended without those inputs.
- `ide/app/fixtures/Test-FIN04R1-PhysicalProtectedPdbReview.ps1`: interactive-only installed desktop source/binary pin check, witnessed real human approve/refuse/cancel/timeout and restart no-replay receipt recording. Explicitly refuses CI and never fabricates user acceptance.
- `ide/app/tests/fin04r1_protected_pdb.test.mjs` and Rust `debug_control_dispatch` unit fixtures: structural and synthetic negative contracts, not Win32 live evidence.

## Current release blockers

1. Compile all changed Rust/Tauri source on the exact pinned Windows toolchain. Audit any compiler diagnostics and repair before promotion.
2. Capture original positive/negative **real Windows** AppContainer + SCM Job results for protected symbolizer, independent PDB line, owner-event race/continuation, worker death, source/PDB tamper and restart recovery, with durable SHA-linked evidence.
3. Make production step and breakpoint dispatch *only after* the requested independent protected OS/human gates have **physically passed**. For step: new owner-event sequence and independently mapped post-step PE/PDB line. For breakpoint: independently observed OS hit or process-memory instruction proof; adapter response is insufficient. Also prove cancellation and revocation at dispatch boundary.
4. Run actual interactive installed NSIS owner-operated consent, accessibility and restart/no-replay tests. Confirm exact first-party patch/runtime provisioning and NUL SCM lease. No CI-only boolean or synthetic input can substitute.
5. Obtain independent exact-head acceptance, then separately consider whether the owner authorizes promotion. FIN-02 remains deferred; no default feature, production debugger command, merge, installer acceptance, signed release or task completion is asserted.

**Safety rule:** In the meantime, `operationControlSupported=false` in production, and the protected release-owner feature permits *only* source observation and immediately revoked one-use action review. Unknown effects remain unknown.

## FIN-04R-1 native failure-path acceptance extension (staged independently)

A separate exact-head staging branch adds
`--project-debug-protected-pdb-cancel` as a fixed, protected, **no DAP
mutation** scenario. It first requires the same first-party kernel stop and
separately AppContainer/SCM Job-contained PDB source mapping as the positive
scenario, then drops the original native gateway, forcing the original
`CancelRequested → Cancelled` receipt and complete Job tree reaping. A
newly reopened Supervisor must find both terminal Job records and the
original immutable native owner receipt, with no old authorization replay.
`Test-FIN04R1-ProtectedPdbWindows.ps1` now runs both normal and
interrupted scenarios before a positive native receipt is emitted.

**No actual kernel test run has executed this staged extension yet.** It
does not prove unexpected LLDB process death, intentional process-tree
escape, physical desktop restart, or installed operator approval. Those
remain separate acceptance gates. The main acceptance branch continues
building its immutable LLVM head; do not invalidate its long-running
original job by merging this test during compilation.
