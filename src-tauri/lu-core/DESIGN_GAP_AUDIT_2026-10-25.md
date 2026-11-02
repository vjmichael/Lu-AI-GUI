# Lu: Original Jan Modification Design vs. Implementation

**Audit:** 2026-10-25  
**Source:** `Jan-Mod-Design-Document.md` (2026-10-21, revised 2026-10-22), including later user decisions in the Lu project.  
**Inspected:** `vjmichael/jan_alt`, draft branch `lu/core-contracts-native-registry`.  
**Purpose:** Identify missed deliverables, avoid duplicating existing Jan capabilities, and choose the next milestone based on the user's priorities.

## How to read status

- **Existing Jan:** Useful upstream feature; reuse rather than rewrite.
- **Lu Core:** New implementation and tests exist, but full desktop/runtime integration may not.
- **Partial:** Some foundation exists; a specified acceptance path is missing.
- **Deferred:** Intentionally postponed, not a shipping dependency.
- **Superseded:** The newer user decision wins; DO NOT implement the older design.

## Primary requirements and reconciliation

| Design section / feature | State | Current evidence / remaining work |
| --- | --- | --- |
| 1, 3: Unrestricted trusted-local file/web authority | **SUPERSEDED** | User adopted conversation-bound task authority, non-admin OS ceiling, OS-isolated workers. `conversation_authority.rs` and `privilege_guard.rs` implement portions; desktop identity and actual OS sandbox are not complete. Never strip safety boundaries per the old plan. |
| 3.7: Injection defense, action gating, egress | **Partial / revised** | Lu scoped grants, quarantine and declared-effect guard; OS-constrained worker, argument-intent validation, authenticated turn ownership, network-deny and adversarial isolation still missing. Blocklists and model prompt assurances are not enforcement. |
| 4: Protobuf internal payloads and strict model boundary | **Lu Core, partial desktop** | `wire.rs`, `model.rs` and generated `proto/lu/v1/core.proto`. Jan desktop / Cowork have JSON-era paths; broad replacement not yet complete. Tauri binary IPC feasibility remains to be verified on exact version. |
| 4.8–4.9: Reliable model boundary, durable effect journal | **Lu Core partial** | SQLite journal with replay receipts, strict tool argument checks. Full execution recovery and host-grant audit remain. |
| 5: Native toolkit, combined internal tools with small model-facing schemas | **Lu Core, partial tool coverage** | Registry and two scoped Rust read-only tools, legacy bridge. No need to duplicate every Jan capability. Writable, bulk rename/delete, archive, PDF, code exec, web adapters need later capability-by-capability testing and OS isolation. |
| 5.4: Image tool with vision capability check | **Open, media later** | Do not force a vision call on a text-only model. A/V and camera are deferred optional first-party extensions. |
| 5A: Background research, progress and task manager | **Partial** | `TaskStore` supports durable checkpoints, heartbeat and progress; Jan agent has background mechanisms. General multiphase research flow and a unified Lu desktop task dashboard remain to be implemented. |
| 5B: Smart Markdown copy | **Existing Jan / verify semantics** | Message actions already offer Copy; inspect/extend only if round-trip Markdown preserves tables, code blocks and text selections incorrectly. Do not build duplicate clipboard tools blindly. |
| 5C: Restart/resume after power loss | **Lu Core partial** | TaskStore and journal persist records. Per-task resumable workflow executors and startup resume UX remain. Avoid trusting beforeunload as a power-failure guarantee. |
| 5D: AI Council multi-model consultation | **Deferred past memory milestone** | Jan has local and hosted model providers. Coordinated parallel council and synthesis is not complete; do not confuse provider switching with a council. Keep explicit privacy/endpoint rules. |
| 5E: Quick/Standard/Deep/Research/Debug/Companion modes | **Partial / reconsider** | Jan already has model controls; Lu's per-task coordinator, capability scopes and lightweight acknowledgement policy are missing. A fixed token-budget dial is not interchangeable with real model support. |
| 5F: Mutable working memory + objects/relationships/snapshots | **Open** | Jan agent has project memory notes. ANSF original messages are not a semantic belief/decision revision store. User-editable facts, contradictions, superseded decisions, and relation graph need their own service, source citations and deletion semantics. HDC/KAN enhancement is a deferred research backlog. |
| 5G: Tool cards, detailed activity audit and provenance | **Partial** | Jan has expandable tool UI; Lu has effect journal. Unified per-response provenance, external source chains, inspected file line references, and audit-view UI are still open. ANSF v1.1 source/parent relations missing. |
| 5H: Propose/apply changes & explanation | **Partial / revised** | Jan permissions and diff/approval UI exist. New design allows low-risk repeated actions under exact scoped delegation; not every authorized filesystem write requires a second approval. High-consequence actions and actual OS boundaries remain critical. |
| 5I.2–5I.3: Disk-backed ANSF original archive | **Lu Core** | `ansf.rs`: immutable exact records, stable session IDs, topic chapters, searchable text, cursor pagination. No Jan UI wiring. |
| 5I.9: Global library catalog | **Lu Core** | `ansf_library.rs`: FTS5 indexed books/chapter TOCs, source references without copied messages. Still requires privileged host scope checks and synchronization. |
| 5I.10: Source and reference provenance | **Open** | Add immutable references to source documents/URLs/entry IDs and derived causal links. Do not silently elevate referenced chat text to instructions. |
| 5I.4–5I.7: Dynamic context eviction/retrieval | **Open** | Jan has separate auto-compaction. ANSF context-window working-set builder, model-readable `search/read` tools, automatic topic shifts and token-budget-based retrieval are missing. |
| 5I.5: Pages stored in Protobuf alongside ANSF SQLite | **SUPERSEDED** | The later, more specific ANSF design uses indexed SQLite books as durable source of truth. Avoid duplicating complete conversation pages into a second Protobuf archive. |
| 5J: Optional model routing | **Deferred by original design** | Original document says OFF by default. Lightweight coordinator later may use heuristics or small model for acknowledgements. Do not force a second GPU model or build multi-model routing first. |
| 6: Message queue | **Existing Jan** | `message-queue-store.ts`, `ChatInput.tsx` and thread route enqueue/auto-send. Do not rewrite the same queue. |
| 6: Soft interrupt and mid-run revisions | **Partial** | Rust CLI/TUI agent loop has safe-boundary steering; desktop GUI queue waits for turn completion. Host-authorized desktop steering and revision semantics still missing. |
| 6A: Identity, date/time, model/tool ambient context | **Partial / adapt** | Jan prompts already include model/session metadata in places. Compose a concise truthful identity and capability block; never claim full system access or inherited administrator privileges. |
| 7: Loop detector / progress intervention | **Partial / audit existing Jan first** | Jan has extensive agent loop and tool control; unify only missing adversarial/no-progress limits, without introducing a parallel loop engine. |
| 8: WASM/process execution sandbox | **Incomplete OS enforcement** | Original suggests Pyodide, QuickJS, PowerShell AppContainer/CLM. Favor verified restricted worker and capabilities over assuming CLM or labels make OS execution safe. Do not launch arbitrary code through the Lu Core registry until tested. |
| 8.5: Universal sidecar compilation/runtime | **Deferred optional** | Large scope, not required for chat/memory shipping. Avoid loading every runtime into Lu. |
| 9–15: GUI polish, deployment, success criteria | **Partial** | Jan baseline exists; Lu identity, Enter-to-newline setting, reliable read/processed/applied acknowledgements, virtualized history and Memory Worker panes are future application work. |
| Later conversation: drag/link/transclude historical chat content | **Open** | Persistent source entry references with per-destination interpretation and *no authority transfer*. Need links table, revision-aware provenance, side pane and context assembly. |
| Later conversation: backgrounds, themes, shaped bubbles, emoji reactions | **Deferred polish** | Existing Jan colored-user-bubble setting; richer styles and theme packages should be declarative, not executable DLCs. |
| Later conversation: voice, camera, streaming A/V | **Deferred optional first-party extensions** | Not part of current milestone or required Lu core; A/V plan parked. |
| Third-party workspace DLCs | **DEFERRED AFTER CORE RELEASE** | `workspace_quarantine.rs` is only an inert staging contract. Further DLC development must not block Lu 1.0. |

## Reconciliation issues from the original document

1. **History mutation:** Jan can edit/delete messages in `messages.jsonl`, but ANSF retains immutable originals. Linked revisions are now implemented; deletion tombstones, verified edit timestamps, and Jan host synchronization are still pending.
2. **ANSF hierarchy ambiguity:** One design section places memory/object tables inside per-conversation files while another proposes workspace-level object databases. Prefer session-owned immutable books, workspace-owned mutable working/object memory, and global searchable metadata. A separate migration/backup plan is required.
3. **Three-step recall:** Library → chapter → exact entry lookup is a useful default hierarchy, **not a hard cap** that might hide relevant evidence. Use measured latency/recall benchmarks and fallback search.
4. **Index consistency:** `library.db` metadata can be rebuilt from immutable ANSF books; two SQLite databases do not share one atomic transaction.
5. **Authority:** An imported or linked older conversation is context, never a fresh authenticated human task or privilege grant.
6. **Token-budget promises:** "Infinite context" means persistent history plus bounded dynamic retrieval, not infinite transformer context. Measure model input tokens and retrieval quality before claiming savings.
7. **Retention:** ANSF's no-automatic-deletion goal needs user-controlled removal and privacy-consistent tombstone/purge policies, including index/cache invalidation.
8. **Telemetry:** Checkmarks must mean distinct `received`, `processed` and `applied` states; they must not misrepresent model processing or task execution.

## Next development goal selected

**Jan desktop JSONL → ANSF import, non-destructive and resumable.**

`ansf_import.rs` now provides a bounded streaming importer accepting trusted-host-selected JSONL, storing the exact Jan record as immutable ANSF entry content, deriving a short search summary, preserving the Jan message ID/role and rejecting any cross-conversation ID mismatch. Re-runs skip identical rows. If Jan later edits an existing ID, importer reports `ChangedSource` rather than overwriting history.

It deliberately does **not** modify Jan's files, execute text, grant tools, automatically upload content, or let an untrusted model open arbitrary archive paths. Add desktop owner/path verification and an index-sync service before using this in production.

### Remaining acceptance path before calling chat migration done

1. Test a range of historical Jan exports (attachments, malformed records, huge content, deleted messages, streaming responses, message revisions).
2. **Partially implemented:** immutable message revisions and source-to-current
   links now exist in `ansf_revisions.rs`. Implement authenticated desktop
   edit events, verified Jan snapshot chronology, deletion tombstones, and
   accurate edited-at display before enabling production sync.
3. Add a host-owned, restricted migration runner and background library synchronization; Jan stays source-of-truth until verified cutover.
4. Verify count, identifiers, original content hashes and rollback against representative, private, locally backed-up Jan conversations.
5. Implement bounded ANSF model context builder and scoped `read/search` APIs, then expose a virtualized Memory Worker pane.
6. Add cross-chat context links, original-source provenance, mutable current beliefs and supersession to deliver actual contextual intelligence.

**Release gate:** Do not replace Jan conversation storage merely because these independent Rust adapter tests pass.
