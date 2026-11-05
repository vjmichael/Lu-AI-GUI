# Lu v1.0 design reconciliation and implementation gates — 2026-11-03

## Sources and precedence
Reviewed original `Jan-Mod-Design-Document.md` (Oct 21–22), Lu Core `DESIGN_GAP_AUDIT_2026-10-25.md`, `MEMORY_AND_MEDIA_ROADMAP.md`, independent desktop docs and subsequent Lu decisions. The old Jan document is historical, **not automatically a current requirement**. The accepted changes below supersede it:
- Independent Tauri/React app + Rust Lu Core, pinned llama.cpp backends (Vulkan first). Jan data import only, not Jan runtime.
- Authenticated per-user-turn grants and non-admin OS containment; **reject** old 'unrestricted trusted-local tools' proposal.
- Native first-party tools with Protobuf host/worker contracts; MCP optional.
- ANSF + SQLite, immutable original conversation records with revision/link/provenance, mutable semantic/object memory with user inspection.
- Single installer; Github repository only; Windows physical platform first.
- Cognitum optional addon. Domain-specific IDE/Film/Research/Writing DLCs are after core v1.0, not prerequisites.
- Alpha `0.1.0-alpha.2` accepted as functional development baseline; known defects tracked separately.

## v1.0 capability gates (do not equate 'implemented in library' with 'integrated and accepted')
| Gate | Scope | Objective acceptance |
| --- | --- | --- |
| V1-01 | Windows shell and distribution | Taskbar/app icon and identity; responsive model load; measured import progress; installation/upgrade/uninstall preserve data; signed distribution plan; Win10/11 physical tests |
| V1-02 | Model runtime and profiles | Pinned engine verification; GGUF+MMProj import/load/unload; Vulkan/optional ROCm device-layer and memory diagnostics; resource-guardian preflight and runtime recovery; recoverable failures; per-model inference profiles; model catalog |
| V1-03 | Durable conversation and ANSF | Immutable originals, editable revision relationships, chat search/folders, image/document refs and recoverability; restart/replay; export/import/backup/migration |
| V1-04 | Native tool host authorization | Trusted persisted human turn identity; scoped grants/expiry/revoke; typed small model-facing schemas, Protobuf boundary; untrusted outputs never grant new authority |
| V1-05 | Worker execution containment | Host-supervised worker lifecycle, cancellation and kill/cleanup; Windows Job Object support where appropriate; keep resource reliability distinct from tool authority and any separately justified OS isolation; Linux parity later |
| V1-06 | First-party toolset | Read/search/write with atomic file effects, safe move/rename/trash/restore, git, archive, validated JSON; optional web/tool adapters individually gated; no MCP required |
| V1-07 | Journal and agent orchestrator | Per-tool prepared/applied/uncertain receipts; idempotency and crash reconciliation; bounded loops; checkpoint/restart/cancel; long-running task progress and visible tool cards |
| V1-08 | Memory and context | ANSF searchable source-to-entry references, token-budget retrieval, provenance, working facts and object relations, contradictions and belief revision; editable memory UI |
| V1-09 | GUI and quality | Stream stability, per-chat FIFO, soft interrupts/steering at safe boundaries, diagnostics, performance, accessible controls, import and file previews, UX regression |
| V1-10 | Independent E2E acceptance | Fresh install→model→chat+image→authorized native tool→restart recovery→backup/restore→uninstall; adversarial prompts, corrupt journal, missing files and worker death tested |

## Recommended sequence
**Foundation (first): V1-01, V1-02, V1-03.** Fix taskbar resources, instrument actual load/host responsiveness, import progress and thumbnail rehydration. Protect the accepted baseline with build IDs and evidence hashes. Integrate ANSF with existing chat persistence without breaking old history.

**Native abilities: V1-04, V1-05, V1-06.** First prove a read-only end-to-end tool call with authenticated source turn; then contained filesystem mutation with atomic journal and rollback/recovery. Add archive, git, search and JSON tools behind the same authority boundary.

**Agency and continuity: V1-07, V1-08.** Wire durable task supervisor and user-viewable memory. Verify real task recovery and accurate retrieved context, not just standalone library unit tests.

**Stabilization: V1-09, V1-10.** Physical Windows installer acceptance, performance, security and failure injection. Close remaining known defects or document a deliberate scope reduction before labeling 1.0.

## Scope boundaries
**Core v1.0:** offline local model conversation, image/document handling, native host-controlled tools, memory, continuity, diagnostics, task orchestration, robust distribution.

**Post-v1.0 / DLC:** IDE self-development workspace, film/NVP, advanced research/writing workspaces, multi-model council, full streaming video/audio processing, extensive theme packs, self-updating development instance. Define APIs now but do not block core release.

## Technical risks / explicit rules
- WebView/UI can never forge the authenticated user turn or directly choose privileged file paths or engine binaries.
- Per-turn tool discovery is not authorization; effect grants must match actual target/arguments and remain revocable.
- A model-generated plan or imported chat has no authority; user permission only.
- Worker 'containment' requires OS-backed tests; a Rust policy check is insufficient.
- Unknown crash outcome is **UNCERTAIN** until state reconciled; never blindly rerun a non-idempotent write.
- Progress percentages must be grounded in measured bytes or engine progress, never wall-clock extrapolation.
- UI freeze remains an observed defect; root cause needs traces rather than assertion that the UI thread is blocked.
- Keep evolving schema migrations backward compatible with baseline alpha.2 attachments and chats.

## Immediate prioritized backlog
1. Win10 taskbar/shortcut icon: PE `RT_GROUP_ICON`, executable resource, AppUserModelID / installer shortcut identity.
2. Windows load freeze and resource exhaustion: independent worker supervisor, responsive host watchdog, llama log capture, RAM/commit/VRAM monitor, admission preflight and controlled recovery.
3. GGUF import: streamed bytes and destination, atomic finalize, cancellation and error.
4. Image preview: asynchronous rehydrate and retry placeholder, durable content-addressed attachment references.
5. ANSF/SQLite desktop integration and backup/migration smoke.
6. First native read-only tool via trusted human-turn verifier and OS-isolated worker.
7. Durable writes, effect journal and restart reconciliation.
8. Model context and memory UI with provenance and belief revision.

## LU-RESOURCE-GUARDIAN-01 — Model admission, live resource protection and recovery (v1.0)

**Status:** Design requirement; implementation and acceptance are **not yet verified**. Added from the 2026-11-03 Windows LM Studio experiment with AMD Radeon AI PRO R9700 (approximately 32 GiB dedicated VRAM), 32 GiB host RAM and Qwen GGUF+MMProj. ROCm and Vulkan both loaded and generated; a high-memory ROCm attempt reported allocation failure and a subsequent inference-settings experiment rendered Windows effectively unresponsive before Task Manager could be opened. The exact freeze cause was **not established**; CPU/GPU dispatch warnings and memory pressure are hypotheses/diagnostic evidence, not proof of causation. One-run throughput comparisons are preliminary and not a backend ranking.

### Principle and boundary
- Lu has broad host **access**, but access is **not authority**. This guardian protects availability and data durability; it does **not** sandbox Lu, curtail a user's explicitly authorized tools, or substitute for per-turn effect authorization.
- Model inference workers are expendable; the desktop UI, journal and lightweight supervisor must remain independent. The guardian is host-side, not implemented solely within the potentially hung inference worker. A Windows Job Object may assist child-process cleanup; it is not, by itself, a GPU VRAM or host-commit quota.
- Default to a conservative, explainable policy. Advanced users may deliberately relax resource limits with explicit, persistent choices; never promise immunity to driver hangs, Windows paging or hardware faults.

### Admission preflight (before starting or resizing a model)
- Inspect verified GGUF tensor sizing, quantization, chosen backend/device and GPU layers, MMProj, model/runtime overhead, target context, KV-cache format/location, attention mode, eval/physical batch, speculative draft buffers, parallel slots, concurrent models and existing host usage.
- Query current *available* physical RAM, available commit headroom, dedicated/budgeted VRAM and shared-memory pressure as supported by Windows APIs/backend telemetry. Distinguish physical RAM, commit, dedicated VRAM and shared GPU allocations; do not add RAM and VRAM into a single misleading free-memory figure.
- Model **peak** load and inference allocations, not just the on-disk GGUF or initial model weights. Record uncertainty/calibration margin. If estimates or telemetry are unreliable, report that fact, choose a more conservative default, and offer a user-directed adjustment rather than silently risking overcommit.
- If admission is unsafe, refuse **automatic** load and offer concrete choices: reduce context/parallelism/batch/offload, unload another model, choose another backend or explicitly override. Changes to model settings require separate approval where needed; never silently switch inference behavior or lose an already working profile.

### Live monitoring and graded intervention
- A small host supervisor samples memory availability, commit utilization, per-process private/working-set usage, GPU memory budgets when exposed, backend allocation failures, worker heartbeat, UI responsiveness and cancellation latency. Persist bounded, timestamped diagnostic samples outside the worker so the most recent evidence survives its crash.
- Define state transitions **Healthy → Warning → Critical → Recovering → Healthy/Failed** with hysteresis and configurable thresholds based on available headroom and rate of depletion. Fixed percentage thresholds alone are insufficient; initial RAM/VRAM reserves are hypotheses requiring hardware calibration.
- **Warning:** surface measured headroom and why a configuration is risky; inhibit additional automatic loads and new high-memory requests. **Critical:** stop admitting new work, request a bounded graceful cancellation/unload and, when necessary, terminate the model worker through its host-controlled process lifecycle. Never depend on Task Manager or the worker UI to remain responsive.
- Detect stalled supervision and bounded cancellation failures; enforce timeouts without retry storms. Prefer graceful recovery, then verified forced cleanup when the worker is unresponsive. Do not claim a guaranteed recovery from a kernel-mode GPU/driver hang; log the last durable event so a subsequent launch can diagnose it.
- Preserve completed streaming tokens, prompt/chat attachments, pending queue and durable turn state. On worker death, reconcile journal outcomes as **completed / cancelled / failed / uncertain**, and do not replay non-idempotent native effects without reconciliation.

### Backend and profile behavior
- Track ROCm/HIP and Vulkan as separately verified Windows backend candidates; retain Vulkan as known working fallback. Record actual loaded backend, per-layer offload, fused-op CPU fallback, Flash Attention effective mode, MTP/speculative-decoding mode and GPU/CPU memory placement, instead of showing only 'GPU enabled'.
- Distinguish **selected settings** from **effective runtime behavior**. An unsupported fused Flash Attention path assigned to CPU does not prove the whole attention workload executes on CPU.
- Keep per-model/per-backend **last-known-good** profiles with provenance and measured peak use. Never automatically repeat a configuration that previously caused OOM/hung-worker recovery without user confirmation. Recover to the previously validated profile after a failed experiment; report changes clearly.
- Offer Balanced (default), Strict and Advanced/Custom policies in the model manager. Numeric reserves and warning thresholds must be configurable and empirically validated, not hardcoded to illustrative values. Resource policy is separate from security/authorization policy.

### Acceptance gates and failure-injection fixtures
1. **Preflight:** low-RAM, low-commit, low-VRAM, oversized context, four parallel slots, large physical batch, MMProj plus model, concurrent model loads and backend-switch cases are estimated and either admitted with bounded peak headroom or rejected with precise reasons; verify no false assumption that VRAM equals available host RAM.
2. **Runtime:** induce growth in KV cache, speculative draft allocation and backend allocation failure; prove independent supervisor can still log, block new loads and cancel/terminate a cooperative or hung worker. Measure host responsiveness and time to detect/recover; thresholds are acceptance-tuned on real hardware.
3. **Durability:** kill worker mid-stream and during tool interaction, restart desktop, and verify chat/attachment integrity, retained partial output, queue state, journal reconciliation, and no orphaned inference workers; never re-execute uncertain external effects blindly.
4. **Compatibility:** repeat on Windows with ROCm and Vulkan where supported, CPU fallback, GGUF+MMProj, 32K context and an intentionally overcommitted configuration; test backend fallback and restored last-known-good profiles. Test with telemetry unavailable and with unexpected driver errors.
5. **Observability:** show user-readable 'why refused/why cancelled' diagnostics with timestamps, backend, settings, measured headroom and final recovery outcome; protect private prompt content from diagnostic logs by default.
6. **Limitations:** explicitly document that OS scheduler starvation and GPU-driver stalls may be unpreventable. The v1.0 gate is *measurably lower risk and recoverable worker failures*, not an impossible promise that Windows can never freeze.

**Roadmap mapping:** V1-02 owns admission estimation, backend/profile telemetry and model UX; V1-05 owns supervised worker termination and cleanup, without redefining authority; V1-07 owns durable recovery receipts; V1-09 owns responsive alerts, last-good controls and diagnostics; V1-10 owns physical Windows failure-injection acceptance. This is a **v1.0 release gate**, not a post-v1.0 DLC.


## LU-AIW-01 — AI-accessible workspaces and operational continuity (2026-11-05)

**Status:** New design and acceptance requirements; not a claim of implementation. Applies to Lu Core first, and later to optional IDE, Film/NVP, Research and Writing DLC Workspaces. This refines V1-03/04/06/07/08/10; the complete multi-workspace protocol remains post-v1.0 and must not delay core stabilization.

### Objective and invariants

Lu must help humans achieve results without repeatedly rediscovering resources, project decisions, tools, or operating procedures. Its host must connect human intent to (1) precise source references, (2) discoverable capabilities, (3) verified available resources, (4) relevant context, (5) bounded execution, (6) evidence of success and (7) rollback/recovery.

**Persistent memory alone is insufficient.** Storing a fact does not guarantee the model will retrieve it, recognize its relevance, or check whether it is still true. Critical task categories must trigger lightweight **host-side** preflight and contextual retrieval rather than relying on a transformer's spontaneous memory. Remembered claims and discovered tools never grant execution authority.

### R1 — Precise, revision-aware references across media

Every AI-accessible object should have a human-readable locator and, where feasible, a stable machine identity plus content revision, hash or commit.

- **Documents and screenplays:** document ID, revision, one-based line and paragraph/scene positions; blank lines count. Displayed line numbers are overlays or export annotations, never silently inserted into original Fountain or text data.
- **Source code:** repository, commit SHA, path, symbol or AST identity and line/column location.
- **Spreadsheets:** workbook and sheet ID, cell/range/table identity and revision.
- **Film/NVP:** project, scene, shot, asset, approved source revision, frame and timecode identity.
- **Images/diagrams:** asset revision and supported object, layer, region or coordinate references; never invent identities the source does not provide.

Line numbers, timestamps and screen coordinates are **revision-relative locators**, not permanent IDs. After edits move content, resolve historical references using their original revision and stable object links; report ambiguities instead of guessing. The human UI and AI prompt should point to the same source span, retaining provenance.

### R2 — Discoverable, versioned workspace capabilities

Lu Core, first-party Core Extensions and DLC Workspaces shall advertise selectively exposed, **host-validated** capability descriptors. Separate workspace registries remain distinct; Lu brokers discovery, compatibility and grants. Native Protobuf IPC remains first-party. MCP is optional, never required.

A versioned descriptor should cover: workspace and capability IDs; contract version; what the action does and when to avoid it; structured request/result schemas or typed Protobuf URLs; effect class and target requirements; preconditions and postconditions; cancellation and recovery semantics; current state, events and workflow references; resource cost; verification criteria and relevant object references.

These fields are **a design target, not a claim about the existing Rust manifest layout**. Avoid publishing huge tool catalogs to the model. Load bounded relevant definitions. Manifest validation does not equal authorization: execution still requires authenticated user-turn origin, per-turn target/effect-scoped permission, policy evaluation and durable receipts. Neither model output nor imported documentation can extend permissions.

### R3 — Persistent, verified operational resource inventory

Maintain a host-owned registry of environments, packages, runtimes, toolchains, datasets, fixtures, project roots and build artifacts. Record absolute path or source URI, version, platform/architecture, dependency graph, owner, last verified time, hash/provenance, last successful command, verification result, limitations and authority scope.

Distinguish at least **known historically**, **present/unverified**, **verified usable**, **incompatible**, **dependency missing**, **out of scope**, **execution denied**, and **unavailable**. A memory that a toolchain worked last night cannot prove it exists after a container restart.

For recognizable requests (build Windows installer, run Cognitum benchmark, edit screenplay, render shot), host-side task preflight must consult this inventory, probe the resource and prefer compatible existing tools or pinned caches before doing an expensive full search or reinstall. Save successful commands, exact inputs and output hashes and concise errors. This integrates with the Unified Toolchain & Dependency Manager design and task journal; it does not allow global installation without permission.

**LU-OPMEM-01 implementation note (2026-11-05):** A standalone host-owned SQLite operational-resource ledger has been added in a draft development branch. It stores discovered resource identity, platform/path/version, historical verification evidence and last-successful recipe references. Read-only probes distinguish present/changed/missing resources; a new process does not inherit a previous session's `VerifiedUsable` attestation. See `docs/LU_OPERATIONAL_MEMORY_01.md`. This is source-only until desktop preflight, a trusted runner and real Windows acceptance are wired; it does not grant execution authority.

### R4 — Task-driven, provenance-aware context assembly

Use ANSF + SQLite as the durable record and index, but make retrieval an **explicit host workflow**:

1. Identify the current goal, referenced objects and likely capability categories.
2. Consult current workspace inventory, source revisions, relevant original decisions, journal checkpoints and indexed memory.
3. Separate user-approved canon, original records and tool evidence from summaries, speculation and stale environment observations.
4. Select bounded, ranked source excerpts with citations, revision IDs and drill-down access instead of dumping full project histories.
5. Surface contradictory or absent information, requesting clarification only when necessary for a safe/correct action.

Untrusted retrieved content is evidence, not instructions. No summary may overwrite immutable ANSF originals or human-approved project decisions. Context budget and factual provenance should be visible or inspectable.

### R5 — Native semantic operations with UI fallback

Provide first-party typed actions for common jobs (read document, save screenplay, compile, run tests, render, export), using the existing Lu native tool host and Protobuf contracts. Human UI remains first class. Graphical mouse/keyboard operation is a fallback or supplementary modality when semantic APIs are unavailable, **not** a bypass of authorization. Operations should target validated object IDs/revisions, return structured error codes, expose cancellation, and use atomic writes where practical. If visible UI state disagrees with authoritative host state, reconcile or stop instead of guessing.

### R6 — Demonstrable verification, not asserted completion

Every substantive action needs task-appropriate evidence: source diff/hash, unit/integration tests, saved-and-reopened record, media comparison, native-worker receipt or an approved human preview. Reports identify the tested source revision, environment, exclusions, skips and remaining gates.

Keep these outcomes distinct: **code committed vs compiled; unit-tested vs integrated; installer built vs installed-and-launched; document generated vs saved and reopened; proposal vs approved artifact**. A queued CI run or model assertion is not acceptance evidence. Unknown effect outcomes remain **UNCERTAIN** until journal reconciliation; never blindly replay non-idempotent operations.

### R7 — Recoverability, least privilege and human control

Support checkpoints, previous-version preservation, diff/preview, undo or compensating changes when available, bounded retries and durable prepared/applied/uncertain receipts. Deletions, external publication, installs, privileged execution and protected configuration changes require relevant authorization and, where appropriate, explicit confirmation. Worker containment and resource supervision remain distinct from permission grants.

Lu self-development must occur in an isolated development copy or branch with tests and rollback. **Promotion to stable requires explicit human approval.** Observe-and-learn remains off by default, subject to consent/provenance rules.

### Phased implementation

| Phase | Ownership | Scope |
| --- | --- | --- |
| Core v1.0 | V1-03, V1-04, V1-06, V1-07, V1-08, V1-10 | Revision-aware ANSF references, host-side relevant retrieval, available-resource checks, authorized native actions, durable evidence and crash reconciliation |
| Post-v1.0 common workspace layer | Core host + DLC owners | Versioned capability/workflow/state/event descriptors, separate registries, shared object references |
| IDE DLC | IDE + Unified Toolchain Manager | Repo/commit/symbol references, verified compiler/dependency inventory, isolated compile/test/debug and human-approved Lu self-updates |
| Film/NVP DLC | Film workspace | Screenplay lines and scene IDs, shot/asset/frame/timecode references, approval lineage, render/edit/QC and dependency invalidation |
| Subsequent optimization | Cross-workspace benchmark | Precision, token budget, latency, permission and recovery improvements based on measured outcomes |

Do not force post-v1.0 workspaces into the v1.0 release criteria. Interfaces can be sketched and independently developed before integration is accepted.

### Mandatory behavior/acceptance tests

1. **Windows toolchain rediscovery:** In a fresh session, request an installer after a toolchain was previously verified. If present, reuse it without unnecessary downloads. After deliberate removal, correctly detect missing files and reconstruct only from approved sources.
2. **Cognitum benchmark continuity:** Reopen a benchmark with saved libraries, fixtures and prior measurements; preserve identical data and version assumptions, and report missing or changed inputs.
3. **Source precision:** Human and AI both refer to the same screenplay line/scene or code span; preserve blank lines and CRLF; use original revision context after inserts/deletions without corrupting source.
4. **Stale/conflicting memory:** Reject old success claims when current health verification fails; superseding human decisions override older summaries.
5. **Discovery vs permission:** Discover a DLC tool, then verify that unauthorized invocation is denied and unloading the DLC does not destabilize Core.
6. **Crash recovery:** Kill an active worker across prepare/apply/publish boundaries; reconcile effects, retain original data, and prevent duplicate execution.
7. **Real outcome:** A Windows build counts only with appropriate installed-app smoke; a document revision counts with saved-and-reopened content; record incomplete acceptance honestly.

Benchmark **real task completion**, wasted rediscovery/downloads, retrieval precision and source provenance, false-positive resource readiness, compute/tokens/time to result, permission violations, recovery success and human clarification burden. Compare host preflight on/off while holding model, hardware, inputs and task difficulty consistent; a trivia memory quiz is not sufficient.

**Principle:** Make software easier for AI to understand and operate. Efficient references, discoverable actions, verified resources, relevant context, robust evidence and reversible effects should reduce intelligence wasted merely figuring out where to begin.
