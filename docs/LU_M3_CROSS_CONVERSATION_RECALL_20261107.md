# M3 ANSF cross-conversation original-source recall

Date: November 7, 2026. Base: M2 PR #94, exact commit 4cfc7ddb0344bff594b1decbfaa88d62e7c52441.
Status: source implemented in a separate draft. Exact-head native and genuine installed-app acceptance are OPEN.

## Architecture and semantics
The original ANSF SQLite book is authoritative for message bytes; the Library is a metadata index. No new parallel transcript database, vector database, or copied message-text catalog is introduced. Protobuf remains the tool/worker IPC boundary. Multimodal bytes remain external to ANSF.

M3 adds a bounded lexical, not semantic, search of up to 24 recent other chats. It uses the original ANSF book FTS5 indexes, at most six literal sanitized ASCII query terms and four candidates per book. At most three cross-chat sources are selected, one per book, using exact word overlap and recency. Two matching terms are required for multi-term queries. Human-supplied attached-file text is excluded from the recall query.

A user-provided canonical ansf:v1 source ID bypasses discovery and opens its original entry through the catalog's trusted source-book validator. Invalid IDs, missing, oversized, or out-of-profile sources are rejected. The source ID is not authorization. The host injects a bounded JSON-encoded historical-data context only for human, text-only local-model inference. Original history is never promoted to a system/user role, tool grant, or verified fact.

The desktop also gains a human-searchable Recall panel with exact source ID links to immutable originals. Incomplete assistant progress, Reader/worker tool roles and unverified arbitrary paths are excluded. The model may cite stable IDs, but citations must not be misrepresented as independent factual verification.

## Verification
The production portable native chat harness tests cross-chat retrieval, stable IDs across process reopen, same-profile source checks, no current-chat authority from another chat, incomplete output exclusion, attachment injection suppression, and malformed FTS syntax. Dedicated Windows and Linux native CI, a UI contract check, and Windows host compilation are provided.

## Open gates
- Actual exact-head Windows/Linux compilation and executed native tests, failure repair.
- Installed NSIS with hash-pinned genuine llama.cpp engine, local GGUF, visible WebView recall, close/restart and legacy migration.
- Precision/recall relevance benchmarks against no-memory and conventional recent-history baselines.
- Scalable selective index beyond the 24-book bounded scan, without duplicating originals.
- Long-history cursor-paged virtualized GUI rendering; currently chat rendering still reads entire history.
- Revision/conflict semantics and externally linked media broker, independently verified.

No M2 release gate is reclassified as passed by implementing M3.

## M3-S semantic retrieval enhancement (November 7, 2026)

S1 versioned source-bound embeddings and S2 bounded FTS+vector reciprocal-rank fusion are implemented as native **opt-in interfaces** with synthetic-vector tests; original source/authorization remains unchanged. See [M3 S1–S4 work plan](LU_M3_SEMANTIC_S1_S4_20261107.md). There is **no bundled embedding model, live semantic inference, installed acceptance or real-model baseline advantage yet**. Default automatic Lu Send continues M3's lexical recall; S3 embedding workers and S4 three-arm real-GGUF evidence remain open. No ANN extension is bundled. All retrieved entries still go through original ANSF source validation.

## Original ANSF/SQLite design fidelity: pre-existing requirements, not M3 discoveries

Earlier October 25-26 Lu decisions already established these requirements. This section corrects the M3 ledger; it does not claim new implementation.

1. **Canonical binary storage, human-readable pages:** SQLite ANSF books own immutable original message text, chapter/page navigation, and revision links. Large flat text transcripts are not the storage or model retrieval interface. Lu renders human-readable pages without changing the canonical binary database.
2. **Independent searchable history pane:** search results and historical pages open in a separate navigation/viewing pane that does not replace, navigate, overwrite, or reset the active chat, its input draft, scroll position or current inference job. Exact source IDs permit non-duplicating linked reads and optional source transclusion.
3. **Effectively infinite-looking active conversation:** stable-ID cursor pagination, virtualized rows and preserved scroll anchors load nearby pages on demand; offscreen rows are released from memory. Neither WebView RAM nor model context needs the whole transcript. This is staged loading, not forced book rollover.
4. **Memory types by retrieval policy:** short-term/working context, full episodic ANSF original source history, semantic and procedural knowledge with verified source provenance and revision/supersession, and operational/continuity records for long-running tasks. No competing master transcript database.
5. **Operational cross-chat continuity:** an ANSF-referenced, trusted host-owned asset and development ledger records verified packages, build environments, SDK/MSVC/NSIS/LLVM/Rust toolchains, file locations, versions, hashes, acquisition provenance, commands, exact source commit, test receipts and blockers. On a new chat the host REVALIDATES files and hashes, reuses what still exists, and downloads/unpacks/rebuilds only what is missing or stale. A previously present package is not automatically considered currently available.
6. **Durable task checkpoints:** a new conversation or new model resumes the last project task, completed operations, unresolved dependencies and intended next action from source-linked operational memory, rather than rereading giant chats and repeating tool calls. Historical text/permissions are never current-turn authority.
7. **Separate external artifact bytes:** multimodal files, SDK ZIPs, binary packages and installer artifacts stay outside ANSF. Store only bounded metadata and authenticated, resolvable references back to the original source/evidence.

### Implementation delta, not new roadmap
- **Source currently present:** ANSF-native SQLite originals and Library, stable source IDs, old-message migration, bounded current model history, basic cross-chat lexical matches, a preliminary inline Recall UI, S1 rebuildable vector projections, S2 hybrid retrieval interface.
- **NOT DONE / high priority:** independent search pane (the existing prototype renders within the main chat layout, not the approved independent-pane UX); actually virtualized long transcript scrolling (current history reader materializes the entire conversation); reliable short/episodic/procedural continuity projections; host-owned persistent operational asset and build-recipe inventory; real resume-without-rediscovery behavior for NSIS/Windows SDK/MSVC dependencies; fully installed physical Windows acceptance.
- Semantic S3/S4 may improve retrieval but must not replace these previously accepted needs. Upgrade the UI and continuity functionality as original requirements instead of repeatedly proposing them.

### Required scenario acceptance
A. A very long chat scrolls smoothly by fetching original ANSF cursor pages and preserving the exact live composer, scroll position and generation state when older entries open in an independently navigable search pane.
B. A fresh Lu conversation asks to resume a Windows NSIS build. Lu retrieves existing durable SDK/MSVC/LLVM/NSIS manifests, verifies path/version/digests in the current environment, and performs ONLY the missing setup/build operations. No blind re-downloading, re-extracting or re-discovering previously supplied packages.
C. After model replacement or app restart, Lu recovers a paused task with completed operations, exact source IDs and outstanding dependencies, but without inheriting tool authorization from historical conversation events.
D. Baselines quantify duplicate package lookups/downloads, repeated environment setup, redundant search calls, interrupted-work resumption time, false memories, retrieved-source precision, latency and disk/RAM costs.

This is a correction to preserve the ORIGINAL design; not a statement that these gaps are closed.


## M3 original-UX recovery — independent History Library pane (source candidate 2026-11-07)

Implemented source: `AnsfStore::page_before` uses stable original-entry IDs and descending SQL LIMIT with oldest-page cursor; `ChatStore::original_history_page` authenticates both active and source book and returns at most 40 user/assistant original entry previews; `original_source_text_page` retrieves the exact original in up to 4096-character chunks. Tauri registers `lu_m3_history_page` and `lu_m3_source_text_page`. React now renders a **secondary independently navigable ANSF History Library overlay** with conversation selector, cross-chat lexical search, older-page navigation, original-source read continuation and scroll-anchor preservation on prepend. The previous embedded M3 Recall area was removed. History pane does not switch chat, update active message array, compose prompt, stream or tool authority. CI adds native page/restart/access tests and source contracts.

**Explicit remaining gaps:** this work makes the *secondary pane* paginated, but the main active conversation currently still calls `lu_history` and materializes its whole transcript; full active-chat virtual scrolling and cancellation-preserving loaded row windows are NOT yet implemented. Previous historic source direct model reads remain 4096-character-capped by design; the human-only page reader can continue up to original length. No physical installed-app acceptance yet.


## M3 operational continuity: source-linked build asset inventory (source candidate 2026-11-07)

Implemented in source: the new `lu_core::operational_asset_ledger` is a **separate small SQLite metadata/evidence store**, not a second chat history. A record preserves project ID, logical asset name, version, original ANSF source ID, host-selected absolute local path, observed byte length, SHA-256 digest and immutable observation ID. It streams up to 16 GiB without reading a ZIP into memory; source ID canonicality and trusted source access are required, file symlinks/unsafe inputs and mismatching pinned digests are rejected. Host receipts never confer tool execution rights.

Desktop now opens the asset ledger at startup, adds an actual **native OS file-picker and explicit user confirmation** for remembering an existing local SDK/MSVC/NSIS/LLVM or other binary, and can recheck latest project records asynchronously. Missing, changed/unsafe and present-with-matching-hash are distinct statuses. The independent History Library panel has a 'Operational memory' section for linking a selected file to a human-readable original source. Native tests exercise reopening, idempotency, mutation/missing file detection, source denial and immutable ID conflicts; CI adds Node integration-contract checks.

**Scope caveats:** This first asset-ledger slice records *local file evidence*, not a verified upstream software publisher signature or full installed toolchain. It does not auto-provision missing SDKs, download packages, recreate build environments, rerun old actions or auto-inject the ledger into a model prompt. The host's existing ContinuityStore/task/effect journals remain separate authorities for task state. Real source-to-task resume, procedural recipes and safe automatic asset reuse are subsequent integration gates; do not claim the historical NSIS repetition problem is already solved end-to-end.


## M3 original active virtualized transcript — bounded forward/backward windows (source candidate 2026-11-07)

**The previously missing active transcript paging now has a source implementation.**
- `ansf_live_chat.rs` stores a *derived pointer-only visible-chat index* (first display event seq, latest original event seq, role, generation ID and terminal status); it never duplicates original message text. Each assistant stream's many progress checkpoints correspond to one visible row. Completed/interrupt/cancel states remain distinguishable and the original immutable event log is untouched. Legacy books with no index can backfill it once; startup doesn't repeatedly full-scan an already indexed book.
- `visible_chat_before/after` use indexed cursor SQL and at most 64 visible rows per request. `ChatStore::history_window` authenticates its original book. Tauri's `lu_history_window` is the bounded GUI read; existing full export/debug history and bounded model history remain independent APIs.
- React's normal chat read/refresh path now uses 64-message latest windows and backward/forward on-demand 48-row pages; the rendered working set is capped at 192 messages. Source event seqs are stable React keys; prepending older entries attempts to preserve the scroll anchor. The independent history search pane remains separate.
- Native tests cover 120+ original events, multiple progress snapshots, cancelled vs completed, pagination both directions, reopen and rebuild from immutable original records. UI source contract checks ensure the GUI no longer invokes full `lu_history` directly.

**Not yet verified:** real installed Win11 WebView smooth scrolling, exact pixel anchoring when windowed rows are evicted on both ends, screen-reader behavior, very long 6+ month histories and live concurrent chat switching, and native exact-head CI. This is bounded windowing rather than a measured mature variable-height virtual list; finalize virtualization acceptance before declaring original UX fully delivered. Core model context is still separately token bounded.


## M3 recovered Context/Task continuity: source-linked desktop checkpoints (source candidate 2026-11-07)

The **existing** `lu_core::continuity::ContinuityStore` is now opened in the desktop host at `lu-context-continuity.sqlite`. Do not create another canonical chat history. The desktop `ChatStore::record_continuity` insists on at least one original ANSF message ID from its own authenticated source conversation, exact original user/assistant role, project/task/ID validation through Core, immutable checkpoint identity, and `append_verified` against the actual ANSF source book. A new chat may retrieve the latest checkpoint for the same project+task through `recall_project_continuity` only after checking both books and all original cited entries. The checkpoint describes completed work, next action and blockers, **never authorization**.

Tauri adds `lu_m3_record_continuity` (separately shown native confirmation after verifying source IDs, before durable save) and `lu_m3_resume_continuity` (read-only). The independent Library pane now has a Project Continuity section with exact source ID, completed work, pending next action, blocker list, Save and latest-task lookup. Cross-chat native restart/denial tests are added in the *production portable native* harness; CI includes a JSX/host contract.

**Still open:** native CI and physical desktop acceptance; automatic proposal extraction from independently verified effect receipts, time/freshness belief updates, CCM change detector and live model Send prompt projection/automatic resume. The user must currently explicitly save the checkpoint and ask the secondary pane to retrieve it. The NSIS task can now be *recorded and located* across Lu chats but will not yet auto-run a previously verified build recipe or populate a complete environment from checkpoint data.

## M3 hot scrolling hardening — bidirectional source cursors (November 7, 2026)

The active chat's 192-row bounded window now uses **stable ANSF event-ID viewport anchors** rather than `scrollHeight` deltas that mis-position tall/variable-height messages when offscreen rows are evicted. It deduplicates overlapping cursor pages, protects the user from automatic scroll-to-bottom on prepend, and restores the exact original message cursor/offset when returning to a chat even if that message is outside the latest 64. The independent History Library has both backward and forward source-page queries, a 192-row cap, and re-loads discarded pages as needed. None of these additional display indexes replace canonical ANSF originals.

Native portable test covers 520 original entries with long and short messages, forward/backward paging, same-profile access and reopen; Node regression tests cover 10,000-entry-style numeric sequence windows, duplicate prevention, variable message heights, anchoring and chat switching. **The Node viewport geometry tests execute on a fake layout; only installed WebView acceptance demonstrates actual fluid scroll behavior.** P0 acceptance still requires exact-head Windows/Linux native runs, genuine app scroll stress and accessibility/streaming during page eviction.
