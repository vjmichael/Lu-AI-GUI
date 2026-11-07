# Lu IDE P05 — GitHub PR and CI consolidation audit

**Audit date:** November 7, 2026  
**Branch:** `ide/release-closeout-p05-20261107` stacked on [P04 draft #84](https://github.com/vjmichael/Lu-AI-GUI/pull/84), source candidate `bc4fd78685c342675e4a80556970e934125a0e20`.  
**Status:** Source and workflow inventories committed; exact-head CI verification pending. No PR was retargeted, closed or merged, and no native/release workflow was removed.

## Decision and boundaries

P05 reduces GitHub duplication **without losing source code, original commit history, approval boundaries or OS evidence**. The current user instruction is to work on P05, not to waive P03/P04 native and release gates. P03 real protected Windows LLDB-DAP/clangd and Linux debuggee acceptance and P04 real installed Win/Linux editing, protected Run/Test/Debug, reviewed notices, signed packages and transactional installer acceptance are separately open. The signed identity remains deferred. **P07/WO10 is explicitly deferred until after Lu Core v1.0 and user authorization.**

The GitHub repository currently had **78 open user PRs**, of which **30 open IDE-specific PRs** were identified by exact `ide/` branch. This replaces the older planning snapshot of 26 IDE PRs. GitHub Git Trees for each PR's exact head were compared against the pinned P04 candidate, checking **`ide/` and `.github/workflows/ide-*` paths and blob SHA-1s**, not merely PR titles or first-page diffs. The full machine-readable [PR inventory](../packaging/p05-pr-source-audit.json) includes exact head and base branch, historical IDE file count, modified-existing-blob count, missing paths and current retain/disposition reason. The [workflow inventory](../packaging/p05-workflow-coverage.json) locks all 38 original GitHub workflow blobs and disables automated CI-pruning authority.

**Important:** A historical file present under the same name but with a different blob is **not** proof the old functionality was preserved or lost. No PR can be closed solely from source-path coverage; native OS acceptance, changed behavior, reviewed rollback, release/CI ownership and explicit source disposition must be assessed independently. The P05 source checker is a provenance guard, not release acceptance.

## Full IDE PR review inventory

| PR | Exact head SHA prefix | IDE files at head | Candidate differing existing blobs | Missing candidate paths | Classification | Disposition |
| --- | --- | ---: | ---: | ---: | --- | --- |
| [#43](https://github.com/vjmichael/Lu-AI-GUI/pull/43) | `3cb62b2d1519` | 84 | 27 | 3 | parallel unported | Retain draft/open |
| [#45](https://github.com/vjmichael/Lu-AI-GUI/pull/45) | `21b1d88aaa83` | 87 | 27 | 6 | parallel unported | Retain draft/open |
| [#46](https://github.com/vjmichael/Lu-AI-GUI/pull/46) | `a94c5d6af266` | 84 | 22 | 3 | parallel unported | Retain draft/open |
| [#47](https://github.com/vjmichael/Lu-AI-GUI/pull/47) | `efe731d874e1` | 88 | 23 | 0 | historical stack | Retain draft/open |
| [#48](https://github.com/vjmichael/Lu-AI-GUI/pull/48) | `ca606dfb378c` | 90 | 23 | 0 | historical stack | Retain draft/open |
| [#49](https://github.com/vjmichael/Lu-AI-GUI/pull/49) | `1dd4a58d84d6` | 93 | 21 | 0 | historical stack | Retain draft/open |
| [#50](https://github.com/vjmichael/Lu-AI-GUI/pull/50) | `c403a8ba47d6` | 96 | 19 | 0 | historical stack | Retain draft/open |
| [#51](https://github.com/vjmichael/Lu-AI-GUI/pull/51) | `ef623bfd22f8` | 99 | 21 | 0 | historical stack | Retain draft/open |
| [#52](https://github.com/vjmichael/Lu-AI-GUI/pull/52) | `c091fbe174b8` | 102 | 22 | 0 | historical stack | Retain draft/open |
| [#53](https://github.com/vjmichael/Lu-AI-GUI/pull/53) | `272c6637b4e7` | 107 | 27 | 0 | historical stack | Retain draft/open |
| [#54](https://github.com/vjmichael/Lu-AI-GUI/pull/54) | `32b208a76ed1` | 111 | 29 | 0 | historical stack | Retain draft/open |
| [#55](https://github.com/vjmichael/Lu-AI-GUI/pull/55) | `341b3ab5ea26` | 114 | 28 | 0 | historical stack | Retain draft/open |
| [#56](https://github.com/vjmichael/Lu-AI-GUI/pull/56) | `c36858ca215f` | 117 | 26 | 0 | historical stack | Retain draft/open |
| [#58](https://github.com/vjmichael/Lu-AI-GUI/pull/58) | `6923b43f52ad` | 121 | 30 | 0 | historical stack | Retain draft/open |
| [#60](https://github.com/vjmichael/Lu-AI-GUI/pull/60) | `b172095a6a10` | 124 | 31 | 0 | historical stack | Retain draft/open |
| [#62](https://github.com/vjmichael/Lu-AI-GUI/pull/62) | `4c2b639c379d` | 131 | 37 | 0 | historical stack | Retain draft/open |
| [#63](https://github.com/vjmichael/Lu-AI-GUI/pull/63) | `988bf11a3607` | 142 | 47 | 0 | historical stack | Retain draft/open |
| [#64](https://github.com/vjmichael/Lu-AI-GUI/pull/64) | `73b8a5bc8f77` | 146 | 49 | 0 | historical stack | Retain draft/open |
| [#65](https://github.com/vjmichael/Lu-AI-GUI/pull/65) | `bd39d01129f8` | 174 | 26 | 0 | historical stack | Retain draft/open |
| [#68](https://github.com/vjmichael/Lu-AI-GUI/pull/68) | `91134704c308` | 180 | 25 | 0 | historical stack | Retain draft/open |
| [#69](https://github.com/vjmichael/Lu-AI-GUI/pull/69) | `785ca287d74c` | 183 | 25 | 0 | historical stack | Retain draft/open |
| [#70](https://github.com/vjmichael/Lu-AI-GUI/pull/70) | `c0b83a304566` | 202 | 30 | 0 | historical stack | Retain draft/open |
| [#73](https://github.com/vjmichael/Lu-AI-GUI/pull/73) | `d42586345a8f` | 226 | 33 | 0 | historical stack | Retain draft/open |
| [#74](https://github.com/vjmichael/Lu-AI-GUI/pull/74) | `bd0f4f1280fb` | 240 | 37 | 0 | historical stack | Retain draft/open |
| [#75](https://github.com/vjmichael/Lu-AI-GUI/pull/75) | `0d271606c8bc` | 252 | 38 | 0 | historical stack | Retain draft/open |
| [#76](https://github.com/vjmichael/Lu-AI-GUI/pull/76) | `8d59bce140d1` | 261 | 43 | 9 | deferred do not merge | Retain draft/open |
| [#78](https://github.com/vjmichael/Lu-AI-GUI/pull/78) | `a8b0a8f02708` | 254 | 38 | 0 | phase chain | Retain draft/open |
| [#79](https://github.com/vjmichael/Lu-AI-GUI/pull/79) | `fab2c06e4745` | 257 | 28 | 0 | phase chain | Retain draft/open |
| [#80](https://github.com/vjmichael/Lu-AI-GUI/pull/80) | `ed070647dde2` | 263 | 15 | 0 | phase chain | Retain draft/open |
| [#84](https://github.com/vjmichael/Lu-AI-GUI/pull/84) | `bc4fd78685c3` | 290 | 0 | 0 | phase chain | Retain draft/open |

All 30 PRs remain **open/draft**. Every one contains at least historical code/data that must remain reviewable; there is no basis for a blanket "superseded; close" action. Of the 30, 26 have **zero entirely missing IDE/CI paths** at the pinned candidate, but some existing files differ, usually because newer work evolved them. The other four (#43, #45, #46, #76) have source files absent from the candidate and require explicit tracking.

## Unported independent source and deliberate exclusion

- **#43, revision-aware buffer identity:** `ide/app/web/buffer_identity.js`, `ide/app/tests/buffer_identity.test.mjs` and `ide/docs/IDE_BUFFER_REFERENCES.md` are absent. Both source blob SHA-1s and docs are pinned in the machine inventory. The modern CodeMirror editor is not proven compatible with the older API. **Retain, review if feature port is required.**
- **#45, installed native buffer acceptance:** includes exactly the three shared #43 missing blobs plus `ide-buf-installed-acceptance.yml`, `tauri.installed-acceptance.conf.json` and `windows-installed-buffer-smoke.ps1`. The three shared blobs match #43 exactly. **Do not claim acceptance by current WO8 installed CI unless the reviewed buffer behavior is ported and tested.**
- **#46, Find in Project:** `ide/host/src/project_search.rs`, `ide/host/tests/project_search.rs` and `ide/docs/IDE_PROJECT_SEARCH.md` are absent; this is an actual omitted feature, not an old renamed workflow. **Retain PR or independently port with explicit owner scope, source review and tests.**
- **#76, WO10 bridge:** exactly nine IDE/CI files are absent, intentionally. `ide/bridge-contract`, `bridge_policy.rs`, `bridge_wire.rs`, `bridge.proto` and its workflow remain **excluded** under P07's post-v1.0 decision. **DO NOT MERGE, PORT, RETARGET OR CLOSE until user-directed post-v1.0 review.**

P01 also flagged late fixes on #47, #53, #56, #58, #70, #73 and #74. They have no missing *filenames* at the P04 snapshot, but their differing blobs and run histories require explicit semantic and CI evidence comparison before PR consolidation. The other ancestral WO9-era PRs have path coverage but must not be auto-closed. For P01's detailed lineage discussion, consult [IDE P01 source audit](IDE_P01_INTEGRATION_AUDIT_20261107.md).

## CI workflow inventory and reduction decisions

All **38** GitHub `ide-*.yml` workflows found at the P04 exact head are currently pinned by original Git blob. Existing concurrency settings, path/branch filters, manual-only signed gates and exact-head native jobs remain unchanged by P05.

The workflows overlap in **topic**, but do not necessarily test equivalent things:

| Area | Candidate or older proof | Reduction safety decision |
| --- | --- | --- |
| Native Windows containment | D7B/D7C/D7D, AppContainer, OS-rights, sandbox, WO6 | **Retain** until P03 direct Windows service/Job/LLDB proof independently accepted |
| Linux cgroups and protected clangd | sandbox, WO4 delegation, WO5 real clangd, E2 Linux, P03 OS checks | **Retain** until current exact-head kernel and editor operations pass |
| DAP/Cargo and failure recovery | WO6 protocol/OS, native desktop, E2, protected recovery | **Retain** native permissions, real debugger launch and cleanup gates |
| Installed Windows/Linux UI and Git/ZIP | WO8 and WO9 actual NSIS/DEB and native UI | **Retain** both, as WO9 adds Git/ZIP behavior WO8 doesn't prove |
| License and signing | WO7 preflight, P04 evidence, manual signed installer/upgrade workflows | **Retain**, user deferred final signature identity but not license verification |
| Older source/protocol checks | E1/E2, supervisor-core, standalone, continuity and appcontainer | Review after P03/P04 replace their particular assertions, not by filename similarity |
| Manual signer-only runs | WO7/WO8/WO9 signed acceptance and P04 upgrade/reinstall | **Never auto-delete** because source jobs pass. These require separate trust/artifacts. |

P05 `verify_p05_pr_inventory.py` reads both pin sets, rejects forged PR closure/native acceptance, incomplete historical exception lists, WO10 import or removed CI. It verifies every original workflow's exact Git **blob SHA** in the current checkout on Windows and Linux; any change requires explicit inventory update and independent review. The P05 CI workflow is new and separate. No native check has been redirected to mock behavior, skipped or made `continue-on-error` to gain a green result.

## Remaining P05 gates

- [x] Inventory all 30 presently open IDE PR heads/bases; record exact source hashes, omitted paths and unresolved blob differences.
- [x] Account for parallel B2/B3 source and preserve excluded WO10 with explicit exact pinned paths.
- [x] Inventory all 38 IDE CI workflows and mark every one as retained until an accepted equivalent exists.
- [x] Add Windows/Linux automated fail-closed PR and workflow integrity guard (source committed; exact-head CI acceptance pending).
- [ ] Compare semantics of modified older implementation/fixture blobs and their historical OS receipts with current P03/P04 evidence; identify any additional missing behavior not detectable by names or hashes alone.
- [ ] After independent acceptance of P02–P04 replacement: decide individually whether to retarget, close or merge old PRs; preserve human review and history. **Current authorized closures: 0.**
- [ ] Only after replacement gates pass, consolidate/restrict duplicate CI triggers without dropping required native security, installed GUI, rollback, signed package or legal checks.
- [ ] Close P05 source audit when CI passes and final disposition/reduction is genuinely evidenced; otherwise mark P05 **OPEN**, not a forced green phase.

**Stop at P05. P06 has not started. P07/WO10 remains deferred.**
