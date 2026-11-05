# Lu IDE R3.5 — Native proposal review inbox

**Branch:** `ide/workspace-development`  
**Status:** Implementation candidate, not yet accepted on Windows.  
**Date:** 2026-11-05

## User goal

Lu should eventually be able to use its standalone IDE to help code. Its model-generated instructions must remain untrusted data. The IDE owns source permissions, changes, review state, and all effects. Lu is not connected in R3.5.

## Implemented

- `ide/host/src/inbox.rs`: native `IdeProposalInbox` with at most **16** entries; random opaque IDs; SHA-256 digest of each validated proposal; `pending`, `approved_for_staging`, or `rejected` status; list, review, digest-bound decision, and dismissal.
- `ide/host/src/proposal.rs`: strict schema v1 parser, 4-file maximum, size and traversal restrictions, unknown command/auto-apply fields rejected.
- `ide/app/src-tauri/src/lib.rs`: five explicit native commands to import, list, review, decide and dismiss. **No apply, run, build, terminal, or shell command** is added.
- `ide/app/web/main.jsx`: a new **Proposals** section. Human can prepare a local JSON draft from an open editor tab, import an untrusted proposal, inspect all file diffs, explicitly approve it **for staging only**, reject it, or dismiss it.
- `ide/app/web/styles.css`: separate inbox and per-file diff review UI.

### Critical security distinction

**Approve for staging only is NOT source-file approval or application.** It records an IDE-local review decision that allows *no filesystem effects*. Neither receipt nor sender claim can instruct the host to write project files. There is still no Lu↔IDE bridge.

A proposal is revalidated against **all** current source file revisions before its review decision can be approved. Any stale source, incomplete diff, changed/replayed decision, or wrong proposal digest is rejected. Reject and dismiss require no source-file access.

The queue is deliberately **memory-only**; restarting the IDE discards all proposals and decisions. No untrusted source body, envelope, or authorization token is persisted to the project catalog. Future persistence requires encrypted storage, recovery and provenance design—not ad hoc JSON files.

### Current limits

- The inbox's **Approve for staging** does not load edits into editor tabs and never writes to disk. That is a future separate, explicitly authorized stage after conflict/review integration. Avoid implying that reviewed proposals are already applied.
- Multi-file *review* is implemented, but multi-file atomic apply is **not**. Avoid claiming rollback or all-or-nothing editing.
- Manual JSON entry is an alpha/test UI until authenticated Lu messaging exists; the `claimedProducer` string is not proof of origin.
- In-process review decision is not OS-level human authentication, and no executable worker or package-manager scope exists.
- Full external-writer races and Windows reparse/ACL hardening remain open as before.

## Automated tests

`ide/host/tests/inbox.rs`:
- Import two-file proposal, inspect complete diffs, approve for staging and confirm disk contents are **unchanged**.
- Reject a stale-file proposal, leaving original source unaffected and decision pending.
- Reject wrong proposal digest and replay of a decision.
- Prevent approval of truncated/incomplete review.
- Verify proposal inbox disappears on restart and source bodies do not enter the project catalog.

`ide/app/tests/standalone.test.mjs`:
- Five native inbox commands are explicitly allowlisted.
- UI provides import, review, approve/reject/dismiss controls and states no automatic file changes.
- No proposal apply or process execution path is introduced.

## Windows manual acceptance

1. Open the disposable source project in Lu IDE, edit a file without saving, then switch to **Proposals**.
2. Click **Prepare from open editor**. Review the generated JSON containing the correct project ID, relative path, expected disk revision and proposed text.
3. Click **Add to review inbox**. See a pending proposal. Verify the on-disk source remains unchanged.
4. Open **Review**, inspect the added/removed lines, click **Approve for staging only**, and confirm the dialog. Verify the proposal changes status but **the file on disk is still original**.
5. Repeat with a fresh draft, choose **Reject** instead. No file changes.
6. Modify the source externally after import, then try to approve a stale proposal. The IDE must refuse approval and preserve external disk contents.
7. Import an oversized/truncated proposal and check the reviewed-approval button remains unavailable.
8. Dismiss an inbox entry; verify only that queue entry disappears. Restart the IDE; confirm the inbox is empty.

**Real user Windows interaction and exact HEAD cross-platform CI remain separate acceptance gates.** See [ADR-0003](ADR_0003_LU_CODING_PROPOSAL_PROTOCOL.md) and [engineering handoff](../ENGINEERING_HANDOFF.md).
