# PERF-01 P2 — Prefix/KV reuse, invalidation and real local measurements

Date: 2026-11-12. Repo: `vjmichael/Lu-AI-GUI`. Work branch: `lu/perf01-p2-kv-cache-20261112`, stacked on `lu/perf01-p2-inference-latency-20261112`. No IDE/NVP changes, no model authorization, no merge/release, no per-edit CI.

## Source-level implementation

- **Stable prefix:** Only scoped human text requests acquire versioned `LU_SAFE_PREFIX_V1` as a native-host-defined, non-authorizing `system` message, placed BEFORE original ordered typed history, changing user prompt and trailing OS clock. The prefix carries no paths, chat ID, secrets, grant or retrieved source content. Original chat history remains ANSF/SQLite-backed and is not a KV cache. The underlying tokenizer/chat-template still decides token-prefix identity.
- **Explicit switch:** The pinned, host-supervised single-slot `llama-server` HTTP body carries `"cache_prompt":false` on cold, unscoped, changed-chat, multimedia and tool requests; `true` only for subsequent successful ordinary text turns in the same trusted conversation and the same live process. The server can reuse only its ACTUALLY matching token prefix; `true` does not attest a cache hit. No extra llama.cpp worker, GPU wake, new tool or file authority is created.
- **Per-turn scope binding:** Original Tauri conversation ID is bound AFTER exact session/generation reservation and BEFORE dispatch. Wrong session, generation, duplicate binding or invalid scope is rejected. Only plain human text with no appended retrieval, planning, native-action hints or other dynamic host envelope can bind; the appended native clock is a trailing suffix. Browser function schemas, multimodal turns and unbound assistant continuations are never eligible.
- **Hard boundary:** Every GGUF and MMProj load creates a new supervised `llama-server` child with native admitted/pinned files. Backend clears the cache gate on load, unload/eject, cancellation, backend failure, browser-schema generation or image generation. Text/model/session/projector identity cannot be shared between child processes; server slot is single parallel lane. On changed conversation scope, the FIRST request explicitly clears the previous slot with `cache_prompt=false`.
- **Policy observation, not fabricated hit:** `PerfReceipt.cache_scope_admitted` tells whether the native host bound a candidate. `cache_hit` remains `null` without actual backend usage/timing evidence; no fabricated token count, prefill throughput, GPU utilization or VRAM claim.

## Added source tests

1. Scoped first turn: false; second successful same-scope turn: true; different chat: false.
2. In-progress/failure/cancellation invalidates; one image/tool request invalidates.
3. Untrusted/invalid scope and tool-bearing JSON cannot reuse; stable system prefix only on eligible text.
4. Native supervisor rejects wrong-session, wrong-generation, invalid/duplicate scope binding.

These tests have been committed but **NOT executed**. A source change is not a compiler or real-runtime PASS.

## Repeatable original-host engine benchmark

A deliberately ignored (owner opt-in) native Rust integration test lives at `src-tauri/lu-core/tests/perf01_kv_cache_real.rs` and invokes `PersistentLlamaBackend` using the same real GGUF/llama-server admission. It pins **full model SHA-256** and verified engine SHA-256, requires a host-generated random 256-bit localhost key, records the same engine PID throughout and compares repeated **same-prefix** cold requests versus seeded same-conversation eligible warm requests. The test writes a bounded JSON receipt to stdout containing OS/arch, model/engine hashes, context, repeats, first-token p50/p95, completion p50, chunk counts and explicitly unknown cache/GPU fields. It does not output private prompts/responses, paths or keys. *This fixture is text-only; projector/image acceptance is separately open.*

Configure the following environment variables locally (never commit the API key): `LU_PERF01_BENCH_ENGINE`, `LU_PERF01_BENCH_ENGINE_SHA256`, `LU_PERF01_BENCH_MODEL_DIR`, `LU_PERF01_BENCH_MODEL_NAME`, `LU_PERF01_BENCH_MODEL_SHA256`, `LU_PERF01_BENCH_API_KEY`, and optionally `LU_PERF01_BENCH_REPEATS` (3–20; default 7). Run `cargo test -p lu-core --test perf01_kv_cache_real -- --ignored --nocapture` in the appropriate workspace. The baseline is a NEW request with the same safe system prefix but a distinct scope (forced cold); the candidate is preceded by a same-scope seed request. All use the same fixed user/assistant history and response budget.

**Not yet measured:** Actual real-GGUF timing distributions, backend `cache_n` / `prompt_n`, cache-hit rate, prefill or decode tokens/sec, physical Radeon occupancy, output quality changes and full render-to-screen latency. A warm request can be eligible but reuse no tokens. The pinned llama-server version must be checked at native acceptance for the exact `cache_prompt` chat API behavior, including its nondeterminism caveat.

## Acceptance / failure gates

- Exact-head Rust unit and integration compile on Windows + Linux, then run targeted new tests; repair demonstrated failures before declaring implemented source accepted.
- Native pinned-engine GGUF+MMProj identity and reload/eject tests, cancelled or failed stream, cross-chat same-content negative test, tool schema and source-revision invalidation, conversation edit/truncation and history-window boundary.
- Real, owner-operated warm/cold fixture with repeated TTFT and total-latency distributions; verify actual KV reused tokens from the pinned backend's supported timing/usage surface (the new test intentionally does **not** fabricate this). Compare quality and resource costs, and verify no model/GPU wake for deterministic CPU queries.
- Installed Windows physical Radeon native chat + MMProj acceptance, recovery and sustained background/foreground interaction. No automatic publication or merges.

**Disposition:** Source integration implemented; native compile, actual KV-hit proof, real performance data, GPU/installed acceptance and release remain OPEN. No result will be called a measured cache hit without the backend's own authenticated evidence.
