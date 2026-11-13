# CORE-01F — Lu Core merge consolidation preflight

**Status: SOURCE TOPOLOGY PREPARED / MERGE BLOCKED.** This draft makes no changes to the CORE-01E source head, production execution, user data, or main. It is not merge authorization. llama.cpp remains the sole production engine; IDE and KoboldCpp/P7L remain excluded.

## Frozen upstream source chain (read-only audit)

| Stage | Exact reviewed SHA | PR/relationship |
| --- | --- | --- |
| main at audit | 16ead199037215fdaf1b049b1d06edadac09b81c | target; recheck before integration |
| R00 Core | e61d4bf998bccf51074b01c864df208c233eb2f8 | draft #200 against main |
| P0 codebase reconciliation | cc312e281b5989a0a9fb972eb04b9e34215a77da | explicit 257-commit, 125-file bridge after R00 |
| P0-GATE | e422d18cef10fdb1092578631b5fec692b3d7a4f | draft #223 against bridge |
| P0-WIN-CHAT | 9b14dde7a01ad36208642be7536159ccb1f42e93 | draft #224 against #223 |
| CORE-01E | 97dd1e897c4cf73a1fd8d8bf36f1233b081322c0 | draft #226 against #224 |

GitHub confirmed the sequential ancestor comparisons. Draft PRs #200, #223, #224 and #226 currently report mergeable *against their own base branches*; this is not proof of an unconditional merge to main. The P0 codebase reconciliation bridge lacks its own direct parent PR in this newer #223/#224/#226 stack and needs explicit reviewed disposition before any main integration.

At this audit main had four commits after the Core common ancestor, all developer-ledger refreshes. The main-only modified path was docs/LU_DEVELOPMENT_LEDGER.md. Main-to-Core history was diverged (2,793 Core-side and 4 main-side commits). Treat that as a snapshot: refresh immediately before integration. Never overwrite the live development ledger with an old source snapshot.

## CORE-01F engineering tasks and promotion conditions

- **01F-01 topology audit:** verify live PR/branch heads, strict ancestry, source-tree.lock.json, and negative scope for IDE/KoboldCpp/P7L.
- **01F-02 bridge reconciliation:** review 257 ancestor commits and their 125 changed paths, account for historical divergent Core PR dispositions, preserve approved R00 decisions. Do not blindly cherry-pick.
- **01F-03 main conflict review:** preserve current main ledger and accepted release metadata, review any newly changed main path, and handle a non-fast-forward merge explicitly.
- **01F-04 acceptance gate:** require ONE final identical CORE-01E commit proving original Windows/Linux compiler and full native regressions, real OS-custody cancellation and restart receipts, genuine hosted NSIS/UIA + Debian/AT-SPI installation/restart/uninstall, original log reconciliation. A skipped job or source-only green job is OPEN.
- **01F-05 safe integration:** following explicit owner authorization only, integrate in dependency order: R00/#200, reviewed bridge, #223, #224, #226. Recheck each new head, source lock, base and OS evidence after any actual merge. No release from CI alone.
- **01F-06 independent release blockers:** physical Radeon/Qwen3.8/Vulkan, owner-interactive acceptance, and reviewed browser packaging (currently not-packaged) must not be inferred from hosted CPU checks.

Prior full CORE-01E native run #43338530675 failed both OSes at a stale interrupted-task checkpoint fixture, after the initial compiler/chat/hosted Qwen/SmolVLM stages passed. CORE-01E head 97dd1e89 corrects the fixture without restoring old authorization. Its follow-up exact-head run #43342512185 had passed source verification; native/installed outcomes must be inspected live. CORE-01F does not modify this acceptance branch.

## Read-only source preflight

With the required full Git objects already fetched locally, run:

    node --test scripts/core01f-merge-preflight.test.mjs
    node scripts/core01f-merge-preflight.mjs --main-ref origin/main --core-ref origin/lu/core01-source-convergence-no-ide-no-kobold-20261112

The script never fetches, rebases, pushes, creates a merge, or interprets old CI as acceptance. It reports main-only changes, the exact frozen ancestry, excluded source paths, current CORE-01E head drift, and full current source-lock verification. A source-only PASS still outputs mergeAuthorized=false and releaseAuthorized=false. If the upstream Core tip moves, freeze a new reviewed chain; do not silently bless the new source.

**Outcome:** Merge preflight authored. Execution and merge authority remain blocked on accepted CORE-01E OS/installed receipts, reviewed bridge, current main collision check, and explicit integration approval.
