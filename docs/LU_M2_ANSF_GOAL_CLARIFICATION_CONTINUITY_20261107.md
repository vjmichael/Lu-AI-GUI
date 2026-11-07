# Lu M2 — ANSF/SQLite goal and clarification continuity

**Owner-approved development:** November 7, 2026.  
**Branch:** `lu/m2-ansf-goals-clarifications-20261107`, stacked on M1 draft PR #89.  
**Canonical decisions:** [Human-centered AI and memory-first design](LU_HUMAN_CENTERED_AI_MEMORY_FIRST_20261107.md) and [M1 live-binding audit](LU_M1_MEMORY_EVIDENCE_AND_AUTHORITY_AUDIT_20261107.md).

## Outcome and what M2 is

M2 introduces **working native goal/clarification memory** into the ordinary Lu desktop, backed by a real `AnsfStore` SQLite database **in the same profile-owned app-data folder**, with original user-origin events journaled in the existing production `ChatStore`. It does **not** silently migrate all chat messages to ANSF or create a second standalone “agent memory” product.

**Why Protobuf and JSONL coexist:** Lu's Protobuf structures provide the **typed model/tool IPC transport** and canonical message boundaries. The live desktop's JSONL remains a **durable chat event journal** and the authenticated origin of a current user turn. ANSF/SQLite is a **persistent indexed memory book** and stores a separately validated projection of user-goal events. The trusted host translates between these roles; serialization formats are not interchangeable.

## Actual source implementation

### ANSF Core (`src-tauri/lu-core/src/ansf_goals.rs`)

- New `ansf_goal_events` table initialized inside the **existing ANSF database schema**, referencing existing `ansf_sessions`, with a unique immutable host-generated `event_id`, source kind, canonical JSON and SHA-256 source digest.
- `GoalEventKind::{SetGoal,AskQuestion,RespondQuestion}` as typed records. The `RespondQuestion` choice is exactly `yes | no | not_now` with an optional bounded comment.
- A deterministic `project_goal_events` reducer implements active goal, explicit revisions, pending/deferred/answered/superseded clarification states, immutable answered questions and question caps.
- `not_now` means **deferred**: not an answer, refusal or authorization. A later yes/no may answer that question. Goal revisions automatically supersede pending/deferred clarifications, not previously answered historical records.
- Idempotent mirror by stable source identity and digest; duplicate exact replay succeeds, conflicting duplicate is refused, invalid event chains and malformed IDs fail closed.
- Existing `AnsfStore` book/session code is reused. No separate memory format/store or model-facing direct SQLite API.

### Live desktop source (`lu-desktop/src-tauri/src/chat_store.rs`)

- A new `GoalMemory` journal event is saved **first** to the existing per-chat append-only JSONL event log, with IDs minted by trusted host code.
- Memory events are validated against the full prior conversation-scoped event chain while the actual `ChatStore` lock is held. They do not become `Event::User` or `Event::ReaderUser`, are omitted from ordinary transcript history, and **cannot** satisfy `TrustedUserTurnVerifier`.
- The existing chat/index, user-turn authority, legacy message migration, files and effect journals remain unchanged.

### ANSF desktop service (`lu-desktop/src-tauri/src/memory_desktop.rs`)

- Opens `lu-ansf.sqlite` only through the profile-owned native host, refusing existing symlinks or nonregular files. The existing `DesktopOwner` exclusive lease is held at startup. **Failure to initialize the optional ANSF memory does not prevent ordinary alpha chat from launching**; memory-specific commands return a clear error and the model receives no falsely claimed remembered goal.
- On read/after an event, verifies the actual `ChatStore` conversation and reconciles every source event into the ANSF SQLite book. Partial publication can be retried after restart.
- Compares the entire source event sequence against the ANSF result; unexpected, missing or conflicting records are refused, not silently promoted to memory.
- Four Tauri commands: `lu_m2_goal_context`, `lu_m2_set_goal`, `lu_m2_open_question`, `lu_m2_answer_question`.
- **Ordinary local-GGUF Send** includes a bounded serialized task-memory excerpt assembled from a verified ANSF view: current goal and up to four latest *answered* clarifications. Memory is labeled context data, not authority. Previous `typed_history` and its chat/provenance limits remain unchanged.
- The user can change a goal or answer a question without sending an extra chat message. User-origin state remains persisted and scoped to that chat.

### Creator-facing UI

- Compact collapsible “Goal & questions” section in the desktop chat area.
- Save/edit goal, stage one short question, answer **Yes**, **No**, or **Not now** with optional comments. The interface is separate from the message composer.
- Refreshes the host-owned memory view on conversation change, and persists it through a restart; question status is displayed without converting the answer into a tool grant.
- **Scope limitation:** M2 questions are **user-reviewed/staged**. The local model is not authorized to directly mint a question card by emitting arbitrary prose, JSON, tool responses, web content or scripts. Model-suggested typed clarification admission, smarter “when to ask,” and cross-chat retrieval belong to later explicitly approved work. This keeps M2 useful without creating a covert control channel.

## M2 security and reliability invariants

1. The `ChatStore` journal remains the trusted identity source for user-origin task context. ANSF is a derived SQLite index/projection, **not** a second authority for current user approvals.
2. A user's yes/no answer expresses an answer to a clarification **only**, not consent to read or write a file, execute code, contact the network or control an application.
3. Source text, model messages, browser observations, imported Jan records, summaries and recalled procedures cannot forge a `GoalMemory` user-origin event.
4. Memory is scoped to the existing conversation and current desktop profile. No implicit cross-chat search or privacy-unsafe global memory expansion.
5. A missing or corrupt ANSF mirror blocks that memory view; the source event remains durable for reconciliation. Conflicting/missing source identity must never be quietly ignored.
6. If a goal changes, old pending questions become superseded. A deferred question remains unanswered and can be reconsidered later.
7. The model receives **data** about the current goal, never a system-role tool grant. Trust and executable effects continue through the existing user-turn authorization and native tool policy engine.
8. No new shell/network/worker permission, silent content capture or automatic external-app execution was introduced.

## Test and acceptance plan

- Unit: ANSF schema initialization, valid and invalid events, revised goal, deferral, later answer, duplicate identity rejection, and source-digest mismatch.
- Portable real-source integration: ChatStore JSONL → ANSF SQLite, restart/replay, isolation between chats, memory events not satisfying human authorization, and no duplicate answer.
- Desktop Rust source: service restart, answer persistence and no tool grant; React/source contract covers a real selectable yes/no/not-now/comment experience.
- Dedicated GitHub Linux/Windows native matrix and existing desktop frontend+Windows cargo-check workflow.

**Release gates (must not be misreported):** exact-head native CI and Windows desktop compile must pass; subsequent installed Windows Lu with a real GGUF must create a goal, answer and defer clarifications, restart, reopen, switch chat, verify appropriate context fed to a model, and deny memory-based execution authority. Linux installed-app support must be assessed independently.

## What remains for M3–M5

- **M3:** bounded, source-attributed relevance-based retrieval across the ANSF book/library into active model turns, freshness and cross-chat ownership policies, and context shaping.
- **M4:** independently verified, versioned semantic and procedural memory with correction, contradiction handling and invalidation. Do not promote unverified model replies to verified procedures.
- **M5:** actual installed, end-to-end acceptance of memory usefulness and safety under model switch, crash/restart, stale state, human correction and revoked permissions.

**M2 code and focused tests are submitted; code existence or queued CI is not release acceptance.** Do not start M3 automatically.
