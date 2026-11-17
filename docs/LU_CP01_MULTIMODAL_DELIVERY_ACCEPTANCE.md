# CP-01D — original multimodal payload and real model acceptance

This is an execution contract, not a test-result record. The backend receipt is deliberately content-free and is not proof of model understanding or GPU residency.

## Native verification (toolchain-equipped host)

Run the exact locked source candidate, with the original Rust and UI tests. Suggested focused commands (from repository root):

```sh
cargo test --locked --manifest-path src-tauri/lu-core/Cargo.toml --lib multimodal_http::tests
cargo test --locked --manifest-path src-tauri/lu-core/Cargo.toml --lib inference_supervisor::tests
cargo test --locked --manifest-path src-tauri/lu-core/Cargo.toml --lib persistent_llama_backend::tests
cargo test --locked --manifest-path lu-desktop/src-tauri/Cargo.toml --lib attachment_store::tests
node --test lu-desktop/tests/cp01-attachment-custody.test.mjs lu-desktop/tests/cp01-submission-lifecycle.test.mjs lu-desktop/tests/cp01-mmproj-delivery.test.mjs
```

Use the normal reviewed Tauri resources for desktop compilation. The socket fixture independently compares actual received HTTP bytes to four original PNGs, not a mock assertion that a UI passed images. Negative cases cover omitted/reordered/changed/remote payload, malformed/error/truncated or token-limit streams, missing visible output, changed reservation, text downgrade and ejected worker identity. Socket fixtures are not real llama.cpp/model evidence.

## Real native model lane (explicit operator/CI opt-in)

Supply trusted expected SHA-256 values from separately reviewed binaries/weights, not values read from an arbitrary file at the time of admission. Use a reviewed matching GGUF and MMProj, a canonical model directory, model filename and valid m-32-hex model reference; the projector filename in that directory must be mmproj-for-<model-ref>.gguf. On a machine where the pinned engine and its runtime libraries are present, set:
- LU_CP01_ENGINE, LU_CP01_ENGINE_SHA256
- LU_CP01_MODEL_DIR, LU_CP01_MODEL_FILE, LU_CP01_MODEL_REF, LU_CP01_MODEL_SHA256
- LU_CP01_MMPROJ_SHA256

Then execute:

```sh
cargo test --locked --manifest-path lu-desktop/src-tauri/Cargo.toml --lib actual_cp01_saved_one_and_four_images_reach_supervised_mmproj -- --ignored --nocapture
```

This native test starts the real pinned PersistentLlamaBackend under the ordinary supervisor, stages genuine randomly ordered synthetic PNGs into the production attachment store/ANSF book, commits the foreground queue, reopens the exact saved bytes, uses typed reservation and dispatch with a real stream, verifies receipt milestones and checks exact ordered model words. Two one-image turns and two distinct four-image permutations are mandatory. No expected order in prompt/history/filename; no external model or fallback. Its output is synthetic evidence only, not installed UI or R9700 proof. No result is accepted from ignored/skipped/missing model or test fixture.

## Installed physical acceptance (CP-01F)

Use the canonical source-SHA installer, physical Radeon AI PRO R9700, reviewed Qwen3.8-27B GGUF and matching MMProj, Vulkan. Preserve binary/weight hashes and OS-owned GPU/worker witnesses separately. Through ordinary chat (not the old diagnostic vision probe), attach one unseen synthetic image and then four independently selected and ordered distinguishable images in a fresh conversation. Repeat with a different permutation; do not encode answers in names or question. Verify each answer against independent original pixels, original saved chips/order, exact native receipt (payloadVerified, uploadCompleted, visibleAnswer, streamCompleted, workerIdleVerified), original response commit and cold reopen. Do not equate transport receipt with visual understanding. Exercise missing projector, changed/missing attachment and backend failure/cancellation with original retained, no silent fallback; reload if worker was ejected. Check draft recovery and absence of duplicate original user turns. Export only sanitized evidence and synthetic oracle samples. Broad CP-12A diagnostic export and installed/physical execution remain later gates.
