# PLAN-06 — Native project task-list interface and reviewed plan editor

**Lu Core only.** This is a native user-facing task list stacked on PLAN-05 PR #213, not work in Lu IDE. PLAN-05's independent installed-app acceptance continues separately. PLAN-07 incremental graph refresh is not part of this change.

## Intended task-list workflow

1. In the normal Lu chat, select **Project tasks**, or use Settings → Tools & permissions → Open project task list. Lu opens its own fixed-host-label native Tauri window. The model, an untrusted project file, or a renderer URL cannot create or assume that window.
2. Select an existing PLAN-05 ANSF-backed project register. The project selector displays a short scope identifier and original task count. The host reopens every original proposal, graph, classifier snapshot and register revision, refusing truncated/re-hashed history.
3. View **categorized and human-ordered tasks** using All, Broken, Not integrated, Unverified and Completed filters, and search by title, description, group or original requirement ID. Completed means only the exact requirement was independently verified and is not flagged for new review; it does **not** prove the whole task is complete.
4. Select a task to see source requirement, exact HEAD, source/status SHA, classifier confidence/health/explanation, original evidence nodes and edges, and its distinct durable human revision history. Project documents and model replies are untrusted data, never commands.
5. Correct task wording, priority, blockers, and acceptance criterion through a **separate native OS human-confirmation dialog**. The original requirement/evidence identifiers are immutable, and PLAN-05 appends one revision only under optimistic expected-version verification. This makes any earlier PLAN-06 plan approval stale.
6. Reorder tasks with accessible up/down controls (not rendered pseudo-dragging). Host verifies an exact permutation of the original IDs. A native OS review adds a new append-only **draft** plan snapshot, never authorization.
7. Approve only the reviewed **plan order and wording** in a native OS dialog. This records human approval of the plan; it does not authorize any tool, worker, Git effect, file write, scheduler, or acceptance outcome. If PLAN-05 source changes since the review, plan approval becomes stale and must be renewed after explicit review. Outstanding source-drift revisions cannot silently carry approval.
8. Export only after native approval of export intent and a native OS Save File dialog to a *new* .json path. Source-bound export includes original model proposal/reply digest, current register/board SHA, tasks, original revision events, approval freshness, and explicit \`noExecutionAuthority=true\`. Existing files cannot be overwritten. Writes are flushed and verified byte-for-byte.

## Durable native contract

- Existing \`ansf/library.db\`: \`ansf_project_task_board_revisions\` is a new table, not a second database. One SQLite IMMEDIATE transaction per new ordering/approval revision, strict sequential hash-chain and original register-source binding, readback before commit.
- All board commands require the **independent \`task-list\` Tauri window**. The model and main conversation window cannot call the board writer directly; there is no model tool route.
- Board approval has its own state, separate from PLAN-05's \`independentlyVerifiedRequirement\` and from the agent execution authority. A later PLAN-05 correction makes the old approval historical (the history is not rewritten).
- Original PLAN-05 history remains the authority for source classification, provenance and human correction. PLAN-06 does not modify the PLAN-05 branch, substitute model responses, or automatically authorize execution.

## Acceptance

Native Rust tests cover approval/reorder hash invariants, actual ANSF SQLite persistence across reopen, source-register revision invalidation of review, byte-preserving export and refusal to overwrite, independent archived evidence, and tampered-board-history denial. Node tests cover filter semantics, grouping, exact task order and keyboard-accessible move logic; UI source contracts assert native-only commands and no model execution routes.

\`native-plan06-board\` is an explicit exact-head, SHA-admitted Windows/Linux matrix in the existing REPO-01 grouped workflow. It verifies frontend builds, actual Rust test execution, native Tauri compilation, ANSF readback and source manifest. **Installed Windows UIA human review, physical operator consent and unrelated PLAN-05 acceptance are not inferred from passing compiler/host tests.** A distinct installed acceptance gate is required for a release claim.

### Outstanding until evidence exists

- Exact-head Windows/Linux native source, React/Node, Rust and Tauri checks.
- Actual installed NSIS task-list window activation, accessibility, project switching, OS confirmation refuse/approve, restart and export in an interactive Windows desktop.
- PLAN-05's pending installed acceptance is independent and must not be called complete based on PLAN-06 testing.

No PLAN-07 work is introduced.


## Installed Windows NSIS acceptance (separate exact-head gate)

The grouped REPO-01 workflow also contains an explicitly SHA-admitted
\`installed-plan06-windows\` job. It requires:

1. A **genuine pinned CPU engine** and authentic exact-head x64 NSIS installer
   with build, installer, installed app, and engine byte digests verified.
   Test-only DLL resource placeholders are created only *after* the authentic
   package is finished, for a separate Rust test fixture build.
2. Two distinct disposable original project scopes in the same **real ANSF
   SQLite** profile, with two source-backed tasks per register. The fixture
   explicitly labels its model JSON as synthetic, never real GGUF inference.
3. The native **Project Tasks** Win32 HWND opened from the installed chat
   window. UI Automation requires the real source/filters, an actual cancel
   of the OS reorder dialog with **no database write**, and an actual OS
   confirmation with exactly one board revision.
4. A second native OS approval *refusal*, then actual human-review confirmation
   of plan order, checked against independent SQLite read-only evidence.
5. A demonstrated change of **displayed source project** via the native
   selector (sending keys without observing the switch is not acceptance).
6. Graceful real installed process exit and restart, reopening the task-list
   HWND and verifying the identical original ANSF revision/approval SHA chain,
   plus cancellation of the native export confirmation without a write.
7. Genuine NSIS uninstall with original user memory, both source projects,
   all proposal bytes and human review chains still preserved.

The acceptance script writes an explicit non-PASS receipt on any failure
(including unavailable Win32 UI Automation on hosted Windows, fake HWND,
misidentified OS dialogs, stale receipts or missing original project).
A hosted UIA run is *not* an assertion of acceptance on the creator's physical
Radeon-equipped machine; physical human testing remains separate.
