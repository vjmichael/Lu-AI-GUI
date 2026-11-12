# PERF-01 P5 — Inference Tuning and Speculative Decoding

**Date:** 2026-11-12. **Source branch:** `lu/perf01-p5-inference-tuning-20261112`, based on `lu/perf01-p4-interactive-priority-20261112`. **Status:** source implemented provisionally; real runtime/physical acceptance OPEN. No CI dispatch, PR, merge, engine upgrade or release.

## Goal and unchanged production contract

The original Lu desktop retains its known pinned `llama-server` identity and parameters: four CPU threads, 4,096 context tokens, one inference slot, 99 Windows GPU layers / zero Linux GPU layers, no explicitly overridden batch, microbatch, KV quantization or speculation. **`PersistentEngineConfig.tuning` is `None` in the normal desktop**. No unverified optimization is applied to every user, and no model prompt, WebView field, JSON tool argument or inherited `LLAMA_ARG_*` variable can introduce tuning or draft file paths.

Experimental settings are legal only through a **native host-owned `TuningProfile` passed explicitly at backend construction**, currently exposed to an **ignored real-hardware test**. New flags are a fixed allowlist; the prior SHA-256-verified server process, bound localhost key, one-slot custody, startup health gate and Windows Job Object remain.

## Implemented source

| Group | Experimental option | Native safety boundary |
|---|---|---|
| CPU threads | Main model `PersistentEngineConfig.threads`, separate `--threads-batch` | 1–256 main threads (original constructor); batch workers bounded by main threads and maximum 256 |
| Prompt processing | `--batch-size` (1–2048), `--ubatch-size` (1–512) | microbatch ≤ logical batch ≤ loaded context |
| GPU placement | `--n-gpu-layers` 0–99 | Linux experiment uses zero; Windows may compare 0 through 99; **no spare VRAM inferred** |
| KV precision | `--cache-type-k`, `--cache-type-v` | allowlisted `f16`, `q8_0`, `q4_0` only; compatibility/perf/quality NOT assumed |
| No speculation | `--spec-type none` | baseline normal production does not even add experimental flags |
| N-gram speculation | `--spec-type ngram-simple` | explicit experimental mode only; no draft GGUF and no unconditional speed claim |
| Draft model | `--spec-type draft-simple`, `--spec-draft-model`, `--spec-draft-ngl 0`, `--spec-draft-n-max 3` | separately native-admitted, pinned draft GGUF; CPU-only draft layer placement; no claimed additional GPU budget |

**Draft compatibility:** `gguf_metadata_summary::tokenizer_metadata_sha256` hashes **all** original bounded `tokenizer.*` GGUF metadata values in stable key order (including declared tokens, merges, special-token IDs, chat-template data), requiring genuine `tokenizer.ggml.tokens` and `tokenizer.ggml.model` records. Main and draft must be **distinct admitted files**, with matching declared model architecture **and** exactly equal tokenizer metadata fingerprints. Target/draft admissions are independently rechecked before and after metadata comparison. Draft file size is verified against admission and the original native file handle is kept alive with the engine child. **Matching metadata is necessary, not proof of neural/tensor compatibility, speculative accuracy, engine support or quality.** The pinned engine must still load successfully and real-model output must be evaluated. This is *not* a trust grant or permission to download a draft model.

**MMProj:** every experimental profile is refused when a projector is attached, even if speculation is off. The normal untuned image path remains unchanged pending its own genuine vision quality matrix.

**Fail closed:** unsupported flags, changed GGUF files, tokenizer drift, missing identity, out-of-range resource settings or a server that fails health checks abort the experiment. Do not quietly retry with a different unpinned engine, different GPU layers, or an arbitrary tokenizer; no hidden model substitutions.

## Reproducible real-GGUF benchmark

Fixture: `src-tauri/lu-core/tests/perf01_p5_real_tuning.rs` (ignored unless explicitly enabled):

`cargo test -p lu-core --test perf01_p5_real_tuning -- --ignored --nocapture`

Supply the following host environment variables on the real machine (never commit secrets or model paths):
- Required: `LU_P5_ENGINE`, `LU_P5_ENGINE_SHA256`, `LU_P5_MODEL_DIR`, `LU_P5_MODEL_NAME`, `LU_P5_MODEL_SHA256`, `LU_P5_API_KEY` (host-generated 64 hex chars), `LU_P5_SPEC` (`off`, `ngram`, `draft`), `LU_P5_KV_K`, `LU_P5_KV_V`.
- Draft only: `LU_P5_DRAFT_DIR`, `LU_P5_DRAFT_NAME`, `LU_P5_DRAFT_SHA256`.
- Optional bounded numeric controls: `LU_P5_THREADS`, `LU_P5_THREADS_BATCH`, `LU_P5_BATCH`, `LU_P5_UBATCH`, `LU_P5_GPU_LAYERS`, `LU_P5_TRIALS` (5–20, default 5).

The fixture independently hashes full GGUF and engine files, uses the **same fixed deterministic prompts and temperature-0 Lu text request builder** for baseline and experiment, starts a separately supervised pinned server per mode, records model load time and response timings, performs an identical untimed warm-up, and alternates A/B then B/A run order to limit thermal bias. Each trial produces three simple exact-response rubric items per mode. Five trials yield **15 baseline + 15 candidate timed samples** plus documented warm-ups. Outputs are not printed; SHA-256 digests and literal exact-rubric pass counts are recorded for comparison.

Receipt: source build SHA if available; OS/architecture; engine and model hashes; experiment settings; sample count; TTFT p50/p95, total latency p50/p95, load p50; output digests and three-item rubric counts. Missing GPU VRAM, joules, exact speculative token-acceptance metrics, complex reasoning/code/vision quality, and independently accepted physical performance are explicitly **NOT MEASURED/NOT VERIFIED**. The test never asserts that tuned output is semantically equivalent to the baseline merely because latency changes.

Suggested native matrix after exact-source compiler acceptance: (1) baseline 4 threads with original Windows Vulkan/CPU presets; (2) vary batch threads 2/4/8, batch 256/512/1024, ubatch 64/128/256; (3) GPU layers 0/partial/99 on the owner Radeon; (4) KV `f16` vs `q8_0` vs `q4_0` if supported by that exact bundled backend; (5) `ngram-simple`; (6) separately admitted same-tokenizer smaller draft GGUF. Each real profile requires at least five comparable alternating pairs, original instruction/quality checks, memory/energy instrumentation and a revert path. Do not multiply all parameter combinations blindly; use one-factor comparisons against a stable baseline.

## Tests and open gates

Added: source Rust tests for bounds and known flags; tokenizer fingerprint exact/different vocab/template/missing fields; distinct target/draft admission and MMProj refusal; native ignored real-model comparative fixture; Node source-contract tests for production defaults, pinned launch, draft boundary, and honest benchmark reporting.

**Actually exercised this turn:** four isolated in-memory JavaScript source-contract assertions against the exact fetched branch files: **4 passed**. They are not evidence of Rust compilation or native inference. Rust/Cargo is absent from this tool environment. All Rust unit tests, exact-head Windows/Linux compilation, native engine compatibility, owner Radeon runs, multi-profile performance, counterexamples, quality assessment, runtime cancellation, full P2/P4/P5 Group B integration, installed GUI and physical performance acceptance remain **OPEN**.

**Known limitations:** The installed pinned llama-server version has not yet been checked for every experimental flag; current upstream llama.cpp documentation may differ from the bundled version. Host-file pinning and GGUF metadata equality do not prove draft tensor compatibility or zero TOCTOU races on Linux; draft model selection is currently a native test-only opt-in, not a feature approved for end users. No reliable VRAM budget oracle exists; CPU-only draft offload is intentional. Experiments that fail startup/quality must stay disabled. No measured speedup or acceptance is claimed before execution.

**Next:** P5 native exact-head compiler and real paired GGUF benchmark acceptance when the owner machine is available, or proceed with P6 source design provisionally while retaining this gate.
