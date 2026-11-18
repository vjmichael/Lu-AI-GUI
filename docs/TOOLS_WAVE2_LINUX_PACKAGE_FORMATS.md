# Wave 2 Step 2 — Linux package and backup formats

Status: DEVELOPMENT SOURCE ONLY. Exact-head Windows/Linux Rust CI and installed-desktop acceptance remain required.

## Implemented this slice

| Format | Inspect | Extract into new folder | Notes |
|---|---|---|---|
| CPIO newc 070701 | Yes | Yes | Standard Linux backup/initramfs ASCII container |
| CPIO CRC 070702 | Yes | Yes | Full 32-bit additive checksum checked for each entry (not polynomial CRC32) |
| CPIO gzip | Yes | Yes | In-process flate2 on Windows and Linux |
| CPIO xz / bzip2 / zstd | Linux with fixed trusted decoder | Linux | Host-installed /usr/bin or /bin decoder required; Windows unsupported |
| Unix ar / .a | Yes | Yes | Normal/GNU long-name/BSD extended names; symbol metadata skipped |
| Debian .deb | Yes | Yes | Inert ordered Debian 2.x ar package; separate control/ and data/ subfolders |

The existing Lu archive tree engine is reused: source and destination directory capabilities pinned by the trusted host, full manifest SHA-256, a NEW output folder, no overwrite, and postwrite content verification. A DEB is NEVER installed or executed, and maintainer scripts are only inert bytes. No dpkg/apt, elevated privileges, owner/permission restoration or arbitrary filesystem root publication.

The unified read-only archive list model tool accepts CPIO, ar and DEB. The desktop Extract archive interface recognizes the new extensions and strips common suffixes for new folder names.

### Safety and limits

Input <= 128 MiB, expanded bytes <= 256 MiB, individual member <= 64 MiB, <= 4096 entries, path depth <= 16. Reject symlinks, hardlinks, devices, FIFOs, path traversal, malicious aliases, Windows-reparse inputs, duplicate names, unsupported CPIO variants, missing trailer, malformed headers and CRC mismatches. CPIO requires a valid TRAILER!!! and no nonzero tail. Debian control and payload trees have separate safe namespaces. Debian data.tar.lzma remains unsupported.

An interrupted multi-file extraction is not rolled back or automatically replayed. It is flagged for manual reconciliation using the existing durable effect journal and full-tree completion marker.

### Native tests

The native_linux_packages test suite covers CPIO newc/CRC, compressed gzip CPIO, checksum tampering, traversal and hardlink refusal, Unix ar GNU long filenames, a real three-member Debian package, extraction and no-overwrite, malformed/truncated headers, and an authenticated read-only Lu archive-list tool call. Dedicated Wave 2 Linux/Windows CI requires the tests to pass.

### Still missing from Step 2

- Full 7z and RAR compressed backends, password/encryption policies and complete host-approved extraction.
- RPM package metadata and decompression/payload handling without invoking rpm.
- LZ4 backup variants, legacy CPIO binary/odc, multi-volume archives, UDF ISO, and full archive creation parity.
- Windows-native xz/bzip2/zstd codecs; current fixed system decoder is Linux only.
- Rich Office editing for complex charts, styles, formulas, macros, templates and embedded objects.
- Exact-head native green verification, installed desktop testing and crash/cancellation/race acceptance.

References: Debian deb(5) at https://manpages.debian.org/trixie/dpkg-dev/deb.5.en.html and CPIO format at https://man.archlinux.org/man/core/libarchive/cpio.5.en.

Do not mark this work release-accepted based on source commits or queued CI.
