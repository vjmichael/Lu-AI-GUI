# Lu K/I/A/F — Active execution roadmap and evidence ledger

**Owner direction:** November 8, 2026. **Status:** binding current prioritization, NOT a production acceptance claim. Based on [canonical K/I/A/F](LU_AI_EXECUTION_LAYER_PHILOSOPHY_20261107.md), [memory-first user-centered policy](LU_HUMAN_CENTERED_AI_MEMORY_FIRST_20261107.md), [tool scope audit](LU_AI_EXECUTION_TOOL_REVIEW_20261107.md), and [v1.0 task ledger](LU_V1_0_TASKS.md).

## Policy: model intelligence, Lu execution, user control

Practical AI Capability = f(**K**: the selected model's knowledge/reasoning, **I**: actual available execution/observation interfaces, **A**: authenticated human-governed authority for the exact task/effect/resource, **F**: observed outcomes, independent verification, feedback and crash recovery). The user supplies the actual goal; Lu's ANSF-backed memory, provenance and clarification continuity allow the model to reason without repetitive recaps. This conceptual model is not a numerical score.

**Exactly three categories:** **Foundational** (general observation, filesystem, model/worker interfaces, authorization, memory, scheduling, execution, verification/recovery); **Utility** (small deterministic, justified shortcuts such as archives, basic image/format conversion, JSON/Protobuf, Git inspection); **Specialist** (mature full Office/LibreOffice, Zotero, graphics suites, IDE/NVP/Cognitum, operated through reviewed existing applications/DLCs, not recreated inside Lu Core).

**Owner decision:** Existing P01–P12 utility breadth is reasonable at this stage. Freeze speculative per-domain mini-app expansion; preserve useful bounded utilities. Improve real model-neutral execution I, exact A, independently verified F, user intent/memory continuity and genuine K benchmarks. Add a new primitive only for a measured gap existing utility/app interfaces cannot solve. No mandatory MCP, Docker or Lu-wide sandbox; OS worker custody where needed is not an assistant-wide access sandbox.

**Evidence is staged:** source candidate ≠ Windows/Linux native pass ≠ genuine GGUF-selected OS work ≠ installed Windows release acceptance. Never promote queued, cancelled or fixture-only CI to proof of end-to-end task success.

## GROUP 1 — Still needing implementation finished (do not stop on queued CI)

| ID | Priority | Missing implementation / completion definition |
|---|---|---|
| IMP-01 AGENT-01F | P0 | Explicit loaded GGUF normal-chat **Draft plan** opt-in, bounded strict JSON step proposal, original human/ANSF goal check before and after inference, saved-memory context, deterministic two-phase draft identity, persisted empty scheduler/witness schema for read-only UI, visible rejection/progress; source candidate. Finish model-output quality, host profile concurrency, genuine device/runtime and installed E2E acceptance. No model-origin tool grant. **Never** infer effect authorization from a draft. |
| IMP-02 MEMORY-LOOP | P0 | Make original source-backed ANSF retrieval, belief revision, operational/procedural memory, human clarification and task continuity work together in actual model/resumed task turns; memory does not confer permission. |
| IMP-03 EXEC-INTERFACE | P0 | Enable actual loaded GGUF tool selection and real reviewed installed Git/FFprobe/app operations through the already designed host native execution broker. Don't clone specialist apps. |
| IMP-04 EXEC-AUTH | P0 | Bind active plan steps to fresh explicit human-origin task, resource/effect, credentials/egress scopes and revocation; implement actual broker dispatch separately from currently non-dispatching AGENT-01E preflight. |
| IMP-05 EXEC-LIFECYCLE | P0 | Real OS process cancellation/reap, UNCERTAIN effect reconciliation, kernel cgroup-v2 descendant escape protection where promised, Windows Job Object child custody and crash stop guarantees. |
| IMP-06 EXEC-FEEDBACK | P0 | Independent output/source oracles, method-specific VERIFIED/UNSUPPORTED/CONFLICT and source revision checks; typed feedback to model/human that cannot override worker journals. |
| IMP-07 TASK-LOOP | P0 | Full plan/clarify/approve→execute→verify→feedback→recover, with model reasoning in K, user and host in A, actual I and independently witnessed F; never auto-replay an uncertain effect. |
| IMP-08 KIAF-MEASURE | P1 | Prospective real-GGUF capability tests: plan-only vs approved execution vs bounded utility; record failure origin, task correctness, context/tokens, latency, CPU/GPU and memory. |
| IMP-09 UX-FOLLOWUP | P1 | Rich thread navigation and user-initiated new authenticated model turn from an edited message are separate; display edits preserve original ANSF history and never replay tools. |
| IMP-10 RELEASE-SHELL | P1 | GGUF loading responsiveness, actual measured progress, image restoration, accessible UI and supported installation hardening. |

## GROUP 2 — Waiting on GH operations, verification, independent acceptance

| Gate | What has to be observed (not merely implemented) |
|---|---|
| GATE-01 REPO/CI | Exact-head source lock plus Windows/Linux Rust/SQLite/ANSF and React/Tauri build/test. Diagnose and repair actual failed jobs. |
| GATE-02 E1-P03–P07 | Genuine Windows/Linux supervised Git/FFprobe operations, original authorization, process cancellation, inner/outer journal, model tool-choice as separate real-GGUF gate. |
| GATE-03 E1-P08C–P13 | Linux cgroup-v2 escape/host-crash containment, Windows Job Object, trusted source freshness and independent Git/MP4/FFprobe verification, prior P09/P10 actual regressions. |
| GATE-04 E1-P15/AGENT | Actual ANSF/SQLite original-task planning, human review, recovery timeline, no duplicate effect, host ownership and eventual live broker/step lifecycle proof. |
| GATE-05 UX-CHAT | UX-CHAT01/02 original identity, edits, replies, startup reconciliation and native/React acceptance; installed WebView deferred. |
| GATE-06 MEMORY | M2–M5 native source migration, restart, retrieval/procedural memory and genuine three-arm GGUF benchmark remeasurement. |
| GATE-07 KIAF BENCH | Same model/quantization, repeatable authorized tasks, independent result witnesses and K/I/A/F vs environment failure attribution. |
| GATE-08 WIN-RELEASE | Near v1.0 physical installed NSIS/Win10/11 WebView, GGUF/MMProj AMD Vulkan, approved tools, worker kill/restart, uninstall. Deferred in **time**, never waived. |
| GATE-09 SIGN/LEGAL | Complete shipped dependency inventory, independent redistribution/legal check and actual release-signing identity decision at end. |

## Current work order

1. Preserve memory-first goal/clarification/source continuity as the grounding for real agent work, while native E1 tests continue.
2. Stage AGENT-01F real host-only plan draft admission; finish model/normal-chat ingress next. Existing AGENT-01A–E plan/revision/receipt/scheduler/review sources are drafted but non-dispatching.
3. Connect the existing supervised reviewed native execution path only after separate E1 real OS authority, output and lifecycle guarantees are established. No new full-feature tool mini-apps.
4. Process GitHub's completed exact-head results when available and repair the failures. Don't block source implementation solely because jobs are queued.
5. Benchmark genuine selected GGUF tasks and independent outcomes; defer routine physical installed-Windows tests until near v1.0.

**Earlier source/evidence snapshot at start of this work:** parent UX-CHAT02 PR #158 head `4e6ac21f915095e2d033cd297a1169c1a7a0240e`; all 48 associated workflow runs queued/pending, zero completed at inspection. Earlier shared E1 Git-pack/MP4 Rust compiler bugs and an obsolete UI assertion were source-repaired on that branch; native pass has not been observed. This AGENT-01F branch is an additional **unaccepted** candidate and must carry its own exact-head CI receipts.

## 2026-11-08 — AGENT-01F stage 2, actual loaded GGUF plan proposal ingress

- **Source:** PR #159 stacked on UX-CHAT02 PR #158. The ordinary `lu_generate` path accepts an explicit one-turn `planOnly` boolean from a mutually exclusive GUI **Draft plan with Lu** control. The loaded model is genuinely asked to propose a narrow JSON step list; there is no second fake planning model or new app/tool. The existing source-backed ANSF goal/clarifications and bounded indexed memory remain in the actual model context.
- **K:** model proposes 1–8 ordered `PlanStepKind`/`ExpectedDeliverable` steps and dependencies. Unknown/duplicate JSON keys, extraneous commands/tools, malformed graph, unsupported types, empty/fenced/prose output fail admission without tool execution.
- **A:** host pins the exact current original authenticated user turn and ANSF SetGoal event before inference; verifies both and pending/deferred clarifications again before the model proposal enters AGENT-01A/B. Model cannot supply IDs/permissions. Separate human native-dialog approval remains necessary. Only the exact same source turn+graph is idempotently restaged.
- **I:** an admitted plan remains an immutable `DRAFT`, *not* a reviewed OS worker execution request. Initialize EMPTY AGENT-01C/01D result/scheduler schema so existing read-only plan-history UI can show it without inventing a worker/effect. The existing actual reviewed-app broker preflight now rejects plan kinds that are presentation/human decisions rather than inspect/source/independent witness operations. It still returns `WAITING_FOR_INDEPENDENT_NATIVE_OS_ACCEPTANCE`, not a live dispatch.
- **F:** acceptance will require native ANSF/SQLite restart, source-conflict and negative JSON tests, React/WebView UI, real local GGUF plan response, actual approved plan→fresh human step→registered broker→real app worker→independent result/witness, cancellation, uncertainty and restart. The last chain is **STILL MISSING**. We have not executed GGUF or Git/FFprobe in this turn.
- **Tests submitted:** 7 portable native production-ChatStore/SQLite plan admission tests, 2 native reviewed-broker guard tests, 4 Node static bridge contracts, plus full Windows Tauri check. On this branch tests are **submitted, not passed**, pending exact-head job logs.
- **Next implementation:** integrate a *new*, independently authorized, exact step claim with `ReviewedApplicationTurnSession::preflight_scheduled_step` and actual `run_journaled_observed` only after E1 worker/custody/independent verification gates; bind result to original AGENT-01C independent witness rather than accept the model answer as success. Keep model planning a proposal-only stage if these gates are not green.

[AGENT-01F detailed design and receipts](LU_AGENT01F_MODEL_PLAN_INGRESS_20261108.md).

## 2026-11-08 — AGENT-01F stage 3: fresh execution-turn independent outcome custody

- **Corrected a concrete K/I/A/F evidence mismatch:** AGENT-01A's original planning user turn is NOT AGENT-01D's fresh per-step executable user turn. Old AGENT-01C code read the original plan turn's `lu-reviewed-{task,effects}-{turn}.sqlite` journals; genuine reviewed Git/FFprobe effects are stored by the newer execution turn. A plan approval cannot substitute for that fresh human/host-owned operation identity.
- **Source integration:** add `AgentDeliverableReceipts::record_verified_execution_turn` with original plan turn **and** actual distinct authenticated execution turn, exact in-process AGENT-01D owner/claim/revision/step check, independent E1-P15 `lu_reviewed_turn_origin` original chat+fresh human turn singleton, actual inner/outer APPLIED journals, completed per-worker TaskStore event, independent `VERIFIED` oracle and receipt SHA checks. Preserve original plan association and immutable verification result across restart; refuse unrelated/wrong/expired claims and altered/missing evidence.
- **Schema:** additive nullable `execution_turn_id` column in AGENT-01C history, upgraded in place for historic receipts. Legacy NULL only means the **original turn** used by earlier tests, never a fictional new worker. Read-only re-verification uses the recorded execution turn; changed/absent worker or independent witness becomes `NEEDS_RECONCILIATION`, not success.
- **Broker:** add `link_verified_scheduled_worker_outcome` to consume the already authorized, actual reviewed-worker effect ID. It checks requested reviewed operation suffix and fresh BoundCallOrigin; after AGENT-01C proof, AGENT-01D marks the exact claim `OBSERVED_VERIFIED_HISTORICAL`, and the broker returns the independently checked historical row. It does **not** invoke an OS executable or give permission. A model-generated statement cannot manufacture a worker call ID, task journal or independent oracle.
- **Native regression source:** fresh-vs-original turn SQLite/journal distinction, missing E1-P15 singleton negative, wrong fresh claim, verified actual effect custody, immutable original association/restart, legacy schema migration; C workflow expected 7 Windows/8 Ubuntu tests and broker workflow 3 tests. **These are submitted source candidates and NOT independently executed native acceptance.**
- **Still unfinished implementation:** user-reviewed scheduled step activation/OS picker integration and live `ReviewedApplicationTurnSession::preflight_scheduled_step` → `run_journaled_observed` handoff; real GGUF chooses a reviewed operation; actual OS process cancellation/cgroup-v2/Job Object evidence; independent source freshness, real installed app; robust concurrency and inter-journal crash reconciliation. Do not claim an autonomous agent runner or verified task success until these finish.
