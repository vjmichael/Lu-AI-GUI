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
