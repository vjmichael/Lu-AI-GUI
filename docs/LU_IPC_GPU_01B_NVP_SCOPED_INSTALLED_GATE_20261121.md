# LU-IPC-GPU-01B — Native NVP/Lu GPU lease ingress and installed proof boundaries

Date: 2026-11-21. Status: **source candidate, fail-closed; physical installed handoff not accepted**.

## One authority, two independently supervised workers

Lu is sole host owner of GGUF/MMProj selection, inference admission, public lu.workspace.v0 IPC, global GPU lease journal and reload. NVP is sole owner of its LTX-2.5 native diffusion workers, original creator/task permissions and production output. Both projects may have local resource scheduling; there is only **one Lu-issued NVP GPU handoff lease**, not two separate authorities.

Reviewed Core PR #244 previous accepted `b7f6b51a1f0278dc411ff3c1311bf6172cb7dddb` and NVP PR #161 source `d5754ed7e32294b9dd6b0b01a9ded0fed0fd2016`, notably NVP's `nvp_gpu_observation.py` and canonical Lu IPC v0 adapter. The latter's counters are observation, NOT authorization or SCM Job-empty evidence.

## Installed Windows NVP enrollment and feature-negotiated gateway

The existing protected enrollment reader (`ipc_installer_enrollment.rs`) is now extended with a **distinct fixed** `%ProgramData%\Lu\IPC\enrollments\nvp.film.json` record, separate from `lu.ide.json`, plus a dedicated NVP credential target `LuCore/IPC/NVP/v1/<package-sha256>`. The protected reader checks SYSTEM/Administrators owner and protected DACL on Lu-owned parents and rejects reparse files. The SYSTEM broker never shares an IDE key with NVP; NVP credential provisioning and revocation are separately bound to its installed package digest.

`workspace_ipc_transport::windows::accept_installer_owned_nvp_film` admits only an installed, signed image with exact protected path/digest/publisher/process birth/OS user, HMAC handshake and per-frame OS message-token witness. A NVP client must request `gpu-lease-v1` during Hello and receive the Core-signed feature in Welcome. This is not enabled for Lu IDE or an unsigned mock. Both NVP and IDE retain their own package identity; no unverified caller may use an NVP GPU endpoint.

`CoreDlcHostGate::receive_nvp_gpu_windows` receives one genuine verified `GpuLeaseRequest` or `GpuLeaseRelease` on the **existing Core Protobuf pipe**, not private Lu SCM or a new HTTP service. It validates original host-owned workspace/conversation/user-turn/task, session/module/epoch, and a host-approved READ + EXECUTE grant pair under `ConversationAuthority`. It mints the stable non-bearer `gpu-<original Core call hash>` lease ref, checks Windows DXGI LUID syntax BEFORE unloading, and invokes the actual sole `TrustedInferenceSupervisor` plus the persisted Core GPU lease manager. The typed answer is `GpuLeaseOutcome` and never contains a grant for a denied/unknown transition. Revocation/client death before reply closes delivery, leaving durable reconciliation evidence rather than invented success.

This source bridge is intentionally **not a model-facing physical device control tool**. Only the host's original user-granted film render effect can request a cooperative memory handoff. Final installed broker registration and startup enrollment are still prerequisites.

## Native independent Windows VRAM budget observation

The private unsafe Windows boundary (`lu-win-worker-job/src/ipc_gpu_vram.rs`) uses DXGI 1.1 adapter enumeration and a *specific* LUID, physical-node-zero only. It requires DXGI Adapter3 local video-memory budget **and** WDDM PDH `GPU Adapter Memory(<same exact LUID>_phys_0)\Dedicated Usage` adapter-global counters; malformed/missing/duplicate counters, software adapters, different LUIDs, over-budget or unknown capacities fail CLOSED. The measured handoff headroom is conservatively `min(physical dedicated capacity - adapter-global dedicated usage, DXGI process budget - current process usage)`. **DXGI is process-budgeted, not evidence of global VRAM on its own.** Neither vendor telemetry, a Protobuf claim nor NVP-provided samples can satisfy this observer. The Core manager requires two valid monotonic samples after model unload and before restoration.

These OS observations are not an atomic hardware VRAM reservation; external GPU processes can race the time between measurement and LTX allocation. NVP must still maintain its local resource guard and monitor pressure. Windows physical driver behavior (especially the Radeon AI Pro R9700) must be independently witnessed at installed release acceptance.

## Independent NVP SCM protected GPU worker-stop receipt

Protected installer must provision `%ProgramData%\Lu\IPC\enrollments\nvp-gpu-stop-p256.pub` as a 72-byte P256 ECCPUBLICBLOB and `%ProgramData%\Lu\IPC\nvp-gpu-stops-v1` with explicit SYSTEM/Administrators-only protected ACL. **Lu does not create/sign the NVP key or file.** NVP SCM must own original OS Job/cgroup worker group, independently verify kernel ACTIVE_ZERO, and sign one immutable original-call receipt with its nonexportable CNG ECDSA P-256 key. Signature covers exact canonical domain `LU_NVP_GPU_JOB_EMPTY_V1\0`, every lease/device/module/workspace/conversation/turn/task/epoch/NVP session field, fixed `WINDOWS_JOB_ACTIVE_ZERO_V1` method and original NVP service+worker PID/birth generations. The receipt file is `<core-lease-ref>.stop.v1` under the protected directory. Raw filename/path/key/signature bytes supplied by NVP IPC are never read as authority.

`InstalledNvpWorkerStopVerifier` requires exact protected file, pinned distinct NVP public key, correct signature, original-call identity and valid native method before Core will attempt to reload its GGUF. A text ALL_CLEAR is ONLY a trigger. Missing signer, absent supervised Job witness, mismatched turn or invalid signature keeps the lease held and Lu's LLM unloaded. This does not reclassify an unknown film effect as successful.

**NVP dependency:** implement the independent **installed native NVP supervisor issuer** for the canonical receipt contract; NVP owns this operation and must not borrow Lu's Core-executor signer. Until that separate issuer and physical positive checks are available, Core's installed receipt verifier refuses release.

## Real model restoration and recovery

The real `TrustedInferenceSupervisor` restores its previously admitted GGUF+approved MMProj from the sealed model registry under the GPU admission fence; the manager now requires an observed `EnginePhase::Ready` with the exact original model/session identity before reporting `LU_MODEL_READY`. Host Core may continue UI/progress/task orchestration while its model is unloaded; new LLM generations are held. The GPU ledger preserves DRAINING / GRANTED / RELEASING / RESTORING / RESTORED, with interrupted states quarantined on opening the DB. Explicit recovery requires original authority, verified NVP worker-empty record and fresh DXGI/WDDM capacity samples; no expiry or blind replay grants memory ownership.

## Tests and remaining blockers

- Native Windows CI compiles the FFI and IPC ingress and exercises exact DXGI LUID/counter parser negative tests, distinct IDE/NVP broker credential IDs, bounded P-256 receipt refusal, and Core lease/schema tests.
- Linux CI tests the Core state machine and schema; its source-only default `InstalledHostGpuObserver` denies positive GPU admission until a separately OS-supervised Linux provider is installed.
- Needs physical installed NVP publisher/signature/pin, native NVP Job-empty signer, Core startup wiring of `reassert_recovery_fence`, dedicated blocking GPU request executor, real GGUF unload/VRAM recovery and LTX/PCM original film workflow, cancellation/revocation/reboot/rollback cases, and an independent actual signed client exchange with pinned exact NVP/Lu build SHAs.
- This is not a production-ready service or a claim that Lu and NVP already completed a physical handoff. No NVP repository changes made from the Lu Core workstream. The existing Core PR/branch remains the sole publication target without stacking CI.

Source design reference: Microsoft DXGI QueryVideoMemoryInfo semantics are *per-process budgets*. WDDM adapter-global dedicated usage is required as a distinct measurement, not added to DXGI process usage.
