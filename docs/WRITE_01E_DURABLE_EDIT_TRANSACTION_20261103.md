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


## WRITE-01E.3 — Trusted desktop integration (development-only)

**Implementation source:**
- `edit_approval_ledger.rs`: separate host-only SQLite WAL/FULL edit review evidence. Stage → Selected → Approved, Denied, or Interrupted. Pending and Selected are interrupted after owner restart, and Approved has a unique effect call ID. This record is *not* authority.
- `lu-desktop/src-tauri/src/edit_proposal.rs`: parses a completed model turn's bounded strict JSON `{"kind":"propose_edit_file","file_name":"notes.md","purpose":"...","content":"replacement text"}`. Model proposals are inert until the user selects an existing matching file with the **native file picker**. Source is read from a verified handle; old and replacement text are checked without guessing binary formats, capped at 64 KiB original preview and 16 KiB model replacement; no truncation.
- `lu-desktop/src-tauri/src/native_host_edit.rs`: after an independent **native confirmation dialog**, emits a host-generated persisted User turn, opens precise `ConversationAuthority` / `PolicyEngine` WriteWorkspace scope, dispatches exact requested Protobuf bytes into pinned directory via `ExactEditPolicy` and the existing `EffectJournal`, then revokes task and scope.
- `lu-desktop/src-tauri/src/lib.rs`: new `lu_review_edit_file`, `lu_confirm_edit_file`, `lu_deny_edit_file`, `lu_edit_recovery` and `lu_inspect_edit_recovery` commands. No UI-provided path, byte payload, or approval token accepted by the effectful command. A **native containing-folder picker** is required to inspect crashed edit artifacts for an externally selected file; this includes missing target scenarios and is read-only.
- `lu-desktop/web/edit_diff.mjs` + React UI: full original and full replacement text, all original/addition/removal lines (not a minimal diff), exact target/digests, explicit full-review UI acknowledgement and a *separate native confirmation*. Recovery shows both durable approval and effect state, plus observations only after native folder re-selection.
- Tests: Lu Core edit-approval ledger restart/identity tests, Windows ledger CI step, frontend change/review contract tests; GitHub CI must compile the desktop Tauri command registry.

**Do not claim production acceptance:** This is source-committed; exact full Linux/Windows CI outcomes, native modal UX, Windows NTFS hardlink/ACL/ADS and externally concurrent replacement races must be verified before enabling as an accepted v1.0 capability. A WebView acknowledgement is *not* the authorization; native OS confirmation and host-bound task scope are. Do not auto-restart or auto-retry ambiguous edits.

**Source convergence caution:** REPO-01 source-tree lock verifies the branch contents only; the earlier unpublished complete local development archive remains a separate unresolved comparison.


### Follow-up integration and Windows API boundary
- Corrected `model_file_creation_hint` so **explicit existing-file edit requests** can produce `propose_edit_file` rather than being told to explain edits only; exploratory how-to questions and negated requests do not trigger the new proposal contract. Neither prompt augmentation nor a model proposal grants authority.
- The portable `native-auth` harness now executes the **real** desktop `EditProposalQueue` and `execute_approved_edit_pinned`, including host-selected existing file, wrong-chat/filename refusal, authenticated persisted user turn, journal, original-byte backup, and observed applied receipt. A separate Windows CI step exercises those same tests.
- Windows reparse-point handling: `cap_std::fs::Dir::symlink_metadata` returns `cap_std::fs::Metadata`, **not** `std::fs::Metadata`; the standard Windows `MetadataExt::file_attributes` methods cannot be applied to it. The code retains proven symlink/type refusal and opened-std-file reparse checks. A Windows-specific handle-identity and reparse-safe adapter with independently verified file IDs remains required before a broad safety claim. We did **not** introduce unsafe FFI into `lu-core` or enable edits without that acceptance.

## WRITE-01E.4A — Read-only undo candidate and exact receipt verification
At the earlier fully green baseline `13cf8de2` (desktop CI `41766078286`, WIN-LOAD `41766078264`, REPO-01 `41766078348`), native edit and desktop commands compiled and passed CI. That does **not** establish installation, NTFS power-loss, external-editor atomicity, or complete metadata retention.

For the next development revision, `native_edit_existing::inspect_rollback_candidate` reads the existing edit's original-byte backup and current target through the approved directory capability, validates both digests and supported UTF-8/format rules, and returns untruncated text only when both are at or below the 64-KiB review bound. It is pure observation. If an external editor changed the target or the backup was corrupted, no preview is offered.

`native_edit_existing::verified_edit_receipt` requires a genuine, type-correct durable `ToolReply` containing a committed `EditExistingResult` with the specific call ID, edit tool ID, source/replacement SHA-256 digests and approved byte count. A bare `APPLIED` state, malformed receipt, wrong tool ID, or matching file bytes are **not** sufficient. The desktop recovery view and verified Open containing folder link use this checker. The UI displays complete prospective restore changes and both texts for evidence only. **No WebView rollback command is registered.**

This phase is not a new undo permission. A separate trusted native confirmation, new authenticated user turn, exclusive target/metadata guarantees, rollback effect-journal dispatch and crash acceptance are required before user-invoked restore becomes available. No automatic retries.


## WRITE-01E.4B — Separately approved and journaled original restore
The preceding `WRITE-01E.4A` recovery preview remains a *read-only* inspection. Restoration requires an independently authenticated action: the native containing-folder picker and verified original effect receipt produce a bounded complete snapshot of the current edited text, original verified text and prospective change lines. `rollback_review.rs` retains the pinned directory and exact bytes; a new `rollback_approval_ledger.rs` stores reviewed, approved, denied or interrupted states in a separate SQLite WAL/FULL ledger, binding chat, original edit ID, target, digests, restore byte length and a unique new rollback effect ID. On restart REVIEWED becomes INTERRUPTED. The model does not choose a restore path, permission, target handle or bytes.

The desktop displays both full versions and requires an explicit review acknowledgement plus a **separate native OS modal** to restore. The host revalidates the original approved edit and the review, writes a new authenticated human user turn, grants only `lu.edit_existing.rollback`, and dispatches the specific Protobuf restore through `ConversationAuthority`, `ExactRollbackPolicy`, `NativeToolRegistry` and `EffectJournal`. This is **not** a retry of the edit or approval previously granted. Any new external changes to the target refuse the rollback; ambiguous effects stay UNCERTAIN and never auto-retry.

Before replacing the target, `rollback_with_stop` creates a durable independent `.lu-edit-<rollback-id>.prior` copy of the edited version and validates its SHA-256, in addition to retaining the original edit's `.orig`. The read-only `observe_rollback` requires verified custody of that displaced edited version to certify restored bytes. The newly added `verified_rollback_receipt` decodes the separate typed Protobuf success reply; journal APPLIED state alone does not count.

The UI includes read-only historical approval/effect/receipt evidence across restarts, plus an explicit keep-current denial. The Windows CI workflow includes actual desktop native rollback integration and persisted approval-ledger tests. The user's Documents-folder selection is unchanged. **This is source implementation pending CI**, not installed-app acceptance. NTFS metadata preservation, reparse and hardlink refusal, noncooperating external-editor races, native Windows directory durability, and physical power-failure recovery remain explicit v1.0 gates.

## WRITE-01E.4C.1 — Windows opened-handle file safety gate (development, CI pending)

**Predecessor closed:** WRITE-01E.4B exact revision `f40e3206d7b1319bf5665d9434403f66976fd605` passed Lu Desktop CI run `41771221728`, REPO-01 `41771221850`, and WIN-LOAD-01 `41771221721`. This was full *development CI* including Windows Tauri compile, not installed-app or power-loss acceptance.

**Implementation:** `windows_file_guard.rs` contains an isolated, audited Win32 FFI adapter for `GetFileInformationByHandleEx`. FileStandardInfo refuses multiple hardlinks, deletion-pending files, and directories. FileIdInfo obtains a 128-bit native FileId and volume serial number directly from the OPENED handle (not pathname, name hash, or unstable Rust Windows metadata accessors). FileStreamInfo enumerates data streams into a bounded, 8-byte-aligned buffer; only a single unnamed default stream is accepted. Unsupported enumerations and malformed/oversized tables fail closed. Read-only, hidden, system, sparse, compressed, encrypted, offline, reparse and special attributes are refused rather than being silently discarded. See Microsoft Win32 docs for `FILE_ID_INFO`, `FILE_STANDARD_INFO` and `FILE_STREAM_INFO`.

`native_edit_existing::regular_bytes` now requires valid metadata from the original opened handle and a new handle at the end of the read, rejecting identity substitutions. The native edit and restore transactions remember the Windows FileId of the original live file and independently compare it after the final hash check, immediately before capability-relative stage rename. Edit/rollback manifests themselves are read through the same strict validation; a reparse/hardlinked/named-stream sidecar cannot be treated as ordinary authoritative recovery evidence.

**New Windows CI tests** exercise ordinary opened-file IDs, a hardlink refusal, named alternate data streams, read-only file refusal before manifest creation, two different FileIds with identical file bytes, and a deterministic adversarial same-byte file-identity replacement inserted just before publication. They check the target remains unchanged and no success receipt is produced. The runner must disclose when its volume does not support named streams; in that case ADS admission is not accepted by that test.

**Explicit remaining limits:** (1) Win32 query/check/rename is *not* an OS-atomic compare-and-swap against a malicious noncooperating editor. (2) An external same-byte file substitution between user selection and transaction start can still pass hash-only selection because the picked FileId is not yet durably bound to the approved tool request. (3) Ordinary inherited or custom Windows DACLs/ACLs, timestamps, creation IDs and all other metadata are not guaranteed preserved across replacement; the guard rejects several special attributes but does not constitute ACL preservation. (4) Directory flush and power-cut behavior remain unverified; Windows `sync_parent` remains intentionally non-durable. (5) 128-bit FileIds may be unsupported on some filesystems, which then fail closed. (6) Windows junction/reparse and filesystem-specific semantics, installed-app native picker/dialog flow, and physical destructive tests remain acceptance gates. The development draft must not be marketed as universally race-free or metadata-safe.

### WRITE-01E.4C FFI containment correction
The initial Windows compile correctly refused Win32 FFI placed inside `lu-core` because that crate deliberately declares `#![forbid(unsafe_code)]`. The exact same native handle-query implementation now lives in `src-tauri/lu-win-worker-job/src/file_guard.rs` behind a safe `lu_win_worker_job::file_guard::inspect_open_file` interface. `lu-core/src/windows_file_guard.rs` contains **only a safe re-export**, preserving the core's unsafe-free rule and existing Windows-specific dependency; no new dependency or Cargo.lock version change. Windows CI calls the isolated FFI crate's unit tests and the core's Windows existing-file transaction tests separately. This is code separation, not a claim of externally race-free replacement.

### WRITE-01E.4C.1 CI evidence and release caveat
The implementation commit `8eb7fa51d36eaef0cfb27222b40aa9c06edea3ed` passed GitHub Lu Desktop CI `41773702637`, source verifier `41773702703` and WIN-LOAD `41773702696` (all SUCCESS, including Windows Tauri compilation and the isolated Win32 FFI adversarial tests). The Windows tests established checked refusal of hardlinked/readonly/named-stream targets and identity-based detection of a deterministically substituted same-byte different-file-id target; the full Lu Core and desktop native authorization suites also passed.

This does not certify lossless preservation of user ACLs, arbitrary metadata or Windows directory flushes; it does not prove an OS-atomic race-free compare-and-rename with an external editor. Those remain open follow-up tasks and must be independently tested before full v1.0 acceptance. The accepted `0.1.0-alpha.2` release baseline has not changed.


## WRITE-01E.4C.2 — File identity continuity from selection through native transaction
Lu's Windows native file picker captures an opened-handle `WindowsFileIdentity`, then compares the corresponding handle opened relative to the pinned directory before completing review. The 48-hex native volume/FileId token is incorporated into a versioned v2 `ReviewedEdit` SHA domain, along with the exact original/replacement bytes and host relative basename. Thus the approval fingerprint already durably stored in the existing SQLite ledger now commits to the *specific picked file object*, not only its name and contents. The model cannot supply the token or grant. Linux's existing v1 review digest remains unchanged.

At confirmed execution, Lu's native desktop host validates the still-opened target's native identity and registers the first-party native tool via `register_edit_tools_pinned_selected`. A Windows registration without this host-selected identity is discovery only and cannot execute the native write/restore effect. The native tool verifies the selected filename and ID after the EXECUTING journal barrier, before any manifest/backup is written, and checks the same ID again just before publication, along with the byte digest. This closes pre-execution same-byte substitution since the original user review, though it is not an atomic Win32 CAS with arbitrary external writers.

A rollback obtains its *own* native selected current-file ID at the new restore review, double-checks it around complete preview reading, and holds it in the single-use host queue. It is independently confirmed and passed to the native rollback transaction, which verifies ID before intent and publish. The rollback ID is not allowed to reuse the original edit's stale FileId.

New test coverage includes different native file IDs with identical bytes between selection and dispatch and direct native effect-entry tests that bypass the earlier host precheck. **Unresolved:** DACL/ACL/timestamps/ADS preservation, true external-writer atomicity, native-directory flush after publication, and power-loss / installed-app Windows acceptance. Source committed; exact CI evidence pending.


### WRITE-01E.4C.2 proven CI status
The exact code commit `2b664fd2b95b6daa7a2d53788006b3e722148afe` passed Lu Desktop CI `41777297445`, REPO-01 `41777297387`, and WIN-LOAD-01 `41777297414`. The Windows runner executed both editor-swap adversarial tests (after native selection and inside the native transaction), passed the trusted edit and separate rollback workflows, and compiled the complete Tauri host. Linux validation also passed. Previous CI errors were limited to an untyped non-Windows `None` in shared picker code and a stale Windows test fixture missing native identity, both corrected in this accepted revision.

This establishes a verifiable *development implementation* of exact Windows selected-object ID continuity; it is NOT an atomic compare-and-swap under adversarial external writers or proof that non-default ACLs, timestamps, alternate streams or other NTFS metadata are preserved. Files with unsupported special attributes or multiple links continue to be refused. Release and physical installed-app acceptance are separate gates.
