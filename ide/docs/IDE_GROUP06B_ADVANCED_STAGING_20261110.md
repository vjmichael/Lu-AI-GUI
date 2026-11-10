# IDE Group 6B — Reviewed multi-file Git staging (2026-11-10)

**Dedicated source branch:** `ide/group06b-advanced-staging-20261110`, based on Group 6A acceptance source `119830a`. It does not mutate or cancel Group 6A exact-head native jobs, and does not grant Lu or source text operational authority.

## Existing accepted design to preserve

`ide/host/src/reviewed_git.rs` already exposes one tracked regular-file preview, OS-dialog one-file staging and separately reviewed local single-file commit. It checks exact registered root, HEAD, clean index, source SHA-256 and staged blob identity; rejects symlink/gitlink, detached HEAD, untracked content and unrelated staged changes. No Git CLI, hooks or push. Stage and commit are separately consented by the native Tauri host. These are the foundation, not a license to silently broaden a grant.

## Implemented now: G06B-01 bounded batch preview

`ide/host/src/git_batch_review.rs` assembles one read-only proposal for 1–8 already tracked, regular files from the active native registered project. Inputs are validated and deduplicated, sorted deterministically, checked against a common Git HEAD and unchanged index fingerprint, and limited to 4 MiB total source. Each path is inspected by the same native one-file reviewed source capability already used for ordinary staging. First file is rechecked after the rest to detect mid-review changes. `revalidate_reviewed_git_batch` re-reads all files and fails closed on stale HEAD, index or any file SHA/byte mismatch. No stage, commit, ref write, hook, Git subprocess, network operation, credential, checkout, operator consent or execution grant is created.

The project-ID-only Tauri routes `ide_git_preview_batch` and `ide_git_verify_batch_review` expose that metadata to an explicit human Source Control batch-preview panel. The panel displays every file digest, head/index pin, and persistent **Stage authorized: No; Commit authorized: No** labels. Existing single-file approved operations remain unchanged. Native fixtures test deterministic two-file preview, duplicate/untracked/traversal denials and stale source rejection; Node tests enforce no-apply/no-commit route.

## Remaining genuine Group 6B engineering work

1. **6B-02 reviewed hunk selection:** exact patch bytes/line identity, binary/rename/symlink handling, partial-stage conflict detection, high-volume limits; no patch apply without explicit OS grant.
2. **6B-03 atomic selected-batch stage:** native one-use operator approval, optimistic HEAD/index/source revision check, exclusive index lock and multiple blob insertion under a single protected transaction; independently verify staged bytes and post-write receipts, and fully preserve unrelated index paths. Do not simply loop the existing one-file stage API.
3. **6B-04 reviewed batch commit/amend:** separate physical approval and branch ref compare-and-swap; original parent/ref/source and index identity receipts. Reject detached HEAD, conflicts, concurrent branch updates and unrelated staged changes. Amend requires an additional explicit destructive-rewrite grant.
4. **6B-05 recovery and installed acceptance:** process crash/restart, stale-plan refusal, split partial write cleanup, physical Windows/Linux native dialog approval/refusal/cancel, installed NSIS/Debian project switching and source identity.
5. **Cross-platform native acceptance:** run Rust unit tests, Node boundary, Windows/Linux Tauri compiler and formatter at one reconciled source SHA. Source tests are not native passes, and installed human acceptance is not performed here.

This plan deliberately prevents false classification of a read-only batch proposal as approved advanced staging or a completed commit.

## G06B-02 — Reviewed hunk selection (2026-11-10)

**Source implemented:** `ide/host/src/git_hunk_review.rs` builds bounded text-only libgit2 hunks from two explicitly pinned buffers, not ambient filesystem Git path lookup: original `HEAD`-equivalent clean-index blob and a regular tracked source file read from the registered-project OS capability. `read_reviewed_git_hunk_buffers` rechecks reviewed file SHA-256, repository HEAD, clean index and index fingerprint before giving bytes to the diff engine. Unsupported binary/NUL/invalid UTF-8 and oversized changes are refused, as are unregistered paths, symlinks and untracked source files (via the established reviewed_git capability).

Each hunk includes its stable per-plan content digest, exact old/new line positions, actual diff line origin/text and original index/source/head pins. Bounds: at most 48 hunks, 256 lines per hunk, 4 KiB per diff line, 96 KiB total diff bytes, and the existing 1-MiB tracked-source cap. `select_reviewed_git_hunks` **independently reconstructs the entire plan** and compares it, denying stale, modified, unrecognized or duplicate IDs. Selected IDs are serialized in native hunk order and hashed for later separate reviewed authorization. The selection response is evidence, never a patch application or Git/index write grant.

Native Tauri exposes only `ide_git_preview_hunks` and `ide_git_select_reviewed_hunks`; the Source Control panel renders readable diff hunks and checkboxes with explicit *No stage / No commit*. All project-switch state and asynchronous inspection requests are invalidated by the existing registered-project effect. New native tests cover two independently selectable hunks, precise line content and identifiers, absence of index/ref modifications, stale source, forged headers, duplicate IDs, binary input, untracked and outside-root denials; Node tests enforce the read-only host/Tauri/UI boundary. One consolidated Group 6B Windows/Linux workflow executes the new Node and Rust checks alongside existing batch preview and Tauri compiler tests.

**Not implemented / acceptance still open:** no Git-index partial patch application, no atomic staging, no consent grant, no patch apply/revert, no binary/rename staging, no source file writes, no commit/amend, no push/fetch/network. Group 6B-03 and 6B-04 require genuine native one-use OS consent, exact-head source/index transaction, durable receipts and crash recovery. Physical NSIS/Debian installed manual approval/refusal/project-switch/restart and accessibility acceptance remains a separate unexecuted gate. CI run status must be inspected by exact SHA before marking source complete.

## G06B-03 — Protected selected-hunk index staging (2026-11-10)

### Native implementation (source; native run separate)

- `ide/host/src/git_partial_stage.rs`: deterministic selected-hunk reconstruction from original clean-index blob and previously reviewed text diff, refusing unrelated/context-mismatching lines, unsupported incomplete newline cases, binary data and oversized output. Only the selected hunks enter the intended staged Git blob; unselected lines remain original in the index. Original worktree files are never written.
- Git index update is prepared as an app-private temporary standalone libgit2 index, preserving all existing tracked regular entries; the Git blob is independently hashed and verified before publication. The original clean index SHA, exact HEAD, selected source SHA, stored OID and unrelated index entries are rechecked.
- Publication acquires Git's exclusive `.git/index.lock` via `create_new`; any existing foreign lock is denied. The prepared full index is synced to that lock and atomically renamed over the main index after a final native source-capability reread, HEAD/index recheck and cancellation atomic compare-and-exchange. No shell, Git hook, checkout, source write, commit, fetch, push or network is used. A failed rename leaves the lock for manual reconciliation, rather than falsely claiming safe rollback.
- Post-publication independently reloads the **on-disk** index via libgit2, validates staged OID, SHA-256 of the blob and exact unchanged original worktree source SHA. A durable app-data JSONL journal syncs `prepared`, `publishing`, `published`, `verified` or `cancelled` receipts; its bound journal schema contains project/selection/HEAD/index SHA, blob OID/SHA and original source SHA.
- `ide/app/src-tauri/src/lib.rs`: one-use native OS confirmation presents exact project, file, HEAD, hunk count and selection digest before a stage; an active session receives an independent native cancellation token. The WebView can request cancellation only prior to the atomic publication commit point; a completed stage cannot be silently rolled back. No WebView-controlled boolean or synthetic consent grants authority.
- Restart recovery is **read-only by default** and classifies an observed original/intended index, HEAD drift, journal phase and remaining Git lock. A separate real OS human-approved reconciliation can append a terminal journal receipt when original index remains unchanged or independently verified staged index/blob/source bytes match. It never auto-deletes a Git lock, overwrites ambiguous index contents or grants another stage; foreign locks and unresolved drift require manual operator review.
- Source Control has protected stage, cancel and journal-inspection controls, plus separately authorized journal reconciliation. Partial stage receipts cannot automatically feed the previously approved *one-file full-file* Git commit path.

### Adversarial native/Node fixtures

The native test group verifies staged one-hunk bytes with a second deliberately unselected hunk and unchanged worktree; cancellation before publication; foreign lock preservation; stale-source denial; injected interrupted journal; actual registry reopen + safe journal reconciliation; and later legitimate Git index changes after a completed receipt. Node tests verify Tauri cannot stage without native OS dialog, cancellation only before commit point, independent blob verification, journal and no shell/remote calls in production.

### Acceptance and exclusions

The GH Actions `ide-group06b-native.yml` is the designated single exact-source Windows 2022/Ubuntu 24.04 Rust+Node+Tauri source gate. Its first actual 6B-03 attempt found both stale 6B-02/03 JavaScript assertions; the next reached Rust, where both native lanes reported `G06B_STAGE_PUBLISHED_BYTES_MISMATCH`. We repaired the verified defect by forcing libgit2's index handle to reload from disk after the native atomic rename, rather than trusting a cached index. The exact-head rerun must independently verify this and any remaining formatter/runtime failures before claiming native acceptance.

**Not accepted here:** a signed/physical Windows NSIS or Debian installed app, genuine observed operator approval/refusal/cancel, accessibility, restart while the process is killed mid-index-write, adversarial concurrent Git-index race, remote/ref operation, or commit/amend. The human consent implementation's source cannot substitute for observed OS approval. These gates remain open separately as previously requested.

Next after 6B-03 source acceptance: 6B-04 separately reviewed atomic commits/amend with fresh OS authorization; 6D interrupted-operation remediation for ambiguous or foreign-owned Git locks.
