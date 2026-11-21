# IDE-MERGE-02C — Real installed Windows/Linux notice-stage repair
Unpublished successor of frozen `cdef9ac364b6856e225fec5e7ebe17ce4d7ab91b`, grouped with the earlier LSP/FIN01/rustfmt source-only repair. **No branch, PR, CI dispatch, main merge, original test weakening, or acceptance assertion.**

Source-identical original installed DEV-01 six-job integration #43775561446:
- Windows native desktop compile/link: **PASSED**
- Linux native desktop compile/link: **PASSED**
- Original Windows 6D/6E/6F Git security/restart: **PASSED**
- Original NSIS installed Windows job #133000943990 **FAILED IN BUILD**, after verified P06 source pin and 477 frontend tests, because Tauri `resource path 'verification-notices' doesn't exist`. No actual NSIS installed GUI test began.
- Original Debian installed Linux job #133000943906 **FAILED IN BUILD** with the identical Tauri missing resource; no actual Debian installed UI test began.
- Original Linux Git job pending at initial analysis. Original LLVM18 Windows owner job pending.
- Older Linux supplemental installed job #133000943794 *did* reach native GTK but failed with missing AT-SPI-accessible `Workspace projects`. Different genuine GUI diagnosis, not solved by packaging.

Root cause: integrated DEV-01 new NSIS/DEB build steps omitted the original WO8 `cargo fetch --locked` → exact source-pinned upstream license retrieval → real local/npm/Cargo license collection → verification chain before Tauri bundling. **Do not substitute a blank directory** or disable required Tauri resources.

Repair: reuse original `upstream_notice_receipt.py`, `collect_verification_licenses.py` and `verify_p04_license_bundle.py` as existing unchanged policies, after locked Cargo fetch and remeasurement by scoped independently pinned `verify_p06_candidate_lock.py --ide-merge02-successor`. Verify the actual staged source SHA and upstream receipts *before* package build; after Tauri NSIS/DEB build require exact package hash-bound receipts with `--require-upstream`. Extend native OS install fixtures to prove actual `THIRD_PARTY_LICENSES/inventory.json` and `THIRD_PARTY_LICENSES/upstream-fetch.json` in the package/installed files, without skipping existing GTK/Windows UIA checks, hard kill, restart, cleanup. New frontend source-contract negative guards ensure both platforms keep these steps.

This correction has NOT run; native original installed WebView2/GTK, real Windows SCM/Job/WFP custody, Linux kernel custody, release/legal/signing, historical P05 trigger authorization and `main` merge remain OPEN. Any next same-SHA acceptance wave must follow completion and reconciliation of all prior `cdef9ac3` original jobs.
