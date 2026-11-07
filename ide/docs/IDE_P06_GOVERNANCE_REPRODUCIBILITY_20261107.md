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

## Dual OS lock parity evidence — P06 first native run

P06 exact-head Windows 2022 and Ubuntu 24.04 [run 42417459140](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42417459140) **PASSED on both OSs** at commit `7310eb1465dfe26fc0aee6b9b450113d1463ad69`. Archived artifacts contain the full 500-entry Cargo.lock and 62 npm resolution package entries, plus all upstream license text and 48 unresolved package-specific legal-text entries. Comparison proved **Cargo.lock byte equality** (`d4312b60209f89181766b60c2982fb8b199e8ed9d6be1a9dcbdcb3dd07abf6b4`) and **npm JSON semantic equality**; npm raw bytes differ only due Windows CRLF vs Linux LF. The canonical sorted compact npm JSON SHA-256 is `9a61ac00a82c8f8ca57ce6e56b2e9eb355fd66a0d9af6081b199ddf0573e97de`.

A reviewed-graph SHA witness and adversarial parity checks now require exact per-OS npm lock bytes + shared semantic JSON, shared Cargo bytes, upstream full notice receipts and no legal approval. CI also runs an actual `npm ci --ignore-scripts` and `cargo fetch --locked` after initial resolution and checks the graph again. This is a **reviewed hash pin**, NOT yet committed package-lock.json/Cargo.lock source files, and not a reproducible installed native binary. An independent immutable artifact and lockfile commit check is still required.

## License source remediation and review queues

Inspecting the actual previous two-platform archived dependency inventories found **48 unresolved** records per OS, including two first-party Apache-2.0 Lu IDE crates and one platform-specific `@esbuild/*-x64` MIT binary package. Source now vendors the repository's exact Apache license text into each local Rust crate source directory; the notice collector may reuse the **already fetched and SHA-verified esbuild v0.25.9 MIT text** only for that exact-version binary package with an explicit MIT declaration, copying those full bytes into the installed verification package. All other missing package texts remain unresolved. The expected change in counts requires the next actual Windows/Linux build; no legal completion is inferred.

Added `p06_license_triage.py` to hash the real notice inventory and produce an immutable-at-source-SHA **open review queue** with every outstanding entry, declared license, notice count and suspected target family. Platform names are triage hints only: no package is excluded from commercial-distribution requirements without real binary scope and independent review. CLI and tests reject a fake release/legal approval or an unexplained missing record. Source review is not an attorney's legal review.

## Hash-pinned locked dependency reruns in every native verification builder

All six unsigned native package-building paths (the two E2 Windows NSIS, WO8 Windows NSIS and Linux Debian, WO9 Windows NSIS and Linux Debian) now require actual platform-specific raw npm lock hashes, the common byte-exact Cargo lock SHA and a common npm semantic graph SHA **before and after** `npm ci --ignore-scripts` plus `cargo fetch --locked`. Any new dependency revision, Cargo transitive runtime drift, npm line-ending anomaly or missing lock fails the native verification build; no source PR or release gate is bypassed. This is only verified generated-lock reproducibility until the original lockfile bytes are committed as source. A dedicated verifier has negative tests; exact-head native OS runs still required.

## Declared MPL-2.0 and alternative-license obligations (commercial readiness)

The 521-package previous native OS inventory identified **five MPL-2.0 crates** (`cssparser`, `cssparser-macros`, `dtoa-short`, `option-ext`, `selectors`) with possible covered-file source/notice obligations and **two `r-efi` versions** declaring `MIT OR Apache-2.0 OR LGPL-2.1-or-later`. MPL is not a ban on commercial software; an `OR` license expression does not force LGPL. An independent review must establish exact distribution scope, changes, notices and any chosen permissive licensing alternative. P06 now detects all declared copyleft or alternative-copyleft options, makes each an open review case, and compares the discovered list to a SHA-reviewed seven-item reference. A changed/new or silently disappearing entry DENIES source review rather than disappearing from the commercial license ledger. Neither permission nor legal signoff is automatically issued.

## Canonical upstream source revision consistency

Corrected three stale sourceRevision strings in the canonical notice inventory (Tauri Apache, CodeMirror and esbuild). The resolved package notice bundle now records the same exact upstream tag as the HTTPS fetch receipt, and the verifier denies altered/mismatched source revisions and source URLs even if the notice bytes and hashes are intact. Exact source audit regression runs in P06 CI. Full package-specific copyright review remains outstanding.

## Authoritative committed lockfile closeout (November 7)

Both original exact CI-reviewed lock byte streams are now Git blobs in this branch: `ide/app/src-tauri/Cargo.lock` = git SHA1 `87d0045a4be1c78651f080300aba71c14dbe1f30` and SHA-256 `d4312b60209f89181766b60c2982fb8b199e8ed9d6be1a9dcbdcb3dd07abf6b4` (500 packages); `ide/app/package-lock.json` = git SHA1 `dfe4412bfacfc22ad1149f5213f4a26aa49b4067` and SHA-256 `3c137aede8749007c062b80e53bef0c83241b73d21e4c692ae1718882e81a316` (62 npm packages). These were independently hash-checked against P06 native Windows/Linux CI run `42417459140` and a repeat real Windows exact-head run `42420310596`. `.gitattributes` requires LF on Windows and Linux, removing raw npm CRLF drift. P06 CI now checks the exact blob identities in `HEAD` and uses `npm ci` and `cargo fetch --locked`, plus pre/post hashed source-graph parity. The temporary log exporter is removed. This **closes the missing-committed-lockfiles source task**. It does not establish identical signed installers, native desktop UI acceptance or legal approval; these remain open.
