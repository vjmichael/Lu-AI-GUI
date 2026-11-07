# Lu Core Tool Completion — implementation waves

2026-11-06. This work is intentionally **source development first**; GitHub native/desktop/installed-app tests are independent downstream acceptance. Stacked on draft #61, not merged to the alpha. Older PR and CI cleanup is deferred pending a replacement integrated candidate. This plan **does not** authorize weakening security boundaries or adding unreviewed runtime dependencies.

> **OWNER PRIORITY CHANGE (2026-11-06):** This original A–F wave table is retained as historical source-development evidence, **not** the priority order for future expansion. The owner explicitly re-ordered future tool work as P01 Filesystem Discovery, P02 Background Tasks + Resource Guardian, P03 AI Operations + Model/Provider Management, P04 Unified Reader/OCR, P05 Browser + Transfers, P06 Observe, P07 Research, P08 Split Archives/ISO-UDF, P09 Office, P10 Basic Image Editing, P11 Basic Core Git, P12 Internal JSON/Protobuf. Canonical scope: [tools priority decision](LU_CORE_TOOLS_SCOPE_AND_PRIORITY_20261106.md); actionable status: [implementation ledger](LU_NATIVE_TOOLS_IMPLEMENTATION_LEDGER.md). No new work starts from this update; wait for the owner's next instruction.

| Wave | Implementation target | Definition of source completion (separate from CI acceptance) |
| --- | --- | --- |
| A — Immutable Image Workspace | Deterministic image transforms, solid redaction, region mosaic, graphical annotations, revision history and safe operator UX | No source overwrite or model-selected path; canonical PNG output with precommitted receipt; return explicit unsupported for generative edits |
| B — Reader and OCR | Pinned offline OCR adapter with image handling, explicit extraction provenance and user-visible review | No executable provided by model, no network, no OCR-as-source-of-truth without reviewing captured text |
| C — Archive / document sophistication | Existing format engines and transactional grouping improved without loss of originals | No claim of blanket encrypted/UDF/PDF editing support where the source parser is insufficient |
| D — Browser and Research | Durable source/claim relationships, publisher provenance, bounded large research pagination and report-quality safe output | No model-authored citation URL; no claim that observed exact passage proves factual truth |
| E — Observe & Learn | Minimized event-driven observation, safe redaction and independently reviewable handoff to larger model | OFF by default; explicit chat/session/target scope; no silent surveillance or fabricated GUI pass |
| F — Unified host lifecycle | Process and cancellation receipt hardening, consistent return/timeout bounds and Development Ledger integration | Native and installed test gates remain OPEN pending real GH CI and physical Windows |


## Follow-on tools workflow pass — OCR, Reader, image interface (2026-11-06)

**Owner priority:** continue native source development; old GH operations and installed acceptance are lower priority. This work is source code committed in the stacked `lu/reader-ocr-image-closeout-20261106` branch, not a release or native-test claim.

| Work order | Functionality implemented with existing components | User interaction / invariant |
| --- | --- | --- |
| OCR-PAGES-03 | Extend the **existing** offline, reviewed PDF OCR command to accept 1–8 consecutive pages (1–32). It calls the previously pinned Poppler + Tesseract chain for each page, preserving each page's distinct original PDF SHA-256, rendered PNG SHA-256, OCR text hash, OCR engine and rasterizer hashes. The entire batch fails on any page failure, with the same single conversation-owned cancellation token. | One OS-picked PDF and **one native approval**; page selector in the image viewer; each unverified page remains individually editable/reviewed and inserted into the chat draft separately. A source is never reopened between pages. Without the approved rasterizer binary, OCR refuses the operation. No automatic citation authority. |
| READER-SELECT-03 | Connect the existing `native_reader::read_selected_bytes`, `read_plain_text_window` and digest-based Reader continuation to the regular native `lu_reader_pick_document` command. Reader now admits the UI-supplied read **selector only**, plus an offset and expected SHA-256. Complex document reads remain bounded to 200,000 extracted characters; ordinary UTF-8/UTF-16 text can continue through the full 8 MiB input. | Desktop `Reader section` input accepts `page:N`, `chapter:N`, `notes:N`, `sheet:Name`, `range:Sheet!A1:D10` and no selector for general text. `Continue Reader` reopens an **OS picker** and refuses a different source SHA-256. No model-provided filesystem path or silent grants. Picker filenames reject controls and bidirectional overrides. |
| IMAGE-POINTER-06 | Add a bounded native **arrow** annotation using the same immutable staged operations; source-pixel mapping now also works on the **staged native preview**, so crop/rotate/resize editors can drag their next rectangle directly on current staged dimensions. Reuses preview identity key, current plan, native geometry validation and operator-approved new-file export. | Staged overlay is keyed to the exact preview/chat/assembly; changes clear stale selections. Selecting never writes pixels, files, prompts or tool grants. Original-image drag remains the separate OCR/original-coordinate input. |

**Verification boundary:** New unit/contract fixtures updated. No fresh native Windows/Linux build, installed NSIS acceptance, licensed PDF rasterizer installation or real OCR/document corpus performed in this pass. Those gates are deferred, not green. No FFMPEG or network fallback.

**Truly open product gaps:** PDF ToUnicode/CMap/object-stream decoding; long-form Office extraction beyond the conservative cap; historical .doc/.xls/.ppt import; rich EPUB/RTF typography and accessibility; OCR confidence/paragraph geometry and robust scanned-PDF corpus/packaging; image text/brush layers or generative edits; independent runtime containment assurance. Do not duplicate already working read-only parsers, existing OCR engines, revision history, or preview/export flows.

## Image Workspace source continuation — IMAGE-WS-FILTER-05 (2026-11-06)

**Source implementation only; acceptance deferred.** Reused Lu's existing original-hash-pinned PNG/JPEG admission, deterministic `ImageRevisionOperation`, transient native preview, ordered 12-step assembly, undo/redo, operator-approved NEW-PNG export and durable APPLIED/UNCERTAIN receipts. Added six native color/spatial adjustments: **grayscale, RGB inversion, bounded Gaussian blur, brightness ±128, contrast ±100 and sharpening** with bounded radius/threshold. Native filters explicitly retain the original alpha channel. They do not constitute secure redaction; only existing solid pixel redaction does.

The desktop now exposes these filters in the same edit form and staged/export pipeline, validates the parameter bounds in both web geometry and native Rust and maintains the current image dimensions. A user can **drag a rectangle on the actual original-image preview** to select source pixels: the WebView converts the displayed (possibly zoomed) pointer geometry to original pixel coordinates, shows a selection overlay, fills the OCR region, and fills editing fields only when no staged edits exist. Later staged edits still use the stage's coordinate system; the native transform revalidates all geometry and SHA identities. A gesture creates no filesystem, model or network authorization.

**Out of scope and not falsely claimed:** natural-language/generative inpainting, AI masks, layer compositing, text/font/brush annotations, rich GPU effects, color-managed HDR, automatic background removal, source file overwrite, real-model multimodal proof, native/installed acceptance. New Rust and Node regression fixtures are authored but have not been executed in this work order.

## Reader source continuation — RTF and EPUB (2026-11-06)

**Work order READER-CLOSE-02 — source implemented, independent acceptance deferred.** Reused the same model-visible `lu.reader.read`, native picker, immutable source SHA-256, caller-supplied `expected_source_sha256` paging and capability/policy checks. Added conservative read-only **RTF Windows-1252/RTF Unicode** extraction with nested group and control parsing, object/font-table suppression, bounded expansion and explicit unsupported encoding. Added **EPUB text spine** with the existing ZIP central-directory admission (12 MiB max total expansion), validated EPUB MIME/container, package-root scoped paths, encrypted-book rejection, chapter ordering and `chapter:N` selection. XHTML body text is extracted without script execution, resource loading or CSS interpretation. Standard Reader selectors for PDF pages, sheet ranges and slide notes are unchanged. The desktop Reader picker now admits ODT, RTF and EPUB extensions.

**Not done:** no rich RTF formatting fidelity, multimedia EPUB presentation, DRM bypass, PDF font CMaps/xref object streams, arbitrary 200K+ whole-Office text extraction, large corpus/parser sandboxing or external office compatibility guarantees. Do not treat a conservative adapter as 100% fidelity.

## Reader/OCR and Image Workspace bounded continuation (2026-11-06)

Branch: `lu/reader-ocr-image-closeout-20261106` stacked on the previous preview source branch. Implemented real bounded **ODS range**, **ODT text**, **PPTX/ODP notes** Reader adapters; extended existing digest-pinned offline OCR to current admitted attachments with native consent and image decode preflight; added native ellipse/line deterministic annotations; corrected staged-edit coordinate validation and inferred same-chat revision parent links from exact SHA-256 history. No new model-facing filesystem or browser authority. Original files remain unchanged.

Release exclusions: scanned PDF rasterization, PDF CMap/object-stream completeness, rich visual Office rendering, autonomous semantic image editing, true filesystem undo, long-running hostile parser isolation, native Windows/Linux CI, installed OCR/NSIS/GGUF acceptance are NOT declared complete.

## Continuation after C/E/F — IMAGE-WS-PREVIEW-01

Branch `lu/tool-completion-preview-20261106` (descendant of C/E/F source checkpoint). A read-only Tauri command now previews 1–12 staged image edit steps using the exact native deterministic transform. The transient local PNG is capped at 4 MiB, never saved or supplied to the model, and is displayed only while chat, original image reference and staged operations still match. Human-reviewed native no-clobber export and the original image remain unchanged. Preview unit/structural tests are committed as source; they have not executed. Version graph/advanced semantic edits and native/installed acceptance remain open.

## Batch closeout — Waves C, E and F (2026-11-06)

**Branch:** lu/tool-completion-waves-20261106 (descendant of PR #61). These are committed source changes. **No native, CI, installed NSIS, visual, network or performance tests were executed in this batch**, by user direction. Existing draft PRs and stale CI were left untouched.

### C — archives and document revision: source-implemented within conservative formats

- New native_archive_batch.rs and archive_batch_desktop.rs: OS selection of **1–8 existing archives**, full source tree review, **512 MiB expanded / 4,096 entries / 900 KiB immutable manifest** maximum, new group output, separate native approval, host cancellation and member-by-member SHA reconciliation.
- Each accepted group effect has a typed Protobuf ArchiveBatchExtractReceipt. A cancellation or partial write leaves an UNCERTAIN journal and an incomplete new folder for review, not an invented all-or-nothing success or silent retry.
- Wave 2 recovery now records the entire approved group manifest BEFORE mutation, and after restart independently rechecks group, marker and each file. Symlink/reparse directories and linked completion markers do not qualify as verified.
- Word and PowerPoint use one safe host-owned edit flow: exact whole-run replacement (maximum 32), source/revised SHA check, independent human approval, new-file publication and durable effect/recovery receipt. PPTX existing slide text nodes are handled across slides while unrelated OOXML parts retain the same uncompressed content.
- **Limits:** Styled-run-spanning replacement, complex charts/layout/OOXML, encrypted/split archives, complex UDF/ISO and scanned/encrypted/signed/macro-bearing documents are not blanket-supported. Batch publication is intentionally not transactionally atomic.

### E — Observe & Learn: user-owned investigation bridge in source

- Real Lu model-supervisor failures can create content-minimized native-origin awareness events while Observe is explicitly consented. Renderer hints are never independent GUI verdicts.
- Windows WGC capture returns only a read-only task/frame digest, not screen pixels, window titles, sound, source identity or model visual understanding.
- ObserveHost prepares a bounded SHA-256 fingerprinted review prompt from scoped, enumerated native awareness notes, with explicit verified-vs-unverified evidence labels.
- The UI requires an explicit Add observed evidence to chat draft action and manual Send. No inference, memory edit, source edit or tool grant occurs automatically.
- **Limits:** No independent authenticated external-app visual perception, autonomous mini-model execution or self-repair. Observe remains OFF by default, scoped, and revoked on pause/stop/expiry/session switch.

### F — workers, cancellation and durable outcomes: source-integrated

- Research campaigns cancel their pinned running curl subprocess during OpenAlex, Crossref and publisher-document reads. Direct document fetches have their own per-chat cancellation command; cancelled documents cannot be silently promoted to accessed evidence.
- Start, resume and review tasks now distinguish worker-ended CANCELLED from COMPLETED and FAILED, with SQLite task receipt. No task is considered cancelled while its owned subprocess is still running.
- Archive full-tree preflight, source SHA rechecks and group publication use a shared host cancellation token.
- Offline OCR rechecks both executable and English-trained-data digests at use; bounded stdin/stdout worker threads prevent deadlocks; timeout kills/reaps the worker. Windows package staging exports both digests, without implying the binary/license is ready.
- Word, PowerPoint and archive-batch writes have durable human-turn effect intents and exact typed receipt checks prior to any claim of completion. Uncertain effects are never automatically replayed.
- **Still independent:** exact-head native compiler and test results, real Windows Job Object/process-tree hostile cancellation, concurrency/crash recovery, licensed package/installer contents, long research runtime, GGUF/Vulkan/NSIS installed acceptance.

**Development closeout:** Waves C/E/F received the bounded source implementation and native UI integration. Explicit unsupported advanced formats, autonomous visual diagnosis, and all native acceptance gates remain open. Lu v1.0 is not declared released.

---
**Execution policy:** Implement each applicable component on this branch, commit source changes, update the central `LU_NATIVE_TOOLS_IMPLEMENTATION_LEDGER.md`. No native tests are being executed within this work order; tests and old PR housekeeping are deferred as the user requested. Do not conflate untouched scope, mock-based source, or unsupported archive/Office cases with complete, released capability.
