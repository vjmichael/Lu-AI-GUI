# AGENT-01L — OS-verified descendant custody (Windows candidate, Linux fail-closed)

## Purpose and scope

Lu Core remains a normal native desktop application, not a sandbox. Its **reviewed external workers** are scoped, supervised and independently checked. The existing AGENT-01K recovery layer refuses to classify a worker tree as fully ended without an operating-system witness; this change supplies a narrowly bound **Windows Job Object kernel observation** while preserving explicit Linux uncertainty.

AGENT-01L is not an unattended execution loop. Neither a kernel witness nor a successful reviewed Git/FFprobe result grants a new operation, tool, model turn, or scheduler lease. Future work still needs fresh human consent and independently reviewed sources.

## Native Windows path (source candidate)

- The audited, first-party `lu-win-worker-job` Job retains its original private Win32 handle. Creation configures `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`. The reviewed worker is spawned **suspended**, assigned to that Job, then resumed. No model-generated PID or arbitrary Job handle can be substituted.
- `Job::terminate_and_verify_empty` requires that same object to have actually assigned a worker. It calls kernel `QueryInformationJobObject(JobObjectBasicAccountingInformation)`, checks the returned ABI length and `ActiveProcesses == 0`, sends `TerminateJobObject` only when a live assigned member remains, and polls the exact still-owned Job to zero with a strict deadline. Failure, missing Job, unassigned Job, incorrect ABI, timeout or still-live processes all deny a proof.
- The method returns `WindowsJobEmptyProof`, an opaque Rust type with private fields. It is produced only after the native kernel observation; no bool or model argument can construct it.
- `BackgroundProcessSupervisor::poll` retains the real handle until the kernel proof is acquired, closes it only after that observation, then joins supervised stdout. A durable witness is inserted **only for successful, bounded, reviewed Git/FFprobe tasks** (not generic processes, cancellation, timeout, failed result or empty output), after task terminal persistence and before the verified result is exposed to the application broker.
- `TaskStore::record_verified_windows_job_empty` accepts only the opaque proof, exact reviewed effect-call suffix, a `COMPLETED` original task, the same `g-...` workspace/execution turn, and a matching saved `lu_reviewed_turn_origin`. Its method-tagged row is unique, immutable, journaled with WAL FULL and is created **after** legacy task-schema migrations (no old task gains retroactive custody).
- `HostOriginalJobCustody` is the read-only AGENT-01K provider. It opens only the trusted profile's exact original task journal; checks regular-file / reparse / sidecar integrity, the exact original chat/turn/call, native task COMPLETED and `WINDOWS_JOB_ACTIVE_ZERO_V1`. It does not accept front-end paths, model assertions, arbitrary PIDs, SQLite effect status alone, or another turn's proof. Missing/uncertain custody remains false.
- AGENT-01K still independently reopens original TaskStore, EffectJournal, independent semantic oracle, latest plan/ANSF source and all ancestry before even reporting `REVIEW_NEW_STEP_WITH_FRESH_AUTHORIZATION_ONLY`. That status is **review only** with `NOT_AUTHORIZED_FOR_EXECUTION`.

## Linux safety boundary

The current first-party `lu-linux-group-guard` can attest that its private PGID was signalled with SIGKILL, **not** that a deliberately detached `setsid()` descendant left the OS. A process group cannot prove escape-capable descendant custody and an OS child may remain alive. For this reason, `HostOriginalJobCustody` deliberately always returns false on non-Windows platforms. The genuine Linux kernel negative-control `benchmarks/e1/e1_p08_setsid_escape_probe.py` is required in AGENT-01L CI.

**Open AGENT-01L Linux gate:** obtain a legitimately delegated per-operation cgroup-v2 subtree and an independently supervised custodian that can survive abnormal Lu termination. The custodian must issue scoped `cgroup.kill`, independently read recursive `cgroup.events populated 0`, and persist an exact task/turn/source-bound receipt. It must not use root escalation, assume writable global cgroups, trust model-selected cgroup paths or treat the host's death as permission to replay the operation. Complete native escape-capable tests before enabling any Linux positive witness. See `docs/LU_E1_P08_CGROUP_V2_ESCAPE_DECISION_20261108.md`.

## Tests submitted / evidence still outstanding

1. On Windows: actual Win32 Job Object with frozen `cmd.exe` creating a descendant; verify kernel `ActiveProcesses == 0` after Job termination. An unassigned empty Job must never create a proof.
2. On Windows: genuine reviewed native Git HEAD through `BackgroundProcessSupervisor`, OS Job verification, immutable task-bound witness in SQLite, drop/reopen, exact-chat/turn/call cross-binding rejection and rejection of updates/deletes.
3. Both platforms: original-journal proof missing, unreadable or mismatched must fail. Linux returns no proof and a real `setsid()` escape negative control must survive killpg.
4. Windows: actual React/Tauri compile with *clearly labelled inert* engine placeholders; not an installer or real GGUF claim.
5. Exact-head Windows 2025/Ubuntu 24.04 GitHub workflow `lu-agent01l-os-custody.yml` with immutable source-tree lock and evidence artifacts.

**Status:** implementation committed as a draft source candidate, native logs not yet observed. No full production guarantee until native success and physical Windows NSIS testing. The loaded-Qwen two-step and exact OS-custody replay-denial flow require separate installed-app proof. Do not enable automatic multi-step/retry.

## Remaining AGENT-01L milestones

- **01L-WIN:** Native Windows exact-head compiler, nested Job and reviewed Git witness, restart/replay rejection; repair actual failures.
- **01L-LNX:** Delegated cgroup-v2 kernel custody and out-of-process guardian, proven after Linux host crash with escaped grandchild.
- **01L-QWEN:** Actual loaded-Qwen two independently approved native steps, independent Git/FFprobe oracles, restored per-turn Job/cgroup custody, OS/crash restart and no replay.
- **01L-INSTALL:** Exact installed NSIS and Linux distribution acceptance. Signing/legal approval remain separate release decisions.

A historical kernel Job observation is host-trusted evidence, not a cryptographic signature against someone who can arbitrarily rewrite the user's local database. It is bounded to Lu's owned Job and does not certify unrelated external OS processes.
