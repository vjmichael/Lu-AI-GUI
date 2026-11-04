# Lu v1.0 — BACKEND-CTRL-01: Universal generation lifecycle, cancellation and ejection

Research/design revision: 2026-11-04. **Design-only: backend-specific production cancellation and live worker recovery are not yet implemented or accepted.** Based on TOOLS-08 PR #19 at `62992c9ac20bed96e815ba732c94ebf186899a1e`. Do not alter accepted `0.1.0-alpha.2`.

## Owner decision

When a response is truly finished, its generation must stop. Keeping model weights or a safe KV/prefix cache resident for future prompts is **not** continued token generation. A model that keeps decoding after a terminal response is a protocol/lifecycle defect or a still-unfinished request, not a desirable background cognition feature. No arbitrary post-reply inference. Any separate work (tool continuation, async agent task, background analysis) must have its own explicitly authorized request, status and budget. Do not equate visible prose ending with the model's EOS/end-of-turn: hidden reasoning, multi-part output and tool calls can precede a real end.

## Backend candidates and documented controls (version-dependent; adapter must probe at runtime)

| Backend | Soft-stop pathway | Important limitations | Target |
| --- | --- | --- | --- |
| llama.cpp llama-server (GGUF, Vulkan/ROCm etc.) | Streaming cancellation through request/response cancellation; internal queue CANCEL task; some resumable streams expose `DELETE /v1/stream?conv_id=...` | An HTTP disconnect is not proof of stopped decoding. Stream deletion and session teardown can differ from actual slot stop across versions; verify no active request/slot and no further decode. `/v1/chat/completions/control` `reasoning_end` is **not** general cancel. | Windows primary, Linux |
| vLLM | `AsyncLLM.abort(request_id)`; cancellation of async iterator triggers abort; parent request abort propagates to child samples | API mode/version varies; verify request actually removed from scheduler, not just disconnection. Pause/keep is *not* request completion. | Linux/server |
| SGLang | `POST /abort_request` with `rid`, or `abort_all` | HTTP 200 can coexist with hung original response in reported concurrency/LoRA cases; verify terminal response and scheduler state. | Linux/server |
| Ollama | Cancel request context/close active stream; scheduler checks canceled pending work; `keep_alive` controls model residence (not generation) | No universal externally stable per-request abort guarantee established in this review. For Lu-owned runner only, kill owned process if stream cancellation fails. Don't kill arbitrary system Ollama daemon. | Optional local/externally managed |
| KoboldCpp | `POST /api/extra/abort` with `genkey` for request-scoped stop | Validate request association; no proof from HTTP status alone. | Optional desktop |
| ONNX Runtime GenAI | Runtime option `terminate_session=1`, then `terminate_session=0` to recover; generator `IsDone()` | Embedded generator/session ownership needed; potentially broader than per-request stop. | Optional Windows/native |
| Hugging Face Transformers | `StoppingCriteria` / explicit control in `generate` loop | Streamer-reader close alone does not prove model thread stopped; own worker process needed for fail-safe interruption. | Python adapter |
| MLX engines | Request ID cancel events / batched request remove, specific to engine | macOS Apple silicon only; inspect/version-pin actual integration. | Future macOS |

Source references reviewed:
- https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README-dev.md
- https://github.com/ggml-org/llama.cpp/blob/master/tools/server/server-context.cpp
- https://github.com/ggml-org/llama.cpp/blob/master/tools/server/server-stream.cpp
- https://github.com/ggml-org/llama.cpp/issues/24496
- https://docs.vllm.ai/en/stable/api/vllm/v1/engine/async_llm/
- https://docs.vllm.ai/en/latest/api/vllm/v1/engine/output_processor/
- https://github.com/sgl-project/sglang/blob/main/python/sglang/srt/entrypoints/http_server.py
- https://github.com/sgl-project/sglang/issues/29179
- https://github.com/ollama/ollama/blob/main/docs/api.md
- https://github.com/ollama/ollama/blob/main/server/sched.go
- https://github.com/LostRuins/koboldcpp.net/blob/main/src/content/docs/docs/api/endpoints.mdoc
- https://github.com/microsoft/onnxruntime-genai/blob/main/docs/RuntimeOptions.md
- https://github.com/huggingface/transformers/blob/main/src/transformers/generation/stopping_criteria.py
- https://github.com/lmstudio-ai/mlx-engine/blob/main/mlx_engine/model_kit/batched_model_kit.py

## Required host-owned adapter contract

Every adapter exposes **capabilities**, not optimistic assumptions: `request_scoped_cancel`, `stop_ack`, `request_active_query`, `can_unload`, `process_owned_by_lu`, `shared_requests`, `backend_process_tree_owned`, `stop_token/eos_report`, and `worker_reap`. A generic OpenAI-compatible endpoint alone does not imply request-scoped cancellation.

The host assigns opaque request/turn IDs and owns backend process handles. Model-generated JSON may not provide lifecycle IDs or dictate the recovery action.

State machine:

`CREATED → PREFILL → DECODING → (COMPLETED | CANCEL_REQUESTED → CANCEL_ACKED → STOP_VERIFIED | ESCALATING → TERMINATED | FAILED_STOP)`

Completion must be driven by an authenticated backend finish event or EOS/end-of-turn semantics, *not* an inferred period, newline, token count plateau or GUI message appearing complete. Capture `finish_reason` (EOS/stop/tool_call/length/cancel/error), final-token timestamp, last observed token counter, and active scheduler/slot state if available.

1. A normal final EOS/turn-end or authorized terminal stop marks request finished; stop decoding **that request immediately**. Persist response, release its slot and generation-only buffers. Retain loaded model based on explicit idle/VRAM policy.
2. Token-loop guard (TOOLS-08) or user Stop triggers request-scoped cancel. Use native `abort(request_id)` or documented disconnect semantics only when backend supports them; do not pretend socket close is authoritative.
3. Verify after cancel with backend-specific terminal event + scheduler/slot state and no further tokens for that request. An ACK, stopped UI stream or quiet socket is insufficient. If no independent query exists, label stop **unverified**, and escalate owned worker after deadline. Do not call cleanly canceled just because local reader closed.
4. If cancellation fails, graceful shutdown of a **Lu-owned exclusive worker**, then hard-kill its OS-owned process tree if needed. On Windows use Job Object containment; on Linux process groups/cgroup where available. Kill/reap is not available for arbitrary remote or user-managed servers. Shared workers require first canceling a single request; if forced shutdown is unavoidable, mark every co-located turn interrupted and retain partial transcripts.
5. Reconcile native effect journal (PREPARED/EXECUTING/APPLIED/FAILED/UNCERTAIN) before any continuation. Never repeat a tool effect or unsafely replay a prompt just to recover a model worker.
6. Check exit and resource release. GPU memory telemetry is advisory and may lag; if GPU driver remains wedged, stop reload attempts and report a blocked/failed backend. Apply resource guardian headroom before reload.
7. Restart/reload only when safe and authorized, respecting a finite restart circuit breaker and exponential backoff. Do not automatically regenerate the same loop-triggering request. Permit deliberate new user prompt after recovery.
8. Request-level stop must not abort other users' or chats' healthy requests unless the entire worker is irrecoverable. Host reports collateral interruption honestly.

## Completion semantics

- Successful `finish_reason=stop` or true model EOS = final request completion; no post-answer tokens.
- `finish_reason=length` = budget-limited truncated output, **not** semantic completion. UI labels truncated; optional continuation requires host/user authorization.
- `finish_reason=tool_calls` = model generation step complete, not user task complete. Native dispatch follows only after existing authorization.
- Thinking/reasoning content before final answer belongs to same request. Once final EOS arrives, reasoning is over; no hidden autonomous decode.
- Prefix/KV caching, batching scheduler housekeeping, telemetry and output flushing may consume some CPU/VRAM after the last visible text; these are allowed only as bounded non-generative actions.

## Acceptance / CI gates

1. In-process fake backend: EOS ends decode immediately; repeated output triggers TOOLS-08 and cancel once; response with natural repetition succeeds; length finish is truncated, not marked complete; tool call stops current inference without ending task.
2. Adversarial fake: ignores cancel, claims stopped while still emitting, fails to ACK, drops stream without stopping, stops one of many shared requests, and leaves resource telemetry stale. Host distinguishes `STOP_VERIFIED` from `FAILED_STOP`.
3. OS fixture: real detached child tree and termination/reap (Windows Job Object, Linux process group). Verify owned-only termination, no orphan grandchildren, no automatic prompt replay, clean ledger after restart.
4. Integration: version-pin llama-server and invoke real GGUF infinite/repetitive generation, cooperative cancel, cancellation failure injection, GPU/backend worker restart; verify no slot keeps decoding after terminal stop and no runaway VRAM allocation. Expand to vLLM/SGLang on supported Linux CI with installed dependencies if runners are available. GPU-specific acceptance is separate from CPU CI.
5. Per-adapter capabilities and regression matrix: report version, OS, GPU backend, native abort method, stop verification, shared-work isolation, forced-kill support, model unload/reload safety. Unsupported capabilities fail closed; never promise hard control over external servers.

**Immediate implementation priority:** llama.cpp lifecycle adapter + host worker supervisor on Windows, then vLLM/SGLang adapters for Linux. Preserve TOOLS-08 progress/token guard and TOOLS-08B durable recovery as separate gates.
