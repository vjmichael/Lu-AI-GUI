# Lu Alpha 0.1 acceptance and baseline — 2026-11-03

## Decision
Project owner accepted the **functional alpha** after physical Windows testing of `0.1.0-alpha.2`. This is a **development baseline**, not a claim of release-grade stability, a signed/SmartScreen-trusted installer, or completion of Lu v1.0.

Freeze the tested installer and matching source snapshot as the reference. Never represent a later code change as part of the accepted baseline until retested. GitHub is the source repository; local compilation and physical Windows testing are authoritative.

## Acceptance observations (Windows desktop, AMD Radeon AI PRO R9700)
- GGUF plus paired MMProj model loading and multimodal image question answering work on the tested machine.
- Generation outputs text; cross-chat FIFO admission, queue-position advance after cancellation, cancellation and direct Retry worked in physical testing.
- Conversation history, text documents, and image attachments persisted across app restart. Image previews can be delayed or temporarily report unavailable, then recover.
- Image preview opens on click; saved image reused by Retry was observed.
- Native NSIS installer installs/starts the independent Tauri/Rust application.
- Version string shown in sidebar as `Lu 0.1.0-alpha.2`.

## Known defects — not silently closed
1. **Windows taskbar icon** not displayed properly. Verify Tauri window icon, compiled PE group icon / ICO resource, NSIS shortcut icon and Windows AppUserModelID grouping on Win10 and Win11.
2. **Temporary 'Lu (Not Responding)' during model loading.** Confirm real UI responsiveness using heartbeat and host lifecycle trace. Separate GPU/driver stalls from WebView/main-thread blocking; never infer root cause from screenshot alone.
3. **GGUF import progress absent.** Implement bytes-copied/total, filename, destination, completion and failure messages, cancellation, and atomic import.
4. **Model load progress** is indeterminate and labeled by actual GGUF size; don't claim percent or loaded GB unless engine supplies measured progress.
5. **Image preview refresh** sometimes delayed or temporarily unavailable during Retry; rehydrate thumbnails asynchronously without losing the attachment ref.
6. **Unsigned Windows installer** triggers SmartScreen for unknown publisher; pursue code-signing and release provenance.
7. Independent GPU-layer offload evidence, upgrade/uninstall persistence regression, extended soak and failure injection remain v1.0 hardening.

## Baseline protection
- Record exact artifact hashes from the successful build evidence, and attach binaries only after verifying their provenance.
- Keep `0.1.0-alpha.2` immutable: forward work happens in new versions / branches. Don't overwrite an accepted installer with a changed binary.
- Native tools intentionally were **not required** for alpha acceptance. No model-facing tool may execute without authenticated conversation origin, narrow per-turn grants, OS-enforced worker isolation and durable effect reconciliation.
- Cognitum is optional addon, not Lu Core. No dependency on Jan runtime or MCP for first-party native tools.

## Next
See [LU_V1_0_DESIGN_RECONCILIATION_20261103.md](LU_V1_0_DESIGN_RECONCILIATION_20261103.md).