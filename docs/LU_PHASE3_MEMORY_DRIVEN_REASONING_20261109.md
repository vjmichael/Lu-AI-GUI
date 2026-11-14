# Lu Phase 3 — memory-driven planning and execution (Nov 9, 2026)

Scope: Phase 3 only. Implements the primary missing path from ANSF sources through real Lu GGUF planning and separately reviewed native Git/FFprobe operation. Phase 4 and Phase 5 remain separate development.

## Source-of-truth integration

- **Goal and clarification**: before Lu calls the real model in explicit Plan mode, the host requires the current authenticated human chat turn, reconstructs its ANSF GoalSnapshot from immutable original events and rejects pending/deferred questions. The existing exact goal/revision source gate still checks stage_completed_model_plan.
- **Source-backed procedures and belief revision**: the new agent_memory_reasoning module consults Lu's existing M4 procedural evidence and assessed claims for two closed existing native tasks: reviewed Git HEAD and media facts. Original ANSF source IDs, historic VERIFIED status, contradicted/retracted/stale statuses and actual journal provenance are read; their model-facing projection contains only bounded status and source IDs, no untrusted stored instructions.
- **Planning**: this projected memory is appended to the actual supervised model prompt in review-only Plan mode. Host recomputes original source evidence after inference and rejects a draft if memory/goal/review status changed during inference, rather than promoting an outdated proposed plan.
- **Native step**: before each *new*, fresh-human-authorized native reviewed operation, host rechecks the current original ANSF goal/questions and method-specific belief/procedure sources. Any stale, contradicted, retracted or unverified claim requires human review. Only existing approved tool registry and original per-step broker may act; historic procedures are never silently re-executed or interpreted as user permission.
- **Procedural learning**: after a genuinely verified Git/FFprobe worker operation, the already-existing reviewed_memory_evidence::record_from_reviewed_turn path atomically publishes native evidence projections into ANSF procedures/beliefs, with source/recovery guards. Phase 3 consumes those in subsequent plan/review decisions; it does not create a second procedural database.

## Important implementation limitations

This release supports the existing reviewed Git-head and media-facts *memory selectors* only; other native tools need a registered method-specific provenance reader and codec. Historic evidence validation may block when the original journal is unavailable; it does not silently downgrade to "model verified." Planning's lexical/hybrid cross-chat recall remains candidate data, not independent current proof. Procedural history never mints authority, and model selection competence must still be assessed with real 7B+ model output.

## GGUF and installer policy change

The historical Linux Qwen3-0.6B test group is DEFERRED behind explicit repository variable `LU_ENABLE_OLD_0_6B_DIAGNOSTIC=true`; it is not the release or phase-completion blocker. The genuine 27B model test is performed by the owner on their AMD Radeon AI PRO R9700, using `scripts/run-qwen38-27b-local.ps1` and a current-source isolated NSIS preview. **Do not substitute a previous AGENT-01G CPU demo or alpha installer.**

New packaging path: `scripts/build-current-windows-nsis.ps1` with pinned llama.cpp `e0f142d5ddc1461ce721644d92af33db23545105` (qwen35/MTP capable) + production Lu Phase 3 source, preview version `0.2.0-rc.1`, isolated product identity `org.lu.current.devpreview`, NSIS `lu-desktop/packaging/NSIS_CURRENT_DEV_PREVIEW.nsi`, one source-verification/release-content ledger. Windows hosted source/build/install result and actual installer SHA must be checked before claiming success. This exact hosted NSIS includes **CPU baseline**, not proven Vulkan; local Vulkan engine compilation and independent real model response are separate, not a replacement for Lu app's pinned backend until Lu is separately rebuilt/repackaged with it.

The physical-machine handoff should record:
- Exact Lu installer/app SHA, model SHA and source.
- Engine SHA, Vulkan device enumeration, actual layer-offload lines, context and MMProj.
- Model output, reasoning/tool choice and host independent Git/media witness (if exercised).
- Native approval/refusal, restart/crash recovery; unknowns reported as NOT_TESTED, never assumed PASS.

## Next engineering phase

Phase 4: desktop/workspace UX, interaction, progress, model management, GPU/backend selection and recovery polish. Phase 5: installed-app upgrade, distinct-version rollback, release hardening. No Phase 4/5 source implementation authorized in this phase.
