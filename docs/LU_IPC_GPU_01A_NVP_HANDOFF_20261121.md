# LU-IPC-GPU-01A — Core-owned NVP diffusion GPU lease
Date: 2026-11-21. Status: source candidate, installed handoff BLOCKED.

## Cross-repository review
Reviewed Lu Core PR #244 at 5c010047a78e4141fe1f800cf9c21a85189ec0ec and NVP canonical PR #161 at d5754ed7e32294b9dd6b0b01a9ded0fed0fd2016. NVP's backend/nvp_gpu_observation.py measures Windows WDDM PDH / DXGI resource counters, but labels those as read-only, not admission or custody. NVP owns its LTX diffusion workers and media task ledger. Lu owns model residency, original user-turn authority, global GPU resource arbitration and resuming selected GGUF/MMProj.

## One public IPC contract
The single Lu-owned lu.workspace.v0 Protobuf schema is extended additively (WorkspaceFrame tags 38, 39, 40) with GpuLeaseRequest, GpuLeaseOutcome and GpuLeaseRelease plus feature-negotiated service semantics. This does NOT replace ModelLease (inference/context) or the existing ResourceTransitionRequest. NVP's currently pinned OLD schema Git blob 4e01d921197fb1c58c99958479b1cede36e13c9a must be explicitly reviewed/re-pinned to the newly accepted Lu schema before enabling gpu-lease-v1. The original v0 major 1/minor 0 wire stays backward compatible. The private Core↔SYSTEM SCM control pipe must NEVER become a DLC GPU endpoint.

## Required handoff
1. NVP requests budget, device and bounded duration under its claimed original operation. Only a separately OS/package-attested Core host with trusted conversation, user turn, task, session and workspace may authorize. Core mints original lease ref. A Protobuf OriginRefs is never authority.
2. Lu serializes the exclusive host-wide GPU lease and checks whether active inference can drain. Deferred is appropriate when another user's generation is running. No forced interruption or fabricated free VRAM.
3. Core atomically closes NEW model load/generation admission via TrustedInferenceSupervisor GPU fence, preserves selected model/session/approved MMProj and fsyncs SQLite DRAINING intent before actual supervised model unload.
4. Core waits for actual supervisor unloaded/process-gone state and TWO increasing driver OS capacity observations sufficient for the requested VRAM. Only then persist GRANTED and send GPU_READY_FOR_NVP; an unload request is never an acknowledgment.
5. NVP produces film using its own native supervised diffusion workers while Lu Core, IPC, journals, deterministic tools and UI remain running. Lu may queue LLM turns while its model is not resident.
6. NVP sends an ALL_CLEAR/release request with lease ID and optional original worker stop locator. Core independently verifies original installed NVP worker group empty, re-samples the same GPU and checks the exact original lease, user turn and epoch. NVP's text or telemetry alone does not establish proof.
7. Core durably transitions RELEASING -> RESTORING, reloads previously selected GGUF/MMProj only if safe, verifies model readiness, reopens inference admission and marks RESTORED. Duplicate exact release is idempotent; wrong identity or fake worker receipts are denied.

## Failure/restart and host scope
The initial contract enforces ONE exclusive GPU resource lease for the whole Lu host, even across claimed different device IDs; multi-GPU concurrency waits for real per-device model residency evidence. SQLite uses WAL + synchronous FULL, exact scope binding, replay refusal and durable state. On startup every unfinished lease becomes QUARANTINED and recovery must assert model-admission hold BEFORE accepting new generation. Lease expiry, disconnected clients, model-process death, Vulkan/ROCm resource loss and missing capacity witnesses never automatically release or replay an effect. Unknown outcome stays blocked. The original film effect's result receipt remains separate from the resource-lease receipt.

## Implemented source candidate and blockers
CoreGpuLeaseManager and HostModelControl adapter in src-tauri/lu-core/src/gpu_resource_leases.rs; existing real inference supervisor extended with GPU admission fence; additive Protobuf schema plus canonical wire tests. The host-injected OriginalTurnAuthority, HostGpuObserver and NvpWorkerStopVerifier are strict extension seams, NOT working physical installed providers. Source-only fixture tests must not be misrepresented as independent GPU/worker custody.

Open: real Core-installed IPC gateway binding + original ConversationAuthority + installed OS signer, physical Windows R9700/AMD VRAM sampler, live NVP signed Job/worker-empty verifier, startup recovery hook, double measurement during actual LTX handoff, worker stop/restore health proof, NVP side schema re-pin and client integration. Lu cannot claim real GPU lease ACK or safe unload/reload until these installed tests pass. No NVP source edits in this Core task. Preserve one existing Lu PR branch and batch CI without stacking.
