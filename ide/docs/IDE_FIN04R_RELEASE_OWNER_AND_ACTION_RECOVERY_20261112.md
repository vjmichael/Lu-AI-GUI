# FIN-04R — release observer, native action audit, independent-result gate and OS-only recovery

**2026-11-12 · Existing FIN branch · source implemented, native/installed acceptance OPEN · production debugger actions DISABLED**

## Changes in this source increment

1. **Windows first-party event owner, release-observer configuration.** The pinned LLVM 18.1.8 `DebuggerThread.cpp` hook remains the sole source for a binary, nonce-bound `DEBUG_EVENT` sideband. `fin04-release-owner-evidence` in the native supervisor can compile this sideband in a release build without inheriting developer-only consent or DAP step/breakpoint permissions. `fin04-release-event-observer` is an independent desktop opt-in that requires the whole FIN-02 protected runtime profile; it is NOT in the default IDE. A project-only LLDB launch receives the fourth allowlisted inherited pipe end. Ordinary LSP and fixed debugger self-tests do not gain this handle. The reader still verifies the one-use nonce, monotonic event sequence, Job membership, original LLDB event-owner OS thread, received/continued state and correct event ownership.
2. **Release-stage LLVM source provenance.** The pinned Windows build recipe now emits the patched first-party C++ event-loop source, header/patcher/LLDB executable SHA-256s, exact upstream source commit, and a non-executable provenance manifest. The Tauri build script independently checks these bytes, their exact hook anchors, opt-in Windows-only feature configuration, and the existing FIN-02 whole-bundle/service pins. The provenance manifest explicitly has `productionDebuggerActions=false` and `replayAuthorized=false`. It is a source/binary input receipt, **not** independent proof of an installed or protected native debugger run. Missing or mismatched stage data rejects the observer build.
3. **Live release owner-only witness.** Within the protected Windows DAP worker, `observe_release_owned_stop` binds the original native Job, live first-party owner event, rechecked native debugger and producing-build journal receipts, immutable source snapshot and still-stopped OS process/thread/RIP. It fsyncs the private first-party owner receipt and rechecks the original live OS event. The normal desktop witness may show its original receipt digest and owner sequence. The independently protected PDB source line is **not** demonstrated in this release path; source-line proof, debugger action authority, recovery authority and installed acceptance remain **false**. No WebView-provided PID, RIP, command or path reaches the observer.
4. **One-use action journal foundation and independent result verifier.** The existing durable Prepared → Approved → Consumed and native one-use action journal reader can be linked into the Windows release-observer build solely for verification. The independent D6C post-step verifier also compiles but its action-slot admission **unconditionally refuses non-test builds** with the release-observer feature. The currently available authentic PE/PDB oracle launches an uncontained helper in its CI-only implementation. Until that oracle is replaced by a truly protected native service and accepted on real Windows, no release-grade breakpoint, step, frame/variable read, positive post-command result, native action consent or general DAP control is executable. The old DEV-04.6D dialog remains explicitly debug-only and unshippable.
5. **OS-only interrupted action recovery.** The new `ide_debug_action_os_recovery(projectId)` entrypoint takes only a registered project identity and displays a genuine native human approval dialog. It requires an unresolved original action, the original FIN-01D stale global marker, no active native debugger and already-terminal original build/debug/LSP journals. It queries actual original Windows named Job Objects or delegated Linux cgroups **twice**, verifies prior owner exit, all expected original job identities and kernel-empty/absent counts, replays original action receipts and compares original journal/action/marker bytes before and after. It then `create_new`/fsyncs one private audit-only witness. The original action outcome remains `unknown_never_replay`, the original action journals and global marker are not changed, and the receipt grants neither a new job nor debugger control. Unexpected provenance, a live owner, changed journal, missing OS witness or duplicate recovery file fail closed.

## Mandatory blockers (not acceptance-only paperwork)

- **FIN-04R-1 — Protected source-line oracle:** replace the CI-only external `llvm-symbolizer` launch with an exact pinned, OS-contained interpreter/worker or independently audited in-process PDB reader. Recheck PE, PDB, original source seal, immutable Job, real owner event before/after analysis.
- **FIN-04R-2 — Production per-operation OS approval:** connect independently authenticated *current* owner+protected-PDB evidence to an original one-use native consent marker, exact operation intent and journal consumption **before** the first DAP request. Never accept an AI/WebView consent field or a historical owner receipt as a grant.
- **FIN-04R-3 — Verified post-command result:** bind the one consumed action to a *new* first-party owner-event sequence and independently reverified thread/RIP and PDB source line. DAP ACK, stack frame and exit code are insufficient; breakpoints and variable inspection need separate physical memory/frame custody proofs.
- **FIN-04R-4 — Recover interrupted action outcomes:** OS-tree cleanup evidence must be separately joined to the original action/result journal with immutable non-replay reconciliation. Unknown effects remain unknown; the original FIN-01D global fence must require its separate OS-backed human clearance. No automatic replay, inferred success, approval restoration or restart.
- **FIN-04R-5 — Native Windows and release acceptance:** compile the exact release feature against the actual approved installer-pinned LLVM, SCM recovery service, AppContainer and protected Job; run real positive and negative native fixtures for owner death, unauthorized DAP, refusal, expiry, cancel/restart, corrupt receipts, event continuation races, process escape and independent post-step PDB mapping. Installed NSIS owner-operated consent, accessibility and restart acceptance are still outstanding. The CI and developer-source tests cannot substitute for this.

**FIN-02 remains deferred** under the owner's instruction; its runtime/source packaging has not been certified. There is no production debugger-control promotion, default feature change, CI submission, native compilation, physical acceptance, main merge, feature freeze or release in this development slice.

## Handoff status

Source changes and negative structural regression fixture committed to `ide/fin01-07-implementation-20261111`. GitHub-only source review can verify the changed files, but the native Windows/LLVM/SCM and installed desktop environment is unavailable in this coding session; all claimed OS evidence is **implementation design or guarded native calls**, not a captured live native acceptance result.

## FIN-04R-1 continuation (source update, 2026-11-12)

The earlier note that no protected production PE/PDB reader source
exists is superseded for the **release-observer** path: the strict
parser is shared, and the pinned fixed-purpose symbolizer is now
launched under the original Windows native Job/AppContainer with SCM
process custody and before/after PE/PDB/source/bundle checks. A live
first-party owner event may therefore acquire a separately parsed
read-only PE/PDB source-line witness *when* all OS checks succeed.
The new human consent route is intentionally a **one-use review**:
even an affirmative approval is immediately journaled Revoked with
no DAP command or executable authorization. Independent step and
breakpoint-hit proof source exists, but native post-command positive
execution evidence is not present.

This does **not** close release acceptance. See
[`IDE_FIN04R1_PROTECTED_PDB_AND_NATIVE_ACCEPTANCE_20261112.md`](IDE_FIN04R1_PROTECTED_PDB_AND_NATIVE_ACCEPTANCE_20261112.md)
for remaining compiler, OS-hostile, physical-human, installed and
post-command gates. No actual Windows compiler or kernel result was
obtained during this source iteration; no production controls enabled.
