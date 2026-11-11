# IDE Group 6C-04 — Rebase and Cherry-pick

Status: **6C-04A implemented; native Windows/Linux acceptance pending**. Work branch: `ide/group06c04-rebase-cherrypick-20261110` stacked on Group 6C-03A exact native-pass `afbbc96122b72e4d2695861c307c753b189b0ec0` ([draft PR #215](https://github.com/vjmichael/Lu-AI-GUI/pull/215)). Group 6C-03B divergent-merge executor remains open and separate; physical operator approval and installed-app acceptance are deferred, **not accepted**.

## 6C-04 task sequence

**6C-04A — Original replay review (this implementation).** For rebase, select an existing local branch and inspect up to 16 original first-parent commits that are unique to current HEAD since the unique merge base, in oldest-first order. Reject merge commits, roots, shallow/unrelated histories and oversized/unverifiable graph ranges; preserve every original parent, committed tree and complete bounded changed-path ledger. For cherry-pick, inspect **only the selected existing local branch tip** and its single parent; arbitrary OIDs and multi-parent cherry-picks are not accepted by this interface. Record exact original HEAD/selected refs and trees, merge-base tree, independent 6C-01 operation review and 6C-02 merge review SHA-256 values, source index SHA-256 and whole immutable commit ledger fingerprint. Report overlapping paths conservatively; a lack of overlap is **not proof of a clean replay**. No object tree is actually replayed or created.

**6C-04B — Non-mutating prospective results and conflicts.** Independently compute the per-commit post-replay tree in quarantine (not the user's Git repository/index/worktree), with rename, file mode and multi-commit conflict resolution gates. Make results reproducible from original objects, do not copy executable hooks, and refuse ambiguous or unsupported history transformations. Preserve original objects.

**6C-04C — One-use protected cherry-pick.** Separate native OS approval for exactly the selected original commit, its original parent and a verified new tree. Cancel before publication; one-time ref CAS, index/worktree custody, durable signed-off receipts, independent byte verification and process-abort restart reconciliation. A read-only digest **never** authorizes execution.

**6C-04D — One-use protected rebase/rewrite.** Additional explicit history-rewrite consent distinct from cherry-pick approval. Preserve original commit identities in the journal, never force-push, never discard original objects or silently run Git hooks. Exact original-ref CAS and independently reviewed branch/index/worktree output.

**6C-04E — Installed and physical native acceptance.** One-use consent, refusal, cancellation, stale source, intentionally interrupted ref/index updates, restart recovery, accessibility, project switching, independent Git history, and Windows NSIS/Linux Debian install/uninstall preservation. **Human/physical tasks deferred** per user; leave gates open.

## 6C-04A exact implementation

`ide/host/src/git_rebase_cherry_plan.rs` performs read-only native `libgit2` original commit/diff inspection inside the exact registered project. It requires local, existing branch names and a unique merge-base; no remote network connection, mutable Git index, checkout, branch ref update, commit, subprocess, hook or signed credential. The original index SHA and both source review digests are re-read at the end; changed sources deny the claim. Results explicitly state `executionRouteAvailable=false`, `oneUseOsApprovalGranted=false`, `resultingTreeVerified=false`, `conflictResolutionPerformed=false`, `originalCommitsRetained=true`, `branchRefUpdated=false`. A rebase is marked as a potential history rewrite, *never* auto-approved. Multi-parent replay, unsupported diff objects, dirty index, foreign lock, untracked files, and possible conflicts are rejected or shown as blockers; subsequent mutation remains impossible.

`ide_git_review_rebase_cherry_pick` exposes this typed plan only, and the Source Control UI rejects a stale project/ref/operation answer or any claim of worktree/ref mutation or authorization. Standalone native Windows/Linux tests exercise an actual diverged Git graph, original commit lineage, stale selected ref, dirty sources, foreign lock, forged selectors and merge commits. Frontend tests verify the absence of executable routes. Source-side CI is not installed-app or real human approval evidence.

**Next:** complete 6C-04A native exact-head tests, repair demonstrated failures, then take 6C-04B. The separately unfinished 6C-03B work remains open.
