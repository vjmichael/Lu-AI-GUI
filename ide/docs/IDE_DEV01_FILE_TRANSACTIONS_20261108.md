# Lu IDE DEV-01.3 — no-clobber file move, recoverable trash and restore

**Status:** Source-implemented only, native Windows/Linux tests and installed GUI acceptance open.
**Parent:** DEV-01.2 [PR #121](https://github.com/vjmichael/Lu-AI-GUI/pull/121), which is stacked on DEV-01.1 [PR #119](https://github.com/vjmichael/Lu-AI-GUI/pull/119). No release candidate or Lu Core integration.

## Safety design

- User selects one already saved, small regular project file; native preview supplies its exact SHA-256 revision, and the native registry re-verifies the SHA. Rename and move use `cap_std::fs::Dir::hard_link` to create an **exclusive new destination**, followed by another source/destination digest check and removal of the original link. They **never** call ordinary overwrite-capable `rename`, ambient path-based `std::fs` mutations, or execute source code.
- The destination must be a normal directory inside the **same native-picker-registered project and filesystem**. Case-only collisions, existing targets, symlink directories, path traversal and reserved `.git`/`.lu-ide` paths are denied. An opened editor tab must be closed before moving or trashing its file; no draft is silently discarded.
- The project-local private `.lu-ide-trash` contains a durable versioned JSON intent receipt, a random token and a same-filesystem hard-linked payload; file and (on Unix) directory metadata are synchronized. Receipt creation precedes payload transfer. A crash may leave two hard links or an incomplete record but must not intentionally lose the only surviving copy. The native Trash panel displays such anomalous states and does **not** auto-delete or auto-claim recovery.
- Restore validates the immutable original path, token format and SHA, refuses any existing original destination (including unrelated external changes), exclusively links the verified payload back and only then removes the trash payload and manifest. Bad or forged records are denied. The trash is hidden from the normal source explorer and preview to avoid accidentally indexing retained private files. The user must retain this project-local folder to preserve trash data; Git may treat it as untracked until an owner-approved policy decides how to exclude it.
- Directory tree rename/move/trash remains **not implemented**. A hard link cannot rename a directory; the ordinary native rename API may overwrite a target and is explicitly rejected for this tranche.
- An independent external writer can race changes between any revision check and unlink. The implementation does not claim a mandatory cross-process lock or crash-atomic transaction. It prefers retaining redundant links to loss and requires real OS failure-injection before installation acceptance.

## UI and tests

- The Files pane adds a selected-file Rename/Move control, deliberate Trash confirmation and Project Trash with explicit Restore. The UI obtains fresh native saved revisions, verifies returned receipts, refuses open editor tabs, invalidates stale Quick Open indices and refreshes exact native directory listings.
- New real-filesystem regression suite covers same-folder rename and cross-folder move, no-clobber, revision conflict, path and case collisions, directory denial, durable trash across restart, restoration, external conflict, malformed token, closed project and symlinked trash directory.
- Dedicated `ide-dev01-file-transactions-native-ci.yml` requires exact-head Windows2022/Ubuntu24 Rust host tests, rustfmt, full npm frontend tests, Tauri type-check and preserved lock blobs. A source pass is not an installed app or crash recovery pass.

## Remaining native acceptance

1. Full real Windows and Linux OS source/host CI at the exact final head; repair specific compiler, filesystem or static test failures without changing source safeguards.
2. Installed Windows NSIS and Linux .deb GUI control and native filesystem acceptance: file move, collision, trash, restart, restore, dirty/open editor-tab preservation and clean uninstall. Real process termination between journal/hard-link/unlink stages is required to assess recovery claims.
3. Finalize directory operations only with a genuine no-replace primitive and explicit subtree, open-tab, symlink and recovery semantics; never approximate via unsafe check-then-`rename`.
4. Follow with app-data-only persistent unsaved draft checkpoints, split-pane usability and measured large-project indexing. Signing/legal review and P07 Lu Core integration remain deferred; no historical PRs or CI lanes are closed.
