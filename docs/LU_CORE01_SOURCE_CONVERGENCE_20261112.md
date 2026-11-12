# CORE-01 — Lu Core source convergence (IDE and KoboldCpp excluded)

Status: **started, source baseline established; integration/acceptance NOT complete**. Owner direction: finish Lu Core backlog, keep llama.cpp as the sole production inference runtime; no IDE, no KoboldCpp, no merge to main or installed release.

## Exact convergence base and demonstrated ancestry (2026-11-12)

New branch: `lu/core01-source-convergence-no-ide-no-kobold-20261112`.
Frozen initial parent: `9b14dde7a01ad36208642be7536159ccb1f42e93` (P0-WIN-CHAT PR #224). GitHub compare shows it is an exact descendant of:
- R00 Lu Core convergence PR #200, `e61d4bf998bccf51074b01c864df208c233eb2f8` (268 commits, 132 changed files to #224; no KoboldCpp-named paths in that differential);
- installed chat UX PR #205, `378ee5273201a6fa451ff511c0051d76bd1ea70c`;
- PLAN-01 PR #207 `e18d92c62baeb5d5d49d6c1744fc25bbd68bfd50`;
- PLAN-02 PR #208 `7e9fa26d20147e3a7bbc6ffa3c2fec8e7808ab83`;
- PLAN-03 PR #209 `8070b69ca98163c3d00843252d307c38b0157e0e`;
- PLAN-04 PR #210 `ea8ea8ecde2bfbb9ccb5429ac0543d9a5f2a91dd`;
- PLAN-05 PR #213 `223d35e65ad58f7aeaabac5c9d1f1d97d87c714f`;
- PLAN-06 PR #216 `67a7c8938427aff4b03854c53a5137bebe858ef5`;
- PLAN-07 PR #219 `fb336d3868a7dccb751ca93027956219e8d8147e`;
- P0-GATE PR #223 `e422d18cef10fdb1092578631b5fec692b3d7a4f`.
MEMORY-LOOP-03E PR #198 (`74dbdfe1f8457631f1992efed2c79b37629715aa`) is also an ancestor of R00; it does not require replay.

**No claim is made that these draft features all pass end-to-end integrated native or installed acceptance.** Ancestry proves inclusion in this candidate's history, not behavioral completion.

## Explicit exclusions — preserve at every CORE-01 commit

1. Do not merge, cherry-pick, copy or edit IDE-owned `ide/`, IDE workers, debugger/LSP source, IDE acceptance tests, or IDE workflows as part of this convergence. Existing historical/shared files in the base are not silently deleted.
2. Do not merge P7L branches, `p7_koboldcpp_backend.rs`, test Kobold server, any `p7l_*` files or the KoboldCpp workflow. Do not register an alternative inference provider; production llama.cpp remains authoritative.
3. Do not merge to `main`, start an installed release or change signing policy. Avoid per-commit CI fanout.
4. Preserve host approval, OS custody, ANSF integrity, fail-closed UNCERTAIN and local-model privacy.

## Remaining divergent Core candidates

- PERF-01 / P0 security and TOOLS-09 branch lineage: the P7L-1 tip is **346 commits ahead of #224**, including legitimate non-Kobold Core fixes mixed with experimental KoboldCpp additions. NEVER merge this tip as a whole. Reconcile required changes selectively with original source comparison, dependency review and scoped tests. Freeze experimental P7/P7L and optional P6 compute-node work.
- Observe & Learn PR #61 head `9b6a886caddaa221e13933446fe0100afe77cf90` is **diverged** from #224 (15 ahead, 1690 behind, merge base `d191c56ecf96c2a2e32de47665852e46a3553b61`). Compare individual remaining source changes; never merge the old full branch wholesale. Maintain OFF-by-default capture and explicit operator consent.
- Core source identity and model-to-tool integration on #224 are present in branch history, but current installed failures in issue #204 remain unresolved without actual run evidence.

## CORE-01 next implementation gates

A. Run a source/ownership allowlist comparison against the chosen parent, identify only Core-specific missing changes, record exact file IDs and dependencies.
B. Integrate indispensable **non-Kobold** Core changes in coherent file groups: P0-SEC, TOOL-09 recursive search, PERF-01 P0–P4 (llama.cpp path only), and Observe & Learn required v1.0 components when genuinely missing. Do not add experimental backend source or IDE artifacts.
C. Rebuild production normal chat: real tool discovery/authorization, browsed evidence, MMProj and time/path tools; compare source with issue #204's actual failures.
D. Verify ANSF task/provenance and PLAN integration, supervisor recovery and GUI state on the same source tree. Preserve source manifest/lock; source-only checks are not native acceptance.
E. After coherent implementation, run **one** grouped Windows/Linux native and installed acceptance and repair original failures. Physical AMD Vulkan / real Qwen vision remain separate acceptance.

## Gate definitions
- **INHERITED_SOURCE:** present in ancestry, NOT automatically accepted.
- **NEEDS_INTEGRATION:** missing or divergent, verified by file/trait and route-level diff.
- **NEEDS_NATIVE:** source integrated, native OS compiler/fixture result absent.
- **NEEDS_INSTALLED:** actual installed workflow/owner observation absent.
- **ACCEPTED:** exact source and appropriate independently observed original runtime evidence.

This branch exists for convergence only; it does not close CORE-01 yet.
