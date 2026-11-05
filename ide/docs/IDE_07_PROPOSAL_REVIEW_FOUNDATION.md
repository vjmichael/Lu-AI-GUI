# Lu IDE — R3.3 reviewed changes / Task 7 foundation

**Date:** 2026-11-04  
**Branch:** `ide/workspace-development`  
**Status:** Implementation candidate. CI and Windows interaction acceptance must be recorded against the exact commit.

## Why this matters

The user requires **Lu to use the standalone Lu IDE to assist with coding**. A future connected Lu should be able to examine project context, propose edits, explain them, and request a human-authorized apply/test/revise cycle. The IDE must own the working files, write authority, worker containment, and durable receipts. Lu does not get filesystem privileges just because a model-generated diff looks plausible.

The first independently useful step is a **read-only review of the editor's current human draft**, with no Lu dependency. This provides a usable review surface now and an explicitly bounded data contract that can later be adapted for authenticated AI proposals.

## Implementation

- `ide/host/src/review.rs`: `review_source_change(project_id, relative_path, expected_revision, proposed_text)`.
- `ide/app/src-tauri/src/lib.rs`: `ide_review_source_change`, a dedicated **read-only** native command. There is no new execute or apply command.
- `ide/app/web/main.jsx`: **Review changes** opens a review dialog; **Save reviewed changes** is a distinct human action that calls the existing revision-checked save endpoint.
- `ide/app/web/styles.css`: contextual change lines in a bounded scrolling modal, displayed as escaped text, not HTML.
- `ide/host/tests/review.rs`: real temp-file tests for review-only behavior, conflicting revision, traversal, symlink rejection, closed project, NULs, and oversized proposals.
- `ide/app/tests/standalone.test.mjs`: native-command allowlist and review/no-execution assertions.

### Safety properties

1. Open-project and relative-capability validation are reused from the existing IDE browser.
2. A proposal must supply the exact SHA-256 file revision being revised. If the file changed externally, review rejects it with `FILE_CHANGED_ON_DISK`.
3. Text over 512 KiB or containing NUL bytes is rejected; no file execution, network activity, model call, shell or package operation.
4. The review displays at most **240 lines** and **32 KiB** of text. Complex or large edits may be summarized as one changed region; the UI marks the result truncated and disables the **Save reviewed changes** button rather than representing an incomplete preview as a full diff.
5. The human action verifies the tab, selected project, draft content, and revision are still the same as when the preview was generated. The existing native save rechecks the revision before replacing the file.
6. Review data is not saved to disk or forwarded to Lu. Normal unsaved-tab handling and explicit manual Save remain unchanged.
7. This is **not** a security proof against all filesystem races or replacement of the project root; existing R3 save safety limitations continue to apply.

### Important limitation

The R3.3 review is **not an AI-generated patch** and not a connected Lu coding agent. Its current input is a human-edited tab. It does not yet provide independent model-authentication, per-task permissions, provenance, staged multi-file proposals, or durable approval tokens. The current human-owned **Save** button continues to exist separately.

Any future Lu bridge **must not** treat access to `ide_review_source_change` or a review result as permission to call `ide_save_source_file`. The bridge should deliver data-only proposals to an IDE-owned authorization/review queue; a model-originated proposal must not be an executable command, raw path grant, or self-approving save.

## Verification

Use the [standalone IDE acceptance workflow](https://github.com/vjmichael/Lu-AI-GUI/actions/workflows/ide-standalone.yml) on the exact branch HEAD:

- Native Rust unit/integration tests (including `ide/host/tests/review.rs`) on Linux and Windows.
- Rust formatting check.
- Frontend build and source/UI tests.
- Windows Tauri native compile.

CI result will be added here once it actually completes. A passing test/compile is **not** proof of Windows UX acceptance.

## Next Windows interaction checks

- [ ] Open a disposable project, edit a Python or Rust file, and click **Review changes**.
- [ ] Confirm removed/added lines and counts match the draft, while the disk file remains unchanged until **Save reviewed changes**.
- [ ] Confirm **Close review** returns without saving.
- [ ] Confirm **Save reviewed changes** updates disk bytes and the tab switches to **Saved**.
- [ ] Change the file in another editor before pressing **Review changes**. Expect a conflict without overwriting either version.
- [ ] Try enough changed lines to exceed the preview bound. Confirm the truncated warning and disabled reviewed-save button.
- [ ] Reopen the project; no unexpected Lu connection, process execution, package install, or hidden source transfer occurs.

## Next stage

A future Task 7 increment can add a versioned **data-only proposal envelope** with project identity, source revision, bounded patch text, rationale, provenance, task identity, and per-operation authorization requirements. Only after Task 2 worker containment should the IDE accept approved build/test cycles. Task 8 (Lu develops Lu) must remain a separate disposable development/worktree boundary.

See [ADR-0001](ADR_0001_STANDALONE_APP_AND_LU_BRIDGE.md), [ADR-0002](ADR_0002_WORKSPACE_CHAT_BOUNDARIES.md), and [the canonical eight-stage plan](IDE_WORKSPACE_DESIGN.md).
