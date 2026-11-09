# DEV-04.5D4B — Windows stopped-thread RIP and independent PE/PDB line

Status: DRAFT. Source and test fixtures committed. Exact-head native and installed acceptance remain open. Stacked on D4 PR #184, D3 #181, D2 #180.

## Work completed

The existing protected Lu Windows debug worker retains its SCM lease, Job Object handle, native DAP gateway, Cargo target identity, and independent one-use job receipts. D4B adds a query-only GetThreadContext sampler in the same worker. It enumerates only the real Job-owned target process's threads, reads AMD64 RIP with THREAD_GET_CONTEXT access, requires one match to the correlated DAP address, and rechecks the thread list, RIP and Job members. It cannot suspend a thread, attach a second debugger, change its registers or grant any source-level debugger operation.

**Important limitation:** Windows documentation says GetThreadContext can report an invalid context for a running thread. Therefore a match to LLDB's reported PC is evidence of agreement, not independently verified OS debug-event custody. The sampler explicitly returns kernel_debug_event_independently_verified=false, pdb_line_independently_verified=false, and debugger_control_authorized=false.

A separate disposable Windows-only native observer is compiled in CI. It starts an exact first-party PE fixture using DEBUG_ONLY_THIS_PROCESS, receives a genuine EXCEPTION_BREAKPOINT through WaitForDebugEvent, rejects the initial loader breakpoint outside the target PE, and reads the authentic stopped event thread's RIP with GetThreadContext. The observer is not attached to Lu's actual project worker.

The corresponding Python CI test compiles a real clang-cl /Zi and lld-link /DEBUG executable and PDB, compares image load base and executable size against the OS event, and asks separately measured llvm-symbolizer to map the event RIP to the original source line. The underlying D2 verifier rehashes PE/PDB/source, checks PE CodeView RSDS against the MSF7 PDB GUID/age, and rejects changes in source or symbol files. Explicit negative tests cover a wrong line, a wrong instruction pointer, a repinned altered PDB age and changed source.

A new exact-head workflow covers native Windows debug-event and PDB verification, Windows Rust Job/thread compilation plus Rustfmt, and Linux native regression. The existing full protected Windows SCM/AppContainer Cargo-to-LLDB acceptance fixture gains the fixed --project-debug-thread mode; it is not a general-purpose native launch API.

## Evidence types and acceptance

The independent Windows DEBUG_EVENT fixture can prove the OS kernel event and compiler source-line semantics for its **own test executable**. It cannot validate the original human-approved Lu job, the actual LLDB-owned debugger event, the live Job's loaded PDB, restart recovery, or an installed release.

The protected Lu Job sampler checks the real worker and durable journal, but cannot itself prove the OS debug event or PDB line. Full DEV-04.5D4B acceptance requires a reviewed, noninterfering handoff of the genuine LLDB-owned stop/event and thread context, tied to the exact SCM Job, sealed Cargo PE+PDB, original immutable source, freshly consumed authorization, and terminal journal. It must not create an arbitrary DebugActiveProcess fallback or loosen AppContainer access.

Do not enable setBreakpoints, stepIn, next, stepOut, evaluate, runInTerminal, variable/scope inspection, or a signed installed debugger release from these partial witnesses. All remain disabled. Windows/ Linux installed GTK and upstream D2/D3/D4 exact-head failures or queued jobs are separate open gates.

Next step: inspect actual Windows debug-event fixture + Rust compiler + SCM Job logs on the exact PR head, repair failures, and connect a kernel-backed stopped-thread witness from the real protected LLDB session to independent CodeView/PDB line decoding.
