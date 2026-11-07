# P01 — IDE Source Integration and Stacked PR Audit

**Outcome:** Source consolidation **COMPLETE** as an isolated draft candidate; **release/native acceptance NOT COMPLETE**.  
**Candidate:** [PR #78](https://github.com/vjmichael/Lu-AI-GUI/pull/78), `ide/release-closeout-p01-20261107`, based on the current `main` at `88ab1f10e1b1a4d6c6187bfb5ad7c5d8a1606e39`.  
**Integration snapshot commit:** `5b1021e34133181c73a4aad421dfebe2abad8315` (one parent: `main`, not a merge of the older draft PRs).  
**IDE source:** [WO9 PR #75](https://github.com/vjmichael/Lu-AI-GUI/pull/75) @ `0d271606c8bcaeabe799cb970e0b6d4aefd25089` plus initial phase ledger, state, changelog and README prepared at `9823bb58d786fbe26ffdb3d260300d20e80fec20`.  
**Tracking:** [P01–P07 ledger](IDE_RELEASE_CLOSEOUT_P01_P07_20261107.md).

## Reconciliation proof

| Invariant | Git object / evidence | Status |
| --- | --- | --- |
| `main` base unchanged | `88ab1f10e1b1a4d6c6187bfb5ad7c5d8a1606e39` | **PASS** — untouched |
| Lu Core `src-tauri` tree | `41312f8ec383eb9eb8fa4d5a2e4ec64163ffab73` in both source trees | **PASS** — identical |
| Lu Desktop `lu-desktop` tree | `633e0bbe4de4b9f43a06f5495fea924fc81445f8` in both | **PASS** — identical |
| Existing Lu documentation, root scripts, license, README, AGENTS, changelog | Main trees/blobs unchanged | **PASS** — no IDE overwrite |
| `.github/pull_request_template.md` | Preserved main blob | **PASS** |
| `.github/workflows/lu-development-ledger.yml` | Preserved `main`; total workflows now 33 (1 Lu + 32 IDE) | **PASS** |
| Last nondeferred IDE source | `ide/` 221 committed files from preparation tree `81f0297d45ca864e2163a845313ebe4b7a781324` | **PASS** — source snapshot only |
| Only changed root paths vs `main` | `ide/` and `.github/`; compare showed 253 files (221 IDE + 32 workflows) | **PASS** |
| Post-v1.0 WO10 prototype excluded | `ide/proto/lu/ide/v1/bridge.proto`, `ide/bridge-contract`, `ide/host/src/bridge_policy.rs` absent; no Lu Desktop changes | **PASS** |
| Source branch release approved / installed OS accepted | WO7/8/9 manifests default deny; exact-head CI red | **NO** — assigned to P02–P04 |

Snapshot import was intentionally **not** a two-parent merge: early PRs remain unmerged and their unique source remains reviewable; `main` can review a coherent IDE-only diff without replacing newer Lu Core/desktop files. Nothing has been released or promoted.

## Dependency audit — 26 open IDE PRs at source review

All PRs remain open/draft. **Ancestry is not proof that a feature is installed or accepted.** This table audits the relationship between each PR's latest branch and the **WO9 source head**, before importing the WO9 source snapshot.

| Classification | PRs | Disposition / evidence |
| --- | --- | --- |
| **Parallel source NOT entirely incorporated — review before product feature claim** | [#43](https://github.com/vjmichael/Lu-AI-GUI/pull/43), [#45](https://github.com/vjmichael/Lu-AI-GUI/pull/45), [#46](https://github.com/vjmichael/Lu-AI-GUI/pull/46) | 30/35/32 branch-unique commits relative to WO9. Buffer identity helper, its installed fixture, and project search host/tests missing from WO9; **do not close or mark shipped**. Retain until a reviewed port-or-defer decision. |
| **Ancestral in WO9 source history** | [#48](https://github.com/vjmichael/Lu-AI-GUI/pull/48), [#49](https://github.com/vjmichael/Lu-AI-GUI/pull/49), [#50](https://github.com/vjmichael/Lu-AI-GUI/pull/50), [#51](https://github.com/vjmichael/Lu-AI-GUI/pull/51), [#52](https://github.com/vjmichael/Lu-AI-GUI/pull/52), [#54](https://github.com/vjmichael/Lu-AI-GUI/pull/54), [#55](https://github.com/vjmichael/Lu-AI-GUI/pull/55), [#60](https://github.com/vjmichael/Lu-AI-GUI/pull/60), [#62](https://github.com/vjmichael/Lu-AI-GUI/pull/62), [#63](https://github.com/vjmichael/Lu-AI-GUI/pull/63), [#64](https://github.com/vjmichael/Lu-AI-GUI/pull/64), [#65](https://github.com/vjmichael/Lu-AI-GUI/pull/65), [#68](https://github.com/vjmichael/Lu-AI-GUI/pull/68), [#69](https://github.com/vjmichael/Lu-AI-GUI/pull/69) | All have zero unique commits beyond WO9 source in GitHub ahead/behind comparisons. Keep draft PRs until P05 review and exact installed verification. |
| **Diverged late fixes / historical doc or fixture deltas — verify before PR closure** | [#47](https://github.com/vjmichael/Lu-AI-GUI/pull/47), [#53](https://github.com/vjmichael/Lu-AI-GUI/pull/53), [#56](https://github.com/vjmichael/Lu-AI-GUI/pull/56), [#58](https://github.com/vjmichael/Lu-AI-GUI/pull/58), [#70](https://github.com/vjmichael/Lu-AI-GUI/pull/70), [#73](https://github.com/vjmichael/Lu-AI-GUI/pull/73), [#74](https://github.com/vjmichael/Lu-AI-GUI/pull/74) | 1–4 unique commits each. C3 test-regex and supervisor formatting, Windows probe diagnostics, detailed WO6 CI notes, WO7 signed-test fixture and WO8 Openbox/UI selector refinements. Latest WO7 test-file and WO8 workflow blobs match WO9; Openbox/windowfocus and Windows executable basename repair are present in WO9, but full equivalence still requires P05 review. |
| **Source snapshot** | [#75](https://github.com/vjmichael/Lu-AI-GUI/pull/75) | WO9 last nondeferred source used for draft candidate; no claim that WO9 installed acceptance passed. |
| **DEFERRED / NEVER IMPORT** | [#76](https://github.com/vjmichael/Lu-AI-GUI/pull/76) | Optional Lu Core/DLC interface postponed until after Lu Core v1.0 and explicit authorization. Not included in candidate. |

### Distinct unported/source-review exceptions (retained, not silently dropped)

- **E01 / [PR #43](https://github.com/vjmichael/Lu-AI-GUI/pull/43):** `ide/app/web/buffer_identity.js`, corresponding tests and `ide/docs/IDE_BUFFER_REFERENCES.md` are missing. The current CodeMirror editor does not expose the old `bufferId` API. **Decision pending:** review safe migration into later editor, or explicitly defer feature; keep PR open.
- **E02 / [PR #45](https://github.com/vjmichael/Lu-AI-GUI/pull/45):** independent installed buffer acceptance script/config and CI not imported. Reevaluate when P02 installed UI harness is functioning; do not claim this test passed on new source.
- **E03 / [PR #46](https://github.com/vjmichael/Lu-AI-GUI/pull/46):** `ide/host/src/project_search.rs`, `ide/host/tests/project_search.rs` absent; the new host has no `ide_find_in_project` command. **Actual feature omission**, not just an old PR workflow. Preserve until product/release scope decision.
- **E04 / [PR #70](https://github.com/vjmichael/Lu-AI-GUI/pull/70):** latest WO6 PR adds detailed Linux Cargo kernel CI and debugger checkpoint notes beyond the snapshot WO9 version of the work-order document. Preserve PR and its evidence; source baseline unit/OS evidence still independently required.
- **E05 / other late diverged PRs:** do not discard D3/D6/D7, C3, WO7 and WO8 branch changes before comparing patch/CI evidence with the candidate under P05.

The P01 decision is **preserve all exceptions as reviewable open PRs**, not merge uncertain code or retrofit extra features into the candidate without P02 native tests. P05 must resolve PR disposition. If the user requires E01/E03 in the first standalone release, schedule their feature ports as separately authorized source work before release acceptance.

## GitHub operations and handoff

- [x] Create candidate branch on current `main`, with a one-parent source snapshot.
- [x] Preserve original Lu assets and all 33 distinct workflows.
- [x] Open [draft PR #78](https://github.com/vjmichael/Lu-AI-GUI/pull/78) targeting `main`; never auto-merge.
- [x] Exclude WO10 and leave original PRs untouched.
- [x] Source identity and missing-branch inventory recorded.
- [ ] **P02–P04**, not P01: repair red CI and obtain exact-head Windows/Linux installed security/GUI receipts.
- [ ] **P05**, not P01: decide E01–E05 feature and PR closure; assess 32 IDE workflow duplicates.
- [ ] **P06**, not P01: protect branch with reviewed required checks and reproducibility policy.

**P01 closeout:** One authoritative **draft source-integration** branch exists. P01 is complete for branch/source isolation and dependency review; it is **not** an approved release candidate binary. **Stop here; next work phase P02 requires separate user instruction.**
