# PERF-01 P4 — Interactive-Priority Scheduling (2026-11-12)

**Scope:** Lu Core + original Tauri/React host only. **State: source changes committed; native/installed/physical acceptance NOT performed.** Repo `vjmichael/Lu-AI-GUI`, branch `lu/perf01-p4-interactive-priority-20261112`, stacked on `lu/perf01-p3-direct-streaming-20261112`. No PR, CI run, merge or release requested.

## Source implementation

1. **Preserve CPU-first deterministic routing:** The original P0D host-owned classifier in `lu_generate` still dispatches ANSF/SQLite literal search/count, native model status, current OS clock and approved workspace literal search to deterministic CPU paths, without reserving an inference generation or loading GGUF. This routing does not introduce model or tool authority. Already-existing specialist tools retain their separate one-use native authorization and OS custody.

2. **Host resource admission:** `native_background_scheduler::ResourceBudget::interactive_admission` consumes ONLY original OS-observed free RAM, measured CPU busyness, logical CPU count, model residency and pending foreground model turns. It reserves a foreground logical core and conservative RAM headroom while GGUF is resident; defers new background starts when a human turn is pending, on one logical CPU, with missing RAM, missing CPU measurement while model is resident, available RAM <1536 MiB, or observed CPU busy >=85%. Every outcome has a concrete policy reason. Limits apply to launch admission, not OS usage proofs.

3. **Unverified GPU/VRAM fail closed:** Scheduler background GPU and VRAM capacity remain exactly ZERO. An AMD Vulkan inference process may use the GPU, but no backend-specific trusted VRAM oracle has proven spare device memory; no GPU render/build is admitted from a fabricated free-VRAM value. Some workers such as Windows graphics capture may use the GPU despite a CPU/RAM-only reservation—this is an outstanding accounting gap, not proof they are GPU-free.

4. **Atomic guardian hold:** `BackgroundScheduler::apply_interactive_admission` updates CPU/RAM capacity and a shared `interactive_admission_hold` under one guardian mutex. New timed native workers and direct guardian-worker leases cannot be admitted while held. The original running processes, cancellation tokens, Job/cgroup custody, terminal event reconciliation and reviewed source hashes remain untouched. A hold is neither revocation nor an OS scheduler priority change.

5. **Just-in-time foreground recheck:** After the watchdog's first policy sample, the original scheduler polls and reconciles running workers. Before each new `start_host_approved` call, it rechecks the native model queue. This narrows the foreground-arrival TOCTOU window but does not establish OS-atomic priority against a simultaneously spawning worker; real contention tests remain required.

6. **Model continuation admission:** Host opt-in automatic model continuation from a finished background task also obeys the P4 pressure hold, before queuing inference. Normal explicit human Send remains independent of this gate.

7. **Latency-focused fairness:** Human generations overtake background continuations, with at most 2 background model tickets in an 8-slot queue. A just-queued background reply does not consume the fairness slot after 4 prior foreground turns; it first must wait at least 2 seconds. After that dwell and 4 consecutive human admissions, one background request can run. Active inference is never forcibly preempted. Host-only `PrioritySnapshot` exposes lane counts, class of active generation, original foreground streak and oldest background wait without leaking chat IDs/prompt text.

8. **Operator-visible decisions:** The existing Resource Guardian panel now shows native foreground/background model waiting counts, active class, last watchdog interactive admission reason, and whether new OS/background starts are deferred. These are host policy observations; renderer cannot use them to authorize an executable or modify scheduling.

9. **Repeatable measurement path:** Existing `scripts/perf01-host-baseline.mjs` accepts opt-in `--priority foreground-background`. It groups original `lu_perf01_recent` completed model receipts by trusted scheduler class and requires at least 5 original, exact-head samples of **each class** with host-observed queue wait and Tauri event-forward timing. Reports p50/p95 for queue wait, host first-forward and total latency; labels all comparisons NOT independently hardware-accepted. Missing groups/stale source SHA fail closed. No model prompt, private path or invented GPU/VRAM telemetry is reported.

## Source test evidence

- Rust tests added to `native_background_scheduler.rs`: measured pressure, unknown RAM/CPU fail-closed, one-core gate, high CPU pressure, insufficient RAM, zero GPU budgets, second foreground probe, and guardian hold refusing new lease while an existing lease retains original custody.
- Rust tests added/updated in `generation_queue.rs`: fresh background yields to arriving foreground even after a prior four-turn streak, aged background eventually runs, bounded queue and chat/session cancellation remain.
- Node contract tests added in `lu-desktop/tests/perf01-p4-interactive-priority.test.mjs` for CPU deterministic routing, zero invented VRAM, policy/guardian integration, scheduling fairness, operator UI.
- Baseline CLI test cases added to `lu-desktop/tests/perf01-baseline.test.mjs` for synthetic two-class p95, insufficient samples, stale SHA.
- **Actual execution this turn:** isolated JavaScript source-contract checks passed 5/5 before an additional guardian-hold assertion; the existing baseline script was parsed and run in an isolated JavaScript runtime with synthetic fixed receipts: two-class 5+5 reported appropriately, insufficient samples refused, stale SHA refused. These are synthetic/parser checks, NOT real GGUF/installed benchmarks or complete Node suite.
- **Not executed:** native Cargo/Rust compilation, Windows Job Object or Linux cgroup work, exact-source-head consolidated tests, genuine mixed-load GGUF/Radeon benchmark, UI smoke/security tests.

## Remaining gaps / acceptance

- Resource Guardian's policy holds NEW starts only. Already-running authorized builds/rendering/research can contend for CPU/GPU. OS-native priority demotion, affinity and VRAM telemetry are deliberately not claimed; they need verified native design + owner hardware acceptance before adoption.
- Guarded launch has a residual race if human enqueue occurs **after** the second foreground probe but **before** the OS start; no atomic OS scheduling guarantee is claimed. A brief startup window precedes the first 500 ms watchdog resource sample, so additional process-start checks are warranted at integration.
- Native GPU/VRAM capacity remains zero; backend-specific telemetry and physical Radeon contention must be measured before admitting GPU workloads. CPU/RAM reservation is estimated admission capacity, not measured physical consumption of each child.
- Background freshness and fairness thresholds are initial conservative policies, not proven optimal; compare repeated real same-hardware TTFT p50/p95, first-forward latency, CPU route latency, foreground queue delay, background starvation, worker throughput, memory pressure, cancellation and recovery. Keep one-use tool approvals and ANSF/SQLite source provenance intact.
- The old issue's P4 contains P0D CPU-first requirements; these are retained from earlier branches and must be reconfirmed in integrated native acceptance. Group B P2/P4/P5 should run one consolidated gate later, not per-commit CI.

**Disposition:** P4 source implementation assembled. May proceed to P5 source development provisionally; grouped native/security/installed performance acceptance remains OPEN.
