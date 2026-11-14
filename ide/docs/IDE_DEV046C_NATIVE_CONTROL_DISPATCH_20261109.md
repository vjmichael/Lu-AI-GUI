# DEV-04.6C — protected debugger command dispatch and independent result verification

**Status: implementation committed on the non-PR D6 work branch; Windows/Linux native compilation and installed acceptance NOT YET RUN.** This is a controlled disposable **CI-only implementation**, not permission for a production user or model to debug arbitrary binaries.

## User-authorized task group and actual implementation

This subgroup extends existing DEV-04.6A review fingerprints and DEV-04.6B one-use approval/action journals. Every proposed operation originates from a D5-verified native LLDB owner event, independently inspected Windows Job/SCM debuggee and thread/RIP, exact compiler-generated PE/RSDS/PDB source-line verification and preserved, hash-bound build/debugger/owner/source receipts.

The host DAP protocol and supervisor have a new **explicitly opt-in** \`dev046c-ci-control\` feature. It is not compiled into ordinary Tauri/NSIS or release builds. The host cannot form arbitrary debugger commands. It can form only:

- \`next\`, \`stepIn\`, or \`stepOut\` for the **one exact stopped native OS thread**, using bounded DAP thread ID and \`singleThread=true\`. The protected native acceptance currently runs one \`next\`/step-over against a fresh, compiler-built, sealed, straight-line Rust test program.
- A single \`setBreakpoints\` request targeting exactly the **already reviewed compiler source path and line**, with no condition, hit condition, log message, evaluation, shell, memory-write API, arbitrary process attach or sourceReference. One adapter-reported \`verified\` breakpoint is **only an acknowledgement**, not independent proof that an INT3 was planted or ever hit.

A new \`CiControlSlot\` can be constructed only from a privately consumed D6B action, immutable native identity and the original protected D5 owner-event witness. The slot cannot be cloned and is **taken exactly once before sending**. The D6B per-action \`Consumed\` hash-chain receipt has already been fsynced. Any subsequent send failure, timeout, changed OS event, cancellation, worker crash or restart leaves the grant unusable. The action journal can be replay-read for audit, not replay-executed.

**No installed native human-consent bridge exists yet.** The CI test deliberately uses \`ci_only_not_installed_consent\`, and the dispatcher is strictly compile-time hidden behind the disposable D5 owner + D6C feature and debug assertions. This is engineering acceptance of a sealed test program, never approval to operate on an actual user's IDE project without a fresh UI consent.

## Distinguishing DAP acknowledgement from independently verified progress

The host DAP session tracks a pending control request and checks its exact correlated response sequence. A \`next\`/step request is not an outcome:
1. The original compiled source candidate is revoked immediately when a step is sent.
2. The adapter must report an actual **new \`stopped\` event with \`reason=step\` on the exact same stopped thread**. An ACK alone, a stop for another thread, an unrelated breakpoint stop, a malformed response or an adapter reverse request is denial/quarantine.
3. The protected gateway obtains a new bounded \`threads\` and correlated \`stackTrace\`, with an actual new instruction pointer and source coordinates.
4. The *original* D5 Windows native verifier must **independently** requery the Job-owned OS process/thread, real PE load base, compiler PDB/source line and immutable source snapshot, then capture a **new first-party LLDB owner-event receipt** for that same OS thread.
5. D6C checks that the new event sequence equals the original sequence +1, the owner receipt is distinct, the independently observed instruction pointer **advanced**, the Job/plan/original Completed build and Running debugger/source receipts have not changed, the PID/TID match and the source line remains independently mapped.
6. Only the typed, non-forgeable \`VerifiedCiStepResult\` from these checks can write a \`ResultVerified\` event. It contains a digest of old/new OS-owner/PDB evidence, consumed receipt SHA and action ID. There is no \`success: bool\` shortcut and no result promotion from LLDB's response text.

The breakpoint command has no independent installed-memory or breakpoint-hit witness yet; even a valid adapter acknowledgement is committed as **ResultUncertain**. Variable inspection is still denied because an independent DAP frame/scopes/variable identity proof does not exist. No debugger console, evaluate, memory write, arbitrary attach, terminal or extension command was enabled.

## Native Windows acceptance and recovery

The existing \`ide/supervisor/examples/wo6_native_dap_acceptance.rs\` now adds two opt-in modes, \`--project-debug-step\` and \`--project-debug-breakpoint\`, sharing the *same* protected SCM/AppContainer Job, pinned real LLVM DebuggerThread hook, reviewed offline Rust Cargo build, sealed PE/PDB, independent source checker, original D5 event receipt and durable journal as earlier WO6 gates.

- For the step mode, a **separate fresh CI-only operator challenge** is consumed before transmission. The exact next source line must be independently attested. Native release must preserve the original build/debugger receipts plus the old/new owner receipts, fsynced \`ResultVerified\` action receipt and terminal Completed journal, and deny grant replay after restarting the supervisor.
- For the breakpoint mode, a *different* one-use action is consumed. The adapter ACK and unchanged original kernel stop are checked; the durable terminal remains \`ResultUncertain\` rather than inventing installed breakpoint proof. Native close/restart must preserve its audit.
- The pre-existing normal, cancel and forced-worker-crash modes remain. On any failed consumed action, a best-effort Drop writes \`ResultUncertain\`; if the native worker is killed before Drop, the already fsynced \`Consumed\` phase remains audit-only and cannot be replayed.

**No native Windows execution has passed for this exact D6C head.** The single existing \`ide/llvm18_owner/Test-PinnedOwnerWindows.ps1\` grouped script now includes the host DAP tests, D6 action-journal hostile tests, D6 independent-stop proof tests, and real protected step/breakpoint runs. No new workflow/PR or small CI run was created. The owner-approved policy is to submit **one consolidated Windows/Linux task-group batch** when the D6 control group is complete.

## Native risks and acceptance gates still open

1. The older D5 first-party owner event-loop/source pin and actual Windows \`lldb-dap.exe\` compilation must pass. D5's correct Windows \`WaitForDebugEvent\`/LLDB stop-vs-\`ContinueDebugEvent\` sequencing needs real native evidence; do not infer an outstanding kernel event from a DAP Stopped message.
2. All D6A/B/C Rust Windows compilation, rustfmt, hostile unit tests and exact-head OS Job/SCM acceptance need actual completed job logs and repair of real failures.
3. In real installed Lu, the **human** must approve the particular breakpoint/step/scopes action through a native trusted UI with a fresh one-use nonce. CI-simulated consent is not that approval.
4. Breakpoint installation requires independent memory or breakpoint-hit proof before \`ResultVerified\` may ever be written; variables need an independently attested frame/scopes/variable identity, bounded redaction and their own new approval.
5. Native cancellation, forced LLDB crash, broken owner-pipe, changed source snapshot, stale sequence, timeout, journal fsync failure and restart must be verified under the actual OS. A DAP reply alone closes NONE of these gates.
6. Signed NSIS installed-app human-debugger acceptance remains separate and **OPEN**.

**Release decision: NOT ACCEPTED.** D6C extends a disposable, sealed, first-party native acceptance path without weakening Lu's default authorization model.
