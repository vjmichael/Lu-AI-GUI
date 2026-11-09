# DEV-04.5D4D — Independent Windows debugger ownership: OS debug port, LLDB ancestry and DEBUG_EVENT gate

**Status: implemented OS-corroboration source; native Windows test results QUEUED; actual LLDB DEBUG_EVENT owner attestation NOT YET ESTABLISHED.** Draft work stacked on D4C PR #189.

## Current verified source-level architecture

Windows D4C already requires:
- one independently verified, protected Cargo build, its sealed PE/RSDS/PDB, original source SHA and durable Completed receipt;
- one separately authorized LLDB-DAP process in the native restricted SCM/AppContainer Job and a distinct Authorized→Running receipt;
- a correlated real LLDB stopped/threads/stackTrace; the Job member's actual PE module base and a unique Windows thread RIP matching the LLDB stop; repeat checks and DAP invalidation;
- independent LLVM/CodeView source-line comparison, original full compiled source-path agreement and immutable source snapshot;
- a distinct Completed or CancelRequested→Cancelled journal, verified full Job descendant termination, reopen and replay denial.

D4D adds **OS-independent corroboration, without new execution rights**:
1. The native host supplies the actual *HANDLE* of its already admitted, SCM-supervised LLDB process and the actual preexisting Job handle. No DAP message, model response or browser may supply a PID/handle.
2. The kernel Job process list must contain BOTH this LLDB process and the exact sealed Cargo debuggee.
3. Win32 GetProcessId on that LLDB process handle provides the debugger PID. A separate, bounded ToolHelp **process snapshot** must give that debuggee's native parent PID as the LLDB PID, not a renderer/adapter supplied parent.
4. CheckRemoteDebuggerPresent on a QUERY-only debuggee process handle must report a debugger attached, and the LLDB/debuggee must still be alive.
5. Check the Job membership, debug presence and parent **a second time** after independent PDB lookup. The witness never grants permission and retains \`actual_debug_event_owner_attested=false\` and \`debugger_control_authorized=false\`.

This is stronger than an LLDB claim alone: both ancestry and debug-port state are independently observed from Windows while a one-use job remains active. It still does **not** identify the owner of the kernel \`DEBUG_EVENT\`, because an attached debug port and a parent relationship do not reveal the event object received by an LLDB debugger thread.

## Actual remaining requirement — owner-side receipt from the *reviewed* LLDB debugger

Microsoft documents that only the thread that created the debuggee can call WaitForDebugEvent, and the kernel suspends all debuggee threads while an event is outstanding, until ContinueDebugEvent. **A second debugger process cannot safely race LLDB to consume this event.** \`GetThreadContext\`, \`CheckRemoteDebuggerPresent\`, a DebugObject handle lookup, a DAP \`stopped\` message or a PDB symbol match cannot independently prove the original owner event.

To close the final gate, a reviewed, checksum-pinned Windows LLDB build must emit a bounded, host-authenticated, one-use **owner-side event witness** *from its genuine DebuggerThread::DebugLoop*:

- When its kernel WaitForDebugEventEx returns an actual EXCEPTION_DEBUG_EVENT, capture the real kernel event's PID/TID, exception code, exception address, first-chance bit, and monotonic event sequence. Capture the event thread's true CONTEXT_CONTROL RIP **while that event remains owned by the debugger**, before its first ContinueDebugEvent or any guest continuation. Record the LLDB process identity bound to the exact SCM Job and the debuggee PE loaded base.
- Emit a small versioned record through a **fourth, separately allowed, write-only inherited event pipe** negotiated before CreateProcess, not through ordinary untrusted DAP JSON/stdout or user-configurable filesystem paths. The host's read end must never be inherited by the AppContainer child. The Job handle and existing exact-handle-list policy remain authoritative; no blanket inheritance, shell, arbitrary attach or extra capability. The original three-handle release path remains unchanged.
- Bind this event record to a fresh host-generated one-use nonce, job ID, debugger plan SHA, reviewed LLDB bundle SHA and a monotonically advancing owner-sequence. The host independently checks that the producing process is *the same* protected LLDB handle, that its Job still owns the debuggee, and that the event's TID/RIP/PID match the native OS/PE/PDB/source witness and original Running receipt. A dedicated service-authenticated MAC/receipt or equivalent protected pipe admission and independent durable journal is mandatory before calling it trusted. A message supplied by an adapter, the model or a random process cannot become an owner receipt.
- Emit a separate **revocation/continued** record on the exact owning thread's ContinueDebugEvent and invalidate the stop on any new event, resume, target exit, child worker loss, missing receipt, sequence gap, stale nonce or restart. The host must not accept a stop whose native event is already continued.
- The resulting *true owner* receipt must then be joined to the completed source-line and journal acceptance in the same real protected Windows run, for both normal and cancelled sessions. Only when real exact-head Windows CI and installed acceptance pass may the host consider **separately authorized** source breakpoints, stepping or variable inspection. A verified owner does not itself grant those operations.

This exact hook is not implemented into the existing externally supplied LLDB binary, and no pinned rebuilt LLDB result exists on the exact candidate head. **Do not manufacture \`lldb_debug_event_owner_attested=true\` from existing facts.** Do not use source-only or standalone fixture results as release acceptance. Windows/ Linux OS jobs and previous GTK failures remain separately open.

## Native results and independent source testing

On current D4C PR #189 exact head \`83b02c4948b0626a1830209a30f9639568fa41da\`:
- GitHub \`42584964922\` (real WO6 protected LLDB): queued; all 3 OS jobs queued, including native Windows.
- GitHub \`42584965121\` (D4C compile/unit/policy): queued; both jobs queued, including native Windows.
- There are **no actual Windows compilation, runtime logs, exit codes or completed artifacts for the newest exact head** as of this inspection. No genuine compiler/runtime failure can be diagnosed or repaired from logs that do not exist.

D4D source contracts and source unit tests add debug-port/parent/Job membership review without changing the default tool registry or policy engine. Exact-head native test must subsequently report success or a concrete denial; if parent or CheckRemoteDebuggerPresent are denied by AppContainer, **record native denial, do not widen privileges**. An OS debugger-present flag is partial evidence, *not the final DEBUG_EVENT gate*. Existing static LLDB/Cargo/PDB fixture evidence remains partial.

### Disallowed shortcuts

Never substitute a second DebugActiveProcess attachment; SuspendThread to manufacture a valid RIP; generic privileges, unsigned binary injection, arbitrary DLL hooks, user-provided PID or path; acceptance from DAP JSON alone; reuse of original human approval; synthetic PASS while native jobs are queued; or an unconditional true owner flag.

**Final status: strict DEBUG_EVENT owner attestation still OPEN until a reviewed, built and independently tested LLDB owner-side event hook is present.**
