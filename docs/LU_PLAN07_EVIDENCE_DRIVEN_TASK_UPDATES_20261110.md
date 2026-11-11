# Lu PLAN-07 — Evidence-driven task updates (Phase 07A)

**Work scope:** Lu Core only. This branch is stacked on PLAN-06 native project task board PR #216, itself stacked on PLAN-05. Existing Windows installed/UIA acceptance remains separate. Do not mix Lu IDE work or begin PLAN-08.

## Agreed PLAN-07 goal

After independent engineering work changes a project, Lu must reconcile new **host-verified facts** against human-reviewed durable tasks without guessing that the work is done. Local GGUF may later suggest status/completion updates, but model text can never establish evidence, acceptance, approval or execution authority. PLAN-07A supplies the prerequisite native evidence-refresh and safe reconciliation layer; model-generated candidate synthesis and end-to-end GGUF evidence-admission are subsequent PLAN-07 work.

## 07A — Current implementation

1. A person opens Lu's independent **Project Tasks** native window and selects one original PLAN-05 register. They choose a local project through the operating-system folder picker. No raw filesystem path is accepted from the renderer or model.
2. The existing PLAN-02/03 pipeline performs an actual bounded source/Git/requirement scan, checks exact HEAD before returning, and classifies original requirements. It compares this audit to the last **original ANSF-backed** task-source snapshot; this is an **incremental evidence delta**, *not* a claim that filesystem scanning is incremental or implemented by a watcher.
3. The result contains the exact prior/current graph and classifier SHA-256, selected HEAD, changed native evidence node locators (up to 64 displayed), task identity, prior/current status, health, original version, conservative disposition, and an independent-review requirement.
4. A new \`ansf_project_task_refresh_observations\` table in the existing ANSF library stores immutable, SHA-verified preview bytes. Both original and new graph/classifier snapshots are archived by the existing PLAN-02/03 code. The preview does **not** write task revisions.
5. If original source identity disappeared, Lu reports **requirement missing** and refuses inferred remapping. If exact native metadata proves the original literal criterion, the preview says **exact criterion review candidate**, never **task completed**. Runtime, integration, installed and human acceptance remain separate.
6. To reconcile the source delta, the human must see the exact task/source review in a real OS confirmation and choose the **same project** again with a second native picker. The host independently rescans actual source after approval, denies foreign scopes, stale register head, manipulated preview, changed exact HEAD or mismatch with originally reviewed facts, and then appends PLAN-05 \`SourceChanged\` revisions through its immutable native transaction.
7. New source revisions preserve the original model proposal, all previous human corrections, task mapping and history while setting \`needsHumanReconciliation=true\` and \`independentlyVerifiedRequirement=false\`. No code execution, network request, issue, Git effect, task worker or completion is authorized.

### Status boundaries

- **UNCHANGED:** no semantic source change. Old acceptance remains historical; no new task completion.
- **REVIEW_SOURCE_CHANGE:** current selected native facts changed. Keep all prior receipt history and require operator review.
- **REQUIREMENT_MISSING:** exact immutable requirement identity is no longer present. Never auto-remap.
- **EXACT_CRITERION_REVIEW_CANDIDATE:** strong, current, complete host evidence may support only a literal exact requirement after new human revalidation. No workflow or full-task completion.
- **NEEDS_INDEPENDENT_EVIDENCE:** incomplete or human-corrected task requires review; model statements cannot promote it.

### Admission and acceptance

- Source lock at an immutable exact HEAD.
- Native Windows and Linux matrix: React build, Node UI/authority tests, Rust core delta tests, real ANSF SQLite host recheck/restart/foreign-folder/stale-token/tamper tests, and native desktop compile.
- A structured **no model tools/no execution authority** invariant is enforced in source and host tests.
- A separate installed NSIS/real Windows UIA and physical-human acceptance gate remains required; passing native tests cannot substitute for it.

### Remaining PLAN-07 groups

**07B — Actual GGUF completion/status suggestions.** Ask a genuinely loaded local model to propose *only* review candidates over host-provided new evidence, with strict bounded schema, original prompt/reply/model/session/generation provenance and no completion claims without independently verified receipts. Reject invented citations and stale source. Maintain the difference between a model suggestion and independently accepted functionality.

**07C — Human reconciliation in the complete workflow.** Show both deterministic and model proposals with versioned refusal/approval and native refresh, preserve corrected original wording, check source again after decision, and durably recover interrupted/stale updates.

**07D — Grouped acceptance and installed release gate.** Independently run genuine GGUF and cross-platform native tests, then installed NSIS UIA human/refusal/project-switch/restart/ANSF preservation and creator-machine acceptance. Keep all existing PLAN-05/06 gates separate.

No autonomous completion, no CI-pass-to-ACCEPTED shortcut, and no PLAN-08 work.


## PLAN-07B — Genuine local GGUF evidence-backed suggestions (implemented, acceptance pending)

The independently hosted Project Tasks UI can ask an **actually loaded local GGUF** for review steps based solely on the original native PLAN-07A source snapshot. The native command refuses if no real ready inference session exists, requires a human-reviewed OS consent dialog and OS folder selection, then uses the existing normal-chat FIFO generation queue and \`TrustedInferenceSupervisor\` reserved-generation protocol. The request passes **empty history**, no browser, tools or worker authority. Both before and after inference, the host rechecks the selected source/HEAD/status and the model/session identity. Interrupted, cancelled or changed-source inference refuses to persist a suggestion.

The strict raw output must contain only \`schema\`, \`previewSha256\` and \`candidates\` (1–16). Every candidate must have exactly seven keys: \`taskId\`, \`requirementId\`, \`status\`, \`action\`, \`evidenceIds\`, \`reason\`, \`nextEvidence\`. IDs, status and citations must be exact host-provided facts from that *same original task*, with duplicate/foreign citations rejected. \`review_exact_criterion\` is permitted only when the host has an independently supported candidate. Completion, acceptance, approval or execution fields are rejected outright, including if invented by the model. The host seals a separate \`ModelSuggestion\` object with \`suggestedOnly=true\`, \`independentlyAccepted=false\`, \`taskComplete=false\`, and \`executionAuthorized=false\`.

A new table \`ansf_project_task_update_model_suggestions\` in the **same existing ANSF SQLite library** stores the original prompt and raw model reply byte-for-byte together with exact prompt/reply digests, model ref, session/generation IDs, preview, register and current graph/status digests. An IMMEDIATE transaction rechecks the latest register source lease before insert. Restart retrieval independently reconstructs the result from original raw bytes and archived audited source facts; a rehashed JSON claim cannot replace the genuine original. A saved suggestion never edits PLAN-05 register, PLAN-06 plan or tool permissions.

Native tests use **explicit synthetic model JSON** to test schema, citation, stale source, tampering and SQLite proof preservation. These tests do not claim that actual GGUF inference has passed. The real generation path is connected and must be independently exercised with a loaded GGUF on the physical Windows system and in a separately admitted integration test; any model returning extra words or non-JSON fails closed.

### B remaining acceptance boundary

- Exact-head Windows/Linux Rust core and Tauri/React native test matrix, including immutable prompt/reply readback, provenance checks and original model replies.
- Genuine GGUF end-to-end acceptance with the owner's Qwen GGUF (or separately pinned real GGUF model): verify selected source, actual inference events, strict JSON parse, original reply, cancellation and no execution.
- Installed Windows human approval/restart evidence still required separately under PLAN-07D.

PLAN-07C (full operator reconciliation/recovery) and PLAN-07D (installed release gate) remain unstarted.


### Exact-head genuine model and cancellation acceptance extension

- \`scripts/plan07b-real-gguf-acceptance.sh\` builds the reviewed llama.cpp
  source (commit \`2bc46baa9207d098d03741ac394e7faaf1a955b6\`), verifies the
  pinned Qwen3-0.6B Q4_K_M GGUF SHA-256, and produces an original real
  \`/completion\` response. The native host first creates an actual disposable
  ANSF register from original project files. Its *initial* PLAN-05 proposal
  is an **explicitly synthetic test baseline**, never represented as model
  generation.
- \`benchmarks/e1/plan07b_real_gguf_task_updates.py\` preflights actual GBNF,
  interrupts a genuine streaming model HTTP request, and preserves exact
  request/reply hashes, original model/engine SHA, generation tokens and
  cancellation evidence. It does not manufacture a model reply or force a
  successful completion if the actual 0.6B model produces invalid output.
  Source identifiers are grammar-constrained solely to prevent citation
  fabrication. The real model must still generate the suggested action,
  reason and requested future independent evidence.
- Two explicit \`#[ignore]\` Rust tests bridge original actual ANSF source to
  the real Qwen inference. The second test rejects altered source or reply,
  reconstructs exact model suggestions from the **original raw reply**,
  verifies the saved original prompt/reply, and independently proves the
  PLAN-05 register's immutable source/revision SHA was unchanged.
- \`scripts/plan07b-independent-original-ansf-oracle.py\` reads SQLite in
  read-only mode and checks original source tasks, saved user memory,
  original model bytes, no completion/authorization and cancelled-stream
  receipts. It does not rely on a Rust test printing PASS.
- Actual **Lu production FIFO scoped cancellation** now has its own native
  task-window command \`lu_project_update_model_cancel\`, a non-renderer-
  controlled lookup of the exact reserved queue label, queued/active cancel
  behavior, supervisor cancel for active generations, and cancel checks
  before persistence. Cross-chat isolation and active cancellation have
  independent native regression tests. However, external streaming
  interruption and isolated queue tests do **not yet constitute a combined
  loaded-GGUF + installed Lu UI cancellation matrix**; that still needs
  physical Windows operator acceptance.

The exact-PR-head \`genuine-plan07b-gguf\` job in REPO-01 and the
\`native-plan07b-model\` Windows/Linux matrix are separately SHA-admitted.
A queued job, source check, or synthetic fixture is not a genuine-model PASS.


## PLAN-07C — Complete human review and durable reconciliation

**Engineering implementation** (native acceptance and real installed human UIA results remain separate):

### Review is not acceptance of completed work

In the independent **Project Tasks** native window the human selects the exact
original task and either a deterministic PLAN-07A evidence candidate or a
verified original PLAN-07B GGUF suggestion. A required 5–250 character human
note records what to inspect. The native host refuses model suggestions that
cannot be reconstructed from original source, prompt, reply and ANSF bytes.

The host commits a **Prepared** record BEFORE displaying a real OS confirmation
dialog. The dialog names the original task and version, requirement ID, original
graph SHA, source register lease, model suggestion SHA (or native-only), and
human note. **Approve** means the person authorizes only a *future evidence
review*. It never authorizes executing a model/tool, marking the exact source
criterion accepted, completing an engineering task, committing Git, or
auto-merging. **Refuse** is an explicit durable terminal event, not a silently
discarded modal. Approval requires a second native folder selection, and a
fresh source/HEAD/native-classifier scan after the dialog. Wrong scope,
changed source, changed model reply, stale register lease and changed original
requirement IDs fail closed and are recorded as **StaleRejected**. A cancelled
folder picker records **Cancelled**. A process interrupted after **Prepared**
will leave no approval: on the next host instance's history load, it is sealed
as **Interrupted**, never replayed or silently retried.

### Original ANSF decision history

New append-only table
\`ansf_project_task_update_review_events\` in the same ANSF SQLite library
stores exactly one immutable prepared event and at most one terminal event per
one-use human review ID. Original SHA-256, predecessor link, sequence,
SQLite row identity, model proof binding, source graph/status, original task
version, operator note and no-authority flags are checked on readback.
An \`IMMEDIATE\` transaction prevents decision replay and checks the latest
register lease during approved decision commit. A bounded history list never
truncates silently and offers independent restart/recovery readback in the
task-list window. Changes to old source tasks and all earlier human corrections
remain in their original PLAN-05 register unchanged.

### Separate human acceptance of an exact requirement

The interface has a second, deliberately separate action
\`lu_project_update_review_exact_requirement\`. It uses a native project picker,
actual current source/status and an OS dialog showing the person's **existing,
possibly human-corrected** original wording/acceptance criterion. It refuses
any source drift since the latest original register revision until the person
uses PLAN-07A's separate source reconciliation. After approval it rescans the
original source and exact HEAD, then appends a single PLAN-05
\`HumanRevalidated\` revision using CAS and the original immutable source
identity. The verified flag describes **only the specific named requirement**,
and only if the independent current native classifier warrants it; it never
states entire task/workflow completion, installed acceptance or execution
authority. Model text is not consulted.

### Tests and remaining acceptance

- Full disposable original ANSF SQLite fixtures: approved/refused/cancelled
  review decisions, source changes during the dialog, one-use denial,
  original preserved user memory, restart recovery of interrupted prepared
  records, tampered event bytes, and historical register SHA unchanged.
- Separate native exact requirement test: refusing unreconciled source,
  applying reviewed source and human wording correction, preserving the
  corrected original wording after exact revalidation, and denying stale
  versions and source changes after the dialog.
- Original-model bound review test: only audited GGUF suggestions may be
  reviewed; mutating original saved reply bytes after preparation denies
  approval and writes only a stale-denial event.
- Windows/Linux grouped \`native-plan07c-review\` exact-commit SHA gate
  includes React/UI contracts, Rust core evidence and model validators,
  ANSF journal/recovery tests, scoped GGUF cancellation and Tauri compile.

**Outstanding independent gates:** actual exact-head compiler/test results;
installed NSIS/deb UIA, operator approve/refuse/cancel, accessibility and
restart on physical Windows; and prior PLAN-07B genuine Qwen GGUF acceptance.
These are not inferred from source code or native unit tests. PLAN-07D
installed/release acceptance has not been begun, and there is no merge or
release authorization.
