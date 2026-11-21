# DEV-04.6B — Native operator consent contract and one-use debugger action receipts

**Engineering status, November 9, 2026:** implemented source and native-CI integration on `ide/dev04-6a-debug-control-admission-20261109`. NO Windows/Linux compilation evidence, live debugger command, installed OS-human-consent acceptance or new CI workflow is claimed. Stacked on the D5 draft #192. Existing protected debugger launch consent is never reused for an action.

## Architecture implemented

`ide/supervisor/src/debug_action_receipts.rs` is compiled only under `cfg(test)` or the explicitly reviewed D5 Windows debug CI feature `dev045d5-ci-event-owner` and debug assertions. It is not a Tauri command, a WebView action, an AI tool, a DAP request or a production worker entitlement.

The host first constructs a non-executing `DebugReviewPreview` from the independently verified **actual** D5 kernel event owner, Job-owned PID/TID/RIP, sealed original Cargo PE/PDB and source snapshot, and original one-use debugger/Completed build receipts. D6B calls `pending_native_binding`, which accepts only that exact live pending review and owner sequence.

For each single reviewed operation, a separate private approval session binds the original Job ID, debugger plan, build Completed receipt, debugger Running receipt, owner-event receipt, source snapshot, source line and OS stop identity. An OS-CSPRNG random nonce derives a unique SHA-256 action ID and a fresh **operation-specific** challenge. Files contain digests and bounded coordinates, **never a reusable raw approval token, source text, variable values or absolute project path**. The native worker selects the trusted journal parent; an adapter/model cannot choose a destination.

The append-only NDJSON chain supports:
- `Prepared`: challenge issued, no grant.
- `Approved` or `Denied`: an explicit decision scoped to this exact challenge and owner event. Without confirmed native approval, deny.
- `Consumed`: the exact operation claim is marked used and **fsynced before any future dispatch**; repeated claims fail, new events cannot use an old approval.
- `Revoked`: wrong/stale event, wrong preview, expiry, cancellation or operator revocation invalidates any outstanding approval.
- `ResultUncertain`: the only currently permitted post-consumption outcome. No debugger command was issued, so the host must not invent a successful result.
- `ResultVerified`: reserved in the phase parser for later D6C independently verified native result evidence; there is **no current writer or caller Boolean** capable of creating it.

The journal is `create_new` under the supervisor's native private receipt directory. Every transition includes ordinal, prior SHA, event/Job/plan/preview/action coordinates and timestamp; serialization is bounded and the file is `sync_all`ed before in-memory state is promoted. Reopening after restart reads and independently verifies each phase transition/hash and the expected final receipt. **Audit is read-only:** reopening cannot resurrect a consent, recreate a consumed token or authorize another DAP command.

The approval window is capped at 30 seconds, and the D6A review is still limited to eight previews and one pending review per verified stop. A different breakpoint line, thread, source snapshot, owner sequence or unverified variable frame is denied before consent.

## CI-only integration and exact remaining human-approval gap

The real Windows protected D5 acceptance fixture calls the new operation admission book only **after** authenticating the actual first-party LLDB `DEBUG_EVENT` sideband and independently verified PE/PDB/source and journal evidence. It uses `NativeConsentMarker::for_native_ci_fixture`, which explicitly stamps `ci_only_not_installed_consent`. The fixture tests a same-thread step proposal, one-use approval/consumption, second-use denial, result uncertainty, and audit-only reopening after supervisor restart.

**This is an executable CI contract, not a real human pressing an OS-native dialog.** The production native consent constructor and installed OS dialog are intentionally absent. Native human UI approval, one-use source-breakpoint/step DAP dispatch, independent result verification and installed-app acceptance must be implemented and independently verified in the later D6C/D6D task groups. Do not claim real human consent from the CI-simulated marker.

## Adversarial source tests

Nine self-contained Rust tests cover distinct operation/nonce receipts, no consent, wrong challenge, wrong owner sequence, wrong source/review binding, expiry, explicit denial, duplicate decision, duplicate claim, cross-operation challenge substitution, stale-event post-approval consumption, tampering, reopen-only audit, and the fact that an approval receipt cannot provide execution authority.

All source tests remain UNVERIFIED against a native Rust compiler at this head. No Windows or Linux run was triggered in this subgroup.

## Consolidated CI policy

Following the owner's explicit decision on Nov 9, 2026, **do not create small workflow jobs or submit a new draft PR after each subtask**. Finish the D6 control/authority task group on the present working branch, then run one grouped exact-head Windows/Linux matrix for native compilation, hostile source tests, pinned LLVM owner-event stop/continue, Job/cgroup custody, cancellation/crash, journal recovery and installed consent (where implemented). Do not disable required acceptance to save CI time; all old and new native/installed gates remain open until real results pass.

**Nonclaim:** Breakpoints, stepping, variable inspection, evaluate, arbitrary attach and shell remain disabled. D6B stores admission/revocation receipts but *cannot* send a debugger command. A source commit, a test declaration and a queued workflow are not native OS acceptance.

## D6C follow-on (same branch, not an upgrade of old consent)

D6C now has a **separate, CI-only, executable test slot**, minted after
a new D6B simulated operator challenge is consumed and fsynced. The
older D6B review exercise still records `ResultUncertain` because it
never dispatched a command. D6C's step test can record
`ResultVerified` only if the original native D5 OS/PE/PDB inspector
establishes a *new owner event* at a new RIP on the same protected
thread. A breakpoint acknowledgement is still `ResultUncertain`.
There is still no production human approval constructor.
See `IDE_DEV046C_NATIVE_CONTROL_DISPATCH_20261109.md`.
