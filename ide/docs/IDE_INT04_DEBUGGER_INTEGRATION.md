# IDE-INT04 — Protected debugger integration (source implementation)

**Status:** INT04 workspace/backend integration implemented in source on the existing IDE implementation branch. Compiled Windows/Linux native gates, installed packages and physical human approval intentionally deferred until the backlog's consolidated acceptance phase. No release/merge authorization.

## INT04-A — Protected native target and source ownership

- Reuses existing D3/WO6 source-frozen `rust_build` receipt, independently audited private executable/symbols, feature-gated LLDB-DAP and original Windows AppContainer/SCM and Linux cgroup/Bubblewrap custody. Does not add new process execution, arbitrary target selection, attach or CLI.
- The desktop now binds a completed LLDB run to the **actual fsynced original terminal supervisor receipt hash**, in addition to the existing exact project/job/plan, frozen source HEAD, LLDB bundle and original Cargo executable/symbol receipt.
- A supported exact saved-source initial stop observation is produced only after an actual correlated `stackTrace`, a safely interpreted fixed native workspace path, independently sealed original source snapshot, bounded SHA-256 file hash, fresh registered-project saved-file hash and line bounds. The DAP source path and runtime instruction pointer never leave the native host. This is **adapter-reported source**, NOT verified CPU instruction to DWARF/PDB source mapping. Missing source or a nonmatching path yields no link, never a guessed location.

## INT04-B — One-use debugger actions and native authorization

- Existing developer-only Windows feature `dev046d-native-dialog` remains the only source entrypoint for one native-human-approved step over, same-source-line breakpoint attempt or one read-only redacted-variable inspection; separate launch and action consent, original independent stopped-owner receipt, one-use action journal, cancellation and kernel release are unchanged. Default and release builds retain **no** operation-control entitlement.
- The frontend's action availability is now constrained by the same native-reported project and **global** process custody state. A debugger owned by another project or any unreconciled job/action prevents a misleading green/runnable state. Breakpoint ACK remains **ResultUncertain**, never a verified hit or execution-success claim.

## INT04-C — Read-only frames, redacted local inspection and navigation

- New typed stop-source evidence and presentation validator carry only project-relative saved-file path, exact SHA-256, 1-based reported line and frame ID. Adapter-controlled absolute paths, PID/TID/RIP, memory references and arbitrary DAP requests remain private.
- New `ide_debug_review_stopped_source` is a read-only native command: WebView passes only project/job/plan, not source path. The original current-process debugger witness, D3 journal terminal hash, completed and nonquarantined native job and currently registered saved-file SHA/line are independently checked again. This does **not** restore a debugger process or grant a second session.
- On a direct user click, the IDE previews the same native-registered source and rechecks the SHA, CodeMirror live identity, unsaved/dirty state, project-navigation epoch and open-tab budget. Changed or dirty buffers are preserved; no stale cursor or auto-save is allowed. The safe destination is the beginning of the reported line, not an unverified debugger column.
- Read-only variable names/types are bounded to 16 redacted records from one separately native-approved developer-only inspection. Duplicates, raw values, expression evaluation, child expansion and memory references are never displayed or accepted.

## INT04-D — Durable recovery, accessibility and evidence boundaries

- Native debugger status now includes **global recovery required**, **active in another project**, and **orphaned action journal** states, in addition to existing project-scoped journal/quarantine. These global facts gate the UI in the same way as the supervisor's actual global `prepare` and launch checks.
- New debugger run invalidates the previous in-memory stop/source receipt atomically under native owner lock. New protected Build/Test invalidates outdated editor debugger observations. Restart remains journal-only; no resumed debugger grant or synthetic last successful stop.
- Jobs UI explicitly distinguishes: protected terminal receipt; DAP-reported source whose saved bytes were independently checked; independently OS-proven line map (not established here); native-only developer action consent; release/installed acceptance (not established). Keyboard/screen-reader labels and read-only failure messages are present.
- Source-level negative tests cover stale original build/terminal identity, foreign project, global quarantine, spoofed source, authority elevation, mismatched native review and redacted-only variables.

## Remaining acceptance and design boundaries

- **Do not claim native acceptance.** Compiler, native Windows/Linux OS/LLDB runs, signed installed packages, real physical human modal approval, crash/restart and accessibility acceptance are deferred by user directive.
- Native Windows/LLDB stopped-owner PE/PDB evidence, Linux LLDB ptrace restrictions and the existing D6D single-operation experiment require the final consolidated exact-head gate. No external CI, PR verification, release merge or installed-app test was launched.
- Source location remains **DAP-reported**, and the UI does not claim a verified breakpoint hit. Full live persistent debugger sessions, arbitrary multithread frame navigation, variable evaluation, production per-action approval and debuggee launch outside the original fixed protected project target are **not enabled**.
- The next implementation group is **IDE-INT05 — Toolchain, package and terminal integration**. Reuse the existing protected IDE-05 work, with optional Terminal CE only where separately reviewed. Keep debug release/physical gates in the deferred acceptance ledger.
