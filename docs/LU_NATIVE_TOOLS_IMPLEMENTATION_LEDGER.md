# Lu Core native tools — implementation and acceptance ledger
Updated: 2026-11-05
Source integration branch: `lu/tools-all-source-consolidation-20261105` (PR #40)
Status: DEVELOPMENT ONLY. No claim of stable v1.0 or installed Windows acceptance.

## Non-negotiable acceptance states
- **Source** = a named implementation exists and has source review coverage.
- **Native verified** = exact-head platform compiler and behavioral tests passed.
- **Desktop integrated** = ordinary or explicit chat flow resolves a trusted user turn, schema, authority, host policy, engine, and cancellation.
- **Recovery verified** = interruption/uncertain effects are observed and never replayed without evidence.
- **Release accepted** = actual installed Windows app + GGUF/backend + real file tests passed.

Do not equate passing file/string assertions, having a Protobuf descriptor or receiving a GitHub queued CI job with any of the last three states.

| Domain | Source state in this stacked tree | Missing source or integration | Highest-value next acceptance |
| --- | --- | --- | --- |
| Native file read/list/stat/search | Existing Lu Core modules from #34 | Ordinary chat routing; broader target selection; permission UI | Host-selected grant, model call, multi-chat revoke |
| Create-new text Write | Existing journaled atomic creation | Ordinary model tool grant UI, exact approval receipts | Installed Windows real file + restart |
| Existing-text Edit/rollback | #5 source and #34 descendants | Host full-diff review UI, NTFS metadata and cross-process races | Windows crash injection across all publish boundaries |
| Copy/move/rename/trash/restore | #36 merged into this integration source | Typed Protobuf adapter, true persisted grant, effect journal, paths beyond flat same-volume source | No-clobber + ambiguous unlink reconciliation |
| Archive listing | Existing #15 / #34 | Genuine installed model call, broaden bounded formats only after safety review | ZIP adversarial tests across OS |
| Archive creation | #37 merged into this integration source | Model-facing exact-target grant, journal/recovery; folder and extraction support | Real ZIP compile and crash test |
| Spreadsheet creation | #39 merged into this integration source | Desktop approval UI, existing XLSX edits, style-preserving round-trip, sheets and formula recalculation | Exact-native Protobuf approval/write/receipt tests |
| Universal Reader | #27 / #34 | Rich PDF, scanned OCR, per-range XLSX, Office producer fuzzing, streams | Real GGUF Reader and Windows OCR |
| JSON/schema | #12 / #16 | Reconcile drafts, consistent strict MBRL boundary in ordinary chat | Exact-head source + platform matrix |
| Git inspection | #14 / #34 | Host-selected repo grant and Code-task integration; advanced coding reserved for IDE | Hostile config/worker and installed acceptance |
| Environment discovery | #17 draft | Real availability/version probes, approved provisioning | Host platform probes with no implicit authority |
| Vision/OCR | #18 draft | Real MMProj route, reliable Windows OCR, packaged Tesseract | Installed MMProj recognition and fallback |
| Tool/stream loop guards | #19 draft | Apply progress guard to all backends, durable checkpoints | Real looping GGUF and subprocess termination |
| Controlled execution | #34 foundations | Complete context-bound authorization, process cleanup, worker stdout contracts | Windows actual native worker cancel/restart |
| Background task control | #34 foundations | Durable result handoff, automatic model continuation with authority fence | Restart and multi-chat lifecycle |
| Browser/research | #26 draft | Verified browser binary packaging, worker crash recovery, larger research workflow | Real read-only browser + human challenge |
| Image Workspace | #35 draft | UI, crop/resize/annotation, masks, editing backend, immutable revisions | Installed native deterministic edit and crash recovery |
| Model/backend lifecycle | #20 draft | vLLM desktop integration, real llama cancellation and GPU release | Actual installed llama/ROCm/Vulkan stop |

## Exact branch graph and ownership
- #34 is the existing unified tools foundation. Its CI concurrency group was repaired to avoid cross-branch cancellations. It retains the original source.
- #38 (read-only multi-tool workspace and GUI `Ask Workspace`) stacks on #34.
- #39 (typed spreadsheet writer + exact Protobuf approval/receipt tests) stacks on #38.
- #40 combines the native feature files of #36 (copy/move/trash) and #37 (ZIP creation) onto #39; its merge commit preserves both parents. No release merge.
- #35 Image Workspace remains a standalone concept/validation slice; it is **not a working editor**.
- #26 browser, #18 OCR, #19 loop guard, #20 backend are distinct drafts and must be reconciled deliberately.

## Immediate next actions
1. Compile the exact SHA of #40 on Linux and Windows; repair compiler and test failures, including Protobuf and ZIP.
2. Add reviewed desktop write proposal for the spreadsheet creator: user sees filename, sheet/cells, hash and target; grant cannot be synthesized from model output.
3. Implement format-aware **existing** XLSX/CSV edits using safe revision baselines, prove unrelated sheets/styles preserved; do not silently fall back to destructive whole-workbook rewrites.
4. Complete safe ZIP extraction and filesystem transfer durable effect adapters with never-replay-on-uncertain semantics.
5. Finish Reader PDF/OCR and actual MMProj routing; unify default chat capability discovery and tools under consent controls.
6. Cross-platform crash/kill matrix and actual installed Windows/real GGUF evidence.
7. Only after all relevant gates pass, promote the integrated source into the release branch.

Known limitation: no working Rust toolchain is available in the current container, and GitHub Actions runs observed for this head were queued, so code publication is **not** compilation or test success.

## Wave 1 consolidation repair — 2026-11-05
- Consolidated feature ancestry: #34 → #38 → #39 → #40 → #41 → #42. Source from #36 and #37 is integrated through #40's ancestry; separate drafts are not claimed merged into alpha.
- `lu/tools-linux-archives-20261105` is the Wave 1 candidate, not the stable release branch.
- Repaired missing `pub mod native_linux_archive_tool;` export. The Linux archive Protobuf request/receipt definitions and paired create/extract native tool definitions now reside in the same source tree.
- Added `.github/workflows/lu-wave1-native-tools.yml` to require explicit source module exports and test all consolidated native areas on Linux and Windows; Linux CI installs real xz, bzip2, and zstd command backends.
- Source-tree inventory is re-pinned after each committed code/workflow change. Exact-head native build and installed-app acceptance remain separate gates.
- **Not Wave 1 closed:** require exact-head GitHub source lock, Linux and Windows compilation and tests; fix any reported failures; then reconcile draft branch topology. Never treat queued CI as pass.

## Wave 2 implementation — 2026-11-05 (in progress, separate from Wave 1 CI)
Source branch: `lu/wave2-file-archive-operations-20261105`. Based on Wave 1 exact source `7a8d03f3e579559b6f42ae62e77c8d4eef06d4a6`.

- **ZIP extract:** `native_zip_extract.rs` adds bounded selected-member extraction for Stored/Deflate ZIPs. Validates whole ZIP metadata, links, path casing/duplicates, decompression ratio, expanded size; reads streamed bytes, verifies CRC32 and SHA-256, then hard-link publishes a synced, no-overwrite stage. Different expected hash and pre-existing targets fail closed. No full-directory multi-file extraction claims.
- **ZIP exact grant:** `native_zip_extract_tool.rs` adds typed Protobuf model codec and `ExactZipExtractPolicy` requiring human-pinned source, member, output and hash; durable effect journal tests cover denial, verified receipt, replay fence and revocation. Catalog registration never authorizes writes.
- **CSV structured edit:** `native_csv_edit.rs` parses UTF-8 quoted CSV with bounded cell/row counts; edits a nominated cell with literal escaping for new formula-looking input, emits a full-document replacement for `WRITE-01E` full-diff review and hash-pinned existing-file journaled commit. The planner does not mutate files or bypass user consent.
- **Native CI:** `.github/workflows/lu-wave2-files.yml` requires exact source lock and Linux/Windows ZIP/CSV native tests.

### Still outstanding within Wave 2
1. Exact-head Cargo CI compilation and repair of all Native/Windows failures. Do not call feature accepted on queued runners.
2. Desktop human approval UX and normal chat registration of write tools, including exact file/member/content review.
3. Existing XLSX round-trip edit preserving workbook parts, formulas/styles, relationships, images, charts, macro safety, and package signatures.
4. DOCX/PPTX native structured create and edit engines, and PDF non-destructive editing.
5. ZIP full-tree extraction with transaction-wide rollback/recovery; 7z/RAR and broader ISO/UDF variants.
6. Durable filesystem copy/move/trash adapters, directory transactions, and Windows ACL/race recovery acceptance.

Do not promote Wave 2 to complete from source-level implementation or queued GitHub Actions.
