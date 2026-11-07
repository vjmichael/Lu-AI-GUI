# Lu P08 — ARCHIVE-SPLIT-01: reviewed multipart archive-byte transport

**Date:** November 7, 2026  
**Branch:** `lu/p08-archive-split-20261107` stacked on draft P07  
**Status:** SOURCE IMPLEMENTED; real exact-head two-OS native CI and installed application remain OPEN.

## Scope — approved owner direction

Lu Core already has archive inspect/extract/create, including specialized platform helpers. P08 adds safe file-byte split, explicit resumed staging and exact-byte reassembly for an *existing archive*. This is deliberately a **Lu byte-parts container** with a JSON manifest; it is **not** native ZIP spanning, RAR multi-volume compression, or solid/multivolume 7z. No password/encryption or RAR archive creation is promised. Complex ISO/UDF repair and specialist archive repair/manipulation are delegated to dedicated tools, with optional read-only ISO extension deferred.

The prior P07 advanced scholarly tool expansion is now OUT OF CORE SCOPE under owner instruction: comprehensive SBL source dictionaries, independently verified publication detail services and sophisticated specialist writing belong to third-party specialist software or a dedicated DLC. Core retains evidence-bound ordinary citations and exports. Native P07 CI and installed acceptance remain separate release checks, not a reason to stop P08 source development.

## Implemented core engine

- `src-tauri/lu-core/src/native_archive_split.rs`: cap-std directory-rooted, flat-name-only format-agnostic original archive byte splitting, limiting a source to 512 MiB, chunk size to 64 KiB–32 MiB internally, and no more than 128 pieces. The desktop offers approved sizes 4/8/16/32 MiB.
- A canonical `lu-parts.json` manifest has version, format identity, original file name, exact byte length, whole-file SHA-256, chosen chunk size, ordered, gap-free `part-NNNN.lup` names and each part's SHA-256/byte span. Noncanonical, overbudget, traversal and extra/unknown JSON fields are rejected.
- Split plan is computed before writes; the original source is rehashed against the plan at admission and after staged copy. Host-generated stage folder is `.lu-p08-split-<token>`; an interrupted staged file may be used only after a user explicitly selects that folder, reapproves the manifest, and the original and every preexisting part rehash exactly. Corrupt/truncated staging fails closed. No blind resume.
- Output uses atomic create-new directory admission. Each verified staged chunk is hard-linked to the output as a create-new file; the canonical manifest is linked **last** as the completion marker. An interrupted output directory is explicitly **UNCERTAIN**, never auto-published/replayed or claimed complete. Existing outputs are never overwritten.
- Reassembly begins with a native manifest and every part hash/combined original hash inspection. It requires a separately OS-picked output parent (usually different from the archive source folder), creates a unique temporary file, rehashes every input during the second copy, fsyncs, and atomically hard-links the fully verified original bytes under the original name with no-overwrite semantics. Originals and split parts remain untouched. A crash leaves a stage for manual reconciliation.

## Desktop and authorization

`lu-desktop/src-tauri/src/archive_split_desktop.rs` exposes three **human UI-only** Tauri commands: `lu_p08_split_archive`, `lu_p08_resume_split`, and `lu_p08_reassemble_archive`. Their read and destination identities come ONLY from trusted native OS file/folder pickers, not model-generated or WebView path strings. Native confirmation displays exact byte count, whole-source SHA-256, manifest SHA-256, number/size of parts, and non-compatibility warning before a write. Work is recorded with a chat-owned TaskStore start/success/failure receipt. The UI offers explicit split, resume, and reassemble actions and displays digest receipts. There is **no model-visible auto-run archive mutation tool** introduced by P08.

## CI and rejection cases

`src-tauri/lu-core/tests/native_archive_split.rs`: real original-byte roundtrip, four-piece output, independently picked reassembly parent, no-clobber, rechecked staged resume, corruption/tampering/malformed manifest, stale source, traversal/case-sensitive names, and Unix symlink rejection. `lu-desktop/tests/p08-archive-split.test.mjs`: source-to-native-GUI authorization, canonical manifest and honest output naming contracts.

`.github/workflows/lu-p08-archive-split.yml` requests Rust regression on Windows/Linux, frozen source identity and Node source-contract tests, plus Windows React build and native Tauri Cargo check. **Source implementation alone is not real CI PASS**; installed-app/NSIS and physical large-archive performance are separately OPEN.

## Remaining and deferrals

- Verify exact-head two-OS Rust/Node tests and native Windows Tauri compile; repair every actual failure.
- Installed Windows acceptance: OS file/folder picker lifecycle, no-overwrite filesystem semantics, cancellation/crash reconciliation and correct user-visible receipts. Linux installed desktop acceptance remains separate.
- P08 optional follow-up: user-facing reconciliation inspector for uncertain partially published folders, explicit safe cleanup of old completed staging after successful publication, output filesystem occupancy/quota and cancellation hooks.
- Optional complex ISO/UDF inspection and native split-volume helper format support deferred to specialist software unless later ordered.
- Core intentionally does not implement standalone archive repair/editor or non-licensed password/RAR archive writers.

**Next gate:** complete exact-head CI and installed desktop acceptance before marking ARCHIVE-SPLIT-01 final.

## Native CI repair / acceptance follow-up — November 7, 2026

Inspected PR #81 exact source head `71873eca4d60b9f585822e4e31c5705f1275e5c8` and associated GitHub Actions job logs. Its Windows `native_archive_split` suite passed **4/4** real round-trip, corruption, staged-resume and source-safety tests. The source-integrity workflow passed. This is native Rust test evidence, **not installed desktop acceptance**.

Separate regression failures were confirmed: Windows `native_archive_tree_authority` fixture deletion encountered OS error 32 while capability holders were still live; Linux `native_workspace_read_turn` failed registration before dispatch in two tests; the frontend had a malformed P07 JavaScript test regex; several other frontend assertions and browser candidate artifact setup are independently failing. Changes in this branch: `8a8de1e` explicitly releases archive policy/registry/authority handles before Windows fixture cleanup; `3f9baeb` adds the native registration error detail to CI diagnostics without weakening authorization; `914ffe1` repairs the invalid P07 test regex (not an expansion of the citation engine).

**Current gate:** fresh CI on the amended head must finish and the Linux registration root cause must be resolved from the more specific diagnostic. Do not count queued jobs as pass. Installed Windows NSIS launch/picker/confirmation/large-file and interruption/restart tests, separate Linux desktop tests, and cleanup of uncertain staged outputs remain **OPEN, NOT TESTED**. A passing native unit suite cannot substitute for installation and GUI acceptance. Citation/SBL expansion remains explicitly out of scope.

### Read-only publication recovery (subsequent repair)
- Corrected the discovery registration namespace (native tool `lu.file_discovery.discover` under `lu.file_discovery`), preserving registry prefix enforcement.
- Added `native_archive_split::reconcile_publication`, which refuses unexpected output members, checks existing part hashes and completion-marker integrity, rechecks the entire original-byte digest for a complete publication, and reports partial completion without writes.
- Added native regression for absent, incomplete, forged manifest-last and unexpected output cases; all uncertain effects remain untouched. **CI results for these new tests are pending.**
- A user-facing installed-application recovery review, explicit cleanup approval, actual OS picker/NSIS launch, and system restart testing remain **open**. This API is not itself a working desktop recovery button.
