# Lu R22: typed conversation transport, multimodal diagnostics and alpha → v1.0 gates

**Source repository:** `vjmichael/Lu-AI-GUI`  
**Development status:** Engineering integration; not Windows-alpha approval.  
**Priority directive:** Close the agreed alpha capabilities and then the v1.0 core. **Defer the JSON-repair feature** while these are being implemented. GitHub is a source and release repository, not the development hub.

## What the November 1 Windows R21 evaluation established

- The existing R21 image admission accepts a content-correct PNG after renaming the misleading `.jpg` extension. This is an intake issue, not proof of working image reasoning.
- Screenshot paste reached llama.cpp but one submission failed as `llama server rejected multimodal request`. The previous generic error hid the HTTP status. A model loaded in the GUI and an MMProj catalog badge do **not** establish that the server accepts every image request.
- When asked to read text from a previously provided picture, Lu repeated `User:` / `Assistant:` text rather than explaining the image. The legacy text backend submitted a *flattened* conversation transcript to llama.cpp `/completion`, inviting the model to continue artificial role labels instead of replying in the correct role.
- A Markdown-formatted EVEmu repository identification attachment was processed and summarized in the desktop. The supplied document identifies `EvEmu-Project/evemu_Crucible`, revision `7d5e2061`, Crucible 1.6.5 client, C++17/CMake, MariaDB and only three registered tests. This demonstrates a successful text-document path **for that example**; it does not validate arbitrary files or PDF/DOCX.
- R18's cross-chat FIFO displayed queued turns correctly; cancellation and response attribution still require continued stress testing and process-cleanup checks.

## R22 source changes

1. `InferenceGenerateRequest.history` adds typed `InferenceChatTurn{role,text}` messages to Lu's Protobuf contract (additive field 6). Roles and limits are validated before worker dispatch, not inferred by parsing a plaintext `User:` label.
2. The desktop reads recent complete entries from the originating conversation's host-owned journal. Interrupted assistant fragments are excluded while prior user questions are retained. The current request remains its own user message.
3. The supervisor binds the typed messages to the same generation reservation and cancellation ID. The existing single-model FIFO, session ownership and host-pinned image admission are unchanged.
4. Both text and image generation now use llama.cpp's authenticated loopback `/v1/chat/completions` with structured messages. External llama.cpp still speaks JSON/SSE; **internal Lu control remains Protobuf**, and this is not the deferred JSON repair feature.
5. A non-200 engine response exposes a sanitized **HTTP status code** and actionable model/MMProj compatibility guidance. No raw server response body, file content, image bytes or bearer token is included in UI errors.
6. Removed the unused legacy raw `/completion` text client and hand-built chat transcript assembly.

### Local validation

- Lu Core: **111 passing Rust unit tests**, including new structured-turn, Protobuf validation, chat SSE and rejected-image HTTP behavior.
- Lu Desktop: Windows-target **Rust type-check and test type-check pass** with the already supplied offline MSVC/SDK/Rust sources. These checks use a **check-only resource placeholder**, never an accepted installer manifest.
- Real engine: Linux CPU-only `llama-server` with `qwen2.5-0.5b-instruct-q2_k.gguf` accepted a three-message structured chat request (`HTTP 200`) and answered the Arkansas follow-up without producing `User:`/`Assistant:` role-label continuations. **This proves the chat boundary only, not image inference on the AMD GPU.**

## Alpha acceptance gates — must close before claiming alpha

| Gate | Current state | Required independent evidence |
|---|---|---|
| A1 Independent Windows Tauri/React | Development build demonstrated | Installed-app launch, repeat use, GUI startup/shutdown, uninstall/reinstall |
| A2 GGUF and MMProj | Import/load demonstrated | Compatible model/projector identified; image-question test truly conditioned on image; clean error for unsupported combinations |
| A3 AMD Vulkan | Packaged Vulkan engine | Device and offload layer logs on physical Radeon; CPU fallback and timing recorded |
| A4 Conversation correctness | R22 typed turns; R18 queue; R15 journals | Restart recovery, interruption/continuation, simultaneous chat navigation, backlog drain and cancellation isolation |
| A5 Native tool execution | Lu Core policies/registry exist | Host-authorized GUI tool flow, per-conversation grants, effect receipts and denial tests |
| A6 Worker safety/recovery | Windows Job/Protobuf pieces exist | Process-tree termination, crash/restart, orphan cleanup, replay and rollback on physical Windows |
| A7 Packaging | R21 dev-eval installer built | New R22 or later installer from the exact integrated tree; no check-only resources; signed/versioned artifact and clean installation/uninstall tests |

No individual unit-test result is an automatic alpha PASS for its end-to-end gate.

## Core v1.0 milestones following alpha

| Workstream | Deliverable and completion criteria |
|---|---|
| ANSF memory integration | Durable per-conversation exact archive and cross-conversation search, revision-aware editing, source provenance, bounded context retrieval; memory pane connected to authenticated host APIs |
| Media continuity | Persist images as bounded, content-addressed, host-admitted conversation attachments with restart-safe references; follow-up questions can refer to prior images or clearly request reattachment; no silent text-only downgrade |
| Documents | Native PDF/DOCX/text parsing, filename + provenance, extraction limits, chunk/citation controls, restart-safe attachments, and negative/security tests |
| Native tools | Secure read/write/search/move/trash/restore/archive/git/code/JSON validation/OCR tools staged by capability; atomic effects and exact authorization; do not bypass the OS broker |
| Agent and tasks | Durable task identities, live progress, background worker supervision, pause/stop/retry, replay-safe effect journal and actionable failure classification |
| Observe & Learn (v1.0 release gate) | Explicit per-app desktop observation and GUI test evidence; default OFF, visible user controls, session leases, short-lived redacted snapshots, event-driven brief/deep inference, approved ANSF learning, separate training consent, crash recovery and installed Windows acceptance. See docs/LU_OBSERVE_LEARN_V1.md. |
| Model manager | Explicit multi-model catalog, MMProj capability checks, Vulkan/CPU settings, safe unload/reload, stable resource usage and no automatic weight loading |
| GUI foundation | Rename/delete/restore chats and folders, attachments, accessible keyboard controls, virtualized large transcripts, response actions, theme preferences; reference Jan/LM Studio behavior without importing Jan runtime |
| Runtime security | Cross-platform IPC, OS containment for worker classes, scoped filesystem/network permissions, hostile-input tests and operational audit |
| Release and quality | One installer per platform, reproducible engine packaging, real-model cross-size benchmark matrix, performance and resource budgets, smoke/integration/recovery suite and migration/rollback acceptance |

### Explicitly deferred or separately staged

- **JSON repair** remains on the design backlog; no JSON-repair engine rewrite is included in R22. Use existing strict parsing on the security boundary.
- The loading-progress **percentage** remains deferred until the backend provides grounded progress telemetry. Elapsed time is allowed.
- DLC workspaces, model councils, native image generation, optional A/V and server mode are not blockers for the **core** release unless the owner changes the release scope.

## Required physical Windows R22 evaluation

1. Load compatible Qwen GGUF plus MMProj and record llama.cpp initialization logs (including GPU layer-offload evidence). If multimodal returns non-200, note the status code; do not diagnose by repeating prompt until it loops.
2. Use a fresh chat with a single image: ask a question with an answer impossible from the prompt alone. Do not accept a generic image description as evidence of vision.
3. Ask a second question about the **same image without reattaching**. For now, mark expected behavior **not supported**: image blobs are not yet durable chat context. This is a tracked v1 feature, not a successful answer.
4. Start a fresh text-only chat: ask about Arkansas, then a follow-up, and verify there are no artificial `User:`/`Assistant:` loops. Repeat with different conversations queued.
5. Confirm cancellation preserves interrupted text; run shutdown/reinstall/restore and native-tool denial tests before alpha gate sign-off.

## Owner supersession (2026-11-04): JSON boundary reliability

The earlier **JSON-repair deferred** wording referred to a future broad repair feature, not a prohibition on the *original design's* bounded internal Model Boundary Reliability Layer. The owner explicitly requested completion of the original bounded MBRL now: strict syntax/schema checks; one deterministic punctuation normalization; up to two model regenerations before any effect; typed errors and transcript evidence; Protobuf and host authorization remain mandatory. Semantic repair, invented parameters and permission escalation are still forbidden. See `LU_CENTRAL_DESIGN.md` and `TOOLS_02_INTERNAL_JSON_BOUNDARY_20261103.md`.
