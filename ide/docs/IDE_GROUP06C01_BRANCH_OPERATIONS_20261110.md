# IDE Group 6C — Branch operations roadmap and 6C-01 native review

Status: **6C-01 source implementation, exact-head native acceptance pending**. Branch `ide/group06c-branch-operations-20261110` stacked on independently verified Group 6B commit `ea584951d7b5acea85e7f1ad9502ba4a461a8f87`.

## Group 6B handoff

Group 6B reviewed staging/commit/amend production source and real crash/restart host tests are complete at its exact-head Windows/Linux native CI, along with actual NSIS/Debian installation and app process restarts. **Not release-approved or physically accepted.** Do not redo acceptance just to block new 6C engineering. Human approval/refusal/cancellation, installed GUI and real installed process interruption remain deferred under [issue #212](https://github.com/vjmichael/Lu-AI-GUI/issues/212). The native hard-abort test proved an orphaned Git `.git/index.lock` can survive after branch-ref publication. Foreign or ambiguous locks remain untouched; no automatic Git ref retry/rewrite is permitted.

## 6C engineering sequence

- **6C-01 — Local branch-operation inspection and conservative preview.** Inspect exact registered non-linked repository, current local HEAD, selected existing local branch and commit relationships for merge, rebase, cherry-pick, revert and stash. Classify blockers for stale/unsupported refs, detached HEAD, shallow/unrelated histories, conflict/active Git state, lock, worktree/index dirtiness, and untracked stash choices. Read-only and zero authority.
- **6C-02 — Reviewed merge planning.** Model explicit fast-forward vs divergent merge, non-destructive merge conflict previews in quarantined state, immutable source/target tree/index identity and planned branch CAS; no live mutation from preview.
- **6C-03 — Protected merge execution and durable recovery.** Separate real one-use native OS approval, immutable source plan revalidation, exact-index tree/ref postcondition, no hooks/network, unowned-lock denial, cancellation/restart with no automatic ref replay.
- **6C-04 — Rebase and cherry-pick.** Restrict source commits, parent semantics, affected ref set, in-progress state and conflict handling. History rewrites require explicitly distinct high-risk approval and independent original-object retention. No implicit force-push.
- **6C-05 — Revert and stash.** Explicit pre/post state and worktree custody, tracked/untracked selection policy, durable non-destructive restoration and no silent overwrite.
- **6C-06 — Cross-platform hardening and installed operator acceptance.** Windows NSIS/Linux Debian native UI, approval/refusal/cancellation, abrupt process death, branch-ref/index/stash recovery, independent Git evidence and unrelated project identity. **Human/physical gates deferred**, not passed.

Conflicts needing interactive resolution remain in **Group 6D**. Remote GitHub write and synchronization are **Group 6E**, separate from all local branch operations. Existing 6B checks remain mandatory.

## 6C-01 completed code boundaries

Host `ide/host/src/git_branch_operations.rs` provides a single read-only `review_local_branch_operation(project_id, typed_operation, selected_local_branch)`. It only accepts an opened OS-picker project whose `.git` is an ordinary directory at exactly its canonical root. The selector is strictly an existing local branch; no raw SHA, file paths, remote-tracking refs, linked-worktree authority, network URLs or executable operation arguments are admitted. Cherry-pick and revert inspect only the selected branch tip (single-parent gate); revert additionally reports when that tip is not already in current local history. Stash is targetless; untracked files are a separate unresolved policy. Outputs are metadata, blocker codes and a domain-separated evidence digest, **never a grant**.

Tauri `ide_git_review_branch_operation` exposes only this typed inspection; no operation executor was added. Source control UI requests it on demand, invalidates stale answers on project/ref selection changes, checks that authorization and execution flags remain false, and displays Git blockers and ancestry. There is no automatic rebase, checkout, stash, cherry-pick, revert, hook, ref update, conflict resolver or remote fetch. The visible review does not imply readiness to execute.

## Verification contract

`.github/workflows/ide-group06c-native.yml` runs exact-source Windows 2022 and Ubuntu 24.04 source gates: actual libgit2 graph fixtures, hostile branch names, detached and dirty/lock refusal, source inspection, web hostile route tests, frontend build, Rust formatting and Tauri compilation. Source testing is not a physical OS dialog or installed-app acceptance. The operator must not rely on this digest to approve future 6C changes; all 6C-02+ mutations require an independently reviewed one-use OS authorization contract and exact source revalidation.
