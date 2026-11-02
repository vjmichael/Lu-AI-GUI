# Lu R21 — Windows attachment admission, loading feedback and protocol findings

**Repository:** `vjmichael/Lu-AI-GUI`  
**Source base:** R20, `4dc6227cc122f8789cf5180e7d3c10d348e62173`  
**Release status:** development evaluation, **not alpha-approved**

## Physical Windows observations

- R20 queue continued to serialize three conversations in order; one conversation displayed queue position 2. This is useful physical acceptance evidence for ordering, **not** proof of parallel model execution.
- R20 Markdown and copyable fenced code displayed correctly on Windows.
- Model-loading spinner appeared static to the operator. The CSS contains a working `spin` keyframe but disables animation under `prefers-reduced-motion: reduce`, and screenshots cannot prove motion. R21 adds actual elapsed seconds while preserving reduced-motion accessibility. **Do not invent a progress percentage without engine-side telemetry.**
- Attempted drag/drop of `test-doc-image.jpg` failed. Inspection of the uploaded file confirms its bytes are PNG (1200 × 1700 RGBA, ~458 KB) even though its name ends in `.jpg`. The other supplied `test_picture-thewahlbergs.jpg` is a genuine JPEG (~136 KB). R20 called `FileReader.readAsDataURL(file)`, trusting the OS-provided `file.type`, while host-side `admit_data_url` correctly verifies file-format signatures, decoded image data and dimensions. A mismatched data URL is rejected by design.
- R20 bound `onDrop` only on the composer form, so dropping on the transcript/sidebar did nothing. R21 moves drag/drop to the shell, with a pointer-transparent overlay and the same conversation-scoped admission path.
- Model output in an Arkansas conversation answered an unrelated Christmas-song question with material about Arkansas weather. This is a **model/context-generation regression**, and should not be classified as a Protobuf or JSON parse failure without transport evidence. An objective “everyone's most hated song” has no single verified answer; the correct behavior would be to answer subjectively or acknowledge uncertainty, not respond about Arkansas.

## R21 changes

1. New `web/image_attachment_input.mjs` checks the first bytes of PNG/JPEG before passing the file to Lu Core. A PNG named `.jpg` is converted to `image/png` Data URL, while the trusted Rust host still decodes, checks sizes, validates dimensions, and pins the original bytes. Unknown or corrupted content is rejected. Filename never authorizes filesystem access.
2. HTML5 drop handling covers the full Lu desktop shell with a clearly visible overlay. `Tauri` retains `dragDropEnabled:false` so WebView2 delivers browser File objects instead of local host file paths. Dragged items are still routed to the initiating conversation's draft.
3. Model loading shows an accessible status with elapsed wall time. It intentionally does not claim a percentage, nor override a Windows reduced-motion preference.
4. Added seven focused tests for MIME sniffing, mismatched extension correction, real JPEG preservation, fail-closed unsupported content, host admission, window-level dropping and accessible loading indicator.

## Tests and evidence

- R21 UI test suite: **41 passed, 0 failed** (R20 regressions plus seven R21 focused cases).
- R20 source/bundle checks: **28 passed, 0 failed**.
- R21 frontend: esbuild succeeded, 214.8 KB JavaScript asset.
- Actual supplied files tested with the R21 normalizer: JPEG retained `image/jpeg`; mislabeled PNG canonicalized to `image/png`.
- Windows x64 native desktop executable linked; embedded Common Controls v6 manifest and PE GUI checks passed.
- NSIS **development** installer built locally from reused offline packages, with four pinned Vulkan-engine SHA-256 checks.
- Windows physical checks for R21 (drop into transcript, paste, image-conditional reply, model loader elapsed indicator) are **still pending**.

## Protobuf ↔ JSON and context-hallucination next gate (R22)

Lu's internal `InferenceFrame` and `InferenceGenerateRequest` are **Protobuf**. However, `InferenceGenerateRequest.prompt` is a **single string**, and the R20 host assembles prior turns into textual `User:` and `Assistant:` labels via `prompt_context::assemble`. The current text backend calls local llama.cpp `/completion` with a JSON body and receives JSON/SSE streaming. The image backend uses `/v1/chat/completions` with JSON/SSE and typed opaque image refs on Lu's internal side. JSON is the upstream compatibility protocol; Protobuf is not failing to be converted.

This raw text role-assembly permits the local model to imitate/continue role text, leak stray `<think>` tags, or wander into previous topics. **R21 does not claim to solve that.** The next backend task should use actual role-structured conversation messages with bounded context and model-appropriate chat template semantics, preserve host-owned turn identities, keep canceled assistant fragments out of prompt context, and separate model-generated reasoning from final content only when the backend supplies reliable typed signals. Test independent conversations and unrelated-topic switches; never rely on a model output check alone to declare transport correctness.

No external network or third-party image services are needed for the R21 frontend fix. Do not merge an unverified new chat serialization protocol into the R21 installed-app evaluation.
