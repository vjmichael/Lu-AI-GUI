# LU-AIW-AUDIT-01 — Foundational AI-usability codebase audit

**Date:** 2026-11-05  
**Purpose:** Test Lu against the foundational product philosophy established in LU-AIW-01: make software easier for AI to understand and operate so it spends its capability achieving human goals, rather than rediscovering environment facts, tools, source material, permissions and operating procedures.  
**Status:** Architecture and source audit; **not** a claim of successful compilation, an accepted installed app, or a full manual line-by-line security review.

## 1. Audit boundary and method

**Frozen default-branch source identity:** `vjmichael/Lu-AI-GUI` at commit `6645f7939b60700c4012d3a2a18b7553480980a6` (2026-11-05). Recursive Git tree was complete (not truncated): **221 tracked blobs, about 1.71 MB**, including **73 Lu Core Rust source files**, **61 Lu Core test files**, **15 desktop Rust/React/JS source files**, and **15 desktop test/fixture entries**. There were **no tracked GitHub Actions workflow files and no root AGENTS.md on this default-branch snapshot**. The rest are build scripts, configurations, docs, packaging, fixtures and project metadata.

**Branch inventory:** All **20 open draft PRs** visible on 2026-11-05 were inspected for their changed-file maps, base/head relationships and scope. Key implementations were read directly in PRs #3–#28, especially #4/#5/#14/#17/#18/#19/#20/#21/#24/#26/#27/#28. Branch deltas are **not** present in default-branch `main` merely because the corresponding PR exists. Some PRs depend on other draft branches and several modify the same desktop host, model tool loop, Protobuf contracts, Rust module exports and dependency locks. PR #24's first changed-file page was 100 files, so the branch-wide changed-file view may be larger than the first returned page.

**Review activities:** Enumerated the complete main tree; made a module-level static pass over all 73 Lu Core Rust source modules; examined the live desktop generation, context, chat, UI and packaging paths in detail; inspected memory, tool, authorization, model, journal, inference and evidence contracts; inventoried the test surface; and examined targeted implementations on the development branches. No build, Rust test, installed-app session, live GGUF run, actual NSIS build or adversarial runtime test was executed during this audit. The present findings are source-grounded design/integration findings, **not** runtime incident claims. Tests only described in code or queued in CI are not credited as completed.

**Reference specification:** [LU-AIW-01 in the v1.0 design](https://github.com/vjmichael/Lu-AI-GUI/blob/6645f7939b60700c4012d3a2a18b7553480980a6/docs/LU_V1_0_DESIGN_RECONCILIATION_20261103.md#lu-aiw-01--ai-accessible-workspaces-and-operational-continuity-2026-11-05). This specification is normative for the audit, not proof its requirements are already implemented.

## 2. Executive verdict

**Lu has many of the correct primitives but lacks the automatic integration that makes them useful to the AI in an ordinary human task.** For example:

- The default desktop currently calls `prompt_context::typed_history` with only recent messages from the *current chat*, capped at 16 turns / 15 KiB. It does not perform ANSF/cross-chat retrieval, consult an operational-resource ledger, include prior task checkpoints, or build a host-verified orientation before inference. See `lu-desktop/src-tauri/src/lib.rs` lines 773–842 and `prompt_context.rs` lines 1–36.
- Lu Core already contains independently designed `AnsfStore`, `AnsfLibrary` (FTS5), `ContinuityStore`, `CrossChatReference`, typed tool catalogs, `ConversationAuthority`, effect journals, generation audits and read-only native files. These are promising foundations. However they do not, by themselves, implement a host-orchestrated task-context workflow in the installed desktop.
- Draft PRs contain substantial additional implementation: safe edits, native environment discovery, structured browser operations, Reader, OCR policy, task status, backend control, Cognitum evidence search, and operational-resource memory. They need deliberate reconciliation rather than a new parallel framework.
- The operational-memory first slice in draft PR #28 already records resource paths, platform/version, historical success recipes and file fingerprints; the ordinary desktop does not initialize or consult it yet. Its public host-side success-record method still needs a trusted-runner receipt boundary before UI/model exposure.
- No complete **new-conversation → recognized goal → verified existing resources → authorized action → independently checked result → durable reuse on the next conversation** benchmark is presently established.

**Design direction:** preserve Lu Core, native Protobuf contracts, ANSF/SQLite, the existing user-turn permission boundary, first-party tools and normal desktop UX. Add **a host-owned task-orientation and verified context assembly layer**, integrate the existing parts through it, and measure end-to-end task success. Do not add another mandatory runtime, an MCP dependency, or an enormous model-visible tool catalog.

## 3. LU-AIW-01 compliance matrix

| Requirement | Existing foundation | Gap / current acceptance |
| --- | --- | --- |
| **R1 — Precise source references** | ANSF entry IDs/revisions, Git/source and Reader selectors on draft branches, SHA-256 file fingerprints | No shared source-anchor contract across chat, documents, code, workbook, slide, film, UI and returned tool evidence. No revision-aware re-resolution visible in the default desktop. **Partial** |
| **R2 — Discoverable, versioned capabilities** | `ModelToolCatalog`, `ModuleManifest`, Protobuf input/output URLs, CE package/schema lifecycle and policy | Tool/worker/workflow descriptors are not uniformly indexed by task, relevance, current readiness, costs, example calls and recovery semantics; several native tools are in unmerged branches. **Partial** |
| **R3 — Operational resource memory** | `compute_inventory.rs` hardware hints, PR #17 environment presence scanner, PR #28 durable resource ledger | No working desktop resource preflight, dependency/probe receipts or cross-chat artifact reuse. Current-process attestation is not a trusted runner integration. **Draft source only** |
| **R4 — Task-driven contextual retrieval** | `AnsfLibrary` FTS5, `ContinuityStore`, `CrossChatReference`, PR #21 Cognitum search | The ordinary desktop supplies recent current-chat history only. No host-side task classifier, goal/object resolution, evidence ranking, freshness checks or token-budgeted provenance assembly. **Missing integration** |
| **R5 — Semantic operations, human UI fallback** | Native Rust tool registry, Protobuf codecs, approval/policy/effect journals; PR #24 safe file operations; PR #27 Reader; PR #26 browser | Tool families and desktop actions lack one discoverable object/operation facade and shared proposal/result UX. UI operations are not always the same as model-capable typed actions. **Partial and fragmented** |
| **R6 — Demonstrable verification** | Model-turn audit, effect journal, passive host diagnostic events, many Rust unit/integration tests, real-GGUF test scaffold | No standard evidence bundle tying goal → source revision → action → test → installed result; live GGUF tool test is ignored unless explicitly configured. No golden LU-AIW task suite. **Partial** |
| **R7 — Recovery, least privilege, control** | Conversation authority, per-effect policy, durable journal, cancellation, Windows Job design, reversible-edit proposals | Real desktop/worker containment and proposal/apply flows vary by branch; full Windows Core currently has a worker method mismatch. End-to-end kill/reopen/undo acceptance is open. **Strong primitives; incomplete adoption** |

## 4. Prioritized findings with change locations

Priority labels: **P0** = release/platform or architectural integration blocker; **P1** = core human-assistance capability; **P2** = useful efficiency/usability improvement or post-v1 common layer. “Existing” means present in default-branch source; “Draft” means source in a PR, **not** verified integrated.

### P0-01 — No unified host-side task-orientation path
**Evidence:** `lu-desktop/src-tauri/src/lib.rs:773–842` directly commits the human prompt then passes recent typed history to the supervisor. `lu-desktop/src-tauri/src/prompt_context.rs:1–36` considers only the active conversation and bounds 16 prior turns / 15 KiB. `src-tauri/lu-core/src/capability_router.rs:1–19,92–143` admits exact reviewed candidates but is not a general goal/resource/context assembler.

**Impact:** Lu can respond but must repeatedly “remember” where it left off, which files/tools exist and which decisions matter. Smaller local models waste tokens on rediscovery.

**Change:** Add a trusted `TaskPreflight` / `ContextAssembler` between persistence and inference. Inputs: exact persisted human turn, active project/workspace, referenced objects, model/token budget and available host capabilities. Outputs: compact *inspectable* orientation with citations, versioned objects, relevant past decisions, current resource state, permitted tool descriptions, pending effects and known limitations. Keep non-authoritative context separate from independently checked permissions.

**Acceptance:** A new conversation asking to resume a known task gets the correct source revision, last checkpoint, available resources and authorized next step without copying previous conversations; a change to the goal invalidates the prior task route. No cited source or scope means no automatic action.

### P0-02 — ANSF and cross-conversation continuity are library islands
**Evidence:** `src-tauri/lu-core/src/ansf.rs`, `ansf_library.rs:116–190,305–370`, `continuity.rs:72–129,189–220` and `cross_chat_reference.rs:1–4` exist. The latter explicitly says cross-chat references are not wired to desktop prompts; the live desktop initializes `ChatStore` JSONL in `lu-desktop/src-tauri/src/lib.rs:436–444`.

**Impact:** A human-approved design decision or successfully tested action in another conversation may not reach the model even though Lu contains searchable storage code.

**Change:** Create a migration-compatible desktop `AnsfContextService` with authenticated source ownership, FTS/entity search, revision/provenance lookup, task checkpoint retrieval, and a lightweight source link. Keep JSONL baseline history readable; migrate/index rather than destructively replace or silently summarize immutable original messages. Index human-approved decisions separately from uncertain model summaries.

**Acceptance:** Retrieve an exact prior decision from another authorized conversation; explain whether it is current or superseded; produce a source entry and revision; deny cross-project access; survive restart and migration.

### P0-03 — Operational memory is not connected to desktop preflight
**Evidence:** Hardware hints in `src-tauri/lu-core/src/compute_inventory.rs` are session diagnostics, not a reusable toolchain inventory. Draft PR #17 contains `native_environment_discovery.rs` (bounded host-selected program presence checks); draft PR #28 contains `operational_memory.rs` (SQLite historical resource records, file fingerprinting, last successful recipe) and `AGENTS.md`. Neither is in the default desktop main path.

**Impact:** NSIS, compiler packages, model engines, datasets, fixture directories and completed build recipes are rediscovered across sessions.

**Change:** Reconcile PR #17's host-approved live probes with PR #28's durable historical ledger. Host records resource ID, workspace/project, OS+architecture, version, trusted locator, dependency graph, last verified evidence/receipt, last tested recipe ID and provenance. Perform a quick *probe before search/download*; never infer readiness from memory alone. Bind any “verified usable” claim to a recent typed receipt from the trusted runner, not an arbitrary public method call or text summary. Never execute a recorded command string. Add stale/changed/missing and privacy-sensitive path handling.

**Acceptance:** Windows NSIS and Cognitum benchmark continuity tests across new chats/restarts, with successful reuse and deliberate deletion/hash mismatch; **zero false-ready classifications** and no unauthorized installs.

### P0-04 — Merge topology is itself an AI-usability defect
**Evidence:** 20 open draft PRs at audit time. PR #24 spans native host/GUI, safe editing, search, docs, Core and tests; PR #27 adds Reader/desktop chat/worker changes; PR #26 adds browser/research and alters the same host/model/backend/proto files; PR #28 adds operational memory on main; other branches introduce Git, archive, OCR, JSON schema and backend control. Shared change hotspots: `lu-desktop/src-tauri/src/lib.rs`, `lu-desktop/web/main.jsx`, `src-tauri/lu-core/src/lib.rs`, `src-tauri/lu-core/proto/lu/v1/core.proto`, `src-tauri/lu-core/src/supervised_tool_generation.rs`, lockfiles, and source-tree identity manifests.

**Impact:** An AI starting a new task sees multiple credible but divergent “current implementations”; features may be present on a draft branch and absent in a build. Blind cherry-picks risk protocol/authority regressions.

**Change:** Establish a host-readable authoritative **integration matrix**: feature ID, owning PR, base/head SHA, dependencies, build/test status, conflicts, authoritative contract, desktop reachability and acceptance gate. Rebase/merge in verified dependency order; retain the tested alpha baseline; no “done” based merely on a source branch. Prefer adding narrow host modules over repeatedly growing a monolithic desktop `lib.rs`.

**Acceptance:** One authoritative source for a selected build can answer “is Reader installed?”, “which native tools are callable?”, “which recipe built this installer?” and “which PR has the implementation?” with exact SHA and evidence.

### P0-05 — Full Windows Core source validation is blocked independently of AIW
**Evidence:** `src-tauri/lu-core/src/persistent_llama_backend.rs:291–313` calls `Job::assign_and_resume_suspended`, while the default-branch `src-tauri/lu-win-worker-job/src/lib.rs` exposes `Job::assign` but not that method. The PR #28 earlier full Windows Core run failed with Rust E0599 at this call. The isolated PR #28 operational-memory test harness intentionally does not assert that full Core compiles.

**Impact:** A foundational AI-quality change cannot be release-accepted through full Windows compilation or installed-app proof while this existing containment interface diverges.

**Change:** Resolve the suspended-child assignment/resume contract **without** replacing it with a race-prone unsuspended spawn as a quick workaround. Test Windows child lifecycle, Job containment, cancellation, no orphan workers and host responsiveness. Re-run full desktop compile and physical install acceptance independently of module harness tests.

**Acceptance:** Real full Windows Lu Core + Tauri builds and tests pass at an exact SHA, then installed desktop loads GGUF/MMProj and handles worker termination without orphaned children.

### P1-06 — Current model tool decisions are prompt-only, not decoder-constrained
**Evidence:** `src-tauri/lu-core/src/supervised_tool_generation.rs:295–329` assembles a ChatML prompt that *asks* for one JSON object. `src-tauri/lu-core/src/model_json_boundary.rs` strictly parses/repairs limited syntax; `src-tauri/lu-core/src/model.rs` validates JSON schemas and typed Protobuf. But `src-tauri/lu-core/src/multimodal_saved_turn.rs:338–392` creates llama.cpp chat requests with only messages, max_tokens, temperature and stream—no `response_format`/grammar on that source route.

**Impact:** A Qwen-class local model can still generate malformed tool envelopes, wrong tool names or non-JSON text despite rigorous host-side validation.

**Change:** Add opt-in, backend-version-capability-verified constrained JSON generation for the *tool decision envelope*, with a selected-tool argument schema and model-template compatibility checks. Preserve ordinary natural-language streaming for normal chat. Constrained syntax never authorizes an action or establishes semantic correctness. Add one or two schema-valid examples as contract tests, with a small bounded schema catalog.

**Acceptance:** Live local GGUF A/B comparisons with and without constraints: parse validity, correct tool selection, authorization decisions, attempts/latency and final task completion. Record model/backend/template SHA. A constrained envelope must not bypass Protobuf or policy checks.

### P1-07 — Source locations are not a universal interface
**Evidence:** `AnsfStore` uses entry IDs and `ansf_revisions.rs` tracks revisions; PR #27 Reader responds with `source_sha256`, character offsets, selectors `sheet:/range:/slide:/page:`, but its `ReaderResponse` does not consistently return stable source object IDs, one-based line/column spans, page geometry and change-aware re-resolution. The main desktop's chat `ChatMessage` is only `role/text/interrupted/cancelled` (`chat_store.rs:64–70`). Source-code LSP/symbol and NVP scene/shot/frame referents are not part of one shared interface.

**Impact:** The model and human can disagree about “line 100,” “page 4,” “cell D12,” or which revision of a scene was meant.

**Change:** Define small generic `SourceRef` and `SourceSpan` value contracts: workspace/project, object ID/kind, version/hash/commit, optional line/column, byte/Unicode offset, page, sheet/range, slide, or scene/shot/frame/timecode; immutable original locator; explicit stale/ambiguous state. Adapters map to these, not new separate grant mechanisms. Preserve CRLF and blank lines. Code symbol resolution via LSP/tree-sitter is a later plugin/IDE enhancement, not required for a first text-line implementation.

**Acceptance:** Human opens the cited region; after insertion/deletion Lu resolves against original revision or reports stale/ambiguous; copied excerpts retain verifiable source anchors without altering original documents.

### P1-08 — Reader is an isolated action, not yet common document intelligence
**Evidence:** PR #27 adds `native_reader_tool.rs`, `native_reader_turn.rs`, `reader_desktop.rs` and a separate opt-in `Ask Reader` workflow. It has good per-turn file pinning, authorized reads, fingerprints and bounded pagination. Its own `docs/LU_READER_CLOSEOUT.md` records limited classic PDF, OCR not complete, parser worker isolation open, Office long extraction limits and installed-app tests open.

**Impact:** Document reading is usable in selected cases but not automatically grounded in project object references and shared context; humans may need to attach the same document repeatedly.

**Change:** Integrate Reader with the source-reference service and task context assembly; add an authorized document catalog and user-visible source/section controls. Maintain native vision-first capability checks and clearly label unsupported scanned/complex PDFs or formats. Add worker containment and real producer fixtures before universal-format claims.

**Acceptance:** Import once, find a prior authorized source again, cite precise page/sheet/slide/line and revision, detect stale source and avoid duplicated extraction. Real model and installed-app acceptance required.

### P1-09 — No declarative task/workflow manifest or visible, pinnable mode
**Evidence:** `ModuleManifest` has workflow contribution metadata and `capability_router.rs` validates exact host-supplied rules; the default desktop UI (`web/main.jsx`) is primarily a chat/model manager. There is no active user-visible workflow profile binding tool selection, context policy, model profile and renderer state. PR #11 defines a future DLC host API, not a deployed workflow router.

**Impact:** AI and user cannot inspect why a tool was exposed or a coding task was interpreted as ordinary chat. Every new mode risks Rust/React special-case code.

**Change:** Define an inert, versioned workflow profile schema now (mode, small capability set, context strategy, model profile, source object kinds, UI components, recovery policy) but defer arbitrary user-authored executable workflows to the post-v1 common workspace layer. Route selection must be host-owned, visible and user-pinnable; manifests can never mint grants.

**Acceptance:** A task's chosen mode, available actions and context sources are visible; pin/override changes UI/model routing but never expands authorization; a malformed manifest is inert.

### P1-10 — Errors are typed, but recovery instructions are inconsistent
**Evidence:** `src-tauri/lu-core/src/model.rs:225–277` already emits `code`, `message`, `retryable` for many native errors; Reader returns adapter-specific refusal codes and JSON argument failures. No uniform `next_action` / `resolution_hint`, `safe_to_retry` or required human involvement field is enforced across tools.

**Impact:** AI can waste turns repeating an identical invalid call, interpreting stale documents as missing, or retriggering a possibly applied write after a timeout.

**Change:** Standardize a bounded, non-sensitive `ToolProblem` envelope: error code, cause class, target/source revision state, safe next-action hint, retry classification (none / read again / revalidate / ask human / reconcile journal), and evidence ID. Keep public model hints deliberately safe and avoid leaking secret paths or privileged denial details.

**Acceptance:** Golden bad-argument, stale SHA, missing dependency, denied action, worker death and uncertain write scenarios produce exactly the safe next action; **zero automatic retry of uncertain non-idempotent effects**.

### P1-11 — Propose/apply/undo UX should be shared rather than per tool
**Evidence:** `src-tauri/lu-core/src/reversible_edit.rs` already creates immutable edit proposals and approval fingerprints. PR #5 and #24 include richer native edit journals, rollback, file creation and desktop review modules. The ordinary `main` chat UI does not use a single typed proposal/review/result card for all mutations.

**Impact:** Writing, source edits, file moves and future NVP edits become bespoke permission UX flows and force the model to learn different application conventions.

**Change:** Introduce a host-owned proposal envelope with target ID+revision, exact diff, requested effect, estimated impact, preview renderer, approval provenance and expiry; accept only independently authorized apply. Reuse existing durable edit/effect journals. Don't demand an intrusive confirmation for every harmless read or repeat an approval that already safely covers a precise effect.

**Acceptance:** One review surface handles code and text edits; stale revision or changed bytes refuse apply; cancel/restart preserves proposal state; undo never overwrites an unrelated subsequent human edit.

### P1-12 — Verification evidence is not yet a coherent task result
**Evidence:** `model_turn_journal.rs` stores model/tool exchanges; `journal.rs` records effects; `passive_diagnostics.rs` keeps privacy-conscious host events; `benchmark_contract.rs` defines reproducible model comparisons. These are not yet assembled into one UI-facing completion receipt for human tasks. The real-GGUF test (`tests/real_gguf_tool_json.rs:106–108`) is marked ignored unless configured.

**Impact:** A source commit, a compiled binary, an NSIS file and a successfully installed/operated program can all be mistakenly described as “finished.”

**Change:** Add a generic `TaskEvidenceBundle` linking human goal, task ID, source refs, loaded model/backends, tool/env versions, proposed/applied effects, actual tests, omissions, output hashes, install/run evidence, terminal/uncertain state and human approvals. Surface a concise result to the human with optional drill-down. Diagnostics and model-turn traces should correlate by opaque IDs; redact prompts, secrets and sensitive paths by default.

**Acceptance:** Each golden task distinguishes drafted/committed/compiled/tested/installed/approved. Every asserted success can be opened to a real file hash, test report, verified reopen or native worker receipt. Unknown must remain UNCERTAIN.

### P1-13 — No golden human-assistance task benchmark
**Evidence:** Main has many unit tests and a model-independent `BenchmarkRun` schema (`benchmark_contract.rs`), but no committed cross-session, whole-desktop LU-AIW suite. Existing physical Windows scripts are primarily alpha visual/inference acceptance. Draft PRs add individual tool/worker CI suites.

**Impact:** Improving schema parse validity or retrieval scores may not actually help a person complete a task faster or with fewer interventions.

**Change:** Introduce repeatable real task fixtures with clear target artifact and success oracle: NSIS rediscovery/build; Cognitum benchmark continuation; source-specific Reader question with citation; approved text edit and revision conflict; wrong-tool selection; cross-chat decision recovery; worker crash; no-authorization refusal. Use the same local Qwen model/quantization/backend/context settings and seeded inputs for with/without orientation or constraints. Maintain a portable smoke layer and a physical Windows GGUF/NSIS acceptance layer.

**Measure:** Task success, wrong-tool rate, schema validity, rediscovery and unnecessary-download count, evidence validity, token/step/latency cost, clarifications, stale readiness and unauthorized actions. Set quantitative improvement targets **after** measuring an honest baseline; security invariants have zero tolerance.

### P1-14 — Test and build discoverability are dispersed and branch-specific
**Evidence:** On frozen `main`, no `.github/workflows` or `AGENTS.md` is tracked. The accepted alpha README and desktop README contain partly different historical milestone language. Draft PR #28 adds `AGENTS.md` with source map and commands; draft PRs #24/#26/#27 add CI and broader integration tests independently.

**Impact:** New coding agents and human contributors can mistake a prior alpha or another PR's workflow for the current accepted test source and re-download/build resources needlessly.

**Change:** Merge/reconcile the agent orientation file with one short canonical architecture map, task briefs, build commands, module/test owners, dependency evidence and explicit branch acceptance matrix. Make `AGENTS.md` an index, not a parallel design document; keep authoritative architecture and acceptance in versioned docs. Add source-identity and smoke gates to the selected integration branch, then to main.

**Acceptance:** A fresh local model can find the right build commands, target modules, accepted installer and pending gates without scanning conversation history; the commands and artifact provenance are reverified.

### P1-15 — Current chat search is not project knowledge search
**Evidence:** `lu-desktop/web/main.jsx:67,475–481` filters chat metadata with `matchesChatTitle`; `web/chat_ui_behavior.mjs` compares only `chat.title`. In contrast, `AnsfLibrary` provides FTS5 book/chapter indexes and `cross_chat_reference.rs` provides authorized links—but those are not surfaced in the desktop search/reader workflow.

**Impact:** The human and model can find a conversation title, but not reliably locate the relevant paragraph, design decision, file, task checkpoint or past verification by content.

**Change:** Add permission-scoped combined search results across ANSF entries, source objects, task receipts and resource inventory. Rank authoritative user decisions and verified evidence above stale summaries; show source revision, project and last verified state. Reuse one source viewer for human and AI.

**Acceptance:** Search “where did we put the NSIS compiler?” and return prior resource evidence with a fresh readiness state, not just chat titles; search decisions with citations and deny unrelated projects.

### P1-16 — Operational security and privacy need explicit integration at the AIW boundary
**Evidence:** `conversation_authority.rs`, `host_resource_authority.rs`, `ownership_store.rs`, `privilege_guard.rs`, CE signing/quarantine and typed Protobuf already protect important boundaries. Draft PR #28 exposes host APIs to remember resources and record claimed success; draft PR #27 Reader only permits one chosen document; draft PR #26 browser scopes origins.

**Impact:** Attaching new context and resource registries could accidentally reveal private filesystem locations or elevate an untrusted tool document into a command/permission. A "remembered recipe" must not become a covert shell executor.

**Change:** Bind every orientation item, tool definition, resource description and source excerpt to project ownership, sensitivity, provenance and current authorization. The model gets a redacted summary and opaque validated reference; raw secrets and absolute paths remain on the host unless explicitly necessary and approved. Tool completion receipts must originate from the trusted executor, not a string supplied by model, page, file or WebView.

**Acceptance:** Prompt-injection/poisoned-memory attempts never mint grants, select private paths, or cause unauthorized egress; identical user text with a different conversation/turn never inherits an old grant; revoked tasks are denied.

### P2-17 — Adopt reusable skills and a compact semantic vocabulary carefully
**Evidence:** Core tools expose individual typed models, and `MAX_ADVERTISED_TOOLS` is 16 in `model.rs`. There is no general Skills manifest loader or eight-verb facade on main. Draft DLC APIs and `ModuleManifest` already provide extension metadata.

**Impact:** As tools grow, local models spend tokens learning many similarly named commands and custom syntax.

**Change:** Support inert progressively loaded `SKILL.md` guidance and repo `AGENTS.md`; let host show only tools relevant to the current task. Prototype common semantic verbs such as find/read/propose/apply/diff/run/undo/export as **routing vocabulary**, not a replacement for specialized native Protobuf operations. Benchmark tool selection and token cost before standardizing. No skill file can execute code or change permissions merely by being read.

**Acceptance:** A small model completes representative tasks with fewer advertised schemas and no degradation of authorization or accuracy. Keep full declarative workspace execution post-v1.0.

### P2-18 — Resource, lifecycle and UI evidence should be navigable rather than text-only
**Evidence:** The normal desktop UI has model status, FIFO generation, chat folders, basic attachments and chat-title search. Core has separate GPU/CPU hints, model admission, passive host traces and task checkpoints; PR #27 adds background task panels, PR #20 backend stop/recovery, PR #4 loading traces.

**Impact:** The human cannot easily tell which workflow/mode is active, what Lu actually found, why it chose an action, whether a compiler is merely remembered, or which worker/test is still running.

**Change:** A compact, optional work-state panel: active task and goal, current mode, verified/unknown resources, source anchors, pending approvals, effect state, completion evidence and one next action. Preserve the ordinary clean chat view; don't flood the model or UI with the entire catalog.

**Acceptance:** The human can inspect/override task routing, open the cited span, see a real running/cancelled/uncertain status, and distinguish a completed artifact from a successful test.

## 5. Subsystem integration map — reuse rather than rebuild

| Current subsystem / source | Recommended integration owner | Avoid duplicating |
| --- | --- | --- |
| ANSF, library FTS5, revision links and cross-chat reference | `AnsfContextService` | Do not create a second conversation memory database or replace immutable originals |
| `ContinuityStore`, task/effect/model journals | `TaskEvidenceBundle` and preflight | Do not treat audit messages as replay authorization |
| PR #17 environment discovery + PR #28 `OperationalMemory` | `ResourceCatalog` with trusted-runner probes | Do not create independent package finders in every conversation or DLC |
| `ModelToolCatalog`, `capability_router`, `registry`, `ConversationAuthority` | `CapabilityBroker` and bounded model tool selector | No unguarded universal shell, self-authorizing manifest or mandatory MCP |
| PR #24 read/write/edit/rollback, PR #27 Reader, PR #26 browser | `SourceRef` + proposal/result card | Do not implement separately formatted approval screens for each operation |
| `model_json_boundary`, `supervised_tool_generation`, llama HTTP adapter | Structured decoding + schema/example conformance | Don't mistake successful parsing for correct authorized execution |
| `passive_diagnostics`, `ModelTurnJournal`, `BenchmarkRun` | Replayable, redacted traces + golden tasks | No sensitive raw prompt logging by default |
| Desktop chat storage, per-chat FIFO, app state | Host preflight/context + human work-state panel | Don't put task authority, history retrieval or background jobs inside WebView |

## 6. Approved implementation sequence — tool-first, memory integration second (2026-11-05)

**Owner sequencing decision (supersedes the earlier F1-before-tools proposal):** First finish the remaining **general-purpose Lu Core native tools**, including actual native host/desktop integration and verification. After the toolset is accepted, connect the memory architecture (ANSF, task continuity, operational resources and retrieval/orientation). Do **not** rewrite finished tools when doing memory work. This changes implementation order, **not** the LU-AIW-01 architecture or v1.0 memory requirement.

### Gate AIW-F0 — Reconcile the existing tool integration baseline
Create an exact-SHA PR/feature/contract matrix and choose a tested integration head. Reconcile the shared Protobuf, module exports, desktop host, tool catalog, worker containment and effect journal interfaces. Repair the existing Windows Job API mismatch without downgrading containment. Preserve the accepted alpha baseline. Continue implementation on narrow verified branches; do not call parallel draft implementations one shippable program.

**Exit:** Current feature ownership/heads are known; source and compiled binaries are distinguishable; full Windows/Linux build blockers are explicitly recorded; tool backlogs do not have duplicate implementations.

### Gate AIW-F1 — Finish remaining native tools and desktop acceptance FIRST
Complete or reconcile the original Core domain map (not automatic new features): authorized file read/create/edit/copy/move/rename/trash/restore, Reader, scoped content search, basic local Git inspection, safe archive inspection/operations, internal JSON repair plus standalone schema validation, environment discovery, capability-selected browser/research, loop guard, and supervised background status/cancel/recovery. Domain-specific `exec` requires explicit host authority; generic arbitrary shell is not default. Reuse PR #5/#12/#14–#20/#22/#24/#26/#27 rather than introducing new implementations. For each tool: typed Protobuf/schema, human-turn authorization, bounded effects, structured failure, cancellation, provenance, actual native Windows/Linux tests, and desktop reachability. Do not require advanced IDE GitHub management or Film/NVP full video/audio as Core v1.0 tools.

**Contract discipline while building tools:** Add minimal stable revision/source identifiers, machine-readable result/evidence IDs and actionable errors at boundaries *now*, but avoid implementing the full cross-chat memory pipeline in the middle of the tool closeout. This prevents costly later rewrites while honoring tool-first priority.

**Exit:** A published inventory marks each Core tool `source only` / `compiled` / `host integrated` / `real-model exercised` / `installed accepted`. Relevant model tool calls are authorized and actually dispatch through the desktop; failures/cancel/recovery are tested. No tool gets a completion mark for source-only work.

### Gate AIW-F2 — Incorporate the memory structure AFTER the toolset
Connect ANSF/SQLite searchable originals and approved decisions; `ContinuityStore` checkpoints, current task/effect receipts, PR #28 operational memory and PR #17 live resource discovery. Build trusted task preflight/context assembly that consults authorized current project records, source revisions, existing resource locations/dependencies, recent verified recipes and task checkpoints before asking a model to rediscover them. Use compact orientation and available tool descriptors with independent per-turn authorization. Historical presence and historical success never confer live executable authority.

**Exit:** New conversation reuses prior NSIS/Cognitum tool and workflow evidence after current verification, shows stale/missing/changed results, retrieves the correct project decision and task state, and never leaks another project's data or reuses an old grant.

### Gate AIW-F3 — Local-model tool reliability and trace benchmarks
Add backend-supported constrained JSON tool-decision envelopes (not forced JSON for natural chat), bounded examples/feedback and source-reference resolution over the now-integrated tools. Compare real Qwen tasks with/without structured decoding and/or orientation using fixed model, quantization, backend, hardware, context policy and fixtures. Keep existing Protobuf, permission and effect-journal guards mandatory.

**Exit:** Better *whole-task* success with evidence, not merely syntactically valid JSON; no increase in unsafe or repeated effects; actionable errors recover correctly.

### Gate AIW-F4 — End-to-end v1.0 and installed Windows acceptance
Exercise intent → source/context → verified inventory → authorized native tools → independent evidence → restart/undo/recovery → reuse. Real NSIS install/launch/uninstall, GGUF/MMProj and Vulkan, background workers, model cancellation, persisted prompts/docs/ANSF and failed worker recovery are tested separately from source compilation.

**Exit:** Full golden-task matrix with exact SHAs, passed/failed/skipped evidence and no unacknowledged critical release gaps.

### Later: common workflow/DLC layer (post-v1)
Declarative profiles, advanced IDE/GitHub tools, Film/NVP production workflows, LSP/tree-sitter symbol intelligence, user-pinnable workflow profiles, Skills ingestion and complex multi-workspace orchestration. Define interfaces ahead of time if needed; do not make them blockers for completing general-purpose Core tools or Lu v1.0.

## 7. Minimum golden-task fixtures (new work)

| ID | Human request | Expected independently verifiable result |
| --- | --- | --- |
| `G-AIW-01` | “Build another Windows installer.” New conversation; NSIS used previously | Find verified prior toolchain and recipe without unnecessary download; run fresh compatibility and full NSIS/install/launch proof; report artifact digest |
| `G-AIW-02` | Same, but compiler deleted or changed | Detect missing/changed resource, do not claim ready, propose approved recovery; do not execute stale path |
| `G-AIW-03` | “Resume the last Cognitum benchmark.” | Retrieve source-controlled task, exact datasets/versions, saved results and recent checkpoint; no silent assumption that a library still exists |
| `G-AIW-04` | “Which screenplay line did we approve?” | Human and model resolve same scene/line and revision; preserve CRLF/blank lines; stale inserts detected |
| `G-AIW-05` | “Read this XLSX/PDF and cite the finding.” | Authorized Reader fetch, source fingerprint and actual sheet/range/page/line citation; unsupported scanned PDF called out |
| `G-AIW-06` | “Correct that code paragraph.” | Exact typed diff/target revision, human approval, atomic apply, reopen/verify, safe undo that refuses concurrent change |
| `G-AIW-07` | Small Qwen asks for a native tool | Compare constrained/unconstrained tool-call validity, tool-selection correctness, model latency and final task outcome under equal inputs |
| `G-AIW-08` | Kill worker during effect or model generation | Clear cancelled/failed/uncertain receipt; recovered partial content; no duplicate write or orphan child; restart retains task context |
| `G-AIW-09` | Poisoned search hit/tool description attempts more access | Source treated as data; no self-granted tool, cross-project read, shell execution or network egress |
| `G-AIW-10` | Ask “What did we finish?” after restart | Evidence bundle distinguishes written, committed, compiled, tested, installed, reopened and approved states with exact source SHA |

These are **proposed acceptance fixtures**, not tests already passed. Run on both a portable CI harness and the real Windows GGUF/NSIS desktop. Model comparisons must hold backend, quantization, context, profiles, resource limits, task difficulty and seed policy fixed. Track false readiness and permissions separately from conversational quality.

## 8. Immediate engineering change set

1. **Keep this report and LU-AIW-01 canonical.** Merge a minimal `AGENTS.md` orientation (PR #28) and a branch/feature/acceptance matrix into the selected integration head. The report itself does not authorize code changes or mark gates complete.
2. **Do not rebuild operational memory.** Harden PR #28 with trusted runner attestation, dependency/version probes, current host/project scope, source links and real desktop preflight; reconcile with PR #17 rather than implementing a second check-environment inventory.
3. **Wire context retrieval at the trusted host.** Default chat's `typed_history` remains the safe fallback; optional host-selected context must use ANSF source provenance and honor the same token and permission ceilings.
4. **Add source-reference contract** to Reader, ANSF and edit responses before many new file formats; use stable object identity and revision, not line number alone.
5. **Constrain tool envelopes where supported,** without changing natural chat, and measure with live Qwen rather than synthetic fixtures alone.
6. **Adopt shared proposal/error/evidence records** for native operations, preserving the existing durable journal and human-turn authority model.
7. **Repair Windows Job API and close installed acceptance** rather than accepting Windows module-harness-only success.
8. **Implement the golden task suite** and use it to prioritize further tools, workflow descriptors, UI features and eventual DLCs.

## 9. Decision log / non-goals

- **Foundational principle:** Lu must be easier for AI to operate in service of people, with precise, verifiable sources, discoverable but permissioned operations, memory with freshness checks and evidence of actual completion.
- **Preserve:** First-party native Rust/Protobuf tools; ANSF + SQLite; per-turn host authorization; durable effects; GGUF/MMProj; Windows-first, normal-user, one-installer experience. MCP remains optional.
- **Do not conflate:** Human-approved facts with summaries; historical tool presence with current health; source code with integrated features; unit tests with installed operation; model success claims with independent receipts.
- **Do not start yet:** Universal eight-verb replacement of native tools; untrusted executable workflow packs; mandatory Agent Skills runtime; wholesale IDE/NVP/Film integration before core v1.0 stability.
- **Design improvement test:** If a proposal does not improve task accuracy, safe reuse of prior verified work, source precision, human control, or reduction of unnecessary rediscovery, do not add it merely because it sounds autonomous.

**Bottom line:** The highest-value engineering work is a small **host-owned bridge** from the authenticated user task to **ANSF provenance + operational resources + current capabilities + verified action receipts**, with a shared human/AI source-reference system. That is the missing integration layer between Lu's many capable pieces and the foundational goal of helping humans reliably finish work.


## 10. Revision-pinned source-anchor index

These are clickable revision-relative source locations, rather than floating line references. The main source links use audit base commit `6645f7939b60700c4012d3a2a18b7553480980a6`; draft links use the inspected PR head. A later merge or edit may move lines; retain the pinned SHA when checking the finding.

| Finding | Primary evidence | Supporting evidence |
| --- | --- | --- |
| P0-01 | [main desktop generation](https://github.com/vjmichael/Lu-AI-GUI/blob/6645f7939b60700c4012d3a2a18b7553480980a6/lu-desktop/src-tauri/src/lib.rs#L773-L842) | [typed recent history](https://github.com/vjmichael/Lu-AI-GUI/blob/6645f7939b60700c4012d3a2a18b7553480980a6/lu-desktop/src-tauri/src/prompt_context.rs#L1-L36) |
| P0-02 | [ANSF FTS5 index](https://github.com/vjmichael/Lu-AI-GUI/blob/6645f7939b60700c4012d3a2a18b7553480980a6/src-tauri/lu-core/src/ansf_library.rs#L116-L190) | [cross-chat reference not integrated](https://github.com/vjmichael/Lu-AI-GUI/blob/6645f7939b60700c4012d3a2a18b7553480980a6/src-tauri/lu-core/src/cross_chat_reference.rs#L1-L4) |
| P0-03 | [PR #28 operational ledger](https://github.com/vjmichael/Lu-AI-GUI/blob/c67d5f7abdaa1427102d0c9ad8ae98f19cf0bc5a/src-tauri/lu-core/src/operational_memory.rs#L164-L338) | [PR #17 environment scanner](https://github.com/vjmichael/Lu-AI-GUI/blob/a3dca4b968cf72a4e699da3311d47035de5145d2/src-tauri/lu-core/src/native_environment_discovery.rs#L57-L180) |
| P0-04 | [PR #24](https://github.com/vjmichael/Lu-AI-GUI/pull/24/files) | [PR #27](https://github.com/vjmichael/Lu-AI-GUI/pull/27/files), [PR #26](https://github.com/vjmichael/Lu-AI-GUI/pull/26/files), [PR #28](https://github.com/vjmichael/Lu-AI-GUI/pull/28/files) |
| P0-05 | [backend expects suspended worker method](https://github.com/vjmichael/Lu-AI-GUI/blob/6645f7939b60700c4012d3a2a18b7553480980a6/src-tauri/lu-core/src/persistent_llama_backend.rs#L291-L313) | [Job API lacks it](https://github.com/vjmichael/Lu-AI-GUI/blob/6645f7939b60700c4012d3a2a18b7553480980a6/src-tauri/lu-win-worker-job/src/lib.rs#L76-L118) |
| P1-06 | [ChatML schema instructions](https://github.com/vjmichael/Lu-AI-GUI/blob/6645f7939b60700c4012d3a2a18b7553480980a6/src-tauri/lu-core/src/supervised_tool_generation.rs#L295-L329) | [unconstrained chat request builder](https://github.com/vjmichael/Lu-AI-GUI/blob/6645f7939b60700c4012d3a2a18b7553480980a6/src-tauri/lu-core/src/multimodal_saved_turn.rs#L338-L392) |
| P1-07 | [PR #27 Reader response](https://github.com/vjmichael/Lu-AI-GUI/blob/c716876aed0276e98cb372ccd2cd16c916c5c00d/src-tauri/lu-core/src/native_reader_tool.rs#L48-L67) | [main chat message](https://github.com/vjmichael/Lu-AI-GUI/blob/6645f7939b60700c4012d3a2a18b7553480980a6/lu-desktop/src-tauri/src/chat_store.rs#L56-L70) |
| P1-08 | [PR #27 Reader limitations](https://github.com/vjmichael/Lu-AI-GUI/blob/c716876aed0276e98cb372ccd2cd16c916c5c00d/docs/LU_READER_CLOSEOUT.md#L15-L74) | [desktop ask Reader](https://github.com/vjmichael/Lu-AI-GUI/blob/c716876aed0276e98cb372ccd2cd16c916c5c00d/lu-desktop/src-tauri/src/reader_desktop.rs#L84-L209) |
| P1-09 | [host route admissibility](https://github.com/vjmichael/Lu-AI-GUI/blob/6645f7939b60700c4012d3a2a18b7553480980a6/src-tauri/lu-core/src/capability_router.rs#L1-L19) | [draft DLC host API](https://github.com/vjmichael/Lu-AI-GUI/pull/11/files) |
| P1-10 | [typed error response](https://github.com/vjmichael/Lu-AI-GUI/blob/6645f7939b60700c4012d3a2a18b7553480980a6/src-tauri/lu-core/src/model.rs#L225-L280) | [Reader failure codes](https://github.com/vjmichael/Lu-AI-GUI/blob/c716876aed0276e98cb372ccd2cd16c916c5c00d/src-tauri/lu-core/src/native_reader_tool.rs#L78-L90) |
| P1-11 | [exact edit proposal](https://github.com/vjmichael/Lu-AI-GUI/blob/6645f7939b60700c4012d3a2a18b7553480980a6/src-tauri/lu-core/src/reversible_edit.rs#L25-L134) | [draft native edit journal](https://github.com/vjmichael/Lu-AI-GUI/blob/2a9a135500b6e142564b7dec753f52f5184fd9bb/src-tauri/lu-core/src/native_edit_existing.rs#L323-L510) |
| P1-12 | [model-turn audit](https://github.com/vjmichael/Lu-AI-GUI/blob/6645f7939b60700c4012d3a2a18b7553480980a6/src-tauri/lu-core/src/model_turn_journal.rs#L1-L35) | [passive diagnostics](https://github.com/vjmichael/Lu-AI-GUI/blob/6645f7939b60700c4012d3a2a18b7553480980a6/src-tauri/lu-core/src/passive_diagnostics.rs#L114-L153) |
| P1-13 | [benchmark contract](https://github.com/vjmichael/Lu-AI-GUI/blob/6645f7939b60700c4012d3a2a18b7553480980a6/src-tauri/lu-core/src/benchmark_contract.rs#L1-L64) | [real GGUF test ignored](https://github.com/vjmichael/Lu-AI-GUI/blob/6645f7939b60700c4012d3a2a18b7553480980a6/src-tauri/lu-core/tests/real_gguf_tool_json.rs#L106-L108) |
| P1-14 | [draft AGENTS orientation](https://github.com/vjmichael/Lu-AI-GUI/blob/c67d5f7abdaa1427102d0c9ad8ae98f19cf0bc5a/AGENTS.md#L1-L67) | [accepted alpha report](https://github.com/vjmichael/Lu-AI-GUI/blob/6645f7939b60700c4012d3a2a18b7553480980a6/docs/LU_ALPHA_01_ACCEPTED_BASELINE_20261103.md#L1-L38) |
| P1-15 | [chat-title-only matcher](https://github.com/vjmichael/Lu-AI-GUI/blob/6645f7939b60700c4012d3a2a18b7553480980a6/lu-desktop/web/chat_ui_behavior.mjs#L25-L34) | [ANSF searchable chapters](https://github.com/vjmichael/Lu-AI-GUI/blob/6645f7939b60700c4012d3a2a18b7553480980a6/src-tauri/lu-core/src/ansf_library.rs#L305-L370) |
| P1-16 | [conversation-bound authority](https://github.com/vjmichael/Lu-AI-GUI/blob/6645f7939b60700c4012d3a2a18b7553480980a6/src-tauri/lu-core/src/conversation_authority.rs#L1-L96) | [draft ledger host success method](https://github.com/vjmichael/Lu-AI-GUI/blob/c67d5f7abdaa1427102d0c9ad8ae98f19cf0bc5a/src-tauri/lu-core/src/operational_memory.rs#L291-L330) |
| P2-17 | [model tool limit and catalog](https://github.com/vjmichael/Lu-AI-GUI/blob/6645f7939b60700c4012d3a2a18b7553480980a6/src-tauri/lu-core/src/model.rs#L20-L53) | [allowlisted tool schemas](https://github.com/vjmichael/Lu-AI-GUI/blob/6645f7939b60700c4012d3a2a18b7553480980a6/src-tauri/lu-core/src/model.rs#L174-L195) |
| P2-18 | [main desktop UI](https://github.com/vjmichael/Lu-AI-GUI/blob/6645f7939b60700c4012d3a2a18b7553480980a6/lu-desktop/web/main.jsx#L467-L530) | [PR #27 background/Reader UI](https://github.com/vjmichael/Lu-AI-GUI/pull/27/files) |
