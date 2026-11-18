# Lu V1-01 — Windows icon + measured model import (development candidate)

Date: 2026-11-03

## Provenance and branch status
The owner-accepted **0.1.0-alpha.2** Windows installer remains the functional-alpha baseline. This branch records the subsequent **0.1.0-alpha.3-dev.1** V1-01 work; it is **not** accepted and does not replace the baseline.

**Important source synchronization warning:** The GitHub main source predates the accepted alpha.2 snapshot. This documentation commit does **not** pretend that all 240 files in the separately tested development source archive are synchronized into this branch. Do not build an installer from the older repository tree and label it equivalent to the tested local snapshot. GitHub is the repository; the offline Windows build and physical acceptance remain separate.

## Completed local changes
1. Diagnosed missing taskbar icon in the original alpha.2 PE binary: resource directory contained only RT_MANIFEST (24), no RT_ICON (3) or RT_GROUP_ICON (14).
2. Extended the pinned offline Windows resource builder to embed validated ICO images as native RT_ICON (3) and RT_GROUP_ICON (14), alongside RT_MANIFEST (24); Tauri config explicitly lists icons. Updated PE verifier to optionally require both native icon resources.
3. Added native file copy progress callbacks for GGUF import, reporting actual copied/total bytes in bounded chunks with stage validation and atomic publication preserved.
4. Wired Tauri host event `lu-model-import-progress` to Settings UI: selected filename, measured bytes, percentage, validation and completion state.
5. Bumped **development-only** version to `0.1.0-alpha.3-dev.1`.

## Evidence from local toolchain
- Frontend tests: **85/85 passed**.
- Frontend/source contract checks: **29/29 passed**.
- Native Lu Core GGUF import tests: **4/4 passed**, including new monotonic measured-progress test.
- Windows x64 Rust check and linked build: **passed** (Rust 1.98.0, offline pinned Windows SDK / MSVC CRT / lld-link).
- New PE resource directory: **types 3, 14, 24**; icon + manifest verifier passed.
- NSIS packaged: **Lu_0.1.0_alpha3_dev1_Windows_V1_01.exe**, 23,082,513 bytes.
- Installer SHA-256: `9ea4878a61a58cd082ee21e1af8df1301a2e138722b382d98d308d46e`.
- Windows EXE SHA-256: `cb53488f94e6bdf3b04e445ea8524ecff9e33672305e7526b7071e2ca1f6f1a7`.
- Complete source archive SHA-256: `e8b28b5a805b0a74944ce14f4b72b195ffd1b91dfe91b5362268fae644f8db97`.
- Source patch archive SHA-256: `8fbefc440f215bb03653cb1c13d2d30ed10a24d7423b2a42f753b458109fdbf8`.

## Pending and known failures
- Actual Windows 10 taskbar icon behavior requires physical retest. PE icon resources are a prerequisite, not definitive UI acceptance.
- GGUF import progress UI requires physical retest on a large model.
- Lu sometimes displays Windows **Not Responding** while loading model weights. Root cause not established; loading progress is still indeterminate (not the same as measured *import/copy* progress).
- Video/audio file handling and native-tool authorization remain subsequent v1.0 gates, not this change.
- Installer is unsigned; SmartScreen unknown-publisher warning expected.
- Source branch still requires **exact full alpha.2 source convergence** before using it as canonical build input.

## Next
Converge the verified full source to GitHub without clobbering the accepted baseline. Then instrument Windows model-loading responsiveness with actual native event-loop and engine lifecycle traces, and test icon and import progress physically.
