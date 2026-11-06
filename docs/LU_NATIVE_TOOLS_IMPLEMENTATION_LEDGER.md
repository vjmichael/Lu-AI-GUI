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

### Wave 2 Step 1: Desktop Integration expanded — structured documents and file transfers

Four real host-side desktop workflows now exist as development source:

| Workflow | Host/Desktop implementation | Native approval boundary |
| --- | --- | --- |
| Extract ZIP/TAR-family archive | `archive_desktop.rs` + `Extract archive` UI shows complete member list, exact source/target folder, full-tree manifest SHA | Two native pickers, review ack, native modal, ConversationAuthority/PolicyEngine/EffectJournal; rehash actual extracted tree after publication |
| Create Word/PowerPoint document | `document_create_desktop.rs` + `+ Document` UI includes bounded text input, DOCX/PPTX format, complete text preview, exact output filename and package hash | Host-picked destination folder, host-generated new basename, native modal, exact DOCX/PPTX content policy, persistent human turn and durable receipt; actual output rehash |
| Revise existing DOCX/PPTX/XLSX | `office_revision_desktop.rs` + `Revise Office` UI accepts one typed field change. Native picker chooses input file and separate output folder. UI shows full before/after field change and hashes of all unchanged package parts | Host previews exact source/revision SHA; versioned new-file only, native modal and exact revision policy; post-write rehash of original and revised output |
| Copy/move regular file | `file_transfer_desktop.rs` + `Copy file`/`Move file` UI shows source hash, original-retention consequences and host-generated NEW name | Native source picker, native consent modal, exact source/target/digest policy, durable effect; postwrite target rehash and move-source absence check |

All four use ephemeral, chat-bound pending review queues, explicit deny, one-time consumption, and native (not WebView) final confirmation. React panels include complete content/member/part previews appropriate to their operation. The desktop code does not promote a model-generated plan to independent file-system authority.

**Verification added:** `lu-desktop/scripts/check-ui.mjs` checks the UI/Tauri command and acknowledgment wiring. Wave 2 CI now builds the real React bundle and runs UI checks; Windows desktop CI compiles the Tauri host and invokes archive/document/Office/file-transfer review unit suites.

**Step 1 remaining:** Native user interface for durable reversible trash/restore receipts; broader document operations (multi-sheet spreadsheet creation and rich presentations), accessible progress/cancellation/recovery dashboards for long operations, installed Windows UI acceptance. Full-feature model invocation from ordinary conversation remains Wave 3. The four desktop flows are source-implemented but not verified on a compiled installed build; this is NOT completion or acceptance.

### Wave 2 Step 1: Reversible trash UI and durable restart recovery index

**Reversible trash/restore** (`lu-desktop/src-tauri/src/file_trash_desktop.rs` + `src-tauri/lu-core/src/native_trash_custody.rs`):
- Only the native OS picker selects an actual regular file and pins the containing directory. A complete source digest, size and destination custody entry are previewed before the separate native approval. Nothing is moved on preview or denial.
- The host retains the file under a host-generated `lu-trash-{call_id}` filename **in the same selected source directory**. This avoids cross-volume hard-link failures and is not the operating system Recycle Bin; the user is told explicitly where the reversible bytes remain. Existing files are never overwritten.
- `TrashCustodyLedger` uses SQLite WAL + FULL synchronization and records the selected original directory, chat, effect ID, content hash and size **before** native dispatch; it cross-checks successful typed `ToolReply` receipts against `EffectJournal`. It does not accept a model-created receipt.
- On startup, incomplete `PENDING` or `RESTORING` records become `UNCERTAIN`; they cannot be auto-executed, or restored without independent reconciliation. Approved `TRASHED` receipts can be restored only if the actual stored bytes still match SHA-256 and the original target is absent.
- Native restore uses a separate confirmed human turn, `lu.files.restore` WriteWorkspace grant, fresh effect ID, no-clobber publication, and durable journaled receipt. The host verifies actual restored contents and original custody entry removal.
- React has a single `Trash file` preview/confirm/deny workflow and `File recovery` read-only per-chat custody history. Records list `TRASHED`, `RESTORED` or `UNCERTAIN` and distinguish verified bytes from missing/changed custody. User can request a new separate restore review only for validated restorable receipts.
- The earlier alternative `trash_desktop.rs`/app-data trash path was removed to avoid duplicate Tauri state/hooks and cross-volume behavior. Source guard tests reject two concurrent custody authorities.
- Native tests cover preview denial without writes, SQLite crash reconciliation, durable applied trash/restore receipt matching, and custody survival across reopening; exact-head Rust/Windows CI still required.

**Generic Wave 2 operation recovery** (`lu-desktop/src-tauri/src/wave2_recovery.rs`):
- Immediately after each distinct native consent and **before** the dispatched effect, Lu synchronously stores host-approved call ID, chat, tool ID, exact destination, name and approved witness SHA. The approval remains single-use.
- `lu_wave2_recovery(chat_id)` is **read-only** and chat-scoped. It joins these host-owned intents to the SQLite EffectJournal, validates the stored Protobuf ToolReply, exact tool/call/target and SHA/manifest, and reports INTENT_RECORDED, PREPARED, EXECUTING, FAILED, UNCERTAIN, RECEIPT_MISMATCH, or APPLIED_RECEIPT_MATCHED.
- React **File operations** displays durable evidence and manual-reconciliation guidance after restart. A matching journal receipt is proof of the historical effect, **not** a fresh filesystem integrity check. No replay/execute/repair action exists in this pane.
- Windows CI must compile the Tauri host, run real unit tests for trash custody and Wave 2 recovery, and validate the React command wiring. The exact committed source must pass CI before Step 1 is accepted.

**Acceptance still open:** Native Windows/Linux exact-head compile/test, installed Windows desktop functional smoke test, full progress/cancellation for long archive/document operations, race/cross-volume and rollback hardening. Step 1 remains development-source integrated, NOT production accepted. Step 2 format completeness, Step 3 crash hardening and Step 4 final acceptance are separate ordered work.

**Reconciliation note (2026-11-05):** Wave 2 has one native reversible file-custody path and one read-only cross-tool recovery index. An interim parallel trash implementation was removed after conflict detection. The current source still requires native compiler, UI and installed desktop acceptance before Step 1 can close.

### Wave 2 Step 1 source-level desktop consolidation checkpoint
- A single reversible-trash custody authority remains: `native_trash_custody.rs` (SQLite) + `file_trash_desktop.rs` (host-pinned native review, journal-verified trash/restore). Obsolete competing app-data trash implementation and its duplicate React/Tauri commands were deleted.
- `File recovery` is conversation-scoped, read-only and restart-visible. It can present `TRASHED` with a verified restorable receipt, `RESTORED`, or `NEEDS_RECONCILIATION`. Uncertain records are never automatically replayed or elevated into a restore grant.
- Generic `File operations` history remains a separate read-only `wave2_recovery.rs` index, verifying historical ToolReply evidence for archive extraction, DOCX/PPTX creation, Office revisions and copy/move. A past applied receipt does not assert that a file has remained untouched by external processes.
- The React interface now exposes accessible in-progress status for native file workflows, without claiming unsupported byte-level percentages or offering unsafe automatic retry.
- Native file absence checks distinguish `NotFound` from permission/IO errors when deciding whether a trash/restore target is empty. Metadata inspection errors fail closed.
- Dedicated UI assertions guard against reintroducing duplicate trash controls and require accurate in-progress/recovery warnings. Linux/Windows Rust CI and installed desktop smoke acceptance remain mandatory, **not completed**.
- Step 1 still needs user-visible installed app functional verification (including native modal focus, error/denial, restart and partial-effect review). Long archive operations still lack interruptible byte-level progress; safe cancellation and crash-injection remain Step 3 hardening gates. Do not mark Wave 2 complete on source-only evidence.

## Wave 2 Step 2 — Format completeness: CPIO, Unix ar and Debian packages

New implementation sources:
- `native_cpio.rs` handles bounded, fail-closed newc (070701) and additive-checksum CRC (070702), with gzip/xz/bzip2/zstd compressed wrappers. Reuses trusted decompression backend, no symlink/device/hardlink or traversal support.
- `native_ar_deb.rs` handles bounded Unix ar/.a with short, GNU long-name and BSD extended names, and Debian .deb v2.x. Debian control and data nested TARs are separately exposed as inert virtual `control/` and `data/` trees; no installation, maintainer script execution or host system path publication.
- `native_linux_archive.rs` now exposes a bounded in-memory TAR parser for nested package members, normalizing a single conventional leading dot-slash path and refusing parent traversal.
- `native_archive_tree.rs` routes these formats through exact manifest hashing and brand-new-directory full-tree extraction; `native_archive_inspect.rs` exposes them via the existing read-only, per-turn native archive-list tool.
- `archive_desktop.rs` uses friendlier suffix handling for cpio/deb/ar; the existing native picker and review/approval model applies unchanged.
- `tests/native_linux_packages.rs` includes actual CPIO, gzip, checksum/hardlink/traversal, ar GNU long-name, minimal DEB with compressed data payload, new-folder extraction/no-clobber, malformed inputs and authenticated model tool listing acceptance.
- `docs/TOOLS_WAVE2_LINUX_PACKAGE_FORMATS.md` records exact limits/compatibility and remaining capabilities. Wave 2 native Linux/Windows CI now requires these tests and source exports.

**Still open in Step 2:** fully working 7z/RAR compressed engines, RPM native inspect/extract, LZ4 and legacy package/archive variants, Windows-native xz/bzip2/zstd decoders, UDF ISO, broad richer Office formats and full-fidelity edits. The existing Linux external codec pathway is not equivalent to Windows in-process format parity. Exact-head compiler/behavioral CI has not been accepted. Do NOT mark this format-completeness phase finished yet.

## Wave 2 Step 2 — Advanced formats and richer Office edits (development source)

Implementation sources committed in this pass:
- Native Windows/Linux libarchive ABI bridge: native_libarchive.rs. Dynamic trusted-library loading, bounded in-memory 7z/RAR decoder, bounded XZ/Bzip2/Zstd native encode/decode; Windows verifies its app-owned DLL manifest, then uses LoadLibraryExW restricted dependency flags. Linux uses fixed OS-owned .so paths with fixed-path decoder fallback only if backend missing.
- native_seven_rar.rs: accepts 7z and RAR/RAR5 bounded unencrypted, regular-file/directory entries. Integrates with native_archive_inspect.rs, native_archive_tree.rs and native archive desktop selection/approval. Rejects links, devices, path traversal, duplicate aliases, unsupported encryption and unsafe/oversized output.
- native_rpm.rs: strict RPM v3/v4 lead/signature/main indexed header parsing; CPIO format, gzip/xz/bzip2/zstd or uncompressed payload; expanded output isolated under payload/ namespace. DOES NOT verify digital package signatures, install packages or execute scripts.
- scripts/stage-lu-native-codecs.ps1, Tauri resource mapping and pinned vcpkg Windows CI: stage native DLLs, all codec dependency DLLs, SHA-256 manifest and available license notices into Lu's NSIS resource directory. Placeholder README without actual staged DLLs is NOT runtime acceptance.
- Richer OOXML revision rendering: native_office_revision.rs now supports ordered 1–32 exact changes as one new package, with entire original/revised SHA and unchanged-part checks; simple Excel inlineStr text values can be changed without touching shared strings or formulas. Unsupported rich XML, signed/macro packages and cross-file links are rejected.
- Typed model/Protobuf API: native_office_batch_tool.rs and OfficeBatchPatch/OfficeBatchRevisionRequest; ExactBatchPolicy re-renders the entire original/revised package before host write authority.
- Native UI: office_batch_desktop.rs, Tauri native pick/preview/confirm/deny commands, complete React batch editor and changed/unchanged-part review, new-version output only and persisted Wave 2 operation-intent recovery; adds cell_text option to single-revision and batch UI.
- Native fixture tests: real 7z and upstream BSD RAR5 compressed/RAR4 rejection test data, native codec filter-id and encode/decode roundtrip tests, constructed RPM/CPIO gzip payload with real extraction, new Office batch per-turn durable authorization, DOCX/XLSX multi-field and safe XLSX inline text tests, native desktop denial/review tests. Wave 2 CI now requires the suites on Linux/Windows, pinned Windows codec staging and desktop React/Tauri builds.
- Exact implementation, limits and verification obligations: docs/TOOLS_WAVE2_ADVANCED_FORMATS_AND_OFFICE.md.

**Not accepted/remaining limitations:** encrypted/password-protected or multi-volume RAR/7z; arbitrary unsupported RPM payload modes; verification of RPM signatures; full rich-object, chart, macro, external-link, formula or template preservation/editing in Office files; native codec worker resource isolation and cancellation; exact-head Windows and Linux passing CI; real installed NSIS artifact with codec DLLs and human confirmation/cancellation/restart validation. Source only is not evidence of completed release acceptance.

No Step 3 recovery hardening or Step 4 native final acceptance is claimed.


## WAVE2-RECOVERY-03 — Native archive cancellation hardening
The archive extraction tool now accepts only a host-owned, chat-scoped, per-review cancellation token. ZIP decode and member publication have cooperative checkpoints, and the desktop exposes an explicit cancel-running-extraction action. Cancellation before directory creation fails without a write; cancellation after creation returns WRITE_UNCERTAIN and preserves partial output for journal and full-manifest reconciliation. Non-ZIP synchronous decoders still require supervised-process termination for immediate cancellation. Exact-head Windows/Linux CI and installed desktop acceptance remain open.


WAVE2-RECOVERY-03B: DurableDispatcher now recognizes the built-in archive extractor's proven pre-publication OPERATION_CANCELLED only, journals FAILED, and still blocks replay. Other native execution failures remain UNCERTAIN. 7z/RAR source reads and libarchive member loops honor host cancellation checkpoints. Desktop archive executions are serialized to bound aggregate 256 MiB per decode. Failed staging cleanup for new-file and copy operations is classified as WRITE_UNCERTAIN. Native Windows/Linux CI and installed acceptance remain open.


WAVE2-RECOVERY-03C: Before archive execution the desktop now persists the entire bounded, canonical tree-manifest snapshot as part of the approved intent (schema 2). Restart recovery validates snapshots and independently rehashes the actual full extracted tree. It displays current VERIFIED/MISSING/INCOMPLETE_OR_CHANGED/UNAVAILABLE status alongside the immutable historical journal receipt, and never auto-retries. Existing schema-1 records retain receipt-only NOT_RECHECKED semantics. Native integration and installed Windows verification pending.

WAVE2-RECOVERY-03D: Windows source and archive-tree reads/recovery checks now reject reparse attributes on already-opened capability handles. This replaces the redundant symlink-only Windows checks in the ZIP/child-folder path and adds opened-file checks for transfer sources, completion markers and recovered files. Native Windows acceptance remains pending.

WAVE2-RECOVERY-03E: Host now reserves the single active archive execution slot before persisting an intent (prevents misleading no-effect intents on busy rejection). Linux/Unix recovery intent creation syncs the directory entry before dispatch. Move operations rehash both linked target and source after publication and before unlink to catch detectable source-swap races; mismatch remains WRITE_UNCERTAIN without deletion. Final source unlink still needs handle-identity/OS-native atomic defense against hostile races; installed acceptance open.

WAVE2-RECOVERY-03F (SOURCE IMPLEMENTED): The approved desktop extraction now dispatches its exact-manifest native effect through a host-owned, independently terminable Lu child process. It reuses BackgroundProcessSupervisor, Windows suspended-process Job assignment and kill-on-close, Linux owned process groups, TaskStore, and a 600-second host deadline. The child reopens independently identified, native-picked source/destination directory handles and refuses path substitution; the parent requires actual full-tree rehash before APPLIED. Cancellation kills and reaps the owned worker; all mid-execution cancellation/crash/timeout outcomes remain UNCERTAIN with no automatic retry. In-process extraction continues only in explicitly non-desktop core tests. Native Windows/Linux and actual GUI installation acceptance still required.

WAVE2-RECOVERY-03G: Same-directory move retains its original open source handle and validates identical opened-file identity on source and newly linked target before final unlink. Even same-content filename substitution at the deterministic post-link fault point is refused as WRITE_UNCERTAIN with replacement preserved. Added real owned worker nonzero-exit and replay-block regression on Unix; desktop Windows CI explicitly runs restarted full-tree recovery tests. Hostile same-privilege atomic source-unlink races remain an OS-level limitation (no unsafe claim of full CAS).
WAVE2-RECOVERY-03H: The *initial archive preview* is now also performed in Lu's existing native child-process supervisor, with a fixed deadline, host-only scoped cancellation, matched parent directory handle identity, strictly bounded no-clobber JSON manifest output, validation and cleanup. The installed desktop's authorization policy no longer decodes an archive a second time in the Tauri host; the child independently decodes and checks exact approved SHA before publication. Core tests retain direct read-only preview access. Both installed preview and extraction now admit host-owned worker cancellation and kill/reap; native acceptance remains pending.

WAVE2-RECOVERY-03I: Linux archive preview/extraction child modes now set PR_SET_PDEATHSIG(SIGKILL), re-check their actual parent PID immediately afterward, and limit virtual memory to 1 GiB before decoding. Windows internally identified archive workers get a 1 GiB Job memory quota in addition to kill-on-job-close; GGUF engine and other background workloads remain unaffected. Linux parent-death admission runs only in actual child mode, never test harnesses. Native CI and installed crash evidence remain required.

WAVE2-RECOVERY-03J: Source-removing same-folder moves no longer use separate hard-link/verify/unlink phases. Windows uses the opened source FILE OBJECT with native no-replace FileRenameInfo and pinned destination-directory HANDLE, then verifies target's original file ID and approved SHA. Linux uses renameat2(RENAME_NOREPLACE) under a pinned directory FD, then verifies target's original inode/device and SHA. Any post-rename mismatch remains UNCERTAIN and never prompts an automatic replay. No extra deletion call can accidentally unlink an externally swapped filename. Unsupported OS/filesystem primitives fail closed; exact-head native tests still required.
