# Lu P0-TOOLS-01 — Multi-document Reader in the genuine workspace model turn

Date: November 11, 2026. Lu Core only, stacked on P0-Next source 67f135b8.
Branch: lu/p0-tools01-multidoc-workspace-20261111.
Status: source candidate; no compiler, native, physical GGUF, installed UI, CI or PR verification performed.

## Gap repaired
Lu already had genuine native file discovery, scoped read, literal search, ZIP metadata and artifact inspection in its normal read-only workspace GGUF turn. Lu also had a real first-party Reader supporting text-layer PDF, DOCX, ODT, RTF, EPUB, XLSX/ODS sheet ranges, PPTX/ODP slide/notes, CSV/JSON, Markdown and source text. But those tools were separate model sessions: the model could discover a document in a folder but not invoke the full Reader in that same authenticated workspace turn.

## Source implementation
- The existing native Reader is now registered in the SAME per-turn workspace NativeToolRegistry and merged via ModelToolCatalog.absorb_registered. Its actual validated Protobuf codec and result decoder are used by the already installed supervisor → strict model JSON → native tool feedback → further GGUF generation loop. No new model, executor or document mini-app.
- One real OS folder picker, host-generated task and persisted authenticated human turn scope all read operations. ConversationAuthority expires after at most 300 seconds and is revoked at the end; no model-created root, write grant or arbitrary command.
- Ordinary Send now conservatively offers the existing native folder selector for explicitly scoped multi-document requests (summarize PDFs in the project folder, compare spreadsheets in the workspace, inspect slides in the directory), while single-document requests retain their independent file picker. Refusals, attached text, explanatory and mixed write requests are not promoted to tools.
- The actual model-facing workspace prompt now describes discovery → Reader, format/section selectors, 256 KiB default / 8 MiB source limit, SHA-256 continuity for paged reads, partial discovery flags, and the separate OCR requirement for image-only PDFs. Current-turn AI-OPS preferences prioritize discovery then Reader only when already granted.
- Reader workspace paths now reject symlinked intermediate directory components; the registered root must itself be a real directory. Scanned PDFs, handwriting and image OCR remain outside this grant, as do writes, process launch, network and acceptance authority.
- Source-level tests were written for a real two-tool JSON model loop, fake-user-turn refusal, linked folder refusal, conservative intent classification, and granted tool ranking. They were not executed by instruction.

## Remaining P0 model-to-tool work
P0-TOOLS-02: Model-proposed effectful file create/edit/move/trash/archive through EXISTING native review and per-operation authority, with durable independent effect receipts. No model-received raw write permission.
P0-TOOLS-03: Join browser/Reader/OCR/Observe source receipts and real GGUF continuation without bypassing native consent for network, screen, or OCR.
P0-TOOLS-04: Complete typed specialist application/worker interface, original OS process custody, and method-specific outcome verification beyond existing Git/FFprobe.
P0-TOOLS-05: Bind true independently verified outcomes to P0-Next source/task link, then recheck source and require separate human acceptance. No model plan auto-completes tasks.
P0-GATE: Later batch Windows/Linux compiler and installed acceptance, genuine loaded GGUF tool selection, cancellation, preservation and regression testing after remaining application development.

No CI, PR, merge or regression-testing campaign is started from this task.