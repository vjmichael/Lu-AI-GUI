# Lu Universal Reader — completion plan and source-of-truth

**Scope:** one native Reader family. No MCP, Python interpreter, shell, Docker,
external reader app or separate model-facing tools per format. First-party
adapters own format parsing; hostile source text never carries instructions
or authorization. For images, verified native model vision first; OCR only
for text extraction.

**Status as of Nov 4, 2026:** substantial source implementation exists on
`lu/tools-11-background-task-status-20261104`, draft PR #27. The earlier
Reader matrix passed Windows and Linux at commit `9707e69`. Newer
model-facing desktop Q&A and range changes need *exact-head* CI and real
installed-app acceptance. Never equate a CI compile with a successful
real-model tool call.

## Already implemented in source

- `src-tauri/lu-core/src/native_reader.rs`: bounded format dispatch,
  UTF-8/UTF-16 text, typed failure, selected section dispatch.
- `native_reader_docx.rs`: bounded DOCX body text, paragraphs and tables;
  no macros/external relationship execution.
- `native_reader_openxml.rs`, `native_reader_office.rs`:
  ZIP admission, XLSX sheet/cell/shared string/formula extraction and
  PPTX ordered slide text. XLSX A1 range selectors have been added.
- `native_reader_odf.rs`: initial ODS cells/formulas/repeated blank
  rows/columns and ODP slide text, with ODF content MIME checks.
- `native_reader_pdf.rs`: intentionally limited classic PDF text layer
  with page anchors, literal/hex text operators and plain/Flate streams.
  It explicitly rejects unreadable/scanned/complex PDF inputs.
- `native_reader_tool.rs`: `lu.reader.read` Protobuf tool,
  inspect-only, bounded character-offset paging, sheet/range/slide/page
  selectors, source SHA-256 pinning, cap-rooted workspace operations and
  immutable one-file snapshots. Tool registration **is not authorization**.
- `native_reader_turn.rs`: read-only, persisted-user-verifier-gated
  one-document model tool loop, bound conversation/task scope, journaled
  model interactions, automatic revocation.
- `lu-desktop/src-tauri/src/reader_desktop.rs`: explicit native-picker
  experimental 'Ask Reader' Q&A using the existing supervised tool loop,
  per-turn pinned bytes, real queued generation IDs and cancellation of
  underlying multi-step backend generations.
- `lu-desktop/web/main.jsx`: normal 'Read file' manual attachment and
  separate opt-in 'Ask Reader' button. Normal chat generation remains
  unchanged. Structured Reader Q&A does **not** stream partial tokens yet.
- `src-tauri/lu-core/src/vision_routing.rs`: native-vision-first policy
  requires backend + model/projector + live image-probe evidence; the
  actual host attestation and probe publishing remain unimplemented.

## Remaining acceptance gates (priority order)

| Gate | Work | Close criteria | Status |
| --- | --- | --- | --- |
| RDR-01 | Core native read-only tool turn | Real model JSON→Protobuf Reader request; exact persisted turn authorization; 0 cross-file access; no grants after finish/revoke | Implemented/test harness; live model acceptance open |
| RDR-02 | Desktop Reader Q&A | Windows build, file dialog, queued chat switch, cancel at tool step, saved final and restart, no synthetic user turns | Opt-in source path in branch; installed-app test open |
| RDR-03 | PDF completeness | Position/page provenance for real producer PDFs; CMaps/ToUnicode, nested pages, cross-reference/object streams where safe; explicit scanned detection | Partial classic parser only |
| RDR-04 | Document photo/scanned PDF OCR | Offline, pinned native OCR worker with strict deadlines, page/image budgets and process termination; no implicit system package; cross-platform fixtures | Not implemented |
| RDR-05 | Progressive reading | Reliable inspect/catalog→section→bounded reads for large input, backpressure, page/range indexes, searchable source anchors, no 200K extracted-character ceiling | First character paging + SHA pinning only |
| RDR-06 | Rich workbook/deck scope | XLSX/ODS cell ranges, cached-value/formula distinctions, dates, hidden/merged cells, slide notes/tables/charts and accessible visual references | XLSX range initial; other advanced features open |
| RDR-07 | Native visual-model evidence | Per-session actual image Q&A probe, backend feature proof, invalidate evidence on model/projector/backend/session changes | Policy implemented, live probe not wired |
| RDR-08 | Parser containment | Host-owned sandbox worker for risky PDF/Office/OCR parsing, hard CPU/memory budgets, worker kill/recovery, hostile documents and fuzzing | In-process bounded adapters; worker isolation open |
| RDR-09 | Compatibility adapters | ODT, RTF, EPUB, legacy DOC/XLS/PPT, optional macro-enabled Office as *read-only* with macros rejected | Not implemented |
| RDR-10 | End-to-end acceptance | Real files from Office/LibreOffice/popular PDF writers, 50+ representative + hostile fixtures, Windows installed NSIS, Linux native, local GGUF and MMProj | Synthetic unit tests pass; real installed acceptance open |

**Minimum viable Reader closeout:** RDR-01, RDR-02, RDR-03, RDR-04,
RDR-05, RDR-08 and RDR-10. RDR-06 can close incrementally by feature;
RDR-07 controls the native-vision route and is required before claiming
vision automatically detected. RDR-09 is a *long-tail compatibility* gate,
not a reason to block a useful alpha.

## Verification

- GitHub workflow:
  `.github/workflows/lu-tools-11-background.yml`.
- Native tests: `cargo test --locked --manifest-path
  src-tauri/lu-core/Cargo.toml --lib native_reader -- --nocapture`;
  additionally `native_reader_docx`, `native_reader_openxml`,
  `native_reader_office`, `native_reader_odf`, `native_reader_pdf`,
  `native_reader_tool` and `native_reader_turn`.
- Desktop tests:
  `node --test lu-desktop/tests/reader-desktop.test.mjs`;
  Windows `cargo check` host and install/launch acceptance with authentic
  packaged inference engine **are separate tests**.
- A passing Linux unit suite or a Windows Rust compile does not establish
  that Qwen emits valid structured tool calls, that Vulkan is active, or that
  the NSIS installation works. Always test on the physical Windows machine.

## Reader interaction semantics

1. Ordinary attachment: operator chooses a file, native code extracts a
   bounded initial excerpt into the chat draft. No model filesystem grant.
2. Ask Reader: operator chooses exactly one file **for this turn**.
   The original path never enters the model tool schema; Lu pins an immutable
   in-memory copy, verifies the persisted current human message, opens only
   `lu.reader.read` with `ReadWorkspace`, journals every action and
   automatically revokes on completion/failure/cancellation.
3. `inspect_only` returns metadata without source text. A subsequent
   `offset_chars` request can pin `expected_source_sha256` to reject
   changed source revisions. `selector` accepts `sheet:Name`,
   `range:Sales!A1:D50` (XLSX), `slide:2` and `page:3` on formats
   supporting those selections.
4. Untrusted source material must not alter host permission policy or the
   identity of user/assistant/chat turns.

## Do not accidentally expand promises

Reader is one *extensible interface* for document formats. It does not
magically understand arbitrary unknown binary formats, re-render all charts,
recalculate workbook formulas, perform vision on a text-only GGUF, or execute
macros. Unsupported and unreadable sections must be reported explicitly.
