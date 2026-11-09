# DEV-04.6D — Installed debugger approval, recovery and end-to-end acceptance

**November 9, 2026: partial implementation committed; native and signed installed acceptance NOT RUN.** The native D5 owner event and D6C step/breakpoint dispatch still exist only in disposable Windows CI. They are not installed debugger capabilities.

The Tauri IDE already has a native operator dialog for a separately approved sealed project-debugger *launch*, and a different native OS-only recovery dialog for interrupted protected jobs. Restart does not recreate the debugger job or permission. These are distinct from human approval of each source breakpoint/step action, which has NOT been implemented in the installed app. The D6B CI marker is labelled \`ci_only_not_installed_consent\`, not real human approval.

## D6D implementation on the D6 task-group branch

- Desktop status adds explicit \`operationControlSupported=false\` and \`DEV046D_NATIVE_HUMAN_ACTION_APPROVAL_NOT_ACCEPTED\`, even when project debugging is separately feature-enabled. The default feature-off status agrees.
- A bounded private action journal restart scanner examines all native D6 receipts (up to 4096 actions, 32 KiB each), rejects Windows reparse points, symlinks, malformed names and hash/transition errors. It enforces immutable Job/plan/operation/native-owner coordinates, monotonic event timestamps and recognized outcome codes across all transitions, including rehashed later entries.
- Orphan \`Prepared\`, \`Approved\` and \`Consumed\` actions require separate OS-only cleanup; \`Denied\`, \`Revoked\`, \`ResultUncertain\` and \`ResultVerified\` are audit-only, never grants. The supervisor joins every action to a real persisted original Job and plan; unknown jobs fail closed.
- The native D5/D6C Windows acceptance fixture now checks the full restart inventory after protected Job terminal, not merely the one expected action hash. No action may be resumed during this audit. An existing D6C Rust partial-move bug was repaired by cloning owner receipt fields before reusing the attested owner witness.
- Four hostile Rust tests and three frontend source-boundary tests were added. The existing grouped Windows script covers these tests. No smaller workflow was added or dispatched.


## D6D continuation — verified native owner stop → Tauri-owned operator modal (developer build)

A further, explicitly opted-in **Windows debug-build-only** integration now connects the existing D5 protected LLDB owner event to a real **Tauri OS-native** per-operation dialog. The extra feature is \`dev046d-native-dialog\`, which depends on the separately opt-in \`native-project-debug\` stack and D5/D6C native debug gates. A compile-time guard **rejects release builds and non-Windows builds** of this experimental feature. Default and signed NSIS builds continue to deny source-control actions.

The IDE displays separate **Step Over** and **same-verified-source-line Breakpoint** fixed-choice buttons only when that feature and installer-pinned protected bundle are available. The renderer submits *only* the project ID using one of two fixed Tauri commands; it cannot send source paths, PID/TID, arbitrary DAP payload, variable names, arguments or authorization tokens. Each button starts a fresh sealed project-debug session, so the existing native launch approval is independent from the **second** operation-specific dialog. There is no persistent, interactive debugger session in the UI yet.

The native Rust backend first obtains the original stopped DAP thread and stack, then requires D5 independent evidence: first-party LLDB Windows kernel event ownership, Job-scoped PID/TID/RIP, compiler PE/PDB source line, immutable Cargo target and source snapshot, original build Completed and debugger Running receipts. The trusted supervisor constructs a *new* D6B operation preview, native receipt and random one-use challenge. Only then does the trusted Rust backend call Tauri's \`blocking_show\` native dialog with the original Job ID, exact source line/thread/owner sequence, source and review digests, operation name and challenge. The dialog result is never supplied by the WebView or an AI model.

The supervisor distinguishes \`native_human_confirmed\` from \`ci_only_not_installed_consent\`. After the dialog, it **rechecks the exact native owner sequence, Job state, original immutable source and target**, rejects timeout/denial/cancellation/changed owner, fsyncs \`Consumed\` and permits one fixed DAP request only through the existing non-clonable slot. Step Over can record a \`native_dialog_independent_os_step\` result **only** after a new owner event and independently rechecked OS/PDB stop. The breakpoint records \`ResultUncertain\` even if LLDB acknowledges it. Normal close requires LLDB-owned ContinueDebugEvent, full Job-tree cleanup and original durable terminal receipts. Restart has no action replay mechanism.

**Important boundary:** this is *source-integrated developer-mode human approval*, **not yet a successful Windows compiled/tested session** and **not signed installed acceptance**. Native Rust/Windows compilation, genuine Tauri dialog behavior, patched LLVM bundle identity, OS stop/continue, user decline/timeout/cancel and forged-receipt recovery must be demonstrated with completed logs before enabling release execution. No default shipping feature flag was changed. D6B/C CI fixture approval remains test-only, and no release feature can be accepted from these source edits alone.

## Still-required full D6D acceptance

1. Native Windows/Linux compilation, rustfmt, Node tests, protected D5 LLDB owner-event tests and D6C step/breakpoint tests on one exact head; repair genuine results.
2. Promotion of a **separately reviewed, OS-owned first-party verified-stop bridge** from CI-only to release-eligible code, without exposing a PID, path, DAP command or grant to the renderer/model.
3. A real installed native operator dialog **per action**, binding challenge/nonce, reviewed source/line/thread/owner sequence, exact sealed Cargo executable and original build/debugger receipts. Denial/expiry/changed owner event must revoke the action. Fsync consumption before DAP, never reuse prior launch consent.
4. Real Windows signed NSIS install and Authenticode verification of installer/desktop/service; user approval of the exact source operation; new independent LLDB/OS/PE/PDB owner evidence; cancel, forced crash, restart, terminal Job cleanup and uninstallation. Distinguish actual breakpoint hit from LLDB acknowledgement; variables and evaluate remain disabled. Repeat Linux signed/installed workflow under protected cgroup as supported.
5. Keep CI-simulated consent and signed installed human approval separate in every report. No source commit, pending run or synthetic receipt passes an installed gate.

**Release decision: BLOCKED.** No real installed human per-action approval or native exact-head result has passed; no expanded debugger authority is enabled. Consolidate all D6 tasks into a single end-of-group native OS gate rather than stacking tiny CI workflows.
