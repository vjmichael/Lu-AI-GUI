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

## Independent recovery-keeper implementation (D7C continuation)

- `src/windows_broker.rs` implements a **separate native process** and `src/bin/windows_recovery_broker.rs` is its fixed-mode executable. The native-only `TrustedBroker` config requires an exact executable SHA-256 from a host-owned installation manifest, a native witness directory and a fixed binary path. It has no WebView, model, project, shell, or Tauri-facing command.
- `BrokerLease::begin` verifies the pinned executable with a Windows file handle that excludes concurrent write/delete, creates an OS-entropy nonce, captures the current process's PID **and kernel creation time**, then launches the fixed broker with an allowlisted environment. The broker verifies both the live parent process handle and creation time, opens the exact named Job, creates a release event, fsyncs `ready` to its own exclusive hash-chained JSONL journal, and acknowledges via a private stdout pipe.
- D7's AppContainer child is not resumed and its D3 one-shot grant is not consumed until that independent READY acknowledgement. D7 checks the broker's health throughout execution; failed admission, early broker exit or uncertain tree kill cause native termination attempts and supervisor quarantine.
- The broker holds its own Job handle. On parent termination it fsyncs `owner_exited` and `kill_requested`, invokes native `TerminateJobObject`, verifies `ActiveProcesses == 0`, fsyncs `tree_empty` or `kill_unverified`, then retains the live named Job for **up to 15 minutes**. A restarted supervisor may open and verify that original kernel object before its own durable `Cancelled` receipt; missing Job or evidence never silently counts as cancellation.
- Normal completed/cancelled release uses a **private anonymous stdin pipe with one-shot nonce** from the original owner and requires Job emptiness. The named recovery-release event is **ignored while the owner is alive**, preventing another same-user process from releasing an idle broker. Any pre-signalled event is reset after parent death before cleanup. After the supervisor has durably reconciled an empty Job it may signal recovery release. A forged release after cleanup can deny future evidence but cannot make a live Job tree be accepted as clean.
- The broker's journal can be **structurally audited** (sequence, stable job/plan/parent identity, full hash linkage, allowed event order, truncation rejection). This is diagnostics, **not cryptographic origin authentication** under a same-user threat model. In particular, a same-account user who can rewrite the journal and compute hashes must not be allowed to clear quarantine merely from those JSONL bytes.
- Fixed native acceptance: `src/bin/d7c_broker_crash_fixture.rs` plus `fixtures/Test-D7CProductionBrokerCrash.ps1`. An external observer retains a live child kernel handle, signals the recovery event prematurely (negative test), abruptly kills the original owner, checks that the worker exits, reopens the **still-live** named Job with zero active processes, independently audits the broker's journal, then signals recovery release and requires its final durable receipt. CI compiles and executes the standalone fixture on Windows.

**Security distinction:** This is an independently running OS process, but not yet a separately protected Windows **service principal**. Its same-user process handle, witness directory, and named Event namespace are not a malicious-same-user-proof authority. For production release, provision a signed service or equivalent separate-identity broker with explicit ACLs and authenticated native host channel. The installed IDE remains disconnected; no general Run route has been enabled.

## Remaining production acceptance gates

1. **Exact-head native Windows compile and test results:** this source has not been accepted by GitHub CI yet. The D6 live-socket denial issue remains separately open, and D7B's complete pinned offline SDK Cargo execution is still pending.
2. **Host root authority and ACLs:** `cap_std` roots child operations, but trusted root selection and exclusion of hostile same-account writes to profile data, toolchain executables, and the journal require a privileged or independently protected host broker. Rooted traversal alone is not proof of provenance.
3. **Independent durable recovery authority:** The new D7C keeper and production-shaped crash fixture are implemented, but exact-head Windows execution and **separately protected service identity plus ACLs** remain unverified. A missing named Job or untrusted JSONL alone still means quarantine; this new process is not a justification to bypass the native gate.
4. **Installed Windows E2E:** no distributed IDE Run command; worker remains disabled by default. Real cancellation/restart/SDK verification on a physical installed build remains open.

**Status language:** D7C handle-rooted source paths are implemented; full production worker is NOT accepted or enabled.
