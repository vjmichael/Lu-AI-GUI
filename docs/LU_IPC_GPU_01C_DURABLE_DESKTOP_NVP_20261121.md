# LU-IPC-GPU-01C — Durable native NVP GPU handoff and desktop restart fence

Date: 2026-11-21. State: **exact-source native candidate; positive installed/physical acceptance BLOCKED**.

## Reuse the one public IPC, two trusted owners

Lu Core remains the sole GGUF/MMProj and host GPU resource-lease authority. NVP retains its own native LTX-2.5 diffusion/film worker lease, supervised Windows Job and original film audio/Take ledger. No second TCP/HTTP control interface, no use of the private Core-SYSTEM SCM command channel as a DLC endpoint.

Source was based on accepted PR #244 SHA 37a86b90a1a3f03971032cc5e52b0621ea486857 and NVP canonical PR #161 SHA d5754ed7e32294b9dd6b0b01a9ded0fed0fd2016. At the time of this source change no NVP installed SCM worker-stop issuer or matching signed Windows app installation was verified. Existing NVP observation code is read-only resource telemetry, not authority.

## A live original operation lasts longer than an IPC frame

Core's local framed Protobuf read/write IO timeout must stay bounded (three seconds in the inherited transport). A real llama.cpp GGUF unload/reload and WDDM measurement can take far longer. Blocking directly inside \`receive_nvp_gpu_windows\` would freeze its current Tokio/pipe handler and could cause NVP to retry an uncertain unload.

The new \`src-tauri/lu-core/src/gpu_async_reconciler.rs\` provides \`DurableGpuAsyncQueue\`, an independent WAL SQLite original-operation journal:

- Original host-approved scope (module, workspace, conversation, user turn, task, Core epoch, NVP signed session), canonical Core-generated lease ID, device, required VRAM and bounded duration are fingerprinted with SHA-256, serialised, and committed to a QUEUED row **before the IPC returns**.
- One active GPU transition globally; duplicate exact original work returns its recorded status **without spawning a duplicate**. A foreign frame that presents an existing lease ID but different original identity/fingerprint is refused.
- \`tokio::task::spawn_blocking\` runs the actual preauthorized CoreGpuLeaseManager -> TrustedInferenceSupervisor native unload or reload, OS GPU observation and NVP independently signed Job-empty verification. It rechecks original \`ConversationAuthority\` before effect and before GRANTED.
- An admitted QUEUED/RUNNING operation is \`AdmissionDecision=DEFERRED\`, with a correlation \`lease_ref\` and phase DRAINING or RELEASING. **DEFERRED IS NOT GPU READY**; NVP must not launch LTX. Only independent successful unload + WDDM/GPU capacity verification produces ALLOWED+GRANTED, and only native NVP Job-empty receipt, capacity and original GGUF READY produce ALLOWED+RESTORED.
- Added additive Protobuf \`GpuLeasePoll\` (WorkspaceFrame field 41) and \`GpuLeaseTransitionKind\`, feature-negotiated under the original \`gpu-lease-v1\` Hello. Every poll uses the original immutable session/workspace/turn/operation and Core-minted lease ref; no user-supplied filenames, memory samples or receipt signer. A failed or uncertain result never returns a permission-bearing lease_ref.
- A crash between QUEUED/RUNNING and the native worker's final durable state reopens as UNCERTAIN. It is never auto-resubmitted on reconnect, timer expiry or user retry; any actual CoreGpuLeaseManager DRAINING/GRANTED/RELEASING/RESTORING record is separately quarantined. A user timeout is not proof of success/failure.

The async ownership object clones the verified host's original \`ConversationAuthority\`, immutable tool/turn scope and exact approved policy identity into a bounded native worker, rather than borrowing a pipe or trusting NVP's Protobuf claims. A closed/revoked original host gate prevents later GRANTED. All paths remain disabled without an installed signed NVP peer, real Core SCM and protected key/witness chain.

## Desktop startup is now part of crash safety

\`lu-desktop/src-tauri/src/lib.rs\` constructs the original \`CoreGpuLeaseManager::open_and_fence_at_startup\` and \`DurableGpuAsyncQueue::open_and_fence_at_startup\` after it acquires the exclusive DesktopOwner and constructs the pinned real GGUF+projector supervisor, but **before** Tauri publishes its DesktopState to normal model-load commands and native work. Any unfinished/unverified NVP GPU lease holds normal inference admission closed. Failed DB recovery prevents desktop startup, not a silent unlocked fallback.

The read-only native \`lu_gpu_lease_status\` returns bounded unresolved/engine-phase indicators; it is not a GPU lease, a signed receipt or a user authorization path. Model profile settings are blocked during unresolved NVP GPU operations even if the LLM was evacuated from VRAM.

## Current trust and installed blockers

1. A single, production-eligible installed **NVP** public IPC caller with independent signed package, OS-created PID/birth/session identity, NVP-specific protected enrollment, SYSTEM credential broker handoff and per-message signer/token checks. Core's current installed HMAC credential reader requires SYSTEM, so a non-SYSTEM Tauri process cannot just call the SYSTEM credential vault. The broker must securely hand off an independently validated admission/session without leaking secrets or relaxing privileges. This **is not** implemented by simply adding a Tauri listener.
2. A real NVP native SCM machine signer that exclusively controls the original NVP LTX worker group's Job, observes kernel ACTIVE_ZERO and independently signs the canonical per-lease stop receipt under protected ProgramData; actual signer key, issuer, installer, and end-to-end positive verification are absent. A toy NVP receipt file or shell-signed surrogate does not count.
3. The OS WDDM adapter-global capacity measurement is read-only; there is no atomic hardware VRAM reservation, and not every AMD driver exposes its counters consistently. Physical R9700 tests must witness actual model VRAM before/after GGUF unload, LTX allocated/resident and after NVP process exit; missing/late telemetry must DEFER/DENY.
4. The Core desktop currently supplies the authoritative real inference supervisor and startup journals, but the installed per-frame accept loop and protected SCM issuer boundary still require real cross-process integration and installed consent. Do not call this a working NVP↔Lu deployment without an installed signed NVP app, Core SCM service and original real film authorization.
5. After crash, an uncertain QUEUED transition that never created the main GPU lease needs independently reviewed administrator recovery; no replay/heuristic can safely prove otherwise. Separate positive original recovery operation and independent worker/device witnesses are still required.
6. Actual signed NSIS + Windows SCM installed test: NVP request -> original user approval -> Lu GGUF/MMProj unload -> OS measured VRAM -> deferred poll to GRANTED -> LTX worker -> signed Job-empty receipt -> remeasure -> reload same GGUF/MMProj -> real model READY, including cancellation, replay, PID reuse, signer tamper, physical restart/reboot and insufficient VRAM.

**Acceptance labels:** Source compile, exact-SHA tests and fixture negative receipt failures establish source contract only. Zero claims of physical Radeon/LTX success or genuine installed NVP worker receipt until independently run. One existing Core PR #244; no NVP source modifications or new GitHub CI stacks.

## LU-IPC-GPU-01C-R1 native compiler repair

The first exactly published 01C source cohort at `bfbffc35320c78e74d52b3918cd587356fffb3a3` failed Ubuntu/Windows native code generation because Prost generated longer enumeration variant names than the authored `GpuLeaseTransitionKind::Acquire/Release` shorthand. The reviewed wire values are explicitly 1=ACQUIRE and 2=RELEASE, validated against the generated enum before use. Repaired both the native installed handler and Protobuf roundtrip test in one source successor. A short bounded `ConversationAuthority::with_authorized_worker_launch` read guard now protects the final SQLite GRANTED commit against original user-turn revocation *after* a lengthy model unload, without holding authority locks across native driver IO. Any failing code/test/Windows desktop source compilation remains a blocker, not an acceptance receipt.
