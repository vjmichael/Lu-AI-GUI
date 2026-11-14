# Lu IDE — R3.3 reviewed changes / Task 7 foundation

**Date:** 2026-11-04 (updated 2026-11-05)  
**Branch:** `ide/workspace-development`  
**Status:** R3.3 Windows interactive review smoke acceptance **passed** (2026-11-05). The broader Task 7 authenticated Lu coding bridge remains unimplemented.

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

**Passed CI:** [run 41981163363](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/41981163363), source SHA `4ea91f2b09025df349ca0e1522c4666f88cd498f` (same R3.3 implementation after formatter patches). Linux native Rust: **33 tests passed**, `rustfmt --check` passed; frontend build and tests passed; Windows native IDE host tests and standalone Tauri `cargo check` passed.

**Windows R3.3 unsigned release:** [run 41981333831](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/41981333831) **passed**; artifact `lu-ide-windows-alpha-r3-3-reviewed-changes` contains a 10,462,720-byte Windows PE executable. Verified EXE SHA-256: `703be602126523b17bbedfa0094648e6174ee11139772e097c63c3f9e7525f3c`.

**User-facing package:** `Lu-IDE-Standalone-R3.3-Reviewed-Changes.zip`, integrity checked, SHA-256 `45d6e6fb22c077a6e611053400cf39f219cd930a6ccd5c9e53d3a6c8a5f24dc0`. Contains the unsigned executable, `ReviewTestProject` and `README-R3.3.txt` with read-only review, explicit save, external conflict and truncation tests. Actual Windows R3.3 review-dialog smoke acceptance was subsequently performed by the user; see the evidence below.

CI alone was not counted as desktop acceptance. The user separately performed the Windows interactive smoke test described below.

## Windows R3.3 interaction acceptance — passed November 5, 2026

- [x] Open disposable `ReviewTestProject`, edit `src/hello.py`, and click **Review changes**. Screenshot shows 1 removed and 1 added line, with red/green highlights.
- [x] Confirm removed/added counts match the draft and a separate **Save reviewed changes** action is offered. The user reports the complete smoke test passed; the screenshots demonstrate separate review/save states.
- [x] User reports **Close review** works without saving (not independently established by the provided screenshots).
- [x] Confirm **Save reviewed changes** updates the tab to **Saved**; screenshot shows the accepted `Greetings` text in the editor and the `Saved` indicator. Independent file metadata/byte comparison was not supplied.
- [x] External change triggers **FILE_CHANGED_ON_DISK**, with the user draft still **Unsaved** and the warning explicitly stating the draft remains intact. Screenshot confirms the error path and preserved draft.
- [x] Large proposed change (312 added lines) triggers the **Preview truncated** warning and leaves **Save reviewed changes** disabled. Screenshot confirms both.
- [x] User reports all R3.3 smoke checks passed, and screenshots show **Lu connection: Not connected**. No separate proof of process/network activity was supplied; threat/security acceptance remains open.

**Evidence:** Four user-supplied Windows 11 screenshots, received November 5, 2026: review dialog with one deleted and one added line; editor saved state; external modification conflict retaining an unsaved draft; and truncated 312-line addition with disabled reviewed-save. The user explicitly confirmed: **“Okay it all passes.”** This closes the R3.3 *interactive smoke* gate, not the unauthenticated-model, full sandbox, crash-recovery, concurrent external-writer, or Windows ACL-hardening gates.

## Next stage

A future Task 7 increment can add a versioned **data-only proposal envelope** with project identity, source revision, bounded patch text, rationale, provenance, task identity, and per-operation authorization requirements. Only after Task 2 worker containment should the IDE accept approved build/test cycles. Task 8 (Lu develops Lu) must remain a separate disposable development/worktree boundary.

See [ADR-0001](ADR_0001_STANDALONE_APP_AND_LU_BRIDGE.md), [ADR-0002](ADR_0002_WORKSPACE_CHAT_BOUNDARIES.md), and [the canonical eight-stage plan](IDE_WORKSPACE_DESIGN.md).
