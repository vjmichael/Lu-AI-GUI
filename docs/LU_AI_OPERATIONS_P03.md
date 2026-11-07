# P03 — AI-OPS-01: AI operations and model/provider management

**Owner authorization:** 2026-11-06, after P02. **Stack:** `lu/p03-ai-ops-model-manager-20261106` on the P02 PR #67 source head `b69e19658b7cb1761180cdeb94aac7d9e7c3dc9d`. **P04 not authorized.**

## Owner-directed P02 deferral

The following independent, previously open P02 release gates are **DEFERRED BY OWNER**, not passed: (1) physical Windows NSIS install/uninstall, restart and installed GUI acceptance; (2) genuine physical AMD backend GPU/VRAM and GGUF/MMProj inference/offload acceptance. All P03 work must respect that deferred boundary: no fake GPU readiness, no simulated model proof, and no silently enabling background GPU workers.

## Existing implementation reused

- Lu Core: `native_capability_inventory`, `conversation_authority`, `capability_router`, `ModelToolCatalog`, `NativeToolTurn`, Protobuf IPC and effect journals; none can be bypassed by AI-OPS ranking.
- P01: scoped file discovery and read tools, with their existing per-turn native picker and permission boundary.
- P02: durable BackgroundScheduler + Resource Guardian, with zero default GPU/VRAM/network budget and host-owned worker lease/cancellation.
- Models: local opaque-ID per-model folders, `ModelAdmissionManager`, GGUF preflight, explicit MMProj import/recheck, pinned `PersistentLlamaBackend`, unprivileged desktop selection and existing backend selection contract (`compute_runtime`).
- Chat: typed, journal-derived bounded recent messages; interrupted assistant text is never treated as a completed model reply. No second model or required online provider.

## P03 source implementation: first integrated pass

1. **Current-user native tool preference:** `ai_operations::recommended_read_tools` only orders names that exist in the already registered, authorized read-only inventory for the exact current human prompt, capped at six; it rejects explicit "without tools". Workspace/Git supervised turns append only an inert preference hint. Tool schemas, per-turn authority, review and native Protobuf execution remain unchanged. This is a preference, not automatic permission, a replacement LLM or semantic routing benchmark.
2. **Source identity reuse primitive:** `SourceReuseIndex` stores at most 64 metadata-only (opaque chat, session and source ID + exact SHA-256 + epoch) records in RAM. A caller must independently reverify the current bytes/epoch; a changed source invalidates reuse. It retains no full file paths, excerpts, tool grants, executable commands or credentials. **Not yet attached to Reader/Workspace results**, hence there is no claimed reduction in repeated work or persistence across restarts.
3. **Native local model inspection and Settings:** the registered model ID (never a WebView-provided OS path) resolves to Lu's model root. The host preflights current GGUF header and native MMProj file identity, displaying file size, header version/tensor count and explicit status. A missing/changed GGUF or claimed MMProj blocks model load *before* the actual supervised backend call. The Settings pane supports local filename search and displays model/backend provenance. A header **cannot certify architecture, tokenizer, GPU offload, context window, license, real inference or VRAM**; `runtimeCompatible=false` by construction.
4. **Provider review contract:** `review_provider_candidate` accepts only explicit `hugging_face` (not inferred "gf"), immutable 40-hex revision, safe `owner/repo` + `.gguf` filename, exact 64-hex SHA-256, bounded byte size and affirmative license/access checks. This is **inert metadata validation**, not a download grant. Network search/download, credentials, ETags/ranges, pause/retry, staging and publication remain disabled until explicit provider/user consent and P05 transfer integration.
5. **Backend and Guardian:** the existing compute backend selection and verified-engine scoring contracts remain. Do not wire a new driver or set P02 GPU/VRAM reservations from hints or WDDM counters; zero remains the safe value pending deferred physical proof. Windows pinned llama.cpp requests Vulkan, Linux native baseline is CPU; neither is automatically considered measured offload.

## Validation and open work

**Authored:** native Core tests for read-authorized hints, chat/session/source epoch invalidation, immutable provider candidate review; Windows host model-admission regression; desktop source/UI contracts; focused Windows/Linux GitHub CI `.github/workflows/lu-p03-ai-ops.yml` including Windows full host/React build without fake packaged inference engine. **Source added does not equal native/installed pass**; exact-head CI must report success before a verified claim.

**Remaining P03 work:** bind exact source reuse to verified Reader/Workspace receipts without regranting filesystem permissions; richer GGUF metadata/architecture/quantization compatibility and real backend diagnostics; explicit online model-provider selection/search UI and a fully supervised, license-reviewed immutable download+resume plan through P05 Transfer; host-observed GPU/VRAM admission only after deferred physical testing. No automatic provider fallback or model-side download. These are not "done" merely because an inert review contract exists.

**Next ranked package:** P04 DOC-STREAM-01 — Unified Reader/OCR. **Stop before P04 pending owner instruction.**
