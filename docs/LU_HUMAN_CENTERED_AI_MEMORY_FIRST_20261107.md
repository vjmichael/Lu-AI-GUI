> **M2 canonical storage transition — November 7, 2026:** [ANSF-first conversation cutover](LU_M2_ANSF_CONVERSATION_CUTOVER_20261107.md) supersedes earlier provisional M2 JSONL-as-authority choices. New Lu chat text, original source entries, folders and goal decisions are source-wired to per-book ANSF SQLite and its Library; JSONL is a non-destructive migration input, not a permanent internal journal. No fresh privilege arises from imported historical turns. Source submitted, CI/installed acceptance OPEN. Continue M2 rather than declaring M3 started.

> **Baseline research contract — November 7, 2026:** [Real-model three-arm Lu memory CI benchmark](LU_ANSF_THREE_ARM_REAL_MODEL_BASELINE_20261107.md) compares the same pinned local GGUF with no memory, conventional recent chat, and real Lu Core ANSF direct source-ID context. The benchmark includes recent-history, stale/missing and cross-chat controls, requires authentic model output, and preserves negative or null findings. Its results do **not** establish full ANSF acceptance or generalization to installed Lu until independently validated.

> **Direct recall and multimodal scaling refinement — November 7, 2026:** [ANSF direct IDs and external artifact links](LU_ANSF_DIRECT_IDS_EXTERNAL_ARTIFACTS_20261107.md). Known source IDs are direct indexed lookups, bypassing three exploratory steps; Library/books/chapters/pages remain human navigation of effectively unbounded disk-backed history. The new ANSF Core artifact schema stores **references only**, with external file availability/integrity checked by a trusted host. Installed-model integration and full ANSF authoritative conversation migration still remain open.

> **M2 development started — November 7, 2026:** [ANSF goal and clarification continuity implementation](LU_M2_ANSF_GOAL_CLARIFICATION_CONTINUITY_20261107.md). The owner authorized implementing existing ANSF/SQLite into the real Lu desktop beginning with user goals and clarification answers. The trusted desktop JSONL `ChatStore` continues to own human-origin events; ANSF receives digest-checked, replay-safe derived goal memory; ordinary local-model Send can use bounded current-goal context. Yes/No/Not now with optional comment are UI-only memory responses, **not** tool authorizations. M3 semantic/cross-chat retrieval and M4 procedural learning remain separate phases. CI and installed application evidence are independent release gates.

> **M1 evidence update — November 7, 2026:** [M1 memory/authority/live-binding audit](LU_M1_MEMORY_EVIDENCE_AND_AUTHORITY_AUDIT_20261107.md) completed as a source audit. The actual desktop uses its `ChatStore` JSONL event log and bounded same-chat `typed_history` prompt path; the ANSF SQLite book/library components are source-implemented but a live ANSF-to-prompt retrieval link is not evidenced. Preserve `ChatStore` as current user-turn authority and implement an idempotent ANSF reference bridge in M2. M2 not started; installed-app memory E2E remains open. Clarifying questions may use a yes/no prompt with optional comment and “Not now,” but must be attributed to a real user-origin event; no implicit authorization.

# Lu — Human-centered AI capability, clarification loop, and memory-first architecture

**Owner decision: November 7, 2026.** This is a philosophical and sequencing decision, not a command to implement software. No permission, agent execution, worker, model or UI functionality is implied complete. Companion canonical source of truth: [AI Execution Layer philosophy](LU_AI_EXECUTION_LAYER_PHILOSOPHY_20261107.md). Earlier [K/I/A/F P01–P12 audit](LU_AI_EXECUTION_TOOL_REVIEW_20261107.md) and [implementation receipt](LU_KIAF_EXECUTION_P01_P12_IMPLEMENTATION_20261107.md) remain preserved.

## 1. Purpose and the missing human

The guiding purpose of Lu is to **maximize the useful practical realization of the model's latent capabilities in pursuit of the human user's legitimate goals**, by connecting model knowledge, changing context, memory, interfaces, authorized actions, verification and recovery.

Lu must **reduce the expertise required to achieve a goal**, **not reduce the user's intelligence**. Users differ in experience, domain knowledge, ability to articulate requirements, willingness to intervene and desired amount of explanation. A highly expert user can expose limitations and push model capabilities further; a beginner should nonetheless be enabled to use expert-level model knowledge. Neither user's intelligence nor competence should be assumed from a prompt. Respect user agency and request clarification only where it will materially improve the outcome.

The user's goals define what successful performance means. The user participates in the feedback loop by clarifying, correcting, reprioritizing and accepting results. The model is not the source of legitimate human intention, and a remembered preference is not permission to act.

**Revised philosophical statement:** Lu is a human-centered AI operating environment that maximizes the practical, authorized expression of a model's capabilities toward user-defined goals through adaptive collaboration, persistent and retrievable memory, external-world interfaces and verifiable closed-loop action.

## 2. Evolution of the mathematical framing

These expressions are **proposed conceptual/engineering models**, not empirically established universal laws or claims that all quantities have known units.

### Original: practical ability

`Practical AI Capability = f(K, I, A, F)`

- **K — Model knowledge/reasoning**: learned competencies, reasoning, planning and procedural knowledge.
- **I — Interfaces**: accessible and real tools, files, readers, applications, processes, browsers, APIs, devices.
- **A — Authority**: host-enforced effects limited to user-origin scope; OS ability/permission is distinct from action authority.
- **F — Feedback**: observation, tests, independent verification, meaningful errors, correction and durable recovery.

The critical distinction remains: **knowledge is not execution; installed software is not necessarily a model-facing interface; registered interface is not authority; tool completion text is not verified success**.

### Human-centered extension: user purpose and operational memory

`C_Lu(U,T,E) = f(K_eff,t, I_t, A_U, F_t ; G_U, E_t)`

- **U**: the human collaborator and decision-maker, with actual goals, preferences, choices and expertise—not a scalar “user intelligence” score.
- **G_U**: what the user seeks to achieve; may initially be under-specified, ambiguous or changing.
- **T**: the current task; **E_t**: changing applications, files, resources and environment.
- **M_t**: selectively retrieved, relevant, source-backed memory.
- **K_eff,t = g(K_model, M_t, O_<=t)**: knowledge effectively usable at this step, combining the model with relevant memories and current observations. Retrieval does **not** rewrite learned model weights.
- **I_t**: only real, currently available interfaces with discoverable prerequisites; **A_U**: current task-specific authorization constraint.
- **F_t**: new environmental, user and validator feedback to revise beliefs and future action selection.

A more precise **design objective** can be written as:

`maximize over collaborative policies pi: E[V_U(Y_T,verified)] - C_user_interaction - C_execution`

subject to `a_t in A_U(T,E_t)`, outcome quality/reliability requirements, bounded risk and honest verification. **V_U** means value defined by the user's real goals, not a score invented by Lu. This need not be a numerical formula in the product.

Only **after specifying the primary human goal and quality/safety constraints**, minimize engineering and execution burden:

`minimize (C_model + C_execution + C_tool + C_maintenance)`

This cost optimization is subordinate to task success and human choice, not a mandate for fewer tools at any price. A small typed utility can outperform an unsafe generic runner; an external specialist app can be preferable to a clone.

## 3. Clarifying questions are an intelligent action in the knowledge loop

A clarification question is neither idle chat nor a failure. It is an optional **information-gathering action** chosen when missing information, conflicting requirements, a changed goal, or high-stakes uncertainty could materially change the plan or authorization.

**Proposed loop:**
1. **Understand the goal.** Infer an explicitly provisional task interpretation; retrieve relevant user-approved context and source evidence.
2. **Identify uncertainty.** Distinguish missing *goal information*, missing *environmental state*, missing *authority*, and missing *technical ability*. Each has a different remedy.
3. **Choose the best next observation.** Inspect approved files/environment when that answers the question more efficiently. Ask the user when only they can define priorities, intent, preferences, thresholds or authorize a meaningful change.
4. **Ask a concise clarifying question when valuable.** Explain the relevant choice in ordinary language; prefer one material question rather than an overwhelming questionnaire. Do not repeatedly ask for answers already in the conversation or memory.
5. **Integrate the answer.** Record the clarified intent, any changed assumption and the provenance of the user-provided answer. Update the plan without discarding unrelated constraints or completed work.
6. **Act within current authority.** Per-effect authorization comes from the trusted host's actual user/task grant; clarification does not silently elevate privileges.
7. **Observe, independently verify and learn.** Compare outcomes with user goals; preserve confirmed results, uncertain status, corrections and reusable procedures; ask again only where a new material uncertainty arises.

A useful theoretical lens is **value of information** (VOI): ask when expected benefit from the answer exceeds user interruption cost and can change the outcome, subject to any mandatory consent rule. This is a qualitative policy here; no numeric VOI estimator is claimed to exist. Silence from a user does not mean approval.

For beginner users, clarification may include guided choices, examples and suggestions that help them discover what they want. For advanced users, be concise and preserve their precise constraints. Do not label users by intelligence or force a special mode without consent.

## 4. Memory as the prerequisite for extending general execution

**Approved sequencing:** prioritize **operationally usable memory** before adding broader **effectful general application execution**. Continue existing source/CI/release verification separately; do not delete established safe interfaces. A minimal set of read/observe/tool events is necessary to produce real memory evidence, but broad arbitrary code/app execution should not outrun its memory/state integration and authority controls.

Memory is not just a longer conversation transcript. It serves different functions with different validation and expiry rules:

| Kind | What to retain | Integrity requirement |
| --- | --- | --- |
| Working/task context | User goal, constraints, open decisions, task plan, active files and unresolved questions | Current-turn origin, explicit updates; short lived, not inferred permission |
| Episodic | What was attempted, actual tool results, failures, corrections, verified outcomes | Immutable references to source event, timestamp, result status |
| Semantic | Facts about applications, capabilities, environment and project state | Source, confidence, last verified, version/freshness; re-check before effects |
| Procedural | Reusable, verified workflows with prerequisites, action templates, failure conditions | Exact tested environment and version, success evidence, invalidate on contradiction |
| Durable operational state | Journaled effect intent, worker state, checkpoint, cancel/recovery and uncertain effects | Host authoritative, monotonic evidence, never reconstructed from conversational summaries |
| User preference/intent history | Explicit user preferences and clarified goals, with scope and revocation | User-controlled and distinguish from task-specific approval |

**Key rule:** memory is reference and context, not authority. A previous tool approval, credential, worker handle, or user's permission from another task can never be revived by retrieval. Untrusted tool/web/document content is not elevated into system instructions.

### Current repository evidence (not installed product acceptance)

- `src-tauri/lu-core/src/ansf.rs`: source-implemented SQLite conversation books, immutable entries, summaries, pagination and FTS.
- `src-tauri/lu-core/src/ansf_library.rs`: source-implemented reference-only searchable cross-conversation session/chapter catalog.
- `src-tauri/lu-core/src/ansf_revisions.rs`: source-implemented append-only edit lineage.
- `src-tauri/lu-core/src/ansf_sync.rs`: source-implemented reconciliation/import coordination.
- Existing effect and model-turn journals, tool-loop and capability inventory are complementary but are not substitutes for model-facing contextual memory.
- `src-tauri/lu-core/MEMORY_AND_MEDIA_ROADMAP.md` explicitly flags retrieval/assembly, semantic/object memory, role/owner controls, virtualized UI and integration gaps. Storage alone does not prove real memory-assisted task execution.

### Proposed memory-first milestones — **planning only**

**M1 — Evidence and authority audit.** Map ANSF books, library index, revisions, task/effect journals, model turn state and desktop interfaces; establish actual live bindings and authentication. Preserve originals, source ids and explicit user controls. No parallel memory store without a demonstrated need.

**M2 — Goal, clarification and task-state continuity.** Maintain current user intent, accepted constraints, clarified questions/answers, pending questions, decisions and changed goals in durable form. Do not use stale state or prompt injection as task authority.

**M3 — Selective context retrieval and source attribution.** Retrieve short, relevant, current, source-referenced history across conversations; supply it to the model in a bounded way, distinguish uncertain/obsolete records, and avoid re-discovery. The context window is a cache, not the permanent database.

**M4 — Verified procedural/semantic memory.** Promote successful workflows only after independent evidence; include tool and application versions, prerequisites, correction history, failure patterns, invalidation/revision and retrieval confidence. Treat suspected claims as hypotheses, not learned rules.

**M5 — End-to-end local-model acceptance.** On a real installed Lu instance, test same-task continuation after restart, switching models, user correction, stale application version, recalled bad advice, permission revocation and an interrupted/uncertain effect. Compare repeated tasks with/without usable memory: correctness, rediscovery/tool calls, interruption cost, recall precision and absence of unauthorized effects.

Only **after M1–M5 demonstrate sufficient integration** should a substantial generic model-directed process/application executor become the next major implementation phase; it still needs its own OS isolation, permission/authority, journal and verification acceptance. Existing targeted tools may be used while establishing memory and remain valid deterministic fallbacks.

## 5. Tool policy remains unchanged

Exactly three **capability** classifications remain: **Foundational**, **Utility**, and **Specialist**. They're not scores for users, degrees of model intelligence, or invitations to clone apps. Preserve current P01–P12 scoped, safe utilities unless evidence warrants reducing them. Don't implement more specialized mini-apps merely to hide weak context retrieval or a missing execution interface.

## 6. Criteria for evaluating the philosophy experimentally

- **Human-centered:** measure legitimate goal completion for users with varying familiarity, compare guided versus direct prompting; avoid conflating user unfamiliarity with low intelligence.
- **Clarification:** count materially useful questions, unnecessary interruptions, resolved ambiguity, forgotten answers, recovered changes of intent.
- **Memory:** compare same local GGUF and task with and without relevant history; measure correct recall, false/stale recall, rediscovery time, user correction retention and permission-boundary behavior.
- **Execution:** measure real tool availability, authorized effects, verified outputs and robust recovery; differentiate failures of model K, interface I, authority A, feedback F, memory selection and goal understanding.
- **Cost:** evaluate model tokens, latency, energy, human effort and maintenance **conditional on task quality**, rather than making lowest token count the objective.

## 7. Decision status

- **Approved philosophy:** the user is central; clarification is part of an adaptive, cyclical knowledge loop; lower the expertise needed to achieve goals while preserving advanced-user control.
- **Approved priority change:** memory/context/clarification integration **before new broad general-purpose effectful execution work**.
- **Not authorized by this document:** source-code modifications, tools deletion, arbitrary shell/network privilege, new OS permissions, automatic memory capture beyond existing consent, or declaring any existing memory subsystem installed and complete.
