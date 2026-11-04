# WRITE-01E.2 — Journal-dispatched existing-text transaction and rollback

Status: Source implementation on `lu/write-01e-approved-edit-gate-20261103`. **Not desktop-enabled and not Windows-accepted.** See `docs/LU_V1_0_TASKS.md` for the deferred verification ledger.

## Architectural integration

This follows the existing `write_format` + `edit_safety` host review and the `native_create_new` pattern, without replacing the create-new-only primitive. The existing `ConversationAuthority::dispatch_bound_durable` enforces trusted persisted human-turn ancestry and task scope, `PolicyEngine` enforces a revocable WriteWorkspace grant, `ExactEditPolicy` or `ExactRollbackPolicy` enforces approved parameters, `NativeToolRegistry` dispatches typed Protobuf and `EffectJournal` commits PREPARED -> EXECUTING before any effect, followed by APPLIED receipt or UNCERTAIN on ambiguous failure. A model cannot approve itself. Registration/discovery is not authority.

**Registration:** `native_edit_existing::register_edit_tools_pinned(registry, Arc<Dir>)` accepts a previously opened and host-selected directory. The request contains only a single basename, expected old digest, exact replacement bytes and the review fingerprint; the pinned directory never comes from a model, WebView path, or imported chat. The host retains the exact original/replacement strings for the independently reviewed diff. `ExactEditPolicy` recomputes that review and insists on exact bytes, filename and digests; the dispatcher still requires a real user-originated task.

**The user's Documents setting remains in `document_preferences`.** For an arbitrary approved existing file, the host must open/pin its parent directory from a trusted native selection and must not infer that every existing path is writable from the default Documents setting.

## Edit transaction and crash windows

1. After authorization and durable EffectJournal EXECUTING, reject non-simple basename, symlink, nonregular file, oversize or invalid format; check the current SHA-256 baseline. On Unix, also reject hardlinked targets with more than one name.
2. Using the same opened capability directory, `create_new` a deterministic `.lu-edit-<call-id>.meta` intent with original and replacement digests, target basename and reviewed fingerprint; `sync_all` file and, on Unix, parent directory. Never overwrite an existing stage ID.
3. Write **independent original bytes** into `.lu-edit-<call-id>.orig`, sync and read back hash. This is *not* a hard link to the target. This original-byte custody remains after success.
4. Write proposed replacement to `.lu-edit-<call-id>.new`, sync and read back hash; re-read live target and reject stale source.
5. Rename staged replacement over the original through the same `cap_std::fs::Dir` handle; sync parent where supported, re-read target SHA-256 and return typed Protobuf receipt. `DurableDispatcher` persists an encoded ToolReply as APPLIED. Any crash/unknown return is UNCERTAIN and is never automatically retried.

Recovery observations `observe_edit` classify absent intent, original+verified backup, published replacement+verified backup, missing target, corrupted backup/manifest, changed target and inaccessible state. Even a byte-for-byte match is **evidence of observed bytes**, not evidence that this process published them; it never upgrades an UNCERTAIN journal row to APPLIED or creates authority.

The original backup and manifest are deliberately retained for rollback/history. Staging aliases may be left after a crash or failed operation. Do not silently garbage-collect unresolved transactions; later authenticated cleanup must reconcile and preserve backups first.

## Separately approved rollback

`lu.edit_existing.rollback` is a **separate** WriteWorkspace effect with another authenticated user turn/approval and unique journal call ID. It validates the original edit manifest, the immutable-intended (but filesystem-mutable) backup's SHA-256, the current live file digest and requested restoration digest. It emits `.lu-edit-<rollback-id>.rollback-meta`, then a synced `.rollback` stage, does another current-source check, atomically renames into place and verifies readback. `observe_rollback` classifies the two legitimate byte outcomes and damaged/changed states. It does **not** auto-revert another editor's later modifications.

## Test coverage added

- Pure read-only preflight: exact review, invalid JSON, mutated plan, wrong target/approval.
- Native mutation fault injection after intent, original backup, replacement stage and publication; every point preserves original bytes or verified replacement and remains recoverable without retry.
- Rollback fault injection after intent, stage and publication; validation of separate original backup.
- Refusal of reused stage IDs, tampered backup, stale baseline, invalid JSON, symlinks and hardlinks (Unix).
- Production native path integration fixture: `ConversationAuthority` + `PolicyEngine` + exact per-call fence + `NativeToolRegistry` + SQLite `EffectJournal` + typed Protobuf, covering commit, applied receipt, replay, revocation, explicit rollback, unauthorized bytes, wrong target, fake approval and persisted UNCERTAIN on source conflict.
- CI workflow explicitly runs the new native authority test on Windows, in addition to Ubuntu `cargo test --locked` for all Lu Core tests.

## Hard limits before production enablement

1. **Desktop approval is not wired.** The host must show a complete diff and hold a fresh authenticated consent record/selected directory handle; the example integration verifier is a fixture, not a proof of desktop user provenance.
2. **Cross-process CAS is not atomic.** A non-cooperating editor can race between the final digest check and rename. Host-exclusive ownership/cooperating file leases and Windows file-identity tests are still needed before promising protection against every concurrent writer.
3. **Platform durability:** Windows directory flush, NTFS rename/replace behavior, hardlink/reparse-point races, ACL/ADS/xattr preservation, sharing violations and power-failure behavior are **not accepted**. Linux directory fsync is attempted; neither implementation is an absolute disk-failure guarantee.
4. **Effect state is not independently recovered to APPLIED.** Startup `EffectJournal::reconcile_interrupted` requires exclusive process ownership and leaves EXECUTING as UNCERTAIN. Read-only artifact inspection provides evidence; any decision to resolve uncertain effects must be an explicitly authorized separate workflow.
5. **Test result:** source and tests committed, **not yet proven green** by Rust/Windows CI in this conversation. No installed-app acceptance. Do not enable GUI edit existing until these gates pass.

This is intentional incremental engineering of the existing Lu Core transaction path, not a replacement architecture or claim that v1.0 file editing is finished.
