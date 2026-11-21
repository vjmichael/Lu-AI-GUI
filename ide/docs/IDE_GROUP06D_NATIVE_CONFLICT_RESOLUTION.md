# IDE Group 6D — Native conflict resolution custody and acceptance

Canonical draft: PR #221, stacked on 6C-03B PR #220. Not a release or authorization to merge.

## 6D-01 — Real unresolved Git index inventory
Read-only original stage 1/2/3 blob identities, mode and bounded text preview. Reject bad paths, symlinks, lock ambiguity, large/binary blob previews and unsupported renames. HEAD, raw index SHA-256 and stage OIDs are bound into a source-rechecked evidence hash.

## 6D-02 — Three-way local editor
The IDE shows original base/ours/theirs and supports a local draft from a complete stage or an empty manual draft. It does not permit a truncated/binary preview to become a full candidate. Drafts are memory-only and invalidated by new inspection, project changes and text edits.

## 6D-03 — Conservative native resolution review
`review_native_conflict_resolution` binds project, current local branch and HEAD, entire original raw index SHA-256, other-index-entry fingerprint, original worktree hash, exact stage OIDs and file mode, and proposed UTF-8 content SHA-256 to the review digest. Reject unsafe mode changes, binary stages, missing/oversized target worktree file, foreign Git locks and stale evidence. This is a read-only proposal, **not** a permission grant.

## 6D-04 — One-use, native-OS-approved application
Separate blocking OS dialog follows a fresh native revalidation. Host code then creates a journal before its first mutation; cancellation is accepted only before commitment. It writes and read-backs the exact candidate blob, atomically replaces only the reviewed worktree path, publishes stage 0 via libgit2 while preserving all other index entries, reads back original branch/HEAD, index, blob and file, then fsyncs a verified journal receipt. The operation does **not** commit, merge, rewrite refs, run hooks or contact any remote.

A real process abort between file replacement and index publication is *manual repair only*. A real abort after successful index publication but before receipt is eligible for **separate OS-approved receipt-only reconciliation** if the original branch/HEAD, unrelated index entries, stage-0 blob and replacement file still match. No automatic file/index retry, rollback, ref update or foreign-lock deletion is permitted. The OS dialog must not be auto-approved in CI.

## Batched native matrix
Exactly four jobs are defined under `.github/workflows/ide-group06d01-native.yml`: Windows 2022 native Rust + Tauri/frontend, Ubuntu 24.04 native Rust + Tauri/frontend, Windows NSIS installed-package smoke, Ubuntu Debian installed-package smoke. Includes original 6C regression matrix and new 6D source and child-abort tests. Trigger only by a deliberate `.github/ide-group06d-batch-trigger.txt` marker update or an explicit workflow dispatch, not every source commit.

### Acceptance status
Source implementation and tests are committed. **Do not call Windows/Linux native, package, physical-human, or hard-abort execution accepted until original exact-head logs from the batched run have been reviewed and failures fixed.** GitHub runners may queue. Physical-human owner acceptance remains deferred in issue #218. No release or main merge authorized.
