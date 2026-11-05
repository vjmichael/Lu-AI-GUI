# Lu Core — Reader Tool and Native Vision Capability Routing

Status: **Design accepted; first bounded-text and vision-routing primitives implemented; format adapters and native-tool registration pending.**

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
| DOCX | Container-validated Office Open XML text/paragraph/table extraction | **Not implemented** |
| PDF with text layer | Page-aware selectable text extraction | **Not implemented** |
| Scanned PDF | Render page with strict resource limits, then OCR adapter | **Not implemented** |
| JPG, PNG, TIFF and other document photos | OCR adapter, orientation and page/region provenance | **Not implemented** |
| ODT, RTF, EPUB and legacy DOC | Separate vetted format adapters | **Not implemented** |
| Encrypted, corrupted, unsupported and unknown | Explicit error; never claim successful extraction | Required |

The initial `lu-core::native_reader` module deliberately refuses PDF, DOCX,
images and unknown types. It handles bounded text and records whether output
was truncated. **It is not yet registered as a model-callable native tool.**
MIME/signature validation and host-authorized file handles belong in the
admission layer; filename extensions alone are insufficient.

Reader outputs should use typed Protobuf messages for source ID, detected
format, extracted text segments, optional page/paragraph/line/region labels,
truncation, confidence and structured warnings. The model must request bounded
pages/chunks rather than receiving arbitrarily large files in one prompt.
Preserve text order and provenance where possible. OCR confidence does not
prove accuracy.

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
2. Secure DOCX extraction including headers, tables and tracked edits,
   with archive limits, tests and source anchors.
3. PDF selectable-text extraction with page boundaries; scanned page
   detection, bounded image conversion and OCR worker.
4. Reader tool registration, typed Protobuf schemas, per-turn host grants,
   chunk paging, provenance and user-facing file selection.
5. Vision session probe and backend capability attestation, per-model
   invalidation and native-vision-first routing integration.
6. Cross-format fixture tests, malicious/oversized documents, Windows/Linux
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
