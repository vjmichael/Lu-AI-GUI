# P10 IMAGE-BASICS-01 — Core image I/O and bounded practical edits

**Start:** 2026-11-07
**Branch:** `lu/p10-image-boundary-20261107`, stacked on P09.
**Policy:** [AI-first tool design and external application reuse](LU_AI_TOOL_DESIGN_PRINCIPLES_20261107.md).

## Decision and boundary
The model should perform visual interpretation, task planning and knowledge work. Lu provides user-authorized image input/output, deterministic lightweight transforms, safe review and durable publication. Lu **does not** attempt to become Photoshop, Krita or a generative image application. When an expert edit requires layers, masks, professional drawing, color management, complex composition or content generation, prefer a separately approved external application or library through the existing/general execution layer. Do not assume general app control is currently implemented.

## Existing source audit
- `src-tauri/lu-core/src/native_image_revision.rs`: source-digest-checked PNG/JPEG decode with bounded dimensions and allocation; immutable PNG output; crop, resize, rotation, horizontal flip, solid opaque privacy redaction, visible pixelation/mosaic (NOT privacy redaction), rectangle/line/ellipse/arrow drawing, grayscale, inversion, blur, brightness, contrast and sharpening.
- `lu-desktop/src-tauri/src/image_workspace_desktop.rs`: native image operations and volatile staged multi-step preview, no automatic save, new-file-only export receipt.
- `src-tauri/lu-core/src/native_image_revision_history.rs`: chat-local PREPARED/APPLIED/UNCERTAIN export receipt, read-only lineage, no auto-replay of uncertain output. Historical receipt does not prove the file still exists.
- React editing UX already supports stage/undo/redo of the in-memory **operation list**, geometry selection, preview and explicit new-file export. This is NOT an arbitrary history restore of filesystem revisions.
- Reader/OCR and multimodal image admission remain separate access capabilities, not an image-authoring product.

## P10 work delivered
1. Review existing bounded operations; deliberately avoid adding a new image engine.
2. Add Rust safety regressions that check solid redaction writes **opaque black** to the entire selected region even when original pixels are transparent, while preserving all pixels outside region and immutable source bytes; invalid region refused.
3. Add Rust tests for immutable two-step crop/rotate output identity and strict empty or >12 operation refusal.
4. Add Node source-contract guard on the approved basic tool boundary and host-owned, no-overwrite, PREPARED/APPLIED/UNCERTAIN publication/recovery.
5. Run exact-head Windows/Linux native and desktop-source tests. **Queued CI is NOT proof of PASS.**

## Not being built
- No drawing-engine layers, arbitrary brushes, AI image synthesis, professional composites, complex filters/color grading, image editor workspace or unsandboxed user-supplied image scripts.
- Pixelation or blur is not a trusted privacy-redaction operation; only verified opaque solid redaction is labeled privacy protection.
- No automatic permission to launch external applications or write arbitrary paths.

## Verification still needed
- Exact-head Rust image revision suite on Linux and Windows; Node regression and frozen source inventory.
- Installed Windows NSIS: original image import, preview/undo, native picker, denial, save NEW PNG, checksum, reopen source and output, cancel/restart review. Installed Linux app equivalent if supported.
- Visual comparison and physical GPU/multimodal image ingestion remain separately tracked and cannot be inferred from these tests.

**Source completion does not mean release acceptance.** The existing basic image facility is sufficient for P10 product scope unless measured use cases reveal a correctness/security flaw. Work requiring a full external graphics application is an execution-layer integration question, not a reason to enlarge P10.
