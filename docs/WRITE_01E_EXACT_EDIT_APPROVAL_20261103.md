# WRITE-01E — Exact edit approval boundary (2026-11-03)

## Repository status
The new `src-tauri/lu-core/src/reversible_edit.rs` module is source-committed and registered in `lu-core/src/lib.rs`. It implements **proposal verification only**. This does **not** enable existing-file writes in the desktop, and is not a claim that a crash-safe reversible commit is complete. This work builds on WRITE-01D's default Documents location, Advanced Settings override, file-link UX and guarded create-only pipeline.

## Implemented contract
- Host supplies the existing file's **actual bytes**, an admitted relative path and proposed UTF-8 replacement. File type is not inferred from the extension. NUL/non-UTF-8, oversize (4 MiB) and malformed relative paths are rejected.
- The proposal captures **untruncated old and new content** for user-facing exact-diff rendering, and separately stores source and replacement SHA-256.
- Approval fingerprints bind the **relative path, source digest and replacement digest**. A model's assertion of approval is never accepted as host authorization.
- The verifier rejects changed sources, corrupted proposal fields, mismatched approval and no-op proposals. The verifier is pure: it makes **no disk effects**.
- Unit tests cover exact approvals, stale-source conflict, tamper, traversal, NUL, no-op and extension-agnostic text.

## Required remaining WRITE-01E stages (not passed)
1. **Trusted host approval binding.** Desktop UI must display all changes, require a real human turn-bound confirmation (or an explicitly granted narrower policy), and preserve the trusted approval record separately from model output.
2. **Capability-safe exclusive file access.** Handle symlinks, reparse points, hardlinks, concurrent editors and root changes; close verification-to-rename races. The current pure verifier is *not* an atomic lock.
3. **Immutable backup + durable journal.** Snapshot original bytes using an authenticated, exact-target identity, fsync backup and prepared receipt before mutation. Write new content to a same-volume temporary file, flush, atomically publish where the platform supports it, sync containing directory as supported.
4. **Crash reconciliation.** Replay prepared, published, verified, uncertain and rollback states without blind re-execution. Verify post-publication bytes and preserve both backup and generated version if readback fails.
5. **Rollback.** Explicit user approval, verify current digest still matches the version being rolled back, never clobber later external changes, and make rollback an independently journaled operation.
6. **Adversarial tests and CI.** Kill between every durability barrier; deny/harden symlink and reparse traversal; race external edits; large text, Unicode and changed encoding; Windows/NTFS sharing and rename behavior; crash after publication; independent hash/readback. Only then expose model-facing *edit existing file* capability.

## Location and link rules
Default new user documents to the OS Documents known folder on Windows and configured XDG Documents folder on Linux, with an Advanced Settings override. Avoid app-data or arbitrary paths as user-visible document destinations. Show an **Open containing folder** action for created/edited files, validated against host-owned path identity. Keep source artifacts distinct from documents and internal journals/backups. A named extension is **not** enough to guarantee correct serialization; format-specific validators (JSON etc.) must run prior to proposal approval and be repeated on commit.

## Verification
GitHub source writes recorded; tests **not executed** in this session because the accessible execution container has no Rust/Cargo toolchain and the GitHub file connector cannot execute CI. The proposed tests must be run by CI before claiming acceptance.
