# Lu R23 — native per-model inference profiles (engineering handoff)

**Date:** November 2, 2026. **Canonical repository:** `vjmichael/Lu-AI-GUI`.
**Source base:** R22 plus the R20 acceptance report (`c9bd1945`), no Jan execution code.
**Status:** R23 source development and frontend checks. **NOT Windows alpha accepted.**

## Motivation and evidence

R22 starts Qwen with `--ctx-size 4096`, `--threads 4`, a desktop `maxNewTokens: 512`, and `temperature: 0.0`. This substantially differs from the user's LM Studio setup (262,144 configured context). It is not evidence of a defective Qwen model. The R22 typed Protobuf chat history and llama.cpp chat-completions API remain intact.

## R23 implementation

- Model-specific profiles keyed by trusted opaque model IDs, stored under Lu AppData in `inference-profiles.json` (not inside the GGUF or editable model output). Invalid/corrupt profiles fail closed; writes stage to a separate file, sync, and replace with backup/rollback on error.
- Defaults: `contextTokens=8192`, `maxNewTokens=2048`, `temperature=0.6`, `topP=0.95`, `topK=20`, `minP=0`, `threads=4`.
- Supported bounds: context 4,096–262,144 in 1,024 increments; response 128–8,192 and always shorter than context; temperature 0–2; top-p 0.01–1; top-k 1–100; min-p 0–1; CPU threads 1–64.
- Profile values are validated in Rust, not trusted from the browser. The profile is snapshotted before model load. Loaded models cannot change profile in place. The single-model supervisor and cross-chat generation queue remain unchanged.
- Engine startup now uses per-model `--ctx-size` and `--threads`; the llama.cpp chat-completions body applies per-model temperature, top-p, top-k and min-p to text and image prompts; the desktop forwards its loaded profile's output-token limit. The Rust host enforces that same limit again.
- GUI Settings has grouped, keyboard-accessible numeric controls, per-model selection, and an explicit Save action. No implicit model loading.

## Known limitations and acceptance criteria

1. **Context profile vs usable history:** R23 retains the pre-existing bounded typed chat-history budget (16 prior turns / 15 KiB) and current prompt/file limits. Increasing engine context creates capacity for longer prompts/responses but does **not** magically make Lu recall 262K of historical messages. Extending context retrieval belongs to the memory/ANSF milestone.
2. **KV cache and VRAM:** A 262K setting is experimental and may fail on a 32 GB VRAM card. Default 8K is chosen to reduce risk. Lu should eventually display an estimated KV cache impact before loading.
3. **Qwen thinking:** R23 does not pretend that model-specific reasoning template toggles have been validated. Qwen enable-thinking options require pinned llama.cpp compatibility testing before exposure.
4. **No full Windows compile claimed** until the offline Cargo cache / target dependencies are available or an independently verified Windows runner has produced an executable. Frontend compile, source checks and tests are not Windows acceptance.
5. **Exact GGUF comparison:** Run the same Qwen GGUF/MMProj in Lu and LM Studio with matched context, output budget, sampler and thinking settings. Compare identical prompts including long answer, code, follow-up context and image-dependent response. Capture load logs, GPU offload layer count, and latency. Treat model-quality variation separately from transport failures.
6. **Profile persistence:** Save a profile, restart Lu, confirm its values persist, load the model, ask for a long explanation, unload, then change profile. An invalid value and an attempt to edit a loaded model must be rejected.

## Tests run in this workspace

- `node --test tests/*.test.mjs`: 46 passed / 0 failed (including 5 new profile tests).
- `npm run build`: compiled React using pre-supplied offline Yarn packages.
- `npm run check`: 29/29 pass.
- `rustfmt` on four modified Rust sources completed.
- `cargo check --offline`: **blocked** by missing Cargo registry entries starting with `base64`, not a compiler diagnostic. No NSIS R23 package is certified from these source-level tests.

## Scope protection

No alterations to Lu's conversation protocol, worker authorization, ANSF, model-import file admission, or the R18 queue. JSON repair remains deferred. GitHub is a source repository, not a replacement development hub; use CI only for isolated Windows build/installer verification if local dependencies are unavailable.
