# IDE-MERGE-02C source-formatting and dependency-lock / P05–P06 evidence reconciliation

**Status: unpublished source-repair candidate; no native or installed acceptance inferred.** The original draft PR #236 integration head stays frozen at `f4e8a1a587c4aadd3d7293896324ceb8fa299738`. The original exact-head 62-workflow wave terminated **22 success / 40 failure**. Keep those original receipts and all negative security assertions.

## Exact original LLVM terminal result

Run `43743661529`, Windows job `132902112135`: pinned LLVM18 compiled, host LLDB DAP **25 passed / 0 failed**, including the prior Windows sealed-project-path failure. The later strict `cargo fmt --check` step failed on historical source/tests; Linux `132902112391` passed. The Windows job is **not** an overall pass.

## Source formatting batch

Mechanical Rustfmt output, extracted from that original Windows log, identifies **438 diff hunks in 40 `ide/host` Rust source, example, and test files**. Each hunk was compared to the current unmodified source before constructing a replacement. GitHub Actions' redaction of fixed fixture text was restored from the actual source, not the masked log. Retain all source behavior, tests, negative security assertions and original formatting checks. This review does **not** substitute for a fresh `cargo fmt --check` and tests on both OSes. Other supervisor/desktop formatting may remain open until their original gates rerun.

## Dependency-lock reconciliations

The original P06 run `43743661400` (Ubuntu and Windows) failed `DENIED: merge02_desktop_delta_unreviewed`. The verified `ide/app/src-tauri/Cargo.lock` source and original OS artifact resolve 500 packages and yield `b3b8c5bf056163c2747c28473b583b996b520917eb63e9a1b4eeb05437abca5a`. The genuine hash for the unchanged-outside-desktop block is `73afb4fa1fcfc63f2e30b15ecdb06570c55804088dcfa4bfbac78e9f41240cb4`. The historical immutable `ide-merge02-reviewed-lock-successor.json` and independently compiled `verify_p06_candidate_lock.py` contained `73afb4fa1fcfc63f2e30b15ecdb06570c55804088dcfa4bfbac78e9f41240cb4c`, a 65-hex typo, causing a false denial. Both sources now carry the 64-hex hash matching independently retrieved artifact bytes. **No dependency, version, tauri release, legal, approval or negative check changed.** The releaseApproved/licenseLegalReviewApproved/independentlyAuthorizedMainMerge flags remain false.

`ide/host/Cargo.lock` and `ide/supervisor/Cargo.lock` are absent from the original tracked tree. Earlier same-SHA native job artifacts produced byte-identical original Windows/Linux copies (host SHA256 `00de403d7aeb63cd476f7694d52af1db1ff25a87fd2460a782ad2301edf893ef`, supervisor `1fcc77eb3790e34b5815f69df2adb3948034d1944e12e4ddc904861038e1edfb`). Both exact cross-OS original standalone lockfiles are now staged in the **unpublished** successor as immutable Git blobs: host `90e52a812a834786d828fefb86d244732a882dd7` (39,190 bytes, 166 packages) and supervisor `cf3ea845e89c7b8242a75d71c9dd5288b5580595` (40,607 bytes, 171 packages). Two separate source-only verifier files and hostile tests pin the hashes and Git `HEAD` blobs, and grouped CI now rejects missing or changed locks instead of generating them. Original `--locked` tests are unchanged; no fresh passing native/installed test or release acceptance is yet claimed.

## P05 historical CI integrity

Original P05 run `43743660871` denied `reviewed_native_ci_workflow_changed:.github/workflows/ide-d7-windows-worker.yml`. Comparison of all **38** original P05 reviewed workflow blob pins with the frozen integration tree finds **seven** deviations:
- `.github/workflows/ide-d7-windows-worker.yml`
- `.github/workflows/ide-d7b-native-acceptance.yml`
- `.github/workflows/ide-d7d-protected-service.yml`
- `.github/workflows/ide-supervisor-core.yml`
- `.github/workflows/ide-wo6-native-os-acceptance.yml`
- `.github/workflows/ide-wo8-installed-windows.yml`
- `.github/workflows/ide-wo8-signed-protected-e2e.yml`

The current D7 file preserves the original CI job assertions but removes the original push micro-job trigger; the historical pinned original retains push and PR triggers. Replacing original P05 blob pins or reverting all seven without a behavioral and release-security review would bypass the immutable-historical-audit intent. This batch therefore leaves P05 original pins, original workflows, and tests **unchanged**. P05 gate remains FAIL / OPEN pending reviewed safe convergence or independently authorized successor proof.

## Independent open gates

Verify updated Rust formatting under the exact pinned toolchain; reproducible, tracked standalone Cargo locks; host/supervisor/native `--locked` builds, negative security tests and Tauri/DEV-01 original OS regressions. Investigate original workflow integrity and release P05/P06 evidence independently. No real Windows/R9700 installed NSIS acceptance, independent SCM/AppContainer/WFP hostile-socket proof, delegated Linux kernel cgroup-v2 custody, verified signed release/legal decision or independent `main` merge is authorized by this record.

## Additional original P06 same-SHA discovery — run #43763652480

The new-source original Windows and Linux P06 jobs remained FAIL with `merge02_desktop_delta_unreviewed`. Independent SHA-256 recomputation over the exact committed 500-package `ide/app/src-tauri/Cargo.lock` established a **second** malformed 65-digit review constant: `newDesktopBlockSha256` was `a5e4a1867eeb99c821b9e0a144aae01fe2895c49717b37756f7519f5b72594e3c` but the genuine exact block digest is `a5e4a1867eeb99c821b9e0a144aae01fe2895c49717b37756f7519f5b72594e3`. The whole Cargo lock digest `b3b8c5bf056163c2747c28473b583b996b520917eb63e9a1b4eeb05437abca5a`, unmodified-desktop digest `73afb4fa1fcfc63f2e30b15ecdb06570c55804088dcfa4bfbac78e9f41240cb4`, original package count 500, and exactly one local `rand` dependency are byte-verified. A targeted regression now requires both 64-digit hashes and rejects reintroducing the malformed constant. No actual dependency/version or original deny/release gate is altered. This correction is **unpublished** and **not accepted** until original unchanged-SHA Windows/Linux P06 tests rerun.
