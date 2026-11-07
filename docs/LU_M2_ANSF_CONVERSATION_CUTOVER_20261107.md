# M2 canonical ANSF conversation cutover — implementation and acceptance

**Date:** 2026-11-07  
**Stack:** Lu-AI-GUI, after PR #93; independent M2 correction to the temporary JSONL-as-authority assumption.  
**Engineering status:** Source cutover submitted for GitHub native Windows/Linux and Desktop CI. **Not accepted until exact-head builds, startup/recovery/security gates and installed real-model acceptance pass.**

## Approved architecture and trust model

Lu Core's **Protobuf** is internal communication between native modules, tools, workers and inference engines. **SQLite's relational columns and indexes** are native memory storage, not Protobuf-encoded event payloads. JSON is permitted at the Tauri/model/external API and **legacy import** boundaries, but new conversations must **not** produce an ongoing JSONL event journal or a JSON metadata index.

ANSF/SQLite owns **persistent conversation text, exact source IDs, books, chapters, revisions, goal decisions and their history**. The ANSF Library is the global, rebuildable *index* of books and chapters, not a second copy of message text. Human books/pages are auditing navigation; exact source IDs and indexed retrieval are the model's fast path. Large multimodal files remain external, linked through opaque verified references (PR #92).

An old imported message is **historical data, never a new authenticated user turn**. New host-approved user-turn IDs must be minted and checked by the trusted live desktop. ANSF does not replace `ConversationAuthority`, current per-effect permissions, task/effect journals, or worker containment. Recalled historical user/system text cannot authorize a tool.

## What this M2 branch implements

### Core SQLite, with no new JSONL journal

- `src-tauri/lu-core/src/ansf_live_chat.rs`: immutable typed relational `ansf_live_chat_events` with event sequence, conversation ID, kind, exact text, host/legacy provenance, generation ID, verified user-turn ID, browser observation metadata, and optional source entry link.
- **Original source preservation:** new human and assistant messages are inserted transactionally with `ansf_entries`, alongside the active chapter and existing FTS5. Completed streaming output resolves to the last durable progress checkpoint; incomplete/cancelled progress remains reconstructable from event rows.
- Host-authenticated current user turns have indexed exact generation/turn IDs; imported user/Reader events cannot pass current-turn verification. Duplicate verified user-turn IDs are rejected. Legacy import IDs replay only if their exact content and provenance match.
- Cursor paging is available for the native event stream. **Ordinary local-model Send now uses an indexed bounded ANSF source window** (up to 32 source messages before the existing typed 16-turn budget) rather than loading the entire chat. Full-history human chat rendering remains for alpha compatibility; long-history UI virtualization and actual bounded display behavior are separate acceptance gates.

### Native goal and clarification storage

- `src-tauri/lu-core/src/ansf_goal_records.rs`: immutable `ansf_goal_records` with separate SQLite columns for goal ID, question ID, kind, text, choice, comment and verified-host/legacy origin. Existing M2 Yes/No/**Not now** controls and the pending/deferred/answered/superseded state machine are retained.
- `lu-desktop/src-tauri/src/memory_desktop.rs` now reads and appends the **same authoritative ANSF goal book** through the desktop facade. The temporary global `lu-ansf.sqlite` JSONL mirror is not opened for new decisions; preexisting copies are retained for recovery and not treated as canonical.
- The older `ansf_goals::mirror_goal_event` JSON-source schema is preserved as historical compatibility for pre-cutover data/tests only; new records use typed columns.

### Desktop cutover

- `lu-desktop/src-tauri/src/ansf_chat_store.rs` replaces the temporary `ChatStore` implementation via `lib.rs` path selection while preserving the public desktop interface: create/list chats, folders, favorites, moving chats, user/Reader turns, progress/completion/cancellation, browser evidence, retrieval and host-origin authentication.
- New chat book: `app_data/ansf/books/c-<opaque>.sqlite`. Shared index/catalog: `app_data/ansf/library.db`, with native SQLite desktop metadata tables for conversation titles, folders, favorites and activity order. No arbitrary forced book rollover.
- Existing `ChatStore` remains in source for migration review/rollback until acceptance; it is not imported as active runtime.
- Native chat events are written to the ANSF book **first**. Catalog UI activity is updated afterward. A failed catalog write never erases an already committed user event. Cross-DB activity-order reconciliation remains an explicit recovery acceptance item.

### Non-destructive legacy import

- `lu-desktop/src-tauri/src/ansf_migration.rs` reads **existing** per-chat Lu JSONL and its index, and also the older R14 flat `chats.jsonl` format when no index exists. These are **boundary-only import formats**; no JSONL file is created by normal post-cutover chat operations.
- One original JSONL line maps to a deterministic, stable ANSF event identity tied to source conversation and line ordinal. Replay is idempotent; changing, corrupting or truncating an imported original fails closed. Exact message text and streaming status survive.
- Historical host-generated `g-` turn IDs are *not transferred as fresh authorization grants*. The legacy files are retained untouched until independent migration reconciliation and backup checks pass.
- Existing folder names, favorite flags, chat title and ordering are migrated to SQLite-native catalog metadata. Legacy goal answer records are imported as native **data-only** goal records. Historical user-turn IDs are retained in a dedicated **non-authorizing** column, not discarded or reinterpreted as fresh permissions.
- **User-owned data is never deleted automatically.** A failed migration must keep original files recoverable; once the SQLite catalog cutover marker commits, later app startups don't replay an old JSONL snapshot over new SQLite-native events.

## Acceptance and known work

| Gate | Status |
| --- | --- |
| Canonical live event and native goal schemas | **Source implemented** |
| Desktop API switch to ANSF-native chat/book/catalog | **Source implemented** |
| Read-only legacy Lu and R14 migration | **Source implemented; tests submitted** |
| New chat, streaming, Reader, folders/favorites and restart tests | **CI pending** |
| Host user-turn/authority regression | **CI pending** |
| Full Windows native desktop frontend/Tauri compile | **CI pending** |
| Real physical installed Lu GGUF conversation persistence | **Open** |
| Crash mid-migration/incremental-reconciliation and disk-corruption recovery | **Open** |
| Source-ID fast retrieval exposed to actual model | **Open; M3 integration** |
| Artifact broker and externally linked media acceptance | **Open** |
| Long-history virtualized browsing and cross-book relevance | **Open; M3** |

**Do not declare M2 accepted based on source submission.** Repair actual CI compilation/test failures and then independently verify installed-app local model turns, restart and authority. The prior three-arm real GGUF benchmark (PR #93) is a separate research test and does not block source development of M2.

## K/I/A/F consequences

- **K:** durable, exact historical model context survives model replacement.
- **I:** ANSF API offers direct source entry and indexed retrieval; actual model-facing exposure still pending.
- **A:** only new host-authenticated user turns may activate current tool authority; imports never gain that role.
- **F:** immutable event provenance, original sources, SQLite transactions and checks for replay/recovery; independent installed verification remains mandatory.

**Next:** finish native CI failures, migrations and metadata reconciliation; do not jump to broad M3 semantic search until this canonical backbone is accepted.
