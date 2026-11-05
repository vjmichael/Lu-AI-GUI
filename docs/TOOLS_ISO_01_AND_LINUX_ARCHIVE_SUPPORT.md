# TOOLS-ISO-01 — ISO 9660 inspection and selected-file extraction
Date: 2026-11-05
Status: DRAFT SOURCE, not installed/accepted.

## Supported in this development branch
- The existing native `lu_archive_list` read-only tool accepts host-scoped `.zip` and `.iso` basenames. For ISO, it parses the ISO 9660 primary volume descriptor (`CD001`), validates dual-endian extent/size metadata, and lists bounded regular files/directories without mounting.
- `native_iso9660::extract_iso_file_new_checked` extracts **one selected regular file** into an **existing, host-selected destination directory**, as a new non-overwriting file. It verifies exact member SHA-256 against the approved preview before publication. First version caps each extracted file at 1 MiB pending a streamed transactional publisher.
- `native_iso_extract_tool` adds a dedicated Protobuf request/receipt and strict target/member/digest model codec. `ExactIsoExtractPolicy` must be used with a persisted human-turn `ConversationAuthority`, host `PolicyEngine`, and durable `EffectJournal`. No automatic write grant comes from model output or catalog registration.
- Native fixtures cover valid ISO listing/extraction, missing members, traversal and malformed PVD refusal, source symlink refusal on Unix, exact approved scope, durable receipt, replay fence and task revocation.

## Explicitly unsupported
- UDF-only discs; Joliet Unicode names; Rock Ridge extensions/Unix metadata; El Torito boot catalog interpretation; ISO multi-extent and interleaved/associated files; special files, symbolic links and device nodes.
- Bulk/directory extraction, creating ISO files, editing an existing ISO, mounting, decompression, and files larger than the single-file extraction cap.
- The ISO parser handles ISO 9660 storage, **not** all optical-disc/drive-image formats. It never reads a physical optical drive.

## Common Linux compressed formats — current code audit
| Format | Core inspect | Core extract | Core create |
| --- | --- | --- | --- |
| ZIP (`.zip`) | Source implemented | No | Source implemented, pending integration |
| ISO 9660 (`.iso`) | New source implemented | New selected-file source, capped at 1 MiB | No |
| TAR (`.tar`) | No | No | No |
| gzip TAR (`.tar.gz`, `.tgz`) | No | No | No |
| XZ TAR (`.tar.xz`, `.txz`) | No | No | No |
| Zstandard TAR (`.tar.zst`, `.tzst`) | No | No | No |
| bzip2 TAR (`.tar.bz2`, `.tbz2`) | No | No | No |
| Single-stream gzip, xz, zstd, bzip2 | No | No | No |
| 7z, RAR | No | No | No |

The mere presence of `flate2` in Cargo dependencies is **not** TAR/gzip archive support.

## Release gates
1. Exact-head Rust/Linux and Windows CI compile and native hostile ISO tests, with source lock verification.
2. Dedicated desktop picker/approval for ISO source, file selection and destination with persisted human-turn association.
3. Streamed bounded extraction, multi-file extraction and atomic directory publish with crash/fault injection.
4. Recovery handles stages, partial publication and replay uncertainty without deleting unexpected external changes.
5. Real installed Windows desktop plus local GGUF integration acceptance.
6. Add Linux `.tar` / `.tar.gz` / `.tar.xz` / `.tar.zst` adapters with strict entry path, link, metadata, CRC/digest, decompression ratio, and expansion caps.

Do not claim complete ISO or common Linux archive support from this source-only PR.
