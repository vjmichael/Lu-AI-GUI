# Lu IDE — P06 governance and reproducibility

**Source start:** November 7, 2026. **Branch:** ide/release-closeout-p06-20261107, stacked exclusively on P05 draft PR #91 source 49c04a7a13770207acfbf553fc79cbaafa1f1dfe. Main and Lu Core unchanged. **Draft; no release approval.**

## Evidence-based implementation

P06 builds independent exact-head Windows/Linux CI for *actual resolved* npm and Cargo graphs. It collects package-lock.json and Cargo.lock after dependency resolution on disposable GitHub-hosted OS runners, hashes the bytes, verifies the pinned Rust Tauri core + runtime + runtime-wry and JS API versions, denies unreviewed Cargo Git origins and missing npm/Cargo remote integrity. It retrieves the **six complete original upstream licenses**, source-verifies their version-pinned texts, bundles them with resolved npm/Cargo notices, and retains the source SHA-bound evidence and lockfiles as GitHub Actions artifacts for 30 days.

The CI-generated lockfiles are **not yet reviewed and committed into source**. The machine receipt deliberately says lockfilesCommittedAndReviewed=false. Resolving them dynamically is useful investigation, not a reproducible release. The next implementation step is to inspect real successful Windows/Linux resolution, reconcile any platform differences, commit both reviewed lockfiles, require --locked/npm ci in every source and native installer job, and rerun full real OS acceptance without relaxing gates.

The native acceptance matrix records the last P05 source-only pass at commit 49c04a7, run 42416663576. P02, P03, P04 native/signed installed requirements remain separate pending evidence, regardless of green P06 unit tests. P07/WO10 deferred until post-Lu v1.0.

## GitHub governance read-only check

The connected GitHub repository returned an empty **repository rulesets** list. The separate GET branch-protection endpoint for main returned **403 Resource not accessible by integration**; therefore the actual main branch protection/required checks cannot be verified from this connection. Do not claim they are disabled or apply speculative administrative changes. A maintainer must inspect protected branches/rulesets/required status checks and an independent approval policy before any release candidate may be promoted.

## Unresolved gates

- Real CI results for this P06 source head on both native OS runners; generated lockfile hashes/notice artifact review.
- Authoritative committed npm/Cargo lockfiles and cargo --locked/npm ci across all candidate builds. No unpinned Tauri runtime.
- Source-to-built-artifact-to-installed-process trust chain, OS kernel/supervisor receipts, and independent artifact provenance with reviewed immutable retention.
- Real signed installer + service identity, legal review of package-specific missing notices, verified upgrade failure injection and transactional rollback.
- Repository rulesets/branch protection and human approval verification. P05 PR and original security checks remain intact.

**Do not merge P06 ahead of P05, or merge P05/P04 ahead of native acceptance.** Signed identity intentionally deferred; no automatic release approval.
