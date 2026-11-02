# Windows physical acceptance — R20 GGUF load/unload memory reclamation

**Observed:** November 1, 2026 (Arizona local date)  
**Test source:** User-provided Windows 11 Task Manager screenshots of **Lu R20** with Qwen3.8-27B Q4_K_M GGUF loaded, then unloaded  
**Hardware shown:** AMD Radeon AI PRO R9700, 32.0 GB dedicated VRAM; 31.9 GB usable system RAM  
**Evidence class:** Physical-PC observation (screenshots); **not** a controlled Vulkan profiler or engine-log trace.

## Observed values

| Windows Task Manager measurement | Loaded | After unload | Difference |
|---|---:|---:|---:|
| Dedicated GPU memory | 17.9 / 32.0 GB | 1.7 / 32.0 GB | 16.2 GB released |
| Combined GPU memory | 19.2 / 47.9 GB | 2.0 / 47.9 GB | 17.2 GB released |
| System memory | 25.1 / 31.9 GB | 9.1 / 31.9 GB | 16.0 GB released |
| GPU utilization when captured | 0% | 0% | No inference-time utilization measurement |

## Interpretation

**Observed:** The user's R20 load/unload action coincides with large increases and subsequent decreases in GPU/system memory. This is strong practical evidence for model residency and successful release of substantial memory following unload. The dedicated VRAM change is consistent with a quantized 27B model's footprint, but the screenshots alone do not show process attribution or Vulkan layer counts.

**Physical test status:** **PASS (qualitative)** for 'load and unload visibly change GPU memory and return near baseline' in the R20 development evaluation. It is not yet a pass for 'AMD Vulkan inference executed on device', since 0% Task Manager utilization is a static idle-time screenshot and no llama.cpp Vulkan offload logs were submitted.

## Gates still unverified

- Confirm Vulkan runtime selection and number of model layers actually offloaded from llama.cpp logs (or another direct backend diagnostic).
- Confirm a generation period's GPU compute activity and performance; do not infer it from static Task Manager VRAM use.
- Confirm no orphaned `llama-server.exe` or worker process remains after unload and app shutdown.
- Repeat memory recovery on R21+ / the structured-chat successor, with two or more load/unload cycles and another model if available.
- Track failures in image-conditioned chat separately; successful model load/unload does not certify MMProj vision requests.

**Provenance / privacy:** Observations transcribed from user-supplied images. No local Windows username, folder paths, or screenshots are committed here.

**Related engineering notes:** `docs/LU_R20_ENGINEERING_REPORT_20261101.md`; `lu-desktop/docs/LU_R21_WINDOWS_IMAGE_ADMISSION_AND_MODEL_BOUNDARY_20261101.md`.

**Development policy:** GitHub is the repository and milestone record; local development/testing and physical Windows acceptance remain authoritative.
