# IDE-MERGE-02C | P05/P06 independent historical security and release review
Source-only reconciliation recorded 2026-11-15. **NOT** a native, installed, physical, legal, or `main` release authorization.

## Exact source and original receipts

- Frozen published IDE branch and draft PR #236: `f4e8a1a587c4aadd3d7293896324ceb8fa299738`.
- Unpublished repair candidate reviewed here: parent `9a2cbe4a98010f03c0d0c99e5b7aaa2ad9241186`; no branch mutation or new CI.
- Original exact-head wave: 62 terminal workflows, 22 success and 40 failure. Original pinned LLVM run `43743661529`: Windows LLVM18 native compilation and 25/25 host DAP tests passed, but original strict Rustfmt then failed; Linux default passed.
- Original consolidated `43743660974`: original native Git/restart job pair passed on Windows/Linux; all four native desktop/installed DEV-01 jobs blocked by 5 identical original frontend source assertions (470/475 passed) before installed GUI/restart evidence.
- Original P05 audit `43743660871`: **FAIL**, `reviewed_native_ci_workflow_changed:.github/workflows/ide-d7-windows-worker.yml`.
- Original P06 evidence `43743661400`: **FAIL**, `merge02_desktop_delta_unreviewed` due to malformed, 65-hex unchanged-desktop digest in otherwise correctly reviewed desktop 500-package graph.

## P05: keep original immutable-source protection (38 reviewed workflow identities)

Comparison was against the exact 38 originally pinned blobs in `ide/packaging/p05-workflow-coverage.json`. 31 match without change; 7 have real source differences. Below, *historical* SHA is the original blob and *current* SHA is the frozen source Git blob. Never update original pins merely to silence a denial.

| Workflow | Historical Git blob | Frozen Git blob | Demonstrated delta / gate |
|---|---|---|---|
| `ide-d7-windows-worker.yml` | `518635e58bbac5b249bfeadba7e60c90b9d0010c` | `6832a33bdab3539d85805ccd43a9aa479758d08d` | Push micro-job trigger removed; original PR/manual and Windows worker assertions retained. P05 still blocks silent CI-trigger reduction |
| `ide-d7b-native-acceptance.yml` | `f70955319c621cbd08cd2a7a6f7eac77caffc3a1` | `c247635e41a4c325a76cc5223239f890898c92c1` | Added actual AppContainer Cargo-produced executable/provenance step and extra frontend source triggers, original worker gate retained |
| `ide-d7d-protected-service.yml` | `ce56f987cb056c01ef7be58318d70c9e136aafad` | `35a0a506186694caaa6ff72d8bfdeec694cf0235` | Debug-control tests now compile with explicit DAP transport; extra strict native SCM executable / error-handling steps; protected service/custody must still be independently accepted |
| `ide-supervisor-core.yml` | `e0d26f80713cdf498c3bfdf82645498dddf34e16` | `11203a4e5f8cfeecee9effa00c88c981492be527` | Push micro-job trigger removed; additional opt-in original read-only DAP journal audit, default-feature test retained |
| `ide-wo6-native-os-acceptance.yml` | `d7627899b3a6cafc7b383f00abbb2b9e7107fe2d` | `de7cae8464649481ff46f148341bd575aca045da` | Windows runner moved 2025 → 2022 for pinned Visual Studio/MSVC/SDK; added exact-head source, independent LLVM/PDB symbolizer, offline toolchain, OS worker checks; older acceptance cannot be assumed equivalent |
| `ide-wo8-installed-windows.yml` | `8fadc3f1829e62123b9da6a5e6bbd49a3f3ec36f` | `9c74218a69ce3a250bf707cf569acba4b0044734` | Added installed Windows Jobs UX/hostile denial and default-off project-debugger UIA regressions; original NSIS checks retained |
| `ide-wo8-signed-protected-e2e.yml` | `08e2ab350b9e0c9e88958a40a3eeca59a3a66df5` | `3cbaf9efc3836bfbe3a978cdd5511daac04f2380` | Added effective process-leak observation, exact LLDB candidate bundle, PE/PDB/SCM source and UI assertions; historical output wording changed. Signing and independent human release still unaccepted |

**Disposition:** Source changes are not automatically unsafe, but the immutable historical P05 audit is valid and **must remain failed** until an independently reviewed successor policy accounts for original and additional execution/security checks, exact trigger coverage, no CI silencing, and original installed evidence. Do not restore old weaker code by reflex, re-enable stacked micro-jobs just to pass a SHA check, rewrite the 38 original blob pins, or mark historical equivalence without exact-OS evidence. P05 original verifier and JSON are unchanged in this candidate.

## P06: original dependency and licensing scope remains separate

- Immutable original P06 graph: 500 Cargo packages; approved successor permits the single previously verified local desktop `rand` block without modifying the 499 other graph blocks. `ide/app/src-tauri/Cargo.lock` SHA256 `b3b8c5bf056163c2747c28473b583b996b520917eb63e9a1b4eeb05437abca5a`.
- Original 65-hex `unchangedOutsideDesktopSha256` was invalid. Genuine independently compared CI bytes: `73afb4fa1fcfc63f2e30b15ecdb06570c55804088dcfa4bfbac78e9f41240cb4`. The code and existing successor JSON now contain this corrected value, with the same original equality guards and hostile rejection tests.
- Newly staged standalone locks (source-only; generated from exact original Windows/Linux jobs with matching bytes, now tracked in unpublished tree): host Git SHA `90e52a812a834786d828fefb86d244732a882dd7` / SHA256 `00de403d7aeb63cd476f7694d52af1db1ff25a87fd2460a782ad2301edf893ef`, supervisor Git SHA `cf3ea845e89c7b8242a75d71c9dd5288b5580595` / SHA256 `1fcc77eb3790e34b5815f69df2adb3948034d194e12e4ddc904861038e1edfb`.
- Original grouped native and installed DEV-01 jobs now fail closed on untracked or changed host/supervisor/desktop lock bytes rather than generating them. Original Cargo `--locked`, original tamper tests, and P06 immutable desktop graph verification remain mandatory.
- Scoped local raw-byte/TOML/negative tamper/cross-OS rehash checks: **16 passed** against original archived bytes; **not** a native cargo build, LLVM/Tauri, OS/UIA, or release result.
- `releaseApproved`, `licenseLegalReviewApproved`, `independentlyAuthorizedMainMerge` and newly introduced lock-receipt `legalReviewed/physicalOsAccepted/mainMergeAuthorized` flags remain **false**. Upstream notice provenance, dynamic inventory/legal decision and signed release gate remain OPEN.

## Acceptance boundaries

Only after one fully reviewed candidate is published via protected non-force expected-head CAS with zero overlapping CI may a new unchanged-SHA Windows/Linux native, Rustfmt, source/security/recovery and real installed DEV-01 group be evaluated. Failures require original logs and grouped repair; the P05 original historical immutable-source denial is **not waived by a clean grouped CI pass**. Physical Windows/R9700 and installed unsigned NSIS regression, independent SCM/AppContainer/WFP, Linux delegated kernel cgroup-v2, legal/signed release and `main` merge each require their separate evidence and authorization.

## Post-publication P06 original test finding (2026-11-15)

Original Windows/Linux P06 run [#43763652480](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/43763652480) still denied `merge02_desktop_delta_unreviewed`. A second independently malformed 65-character hash existed in `newDesktopBlockSha256`, separate from the previously corrected `unchangedOutsideDesktopSha256`. Rehashing the original committed lockfile establishes `a5e4a1867eeb99c821b9e0a144aae01fe2895c49717b37756f7519f5b72594e3` for the local desktop block and the already-correct `73afb4fa1fcfc63f2e30b15ecdb06570c55804088dcfa4bfbac78e9f41240cb4` for all other blocks. A new source-only test rejects the malformed 65-character pin; original P05/P06 historical reviewers, all hostile tests, the 500-package graph and explicit `releaseApproved=false`, `licenseLegalReviewApproved=false`, `independentlyAuthorizedMainMerge=false` remain unchanged. No new CI PASS or release authorization is inferred from a corrected constant.
