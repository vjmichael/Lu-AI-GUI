# E1-P13 — Reviewed application source-revision custody

Date: November 8, 2026

## What changed

Lu's existing reviewed Git/FFprobe operations already required per-original-human-turn
resource-read and external-application-execute authorization, independent parsing,
an APPLIED native OS effect receipt, and a second source-derived semantic observation.

**E1-P13 closes the obvious two-observation gap**: a source can change
while the worker is running, and its final read can coincidentally agree
with the worker's answer without proving it was the *originally approved*
source revision.

The host now captures a source identity/revision witness **before**
launching the supervised child. It holds any approved media file
descriptor through the child lifetime and rechecks after the original
OS effect and independent source parser complete, before the model
receives feedback.

The original user chooses the resource through Lu's native host.
Neither the model nor an outside app can choose a new path, executable,
permission, verification method or evidence digest. Lu Core remains
natively unsandboxed, with its external reviewed workers supervised
under the existing operating-system process model. No Docker, MCP, or
additional user installation is introduced.

## Git source-revision custody

The new `git_source_revision(root, operation)` samples an operation-specific
host-owned source witness without invoking Git or trusting its output:

- Git HEAD — the independently read committed hash at the approved root.
- Current branch — the independently read symbolic or detached HEAD.
- Tracked status — the approved committed tree and fully merged index,
  plus SHA-256-derived **content revision of every supported tracked
  worktree file**, including presence/absence markers. Two different
  file edits that both yield "one changed tracked file" do *not* share
  a revision witness.
- History count — independently parsed HEAD hash and actual bounded
  parent graph count (including P12's checksum-validated packed objects).

The guard also independently checks the original repository and `.git`
directory identities before and after execution and refuses symlink
or reparse components. On Unix the host holds open directory handles
and compares device/inode; on Windows the conservative metadata
checks do not claim an atomic directory snapshot.

If an operation's source semantics cannot be captured before launch,
the operation may continue as a reviewed read-only request, but it
**cannot be promoted to independently VERIFIED** by a later successful
observation. A revision change becomes CONFLICT, blocking model feedback.

## Pinned media-file custody

`reviewed_source_guard.rs` captures the approved original WAV/MP4:

1. Refuses symlink/reparse paths and opens the actual local file
   read-only. On Windows it sets FILE_SHARE_READ, which permits FFprobe
   to read the same file but denies newly opened writers/delete/rename
   while the host holds its handle.
2. Verifies the file's exact SHA-256 matches the original host-approved
   source digest (limit: 256 MiB).
3. Retains the file handle throughout the supervised FFprobe operation.
4. Rechecks its own descriptor's metadata and SHA-256, resolves the
   original path a second time without following links, and checks
   the fresh path's identity, metadata and content SHA-256.
5. On Unix, compares original device/inode to reject replacement by
   another file with **identical bytes and length**; on Windows it
   relies on the shared-open constraints plus content and timestamp
   checks, rather than falsely claiming nightly-only file-index APIs.

An insufficient initial witness returns UNSUPPORTED for the
*independent verifier*; an existing successful original external worker
is not falsely erased. A witness captured before launch but differing
afterward becomes CONFLICT, with no model-visible result.

## Receipt and provenance boundary

No new native operation is exposed. E1-P13 modifies only the
already-reviewed operation call path to:

- Capture the source revision before the broker's authorized launch.
- Let the existing cooperative guardian / Windows Job Object own
  external process lifetime and let E1-P10/P12 parse the actual source.
- Compare post-worker revision with the initial witness.
- Record VERIFIED, UNSUPPORTED or CONFLICT in the separate immutable,
  exact-APPLIED-receipt-bound `lu_independent_outcomes` SQLite journal.
- Return the bounded result to the model only when semantic verification
  does not conflict; never disclose source file identities, hashes,
  paths, pinned handles or raw subprocess output.

The ordinary reviewed-app model/tool audit and E1-P09 Tool activity
remain read-only. A model FinalAnswer or APPLIED process receipt is not
evidence of stable source identity, or descendant termination.

## Concrete native acceptance, initially OPEN

New independent workflow:
`.github/workflows/lu-e1-p13-resource-revision.yml`.

Both native Windows 2025 and Ubuntu 24.04 must pass at the exact frozen
head SHA with real source and native application binaries:

- Genuine Rust production-source guard unit tests: stable approved media,
  wrong digest rejection, approved Git branch revision mutation;
  Unix in-place media mutations, same-content/same-length path swaps
  and symlink refusal; Windows pinned media read-sharing that refuses
  a concurrent newly opened writer.
- The **real Git** index and repository fixture: mutate the same
  tracked file twice while the changed-entry count remains one,
  and prove the pre-worker SHA-based source revision becomes CHANGED.
- The previously failing multi-line Git packed-refs native fixture,
  using the correct multi-line file reader instead of single-ref
  parsing. This is a separately observed upstream E1-P10 bug.
- The complete supervised, authorized Git/FFprobe model-tool native
  acceptance on both OSes, with independent final-source check,
  immutable effect receipts, and no model-provided resource paths.
- Source lock proof, immutable commit provenance and uploaded native
  log artifacts.

These tests must actually run and be inspected before E1-P13 can be
declared accepted. A queued workflow, compile-only result, parser
fixture or source-lock pass is insufficient. Physical installed Windows
and genuine loaded-GGUF autonomous tool selection remain separate
late-v1.0 acceptance gates.

## Deliberate limits

A before/after source comparison is **not atomic**. A malicious actor
with an already writable file handle, a filesystem capable of replacing
identical content and restoring timestamps, or an adversary changing
Git internals then restoring them between observations can evade
a two-point test. Full protection against this class requires stronger
trusted OS-open-handle and repository-content snapshot/custody semantics;
this module explicitly does **not** upgrade E1-P08C's process-tree
security, does not prevent same-UID escape from delegated cgroups, and
does not claim all future file mutations are prevented.

The host currently pins only ordinary media files up to 256 MiB and
independently supported Git repository states. Large cinema files,
special files, linked repos, unsupported Git worktrees, encrypted MP4,
arbitrary filesystem mutators, and unrestricted third-party applications
remain outside this positive guarantee. Refuse a false VERIFIED status;
do not silently broaden it.
