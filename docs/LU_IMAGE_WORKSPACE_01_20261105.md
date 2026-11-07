# LU-IMAGE-WS-01 — Human-first, AI-ready Image Workspace
Date: 2026-11-05
Status: contract and image-edit intent validation only; no image-processing engine or installed UX acceptance.

## Purpose
On opening a persisted Lu image attachment, show a zoomable viewer with controls for markup, comment, resize, crop, erase, background removal and natural-language edit requests. The user and Lu share the same image revision, selections and operations, without model-driven GUI clicking.

## Scope and sequencing
This is a shared workspace feature, **not** a prerequisite for the current TOOLS-01–05 unified native-tool closeout (PR #34). Preserve accepted alpha.2 image upload, display, persistence and original bytes. Implement deterministic image operations before adding optional generation/inpainting backends. A dedicated editing model is distinct from a GGUF vision model; native vision can understand a picture without being able to alter its pixels.

## Common object and authority model
- Asset identity: immutable `asset_id` tied to the host attachment record, `base_revision` and content hash. Regions use pixel-space coordinates relative to the declared base revision and width/height; never treat screen coordinates as persistent locations.
- Human gestures and Lu tool calls produce the same typed `ImageEditIntent` proposal. Model-provided asset IDs, paths, masks and arguments are untrusted.
- Host resolves the current asset, confirms a matching revision and authorized source turn, validates geometry and admission limits, then binds a backend selected from the host capability registry.
- **Never overwrite originals.** Every accepted edit generates a new immutable revision, provenance/recipe, optional derived mask, verified output hash and preview. Save/export to another filesystem target needs its own scoped write grant and journal receipt.
- Edits use a prepared/applied/uncertain receipt with idempotency key, cancellation/restart reconciliation, atomic publish and retention of prior revision. Uncertain non-idempotent operations must not be silently repeated.
- An AI-proposed edit is shown as a preview for user approval before committing unless a matching explicit target/effect-scoped grant permits it. A comment or markup has no implicit filesystem authority.
- Image bytes and prompts remain local by default; any optional remote image-edit service requires separately explicit disclosure and permission.

## Phase 1: viewer and deterministic editor
Open, zoom/pan, inspect metadata, before/after, undo/history, region select, crop, resize, markups, region comments and export. Input admission: magic-byte MIME verification (currently PNG/JPEG), decoded dimensions/pixel/memory/bytes ceilings, rotation orientation handling, thumbnail jobs and cancellation. Never assume that the filename is trustworthy.

## Phase 2: optional AI-powered edits
Background removal; region erase/inpainting; prompt-directed transformations. Declare availability at runtime per backend. If no compatible backend exists, disable the action with a clear explanation, not a fabricated result. Preserve masks and revision-relative coordinates across model calls.

## Capability registry
Descriptors: `lu.image.inspect.v1`, `lu.image.crop.v1`, `lu.image.resize.v1`, `lu.image.annotate.v1`, `lu.image.comment.v1`, `lu.image.remove_background.v1`, `lu.image.erase.v1`, `lu.image.edit_prompt.v1`, `lu.image.export.v1`. Each advertises input/output schema, supported MIME and limits, backend, effect class, recovery, cancellation, preflight, verification and availability. Allowlist only context-relevant actions. Tool discovery never grants an effect.

## Minimum tests and acceptance
1. PNG/JPEG magic-byte admission, decompression/pixel budget, incorrect MIME and malicious path tests.
2. Stale image revision, malformed region, negative/overflow geometry, unexpected operation, missing masks or prompts reject with no filesystem effects.
3. Pixel-space selection maps correctly under zoom, pan, device-pixel scaling and EXIF orientation.
4. Immutable original remains byte-identical after crop/resize/markup, undo, crash and restart; hashes prove published revisions.
5. Cancellation mid-edit and kill during prepared/published journal boundaries recover without duplicate revision or unintended overwrite.
6. Actual installed Windows Lu opens an existing persisted image and edits/exports it; native GGUF vision model may inspect that image, but unavailable editing backends are accurately labeled.

## Dependencies and next gate
Keep the implementation out of the PR #34 tool integration branch until native tools pass exact-head CI and authorized desktop dispatch. This PR adds a strict intent contract and tests only; follow-up requires Rust native engine, model capability bridge, desktop UI and installed acceptance. Document each evidence level in the central ledger rather than the release changelog.
