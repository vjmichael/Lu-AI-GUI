# CORE-01F-03 / 04 — Final Lu Core integration preflight and acceptance reconciliation

**State: SOURCE CONFLICT RECONCILED ON REVIEW BRANCH / MAIN INTEGRATION BLOCKED.** This is a contemporaneous evidence snapshot, **not** native, installed, physical-owner, or merge acceptance. Do not merge while any mandatory acceptance is failed, skipped, absent, or attached to a different SHA. Production runtime remains llama.cpp; IDE, P7L and KoboldCpp changes are outside this Core integration.

## Branch identity and protected source

- Authoritative `main` at this review: `16ead199037215fdaf1b049b1d06edadac09b81c`.
- CORE-01E exact head: `54c1cf305fade7bfbfe0f19e181ea18d3e4af3b5`, original draft PR [#226](https://github.com/vjmichael/Lu-AI-GUI/pull/226).
- Reconciliation starts from precisely that CORE-01E head on `lu/core01e-main-ledger-reconcile-20261112`. CORE-01E and `main` have not been changed by this review.
- The four `main`-only commits since the common ancestor change only `docs/LU_DEVELOPMENT_LEDGER.md`.
- Reconciliation restores the **exact `main` Git blob** for `docs/LU_DEVELOPMENT_LEDGER.md` (`627cb6f4a5b464a0631fd7f25cbb267eab0729f8`), and the **exact `main` workflow blob** for `.github/workflows/lu-development-ledger.yml` (`35c5de7ab6bb4e4b50db76f785064809f342396b`). The source-tree lock is adjusted to the actual review branch, not copied blindly from either side.
- Preserve the live `main` ledger through each future integration step. Never merge the stale historical Core copy into `main`. Scope and release gates must be rechecked after a final restack.

## Original acceptance receipts by source SHA

| Evidence | Exact source | Run / job | Verified disposition |
| --- | --- | --- | --- |
| Earlier full native Linux | `88e34abd04e91c7ac14c20656673d3c7a5d59a3f` | [run 43338530675](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/43338530675), job 131609251367 | **FAIL:** `task_checkpoints::durable_progress_survives_restart_without_forced_timeout`; later source changed |
| Earlier full native Windows | `97dd1e897c4cf73a1fd8d8bf36f1233b081322c0` | [run 43342512185](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/43342512185), job 131621801138 | **FAIL:** `tauri::generate_context!()` cannot find `../dist` during desktop test compilation |
| Latest source identity | `54c1cf305fade7bfbfe0f19e181ea18d3e4af3b5` | [run 43346346769](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/43346346769), job 131627466542 | **PASS:** frozen source lock and REPO-01 tests |
| Latest Windows and Linux compiler matrices | same `54c1cf30` | run 43346346769, jobs 131629471250 and 131629471370 | **PASS:** exact-head compiler typechecks, desktop test compilation and frontend build |
| Latest Windows and Linux model-to-tool/UI matrix | same `54c1cf30` | run 43346346769, jobs 131629471180 and 131629471368 | **PASS:** targeted model/tool/image/UI and host test contracts |
| Real Qwen / SmolVLM, hosted CPU | same `54c1cf30` | run 43346346769, job 131629471214 | **PASS** targeted local-model trials; not physical Radeon acceptance |
| Latest full native Linux | same `54c1cf30` | run 43346346769, job 131634858573 | **FAIL:** Core Rust original regressions pass, but full frontend/UI suite: **544 total; 486 pass, 55 fail, 3 skipped**. Includes source-contract mismatches and missing `lu-file-scan-worker-acceptance` executable (`ENOENT`). Desktop tests did not run |
| Latest full native Windows | same `54c1cf30` | run 43346346769 | **OPEN:** no successful original full-regression job receipt verified at time of this review; targeted compiler success is not a substitute |
| Hosted NSIS and Debian installed evidence | same `54c1cf30` | [PLAN-07D run 43346346727](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/43346346727) | **SKIPPED:** hosted NSIS, Debian, and genuine-model installed jobs (no installed success receipts) |
| New reconciliation candidate | review branch, distinct from `54c1cf30` | new Git commit / PR source job | **OPEN:** full source lock and same-head native+installed acceptance need separate proof |

The Linux failure summary comes from the original job log, not a green source-only check. Several of the 55 expectations refer to absent/changed frontend integration points (planning, browser/research, Observe & Learn, Reader and workspace UI). This is not proof that tests are obsolete; each expectation must be resolved against the intended production feature and authorization contract. The worker executable must be built/provisioned for its real end-to-end tests, not mocked into a pass. In the previous Windows run the failure was the Tauri frontend-dist build order; subsequent compiler success supports the repair but cannot certify full native test completion.

## 01F-03 preflight disposition

**PASS for the narrow source-conflict resolution only:** current `main` ledger + ledger automation are byte-identically retained; reconciliation diffs against CORE-01E are ledger, ledger workflow, lock and this evidence record. No merge commit and no change to protected CORE-01E. **Not yet a whole-repository source-verification PASS:** run the exact-head source verifier after recording this file and re-locking the manifest. Re-evaluate if either `main` or CORE-01E moves.

The older [CORE-01F #228](https://github.com/vjmichael/Lu-AI-GUI/pull/228) preflight was based on CORE-01E `97dd1e89`, and its pinned SHA comparison needs a reviewed restack/update before it can attest `54c1cf30` or any post-reconciliation head. Do not force-update it while the independent original acceptance run is active.

## 01F-04 required proof before any integration authorization

1. One **immutable final SHA**: on both native OS runners execute complete original Rust Core and Tauri desktop regressions, frontend/UI suite, and review the original compiler/test logs. Zero unexplained failures or skipped mandatory tests.
2. Independently verify Windows Job Object process-tree custody and Linux group/cgroup custody, revocation, cancellation, guardian death and durable journal/restart reconciliation. No source-only proxy substitutes.
3. At **that same reviewed SHA**, perform actual hosted Windows NSIS install, UIA interaction, restart and uninstall; and Debian package install, AT-SPI interaction, restart/removal and post-uninstall state verification. Require genuine recorded binary/source identities and original receipts.
4. Reconcile original artifacts/logs and any rerun or repair at the final SHA. Any code or workflow fix changes the final SHA and invalidates prior full acceptance on earlier SHAs.
5. Recheck moving `main` paths, reviewed bridge ancestry, source lock and IDE/KoboldCpp exclusion. Preserve `main`'s current ledger and workflow. Then request separate explicit merge authorization.
6. Physical owner Windows Radeon/Vulkan with primary Qwen/MMProj, human interaction, and reviewed browser runtime packaging remain separate unclaimed release gates.

**Current gate verdict:** `mergeAuthorized=false`; `releaseAuthorized=false`; `nativeWindowsLinuxAccepted=false`; `installedWindowsDebianAccepted=false`; `independentCustodyAccepted=false`. No existing source-only or targeted compiler/model green run overrides these negative assertions.
