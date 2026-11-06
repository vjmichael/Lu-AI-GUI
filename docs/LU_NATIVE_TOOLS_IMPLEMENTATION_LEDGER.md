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

### Word document creation added within Wave 2
- `native_docx_create.rs` produces a real macro-free DOCX OOXML package with styled Title, Heading 1, Heading 2 and paragraph blocks, escaped text, line breaks and tabs, bounded sizes and no-overwrite create-new publication.
- `native_docx_tool.rs` exposes a typed Protobuf Create operation whose `ExactDocxCreatePolicy` re-renders and verifies the approved content SHA256 and destination before the persisted-human-turn / effect-journal admission.
- Native tests exercise package structure, Word paragraph styling, DOCX Reader round-trip, no-clobber, invalid XML characters and durable authorized dispatch/revocation.
- This remains a first structured Word creator. It does NOT claim existing DOCX editing, tables, images, sections, headers, footers, revision tracking, styles from templates, or installed desktop approval UX. All require dedicated tests and recovery boundaries.

### Host-approved filesystem copy and move
- `native_file_transfer.rs` now accepts an expected approved source SHA-256 for copy and move. The copy path checks the digest of the bytes actually written before publishing a new target. Moves recheck before same-directory hardlink/unlink but still need stronger OS identity locking against hostile cross-process mutations.
- `native_file_transfer_tool.rs` registers separate model-visible `lu_file_copy` and `lu_file_move` typed Protobuf operations, both `WriteWorkspace` effects, with exact source/target/digest `ExactFileTransferPolicy`; no model-originated path, parameter or hash grants permission.
- Added native tests for an authenticated user turn, exact grant, durable write receipts, replay denial after revocation, copy no-clobber, move source removal and changed-source refusal.
- Not yet completed: trusted desktop approval UI, cross-directory moves, safe nested folders, trash/restore model adapters, stronger Windows native file identity and installed Windows acceptance.

## Wave 2 final source-integration update — 2026-11-05

The following **development-source** areas were added after the initial ZIP/CSV/DOCX/file-transfer slices. This update is **NOT** evidence of native CI or installed-desktop acceptance:

| Area | Added source | Remaining acceptance |
| --- | --- | --- |
| Existing DOCX/PPTX/XLSX revisions | `native_office_revision.rs` creates a non-overwriting revised copy, preserving every unchanged OOXML part's uncompressed contents; targeted Word run, PowerPoint run or simple numeric/boolean cell supported. `native_office_revision_tool.rs` binds full source/new version SHA and selected edit. | No in-place Office edit; no arbitrary workbook layouts, signatures, macros, styles/templates/rich structures; full diff preview UI and installed model tool tests needed |
| Presentation Write | `native_pptx_create.rs`, `native_pptx_tool.rs` text-first multi-slide OOXML and exact host-approved bytes; real Reader round-trip and durable authorization tests | Template/theme/master/layout, charts, images, full fidelity, desktop UI and native acceptance |
| Full archive tree extraction | `native_archive_tree.rs`, `native_archive_tree_tool.rs` parse bounded ZIP/TAR-family trees, reject unsafe entries, create one new folder, publish completion marker only on success, and verify the entire tree during post-crash observation | Multi-file operations are NOT atomic; interrupted folders remain until reconciliation, not automatically replayed; Windows reparse/race hardening and installed GUI needed |
| Reversible filesystem trash/restore | `native_file_trash_tool.rs` typed independent trash/restore tools; restore requires host-persisted actual receipt; receipt SHA checked before restore; durability and revocation tests | Windows native identity/race tests, other volume support, folder-level trash/restore, integration with desktop directory picker |
| ZIP Deflate | `native_zip_extract.rs` supports Stored/Deflate, per-file SHA256 and CRC32, path/symlink/size checks | Other compression methods, encrypted ZIPs, full malicious archive corpus; Windows native acceptance |

### Explicitly not completed by this branch
- Real desktop native approval panels for structured Office writes/revisions and archive/folder operations. Existing text create/edit approval infrastructure is the starting point but not a replacement for format-specific review.
- Real 7z / RAR reading and extraction engines, UDF ISO, legacy Office binary files, broad PDF editing, full Word/PowerPoint rich-object authoring.
- Comprehensive transaction-wide crash injection, partial-directory resumability, cross-process/multi-volume rename race hardening.
- Exact-head Linux/Windows Rust compiler suite green, installed Windows desktop and actual GGUF/MMProj model execution.

**Rule:** Wave 2 source integration is broad, but **Wave 2 is OPEN** until all acceptance criteria and remaining declared tool capabilities are satisfied. Never mark this wave complete solely from source commits or queued GitHub Actions.

### Wave 2 Step 1: Desktop Integration — archive approval slice (2026-11-05)
- `lu-desktop/src-tauri/src/archive_desktop.rs` is a trusted host-only archive review queue. **Both** the archive input and extraction parent are chosen via native OS pickers; no model or WebView argument supplies an OS path. Host pins two `cap_std::fs::Dir` capabilities and the SHA-256 full-tree manifest.
- The host creates a **new, unpredictable child folder** derived from a bounded source basename and host-generated ID; never overwrites the chosen directory or an existing child.
- Tauri commands `lu_review_archive_tree`, `lu_confirm_archive_tree`, and `lu_deny_archive_tree` keep the preview inert. A final native confirmation is required after the React panel displays **every** archive member/path/size/hash and requests a review acknowledgment.
- On explicit consent, Lu records a fresh authenticated chat user turn, grants a single `lu.archive.tree.extract` WriteWorkspace effect through `ConversationAuthority` + `PolicyEngine`, and dispatches it via durable `EffectJournal` and an exact-manifest host policy. Full-tree content is rehashed after effect completion. Denials do not create files.
- If a write fails after directory creation, the host reports manual recovery/uncertain state, never auto-retries or deletes unexpected files. Preview queue is ephemeral; restart cannot replay it.
- React `Extract archive` action, exact member-list panel, checksum and destination display, acknowledgment, native approval, success receipt and deny paths are wired into `lu-desktop/web/main.jsx`.
- Unit source tests validate a real ZIP preview, chat isolation, deny/review consumption, and symlink refusal. `lu-desktop/scripts/check-ui.mjs` statically asserts end-to-end native host/UI command wiring. Dedicated Wave 2 CI includes full React UI build/check/tests and Windows Tauri cargo check + native archive queue tests.
- **Not yet accepted:** Exact-head Windows CI must actually pass; manually verify installed desktop flow with ZIP/TAR.GZ and interrupted writes. Desktop preview/approval for native Office revisions, structured DOCX/PPTX writers, spreadsheet edits, file copy/move/trash/restore are still separate gates. Model-generated proposals to these new tools remain Wave 3 normal conversation integration.
