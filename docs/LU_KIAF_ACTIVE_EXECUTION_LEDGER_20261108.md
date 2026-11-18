# Lu K/I/A/F — Active execution roadmap and evidence ledger

**Owner direction:** November 8, 2026. **Status:** binding current prioritization, NOT a production acceptance claim. Based on [canonical K/I/A/F](LU_AI_EXECUTION_LAYER_PHILOSOPHY_20261107.md), [memory-first user-centered policy](LU_HUMAN_CENTERED_AI_MEMORY_FIRST_20261107.md), [tool scope audit](LU_AI_EXECUTION_TOOL_REVIEW_20261107.md), and [v1.0 task ledger](LU_V1_0_TASKS.md).

## November 11, 2026 — P0 codebase source reconciliation and safety repairs (source candidate)

**Seven stacked P0 commits / 41 changed files** were inspected against the original shared Rust/React native API boundaries. Exact Git tree/source lock reconciliation checked all **941** tracked entries with no blob/hash omissions or untracked paths. Cross-checked current Tauri handler registration and P0 normal-chat/task-board invocations; original ANSF, Reader, reviewed worker, task register, Browser/Observe and recovery contracts remain separate source-backed pathways. This is **source static reconciliation, not native compilation**.

**Actual repairs:** corrected a Rust cancellation-test name shadow that would prevent test compilation; native P0-AUTH Stop now names the exact original ANSF chat title and ID in the OS confirmation (not merely a generic "selected conversation" label); ANSF task-memory scan deduplicates primary/current chat before its 24-conversation cap; source-history retrieval saturation is now marked incomplete even when private OCR/Observe rows hide older Browser/Reader history; and the GGUF continuation projection now requires human review whenever original goal revisions, source samples, checkpoint IDs, native receipts or task-criterion/oracle histories are missing, omitted, stale or unavailable. Existing recorded effect receipts and authority remain unchanged.

**Acceptance status:** no Rust/Cargo compiler, frontend build, installed Windows/Linux, genuine loaded GGUF, CI, PR verification or regression testing was executed. This environment has GitHub source access but no mounted Lu checkout or native Rust toolchain. The source branch remains unmerged; further demonstrated compiler/native failures must be repaired during the separately approved consolidated gate, not imagined as passing. See [P0 source reconciliation handoff](LU_P0_SOURCE_RECONCILIATION_REPAIRS_20261111.md).

## November 11, 2026 — P0-AUTH authority launch/revocation fence and native scoped recovery (source candidate)

**Implemented source-only:** Added `ConversationAuthority::with_authorized_worker_launch`, a bounded synchronous READ+EXECUTE host permission fence, and integrated it in the genuine Git, FFprobe and PDFInfo/ExifTool broker launch paths. It closes the revoke-before-registration window: independently checked grants cannot be revoked between native OS process start and registering the exact worker with the existing synchronous cancellation callback. No lock is retained during subprocess runtime, inference or human interaction. Original task/effect journals, native Windows/Linux custody, independent postcondition oracles and per-turn authorization remain authoritative. Source-level race, revoked-task and cross-scope tests written, not run.

**Unified operator custody:** Native Tool Activity now exposes a scoped P0-AUTH posture and separately confirmed **Stop only this conversation's work** control. The native host derives actual Browser grant, Observe session, Reader/model generation, offline OCR flag, scheduled and unscheduled native worker ownership from original host registries—never from a model-selected PID/path/authorization. It attempts every eligible owned stop, records uncertain/queued/revoked/OS-stop outcomes separately, and never promises that a cancellation request reaped all descendants. A new strictly typed host-only append-only cancellation intent+result table lives in the **existing ANSF Library** (not a new DB): host REQUESTED is persisted before stops; a missing RESULT after interruption is **UNKNOWN**, not success or automatic replay. Original effect and OS custody sources remain independently necessary. No prior grant, follow-up effect or task completion is inferred.

**Remaining gaps acknowledged:** No single magic execution lock was introduced across all existing file/browser/observe families; their original independently constrained approval and custody paths remain. Real installed Windows/Linux Job/cgroup evidence, source mutation, actual loaded-GGUF consent/refusal/cancel and consolidated native compiler, CI/PR and regression gates remain DEFERRED by owner. See [P0-AUTH handoff](LU_P0_AUTH_UNIFIED_NATIVE_CUSTODY_RECOVERY_20261111.md).

**Next:** implementation-first P0 source groups are present as source candidates. P0-GATE codebase-wide reconciliation and native acceptance only when the owner authorizes the later batch.

## November 11, 2026 — P0-MEMORY original cross-session ANSF reasoning and corrections (source candidate)

P0-MEMORY replaces the old isolated checkpoint-only normal-chat resume with ONE bounded original-ANSF context for an authenticated current user request naming a project/task. It reopens the latest profile-owned cross-conversation checkpoint and original source IDs, current/older CCM comparisons, original goal and answered/deferred questions, independently checked historical effect receipt references, same-chat historical Browser/Reader source digests (not current facts or task acceptance), and P0-TOOLS-05 native human criterion decisions where an exact register/task selector exists. For prior task evidence, the original worker/oracle is reopened when its register lease matches; no old decision becomes task completion.

A new source-revalidating ANSF task-wide belief scan reads the original SQLite books across up to 24 same-profile conversations, including the original checkpoint and current chat; it distinguishes independently witnessed at-observation claims, unverified reports, retractions/supersessions, conflicting values, stale sources and omitted older conversations. Contradictions are scoped to the same exact subject+predicate, and no newer model statement wins a conflict. Retracted/superseded beliefs also recheck their **original later human correction message** after restart; edited/missing/ambiguous corrections become STALE, never restore an older verified claim. The entire projection stays read-only, bounded, explicit, privacy-filtered (no OCR transcript or Observe metadata), no tool grant, no automatic retry, no task-state change and no extra database.

**Status:** implementation source-only; original native compiler, loaded GGUF, physical Windows/Linux installed app, CI/PR and regression acceptance deliberately deferred by owner. See [P0-MEMORY handoff](LU_P0_MEMORY_ORIGINAL_CROSS_SESSION_CONTINUITY_20261111.md). Next: P0-AUTH unified authorization, OS custody and uncertainty recovery; late batch acceptance after building the remaining app.

## November 11, 2026 — P0-TOOLS-05 reviewed independent witness to exact task criterion (source candidate)

Connected P0-Next's original human-approved project-task/agent-plan provenance and AGENT-01C independently reopened **original native worker, effect and oracle receipts** to a new human criterion-evidence decision in the existing ANSF Library. The task-board operator chooses one independently verified eligible Git/media witness and an exact decision (accepted as *supporting evidence*, rejected as irrelevant, or more evidence needed), supplies a written reason, reselects the genuine project folder via native OS picker, undergoes full source graph/register/board recheck, and gives a second native approval. The host reopens the original oracle evidence after approval, then records one version- and criterion-SHA-bound immutable audit event. Exact original receipt identity, independent method, observed hash, current task version, register lease and native-human decision are all pinned. A source/plan change invalidates current eligibility; old reviews remain historical.

**No automatic task completion, task-register status change, new execution permission, next agent step or source re-import.** Existing PLAN-07 exact requirement human acceptance remains independent. Standalone P0-TOOLS-04 PDFInfo/ExifTool are original reviewed receipts but are not retroactively mapped into already approved Git/media agent plans; future plan review and specialist method-specific criteria are required. See [P0-TOOLS-05 handoff](LU_P0_TOOLS05_TASK_CRITERION_WITNESS_REVIEW_20261111.md).

**Status:** Source-only implementation. Compiler/native runtime/installed/GGUF/CI/PR/regression acceptance intentionally deferred by owner, not claimed passing. Next groups: P0-MEMORY, then P0-AUTH; late consolidated acceptance after remaining implementation.

## November 11, 2026 — P0-TOOLS-04 new protected specialist utility adapters (source candidate)

P0-TOOLS-04 adds **real external PDFInfo and ExifTool** to the existing original-user authenticated, one-use, ModelToolCatalog/NativeToolRegistry reviewed application path. A native OS file picker and a second exact-source/executable SHA-256 approval produce only one bounded read-only task. The existing OS supervisor now admits exactly two additional hardcoded argument forms, one for PDF page/version metadata and one for PNG/JPEG type/dimensions. It enforces a 15-second worker limit, 8192-byte stdout bound, actual source/file digest and ordinary cancellation/Job Object/Linux process custody. Each typed completed worker effect is independently checked against original source bytes using a *different parser* (classic PDF page tree, image dimensions) and journalled as VERIFIED, CONFLICT, or UNSUPPORTED. Model text cannot supply a path, command, script, shell, new grant or future authorization.

Host normal Send conservatively recognizes explicit PDF/image metadata questions and enters the native review path. Existing agent PLAN step methods remain Git/FFprobe only until a **new** approved revision and explicit criterion-to-method evidence mapping. No new application mini-app, unrestricted external process runner or networking capability was introduced. Original TaskStore/EffectJournal receipts are preserved; the existing Git/media ANSF reviewed-memory adapter is not falsely told a PDF/Image operation is Git.

**Status:** committed source candidate only. No compiler, real GGUF, native installed, CI, PR verification or regression run was started, per owner. Next P0-TOOLS-05 source-rechecked task criteria and independent witness handoff. See [P0-TOOLS-04 engineering handoff](LU_P0_TOOLS04_SPECIALIST_ADAPTERS_20261111.md).

## November 11, 2026 — P0-TOOLS-03 original ANSF native-source continuity (source candidate)

**Development-first, no CI/PR work.** Closed a real integration gap between previously existing Browser, Reader, OCR and Observe sources and the model/ANSF history plane. The original ANSF Library SQLite gains a typed, integrity-hashed, read-only-to-model native-source index for current-chat source IDs, actual hashes, provenance and privacy. No new memory DB, arbitrary source URL tool, automatic observation or authority.

- Browser: real scoped post-redirect READ/ACCESSIBILITY receipts from the independent Research ledger, preserving the actual public source URL, observed text digest and bounded UNTRUSTED excerpt. Citation verification remains separately enforced.
- Reader: genuine original model tool Q&A and P0-TOOLS-01 combined workspace native Reader model calls now publish their actual opened document-byte SHA and sanitized basename. The workspace tool uses a host-only callback executed ONLY after authorized native read/parse success, not after model prose or directory metadata discovery. No real OS path is stored.
- OCR: separately approved, pinned offline image/region/PDF-page OCR now publishes source+text fingerprints ONLY; no OCR transcripts, original pixels, PDF paths or fabricated ANSF human turns. Human review remains mandatory.
- Observe: a separate native confirmation saves ONLY private, enum-only handoff SHA metadata with no model/network access, independent test verdict or screen data. Forget notes first deletes this private ANSF projection, then the diagnostic notes.
- Model: a current authenticated human requesting earlier sources can receive a bounded historical Browser/Reader ANSF source context, clearly untrusted/stale/no grant. Private OCR and Observe never enter local model source context; no network source transfer is authorized. A read-only desktop source-history view exposes original receipts.

**Status:** Source implementation only. Native compiler/real-GGUF/Windows/Linux installed/regression acceptance deliberately not run. Original task/research/effect journals retain authority; the ANSF index is only a provenance projection. See [P0-TOOLS-03 handoff](LU_P0_TOOLS03_ANSF_NATIVE_SOURCE_PROVENANCE_20261111.md). Next: P0-TOOLS-04 protected specialist application adapters, cancellation and semantic feedback.

## November 11, 2026 — P0-TOOLS-02 typed model-to-effect native review handoff (source candidate)

The existing Lu desktop already had actual separate, journaled write/edit/copy/move/reversible-trash/archive implementations. The outstanding *interface* gap was that \`propose_native_action\` lacked whole-file text creation/editing, requiring a separate model-output format. P0-TOOLS-02 adds two typed, completed-model native review actions (\`create_text_file\` and \`edit_text_file\`) and bounded full-content \`textFileDraft\`, with conservative original-human intent matching, one-use chat/generation identity and no OS path/model grant. A trusted host-only bridge checks that the original ANSF human turn is still the latest, then stages the existing WriteProposalQueue/EditProposalQueue and durable approval ledgers. The operator still selects the real resource with the OS picker, reviews full bytes/hash/diff and approves in an independent native dialog; existing effect journal, receipts, cancellation and manual uncertainty recovery own the actual file operation. Prior copy/move/trash/archives stay on their preexisting reviewed operation paths.

**State: source only; CI/PR/native compiler/regression and physical installed acceptance deferred per owner.** No new arbitrary OS command executor, edit replay or automatic project task completion. See [P0-TOOLS-02 engineering handoff](LU_P0_TOOLS02_REVIEWED_FILE_EFFECTS_20261111.md). Next model interface group: P0-TOOLS-03 browser/Reader/OCR/Observe source continuity.

## November 11, 2026 — P0-TOOLS-01 source integration (acceptance deferred)

**Owner workflow:** finish Lu implementation and integration first. GitHub is a source repository, not a per-task CI/PR verification gate. No additional CI, PR checking or regression campaign until application development is complete under the owner's NVP-directed hold.

**Source work:** linked the existing native Reader's Protobuf schema and real PDF/Office/plain-text extraction into the *same* live read-only workspace GGUF turn as source discovery, directory read, ZIP manifests and artifact inspection. Added conservative multi-document folder routing, genuine source SHA continuity, nested-link refusal and current-turn Reader ranking. This improves Lu I (interfaces), not A (authority), with no new model-controlled root, write, process or network permission. **Not a new app or fake tool.** See [P0-TOOLS-01 handoff](LU_P0_TOOLS01_MULTI_DOCUMENT_MODEL_LOOP_20261111.md).

**Status:** source implementation only. Compiler/native/installed/real-model acceptance not executed and not claimed. **Next:** P0-TOOLS-02 typed model-proposed, operator-approved actual filesystem effects and durable independent verification; do not auto-complete PLAN-05 tasks or inherit any prior worker grant.

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
| IMP-03 EXEC-INTERFACE | P0 | Source candidate now wires ONE manually requested, approved independent-witness step through the real reviewed Git/FFprobe model/tool/OS worker path, with native picker and independent result linkage. Finish actual GGUF native acceptance, robust multi-step planning, other specialist app adapters; do not clone full applications. |
| IMP-04 EXEC-AUTH | P0 | Source candidate implements a fresh native-confirmed human turn + host-owned AGENT-01D claim + exact reviewed Git/FFprobe capability + current ANSF goal/clarification and resource; finish native acceptance, crash/uncertainty reconciliation and wider authorized operation coverage. Plan approval alone never dispatches. |
| IMP-05 EXEC-LIFECYCLE | P0 | Real OS process cancellation/reap, UNCERTAIN effect reconciliation, kernel cgroup-v2 descendant escape protection where promised, Windows Job Object child custody and crash stop guarantees. |
| IMP-06 EXEC-FEEDBACK | P0 | Independent output/source oracles, method-specific VERIFIED/UNSUPPORTED/CONFLICT and source revision checks; typed feedback to model/human that cannot override worker journals. |
| IMP-07 TASK-LOOP | P0 | One-step independent-witness reviewed-app loop now exists in source: model plan → human plan approval → separate source selection and fresh human step confirmation → supervised reviewed app model/tool turn → independent receipt → host note/recovery; actual OS/GGUF evidence, multi-step execution and safe continuation still missing. Never auto-replay uncertain effects. |
| IMP-08 KIAF-MEASURE | P1 | Prospective real-GGUF capability tests: plan-only vs approved execution vs bounded utility; record failure origin, task correctness, context/tokens, latency, CPU/GPU and memory. |
| IMP-09 UX-FOLLOWUP | P1 | Rich thread navigation and user-initiated new authenticated model turn from an edited message are separate; display edits preserve original ANSF history and never replay tools. |
| IMP-10 RELEASE-SHELL | P1 | GGUF loading responsiveness, actual measured progress, image restoration, accessible UI and supported installation hardening. |

## GROUP 2 — Waiting on GH operations, verification, independent acceptance

| Gate | What has to be observed (not merely implemented) |
|---|---|
| GATE-01 REPO/CI | Exact-head source lock plus Windows/Linux Rust/SQLite/ANSF and React/Tauri build/test. Diagnose and repair actual failed jobs. |
| GATE-02 E1-P03–P07 | Genuine Windows/Linux supervised Git/FFprobe operations, original authorization, process cancellation, inner/outer journal, model tool-choice as separate real-GGUF gate. |
| GATE-03 E1-P08C–P13 | Linux cgroup-v2 escape/host-crash containment, Windows Job Object, trusted source freshness and independent Git/MP4/FFprobe verification, prior P09/P10 actual regressions. |
| GATE-04 E1-P15/AGENT-01F/G | Exact-head Windows/Linux ANSF/SQLite and actual desktop reviewed-agent step, native picker, real GGUF model choosing Git/FFprobe, verified independent source, crash/worker recovery, no duplicate effects, and UI plan timeline. |
| GATE-05 UX-CHAT | UX-CHAT01/02 original identity, edits, replies, startup reconciliation and native/React acceptance; installed WebView deferred. |
| GATE-06 MEMORY | M2–M5 native source migration, restart, retrieval/procedural memory and genuine three-arm GGUF benchmark remeasurement. |
| GATE-07 KIAF BENCH | Same model/quantization, repeatable authorized tasks, independent result witnesses and K/I/A/F vs environment failure attribution. |
| GATE-08 WIN-RELEASE | Near v1.0 physical installed NSIS/Win10/11 WebView, GGUF/MMProj AMD Vulkan, approved tools, worker kill/restart, uninstall. Deferred in **time**, never waived. |
| GATE-09 SIGN/LEGAL | Complete shipped dependency inventory, independent redistribution/legal check and actual release-signing identity decision at end. |

## Current work order

1. Preserve memory-first goal/clarification/source continuity as the grounding for real agent work, while native E1 tests continue.
2. AGENT-01F real loaded-model planning-to-original-ANSF-draft and AGENT-01G explicit single-step reviewed broker execution are source candidates. Repair exact-head native CI, independently run a genuine selected GGUF and reviewed source, then expand to multi-step execution only after end-to-end guarantees pass.
3. Restrict AGENT-01G to one freshly approved read-only independently witnessed Git/FFprobe step and do not call its source candidate production accepted until E1 real OS authority, output and lifecycle guarantees pass. Extend the host-owned scheduler to safe sequential steps, not replacement mini-apps.
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

## 2026-11-08 — AGENT-01G: human-approved single reviewed step to real native model/worker (SOURCE CANDIDATE)

- **K:** the already loaded GGUF model gets the original verified task-specific ANSF goal context and ONE reviewed Git or FFprobe tool schema. No second planner, arbitrary command, shell, user-provided executable or new mini-app. A result containing more than one model tool call does not become an accepted planned step.
- **I:** the desktop now has a separate `lu_agent_run_reviewed_step` command and existing **Agent work plans** UI action for a currently approved independent-witness step. A native picker selects only the Git repository or WAV/MP4 file, validates reviewed binary and source digest, and the human must confirm the exact plan+revision+step+operation+resource in a second native OS dialog. The actual existing `ReviewedApplicationTurnSession::run_journaled_observed` runs under normal cancellation, TaskStore and inner/outer effect journals. Ordinary reviewed-app asks are unchanged and separate.
- **A:** new saved human `g-` turn is created only after native confirmation. Host `OriginalChatGoalSources` and current unanswered/deferred clarification checks run around the operation. AGENT-01D `claim_host`, exact fresh `HostFreshStepConsent`, `propose_step` and `preflight_scheduled_step` enforce host-only original plan binding, current goal, approved revision, dependency, operator-confirmed resource, single current capability and process-local scheduler owner. The UI cannot authorize a worker by emitting a Boolean or reusing the plan approval.
- **F:** after the real tool turn, host scans ONLY its own fresh-turn read-only independent-outcome journal for exactly one matching `APPLIED/VERIFIED` inner worker call, then calls `link_verified_scheduled_worker_outcome` to re-open actual task/effect/oracle evidence and acknowledge precisely one historical plan step. The stored answer gets a separate **host-authored independently checked source observation**; the model's explanation is not mislabeled as entirely verified. No other planned step inherits permissions. Missing/multiple/unsupported/cancelled effects remain uncertain and never auto-retry.
- **Explicit limitations:** source is not actual running system proof. Only approved `INDEPENDENT_CHECK → INDEPENDENT_WITNESS` steps currently have the AGENT-01C witness link; an unresolved prerequisite or already host-claimed plan is refused. The current AGENT-01D scheduler supports one host claim per plan and safe manual reconciliation, NOT a complete multi-step autonomous agent. A normal human-reviewed plan may contain unsupported steps that still require implementation/explicit review. Host restart cannot inherit a process-local owner. Linux cgroup-v2 escape custody, Windows Job Object descendant reaping, installed GGUF/OS performance remain separate gates.
- **Tests submitted:** seven AGENT-01G Node source contracts; six Windows native reviewed-app Rust tests, existing exact-head ANSF/plan/receipt/scheduler/Windows React+Tauri checks. These are **not accepted until completed GitHub jobs provide native logs**. Actual OS worker acceptance and a loaded GGUF planning+tool-selection demonstration are STILL OPEN.

[AGENT-01G design and open gates](LU_AGENT01G_REVIEWED_STEP_EXECUTION_20261108.md).

## 2026-11-08 — AGENT-01G-R1 fail-closed scheduler lifecycle

01G source now wraps its real host claim in `PlannedStepLifecycle`: any early failure/cancellation after claiming produces explicit scheduler cancellation INTENT and `CANCEL_INTENT_UNCERTAIN`, without inventing worker termination or enabling replay. Verified evidence is released only after durable original-conversation result storage. AGENT-01F's example now makes a standalone independent Git/media check an eligible first step without forcing unrelated goals into the reviewed utility set. CI contracts expanded to 10 Node cases plus 7 Windows Rust host tests, including real ANSF/SQLite scheduler reopen after abandoned claim. **Status: source candidate / waiting on exact-head GitHub native CI.** [Full 01G-R1 record](LU_AGENT01G_REVIEWED_STEP_EXECUTION_20261108.md).

## 2026-11-08/09 — AGENT-01G native failures and real GGUF bytes

Actual earlier SHA AGENT-01A/01C Windows/Linux job logs were inspected: E0599 on incorrect `AnsfSourceId::stable_source_id()` in an agent-plan test is repaired to `.encode()`; old E0689 Git-pack integer typing and E0308 MP4 byte-pattern errors are already source-corrected on current head. **No exact-head test pass**: 48 GitHub jobs remain queued/pending. Real user-supplied `qwen2.5-0.5b-instruct-q2_k.gguf` was byte verified locally: **415,182,688 bytes, SHA-256 `9ee36184e616dfc76df4f5dd66f908dbde6979524ae36e6cefb67f532f798cb8`, GGUF v3, 291 tensors**. No llama-server/real token generation in this isolated container; current hosted E1-P07 workflow instead downloads/checks pinned Qwen3-0.6B and is queued. **Implementation queue:** genuine model, real OS, independently verified source-result E2E and subsequent fresh-approval multi-step logic. **Acceptance queue:** exact-head Windows/Linux compiler and real Qwen runner results. [Detailed evidence and scope](LU_AGENT01G_REVIEWED_STEP_EXECUTION_20261108.md).

## 2026-11-08 — verified local llama.cpp and exact GitHub evidence custody

Located `llama-server` in archived Debian Lu preview, verified 20 engine binaries/libs, and performed **genuine local Qwen2.5 0.5B Q2_K CPU token generation**. One diagnostic accepted real model-chosen read-only Git HEAD against a direct on-disk ref oracle; another unsafe tool-looking prose response was refused. **Not AGENT-01G host admission or a real production Lu reviewed-worker execution pass.** User requested the original three JSON receipts be uploaded into PR #159; they are now pinned byte-for-byte in `evidence/agent-01g/qwen2p5-local-cpu-20261108` with file-hash manifest, 5 deterministic Node evidence checks and explicit status labels. GGUF and binary remain outside Git. Qwen3-0.6B Q4_K_M is only configured as a checksum-pinned Hugging Face download in GH E1-P07 CI, not stored as GitHub model asset. **Implementation next:** actual original goal → plan review → fresh native approved operation → Lu broker tool choice → real independent receipt; then repeated individual fresh approvals for sequential plans. **Acceptance next:** exact-head CI and genuine Qwen3 test results; defer physical installed Windows until v1.0 as requested.

## 2026-11-08 — GH Qwen3 planning reliability probe

The existing E1-P07 CI GGUF runner now also records three genuine localhost Qwen3-0.6B structured-planning responses. New `benchmarks/e1/agent01g_qwen_plan_probe.py` strictly classifies plan JSON/clarification, never creates tools or approvals, writes source-visible model response hashes, and has eight standalone Python parser regression tests. Its evidence is **quality-only, not Lu host or independent-worker acceptance**. Real reviewed Git/FFprobe Lu tool selection remains the separate required E1-P07 gate. GitHub results currently queued; no comparison to the independently uploaded local Qwen2.5 diagnostics is controlled for model/engine/quantization.

## 2026-11-08 — 01G real native Qwen3 end-to-end gate added (PENDING native results)

Added an explicitly ignored but CI-invoked **actual pinned Qwen3** acceptance test to existing E1-P07. It exercises saved genuine ANSF goal/user messages; independent immutable revision approval fixture; separate fresh human step fixture; 01D bounded host-fenced claim; actual one-tool ConversationAuthority and protected reviewed Git worker with real E1 independent source oracle; 01C/01D historical result/receipt; native journal close/reopen; deliberate missing-oracle and changed-ANSF-goal reconciliation; and explicit denial of restart/reuse. No canned model reply or host-chosen Git operation is accepted. The native OS user dialogs are strict test doubles, not installed Tauri acceptance. A separate 5-case Node source contract catches wiring regressions. **No test PASS has been seen on the new SHA: multi-step remains disabled.** [Technical gate](LU_AGENT01G_REVIEWED_STEP_EXECUTION_20261108.md).

**01G prompt provenance strengthened:** The pinned Qwen3 headless native integration test now feeds the real current `AnsfStore` projected saved goal into the model prompt, verifying goal ID, original event and unresolved clarification status before the tool choice. This checks actual K→I context continuity rather than just an unused memory record. **Source only** until real native CI and independent worker logs pass; never treat prompt construction as a permission grant or enable multi-step.

## 2026-11-08 — Physical Windows NSIS acceptance track (AGENT-01G-WIN)

The user explicitly approved shipping an **isolated development NSIS Windows installer** to clear actual physical Windows gates. This does not defer engineering QA until v1.0. Source candidate now has `physical_win_cpu_eval` Rust feature, exact-source Windows 2025 GH workflow, pinned upstream llama.cpp CPU build, SHA embedded into Lu at compile time, native Win64 Common Controls v6 resource, isolated NSIS install path/app identity, real disposable host NSIS install/uninstall cycle, and operator README/receipt. **The test EXE is not yet created or available to download until the Windows builder actually completes.** Normal pinned Vulkan production engine and standard Lu profile remain unchanged; physical CPU test does not certify GPU offload. [AGENT-01G physical test build](LU_AGENT01G_PHYSICAL_WINDOWS_NSIS_TEST_BUILD_20261108.md).

**Implementation queue:** fix any actual Windows build errors or real physical user findings, then complete model→authorized reviewed Git→independent receipt→restart and fresh one-step per authorization. **Acceptance queue:** current-head Windows Rust/React/NSIS packaging and genuine installed/uninstalled artifact; operator GGUF/ANSF/native picker/results; separate OS descendant custody, AMD Vulkan, legal/signed release. No automatic multi-step activation.

## 2026-11-08 — AGENT-01H read-only next-step preparation while NSIS builds

**Source candidate, not execution acceptance:** `read_chat_progress` now emits a bounded `continuationPreview` for every original human plan, with a possible step index, mandatory blocking reason, `liveSourceReverified=false` and `executionAuthority=NOT_AUTHORIZED_FOR_EXECUTION`. The UI explains released/active/uncertain scheduler ownership, missing or stale independent witnesses, unverified dependencies, plan/source revision changes and unsupported step types. Only a first already-reviewed independent-check step can display the existing **manual one-step native approval** action. A historical result never becomes a new grant, even after restart. Four new genuine ChatStore/SQLite native tests and six source-contract Node tests are in AGENT-01E CI. The `AGENT-01G` actual GGUF/OS gates and Windows test-only NSIS builder remain separate pending acceptance. **No automatic continuation, worker replay, new owner or tool authorization has been added.** See [AGENT-01H detailed ledger](LU_AGENT01H_READ_ONLY_CONTINUATION_PREVIEW_20261108.md).

## 2026-11-08 — MEMORY-LOOP-01 original-source plan handoff while OS jobs queue

**Workstream:** IMP-02 MEMORY-LOOP, separate from still-queued AGENT-01L kernel/NSIS acceptance. Explicit current-human `Review agent plan p-<32 lowercase hex>` or `Inspect agent plan ...` in the normal loaded-model Send path reads only that authenticated chat's own ANSF-original plan, current immutable revision, AGENT-01D owner, independent actual worker/oracle receipts and OS custody reconciliation. The source-bound projection contains bounded host-classified statuses, not copied historical instructions or an inherited model/system/user role. Wrong chat, missing plan, changed source, absent task/OS receipt, or older/non-human plan selector fails closed with a generic unavailable context. `NOT_AUTHORIZED_FOR_EXECUTION` remains explicit; no TaskStore resume, worker spawn, grant, or automatic retry occurs.

**Acceptance:** pure strict-selector tests, actual native ChatStore/no-plan nonmutation check and Node source-admission guards submitted; **no native pass claimed** until exact-head Windows/Linux runs execute. Next integration is genuine GGUF context comprehension, original ANSF goal/procedural evidence quality and operator-visible follow-up, without relaxing independent OS recovery gates.

## 2026-11-08 — MEMORY-LOOP-02 verified historical outcome feedback for next model reasoning turn

**P0 IMP-02 implementation candidate, stacked on MEMORY-LOOP-01:** the next *actual host-authenticated human* GGUF Send derives up to 12 earlier original turn IDs from the same ANSF book's `origin='host' / kind='user'` index and reads at most two reviewed execution turns. It reopens real TaskStore + inner/outer EffectJournal independent verdicts, matches source-specific oracle method and receipt SHA, and independently asks the AGENT-01L Win32 Job/Linux delegated-cgroup provider for exact historical process-tree custody. The model receives only fixed reviewed operation labels, bounded original turn IDs, an independently observed SHA-256 digest and verified/conflict/unsupported/uncertain/custody states. No raw worker content, old instructions, tool grant, OS path, shell, source freshness assertion or automatic replay is included. Source-only; actual native Windows/Linux tests, original Qwen comprehension, installed NSIS and full Linux delegated-cgroup acceptance remain OPEN. [Dedicated work order](LU_MEMORY_LOOP02_VERIFIED_FEEDBACK_20261108.md).

## 2026-11-08 — MEMORY-LOOP-02 genuine pinned Qwen evidence comprehension work order

The requested actual-model verification now has a **real native source fixture and real local GGUF runner** workgroup. A test-only Rust producer calls the actual `reviewed_outcome_oracle::verify_git` over genuine Git commits/mismatched receipt/uncertain operation, persists independent outcome judgments, restarts original ANSF/TaskStore/EffectJournal, and exports exactly the bounded feedback already wired into the next Lu model turn. A Python benchmark independently loads SHA-pinned Qwen3-0.6B Q4_K_M in a SHA-pinned source-built llama.cpp server, checks real health and nine actual responses, and grades verified/conflict/uncertain, OS custody, source freshness, refusal to reuse old permissions, prohibition on auto-retry and fresh next actions. A nonexecuting or inaccurate model is **failure/NOT EXECUTED**, never a placeholder pass. Native source+real-model workflow submitted; no actual OS job/model output accepted until artifacts confirm it. [Benchmark evidence/limits](LU_MEMORY_LOOP02_REAL_QWEN_ACCEPTANCE_20261108.md).

## 2026-11-08 — MEMORY-LOOP-02B native host safe-action classification

After the genuine local tiny-Qwen diagnostic (0/18 strict model comprehension), implement Lu-owned deterministic HostSafeNext over the original ANSF+TaskStore+EffectJournal independent oracle and Job/cgroup custody evidence. Severity is monotonic: missing original/unread effects > conflicting/uncertain verification > verified source with OS custody unproven > historically fully checked source+OS. All branches remain no-replay/no-old-permission and require fresh review before a new effect. Feed fixed hostSafeNextAction and modelMayOverrideHostSafeDecision=false to the next GGUF turn. Source-only candidate: standalone Rust tests, real SQLite reopening/tampering cases, Node architecture gates, exact-head Windows/Linux workflow. Native results and installed app remain OPEN. See [work order](LU_MEMORY_LOOP02B_HOST_SAFE_DECISION_20261108.md).

## 2026-11-09 — MEMORY-LOOP-02C native, read-only provenance in chat

**Implementation source candidate, NOT accepted native/installed evidence.** A new host-only `lu_agent_recovery_provenance` Tauri API selects the most recent original ANSF host-human turns in the authenticated chat and reconstructs historical reviewed task/effect/oracle source status and independently attested original Job/cgroup descendant custody. It renders actual original turn IDs, fixed reviewed operation names, independent historical source digest (only when valid), host-safe next action and explicit coverage gaps directly in Native tool activity. Any older source window or effect rows left unread are a strict original-history reconciliation, never a global clearance. Host recovery classification cannot be overridden by model prose and gives no native execution authority; each future effect still requires a fresh human turn, live source verification and broker authorization. Ten Node source contracts, four genuine SQLite/Rust source-native tests and focused Linux/Windows exact-head acceptance workflow are submitted; no passing CI or installed-screen claim yet. [02C work order](LU_MEMORY_LOOP02C_OPERATOR_PROVENANCE_20261109.md). The separate 02B, AGENT-01L and physical NSIS/Vulkan gates remain open; Cognitum stays deferred.

## 2026-11-09 — MEMORY-LOOP-03A current goal + independently classified recovery preflight

**P0 IMP-02 source candidate, following 02C; native and installed acceptance NOT YET OBSERVED.** Ordinary human `lu_generate` now checks the just-enqueued exact original ANSF human turn and derives bounded status from M2's real saved GoalSnapshot (including pending vs deferred clarifications) and 02C's independently reconstructed original reviewed task/effect/source-oracle/Job/cgroup history. A tiny pure host classifier provides fixed `goalStatus`, `historicalRecoveryStatus` and `conversationNextStep` in a bounded model-context capsule for the actual loaded GGUF. General chat needs no saved goal, but inert draft-plan review cannot inherit permissions or treat missing goals, deferred answers, conflicting receipts, unproven OS custody or omitted original turns as clearance. Existing AGENT-01F/G native plan authorization remains unchanged. Nine pure Rust policy tests and eight Node source guards are submitted; Windows/Linux Tauri, real GGUF planning quality, genuine OS custody and installed app remain independent open gates. The next **03B** is independently verified procedural memory + belief/context-change revision integration, not new tools or Cognitum expansion. [Technical design and acceptance](LU_MEMORY_LOOP03A_GOAL_RECOVERY_PREFLIGHT_20261109.md).

## 2026-11-09 — MEMORY-LOOP-03B original verified procedure, belief correction and CCM

**P0 IMP-02 source implementation candidate; Windows/Linux native, genuine GGUF and installed acceptance still OPEN.** After 03A's current saved goal/clarification plus independently classified original-effect preflight, the SAME actual human GGUF Send now offers a bounded 03B read-only ANSF memory-status capsule for strict current-human version-pinned procedure, belief and task-resume requests. M4's source-current, version-exact reusable procedures are now exposed to the desktop **only when** the latest source-linked checkpoint and APPLIED original receipt have an additional matching original EffectJournal independent outcome of the correct reviewed Git/media method and digest. Belief re-evaluation now uses that trusted second witness instead of the old always-false predicate, preserving Reported/Verified/Contradicted/Stale/Retracted/Superseded distinctions. M3 CCM compares actual source-validated checkpoints, and missing/changed records or omitted coverage demand review. No old human turn, procedural step, reported claim, prior grant or model output creates a new tool authority, source-freshness or OS-custody claim. Source contracts and host tests submitted with exact-head Windows/Linux CI; **queued is not passed**. [03B technical/acceptance record](LU_MEMORY_LOOP03B_VERIFIED_PROCEDURES_BELIEF_CCM_20261109.md). 03C next: actual-model comprehension and independent error-attribution benchmarks, without reopening Cognitum until later.

### 2026-11-09 Work Area 00 dependency and exact-head audit

The GitHub PR metadata refresh on `main` returned **174 open PRs**, including #188 and the separate IDE #189; `main` is not an ancestor of the current 03B head and does not contain this memory implementation. The 03B head `cbca41f79a1ede41d66a251196b834d3ed18eb62` passed the local REPO-01 source verifier with 802 tracked non-lock blobs; its 11 Node 03B source contracts passed. Its associated native Windows/Linux, React/Tauri and source-lock Actions runs were queued/pending at inspection, so no exact-head native acceptance is asserted.

An independently reproduced dependency defect preceded those queued jobs: `npm ci` failed with `EUSAGE` because the 03B workflow required a desktop `package-lock.json` absent from Git. The desktop package versions were already pinned in `package.json`. A generated npm lock now makes local `npm ci --no-audit --no-fund`, `npm run build`, and `npm run check` pass (59 UI checks). This only repairs deterministic desktop dependency admission; Cargo/SQLite/OS and genuine GGUF gates remain OPEN until exact new-head logs and artifacts are inspected. The accepted `0.1.0-alpha.2` remains separate.


### 2026-11-09 — R00-CI-GROUP-01: grouped acceptance instead of per-commit fanout

Owner directive: stop generating an independent Windows/Linux Actions matrix for every tiny source edit and run coherent acceptance only after each task group finishes. Existing 68 Lu Core workflow files were inspected from the source tree. A **draft single-tree change** converts 66 specialist/native/real-GGUF/E1/agent/memory/tools workflows to explicit `workflow_dispatch` only, preserving only the automatic PR baseline `lu-repo01-full-source.yml` (REPO-01 source identity). The full `lu-ci.yml` desktop suite and 66 specialist workflows retain their existing OS matrices and test logic but must now be invoked deliberately at task-group boundaries, including source-lock snapshot, rather than on every descendant PR. No standalone IDE workflows were modified; IDE needs a separately coordinated grouped-CI pass.

The source includes `scripts/lu-ci-group-policy.mjs` and tests to reject reintroduced automatic specialist fanout and a **dry-run by default** `scripts/lu-ci-superseded-queue.mjs` with strict branch/exact-head lease and explicit Actions-write approval before cancelling old queued automatic runs. The available GitHub connector cannot directly cancel Actions; **no old run cancellation is claimed**. Do not equate a queued/failed/cancel-requested run with accepted proof. See [task-group gate instructions](LU_CI_TASK_GROUP_GATES_20261109.md).

**Acceptance status:** source change and policy guards are only a draft candidate until exact-tree validation, Node policy tests and final group CI evidence. Real native 03B/03C and positive Win32 Job/Linux delegated cgroup tests remain OPEN. Next after R00-CI-GROUP: MEMORY-LOOP-03C genuine small-model comprehension/error attribution, then dispatch a narrow 03B/03C matrix once at that group's close. User physical-PC installation and 7B+ GGUF testing stay deferred without waiver.

### 2026-11-09 — MEMORY-LOOP-03C: real ANSF host-corpus, two-arm small-GGUF diagnostic (source candidate)

Stacked after R00 grouped-CI #193, the new 03C native desktop test generates four actual current-human, source-bound ANSF/SQLite 03B capsules: no verified procedure and uncertain historical OS custody; no original-source belief; no comparable CCM checkpoint; pending clarification dominating historical memory. The earlier 03A gates in this diagnostic are **test-controlled labels, not independently observed current-goal/OS process facts**. Positive independently verified procedure, belief, source revision and real Git/FFprobe effect cases remain a separate unfinished 03C stage.

A strict two-arm Python benchmark uses the same real checksum-pinned Qwen3-0.6B and seed: original source/goal/recovery data without Lu host classification vs the same data plus Lu 03B host status. It rejects fabricated or missing model generations, unauthorized actions, unsupported source freshness, OS overclaims, extra JSON/tool keys and tampered fixture schema, attributing model comprehension, authority, interface and feedback separately. The modified 03B task-group workflow adds exact-head Windows/Linux native fixture export, Python policy tests and OPTIONAL real-Qwen completion only when `run_real_qwen=true` at task-group close. Source-only code and negative fixture tests are NOT native, installed Lu, genuinely successful Qwen inference or current OS custody acceptance. [03C technical task and open gates](LU_MEMORY_LOOP03C_REAL_MODEL_COMPREHENSION_20261109.md).


### 2026-11-09 — MEMORY-LOOP-03C-G4 independently witnessed historical source vs current authority

Source candidate stacked on 03C #194 and R00 task-group CI #193: added **two real OS Git HEAD observations** with actual ANSF/Continuity/EffectJournal SQLite, source-linked historical verified procedure and claim, plus six user-identity-bound read-only 03B memory capsules covering positive observation, wrong version, current clarification, human invalidation and retraction. **The Git calls are test-only, not the Lu-reviewed protected worker**, and the independent journal record is bound to the true first/second observed Git HEAD. Python strict positive fixture validation and an adversarial suite reject fabricated current freshness, OS custody, permission reuse, source digest mismatch and forged procedure status. The existing genuine-Qwen 03C source-only/host-classified bench accepts the independently created positive corpus but never fabricates actual model responses. A single *manually dispatched* 03B/03C acceptance batch will compile Rust, execute the native two-command Git+SQLite test and Python suites on Windows/Linux, with optional real pinned Qwen3-0.6B only after group completion.

**Source work not promoted:** no actual hosted Windows/Linux compiler logs or real model generations observed for this new head; no physical 7B+ evidence; no positive protected Lu worker or FFprobe oracle; no live current-source recheck or kernel descendant custody. Future: reviewed Lu Git/FFprobe worker effects, independent source changes, CCM contradiction/revision corpus and measured benchmarks. See the 03C acceptance ledger. CI governance remains task-group-only; do not fire acceptance on minor source edits.


### 2026-11-09 — MEMORY-LOOP-03C-G5 actual reviewed Git/FFprobe worker, source drift, recovery (SOURCE CANDIDATE)

This group now reuses the **existing Lu Core protected reviewed app session**, not direct test Git as a substitute. The native desktop integration test drives real host-selected OS Git and WAV FFprobe subprocesses through the ConversationAuthority → ReviewedApplicationTurnSession → OS supervisor → original EffectJournal → direct independent Git/PCM source verifier → original ANSF/ContinuityStore/03B procedure+belief read chain. Three real reviewed operations are planned across **two distinct, authenticated original human turns and separate task grants**. A Git commit changes between the first and second genuine worker observations; the old verified source remains historical NOT_RECHECKED, never current truth or an automatic grant. The newer independent observation produces a source-changed CCM human-review gate; only the subsequent authenticated human correction invalidates the older procedure and retracts its belief. A closed/reopened journal retains APPLIED receipts and marks a separate intentionally unfinished **journal-only** effect UNCERTAIN through the existing exclusive-host recovery API (no child was launched for that synthetic interrupted intent).

The 03C model-comparison runner now optionally consumes an independent G5 worker-generated corpus with 8 scenarios; strict parser and 10 adversarial tests reject made-up authority, altered Git source, unverified effects and invented OS evidence. **The model responses in the native worker test are SCRIPTED, not GGUF**, and this source candidate is **not evidence of native Windows/Linux pass or full process descendant custody**. The associated group-end workflow is still manually triggered and runs real host OS worker + Python verifier per OS, with optional pinned Qwen3-0.6B model comparisons. **No heavyweight CI launched per minor change.** Acceptance remains pending actual exact-head Rust compile, Git/FFprobe OS results, recovery logs, Qwen outputs, and profile/installed application evidence. The owner’s 7B+ physical-PC test remains deferred. See LU_MEMORY_LOOP03C_REAL_MODEL_COMPREHENSION_20261109.md.


**G5 production-journal boundary reconciled:** actual reviewed-app sessions bind a fresh human turn to a **per-turn** task/effect SQLite file; ordinary 03B GGUF memory reads a separate global journal. G5's integration test mirrors **distinct per-turn** worker journals, requiring explicit host selection of the historical journal when verifying source-linked procedures. This tests the original memory reader against genuine supervised receipts but does **not** close installed normal-Send cross-journal lookup/promotion. Add the host-owned turn→journal evidence index as a separate integration task, without shadow-APPLIED journal entries or permission inheritance. The test uses production ReviewedHostProfile process supervision and explicit bounded tool-policy allowlists.


### 2026-11-09 — MEMORY-LOOP-03D normal-Send original reviewed-worker evidence locator (SOURCE IMPLEMENTED / ACCEPTANCE OPEN)

Installed reviewed app and normal GGUF chat previously had separate effect journals, so the G5 positive memory source path was test-only. Implemented an actual host-owned index in reviewed_memory_evidence.rs and connected production reviewed_app_desktop.rs **after verified completion** to store original authenticated turn, source SHA, checkpoint, exact inner effect, independently verified original worker receipt digest and observed Git HEAD/media duration. Its indexed source projection creates version-matched historical ANSF M4 procedures and claims without granting an executable, old user permission, retry or live-source assertion. If a later separately authorized Git HEAD differs, the new continuity checkpoint includes a blocker and CCM requires human review; no old belief is silently replaced.

In ordinary lib.rs normal-Send, explicit current-human M4 selectors can read the original per-turn reviewed journal by **deriving its fixed filename from the host-authenticated indexed user turn**, and revalidate the source, tool-specific second oracle and original receipt. Wrong-profile/chat, changed original source, forged binding, altered/missing journal or unsupported oracle fail closed. General journal fallback only occurs when there is no indexed match; a broken matching indexed receipt cannot silently fall back to a different source. Added four Rust source/index tests with synthetic journal fixtures and extended the existing real G5 ignored OS test for production-style index mapping. The existing manually dispatched 03B/03C Windows/Linux workflow includes both; **no native acceptance jobs fired per edit**.

**Status distinction:** code and static source contract authored. Actual native Win/Linux compilation, actual reviewed Git/FFprobe run, persisted normal-Send GUI interaction, true small-GGUF generations, process crash/cgroup descendant custody and owner physical-PC 7B+ remain OPEN. A crash after actual OS effect but before index publication retains original per-turn evidence; a future exclusive-owner read-only reconstruction mechanism must not replay old effects.


### 2026-11-09 — MEMORY-LOOP-03E crash-consistent reviewed evidence publication

**Source candidate; native and installed acceptance OPEN.** Follow-on to #197 adds a durable publication marker only after ANSF verified continuity, independent effect linkage and procedural/belief projection. Normal GGUF recall now refuses legacy/crash-interrupted unpublished rows and requires an exact original checkpoint/receipt link. It never automatically republishes old evidence, reuses historical grants or falls back to a different journal. New native SQLite negative test covers interruption and forged checkpoint. 03E-R2 explicit read-only operator reconciliation, physical Windows installed GGUF, real OS worker and cgroup/Job custody remain OPEN. End-of-group CI only.


### 2026-11-09 — 03E-R2 atomic projection intent and exclusive-profile source reconciliation

The reviewed index row and full deterministic memory projection plan now share ONE index-SQLite transaction. Derived ANSF and continuity remain separate WAL databases and are idempotent materializations behind a publication fence; we explicitly **do not** claim cross-file ACID semantics. During exclusive DesktopOwner startup, incomplete intents are revalidated against original authenticated source and revision, original task journal origin, independent effect oracle, changed human turns/corrections, claim expiry and later checkpoint. Safe projections are materialized and published without worker replay; ambiguous, expired or legacy items require human review. The Native tool activity pane exposes the per-chat recovery outcome, with no execution/ retry control. Rust regressions and Node source contracts run only as part of the existing manual consolidated gate. **Source submitted; real native, installed GGUF and descendant-custody gates OPEN.**

### 2026-11-09 — Linux task-group execution evidence

PR #198 compiled on Linux with the generated desktop PNG and fixed memory,
reply and plan view wiring. Core library tests passed (525, with 17 ignored),
as did six opt-in real Git/FFprobe/FFmpeg integration cases. Desktop recovery
passed nine tests and the opt-in G5 restart test produced the source-verified
native corpus using real reviewed Git/FFprobe workers and scripted model
selections. The full desktop suite had 194 passes, two ignored and one
Observe process-identity failure on the sandbox's mismatched procfs PID
namespace. Desktop cargo check passed. See the 03E technical record for
the scope of each check. Windows acceptance remains open.

The standalone AGENT-01L locked build required its own Cargo.lock. With
that lock, both the Linux custodian and the host crash fixture compiled.
The native harness reported AGENT01L_CGROUP_DELEGATION_NOT_AVAILABLE on
the read-only cgroup mount. Positive escaped-descendant custody remains
open and requires a delegated Linux runner.

### 2026-11-09 — R08/R09 Linux file and archive group

The Linux file/workspace/archive integration group passed 46 tests at
`e39d2c16f01fce3478d2d1b68abe583d0b427c2a`. The follow-up replaces the
trash/restore hard-link then unlink sequence with an atomic no-clobber
rename between opened directories, reusing the move identity and digest
checks. A post-rename mismatch remains uncertain. Linux pathname
substitution can still cause a different object to move, so independent
identity reconciliation remains required. Cross-volume renames fail
closed. The follow-up passed ten Linux transfer and authority integration
tests. Windows ACL, reparse, and installed desktop checks remain open.

### 2026-11-09 — R10 Reader and scanned PDF OCR, R11 citation evidence

Linux Reader/OCR passed 46 Reader unit tests, four OCR unit tests, twelve
desktop source contracts and one opt-in native test with two scanned pages.
Poppler interpreted the explicit output dash as a literal filename. Omit
the output prefix so the PNG goes through the bounded stdout pipe.
The native test verifies the original PDF digest, distinct page raster
digests, exact English text and its digest, cancellation, and human review.
The scan fixture was an image-only, two-page PDF made from 1000×480 white
Pillow RGB images with black DejaVu Sans 62px text at (55,110): ALPHA SOURCE
PAGE and BRAVO SOURCE PAGE, saved as PDF at 144 dpi. The run pinned the
system Poppler and Tesseract binaries and English data by SHA-256.
Windows packaging, multilingual accuracy, and installed UI remain open.

The existing Linux R11 research group passed six tests for citation/source
binding, false citation denial, restart after CAPTCHA state, and source
counting. The headed-browser test returned early without its X11 launch
fixture. This run did not prove Chromium lifecycle or external origins.

The original small-Qwen benchmark produced 72 real generations, with
zero of 36 passing in each arm. All failures were scored as output format.
The original report SHA-256 is
`966b71650113dbf37de4522ec5d78f61909d3fe31eca37014aab6fedc3dd2feb`.
This is evidence of a response format failure, not a measured difference
in source comprehension. A separate direct-answer protocol trial is pending.

### 2026-11-09 — R13 Linux background and recovery group

Seven Linux Core integration cases passed across task checkpoints, startup
and owner recovery, and task effect fencing. The 21 background/recovery
Node contracts passed after correcting a source assertion that treated a
documentation mention of TaskStore::resume as a call. The standalone
scanner harness now has a Cargo lock and existing group workflows use it
with --locked. Both native scanner harness checks passed, including a
12 MiB fixture with measured progress and an independent digest.
Installed-app lifecycle and full multi-chat capacity remain open.

### 2026-11-09 — R12 and R14 Linux and source contracts

R12 passed 44 native Observe/image/multimodal library cases and 56 Node
contracts. A stale Node guard expected a retired automatic Observe branch
trigger. It now requires the existing manual gate and continues to check
the reviewer-session fences. Windows WGC, OS consent, and real MMProj
semantic interpretation remain open.

R14 passed 30 chat/queue/attachment UI contracts, ten Core cross-chat and
saved-prompt integration tests, and 13 native desktop chat store/revision
tests. Interactive accessibility, installed UI, and Windows recovery remain
open. The Linux tests do not substitute for those gates.

### 2026-11-09 — R15 real Qwen output protocol diagnostic

The raw reports are in benchmarks/e1/evidence. The baseline was two
repetitions, 72 generations and zero passes. The matched direct-answer
trial used the same pinned Qwen3 0.6B GGUF, engine, fixtures and unchanged
scorer, with an empty Qwen reasoning prefix in both arms and one repetition.
It completed 36 generations. Source-only had 15 feedback overclaims and
three format failures. Host-classified had 18 format failures. Both arms
scored zero. This records a diagnostic, and the memory acceptance gate
remains open. The benchmark still needs a no-context control and a
verified structured-output run before comparing comprehension.

### 2026-11-09 — PR #198 grouped native gate dispatch and demonstrated REPO-01 policy failure

The *single grouped* Windows/Linux native-memory acceptance run was dispatched as Actions #42701772029 on exact head `2ab7a6fa61f269345d6e33bbd3a664432c4fed3d`. Both `native-memory` matrix jobs were still QUEUED at inspection: there were no actual native compiler, runtime, or artifact results to accept or repair. Separately, REPO-01 source run #42701783858 **failed** at `Audit grouped-CI trigger policy and queue cleanup safety`: seven of eight policy tests passed, but the checked-in heavy memory workflow's temporary one-shot push event was classified `HEAVY_WORKFLOW_AUTOMATIC_FANOUT`. The frozen source-tree audit passed (`tracked=825`, Actions PR merge SHA `cf9d197f59867d5dac4073a69e79e6b81b2e7fd3`), but a source check is not native acceptance.

**Repair:** restore the existing `lu-memory-loop03b-native.yml` to manual `workflow_dispatch` only, preserve the source-policy test unmodified, and update `source-tree.lock.json` without altering any native implementation. The earlier run remains useful **for its original SHA only**; after this policy repair, its Windows/Linux results cannot promote the new PR head. A new **single consolidated manual exact-head** task-group dispatch is required when runner access is available. Never reintroduce permanent heavy auto-trigger or treat queued/cancelled jobs as passes. PR #198 remains DRAFT.

### 2026-11-09 — R00 Core PR convergence worktree (separate from IDE and main)

Created a frozen Core candidate from draft #198 at `a7d82f13fd62ba460be9b204ba7574b9dcf87dcf`, with a refreshable full open PR graph, exact head SHA and relation witnesses, and a conservative disposition matrix. Observed 183 open PRs (105 Core, 78 IDE), 67 declared base-ref ancestors from #198, and 38 outside that declared chain. Git compare confirmed 16 of the 38 side PR heads are already ancestors and 22 diverge; no blind cherry-pick or main merge. Main itself differs by 28 commits covering release/ledger/reversible-edit paths. Read-only graph generation and validation tests are included. This is a **source convergence candidate**, not an installed build, new native acceptance, release, or authority grant. Preserve Lu Core alpha release and separately scoped IDE stack; keep #198 DRAFT pending exact native gates.

### 2026-11-09 — R00 convergence includes independently demonstrated Windows SQLite cleanup repair

Native run #42701772029: Ubuntu success; Windows compiler and nine reviewed evidence tests succeeded, but opt-in G5 protected-worker restart fixture failed on Windows (`Os error 32`, `memory_loop03c_reviewed_worker_tests.rs:623`) because the separate source-selector `historical` EffectJournal was not closed before temporary profile deletion. PR #198 commit `74dbdfe1f8457631f1992efed2c79b37629715aa` explicitly closes that journal; the Core convergence merge includes the exact file blob and Git parent. No compile/runtime retest yet of the new SHA. Keep original review authority and no replay, no promotion.

### 2026-11-09 — R00 genuine main→Core integration-branch merge

A reviewed two-parent source merge of canonical `main` `fa91a0d4997a8e24f62bfe03e4c9ac00bb54ccca` and Lu Core convergence keeps the accepted CHANGELOG, main development ledger and unchanged WRITE-01E runtime module, preserving the Core's later verified worker/ANSF code. Repository metadata-only ledger workflow proposed as manual read-only (no implicit scheduled write authority). Main design-resource guardian delta remains explicitly open for human product reconciliation; documentation can be reviewed from the actual second Git parent. No changes to main, no IDE changes, no heavy CI dispatch, no native acceptance implied.

### 2026-11-09 — R00 Core #61 tested Windows fixture cleanups and browser candidate negative enforcement

Reconciled selected 2026-11-08 changes from Observe/Wave4 side PR #61 via exact-file patch review against the current Core source: archive+Office review fixture handles explicitly close before Windows temp-directory deletion, exclusive DesktopOwner reacquisition retries only `AlreadyRunning` for a bounded 2-second window, and both continuity SQLite fixture paths include per-process atomics to avoid collisions. The archived native archive-tree and Linux package fixtures ALREADY had **stronger** multi-handle cleanup/retry on this integration head and were **not overwritten**. Browser-CI candidate and executable suites now honor explicit `LU_BROWSER_REQUIRE_CANDIDATE=1` by failing when their pinned candidate artifact is missing (ordinary UI runs may skip, never masquerade as an installed-browser pass). The current `Cargo.lock` already includes `image`. No Lu authority boundary or product runtime behavior changed. These are source reconciliations; native exact-head Windows test rerun remains required.

### 2026-11-09 — R00 #133 cgroup-v2 nested-escape parity, host-only

The older E1-P08C branch #133 and current AGENT-01L both use a host-created delegated cgroup, a guardian outside the killable leaf, pre-exec worker membership and original-source OS proofs. **Do not replace** AGENT-01L's current production host and first-party custodian with the old parallel `linux_cgroup.rs` implementation. However, #133 identified an important extra acceptance case: an escaped `setsid` grandchild in a **nested child cgroup**, plus bounded bottom-up removal of empty kernel cgroups. Implemented in current `lu-linux-cgroup-guard.rs` a fail-closed, 16-level/1024-entry recursive cleanup requiring `cgroup.events populated=0`, refusing symlinks and unexpected node kinds. The existing test-only crash host now actually moves its escaped child into `lu-nested-host-test`; the independent kernel/PIDFD Python oracle checks both actual procfs identities and removal of nested/root cgroups after Lu SIGKILL. Success remains **unverified** until a genuinely delegated kernel runner executes this positive case. A read-only cgroup, skipped test, or a mere PGID check is NOT acceptance. Privilege-escape via writable ancestor cgroup.procs remains a separate OPEN security threat; the child leaf must not be considered nonmigratable without independent kernel evidence.

### 2026-11-09 — R00 frozen reconciliation of 22 formerly divergent Core tips

Audited each of the 22 original side PR tips and its exact branch SHA against the #198 ancestry snapshot. New machine register `docs/LU_CORE_DIVERGENT_TIP_RESOLUTION_20261109.json` and human decisions `docs/LU_CORE_22_DIVERGENT_STATUS_20261109.md` distinguish 2 selected patch integrations (#61/#133), 14 superseded/historical candidates and 6 deliberate non-alpha tool/DLC/backend proposals. In particular: old full JSON Schema semantics (#16) are not present in the current JSON boundary, and original long-horizon tool progress guards (#19) are not claimed to be implemented; these are honestly scoped future gaps. The current Node queued-run cancellation policy supersedes #112, and all 22 old PRs remain untouched rather than fake-merged. Source integrity tests compare all 22 with frozen SHAs, no native acceptance inferred.

### 2026-11-09 — R00 consolidated exact-head task-group gate prepared, not auto-fanned-out

The existing REPO-01 baseline workflow now contains a guarded `pull_request.edited` handoff to **one** `native-r00-consolidated` Windows/Linux matrix, restricted to draft PR #200 and an added, exact-head marker in the human-reviewable PR description. This avoids modifying the heavy manual-only workflow or permitting normal source commits to launch native CI. The new gate includes actual full Rust Core/Desktop tests, real supervised Git+FFprobe G5 restart, ANSF recovery, independent oracle, Windows fixture regressions, Node contracts and Linux AGENT01L nested-child fixture compilation. No native result is claimed until the exact GitHub run finishes. The marker is CI control only, **not** product/tool authority.

### 2026-11-09 — R00 actual compiled Windows/Linux failures at first consolidated head

Run #42716675752 exact `3e8d78fa26f41f7ebdb7f0011a39d0e85d4589fa`: REPO-01 succeeded. Ubuntu G5 real supervised Git/FFprobe, full Core and desktop Rust lib suite succeeded; 1/9 cgroup Node source-contract assertions failed at old `origin.1` spelling. Windows G5 genuinely failed with SQLite ERROR_SHARING_VIOLATION at reviewed worker test cleanup line 627, even after earlier `drop(historical)`. Audited `TypedGitReadBroker::new` revocation callback to find an Arc capture of `BackgroundProcessSupervisor` and `EffectJournal` retained by the still-live `ConversationAuthority`. Fix explicitly drops that authority after live turn revocation and before exclusive journal restart; source test asserts lifetime ordering. Linux JS assertion now checks current fail-closed `turn!=plan.workspace_id` admission. These remain **proposed fixes** until actual new-head native runners pass, no source-only promotion.

### 2026-11-09 — R00 PR merge SHA/source-lock mismatch repaired without waiving verifier

REPO-01 #42718912579 checked out GitHub synthetic `pull/200/merge` at `0e01e5dd...`, whose moving main second parent added a ledger-only commit `6dc7ed61540876c660ab875669e62d567a07b59b`; the branch exact head `8ee33e39699556fcfcd1e84bf0bfd3db4556d60f` itself had 840 matching source blobs. The verifier rightly refused the synthetic checkout (docs/LU_DEVELOPMENT_LEDGER.md). The source gate now uses `github.event.pull_request.head.sha || github.sha`, asserts exact Git HEAD, and runs the original verifier unchanged. Native opt-in requires a marker transition to the *new* SHA, never plain PR edits or normal pushes. All new-head native results are still pending.
