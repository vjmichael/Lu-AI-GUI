# DEV-04.3 — Native executable format and debug-symbol identity

**Status:** Source implementation on branch `ide/dev04-3-executable-debug-symbols-20261108`, stacked on draft DEV-04.2 PR #164. The native and installed acceptance gates remain OPEN until exact-head real OS results.

## Purpose and authority boundary

A completed protected Cargo build's advisory binary hash is not enough to offer a project debugger. Only after the existing supervisor has proven zero-exit Completed, durable exact-job receipt, no kernel quarantine, and independently hashed immutable output, the host may attempt a **read-only structural format and debug-info audit** of the same native-owned output root.

No WebView-supplied path, external executable, PDB path, symbol file, command, PID or permission enters this verifier. Each file is required to match the existing output manifest by exact SHA-256. Directory components are checked for symlink/reparse exposure; file reads use no-follow handles, size/time checks and Unix inode/hard-link checks, bounded to 64 MiB per file. Missing, changed, ambiguous and unsupported formats return **no symbol witness**. Any reader error fails closed.

## Linux executable and DWARF checks

Only native little-endian x86-64 ELF64 executable/PIE images are supported:
- Validate ELF ident, architecture, executable/shared-object type, header sizes, program/section table ranges, a loadable executable segment and entry point.
- Resolve section names from the actual section string table. Require exactly one each of nonempty, uncompressed `.debug_info`, `.debug_abbrev` and `.debug_line` sections, all with bounded offsets/sizes.
- Validate initial DWARF compile-unit and line-unit length/version, address size and abbreviated section offset. Hash the exact three sections with domain separation; the source binary's full SHA must independently match the output witness.
- Reject foreign machine formats, absent sections, bad section offsets, compressed/split DWARF and malformed unit headers. This deliberately does **not** parse every DIE, relocations, ranges, variables, source-line row, or ensure LLDB can read all symbol records.

Result kind: `embedded_dwarf_info_abbrev_line`.

## Windows executable and PDB checks

Only native x64 PE32+ images are supported:
- Validate DOS→NT/COFF headers, 64-bit optional header, executable characteristic, section table bounds, executable section and mapped debug directory.
- Require exactly one CodeView RSDS record, carrying a nonzero GUID and age, and a bounded PDB basename. Never follow the embedded potentially absolute PDB path.
- Resolve the **sole** expected fixed-output PDB `target/debug/<binary-stem>.pdb`, which must be part of the previously independently hashed output manifest. Independently read its complete bytes and rehash its SHA-256.
- Validate its MSF7 superblock/stream directory, PDB Info stream #1 GUID/age exactly matching RSDS and DBI stream #3 signature/age/x64 machine. Reject malformed block maps, missing streams, mismatched PDB, wrong platform and manipulated contents.

Result kind: `external_pdb_guid_age_dbi_match`.

These checks follow the **Microsoft PE Debug Directory** and **LLVM PDB Info / MSF / DBI** documented layouts. They do not verify all CodeView records, line mappings, signed developer identity or runtime debugger compatibility.

References:
- https://learn.microsoft.com/en-us/windows/win32/debug/pe-format
- https://llvm.org/docs/PDB/PdbStream.html
- https://releases.llvm.org/20.1.0/docs/PDB/MsfFile.html
- https://llvm.org/docs/CommandGuide/llvm-dwarfdump.html
- https://dwarfstd.org/doc/DWARF5.pdf

## Exact-job witness

The returned `DebugSymbolWitness` contains only exact relative binary path, binary SHA, output tree SHA, platform and debug-symbol kind, optional expected relative PDB path, symbol digest and narrowly named verification scope. The frontend validates all identity fields against the same native `BuildRunView` and its manifest, which is tied to supervisor job/plan/source/toolchain/terminal receipt through DEV-04.2. No filesystem/worker handles, source text or private PDB source paths enter the WebView.

`debugSymbolsAttested=true` means only that **these specifically described structural checks were verified against the output bytes at observation time**. `sourceLineMappingVerified=false`, `debuggerExecutionAuthorized=false` and `installedAppAccepted=false` are invariant.

## Tests and genuine OS work

- Native Linux unit fixtures: valid/malformed ELF64 sections, DWARF unit lengths, architecture, full output-manifest rehash, changed executable denial.
- Native Windows unit fixtures: PE32+ debug directory and RSDS, MSF7 stream-directory resolution, matching PDB Info GUID/age and DBI machine/age, changed PDB manifest denial.
- JS contracts reject a false match, forged PDB path, mismatched manifest, cross-project result, cancelled build, fake authorization or source-line verification.
- Native Linux WO6 delegated cgroup-v2/Bubblewrap **real** `rust_build` acceptance is now required to report a matching ELF/DWARF witness.
- Native Windows D7B/D7D pre-assigned AppContainer/Job/SCM **real** `rust_build` acceptance is now required to report matching PE/RSDS/PDB evidence.
- Separate exact-head Windows/Linux Node/Rust/Tauri workflow: `.github/workflows/ide-dev04-3-debug-symbol-native.yml`.

A source fixture or a synthetically assembled PE/PDB is **not** proof of a native protected Cargo build or installed debugging.

## Open gates before DEV-04.4

Obtain actual passed exact-head Windows/Linux source, protected Cargo and kernel cleanup results, fix genuine failures, and independently verify the real symbol witness artifacts. Verify file identity/handle custody against swap attacks (particularly Windows hardlinks), full format-and-DWARF/CodeView semantic validation, source-to-line mapping, protected installed debugger adapter and symbol-load integration, then fresh user approval, OS custody, cancellation and journal recovery. Signing and third-party license review remain independent.

DEV-04.4 may only implement a separately approved protected project debugger; do not change Lu Core containment, advertise arbitrary launch, or treat source-level checks as release acceptance.
