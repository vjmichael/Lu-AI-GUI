# Lu — Canonical repository and R16 migration handoff

**Canonical source repository:** https://github.com/vjmichael/Lu-AI-GUI  
**Source baseline:** R16 independent Lu-only source, imported November 1, 2026 (Arizona local time).  
**Release status:** Windows development evaluation only; not alpha-approved.

## Source transfer provenance

- Original archival source: `vjmichael/jan_alt`, branch `lu/standalone-clean-tree-r16-20261101`, commit `282eba031ef06821fb5d1440154838700a5d33e5`.
- Source imported with a temporary, one-shot pinned workflow into `vjmichael/Lu-AI-GUI`, producing commit `e203b4dc16663b06110fd5776c0bc5f51c749f2a`.
- The temporary workflow was deleted after success in commit `6b77fcaaf8335e6f75be0418bad5c4ee3fc684bf`.
- Verified after deletion: **190 of 190 tracked source blobs byte-for-byte identical by Git SHA**, no missing or extra files.
- This repository starts its **own Git history**. Jan's Git commit ancestry was not imported.
- Current top-level source: `lu-desktop/`, `src-tauri/` (Lu Core and Windows worker crate), `docs/`, README, LICENSE, NOTICE, .gitignore.
- No Jan desktop/app directories were copied. Historical source names, optional data-migration adapters, and platform/dependency traces must still be audited; a clean tree alone does **not** prove complete runtime independence from all Jan-era components.

## Development policy

- Develop and test locally whenever possible; GitHub is the source repository and documentation/milestone record, **not the development hub**.
- Use GitHub Actions selectively when a real Windows runner or NSIS packaging verification materially helps. Do not block local development on CI.
- Do not copy the Jan desktop, Cowork, extensions or inherited application code into this repository.
- Do not integrate Cognitum into Lu Core. It remains an optional add-on/integration.
- Maintain one coherent cross-platform Lu application, rather than independent Windows and Linux products.
- Reuse the documented dependency artifacts and known locations; do not rediscover or redownload supplied packages without verifying the inventory first.

## R16 baseline and outstanding Windows alpha work

**Implemented locally and checked before migration:**
- Standalone Tauri/React desktop, GGUF/MMProj import and native Lu model control.
- R15 conversations: New Chat, favorites, single-level folders, individual durable chat journals, legacy conversation migration, and quick model selection that never auto-loads.
- R16 model storage: one opaque model directory per new GGUF with its bound MMProj, while discovering existing flat-layout models without moving them automatically.
- R16 Windows-target Rust check; Lu Core import tests; R15 frontend checks. These are development tests, not installed-app acceptance.

**Still open or deferred:**
1. Build a new R16 standalone Windows NSIS installer from this repository; none was released for R16 at migration time.
2. Windows installed-app tests for R16 model directories, existing-model discovery, persistence/migration, cancellation/restart and process cleanup.
3. Verify AMD Radeon Vulkan layer offload on a real Windows system, with independent engine logs; device listing alone is insufficient.
4. Verify multimodal GGUF + MMProj image-conditioned responses; the prior installed-app image test was promising but displayed an image-reference cleanup error, addressed in R15 source and requiring retest.
5. Integrate and accept the native tool/authorization/ANSF orchestration path end-to-end in the independent desktop.
6. Loading progress indicator deferred by product decision; avoid claiming true percentage progress without backend progress telemetry.

**Physical testing note:** A prior model-loading test was deferred because LM Studio was using Flint concurrently. Do not record this as a Lu/GPU regression without a controlled rerun.

## Next step

Use this repository for Lu R17 development. Start with a repository-local source build and model import regression tests, then produce and physically test the next standalone NSIS development installer. Keep the alpha release gates independent from development-only evaluations.
