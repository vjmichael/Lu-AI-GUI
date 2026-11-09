# DEV-04.5D4C — Real Lu LLDB-owned Windows stop → Job RIP → PDB line → durable receipt

**Status: DRAFT. Source and exact-head OS acceptance test committed; no native Windows successful result yet.** Stacked on D4B PR #186, D4 PR #184, D3 PR #181. No merge or release permission.

## Existing chain reviewed

1. A real protected Cargo `rust_build` under the native Windows restricted Job finishes, independently verifies an actual PE32+ executable and matching CodeView/RSDS PDB, and persists a terminal **Completed** journal hash.
2. An independently approved `dap_lldb_project` plan pins that target output and its exact source snapshot, seals a private runnable target, starts **real LLVM LLDB-DAP** inside the original native SCM lease / AppContainer / Job, and fsyncs its distinct **Authorized→Running** receipts. The verifier cannot choose a PID, target or binary based on the AI model or WebView.
3. Native LLDB must actually send `Stopped`, `threads`, and a correlated `stackTrace` including a bounded instruction pointer, source path and line. The native DAP state must remain `Stopped`.
4. Native Windows Job enumeration, process executable query and PE ToolHelp image mapping independently bind the observed address to the exact staged executable and computed relocation. A read-only `GetThreadContext` path selects **one unique** matching thread inside this exact Job-owned process, checks live `GetProcessIdOfThread` on its handle and repeats its RIP/identity. It does not suspend/attach or borrow another debugger.
5. **New D4C native-only step**: A separately compiled, independently hashed `llvm-symbolizer.exe` is copied into the **same plan-SHA-bounded, privately staged** LLDB-DAP test bundle and invoked by a fixed-purpose, *debug CI-only* native method. It maps the actual Job-observed RIP with the OS-loaded PE base to the exact compiler-generated source file and line in the independently attested RSDS/MSF7 PDB. It never asks LLDB to symbolize the same line for itself. The reader has no user-configurable path, no shell, no caller arguments, no inherited environment, bounded output and a 6-second kill/reap deadline.
6. The DAP path and independent PDB source path must match each other, with the **full** expected `src/main.rs` relative identity (not just `main.rs`). The immutable approved native source snapshot supplies the original SHA and actual line count. A private compiler workspace may have been deleted after the build; no arbitrary basename or unrelated replacement source is accepted.
7. On success the original source, sealed target, OS/DAP stopped state, PDB, bundle digest and both journal chains are reverified. The actual real protected `--project-debug-line` CI mode requires the complete chain to report `DEV045D4C_REAL_LU_JOB_RIP_PDB_AND_RUNNING_RECEIPTS=PASS`, then closes the LLDB worker, verifies **a distinct fsynced terminal receipt** preserving the earlier Running receipt and original build Completed receipt, removes the target stage, restarts the supervisor and proves both consumed human approvals cannot be replayed.

## Essential distinction / remaining limit

This is **not** a new native debugger permission. It runs solely under `dev045d4c-ci-oracle` plus debug assertions, has no Tauri/WebView IPC action, and the default Windows/release compilation does not include its external reader. All breakpoints, stepping, variable inspection, evaluate/setExpression, arbitrary attach, shell and unsigned product execution remain disabled.

Windows OS guarantees a debug-event callback suspends the debuggee while the **owning debugger** processes it; however, the independent Lu observer only sees a correlated LLDB stopped message and the same Job-owned thread's stable OS registers. It does **not** receive or cryptographically authenticate LLDB's private `DEBUG_EVENT` owner record. The evidence therefore correctly leaves `lldb_debug_event_owner_attested=false`, `debugger_commands_authorized=false`, and `installed_app_accepted=false`. Do not misreport this as completed strict debug-event ownership, official production source-debug approval or signed installed test.

**Further required gate:** an independently reviewed stop-event handoff from the actual debugger owner (without attaching a second debugger), or a comparably kernel-backed proof, joined to the same PID/TID, RIP, module, plan, one-use grant and journal. Then obtain completed exact-head Windows kernel process/LLDB/PDB tests, repair any native compiler, source-location or PDB lookup failures, and establish installed acceptance. The standalone D4B `WaitForDebugEvent` fixture remains separate.

## CI

- `.github/workflows/ide-dev04-5d4c-real-lldb-line.yml`: exact-head Windows source/policy/unit/format, Windows release-default denial and Linux compilation regression. Source checks **do not** close native running/installed acceptance.
- `.github/workflows/ide-wo6-native-os-acceptance.yml`: real Windows service/Job/AppContainer, sealed offline Cargo compiler and first-party LLDB-DAP, **new real protected `--project-debug-line`**, then service null-lease revoke and SCM uninstall. Linux native existing cgroup tests remain separate.
- Test deliberately **fails closed** on mismatched Job process/thread, missing source coordinates, changed binary/PDB, wrong module ASLR, wrong PDB line, malformed symbolizer JSON, missing/altered one-use receipts, changed source or exited/continued DAP.

No exact-head native Windows result or installed Lu application success is claimed by this document.
