# Lu Core P0-SEC — Linux guardian and process-custody hardening

Source-only candidate, November 11, 2026. Branch: lu/p0-sec-guardian-hardening-20261111, stacked on Lu Core P0-WIN-CHAT #224. Excludes standalone Lu IDE. No merge, release, or native acceptance.

## Security invariants implemented in source

1. Linux process-group and delegated cgroup guardian READY handshakes use a bounded 3-second host poll/read on the already-owned stdout FD. No blocking read_exact/read_line, detached stdout thread, or silent helper is admitted. A failed admission kills/reaps the exact spawned helper. UNAVAILABLE additionally requires the helper to exit successfully within two seconds.
2. When a kernel cgroup is available, Lu verifies the actual admitted worker PID in its original /proc/PID/cgroup before marking a task RUNNING. It separately checks that the custodian remains alive and stays outside its own killable subtree.
3. The active host polls the custodian's lifetime. Premature loss is ProcessUncertain, not a completed task or positive kernel-custody receipt. Lu attempts to stop the known child leader but cannot promise detached descendant cleanup without a surviving guardian.
4. A terminal worker is only removed from the native supervisor's owner map after the Linux cgroup or process-group guardian reports successful shutdown. A failed stop retains task ownership; neither cancellation nor completion is falsely accepted.
5. The original guardian still independently runs cgroup.kill, requires recursive cgroup.events populated 0, and creates the scoped OS receipt only on success. Receipts are historical observations, never execution authority; no model, WebView, or recovered task can select a PID, OS scope, helper path, or worker.

## Changed source

- src-tauri/lu-win-worker-job/src/lib.rs — bounded admission, RAII failure cleanup, live custodian and /proc membership check, Rust native negative handshake cases.
- src-tauri/lu-core/src/native_background_process.rs — verified post-spawn membership and fail-closed active-poll and cleanup ordering.
- src-tauri/lu-win-worker-job/src/bin/lu-e1-p08-linux-cgroup-crash-parent.rs — independent fixture membership check.
- lu-desktop/tests/p0-sec-guardian-hardening.test.mjs — source-regression contracts for timeout, membership, loss and ownership ordering.

## OPEN acceptance and implementation boundaries

- Native exact-head Linux and Windows compile not run. Destructive delegated cgroup-v2 systemd Delegate=yes test of actual host SIGKILL, nested setsid descendant, kernel empty state and unrelated process survival not run on this branch. Source contracts do not establish kernel custody.
- Negative real-worker Linux guardian SIGKILL/stall, cancellation-versus-completion, authority revocation, uncertain-effect durability and restart reconciliation still need executable fixtures with original host journals and proof. In particular, current active-host polling is not an independently surviving guardian watchdog.
- A hostile process with permission to write an ancestor cgroup.procs may migrate outside the originally delegated leaf. Without an OS nonmigratable boundary or explicit negative proof, general descendant containment remains UNVERIFIED.
- Kernel-uninterruptible helper teardown/reaping cannot be guaranteed merely by a user-space timeout. Treat an unobserved reap as uncertain, never success.
- Portable Windows Job subtree and human-installed Windows/Radeon alpha acceptance remain separate, explicitly deferred. No IDE, NVP, automatic CI, PR merge or release operations are included.

## Next P0-SEC implementation slice

Construct a real delegated-kernel negative fixture that kills or stalls a first-party guardian after a live admitted worker/nested setsid descendant exists, then prove bounded host failure reporting, no false COMPLETED/cancelled receipt, independently observed survivor state, and recovery. Combine with reviewed Git/media cancellation and revocation races and genuine persisted restart evidence before closing P0-SEC or advancing to P0-PLAN.