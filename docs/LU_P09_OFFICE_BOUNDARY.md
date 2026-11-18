# P09 OFFICE-BOUNDARY-01 — Lu Core Office and spreadsheet maintenance

Date: 2026-11-07
Branch: `lu/p09-office-boundary-20261107`, stacked on P08 `lu/p08-archive-split-20261107`.

## Owner-approved scope
Maintain a competent, bounded document/spreadsheet reader and non-destructive ordinary editing capability. This is NOT an Office suite, DTP engine, chart designer, formula recalculation engine, or high-fidelity document renderer. Advanced layout and intricate formatting belong in specialist applications. P04/P05 are deferred; P07 comprehensive SBL/citation engine is not required.

## Capability and fidelity table

| Format | Lu Core read | Lu Core author/create | Existing-file revision | Explicit limitations |
| --- | --- | --- | --- | --- |
| DOCX | Bounded plain text (document structure/anchors where available) | New DOCX title/paragraph blocks | Exact individual w:t text-run replacements, single or ordered batch | No cross-run heuristic, full layout editing, tracked-change fidelity, macro/signature editing |
| PPTX | Bounded slide text, slide selection, notes | New simple presentation from structured slides | Exact existing slide a:t text-run replacements | No editable 3D, animation, media timeline, masters, chart or geometry editing |
| XLSX | Bounded sheet/cell/range extraction with typed text and cell anchors | New one-sheet workbook with typed cells, limited trusted formulas | Existing numeric, boolean, inline-string cell values only | No formula evaluation/recalculation; shared-string/rich-string mutation, pivot tables, charts, macros, external links or signature rewrites |
| CSV | Bounded data-only export | New CSV, formula-looking text escaped as literals | Existing CSV bulk edit not part of P09 | No type preservation or formulas on CSV import/export |

Reading a spreadsheet is **not** calculating formulas. Text extraction is **not** visual rendering or an accurate representation of complex page/slide geometry. New OOXML package ZIP bytes, compression metadata and timestamps may differ even when unaffected part **uncompressed content** hashes are preserved.

## Existing implementation reused
- `native_reader_docx.rs`, `native_reader_office.rs`, `native_reader_openxml.rs`: bounded read and selectors.
- `native_docx_create.rs`, `native_pptx_create.rs`, `native_spreadsheet_create.rs`: bounded new-file creation.
- `native_office_revision.rs`, `native_office_replace_all.rs`: exact SHA-pinned previews and ordered 1–32 native edits, invalid/unsupported package refusal.
- `office_revision_desktop.rs`, `office_batch_desktop.rs`: source/destination chosen by OS dialog, exact human preview and separate native approval; new output, no overwriting the original.
- `native_atomic_write.rs`, effect journal and per-turn authority remain the publication boundary.

## P09 additions
1. Before individual and batch revision review, disclose exactly what can change and that unaffected package part bytes do not guarantee full visual/layout fidelity. Explicitly warn that Office application rendering should be inspected.
2. Add `lu-desktop/tests/p09-office-boundary.test.mjs` to bind the current read/write/native approval and rich-object refusal contracts to these claims.
3. Retain existing native `native_office_revision` and `native_office_batch_tool_authority` tests, including baseline changes, duplicate targets, original preservation, no-clobber and unsupported rich structures. No new Office formats or model privileges are added.

## Gates and scope boundary
- **Source changes:** committed on P09 branch; source review/CI still necessary.
- **Native exact-head Windows and Linux:** must run existing Office revision and read tests (new P09 CI workflow can independently verify them). Do not call queued or absent jobs PASS.
- **Installed desktop:** choose a real DOCX, PPTX and XLSX through native pickers, preview approved edits, deny one edit, publish new output, reopen both originals and outputs in a dedicated Office application to compare content/layout, restart and verify no unintended replay, and uninstall. This is NOT proven by source inspection or GitHub cargo tests alone.
- **Deferred:** chart and graph design, formulas/pivot recalculation, rich text/style editing, macros, digital signature preservation, embedded object authoring, full Office suite, P04/P05.
