# Lu M1 — Memory, evidence, authority and live-binding audit

**Date:** 2026-11-07  
**Source examined:** `vjmichael/Lu-AI-GUI` branch `lu/human-centered-memory-first-philosophy-20261107`, exact parent HEAD `fddc7b9510ac56f80bdb1001040e31e5a3216c3a`.  
**Work order:** M1 from the [human-centered memory-first roadmap](LU_HUMAN_CENTERED_AI_MEMORY_FIRST_20261107.md).  
**Disposition:** M1 **source inventory, wiring and trust-boundary audit completed**. Actual installed-memory end-to-end acceptance is **not** claimed; absence of a verified binding must remain an integration gap. **No source/runtime code or permissions changed.** M2 is not started.

## 1. What the owner wants

Lu is the model's human-centered operating environment. Memory is meant to reduce unnecessary rediscovery of context, tools, past choices and verified procedures. A model should ask concise clarifying questions when they can materially improve the user's goal, incorporate and remember the answer, and continue without resetting its task. **Memory is not authority**; historical content and prior approvals must not become executable grants.

**Owner's UI suggestion:** a compact yes/no decision with an **optional comment field**, rather than forcing every clarification into the main chat stream. For a non-mandatory clarification, include **Not now**; no response means neither yes nor authorization. The UI may be transient, but answers relevant to a task must become **durable, user-origin, attributable task context**. This is a proposal for M2 interaction design, not a UI implementation or authorization mechanism. Do not use this binary UI for ambiguous choices that genuinely require richer responses.

## 2. Actual data paths found in source

| Responsibility | Source-backed implementation | M1 determination |
| --- | --- | --- |
| Desktop chat history (LIVE) | `lu-desktop/src-tauri/src/chat_store.rs` — host-generated `c-` chat IDs, `conversation-index.json`, per-chat `conversations/{id}.jsonl` append journal, event-to-history projection, safe legacy `chats.jsonl` migration | **Live**: `DesktopState::new` opens `ChatStore::open(&root)` in `lu-desktop/src-tauri/src/lib.rs`. User turns and assistant outcomes are recorded there. This is the current operational conversation source, NOT ANSF. |
| Model prompt history (LIVE) | `lu-desktop/src-tauri/src/prompt_context.rs::typed_history`, invoked by `lib.rs` during generation | **Live, narrow**: current chat history only, max 16 previous turns and 15 KiB history budget; interrupted assistant output is omitted. No ANSF or cross-chat source retrieval at this boundary. |
| ANSF conversation books (CORE ONLY) | `src-tauri/lu-core/src/ansf.rs` — per-book SQLite, immutable entries, session/chapter structure, FTS5, cursor reads; original content remains independently retrievable | **Source implemented, not shown wired to desktop runtime.** No `AnsfStore` field/open/read call in the reviewed desktop `lib.rs` path. Do not substitute an ANSF search result for the authenticated chat event. |
| Global ANSF index (CORE ONLY) | `src-tauri/lu-core/src/ansf_library.rs` — separate `library.db` reference catalog, session/chapter FTS5, local `relative_path` metadata | **Source implemented, not live model-facing retrieval.** This index does not open source books or authenticate users. Full source resolution must happen through host-validated ownership and confined paths. |
| Revisions and source changes (CORE ONLY) | `ansf_revisions.rs` — append-only revisions and `VerifiedUser` vs `ImportObserved` provenance; `ansf_import.rs` — streaming **Jan** JSONL importer; `ansf_sync.rs` — book/catalog reconciliation | **Implemented components**, not verified as active desktop chat/memory plumbing. Existing importer targets Jan message records, not automatically the **Lu-owned ChatStore event format**; a tested adapter and stable ID mapping will be needed. |
| Runtime task state (LIVE, separate) | `src-tauri/lu-core/src/tasks.rs` and `lu-desktop/src-tauri/src/lib.rs` — `TaskStore::open(root/lu-background-tasks.sqlite)` with checkpoints/heartbeats, status and restart reconciliation | **Live task ledger**, not conversational/semantic/procedural memory. Must remain authoritative for task status; summarization cannot rewrite it. |
| Effect intent/recovery (LIVE, separate) | `src-tauri/lu-core/src/journal.rs` and desktop `lib.rs` — `EffectJournal::open(root/native-effects.sqlite)`; PREPARED/EXECUTING/APPLIED/FAILED/UNCERTAIN | **Live effect journal**, never replay an UNCERTAIN side effect from a memory suggestion. |
| Tool/model audit evidence (LIVE, separate) | `src-tauri/lu-core/src/model_turn_journal.rs`, `lu-desktop/src-tauri/src/workspace_desktop.rs` and `lib.rs::lu_tool_turn_history` | **Source-integrated for selected Workspace/Reader/Git turns** with separate audit SQLite paths. An audit record is evidence, not a saved procedure or fresh approval. |
| Short-lived source reuse | `src-tauri/lu-core/src/ai_operations.rs::SourceReuseIndex`, constructed in desktop `lib.rs` | Process-memory, scoped source digest/epoch cache; **not** persistent long-term procedural or semantic memory. |
| Observe & Learn to ANSF | `src-tauri/lu-core/src/native_observe_ansf.rs` — reviewed-test publication helper requiring trusted review/ownership | **Inert Core helper until proven host-wired**: the reviewed `lu-desktop/src-tauri/src/observe_desktop.rs` does not call it. Observer privacy/consent must not be bypassed to fill memory. |
| Desktop ownership | `src-tauri/lu-core/src/desktop_owner.rs` — exclusive SQLite writer lease | Supports single-instance host fencing; **not** a per-user/per-book authorization implementation. |

### Existing trust path we must reuse

`lu-desktop/src-tauri/src/chat_store.rs::trusted_user_turn_text` verifies exact durable host-generated `c-` conversation and `g-` turn ID, refusing duplicate/corrupt records. `ChatStore` implements `TrustedUserTurnVerifier`; `workspace_desktop.rs::PersistedWorkspaceTurn` additionally pins conversation, generation and expected prompt before granting read-only tools. Existing `conversation_authority.rs` requires a verified user-origin turn. **Do not create a second ANSF-based authorization path**. A recalled user sentence, search hit, quoted text, summary, procedural record or model output must never be allowed to become a current user request.

ANSF `ensure_session` validates session/workspace IDs; its caller must still authenticate owner and filesystem scope. `AnsfLibrary` stores references, not the actual book contents. Neither per-record SQLite indexing nor local presence of data proves authorization.

## 3. Functional gaps in K/I/A/F/M and the user loop

1. **Memory lifecycle gap (M):** A single real human turn is stored in the Lu JSONL chat journal. There is not yet an evidenced, durable, crash-recoverable mapping to an ANSF `session/chapter/entry` nor an ANSF-backed retrieval path into the active model prompt. Calling ANSF implemented does not resolve this.
2. **Source identity gap (F):** Need stable mappings for chat ID, user-turn/generation ID, message event kind and revision, with deterministic duplicate/retry behavior. Avoid copying sensitive/irrelevant raw tool records into summaries. Never import the legacy Jan schema as if it is Lu's current chat event schema.
3. **Search relevance gap (I/M):** ANSF supports original-message local FTS5; the library supports book/chapter lookup. A trusted host still must authenticate and read exact supporting entries, freshness/revision state, and inject bounded excerpts with provenance and “context only” status. This is M3 work, not M1.
4. **Goal and clarification gap (U/K/M):** No proven durable goal/assumption/decision/clarification lifecycle integrated with the normal Send path. Questions, user answers, rescoped goals and pending state need an explicit record and correctness tests. A **Yes / No / Not now + optional comment** can be one review control, not a permanent approval token.
5. **Procedural experience gap (M/F):** Evidence and effect journals exist, but no proven pipeline promotes **independently verified** successful procedures into versioned searchable memory, handles superseded application versions or invalidates incorrect remembered advice. M4 work.
6. **Authentication boundary (A):** Desktop single-profile `ChatStore` checks chat and exact human-turn identity. ANSF and ANSF Library APIs assume a trusted caller but do not supply the full host user/ownership/path-binding boundary themselves. Future binding must verify the local account/profile/workspace and current task scope before reading cross-chat data.
7. **Multi-DB durability gap (F):** Chat JSONL, ANSF books, library index, model audit, tasks and effect journal have separate persistence. There is no cross-store atomic transaction. Choose an authoritative event source, idempotent derived updates and startup reconciliation; incomplete index writes cannot silently create false facts.
8. **Installed evidence gap:** Native source/test suites exist, but a real Windows installed Lu with a loaded GGUF has not been shown by this audit to recall prior-chat facts, ask a useful clarifying question, store the answer and use it after restart while refusing stale authority.

## 4. M1 outcomes and decisions

- **One canonical live turn authority:** retain the existing authenticated `ChatStore` event and host turn ID as the source of human intent and current permissions; **ANSF and the library are derived/reference sources**, not replacement authorities.
- **No second memory database for M2:** build on ANSF SQLite and its library rather than creating a parallel “agent memory” for user goals. Keep task/effect journals separate and authoritative for state/side effects.
- **No silent mutation or import:** append source-linked events and revisions; deduplicate by stable identity, preserve originals, and distinguish directly confirmed user answers from inferred or imported context.
- **No automatic model-facing search grants:** memory retrieval is scoped to user/profile/task and current conversation; foreign/cross-chat sources require an ownership and source-selection policy. Search results are reference data, not a new effect grant.
- **No broad execution expansion in M1:** existing P01–P12 safe tools stay intact, as does the existing execution approval architecture.
- **No M1 app feature claim:** evidence mapping is complete; actual memory-to-model integration is an explicit future milestone.

## 5. M2 implementation entry contract — start only on subsequent instruction

M2 objective: **durable user-goal, clarification and task-state continuity** using the authoritative desktop chat event plus trusted ANSF/TaskStore mapping; user questions need not all occupy the chatbox.

Recommended minimum working record, as an *illustrative schema* (NOT code):
- `chat_id`, `goal_id`, `origin_user_turn_id` and scoped `task_id` (all host-issued);
- `goal_text`, current constraints, declared assumptions, outstanding decisions;
- `question_id`, question kind (`yes_no`, `choice`, `open_text`), supported choices, optional comment, pending/answered/declined status and answer's exact authenticated event reference;
- version/revision, evidence pointers, updated timestamp and explicit `confirmed | inferred | superseded | uncertain` labels;
- **no** bearer token, executable path, ambient capability or stored approval grant.

Clarification handling: Lu may recommend a yes/no question with optional comment and **Not now** for nonmandatory matters. The UI must present a clear question, distinguish a substantive answer from merely dismissing, preserve the human's explicit reply with provenance, and update only the affected goal dimension. Host action authority remains in the current per-effect permission workflow, not the clarification response by itself.

M2 acceptance examples: ask a useful clarification once; answer using the small control; reopen chat/restart and recover the exact answer; preserve independent earlier constraints; recognize revised user intent; do not repeat already answered questions; reject model/tool-forged answers; ensure no ambiguous or stale memory grants execution. Direct conversation should remain available and fully functional if the compact UI is unsupported.

## 6. Evidence references and status

Reviewed branch `fddc7b9510ac56f80bdb1001040e31e5a3216c3a`:
- Desktop: `lu-desktop/src-tauri/src/chat_store.rs`, `lib.rs`, `prompt_context.rs`, `workspace_desktop.rs`, `observe_desktop.rs`.
- Core ANSF: `src-tauri/lu-core/src/{ansf.rs,ansf_library.rs,ansf_revisions.rs,ansf_import.rs,ansf_sync.rs,native_observe_ansf.rs}`.
- Authority/state: `src-tauri/lu-core/src/{conversation_authority.rs,desktop_owner.rs,tasks.rs,journal.rs,model_turn_journal.rs,ai_operations.rs}`.
- Existing owner roadmap: `docs/LU_HUMAN_CENTERED_AI_MEMORY_FIRST_20261107.md`, `src-tauri/lu-core/MEMORY_AND_MEDIA_ROADMAP.md`.
- Existing fixture tests: `src-tauri/lu-core/tests/{ansf.rs,ansf_library.rs,ansf_revisions.rs,ansf_import.rs,ansf_sync.rs,effect_journal.rs,model_turn_journal.rs}` and desktop native-chat tests.

**M1 source/authority audit:** COMPLETE; **live ANSF retrieval integration:** NOT IMPLEMENTED/NOT EVIDENCED; **M2 work:** NOT STARTED; **physical/installed-app memory E2E:** OPEN. No runtime/source implementation change or native execution test is claimed in this documentation-only work order.
