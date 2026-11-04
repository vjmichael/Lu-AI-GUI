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
