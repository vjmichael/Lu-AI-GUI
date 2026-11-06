# Wave 2 — 7z/RAR/RPM, Windows codecs, and richer Office revisions

Status: **SOURCE-IMPLEMENTED, NOT YET RELEASE-ACCEPTED**. Exact-head Windows/Linux Rust CI plus a real installed NSIS desktop test are required.

## Native architecture

Lu uses libarchive's native C API, dynamically loaded from strictly controlled paths. No user/model-supplied DLL paths, PATH search, shell, mounted filesystem or external unpacker commands are permitted for 7z/RAR/Windows codecs.

- Windows: only a Lu-owned lu-native-codecs directory adjacent to the signed application's executable is used; the DLL SHA-256 inventory and manifest are checked before LoadLibraryExW with safe DLL dependency search flags. An unbundled or mismatched native library fails closed.
- Linux: a fixed system-owned libarchive.so.13 is preferred; the prior fixed /usr/bin decoder fallback is used ONLY if the shared library is unavailable. A corrupt compressed payload never causes a fallback decoder retry.
- macOS: not a Wave 2 acceptance platform.
- Windows staging: scripts/stage-lu-native-codecs.ps1 copies a pinned vcpkg libarchive DLL, its full native dependencies and available license texts to the desktop resource directory and a test executable's lu-native-codecs folder. See source workflow for pinned vcpkg commit.
- NSIS: the Tauri bundle now includes the lu-native-codecs resource directory. Placeholder README does NOT constitute a bundled codec runtime; production packaging must stage real verified binaries first.
- Native read/write raw XZ, BZip2 and Zstandard uses in-process libarchive, including actual-format filter ID checks. Pure gzip uses the existing flate2 backend.
- Read-only package extraction and archive list run through the same exact-hash, human-turn-scoped, journaled, new-folder-only archive tree dispatcher as ZIP/TAR.

## Supported safe subsets

| Family | Implementation | Refusals and constraints |
|---|---|---|
| 7z | List and extract validated unencrypted regular files and directories, including LZMA/LZMA2 where libarchive supports them | Encrypted/password-protected, external-volume, unsafe links/devices, malformed and oversized entries refused |
| RAR4 / RAR5 | List and extract validated unencrypted regular files and directories using libarchive's RAR/RAR5 decoders | Encrypted/password-protected, unsupported proprietary compression, symlinks/hardlinks, malformed or missing volumes refused |
| RPM | Parse v3/v4 lead, signature-header structure, indexed header tags and bounded payload; extract gzip/xz/bzip2/zstd/none compressed newc CPIO under a payload/ namespace | Package signatures are NOT verified; Lu is NOT a package installer. Unsupported CPIO links, scripts, unsafe entries, alternate compressors or missing payload refused |
| XZ / bzip2 / Zstd | Native Windows and Linux in-memory decompress/compress with bounded output, strict filter identity | No arbitrary untrusted executables or password prompts |
| OOXML Word/PPTX/XLSX | Up to 32 ordered exact old/new text-run or numeric/boolean/inline-string cell edits into one NEW DOCX/PPTX/XLSX version; whole revised package SHA-256 and unchanged package parts are human-reviewed | Existing original never overwritten; rich Excel shared strings, formulas, charts, macros, digital signatures, external links and unsupported structured objects not rewritten; do not claim general Office fidelity |

## Risk and authority

Each 7z/RAR member is decoded into bounded memory, then checked for path traversal, case-insensitive duplicates, symlinks, hardlinks, special nodes, unsafe names, member/output limits and unsupported encryption. The entire tree has a deterministic SHA-256 manifest before publication. The user selects source/destination with native pickers and approves the exact new folder. An extraction is never installed, executed, mounted, or given privileges.

RPM content is inspected, never installed. The RPM signature header is parsed only for structure; no public-key verification is claimed. Package scripts are inert file bytes.

Office batch editing has two separate approval boundaries: complete before/after preview in React and separate Tauri native dialog. The host checks the original package SHA-256, rendered final SHA-256 and all unaffected OOXML part hashes before granting a one-use native effect. All intermediate edits are in memory. The original file is never replaced.

All native writes require a persisted human turn, exact host policy and durable effect journal; uncertain outcomes must be reconciled manually. Hardening still required for external filesystem races and worker containment.

## Native test coverage

- native_codecs: real libarchive-produced 7zip test, native XZ/BZip2/Zstd roundtrip, truncated/mislabeled stream refusal and bounded input.
- native_7z_rar: real compressed RAR5 fixture, RAR4 fixture rejection of unsafe entries, full-tree extraction, no source-root publication.
- native_rpm: constructed valid RPM header and gzip newc CPIO payload, full-tree extraction, malformed headers and traversal rejection.
- native_office_revision: DOCX multi-run, XLSX multi-cell and inline text, package-part preservation, source-baseline mismatch, unsupported rich text and no-overwrite publication.
- native_office_batch_tool_authority: real user-granted, exact full-package batch tool dispatch, durable journal, denial and revocation.
- office_batch_desktop native queue tests: source identity, two-edit preview, no changes before native confirmation and rejection of mismatched expected content.
- Native Linux/Windows CI stages pinned codec DLLs and runs the same native suites. Installed Windows NSIS smoke and native runtime loading still require actual acceptance results.

## Outstanding release gaps

1. Verify exact-head Rust compilation and all Linux/Windows test matrices. If CI is queued, this is NOT a passing acceptance result.
2. Verify Windows NSIS install includes actual codec DLLs, manifests and legal notices; launch Lu and test real RAR4/RAR5/7z/RPM/XZ/Bzip2/Zstd operations, modal focus, denial, cancellation, reboot and restart recovery.
3. RAR/7z proprietary format limitations: encrypted/password-protected and multi-volume sets remain intentionally unsupported. Do not label them full universal archive support.
4. General Office rich-object editing is not provided. The implemented non-destructive versioned field/batch editing has explicit structural limits.
5. Long-running decompressor cancellation, memory-dictionary containment and crash-injection must be verified before native file handling is marked production accepted.

Upstream API and licensing references:
- libarchive official source: https://www.libarchive.org/
- libarchive C API: https://github.com/libarchive/libarchive/blob/master/libarchive/archive.h
- vcpkg libarchive build features: https://vcpkg.io/en/package/libarchive
- RPM file/header background: https://rpm.org/docs/latest/manual/format.html

**Do not advance the release or merge the draft PR solely from source commits.**

## Step 3 — Cooperative archive cancellation and fail-closed publication (2026-11-05)

- Host-created per-review cancellation token, scoped to the active chat, passes through the native tool registry to the bounded archive extraction worker. WebView requests cannot supply a token, path, or extra write grant.
- ZIP preflight and on-disk member publication check cancellation at bounded chunks. Cancellation **before** destination folder creation is a clean no-effect refusal; cancellation **after** folder creation is reported as WRITE_UNCERTAIN and the folder is retained for read-only reconciliation.
- No automatic replay or silent partial-folder deletion. The completion marker remains a last-write witness; native full-tree rehash remains required.
- Backend decoders for TAR/7z/RAR/RPM currently have synchronous bounded in-memory preflight. Cancellation while those native decoders are executing is **not** preemptive; it is honored on return, before publication. Hard worker-process termination requires follow-up native containment acceptance.

## Step 3 — cancellation extension and cleanup-failure reconciliation
- Explicit pre-publication archive cancellation is recorded as FAILED only for the trusted Lu archive extractor and only for its dedicated OPERATION_CANCELLED signal. All unknown native failures and every possible partial publication remain UNCERTAIN; neither state permits replay.
- 7z/RAR libarchive decode checks the host cancellation flag between native header and data chunks, with source reading in bounded cancellable chunks. No hostile decoder is assumed to support preemption inside one native C call.
- Desktop archive extraction concurrency is restricted to one active approved operation to bound expanded-memory pressure. Interrupted staging cleanup in atomic new-file and copy operations now promotes cleanup failures to WRITE_UNCERTAIN instead of reporting an ordinary safe error.

## Step 3 — post-restart archive filesystem observation
- The host now writes a bounded, canonical, verified complete archive manifest alongside the pre-effect intent before any extraction effect. Records exceed the 1 MiB persistence limit are refused before publication.
- File operations recovery still distinguishes the historical journal result from current filesystem state. For new archive records it independently reopens the previously approved parent **read-only** and verifies the completion marker, all expected folders and file SHA-256 digests, and absence of extras. It surfaces VERIFIED, MISSING, INCOMPLETE_OR_CHANGED, UNAVAILABLE or INVALID_TARGET without rewriting, retrying, or granting capabilities.
- Legacy archive intent schema 1 lacks a complete manifest and remains NOT_RECHECKED; receipt evidence alone is not represented as current filesystem verification. Current schema 2 includes the full snapshot.

## Step 3 — recovery write ordering and move revalidation
- Archive slot reservation now precedes intent publication, avoiding an approved-intent record for an operation rejected solely because another archive is active.
- Persisted recovery intents are file-synced, and their containing directories are synced on Unix before effect dispatch. On Windows, native installed crash durability must still be tested.
- Same-directory moves rehash both the published hardlink target and the existing source after linking and before unlinking. A changed source or wrong linked target stops deletion and returns WRITE_UNCERTAIN for manual reconciliation; this narrows but does not eliminate hostile source-path replacement races during the final unlink.

## Step 3 — native owned-worker execution and forced cancellation
- The desktop-approved archive tree is no longer decoded within the Lu host process. The existing background process supervisor starts Lu's own executable in a restricted internal `--lu-archive-extract-worker` mode, with verified executable digest, empty environment, no arbitrary shell, a Windows kill-on-close Job Object (suspended until assigned) or dedicated Unix process group, a 600-second host deadline and independent worker reaping.
- Before worker publication, both selected parent directories are independently opened and checked against the original *opened-handle directory identities*, preventing a substituted folder pathname from redirecting a previously approved extraction.
- A worker exit code alone never establishes success. Only a successful process lifecycle together with complete parent-side manifest and completion-marker rehash produces an APPLIED receipt. Forced termination, crash, timeout, failed status or unverifiable output all remain UNCERTAIN and are never replayed.
- The existing in-process cancellable archive implementation remains available to native core unit tests. The installed desktop uses the supervised path. Archive preview still requires native CI review of libarchive behavior during initial operator inspection; release acceptance remains separate from source completion.
