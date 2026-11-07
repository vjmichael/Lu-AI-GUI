# P02 — Background Task Scheduler + Lu Resource Guardian

**Work order:** TASK-SCHED-01 · **Priority:** P02 (after FS-DISCOVER-01)  
**Status:** Native source and desktop integration implemented; GitHub exact-head compilation/tests and physical Windows acceptance **OPEN**. Stop before P03 until new owner instruction.

## Owner goal

Lu must coordinate long work while the user chats, without wasting LLM tokens polling workers or overrunning hardware. The shared native host—not the model—owns queue order, process launch, cancellation, result receipts, and budgets. Work is bound to operator-authorized tasks and a registered reviewed worker. Resource admission does not equal new permission to read files, run executables, or use the network.

## Reused first-party infrastructure

- `tasks.rs`: existing durable SQLite task lifecycle (RUNNING/COMPLETED/FAILED/CANCELLED/TIMED_OUT/INTERRUPTED), checkpoints, progress, final notices, acknowledgements, chat-scoped dashboard, opt-in model continuation.
- `native_background_process.rs`: verified executable SHA, genuine host-owned process, Windows Job Objects/POSIX process groups, reaping/cancellation and terminal proof. Unmodified spawn/executable path authority.
- Lu background watchdog: polling owned processes and delivering terminal notices without keeping a transformer active.
- Lu's existing task pane and native source-file picker. No separate process runner, shell, or external task provider.

## New implementation

### A. Persistent scheduler — `native_background_scheduler.rs`

- New private SQLite `lu-background-schedule.sqlite` holds **task metadata only** (task ID, chat/workspace owner, job kind, state, priority, due time, recurrence interval, explicit dependencies, reservation request, event records, retry limits and result states). Executables, argv, source paths and tool privileges are **not** serialized.
- Only the native host can submit a previously approved `TrustedWorkerPlan`. Runnable plans stay in **volatile host memory**. On restart, previously queued/running/paused scheduled rows become **NEEDS_REVIEW**; the persisted task store also marks previously RUNNING work INTERRUPTED. No silent auto-replay or false finished-artifact claim.
- Work is sorted by priority and age, with aging after hours to discourage starvation. Due/future jobs and dependency-blocked jobs stay queued. The SQLite queue is bounded to **128** active scheduled rows; valid owned query returns at most **100**.
- Allows only *host-declared idempotent* jobs to retry automatically (0–3 retries; bounded backoff). Completed recurring jobs can queue a new, uniquely identified occurrence while the original reviewed in-memory job plan still exists in this Lu session. **After restart, recurrence needs new operator approval**. Cancellation, interruption and uncertain effects are not automatically repeated.
- The only automatic execution effect is invoking the existing independently pinned `BackgroundProcessSupervisor::start_host_approved` after schedule and resource admission. The existing process supervisor remains solely responsible for OS containment and terminal TaskStore events.
- Operators can pause/resume **queued**, not arbitrary running, tasks. Queued cancellation removes in-memory plans; running cancellation is performed by the owned-process supervisor after native confirmation.

### B. Resource Guardian admission and monitoring contract

- `ResourceClaim` and `ResourceBudget`: checked sums for concurrent worker count, CPU slots, RAM reservations, GPU slots/VRAM, disk slots and network slots.
- Conservative default: at most two CPU/background worker slots when hardware has capacity, at least one CPU reserved for foreground activity, **1 GiB maximum background reserved memory**, disk slot 1 and **no GPU/network background starts by default**. The host watchdog reduces admission to one worker/CPU and 512 MiB when a model is loaded.
- Resources remain reserved until real TaskStore terminal records confirm completion or termination; uncertain or missing receipts **do not release reservations**. Shrinking the budget refuses subsequent jobs, rather than abruptly killing existing workers.
- **This is a native admission/reservation guardian, NOT live OS-wide CPU/RAM/VRAM usage instrumentation.** Workers remain subject to their existing OS containment limits; independent physical resource telemetry/driver resource admission and moving all non-scan native background paths into this scheduler remain open.

### C. Desktop and real first-party job

- Existing **Scan media file…** now uses `BackgroundScheduler::enqueue_host_plan` instead of direct starting, after native OS file selection and installed executable digest pinning.
- UI controls: start as resources permit or delay 5/30/60/1440 min; repeat once/hour/day/week; background/normal/high priority; native confirmation for delayed/recurring plans; no model job authority.
- The background pane shows scheduled entries alongside actual TaskStore receipts, with priority, due time, repeat interval, the assigned resource admission limit, and queued pause/resume/cancel. A separate chat-scoped guardian status returns queue/running/paused counts and global reserved slots but no other chat's file paths or task payloads.
- Native watchdog dispatches due queued jobs every ~500 ms; actual TaskStore terminal notification logic and user opt-in model handoff remain unchanged.
- Dedicated queued control verifies existing chat ownership. Live cancellation still uses the exact native worker owner and requires operator confirmation.

## Regression and CI plan

- Rust native tests: queue isolation, duplicate rejection, GPU budget denial, pause/resume/cancel, due/dependency gating, and fail-closed missing-plan recovery.
- Node source/desktop contract tests: scope, host ownership, resource reservations, UI controls, no path/argv persistence and existing terminal notice/ack.
- `.github/workflows/lu-tools-11-background.yml` explicitly runs P02 native Rust tests on Windows/Linux and P02 UI contracts. Existing native desktop host compilation remains a separate step. Exact-head CI green is required before claiming build-verification; installed NSIS and physical resource telemetry are independent gates.

## Remaining P02 work and explicit limits

1. **Native verification and repair**: exact-head Rust/Windows/Linux compilation, Node/React, authentic installed Windows worker, cancellation/restart/queue tests. Existing CI and installed-app gates are not yet claimed green.
2. **Universal scheduling adoption**: this scheduler is integrated into the first operator-launched media scan. Other existing host-native workers (e.g. Browser, OCR, Research, Observe, archive model-start) still need distinct authorized admission adapters, planned when those tool domains are advanced; they must not be described as governed by this scheduler already.
3. **Actual OS resource monitoring**: CPU pressure, physical available RAM, GPU VRAM/backend usage, disk/IO rate and network shaping require trustworthy platform probes and job-specific budgets. Current P02 limits are conservative reservations, not physical measurements. More resource policy can be shared with P03 model management and P05 transfer/download manager.
4. **Durable recurring rebind**: restart intentionally requires fresh host authorization. Cross-restart unattended recurring work requires an explicit, separate durable recurring grant with revocation and source identity validation; do not infer it from a historical job.
5. **Rich result artifacts**: terminal progress and state are durable and notified through the existing TaskStore, but a typed verified artifact manifest and richer dependency DAG UX are future enhancements; no false file-result claims.

**Execution boundary:** The owner instructed one Pxx area at a time. P02 is the current work only. Update its ledger entry after committing, and **stop before P03 — AI Operations Layer + model/provider manager** until explicitly requested.
