# IDE 6C-03A — Protected local fast-forward and durable recovery

**Branch:** `ide/group06c03-protected-merge-20261110`, stacked on 6C-02 draft PR #214 exact `f2cd984cf89b25c8a8ba70109e979a89fd13451f`.

## Safety and operation scope

6C-03 is intentionally split into independently testable engineering slices. **6C-03A implements native one-use OS-confirmed local fast-forward only.** Divergent merges (even disjoint paths), octopus merges, merge-commit creation, rename resolution, submodule/symlink checkout, conflicted indexes and abnormal Git states are **not authorized**. 6C-02's conservative tree review is not evidence that an actual divergent merged tree is verified; 6C-03B must supply that verification and separate protected execution/recovery. Do not describe 6C-03 as fully completed until that work and acceptance gates close.

The execution path **must** originate in the OS-picker-registered native project, with a plain local Git directory and no linked worktree or external Gitdir. It accepts only existing local branch names plus the SHA-256 of a **fresh** 6C-02 review. The original digest is **never an authorization token**. The Tauri host re-runs the planner before showing a blocking native OS confirmation dialog and again before executing. The same process-local per-project one-use coordinator serializes this with 6B Git staging/commit authorization and recovery. A denied OS dialog, stale digest, cancellation or changed Git state never calls checkout.

Approved fast-forward rules: exact selected branch tip descends from current HEAD; a unique merge base equals original HEAD, no blockers; maximum 80 all-visible path entries; ordinary ASCII paths and regular file modes only; no detected symlink, path alias, reserved device or missing source tree evidence. All changes are reviewed by the host; the UI cannot supply a checkout path or arbitrary commit SHA.

## Native irreversible boundary

1. Re-read exact current local branch ref, selected ref, Git index bytes and tree, source worktree state, project registration, and source evidence. Refuse existing `.git/index.lock` rather than remove it.
2. Append and `sync_all()` a host-private per-project durable journal record in the installed application's data directory (`prepared`). One-use native cancellation succeeds only before the first worktree mutation.
3. Atomically fence cancellation, then **durably record `checkout_intent` before changing any worktree/index files**. Libgit2 `checkout_tree` runs in safe mode, with no forced overwrite and no untracked/ignored removal. The independent target index tree must equal selected committed tree and the working tree must match that index, or the operation stops for manual recovery.
4. Persist `checkout_verified`, original and selected exact index SHA-256, then `publishing`. Re-check expected original local ref/selected tip and index bytes before the only ref write: `reference_matching`, an exact old-HEAD compare-and-swap. No `reset --hard`, hook, Git command, remote fetch/push, stash operation or force-ref update. A concurrent ref update stops, leaving journal evidence and potentially changed working files for manual review; **never automatically retry**.
5. Append `ref_published` then independently reopen/refetch local Git ref, index tree/bytes, and worktree status. Append terminal `verified` only after those checks. An error after checkout is **not safe to retry**.

An abrupt process death can strand native `.git/index.lock` or leave the index/worktree at the selected tree while the branch still points at the original commit. The application intentionally does **not** undo checkout, delete a lock, or advance a ref on restart. Recovery read-only inspection differentiates no journal, verified fast-forward, untouched prepublication intent, checkout completed without ref publication, published ref awaiting final receipt, and drift/ambiguous effects. **Only** if an OS operator separately approves and all original evidence still agrees, native reconciliation may append a terminal *receipt* for an already-published change or original untouched state. It never repairs ambiguous outcomes or recycles consent.

## Actual testing and exclusions

Source module: `ide/host/src/git_protected_merge.rs`; UI and one-use OS dialogs: `ide/app/src-tauri/src/lib.rs` and `ide/app/web/main.jsx`. Dedicated `.github/workflows/ide-group06c03-native.yml` runs real local libgit2 fixtures, cancellation, stale plans, dirty source, foreign index locks, actual source/head/index/worktree integrity, process-abort restart checks, hostile JS boundaries, frontend bundle, and Rust Tauri compilation on Windows 2022 and Ubuntu 24.04. Do **not** claim native CI passed until the exact head results are examined.

Native unit tests directly call the production host method with a disposable test authorization token to verify its mechanics; these are **not** human consent and are never presented as such. Actual OS approval, refusal, mid-dialog cancellation, installed NSIS/Debian restart/recovery and physical Windows tests remain deferred, consistent with [6B physical issue #212](https://github.com/vjmichael/Lu-AI-GUI/issues/212).

**Outstanding 6C-03B:** Proven-divergent resulting tree, conflict/rename resolution policy, protected merge-commit creation, exact worktree/index publication and manual recovery after actual multi-parent commit. **No merge to `main` and no release claim** while these gates remain open.
