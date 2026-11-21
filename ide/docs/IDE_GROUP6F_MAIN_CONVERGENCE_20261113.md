# IDE Git 6C–6F: convergence into main (integration candidate)

**Date:** 2026-11-13 (America/Phoenix)  
**Branch:** `ide/integration-git-6c6d6e6f-to-main-20261113`  
**Status:** DRAFT, no merge or release acceptance.

## Consolidation scope and source-of-truth

The integration candidate is based on FIN-03/Group 6F, whose ancestry includes the original Group 6C-03A, 6C-04, 6C-03B, 6D and 6E Git work. The original operation journals, one-use OS grants, source checks, and recovery policy remain authoritative; neither CI, a digest, a UI message, a stale local tracking ref, nor a read-only GitHub API response grants a Git or remote mutation.

The original Group 6F native Windows/Linux workflow is `.github/workflows/ide-fin03-6e05-native.yml` (draft PR #230). The additional **integration** workflow `.github/workflows/ide-group6f-integrated-native.yml` runs the original 6D/6E/6F native domains, the earlier 6E pull-request read-only UI tests, and actual Tauri desktop compilation at the **same exact integration source SHA** on Windows 2022 and Ubuntu 24.04. It does not replace the original Group 6D installed, hostile interruption or physical operator tests.

## Original failure receipts and fixes

- Group 6F original native run #43560518279: Linux job `132342931380` FAILED compiler E0618 because its remote-consent fixture shadowed the `observed()` function with a local variable. Windows was CANCELLED by a superseding source run; it is never counted as a pass. Pure fixture-name repair: `f656b166536c51b81fbfaaf15bb3b3d5bc1a3c31`.
- Replacement original run #43560834931: Windows job `132343928479` compiled but FAILED original Group 6F remote-restart matrix (11 passed, 1 failed) with `FIN03_D_REMOTE_RECONCILIATION_UNVERIFIED`. The test incorrectly built remote source evidence after introducing a local branch lock; the original source-fingerprint guard was correctly refusing it.
- Source-fingerprint fixture repair: `385464f1533b0e0b56973970eb774e57238cdde9` on original PR #230 and equivalent logic at `b524aa463ad570b51f7eab0de54485c9f2ddbd7a` on this integration branch. The revised matrix independently tests the earlier genuine remote view with a new local lock and newly changed source evidence (must be refused).
- NO original Windows/Linux PASS is claimed until the exact-head full jobs and their original logs pass.

## Cross-PR CI gaps visible from repository review

1. **6D (PR #221):** its original consolidated workflow is a branch-marker trigger. Later FIN-03/6F source commits do not automatically rerun it, and its installed acceptance has separate gates.
2. **6E (PR #222):** original PR states no per-commit dedicated CI. The integration workflow explicitly runs native `github_pull_review` and the original `g06e01_github_pull_review.test.mjs`.
3. **FIN-04/C4 (PR #234):** original Windows `ide-fin04w2-c4-desktop-source` run #43540994252 FAILED with E0432, E0382, E0624, E0283 compiler errors. Its workflow and source are on a diverged FIN-04 branch and cannot be represented as passing Git 6F acceptance. PR #231 also reports unmergeable at inspected head.
4. **CORE-01F (#228/#229):** head-SHA PR-triggered runs were not returned by the linked query. This is NOT proof that no separately dispatched runs exist; CORE reconciliation is an independent dependency and not automatically closed by IDE Git CI.
5. Historical head-specific failures in inherited IDE workflows (including P03 and installed editor) cannot be explained away by a focused Group 6F native pass.

## Main convergence prerequisites

The candidate is **not** equivalent to `main`. The original Group 6F branch comparison with `main` showed roughly 1,865 candidate-side commits and seven main-only commits. The seven main-only commits modify `docs/LU_DEVELOPMENT_LEDGER.md`; blindly copying either version would erase valid entries. Perform a reviewed, history-preserving ledger merge on the final integration head, not a force-reset, squash that discards receipts, or unreviewed replacement.

The current FIN-04W2-C4 branch shares FIN-01D as merge base with FIN-03/6F and has independent debugger, SCM and Tauri edits. It must be integrated by a **real conflict-resolving merge**, not a synthetic multi-parent commit with one tree substituted. Preserve W2-A/B kernel and C2 six-receipt source evidence. FIN-04 LLDB/W2-D remain disabled until their own original Windows native, physical owner consent, negative process-custody, restart and installed acceptance.

## Release and integration gate

Merge to `main` only after:
- Original Group 6F nine Git domain and three remote uncertainty outcomes PASS on both OSes at the identical source SHA.
- Integrated Group 6D/6E/6F native, UI and Tauri matrix PASS at its own identical Windows/Linux source SHA.
- The original 6D installed source/runtime matrix and independent original 6E GitHub read-only tests are reconciled without gaps.
- FIN-04 conflicts and current C4 compiler errors are resolved and validated independently, or the unfinished debugger work is kept disabled and explicitly excluded from a reviewed partial main merge.
- Main-only ledger commits are preserved in an actual merge and all original CI failures/skip/cancellations affecting acceptance are reviewed.
- One-use operator authorization, remote live credentials/CAS, production install and release gates retain their independent DENIED/UNVERIFIED status until genuine original evidence exists.

No automatic CI result provides real human OS consent. This file is an integration register, not a merge approval.
