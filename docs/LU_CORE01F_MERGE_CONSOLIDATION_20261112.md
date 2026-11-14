# CORE-01F — Lu Core merge consolidation preflight

**Status: LIVE CORE-01E CANDIDATE SOURCE REVIEW ENABLED / MERGE BLOCKED.** This draft makes no changes to the CORE-01E source head, production execution, user data, or main. It is not merge authorization. llama.cpp remains the sole production engine; IDE and KoboldCpp/P7L remain excluded.

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

## Source-only CI test admission

The original REPO-01 source verifier now runs the five new read-only
CORE-01F source-contract tests only when the new test file exists. This is
a lightweight source job, not new native job fanout; it does not alter
CORE-01E's protected branch, native thresholds, or installed-workflow
admission. A green source run remains no merge authority.

## 01F-02 source bridge review and original Git blob accounting

See `docs/LU_CORE01F_P0_BRIDGE_REVIEW_20261112.md` and
`docs/LU_CORE01F_P0_BRIDGE_BLOB_REVIEW_20261112.json`. Frozen original
R00 → P0 bridge: 257 ancestral commits, 125 changed paths, zero deletions.
All 125 paths are present at the CORE-01E reviewed snapshot `97dd1e89`;
105 exact Git blob identities, 20 subsequently revised blobs, zero excluded
IDE/KoboldCpp/P7L source. The existing REPO-01 source-only verification now
tests negative tamper, missing/extra and cross-scope cases and inspects the
original historical Git trees/diff. No extra native CI admission.

**Concurrent CORE-01E branch motion:** after that frozen source snapshot,
CORE-01E advanced by two commits to `54c1cf305fade7bfbfe0f19e181ea18d3e4af3b5`;
the actual delta only changes the REPO-01 acceptance workflow and source
lock. Those are already in the 20 downstream-changed path set. This
CORE-01F branch remains deliberately stacked on its original reviewed
CORE-01E parent and has NOT been automatically rebased. Before merge,
refresh the CORE-01E accepted head and restack/reverify CORE-01F with
an owner-reviewed safe source and main-ledger reconciliation.


## CORE-01F-03 modernization: new CORE-01E candidate without a silent security promotion

The original pinned CORE-01E SHA is retained as an **immutable reviewed ancestry
anchor**. The live branch may advance only by descendant commits. The source
preflight independently reviews the *current* candidate Git tree and its
source-tree.lock.json (including 1:1 blob identity and the lock commit's
immediate code parent). It rejects ancestry changes, excluded source paths,
unreviewed main changes, forged locks and, when supplied, an exact candidate
SHA mismatch. The bridge's own current source lock is separately verified.

For a user-reviewed **read-only** audit with all historical Git objects
available locally (ref acquisition must be independently authorized):

    node --test scripts/core01f-merge-preflight.test.mjs
    node scripts/core01f-merge-preflight.mjs --main-ref origin/main --core-ref origin/lu/core01-source-convergence-no-ide-no-kobold-20261112 --expect-core-sha f4beaba3f6e77693f9048cd7e67764142d6c41d2

Omit --expect-core-sha to inspect the currently resolved candidate without
prior endorsement; the report still includes its exact SHA and never grants
security or merge authority. In CI, pin --expect-core-sha to the independently
reviewed PR head to reject concurrent branch drift. Never pass a moving ref
as a substitute for an exact receipt's source identity.

The returned status SOURCE_INTEGRATION_REVIEWABLE_ONLY means *source-review
preparation*, not native or installed acceptance. The integration plan names
the changed CORE-01E and CORE-01F paths, including overlap and the source
lock that must be regenerated only after a separately reviewed integration.
A conflict or restack requirement is reported but never executed. Main's
live docs/LU_DEVELOPMENT_LEDGER.md must be preserved; every other new main
path is an explicit blocker until reviewed.

**Separate promotion gate:** SEC-06C original kernel and browser custody,
unchanged historical hostile fixtures, SEC-06F matching Windows/Linux/source
receipts, genuine tool/model and native regressions, installed Windows NSIS
and Debian lifecycle tests, and independent owner permission remain OPEN
unless established by original exact-head evidence. Never turn a source-only
green check into a release, merger, or one-use executor grant.

This authoring does NOT restack PR #228, mutate PR #226, merge anything into
main or admit an installed workflow. When the candidate passes its full
security gate, update the integration review's exact source SHA, reconcile
the overlapping paths with the protected main ledger, create a separately
reviewed new source-lock commit, and rerun all original gates.


## CORE-01F-03 — live main protection and historical PR #229 collision

The ledger's independently actionable CORE-01F-03 review now compares the exact Git blob identities of **both** main-owned files: `docs/LU_DEVELOPMENT_LEDGER.md` and `.github/workflows/lu-development-ledger.yml`. The source preflight requires `--expect-main-sha` alongside the explicit CORE-01E candidate SHA in its PR #228 source-only CI job. No stale ledger or scheduled workflow may be automatically preferred over the live main bytes; `requiredIntegrationSource=EXACT_CURRENT_MAIN_BLOB`, and all execution, merge, installation and release authority remain false.

At this review, `main` was `2e31b22a597c8a92aec979d9ff057401c518088c`. Its latest ledger blob was `f340ea1f8e8096e23e984c6cc7ba067cf4d795dd`, while PR #229's earlier reconciliation ledger blob was `627cb6f4a5b464a0631fd7f25cbb267eab0729f8` (STALE). Main's metadata workflow blob was `35c5de7ab6bb4e4b50db76f785064809f342396b` and matched PR #229's preserved workflow at that snapshot. This is a dated exact-object comparison; the CLI refreshes current immutable Git identities for each audit.

An owner-reviewed comparison with the historical reconciliation branch already available locally can also pass `--legacy-reconcile-ref origin/lu/core01e-main-ledger-reconcile-20261112`. The tool records any stale legacy paths in the machine-readable report. It never fetches refs, changes branches, cherry-picks a workflow, writes the active ledger, merges or rebases. The two CORE-01E/CORE-01F overlap files remain `.github/workflows/lu-repo01-full-source.yml` and `source-tree.lock.json`, requiring a separate reviewed integration and new lock after original security acceptance.
