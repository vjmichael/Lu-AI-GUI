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
