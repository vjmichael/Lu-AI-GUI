# CP-12B-R7-R2 — Exact-source Windows CI publication and installation gate

**Status (20 Nov 2026): SOURCE-LOCKED HANDOFF PREPARED. Remote Git publication, GitHub Actions execution, installed Windows two-model GUI, and physical Windows GPU acceptance remain OPEN.** This is not a release.

## Verified prerequisites and obstruction

- R7-R1 baseline: original local Git commit `65984038434426ea21b263751f99a6c5ad15aaa0`, Git tree `840a9bd520c81a4b8e9e092f360c36eff3b88fef`; tracked source integrity 1,259/1,259 and four focused contract test files 14/14 in R7-R1.
- This execution environment cannot resolve `github.com` for Git transport. The connected GitHub service can create new Git objects and manipulate branches, but does not import an original Git pack with its author/committer metadata and exact commit history. **Reconstructing a different commit chain or replacing a remote branch is not authorized.** No R7 branch/PR was present on GitHub at last inspection; the default branch had advanced independently.
- The initial R7-R1 workflow used only `workflow_dispatch`; GitHub requires that workflow file to be present on the default branch before it can be manually dispatched. To avoid changing `main` solely to start a test, R7-R2 adds a branch/path-scoped `push` trigger, with both expensive jobs gated to **one exact R7-R2 commit subject**. It retains explicit `workflow_dispatch` for possible later controlled use. No push to unrelated branches, no scheduled CI, no PR-triggered CI.
- The two Windows CI jobs verify `git rev-parse HEAD == GITHUB_SHA`, the original frozen source inventory, actual Windows named-pipe protocol v1/v2/v3 and default-off tests, and optional feature-enabled Windows desktop source check. The workflow does not fetch GGUF weights or claim installed/physical Windows acceptance.

## Publication handoff

A self-contained *full* Git bundle, its SHA-256 manifest, and `scripts/cp12br7r2-publish-exact-source.ps1` are the transport package for a Git-authenticated machine or ChatGPT Work desktop workflow. The publisher defaults to read-only verification. `-Publish` is the separate deliberate publishing action; it refuses an already occupied branch unless its SHA exactly matches, never force-pushes, verifies original commit and tree from a fresh checkout and performs remote SHA readback. It uses the normal local Git credential manager, not embedded keys.

The expected commit/tree/bundle digests are recorded in the adjacent publication manifest **after** this source commit is sealed. Never infer the SHA from a branch name or the old R7-R1 artifact. Before publishing, independently compare the new manifest and bundle against the preserved R7-R1 source history and source lock. In particular, don't use the GitHub connector to create a nominally similar but SHA-different commit.

After exact publication, record the actual GitHub Actions run ID, matching head SHA, terminal job outcomes and logs. The one first-push event is **eligible** to start a hosted run; its existence and success are not established until observed. If the run is missing, stop and determine why rather than manually dispatching a stale revision. Do not queue overlapping runs.

## Remaining Windows acceptance

Hosted native Windows transport tests and a desktop **source check** are not an installed Tauri/NSIS executable. The physical acceptance runner `scripts/cp12br7r1-windows-installed-acceptance.ps1` separately requires the real installed GUI executable and GGUF identity, two independent Lu/model workers, user consent, stage-review findings, cancellation, revocation/replay, process death/restart and another-account denial. Each manual observation remains non-authoritative until corroborated with real kernel/process evidence. The current local environment has neither access to the user's physical Windows desktop nor a fully built installed Windows GUI with approved inference resources. Windows stage collaboration must remain disabled by default until these gates pass.

## Open sequence

1. Publish the preserved original Git commit chain to **only** `lu/cp12b-r7-windows-stage-eval`; verify remote exact SHA and tree, without force or branch replacement.
2. Reconcile exactly one hosted Windows workflow run against that SHA, including both jobs and all terminal failures/skips.
3. Repair any actual hosted failures on a new source-locked candidate and repeat *only after* previous jobs have completed.
4. Perform actual installed Windows + physical Windows two-model security/lifecycle qualification independently, preserving owner consent. Keep larger-model positive fault-localization accuracy and overall Core regression acceptance separate.
