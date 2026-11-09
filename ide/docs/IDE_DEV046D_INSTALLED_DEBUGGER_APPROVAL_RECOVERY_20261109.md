# DEV-04.6D — Installed debugger approval, recovery and end-to-end acceptance

**November 9, 2026: partial implementation committed; native and signed installed acceptance NOT RUN.** The native D5 owner event and D6C step/breakpoint dispatch still exist only in disposable Windows CI. They are not installed debugger capabilities.

The Tauri IDE already has a native operator dialog for a separately approved sealed project-debugger *launch*, and a different native OS-only recovery dialog for interrupted protected jobs. Restart does not recreate the debugger job or permission. These are distinct from human approval of each source breakpoint/step action, which has NOT been implemented in the installed app. The D6B CI marker is labelled \`ci_only_not_installed_consent\`, not real human approval.

## D6D implementation on the D6 task-group branch

- Desktop status adds explicit \`operationControlSupported=false\` and \`DEV046D_NATIVE_HUMAN_ACTION_APPROVAL_NOT_ACCEPTED\`, even when project debugging is separately feature-enabled. The default feature-off status agrees.
- A bounded private action journal restart scanner examines all native D6 receipts (up to 4096 actions, 32 KiB each), rejects Windows reparse points, symlinks, malformed names and hash/transition errors. It enforces immutable Job/plan/operation/native-owner coordinates, monotonic event timestamps and recognized outcome codes across all transitions, including rehashed later entries.
- Orphan \`Prepared\`, \`Approved\` and \`Consumed\` actions require separate OS-only cleanup; \`Denied\`, \`Revoked\`, \`ResultUncertain\` and \`ResultVerified\` are audit-only, never grants. The supervisor joins every action to a real persisted original Job and plan; unknown jobs fail closed.
- The native D5/D6C Windows acceptance fixture now checks the full restart inventory after protected Job terminal, not merely the one expected action hash. No action may be resumed during this audit. An existing D6C Rust partial-move bug was repaired by cloning owner receipt fields before reusing the attested owner witness.
- Four hostile Rust tests and three frontend source-boundary tests were added. The existing grouped Windows script covers these tests. No smaller workflow was added or dispatched.

## Still-required full D6D acceptance

1. Native Windows/Linux compilation, rustfmt, Node tests, protected D5 LLDB owner-event tests and D6C step/breakpoint tests on one exact head; repair genuine results.
2. Promotion of a **separately reviewed, OS-owned first-party verified-stop bridge** from CI-only to release-eligible code, without exposing a PID, path, DAP command or grant to the renderer/model.
3. A real installed native operator dialog **per action**, binding challenge/nonce, reviewed source/line/thread/owner sequence, exact sealed Cargo executable and original build/debugger receipts. Denial/expiry/changed owner event must revoke the action. Fsync consumption before DAP, never reuse prior launch consent.
4. Real Windows signed NSIS install and Authenticode verification of installer/desktop/service; user approval of the exact source operation; new independent LLDB/OS/PE/PDB owner evidence; cancel, forced crash, restart, terminal Job cleanup and uninstallation. Distinguish actual breakpoint hit from LLDB acknowledgement; variables and evaluate remain disabled. Repeat Linux signed/installed workflow under protected cgroup as supported.
5. Keep CI-simulated consent and signed installed human approval separate in every report. No source commit, pending run or synthetic receipt passes an installed gate.

**Release decision: BLOCKED.** No real installed human per-action approval or native exact-head result has passed; no expanded debugger authority is enabled. Consolidate all D6 tasks into a single end-of-group native OS gate rather than stacking tiny CI workflows.
