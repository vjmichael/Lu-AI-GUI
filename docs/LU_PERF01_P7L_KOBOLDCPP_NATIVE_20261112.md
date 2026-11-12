# PERF-01 P7L — Concrete Independently Supervised KoboldCpp Adapter

**Date:** November 12, 2026. **Repo:** `vjmichael/Lu-AI-GUI`. **Branch:** `lu/perf01-p7l-koboldcpp-supervised-20261112`, based on `lu/perf01-p7-backend-evaluation-20261112`. **Disposition:** original source implementation + native test harness committed, **native tests and genuine GGUF A/B evidence NOT EXECUTED**. No per-commit CI, PR, merge, release, third-party package download or default backend promotion.

## Selection and trust boundary

P7 originally reviewed mistral.rs, KoboldCpp, vLLM GGUF. **KoboldCpp is the first native evaluation candidate** because its actual docs specify an authenticated, host-bindable `/v1/chat/completions` SSE API and `--model`, `--mmproj`, `--usecpu`, `--contextsize`, `--threads`, `--host`, `--port`, `--quiet`, plus `KCPP_PASSWORD`. Alternative mistral.rs does **not** authenticate its HTTP bearer key without an additional reverse proxy, making it less suitable for Lu's existing independently authenticated localhost design. KoboldCpp also documents server-supplied `tool_calls` deltas, but **original GGUF/quant/tool-stream compatibility remains UNVERIFIED**.

- KoboldCpp CLI and CPU/Vulkan flag definitions: https://koboldcpp.net/docs/settings/command-line/ ; https://koboldcpp.net/docs/reference/flags/
- KoboldCpp Bearer header/password and endpoint access boundaries: https://koboldcpp.net/docs/api/passwords-and-security/
- KoboldCpp native and OpenAI paths, including tool/image endpoints: https://koboldcpp.net/docs/api/endpoints/ ; https://koboldcpp.net/docs/api/streaming/
- mistral.rs documents no built-in auth: https://docs.mistralrs.dev/reference/http-api/

**Product boundary remains unchanged:** the installed desktop still constructs its original `PersistentLlamaBackend` source-pinned llama.cpp CPU/Vulkan model using the original `TrustedInferenceSupervisor` and original Protobuf IPC. `lu_model_select_backend` has not been modified to accept KoboldCpp. `packaged:false` / `selectable:false` remain in the backend inventory. This is a manually constructed host-evaluation adapter, not an app/plugin installed for end users, and not an automatic provider fallback.

## Actual source implementation

1. **Concrete `InferenceBackend`:** `src-tauri/lu-core/src/p7_koboldcpp_backend.rs`. `KoboldCppBackend` owns a `VerifiedCandidate`, original separately admitted model and optional projector, full child process `Child`, dedicated loopback port, host 64-character hex secret and one durable process session. It can satisfy the original `InferenceBridge`/supervisor's backend trait but is not registered in installed Lu. It remains evaluation-only. The runtime is always **CPU-only** for these first tests to avoid unverified Vulkan fallback and hardware equivalence claims.
2. **Original file identity and bounded configuration:** P7's `VerifiedCandidate::recheck_originals` re-hashes the entire reviewed executable and original GGUF immediately before spawn, plus the independently SHA-256-pinned original MMProj if one is selected. No model-supplied executable, URL, shell fragment, config file, third-party provider or untrusted Hugging Face download is accepted. Bounded main threads (1–64), context (256–262144) and timeouts (≤600 s load, ≤3600 s request). Original pinned file handles are kept alive for child lifetime. **An executable digest alone is not full dependent DLL/shared-object, signature or race-free OS path attestation.**
3. **Original secure launcher:** `Command` sends only fixed `--model`, `--contextsize`, `--threads`, `--usecpu`, `--gpulayers 0`, `--host 127.0.0.1`, `--port`, `--quiet`, and optionally the independently admitted `--mmproj`. Kobold's host password is passed via **child-only `KCPP_PASSWORD` environment**, never process argv, journal, chat or UI. The child environment is cleared before a small allowlist of offline/loader runtime values is re-added. It never enables `--agent`, remote tunnels, MCP, router-mode administration, tool execution, image-generation providers, online models or automatic software updates. The Kobold HTTP server still exposes some informational web endpoints to other local processes; **this is not an OS network sandbox**. No remote data egress protection has been independently proven.
4. **Authenticated readiness, no foreign listener:** after reserving a host-owned ephemeral loopback port and spawning the original child, `authenticated_probe` posts fixed synthetic text to Kobold's **non-generating** `/api/extra/tokencount`: a deliberately wrong key must produce HTTP 401 and the original 256-bit host secret must produce HTTP 200. A passwordless or unexpected listener is rejected. Readiness also requires the original child PID to still be alive. Loopback port bind/drop/spawn has a brief window and this HTTP challenge is not cryptographic OS PID-to-listener attestation; the native release gate must independently verify original port ownership and executable package identity.
5. **Original process lifetime:** Windows creates Lu's existing kill-on-close native Job Object and assigns the original child before HTTP health checks; failed assignment kills/waits the original child and drops the Job before durable release. Linux/Unix launches an original process group using `process_group(0)`, sends group SIGKILL on unload/cancellation and waits for the original parent. Direct wait evidence and Job/group control are recorded as available; escaped groups, load-time races and post-crash orphan ancestry are **not independently accepted**. An unsuccessful reap leaves `custody_uncertain=true`.
6. **Durable recovery/quarantine:** `src-tauri/lu-core/src/p7_kobold_custody.rs` adds a single-original-session SQLite WAL/FULL-sync journal with exact session, binary SHA-256, GGUF SHA-256, original OS PID and `STARTING`/`RUNNING`/`REAPED` state. `BEGIN IMMEDIATE` admission prevents a new child while an active or unresolved older session is present. On unexpected process death, aborted startup or restart, uncertainty remains visible and a new host instance **fails closed** until an actual original OS process/reap reconciliation. A stale PID is never silently accepted as cleanup evidence. The SQLite journal is a recovery record, not a kernel process witness.
7. **Original streaming and tool calls:** text/image requests use original `build_structured_chat_request`, `generate_chat_stream_body` and bounded SSE parser with host `Cancellation`. Images fail if the original pinned MMProj is absent. Browser proposals use original `build_read_only_browser_tool_request` and `generate_browser_tool_stream_body`, allowing only `lu_browser_read` and original JSON schemas; tool prose is never treated as executable output. Successful proposals return `CompletedToolCall` records to the original human/host authorizer, **never execute** a browser command. P2 KV reuse is disabled entirely until Kobold's original cache-identity rules are independently proven. A failed/malformed/aborted stream terminates and reaps the original engine with no automatic retry or backend switch.

## Reproducible native and hardware test matrix

**A. Rust unit + original HTTP parser tests** (ordinary native compile on Windows/Linux):

`cargo test -p lu-core --test p7l_kobold_http_protocol -- --nocapture`

Includes actual 127.0.0.1 HTTP sockets with real fragment/chunk boundaries, authenticated 401, original text SSE assembly, fragmented structured `lu_browser_read` tool-call assembly and JSON validation, unauthorized `lu_browser_interact`, malformed/error frames, and cooperative mid-stream cancellation. Unit tests in the main modules also check incorrect/passwordless auth, journal persistence and wrong-session release. These test *Lu's real parser against synthetic wire bytes*, not the genuine KoboldCpp binary.

**B. Real OS child fixture** (compile-only opt-in feature, excluded from normal Lu installer):

`cargo test -p lu-core --features p7l-native-fixture --test p7l_kobold_process_custody -- --nocapture`

This builds `lu-p7l-kobold-http-fixture` as a separate native test executable that accepts the precise bounded Kobold flags, enforces `KCPP_PASSWORD`, and emits OpenAI SSE/tool-call wire responses. Tests require the original `KoboldCppBackend` to spawn it, observe native PID, stream two fragments, retrieve a read-only tool proposal, process cancellation, kill/reap, and record a durable SQLite `REAPED` before a second launch. Negative cases inject invalid SSE, unauthorized tool names and an unresolved pre-spawn journal record. The binary is **not KoboldCpp or real inference**. Test-only fixture code cannot ship in regular packages.

**C. Genuine physical candidate evaluation** (ignored unless owner supplies original approved artifacts):

`cargo test -p lu-core --test p7l_real_kobold_comparison -- --ignored --nocapture`

Owner must provide:
- `LU_P7L_MODEL_DIR`, `LU_P7L_MODEL_NAME`, `LU_P7L_MODEL_SHA256` — original admitted GGUF and SHA.
- `LU_P7L_KCPP_ENGINE`, `LU_P7L_KCPP_ENGINE_SHA256` — independently reviewed original Kobold binary.
- `LU_P7L_LLAMA_ENGINE`, `LU_P7L_LLAMA_ENGINE_SHA256` — original baseline engine.
- `LU_P7L_KCPP_SECRET`, `LU_P7L_LLAMA_SECRET` — separate 64-hex OS-generated temporary host keys (do not commit).
- `LU_P7L_EVAL_DIR` — existing trusted absolute writable folder for a persistent original session journal and `create_new` receipts.
- `LU_P7L_SOURCE_HEAD` — exact source commit SHA; `LU_P7L_HARDWARE_ID` — sanitized anonymous local hardware label.
- Optional `LU_P7L_THREADS` (1–64, default 4), `LU_P7L_TRIALS` (5–20, default 5).

The real-GGUF fixture independently hashes full original binary/model files, launches a new pinned llama-server and Kobold child for each trial, alternates baseline then candidate/candidate then baseline, and issues identical fixed `text_exact` and `text_code` prompts. Receipts include first event-forward time, total response time, exact-answer rubric results and SHA-256 of original output without persisting the output text. **Actual token throughput remains null** when native decode token accounting is unavailable; fabricated tokens per second are never reported. It writes `lu-perf01-p7-receipts-v1` to the trusted directory, then:

`node scripts/perf01-p7-backend-comparison.mjs --input <original receipt> --head <exact git SHA>`

The analyzer reports p50/p95 and strict sample counts but never attests independent physical acceptance or promotes a backend. **The normal llama.cpp baseline requests 99 GPU layers on Windows; Kobold first runs CPU-only**, so results are exploratory and not a controlled equal-accelerator performance comparison until two genuinely CPU-only verified engines are compared. No speedup or quality equivalence is currently proven.

**D. Real native cancellation/reload and tool streaming tests** (same ignored file):
- `actual_cancellation_kills_owned_child_and_reloads_after_clean_reap`: cancellation after first streamed token must return terminal failure, original PID must disappear, durable journal must be reaped, a later separately admitted original instance must reload.
- `actual_original_read_only_tool_sse_never_executes_commands`: authentic Kobold/GGUF must produce valid `lu_browser_read` SSE proposals under the original schema; no browser executor is invoked.
- Actual OS **guardian death**, Linux escaped descendants/cgroup reaping, Windows Job/SCM recovery across full application restart, original installer, model-specific MMProj comprehension, full model-quality parity and native GPU measurement still require separately accepted genuine hardware tests.

## Evidence actually obtained in this turn

1. Reviewed current upstream Kobold and mistral.rs documentation; verified Kobold has the documented password, loopback, GGUF/MMProj and streaming/tool API surfaces. **Upstream docs are NOT Lu runtime test evidence.**
2. Committed concrete P7L adapter, durable native process journal, real-socket SSE/tool test sources, opt-in native HTTP child fixture and end-to-end fixture tests, ignored genuine candidate benchmark and original tool/cancel evaluation.
3. **Five isolated JavaScript source-contract checks PASSED** against current fetched P7L branch (offline default, pinned native spawn, kernel custody, original tool parser, no shipping fixture).
4. Existing exact-head P7 comparison analyzer **parsed and executed in an isolated JavaScript runtime with synthetic records**: accepted five paired text/code trials with unknown token counts honestly null; rejected missing trial pairs, stale source head and fabricated decode-token fields. All results remain `installedBackendPromotionAllowed:false`.
5. JSON SSE test fragments were independently parsed in JavaScript, confirming coherent tool-call fragments and combined JSON arguments. This does **not** run the Rust HTTP client or binary.
6. **NOT RUN:** Rust/Cargo compilation and native unit/integration suites (Cargo toolchain unavailable locally and cannot be fetched from the network in this environment), actual Kobold binary/GGUF/MMProj, genuine original-child OS admission/reap/restart, physical hardware quality/performance or installed UI. No CI/PR verification was dispatched. No native success claim is permitted.

## Remaining P7L blockers

- **P7L-1 native compilation:** run one pinned Windows/Linux exact-head Cargo test and repair compiler or protocol failures, including protected Windows Job, Linux group/cgroup custody and the opt-in original process fixture.
- **P7L-2 genuine exact-binary compatibility:** source-pin original KoboldCpp executable + DLL/.so/package dependencies, licenses and startup flags; prove the installed build actually loads Lu's original Qwen/Gemma GGUF and the correct MMProj offline. Do not infer Vulkan offload from command-line acceptance (Kobold documents CPU fallback even if Vulkan is unavailable).
- **P7L-3 runtime/cancel recovery:** run the authentic streaming/structured-tool SSE tests against genuine Kobold; prove host cancellation kills original child/descendants and that after crash/restart no old session or model output can be reused automatically. Reconcile original ANSF/SQLite interrupted-turn data.
- **P7L-4 real paired quality/perf:** run 5+ alternate original-model A/B trials per text/code + vision/tool/cancel/restart modality with comparable CPU baseline first, then independently verified Vulkan Radeon if desired. Compare TTFT/throughput/quality/RSS/VRAM/energy and native functionality; reject improvements bought by missing output or a weaker permission boundary.
- **P7L-5 product admission:** only a separate explicitly approved release after complete P0-SEC, P1–P7 and physical Windows installed acceptance can make the alternative selectable. Until then keep the original pinned llama.cpp runtime authoritative and the candidate opt-in test only.

**Next:** P7L-1 exact-head Windows/Linux native compile and test result reconciliation; followed by P7L-2 actual package + GGUF test. **Operational P7L is not complete**; source functionality has advanced substantially, no alternative backend has been promoted.
