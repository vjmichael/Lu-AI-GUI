# PERF-01 P7L-1 — Exact-Head Native Compiler and OS-Child Acceptance

**Date:** November 12, 2026. **Repo:** `vjmichael/Lu-AI-GUI`. **Branch:** `lu/perf01-p7l1-native-repair-20261112`, based on `lu/perf01-p7l-koboldcpp-supervised-20261112`. **State:** native matrix staged, source benchmark defect repaired, no native execution evidence. **Do not close P7L-1 or mark the candidate admitted.**

## Verified original project status

At the P7L-1 starting source head `f1c1ddea3704267a6350f8b32a90e78a64b69a73`, GitHub Actions returned **zero workflow runs** for branch `lu/perf01-p7l-koboldcpp-supervised-20261112`; there were no actual Windows/Linux P7L native logs or jobs to reconcile. The current execution container has **no `cargo`, `rustc`, `rustfmt` or `rust-analyzer`**, and its GitHub network DNS is unavailable. The connected GitHub integration provides repository source reading and changes but **no workflow dispatch action**, so it cannot launch a new original native compiler job here. It would be false to claim that native compilation or OS child custody passed.

**User CI policy:** No routine PR/CI validation during incomplete task development. P7L-1 has no opened PR, no merge and no execution of the new exact-head gate. The new manual-only workflow requires separate owner admission when consolidated native verification is authorized. In GitHub Actions, a `workflow_dispatch` workflow normally must also be present on the default branch before it can be manually triggered; staging this file on a development branch alone does not prove that it is dispatchable.

## Implemented and repaired in this task

1. **Real A/B fairness defect found and repaired.** The original ignored P7L real comparison invoked `PersistentLlamaBackend` with `tuning:None`. On Windows that requests 99 GPU layers, whereas the KoboldCpp evaluation candidate is deliberately CPU-only. This would generate invalid apparent performance conclusions. `src-tauri/lu-core/tests/p7l_real_kobold_comparison.rs` now makes the owner-only original llama.cpp baseline use an explicit experimental `TuningProfile` requesting **zero GPU layers**, F16 cache, disabled speculation, 512/128 batching and 4 batch threads. The report truthfully records `requestedGpuLayersOnWindows:0`, `baselineRequestedBackend:CPU` and `baselineCpuVsKoboldCpuEquivalenceProven:false`. The exact pinned llama.cpp may reject the P5 experimental flags; that must produce a genuine native failure, not a fabricated measurement. No production backend configuration was changed.
2. **Bounded exact-head native gate staged.** New manual-only `.github/workflows/lu-perf01-p7l1-native.yml` verifies a supplied exact 40-hex SHA and that its checked-out original commit matches it; runs Windows Server 2022 + Ubuntu 24.04 native Rust 1.98; executes original Rust core unit tests for P7 admission, the actual KoboldCpp adapter, SQLite custody, real loopback SSE/tool framing/cancellation, and opt-in independently spawned native fake-Kobold child. Both platforms compile the ignored genuine Kobold-vs-llama A/B harness **without running it**. All tools/probes are test-only and do not install a different engine. The workflow has **`workflow_dispatch` only** (no push or PR triggers), so authoring it cannot launch an unwanted CI cycle.
3. **Native evidence design.** The manually admitted workflow records the exact source identity, OS, each compiler/test log via `tee`, and an explicit artifact saying `PHYSICAL_KOBOLD_ACCEPTED=0` and `BACKEND_PROMOTED=0`. It uses `set -euo pipefail` so failed native Rust commands cannot be counted as successes by successful logging. The material remains task-source code until the workflow has actually been run and its original GitHub jobs/logs inspected.
4. **Source-level gates.** New `lu-desktop/tests/perf01-p7l1-native-gate.test.mjs` checks no CI auto-trigger, original Windows/Linux matrix and exact source SHA, CPU-only baseline, no imaginary throughput, real native fixture wiring, and the continued `packaged:false/selectable:false` alternative. **Thirteen source-contract checks were executed and passed in an isolated V8 JS evaluator against exact fetched P7 + P7L + P7L-1 sources**: five P7, five P7L and three P7L-1 checks. These are source assertions, not `node --test` in the original repo, native process tests or physical acceptance.
5. **No runtime change to installed Lu.** `lu_model_select_backend` still only permits its existing source-pinned llama.cpp CPU/Vulkan choices and the original `PersistentLlamaBackend`; KoboldCpp remains an opt-in `lu-core` test-evaluation component and is not installed, downloaded or selectable.

## Native test matrix prepared — NOT executed

| Gate | Exact original test command in staged workflow | Status |
|---|---|---|
| P7 original candidate preflight | `cargo test --locked --manifest-path src-tauri/lu-core/Cargo.toml --lib p7_backend_evaluation::tests` | Not run |
| P7L SQLite custody and restart | `cargo test --locked --manifest-path src-tauri/lu-core/Cargo.toml --lib p7_kobold_custody::tests` | Not run |
| P7L authentication / backend unit tests | `cargo test --locked --manifest-path src-tauri/lu-core/Cargo.toml --lib p7_koboldcpp_backend::tests` | Not run |
| Real loopback parsed SSE/tool/cancel fixtures | `cargo test --locked --manifest-path src-tauri/lu-core/Cargo.toml --test p7l_kobold_http_protocol` | Not run |
| Actual OS child / Job + group and SQLite test fixture | `cargo test --locked --manifest-path src-tauri/lu-core/Cargo.toml --features p7l-native-fixture --test p7l_kobold_process_custody` | Not run |
| Real Kobold + llama GGUF A/B harness compilation | `cargo test --locked --manifest-path src-tauri/lu-core/Cargo.toml --test p7l_real_kobold_comparison --no-run` | Not run |
| Original P7 local synthetic candidate protocol probe compilation | `cargo test --locked --manifest-path src-tauri/lu-core/Cargo.toml --test perf01_p7_candidate_protocol_real --no-run` | Not run |

The real hardware tests are intentionally `#[ignore]`; they still require independent original executable, library and GGUF/MMProj SHA-256, host-generated local keys, actual OS custody, and owner-reviewed test evidence. Staged compiler tests can reveal code or mock-process failures but **cannot** by themselves prove the genuine KoboldCpp engine works, has equivalent vision/tool behavior, is contained after guardian death or beats llama.cpp.

## Pending next work

- **P7L-1A exact-head native results:** Once the owner allows the grouped compiler gate, admit one exact SHA on Windows/Linux, fetch original workflow jobs/step logs, fix any actual compiler/test errors and rerun only after commits are reconciled. P7L-1 stays OPEN until both native OS suites actually pass. No guessed PASS results.
- **P7L-1B OS custody acceptance:** After native fixture tests, challenge Windows Job kill-on-close and Linux process group/cgroup containment under interrupted guardian, OS restart and escaped descendants; verify original child PID/process-tree and durable journal `REAPED` independently.
- **P7L-2–4 real Kobold support:** Pin original Kobold executable and dependencies; test real Qwen/Gemma GGUF, MMProj, streaming and one-use tool calls, cancellation/reload; only then run identical CPU-only and optional verified Radeon performance benchmarks. Confirm real HTTP framing and loader flags against the pinned Kobold version.
- **P7L-5 promotion:** Security and Windows installed acceptance remain a separate release gate. **No alternative engine may replace llama.cpp** until these tests and user approval pass.

**Latest run check:** Both the parent P7L branch at task start and the new P7L-1 branch after the code changes returned **0 GitHub Actions workflow runs**. Thus there are no original compiler logs or native result artifacts to inspect yet. The workflow file is intentionally manual-only.

**Decision:** P7L-1 source checks/benchmark repair are complete; **native compilation and installed process acceptance remain blocked/not executed**. Do not silently convert this task to complete, infer native results, dispatch CI under an old SHA, or release an alternative runtime.

## Continued P7L-1 source repair — CPU thread parity

An additional benchmark fairness defect was identified: the candidate honors `LU_P7L_THREADS` (1–64, default 4), while the baseline previously always used four inference threads. The evaluation-only `llama()` constructor now reads the same bounded owner setting. A P7L-1 source regression asserts both engine settings remain coupled. This changes no installed Lu configuration and does not claim effective runtime CPU-thread parity without native logs.

Exact fetched-source assertions verified the candidate setting, baseline setting, zero GPU layers, and both regression assertions (six focused checks). These checks are source-only; neither original `node --test` nor Cargo native execution was performed in this continuation. The pinned llama.cpp batch-thread setting remains four, and scheduling/thread effectiveness across engines is still unproven.

**Still OPEN:** no demonstrated Windows/Linux native test results, OS child containment evidence, or real Kobold GGUF/MMProj evaluation. The manual-only workflow may not be dispatchable while absent from the default branch. Do not infer a pass or promote the candidate.

## Execution attempt and custody-evidence blockers — follow-up

The requested exact-head native matrix was attempted from the available execution environment at source SHA `dafbca0cf1d39aa11d95e4125a76bdcca5a89121`. The environment has Node 22 and Git but no `cargo`, `rustc` or `gh`; a real `git ls-remote https://github.com/vjmichael/Lu-AI-GUI.git HEAD` failed with `Could not resolve host: github.com`. The connected GitHub API supplied source files and returned zero PR-triggered workflow runs for this SHA, but exposes no workflow-dispatch action. **No native compiler command or fixture test executed, and no native failure logs were produced.** This is an infrastructure block, not a compiler pass or a compiler failure.

Independent physical custody proof remains outstanding. The currently staged mock tests verify parent-child cancellation/reap and SQLite session logic; they do not inject guardian death, verify Linux escaped descendants/cgroup-v2 custody, or independently inspect Windows Job Object membership/kill-on-close and surviving descendants. Source inspection alone cannot establish these outcomes. Do not clear the journal solely because the direct child exited: descendants must be independently reconciled before the lane is promoted.

When an authorized Windows/Linux native runner is available, admit the **then-current, reviewed exact 40-hex source head** and run the manually gated matrix (first ensure its workflow is dispatchable without merging unreviewed changes). Fetch original native job logs and artifacts, repair demonstrated compiler/fixture failures, then run separate guardian-death and process-tree adversarial evidence on each OS. Maintain `PHYSICAL_KOBOLD_ACCEPTED=0` and `BACKEND_PROMOTED=0` until verified.

## P7L-1 custody source repair continuation (November 12, 2026)

- **Execution blocker reconfirmed:** sandbox has Node + Git, but no Cargo/Rust compiler or GitHub network DNS. Connected GitHub actions can read original source/runs but cannot dispatch `workflow_dispatch`; zero pull-request-triggered runs were returned for the prior `6a4b17eb...` head. **No native compiler/fixture tests ran and there are no original error logs to inspect.**
- **Windows admission repair:** P7L now launches the experimental child with `CREATE_SUSPENDED` in addition to `CREATE_NO_WINDOW`; existing `lu-win-worker-job::Job::assign_and_resume_suspended` admits it before resuming. Cleanup uses `terminate_and_verify_empty()` on the original assigned Job; failure quarantines the journal rather than treating the leader's `wait()` as complete Job-tree evidence. The existing first-party Win32 FFI implements a kernel ActiveProcesses==0 query.
- **Linux group cleanup repair:** after signalling the original in-memory process group and waiting on its leader, P7L now makes a bounded `kill(-pgid,0)` emptiness check; any remaining group members, an inconclusive kernel result or a timeout keeps the journal pending. This **does not** prove containment of a `setsid` descendant or of a process that escaped the group. Cgroup-v2 guardian-death verification remains a separate gate.
- **Source regression:** the P7L-1 contract asserts suspended Job admission, native Job-empty verification and Linux group-empty checking. Nine focused fetched-source checks passed through the connected code evaluator. No `node --test` or Cargo result is claimed.
- **Risk and acceptance:** behavior is intentionally fail closed; a Linux zombie/group that cannot be proved absent may leave P7L quarantined. Genuine Windows Job/SCM kernel integration, Linux cgroup-v2 escalation, killed-guardian/restart cases, and real native compilation **are OPEN**. The experimental backend stays unselectable and unpromoted.
