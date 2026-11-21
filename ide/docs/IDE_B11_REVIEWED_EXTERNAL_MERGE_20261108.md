# Lu IDE B11 — reviewed external-source conflict merge

Source stack: B11 branch ide/b11-reviewed-conflict-merge-20261108
based on B10 exact commit ab20deade3974f14770fa94f1f6c3687f36dcc13.
Scope: standalone human-owned CodeMirror editor, never Lu Core,
no new native tool or worker capability and no release permission.

## User workflow

B10 identifies an external disk revision change. B11 adds
**Review external merge** to the selected primary source editor.

1. Read the registered native source via the existing read-only
   ide_preview_source_file cap-std capability. Reject project or file
   mismatch and any editor state drift. No background watcher.
2. Construct a bounded three-way line merge: original saved source,
   live CodeMirror draft and latest independently previewed disk source.
   Nonoverlapping edits combine. Touching/overlapping edits conflict
   unless both sides produce identical bytes.
3. For each conflict, the user explicitly chooses **Keep my editor
   change**, **Keep current disk change**, or **Keep earlier saved base**.
   No default choices. The panel shows bounded escaped text excerpts,
   never HTML or an automatically executed patch.
4. **Apply to unsaved editor** requires human confirmation and a
   second native source read. It checks the exact same native project/
   file, saved base revision, new disk SHA and text, editor generation,
   buffer revision, live CodeMirror EditorState identity and draft text.
   Any mismatch rejects the request.
5. An approved merged text is applied by **one Undo-able CodeMirror
   document transaction** (no EditorState reconstruction). After the
   editor verifies the exact resulting document, its saved base text
   and SHA become the verified latest disk text and SHA. The merged
   document remains unsaved if it differs from that disk base.
6. **Save is separate.** A later native Save still requires an exact
   matching disk revision and must reject a second external write.

Original B2 private draft checkpoints remain on disk and are not
deleted. The old in-memory private checkpoint buffer identity is not
silently reused after merge. To preserve the pre-merge draft across
restart, the user must explicitly Preserve draft BEFORE applying;
CodeMirror Undo retains the old draft in the running app.

## Algorithm and safety limits

Pure module: ide/app/web/editor_conflict_merge.js

- Exact LF, CRLF and CR line tokenization, including no final newline.
- Bounded LCS (at most 601 by 601 cells for each side), no unbounded
  whole-file diff or automatic semantic conflict resolution.
- Three input strings each limited to 128 Ki UTF-16 code units and
  600 exact lines; no more than 80 conflicts.
- The final rendered result is also limited to 128 Ki UTF-16 code units
  and 600 lines. A result that exceeds either cap is rejected.
- Each displayed conflict excerpt has at most five lines and 180
  characters per line; every truncated excerpt is labeled.
- Conservative grouping of overlapping and boundary-touching edits;
  repeated identical lines may yield ambiguous alignments, so the
  user must review the proposed resolution before applying.
- Mixed newline format is an explicit fail-closed limitation:
  CodeMirror's existing immutable line-separator facet could normalize
  inserted bytes. B11 therefore rejects a disk/merged newline format
  incompatible with that editor state *before* dispatch. The user may
  separately preserve the draft and explicitly reload.

## Trust boundary and concurrency

- Only existing registered cap-std native read commands run; no new
  Tauri command, worker, process, file write, Git action or model call.
- B6's synchronous native explorer-I/O ticket serializes preview
  operations. A selected-project change, tab change, edit, reload,
  Save or close invalidates the previous merge review.
- A second native read before Apply checks the preview disk SHA and
  exact source text against the reviewed snapshot; afterward any
  additional disk write is fenced by the original native Save CAS.
- The CodeMirror component refuses merge requests unless the actual
  immutable EditorState instance, tab mount identity and draft match
  the approved request. The source text is changed in one undoable
  editor transaction, with no disk side effect.
- No silent conflict choices, automatic merge Save, background polling,
  merge/rebase of a foreign project, or durable source snapshot.

## Development evidence and remaining gates

Changed source:
- ide/app/web/editor_conflict_merge.js (pure merger and guard)
- ide/app/web/main.jsx (manual review, choices, native recheck, rebasing)
- ide/app/web/code_editor.jsx (single undoable exact-state transaction)
- ide/app/web/styles.css (bounded accessible conflict display)
- ide/app/tests/editor_conflict_merge.test.mjs (18 regression tests)

**18 committed test bodies passed** in a local V8 execution harness with
Node assertion emulation against exact fetched source files; **1,488
generated merge cases** also passed unilateral, identical and disjoint
edit invariants. This is NOT genuine Node, locked Tauri compilation,
installed OS UI acceptance or production release approval.

Open native gates:
1. Exact-head Windows 2022 and Ubuntu 24.04 GitHub tests: complete
   npm check, host Rust and locked desktop Tauri compilation.
2. Real installed Windows NSIS and Debian/GTK: external source changes,
   per-conflict UI choice, apply into live editor, Undo, distinct Save.
3. Compare independent disk hashes: preview and Apply must never write
   project sources. Test a second external edit before Save and require
   native revision conflict.
4. Test disappeared/renamed files, concurrent typing, project switch,
   tab closure, private draft checkpoints and LF/CRLF distinctions.
5. Reconcile the B2–B10 independent source/installed gates, especially
   physical B4/B5 memory receipts and B6 GTK chooser repair. No signing
   or copyright review is implicitly approved.

B11 is SOURCE-IMPLEMENTED, NOT accepted. GitHub runs may continue
while feature work proceeds; inspect and repair exact-head native
results when they finish instead of blocking unrelated development.
