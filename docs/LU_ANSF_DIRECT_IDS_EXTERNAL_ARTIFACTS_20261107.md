> **Benchmarking direct IDs:** [Three-arm real GGUF comparison](LU_ANSF_THREE_ARM_REAL_MODEL_BASELINE_20261107.md) exercises the actual ANSF indexed source-ID resolver from a headless Lu Core adapter and compares it to no-memory and bounded recent-chat conditions under identical model settings. The CI protocol explicitly fails an ANSF no-gain/regression hypothesis; its result is not an installed-app acceptance or a comprehensive M3/M4 claim.

# ANSF direct source IDs, infinite-history navigation, and external multimodal references

**Owner refinement:** November 7, 2026. Stacked on M2 draft PR #90.  
**Scope:** source-implemented ANSF **Core** primitives and acceptance. No claimed installed-desktop ANSF migration, model-facing tool, or complete artifact broker.

## 1. Original intent reaffirmed

The original ANSF books, chapters, pages and Library are useful chiefly to make arbitrarily long conversation histories **human-browsable, auditable and practical to manage on disk**, while never discarding exact originals because the model's context window is full. The history should **feel effectively infinite** through cursor-based, bounded view loading and retained source. A “page” is a chunk of history, not a new source of truth or a mandatory retrieval hop.

**For models, a known stable source ID should resolve in one host/model request.** Three moves remains an upper-level discovery heuristic for an unknown reference, e.g. Library search → book/entry search → exact read. It is neither an obligatory three-query protocol nor a guarantee that ambiguous search always succeeds in three operations. No new selective-loading engine is part of this work; prior ANSF paging and bounded context features are reused.

### Direct ID contract — source implemented

`src-tauri/lu-core/src/ansf_source_ids.rs` implements:
- An opaque, versioned, canonical `ansf:v1:<base64url(session-id)>:<entry-id>` identifier. The session ID and exact SQLite entry ID uniquely locate a source even if the book later moves between disk directories. The identifier is *not* a file path.
- `AnsfSourceId::new / parse / encode` and `Entry::stable_source_id`.
- `resolve_known_source(source_id, active_conversation, library, host_access, open_authorized_book, budget)`: **one callable host-side operation** using already-implemented `AnsfLibrary::get_book(session_id)` and `AnsfStore::read_entry(session_id,entry_id)` indexed lookups. It does not invoke search, scan chapters or create a new database.
- Host-supplied, principal-aware `HostSourceReadAccess`, a trusted book opener, and a bounded result (max 64 KiB; original source role remains historical metadata and is never promoted to current conversation/tool authority).
- Tests for canonical round trips, malformed IDs, restart persistence, cross-chat access denial, absent entries and size limits.

**Deployment gate:** the normal installed desktop still needs the owner-approved **canonical ANSF conversation migration** and a model/host-facing read API connected to the active chat user-turn verifier. The Core resolver is not yet registered as an automatically executable model tool, and it must not be described as working in the installed app until separately verified.

## 2. Artifact and multimodal storage stays outside ANSF

**ANSF is the conversation memory database, not a video/image/audio warehouse.** User files may stay at their original approved locations; optionally Lu may later manage content-addressed copies outside ANSF. There is no implicit copy or duplication requirement.

Only small, typed conversation **reference metadata** is kept in ANSF:
- The original conversation/session ID and immutable source entry ID.
- An opaque host-issued artifact ID that resolves to a managed object or an external user-selected file in a separate host-owned registry.
- Display name, declared media type and optional expected byte length and SHA-256.

`src-tauri/lu-core/src/ansf_artifact_links.rs` adds `ansf_artifact_links` metadata to the **existing** ANSF SQLite schema and provides `append_artifact_link`, `artifact_links_for_entry` and `probe_granted_file`. Linking the same exact artifact record twice is idempotent; a conflicting replacement is refused. A link can only be attached to an existing original ANSF conversation entry; binary payloads are never placed into this table.

### Link verification semantics

The trusted desktop host **must first identify and authorize the correct file**. The Core probe accepts an already-open OS file handle:
- **Unavailable / access required:** host cannot open or has not received an approved handle; no model-directed path opening is attempted. Not claimed to be verified.
- **PresentUnverified:** host opened a file and performed cheap metadata checks; identity is not proven, or the file is larger than an explicitly bounded hash budget.
- **ContentVerified:** the complete file content matched the pinned SHA-256 within the explicit limit.
- **Changed:** observed file size or complete digest disagrees with the recorded expectation.

Hashing inline is capped at **64 MiB**. Large multimodal media should use a separately supervised streaming hash/checkpoint worker when full content verification is required; don't block a local model turn to rehash terabytes. Merely finding a file at the same path or matching a filename is insufficient to assert byte-identical source. File identity protections and cross-workspace ownership remain host responsibilities.

The Core implementation **does not yet include** a production artifact-ID-to-host-file broker, automatic file picker, content-addressed artifact store, missing-file UX or installed application binding. These are explicit integration gates, not features implied by the existence of the metadata schema.

## 3. Scale and original navigation

- **Known source:** parse ID → authenticate source/book → open verified book → direct indexed entry read. One API operation (some internal indexed queries).
- **Unknown source:** discover by Library/FTS5 with source title and scope → inspect candidates → exact original using the resulting ID, usually within three navigational steps, not a hard cap on reasoning.
- **Human:** Library → book → chapter → virtualized pages. Page count can grow indefinitely subject to real disk/resource limits; don't impose arbitrary fixed maximum messages per book merely to claim “bounded.”
- **Large media:** store out of database; ANSF retains the exact conversation and proof-bearing links. A missing external file is reported as unavailable, never silently recreated or hallucinated.
- **Persistence:** book and Library indexes must be recoverable from exact source history. The source ID's meaning does not change when a book is reindexed or moved.
- **Memory semantics:** extracted facts, verified procedures, source relationships and belief revisions must point back to exact source IDs and respect ownership; retrieval never revives previous tool permissions.

## 4. K/I/A/F application

- **K:** model recovers precise historical decisions and evidence without rereading huge conversations.
- **I:** source-ID direct reads plus contextual search as a fallback, with explicit external artifact checks.
- **A:** host validates owner, active task, book access, separate file grant and read budget. An ID is not authority.
- **F:** exact original content, source provenance, stale/missing/changed artifact status and later verifiable correction history.

The user defines the objective; the model does not need to know where the files are physically stored, and the UI does not require every record to fit in RAM.

## 5. Relationship to M2–M5

- **M2 closeout:** migrate the live JSONL `ChatStore` to authoritative ANSF conversation records. Do not prematurely substitute the proposed ID API for the missing desktop migration.
- **M3:** make the direct-ID operation discoverable to the model in **one turn**, alongside bounded search and context assembly, and display source IDs in suitable results. Reuse existing selective loading rather than rebuilding it.
- **M4:** link semantic/object/procedural memory and verified experience back to immutable source IDs.
- **M5:** demonstrate long-history continuity, direct-read hit rates, exact source fidelity, verification after moving/deleting media, cross-chat access control and installed-model use.

**No new archive database, vector service, media embedding, forced book rollover, ambient OS access, network grant or generic process execution was added by this work.**
