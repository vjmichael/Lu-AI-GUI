# Lu P0-TOOLS-02 — Model-proposed, human-reviewed filesystem effects

Date: November 11, 2026.
Development branch: lu/p0-tools02-reviewed-file-effects-20261111
Stacked source parent: d18c46637e2de9db88ef1d7cb2324a919628fe66
Status: SOURCE IMPLEMENTED; native compiler, runtime, physical GGUF, GUI, CI and regression acceptance deferred by owner directive. No PR, merge or CI submitted.

## Codebase findings

The existing desktop already implements actual protected native effects with
separate human approval and journals:

* UTF-8 new file: WriteProposalQueue / WriteApprovalLedger /
  native_host_write, pinned native-picked target and no-overwrite effect.
* Full text replacement: EditProposalQueue / EditApprovalLedger /
  native_host_edit, exact old/new byte preview, native-selected file identity,
  backup, source SHA and conflict check.
* Copy and move: file_transfer_desktop + native_file_transfer_tool, exact
  source hash and newly generated target basename; move ambiguity requires
  reconciliation, not replay.
* Reversible trash: file_trash_desktop + native_file_trash_tool and custody
  ledger, separate restore approval.
* ZIP/archive extract, batch extraction and split archive: native source and
  destination selection, complete member preview, effect journal, cancellation
  and explicit partial recovery.

The missing integration was that one normal-chat "propose_native_action"
model intent supported the file copy/move/trash/archive paths and Office
drafts, while complete UTF-8 text creation/replacement was exposed through
separate incompatible model JSON envelope types. A model could not make one
uniform typed file-action suggestion that handed off to the already-written
reviewed creation and edit pipelines.

## Source implementation

1. Add CreateTextFile and EditTextFile to NativeAction's conservative,
   current HUMAN text classifier. Refuse mixed actions, negations, explanatory
   requests, archived/attached text and Office-as-plain-text confusion.
2. A genuine completed local model response may now supply one exact
   `propose_native_action` object with `action` create_text_file or
   edit_text_file and `textFileDraft` (safe basename and whole UTF-8
   contents). No arbitrary filesystem path, command, authority, tool ID or
   partial patch is permitted. Strict JSON parsing, unknown-field rejection,
   format validation and fixed size limits apply.
3. The host consumes an original chat-bound, completed-model proposal once
   and re-reads the **original ANSF-authenticated human turn**. A stale
   different/newer human turn, changed intent, missing full content or
   incompatible action is refused. The proposal is not replayed on failure.
4. A trusted Rust-only bridge translates the vetted original model bytes
   to the EXISTING create/edit proposal schema, stages the real durable
   approval ledger, then emits the existing UI event. The WebView cannot
   send a model-created directory, replacement bytes or effect grant.
   If durable staging or UI publication fails, fail closed and deny/dismiss
   that pending review. No native write tool is dispatched by the bridge.
5. The UI reveals the complete model-proposed text on the initial review
   card, then routes the user into the existing whole-file write/edit
   preview. Native OS folder/file pickers and independent final native
   confirmations stay mandatory. Existing journal, per-operation grant,
   independently checked hash/receipt and interrupted-effect reconciliation
   paths perform the actual effects; this task did not make another executor.
6. Existing copy/move/trash/archive/Office model actions retain their
   native review path and are not silently widened to raw commands.

## Authority and outcome contract

Model JSON -> inert human-review suggestion -> original ANSF source check ->
existing review queue and durable approval intent -> OS picker ->
exact full preview and SHA/identity checks -> separate native confirmation ->
one-use effect grant and existing journal-dispatched native operation ->
existing receipt/recovery UI.

No link in this chain upgrades plan approval to execution authority. This
integration does not claim that a model suggestion, a journal APPLIED state or
a queued operation proves a task completed. Interrupted and uncertain
effects remain manual-reconciliation cases, NEVER automatic retries.

## Source-level tests written but NOT executed

* Exact current-user action classification, including mixed/negated requests.
* Typed text drafts with whole-file content only, strict unknown and
  duplicate JSON rejection, no directory fields or permission laundering.
* Wrong conversation and one-use original generation handling.
* Native model hint describes new typed action without execution authority.

No CI/PR checking or regression suite was run, in accordance with the
owner's implementation-first instruction.

## Next engineering groups

P0-TOOLS-03 — Real browser/Reader/OCR/Observe evidence and original ANSF
source continuity for model reasoning. Separate network and screen consent.

P0-TOOLS-04 — Extend actual reviewed specialist application operations
beyond Git/FFprobe with containment, bounded receipts and semantic oracles.

P0-TOOLS-05 — Current-source independent execution evidence into the
P0-Next project task/criterion review workflow, without automatic completion.

P0-MEMORY — Fully integrated ANSF goal, correction, retrieval and safe task
continuation across model turns.

P0-AUTH — Unified worker and file-effect custody/reconciliation, crash-safe
journal linkage and human-denial guarantees across all effect domains.

P0-GATE — Near product completion, batch native Windows/Linux installed
compiler, genuine GGUF model action selection, authorization and regression
acceptance. Remains deliberately deferred; no merge or release authorized.

STOP: P0-TOOLS-02 only.
