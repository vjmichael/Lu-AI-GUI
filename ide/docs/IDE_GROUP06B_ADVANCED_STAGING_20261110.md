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
