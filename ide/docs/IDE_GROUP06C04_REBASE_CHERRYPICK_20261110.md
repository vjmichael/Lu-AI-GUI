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


## Implementation update — 6C-04B/C/D and automated 6C-04E

**All changes remain in draft PR #217, unmerged. No installed physical operator approval has been performed.**

### 6C-04B — Prospective resulting trees in quarantine

`ide/host/src/git_replay_quarantine.rs` creates a fresh, randomly named **temporary bare** Git repository; it adds the original project's object directory as an alternate read source. Real native libgit2 `cherrypick_commit` creates a temporary in-memory index for each reviewed commit. The candidate result tree and synthetic new commits are written **only to the temporary ODB**. Rebase replays at most 16 verified linear original commits, in sequence, onto the selected local branch tip. Cherry-pick replays only the selected local branch's single original tip onto the current HEAD. Any actual merge-index conflict, unsafe delta, unsupported path/file mode, oversized change set, or original path overlap is reported conservatively; publication is blocked. The quarantine is deleted on return, and the registered source repository's refs/index/worktree/ODB must be unchanged. This is distinct from 6C-04A's original-commit-only view and never issues any consent.

### 6C-04C — OS-authorized cherry-pick

`ide/host/src/git_protected_replay.rs` implements a separate **native OS dialog** for cherry-pick, revalidates the full exact prospective result after the dialog, and refuses any candidate with conflicts/blockers. A durable app-private journal is synced before first object publication. Cancellation becomes irrevocable **before** the first copy of synthetic Git objects. After consent only, the host copies previously verified tree/commit objects out of quarantine into the original Git ODB (without executing scripts), protects the existing local ref, safely checks out the new result, rereads the candidate index tree and worktree, and advances the local HEAD branch using `reference_matching` and its **exact old-head compare-and-swap**. The selected source branch and old commit remain intact. A separate native route handles cancellation.

### 6C-04D — Separate explicit history-rewrite consent for rebase

Rebase uses a **different native command and conspicuous OS warning**, not a cherry-pick or stage authorization. Before checkout/CAS it writes a unique protected `refs/lu-ide/g06c04/original/<operation-id>` original-tip ref, checks its exact identity, and records that ref in the journal. It reproduces and verifies a sequential, bounded set of new local commits; the first parent is the **exact selected local branch tip**, and subsequent commits follow their previous newly created commit. The old original commits and backup ref survive the new branch tip; no push, force-push, fetch, hooks, Git CLI, model authority or automatic rollback is allowed.

### Interrupted execution, source integrity and recovery

The journal phases are `prepared → objects_intent → objects_ready → checkout_intent → checkout_verified → publishing → published → verified`. Errors or process death **after** object publication are never auto-retried or silently rolled back. Independent read-only restart inspection checks current local branch, original/new ref identities, source tip, index bytes, index/worktree state, backup ref and journal. If a crash occurs **before** exact ref CAS and after checkout, old HEAD remains and operator repair is required; no recovery command will silently publish. If a crash happens **after** CAS but before terminal verified receipt, a separately OS-approved operation can append a historical verification receipt after an exact independent reread. This receipt reconciliation never checks out files, changes a ref or removes an unowned Git lock. Native fixtures include both genuine child-process abort points, stale plans, dirty sources, an unowned index lock, successful cherry-pick and a two-commit rebase with preserved original history.

### 6C-04E — Automated installed acceptance vs deferred physical acceptance

The dedicated `.github/workflows/ide-group06c04-native.yml` now includes **four exact-source Windows/Linux jobs**: two native real-Git, frontend and Tauri build gates, plus a Windows 2022 **real NSIS installer/restart/uninstall** smoke and Ubuntu 24.04 **real Debian package/install/restart/uninstall** smoke. The installer jobs reuse the established Group 6B5 independent Git oracle and packaging/license verification, not a fabricated installation. Their test-source SHA and result must be inspected before they can be accepted. No workflow clicks a native permission dialog for a human; owner-operated approve, deny, timeout, cancellation, accessibility, restart and physical Radeon testing remain **DEFERRED, NOT PASSING**.

**Do not merge to `main` or claim 6C-04 finished at full installed-human acceptance level** until both the real source/install jobs and the operator-owned acceptance gates are independently confirmed. 6C-03B divergent merge work also remains separately open.
