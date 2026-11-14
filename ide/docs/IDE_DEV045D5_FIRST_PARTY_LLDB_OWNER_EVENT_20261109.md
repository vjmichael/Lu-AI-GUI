# DEV-04.5D5 — first-party LLDB Windows event owner and protected receipt

**Status (2026-11-09 UTC): SOURCE INTEGRATED, EXACT-HEAD NATIVE WINDOWS ACCEPTANCE OPEN.** Stacked on D4D PR #190. Never infer installed or release acceptance from this design or a queued workflow.

## What was actually implemented

The source is pinned to LLVM 18.1.8 Git commit
\`3b5b5c1ec4a3095ab096dd780e84d7ab81f3d7ff\`. The small first-party
\`ide/llvm18_owner/apply_llvm18_owner_hook.py\` applies to the **actual**
\`lldb/source/Plugins/Process/Windows/Common/DebuggerThread.cpp\`; it
rejects a different source commit, changed compiler anchor or double patch.

\`LuIdeOwnerEvent.h\` emits a 76-byte fixed-layout owner record on the
native **WaitForDebugEvent** return, *before* LLDB handles the exception,
and emits the exact matched revocation after the original
\`ContinueDebugEvent\`. It obtains the native Win32 debugger-owner TID
on that exact event-loop thread, opens the event's real TID with
\`THREAD_GET_CONTEXT\` and records the kernel event's PID/TID, exception
address, context RIP, first-chance flag and monotonic owner sequence.

A normal, unpatched/release LLDB requires no such pipe and remains
unchanged. Under this first-party test build, Lu supplies a fresh 128-bit
nonce and an inherited anonymous owner pipe via a **fourth OS-handle
allowlist entry**, exclusively for the separately approved project-LLDB
recipe and debug CI feature \`dev045d5-ci-event-owner\`. All ordinary
Windows LSP/DAP/release launches retain the original exact three-handle
allowlist. The event pipe's **read** handle remains in native host memory,
and the reviewed LLVM hook marks its **write** handle non-inheritable
before launching the actual debuggee.

The native host uses an exclusive bounded Windows pipe reader and
strict binary parser. It rejects wrong magic/nonce, out-of-order or
duplicate sequences, cross-process or cross-thread continuations,
truncated records, unknown event kind and invalid status. A valid
CONTINUED record immediately revokes the corresponding active stop.
On event-reader startup denial, Lu quarantines the already Running job
and kills the Job process tree; it never falls back to DAP JSON,
uncontained process launch or a debug attach.

The new CI-only protected DAP method requires that the original Job's
kernel-observed LLDB process contains the claimed owner thread, that
the event PID/TID/RIP matches the independently observed *same*
Job-owned debuggee and exactly correlated LLDB stack source line,
and that the original Cargo PE/RSDS/PDB, source snapshot, separate
one-use authorization, running debugger receipt and original Completed
build receipt remain verified. A distinct event receipt is created
with \`create_new\`, fsynced and hashed under the original trusted
supervisor journal's native parent. The host rechecks that the same
event has **not continued**, then tests the durable owner-event file
against its hash after reopening the supervisor.

### Strict native continuation and crash-handle tests

The normal `--project-debug-owner` acceptance now sends only the
existing fixed `disconnect` DAP request and **waits for the matching
`CONTINUED` record from the actual LLDB owner thread** before the
supervisor can mark the OS Job terminal successful. A failed
`ContinueDebugEvent` Win32 return (wire status 2) is explicitly denied
by the native parser and a negative unit test; it is never counted as
successful revocation.

The `--project-debug-owner-crash` acceptance uses the original
native LLDB process HANDLE to terminate that *already approved* worker.
It then explicitly requires **kernel pipe EOF** within three seconds,
using the sideband receiver, so a leaked write handle in the inferior
or a surviving worker cannot masquerade as cleanup. The old active stop
proof must fail, and Drop must record CancelRequested→Cancelled and
terminate all Job descendants. The separate owner-event receipt must
still match after supervisor restart, but never grants resumed
execution or a repeat approval.

These are newly committed **test requirements**, not observed OS passes.
The exact-head Windows builder must actually execute all three modes,
and real logs must be examined before any acceptance closure.

The actual protected Windows acceptance executable now has
\`--project-debug-owner\` and \`--project-debug-owner-cancel\`, in
addition to the older independent PDB modes. They run in the original
SCM/AppContainer Job with a fresh one-use grant and real compiler
binary. Each mode requires event ownership, exact PE/PDB source,
original journal references and the correct terminal Completed or
Cancelled receipt; reopen must deny grant replay.

## Native CI (not yet achieved)

\`.github/workflows/ide-dev04-5d5-owner-event-native.yml\` checks out
the **exact GitHub head**, compiles the bounded Rust native event
reader/worker, builds the *patched LLVM* source with CMake and a Windows
compiler, records native binary/source hashes, stages a limited offline
Cargo toolchain and runs the normal/cancellation SCM/AppContainer owner
tests. It also checks Linux cgroup transport regression. This workflow
must prove the patched LLDB actually compiled; a preinstalled stock
LLDB cannot substitute.

At the last upstream inspection PR #190 \`2e7269dad096ac83729791e85067929cf5f50d00\`
had 26 workflows queued, with **zero actual Windows CI results**.
The older D4C/D4D Windows jobs likewise had no completed exact-head
native compiler/runtime artifacts. A native build error cannot be
diagnosed or called repaired until those logs exist.

### Remaining acceptance gaps / no false promotions

- Confirm actual patched LLDB Windows native compilation and unit tests;
  fix every real compiler/SDK/PDB or OS failure from completed logs.
- Confirm actual WaitForDebugEvent → owner record → DAP Stopped →
  independent PE/PDB match → fsynced receipt → ContinueDebugEvent
  revocation ordering and exact native observer ownership in an installed
  Lu debugger, not only the CI entrypoint.
- The fixed **real protected LLDB crash-injection** test entry point
  \`--project-debug-owner-crash\` now exists. It first obtains the same
  real OS owner/PDB/source receipt, then uses the *original owned Windows
  process HANDLE* to forcibly terminate the already authorized LLDB,
  requires the earlier stop attestation to become unusable, and drops the
  gateway so existing CancelRequested→Cancelled/Job descendant cleanup,
  SCM lease release, journal restart and event-receipt rehash gates apply.
  The new Windows CI script runs it alongside normal and cancellation modes.
  **Still pending native OS execution:** a completed Windows result proving
  crash exit, sideband EOF/partial frame denial and full descendant cleanup.
  Source and unit tests alone cannot close this acceptance.
- Obtain signed installed application acceptance and installed-app
  debugger UI proof separately.

**No breakpoints, stepping, variables, evaluate/setExpression,
runInTerminal, arbitrary attach or reusable execution permission have
been enabled.** An authentic LLDB-owned DEBUG_EVENT remains a receipt
of *state*, never blanket authorization to control the debuggee.
