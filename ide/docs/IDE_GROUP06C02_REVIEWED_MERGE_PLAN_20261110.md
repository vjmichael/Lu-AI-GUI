# IDE 6C-02 — Read-only reviewed local merge planning

**Branch:** `ide/group06c02-reviewed-merge-plan-20261110`, stacked on 6C-01 `3234aeb514c81c37f7f4098ed6e654915a8cf65c`.

## Scope

This task provides **planning only**. The user selects an existing local branch in the native Source Control screen, explicitly inspects the 6C-01 operation review, and requests 6C-02's separately generated merge preview. **No Git ref, index, worktree, stash or other Git object is created or modified. No OS approval is requested and no execution route exists.**

The host `git_merge_plan::preview_reviewed_local_merge` enforces the OS-picker registered project and exact ordinary `.git` directory guard reused from 6C-01. Remote-tracking branches, raw OIDs, linked worktrees, detached HEAD, arbitrary command arguments and unregistered paths are not admitted as requested branch selectors.

## Evidence model

- Records exact current branch, current HEAD OID, selected local branch, selected commit OID, both committed tree OIDs, merge base commit/tree (if unique), ahead/behind, source index SHA-256, 6C-01 review digest, conservative blockers, and a domain-separated **6C-02 evidence digest**.
- Uses native libgit2 **read-only** ancestry `merge_bases` and tree metadata walkers; no `merge_trees`, `merge_commits`, merge driver, checkout, ref CAS, hook, Git CLI, process spawn, GitHub request, or artificial conflict-resolution worktree. Both the initial and final native source fences independently recheck current/selected refs, saved index bytes, Git state, conflicts, and full local worktree dirty count; a mismatch denies the preview.
- Scans entire committed base/current/selected tree paths with explicit 20k-per-tree entry cap and 1024-byte relative path maximum. The complete path ledger contributes to the digest, even though only the first 80 paths are shown in the UI. Unsafe/oversized/unverifiable trees cause denial, never a silently truncated claim of a clean merge.
- Distinguishes **fast-forward candidate** (`base == HEAD`), **already contains selected** (`base == selected`), **divergent merge candidate**, **same commit no-op**, and **unverifiable ancestry** (missing or multiple bases). These labels are observational, not approvals or commitments about content mergeability.
- Computes a conservative three-way path-level overlap: a path modified differently on both sides is a **possible conflict** (including cases where Git might merge disjoint hunks cleanly). Unsupported file modes, symlinks, gitlinks and directory/file changes require additional review. No rename-aware analysis or complete three-way content merge is asserted. A divergent merge with no overlap still emits `divergent_result_tree_not_verified`; the merged output tree is not yet constructed or verified.
- A dirty worktree/index, foreign `.git/index.lock`, pre-existing conflicts, shallow history, unsupported branch or active Git state is independently marked as a blocker. The native preview never removes any lock or modifies unrelated files.

Every review response explicitly states **`sourceRechecked=true`**, **`threeWayContentMergePerformed=false`**, **`renameAnalysisPerformed=false`**, **`authorizationGranted=false`**, **`branchRefUpdated=false`**, **`indexOrWorktreeModified=false`**, **`hooksExecuted=false`**, **`remoteQueried=false`**, **`processStarted=false`**, and **`executionRouteAvailable=false`**.

The frontend validates the project ID, exact source-selected branch/HEAD OIDs, matching 6C-01 review digest, schema, source fence status and negative authority flags. Project changes, refresh and selector changes invalidate cached plans and discard stale replies.

## Independent acceptance

`.github/workflows/ide-group06c02-native.yml` checks the exact branch head on real Windows 2022 and Ubuntu 24.04 runners, with libgit2 fixture tests for fast-forward, independent divergent changes, overlapping same-path edits, index/worktree dirtiness, unowned Git locks, stale evidence fingerprints and forged branch selectors. It runs strict Rust formatting, desktop UI source-boundary tests, a real frontend bundle and Tauri compilation.

**Do not record tests or CI as passing until the exact-head job results have been inspected.** This is source/native acceptance, not physically operated NSIS/Debian consent. Human acceptance remains deferred per Group 6B issue #212. Group 6C-03 must independently design a sealed prospective result, one-use human OS authorization, strict preconditions, cancellation, durable journal, CAS and restart reconciliation. Do not convert a 6C-02 digest into an execution grant.

## Next work

**6C-03** — Protected merge execution and recovery. Any live operation must be a separate native capability with explicit current-source readback and OS consent; it must not infer authorization from this read-only plan.
