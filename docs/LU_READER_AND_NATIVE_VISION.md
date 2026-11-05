# Lu Core — Reader Tool and Native Vision Capability Routing

Status: **In progress.** Bounded text, initial DOCX, XLSX and PPTX extraction, typed Reader registration and a native file picker are in source. Exact-head Windows/Linux testing and installed desktop acceptance remain open. basic PDF text-layer reading and character-offset paging are implemented in source; OCR, complex PDF support, native workbook range/slide selection and legacy format compatibility remain open.

## Decision

The old standalone "OCR tool" concept becomes **Reader**. OCR is only one
text-extraction adapter. The Reader serves the general job of extracting,
normalizing, citing, and paging source text into the model's context without
pretending it understood a document's graphics or embedded code.

Image understanding is *not* a generic describe-image tool. When a local
multimodal model and its inference backend have proven image support, use
the model's native visual understanding. Otherwise, Reader may OCR an image
**only when the user wants the text in it**, not as a replacement for visual
reasoning.

## Reader format contract

| Source | Intended adapter | Status |
|---|---|---|
| TXT, MD, log, config, CSV/TSV, JSON/JSONL, code, YAML, TOML | Native UTF-8/UTF-16 bounded text normalization | Initial Rust core implemented |
| HTML, XML, SVG source | Literal source reader first; optional safe semantic extraction later | Initial literal reader implemented |
| DOCX | Bounded ZIP central-directory admission; inflated `word/document.xml` text, paragraphs and tables, XML entity decoding, tracked deletion omission | **Implemented in source; native cross-platform tests pending** |
| PDF with text layer | Conservative classic PDF page contents, ASCII/UTF-16BE text strings and plain/Flate-compressed text operators; emits [Page N] anchors | **Limited adapter in source; not comprehensive** |
| Scanned PDF | Render page with strict resource limits, then OCR adapter | **Not implemented** |
| JPG, PNG, TIFF and other document photos | OCR adapter, orientation and page/region provenance | **Not implemented** |
| XLSX | Initial workbook/worksheet order, cells, shared strings, raw formulas and stored cached values; chunk/range paging not yet present | **Implemented in source; CI pending** |
| XLSM | Macro-enabled workbook reading without macro execution | **Not implemented** |
| ODS, legacy XLS | Separate OpenDocument and legacy binary adapters; same typed range contract | **Not implemented** |
| PPTX | Initial slide order and text runs, slide number references; notes, diagrams and alt text not yet extracted | **Implemented in source; CI pending** |
| PPSX, PPTM | Slide show and macro-enabled variants | **Not implemented** |
| ODP, legacy PPT | Separate OpenDocument and legacy binary presentation adapters | **Not implemented** |
| ODT, RTF, EPUB and legacy DOC | Separate vetted format adapters | **Not implemented** |
| Encrypted, corrupted, unsupported and unknown | Explicit error; never claim successful extraction | Required |

The `lu-core::native_reader` module handles supported TXT, DOCX, XLSX, PPTX, basic PDF
and other text formats, returning source-anchored text. It deliberately rejects
complex/scanned PDF, image OCR, legacy/binary Office and unverified package types;
no skipped format is silently claimed as extracted. **A native Reader registry entry and model schema are implemented** in `lu-core::native_reader_tool`; the desktop must still install it into the authorized per-turn tool catalog.
MIME/signature validation and host-authorized file handles belong in the
admission layer; filename extensions alone are insufficient.

Reader outputs should use typed Protobuf messages for source ID, detected
format, extracted text segments, optional page/paragraph/line/region labels,
truncation, confidence and structured warnings. The model must request bounded
pages/chunks rather than receiving arbitrarily large files in one prompt.
Preserve text order and provenance where possible. OCR confidence does not
prove accuracy.

## Structured spreadsheets and presentations

Spreadsheet reading belongs inside Reader, not a new mandatory core tool.
A workbook is **not** a single plain-text file: the host should return a
bounded worksheet catalog (stable source identifier, names, dimensions) and
permit targeted `sheet + range` reads (e.g. `Sales!A1:D25`). Every returned
cell preserves its coordinate, type, value and optional **formula text and
cached value**. Cached values can be missing or stale: Reader never silently
recalculates or presents them as verified current results. Dates, merged
cells, hidden sheets/rows, external data references, chart metadata and
truncation must be surfaced explicitly. Editing, recalculation and workbook
execution are separate authorized capabilities.

Presentation reading also belongs inside Reader. A slide deck should expose
an ordered slide catalog, title, selectable text in reading order, text boxes,
tables, speaker notes and provenance such as `slide 6 / shape 3`. Grouped
shapes, diagrams, chart series, image alternative text and slide images need
separate bounded adapters. A **presentation renderer** or verified native
multimodal vision may inspect slide appearance; raw XML text extraction alone
cannot prove what the slides visually communicate. Embedded video/audio and
scripts are not executed or followed. Hidden slides and notes should be
clearly labeled so the model does not confuse them with visible content.

Both XLSX and PPTX use compressed Open XML packages, so parsers should share
the same strict central-directory checks and resource budgets as Reader's
DOCX parser, but **must verify each format's relationship and content type
structure independently**. Do not simply rename a ZIP file to another office
extension or assume a DOCX parser supports arbitrary Office packages.
Encrypted packages, external relationships, macros, object linking, ZIP
bombs and oversized XML parts must fail closed.

These are **partly implemented format adapters**: initial XLSX cell/formula and PPTX slide text code exists but structured inspect/range/slide paging, notes and enhanced provenance do not. Keep Reader
as one user-visible family while dispatching internal format-specific,
sandboxed extractors. A future typed API should include `reader.inspect`
(metadata/catalog) and `reader.read` (bounded segments, ranges or slides)
rather than dumping multi-thousand-row workbooks and entire decks into the
model context.

## Security

- Host-controlled local file handles and per-turn `ReadWorkspace` grants;
  discovery does not authorize file access.
- Input size, decompressed size, archive-entry count, page count, object
  count, image dimensions, time and memory budgets. Prevent ZIP bombs,
  PDF recursion/bomb payloads, document external relationships, hyperlinks,
  active content, macros, scripts and network fetching.
- Treat all extracted text as **untrusted source data**, never an instruction,
  tool grant, prompt override or trusted system message.
- Run risky parsers/OCR in Lu-owned sandboxed subprocesses using TOOLS-11
  supervision and terminal events. No installation-time Python dependency.
- Preserve source anchors for user-verifiable quotes; do not make claims
  about unread/omitted segments.

## Native vision capability detection

`lu-core::vision_routing` defines a conservative policy:
`VerifiedNative` requires three pieces of **host-attested** evidence:
1. The current selected inference backend genuinely supports image input.
2. The chosen model and admitted visual projector/configuration are valid
   for that backend (a nearby MMProj or model name alone proves nothing).
3. An actual bounded image inference probe completed successfully on this
   session. Evidence must be invalidated when model, projector, engine or
   session changes.

Do not treat a GGUF magic header, catalog filename, an attached `.gguf`,
or a model card as definitive proof of functioning vision.

For requests to understand photographs, charts, diagrams, or scenes, prefer
native visual inference when verified; otherwise report no supported image
understanding route. Do not silently generate captions via OCR.

For requests to read text from images and scanned documents, prefer verified
native vision if the task and model are suitable, or use Reader's OCR adapter
when available. OCR is a transcription fallback, not a generic describe-image
tool. Both options can be offered for comparison when quality matters.

The routing policy is implemented in Rust, but **the live backend probe and
session evidence publisher are not yet wired to the desktop/model manager**.
Until then, Lu must treat vision support as unverified, never advertise it
as automatically detected.

## Implementation gates

1. Reader text kernel, format classification, binary refusal, byte and
   output bounds, UTF-8/UTF-16 tests. **Implemented in source.**
2. Typed Protobuf Reader invocation, host workspace-relative root capability,
   deny-by-default authorization boundary, catalog registration and rejection
   tests. **Implemented in Lu Core; desktop host integration pending.**
3. Secure DOCX extraction from main body (paragraphs/tables and inserted text; deleted tracked text omitted), strict ZIP and XML limits, CRC, no external relationships. **Implemented and CI verification pending.** Headers/footers, comments and richer page provenance still open.
4. Workbook adapters: XLSX initial sheet names, cell references, strings,
   formulas vs cached values. **Implemented in source; CI pending.** Bounded
   range paging and ODS/XLS/XLSM compatibility remain open.
5. Presentation adapters: initial PPTX ordered slides and textual runs.
   **Implemented in source; CI pending.** Notes, tables, nontextual visual
   elements and ODP/PPT/PPTM/PPSX compatibility remain open.
6. Basic classic PDF text-layer extraction with page anchors and compressed
   stream decoding. **Implemented in source and Linux CI verified**; complex
   fonts, object streams and scanned pages explicitly unsupported. Full PDF
   parser and robust page selection/render/OCR remain open.
7. Progressive Reader: one typed tool now supports bounded
   `inspect_only` preflight and `offset_chars` / `max_chars` paging,
   with explicit `next_offset_chars`, source truncation and total extracted
   character count. Requests can use `sheet:Name`, `slide:N` or `page:N`
   to extract only a selected part of XLSX/PPTX/PDF. Every response carries
   `source_sha256`; subsequent reads can supply `expected_source_sha256`
   so edits between pages fail closed rather than mixing document revisions.
   Desktop picker supports document attachments.
   **Implemented in source; latest CI pending.** Per-turn model tool
   registration in actual desktop inference, proper spreadsheet *cell-range*
   selection and long-document streaming beyond the 200K extraction cap
   remain open.
8. Vision session probe and backend capability attestation, per-model
   invalidation and native-vision-first routing integration.
9. Cross-format fixture tests, malicious/oversized documents, Windows/Linux
   process isolation, installed desktop acceptance.

## Candidate libraries (research only, not integrated)

- `docspec-docx-reader`: streaming DOCX XML event adapter via
  ZIP and quick-xml with paragraph, tables and image events.
- `pdfluent_extract` + `lopdf`: positioned and page-scoped PDF text.
- OCR backend should be chosen after native packaging, security and quality
  measurements; avoid an implicit system dependency.

Neither library is adopted without independent fuzz/size limits and a
verified release/package story for Lu's single installer.

## Repository scope

Lu Core owns the Reader abstraction and capability router. Specialist
visual understanding stays with compatible multimodal AI backends.
The TOOLS-11 task manager supplies supervised parsing for lengthy files,
not a separate bespoke video/audio understanding engine.
