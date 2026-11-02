# Lu R22 — Windows development evaluation build evidence

**Status:** NSIS evaluation installer built and locally validated; physical Windows acceptance still pending.

**Source baseline:** [Lu-AI-GUI commit 0d34d70595f93979c8d4370627be217cda5f8041](https://github.com/vjmichael/Lu-AI-GUI/commit/0d34d70595f93979c8d4370627be217cda5f8041)

## Verification
- Frontend bundle built using existing offline `node_modules`; 28/28 source checks passed.
- Node frontend regression suite: 43/43 passed.
- Prior R22 offline Lu Core Rust suite: 111 passed, 0 failed (recovered test log).
- Rust Windows MSVC x64 GUI binary linked from R22 source and existing offline vendor/toolchain.
- PE inspector: x64 GUI subsystem, embedded `RT_MANIFEST` with Common Controls v6, `TaskDialogIndirect` import.
- Pinned Vulkan package manifest and four engine-file SHA-256 hashes verified at NSIS build.
- NSIS build succeeded with local NSIS toolchain and zlib compression; Windows execution not observed here.

## Installer
- File: `Lu_R22_Typed_Chat_Windows_DEV_EVAL_NOT_ALPHA.exe`
- Size: 22806404 bytes
- SHA-256: `fbb41433df3446dadef73157e0985dd18d232b886af8a0df103274991e4aaed4`
- Source executable SHA-256: `201234ff03da1b82bfde35c847dd7c4208860e746364352fe5cebf28a38c6022`
- Windows Vulkan server bundled: `llama-server.exe` and `msvcp140.dll`, `vcruntime140.dll`, `vcruntime140_1.dll`.

## Caveats
- UI source checks were adjusted to assert typed Protobuf context rather than the obsolete flattened prompt.
- Installer display name, install directory, uninstall name and bundled README were changed from R20 labels to R22, preventing an accidental R20 overwrite.
- These packaging-only/test-check edits need to be committed separately from the R22 inference source baseline.
- No current-turn Windows hardware execution or Vulkan compute logs available; do not promote to alpha.
