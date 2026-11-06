# Lu IDE Wave D7C — Handle-rooted Windows worker staging and publication

**Status: source implementation in draft, not native-accepted, not exposed to desktop execution.** D7C builds on D7B PR #60, D7 PR #58, and D6 PR #56. Preserve that merge order.

## Reason for D7C

D7B opened directory handles to check identity but walked child paths via `std::fs::read_dir`, `Path::join` and absolute `OpenOptions`. A concurrent same-user rename, junction or leaf swap could redirect those later path operations. D7C removes the ambient-path tree copier rather than simply adding more path comparisons.

## Implementation

- Added `windows_cap_tree.rs`, compiled only under `windows-worker` on Windows, using **pinned cap-std/cap-fs-ext 4.0.3**. cap-std implements Windows directory-handle-relative traversal; no arbitrary path or system command is supplied by Lu models, the WebView, or JobPlan.
- The trusted absolute source/destination roots are opened **once** through Windows `OpenOptions` with `FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS`, and `share_mode(READ|WRITE)` without `FILE_SHARE_DELETE`. Root handle metadata must be a normal directory, not a reparse point. All children are opened via `Dir::entries`, `Dir::open_dir`, or `Dir::open_with` on the pinned parent handle.
- Leaf files use `FollowSymlinks::No`; Windows `GetFileInformationByHandle` rejects any non-regular file, reparse point, or hardlinked object (`nNumberOfLinks != 1`). Path components reject traversal separators, NTFS stream separators, controls and invalid names.
- Fixed depth/file/total/single-file quotas are enforced during copy. SHA-256 covers relative path, length, and file content, preserving D7B's canonical depth-first lock format. Contents and metadata are rechecked after read.
- Source workspace and complete offline SDK/toolchain now stage through `publish_into_native_parent` rather than a full-path `create_dir_all`. It opens the already-authorized native parent, exclusively creates each child with handle-relative APIs, and re-hashes the copied tree through a still-open child directory. Workspace snapshot and toolchain SHA-256 are revalidated immediately before consuming the one-shot supervisor grant.
- Native output publication uses the same handle-rooted, create-new, bounded and re-sealed operation. A job cannot overwrite an existing output directory or redirect output by replacing an intermediate junction.
- The fallback ambient-path `tree_transfer` and `directory_identity` implementations have been removed. There is no disabled-security fallback branch.

## Test matrix added

| Test | Native assertion |
| --- | --- |
| Capability copy and tampering | Rooted copy hash equals original and changes when output modified |
| Concurrent child rename/junction swap | 24 attempts per run while an adversary swaps a directory; every accepted copy equals the trusted seal and excludes the external secret |
| Repeat race | Dedicated Windows workflow repeats the race test three times |
| Native publication | Job output is create-new and re-sealed; same-ID reuse and parent traversal denied |
| Symlinked output parent | Publishing to a native parent junction fails; no host directory writes |
| Inherited D7B checks | Hardlinks, source swaps, toolchain drift, missing-Job recovery quarantine, receipt ordering, real Cargo cancellation |
| Installed desktop separation | No `ide_run_job` or Windows worker command in Tauri |

CI: `.github/workflows/ide-d7c-windows-capability.yml` runs `cargo check --features windows-worker --all-targets`, Windows Rust tests, repeated race test, formatting, and static desktop-denial checks.

## Remaining production acceptance gates

1. **Exact-head native Windows compile and test results:** this source has not been accepted by GitHub CI yet. The D6 live-socket denial issue remains separately open, and D7B's complete pinned offline SDK Cargo execution is still pending.
2. **Host root authority and ACLs:** `cap_std` roots child operations, but trusted root selection and exclusion of hostile same-account writes to profile data, toolchain executables, and the journal require a privileged or independently protected host broker. Rooted traversal alone is not proof of provenance.
3. **Independent durable recovery witness:** D7B's external observer is a fixed Job Object fixture. Production worker crash reconciliation still cannot mark a missing named Job cancelled without a separately authenticated, durable process-tree witness. Until such a broker is implemented and verified, missing Job means quarantine, not success.
4. **Installed Windows E2E:** no distributed IDE Run command; worker remains disabled by default. Real cancellation/restart/SDK verification on a physical installed build remains open.

**Status language:** D7C handle-rooted source paths are implemented; full production worker is NOT accepted or enabled.
