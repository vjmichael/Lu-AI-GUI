# P01 — Filesystem Discovery and AI-Friendly Native File Search

**Work order:** FS-DISCOVER-01  
**Priority:** P01 (first in the owner-approved sequence)  
**Source status (2026-11-06):** **BOUNDED CORE SOURCE IMPLEMENTED — native/installed acceptance OPEN**. Stop after P01; P02 requires the owner's next explicit instruction.  
**Repository:** `vjmichael/Lu-AI-GUI`, development branch `lu/reader-ocr-image-closeout-20261106`, stacked draft #67.

## Goal and deliberate limit

A local AI should locate relevant files quickly without repeatedly crawling the filesystem or consuming model context with raw directory dumps. The source of authority is still the **human-selected native folder** and an authenticated, per-turn `ReadWorkspace` grant, *never* a cached filename, model-supplied root, or untrusted document. Keep existing secure read/create/edit/copy/move/trash/restore; improve finding things rather than replacing file mutation code. There is no all-drive crawler or silent home-directory indexing.

## Existing components reused

- `native_files.rs`: bounded read and directory listing, cap-std root.
- `native_file_extra.rs`: direct stat and 64-file bounded literal content search.
- `native_workspace_read_turn.rs`: authenticated persisted user turn, native OS folder picker, turn-scoped `ReadWorkspace` authority, typed Protobuf/model JSON boundary and durable model journal.
- `workspace_desktop.rs`: model/tool turn queue, cancellation and progress; no WebView-provided root.
- Existing atomic write, reversible trash, file transfers, approved edit staging, and crash-recovery receipts are **not duplicated or broadened**.

## Newly implemented — `native_file_discovery.rs`

- **New registered tool:** `lu.files.discover` / model-facing `lu_files_discover`. Returns compact ranked file/folder candidates, not raw root paths or file content unless content search is requested. Available ONLY when `WorkspaceReadTurnSession::new` is constructed for an authenticated operator-picked folder; the read-only tool registry and per-invocation authorization remain mandatory. Git mode remains unchanged.
- **Index:** Up to **12,000 entries / 512 directories / 20 levels**, excluding hidden dot entries and symlinks. An in-process, bounded **4-root metadata-only cache** survives repeated approved turns while Lu is running. Directory modification timestamps support incremental subtree reconciliation after a **15-second refresh interval** or immediately on `refresh=true`; scan work has a nominal **4-second budget**. Results flag incomplete scans instead of claiming exhaustive coverage.
- **Metadata query:** UTF-8 filename/path substring ranked exact name → name prefix → name substring → path; optional relative subtree, case-insensitive suffix, file/directory kind, size range, modified-since Unix timestamp and result limit 1–80. For filter-only searches, newest modified candidates rank first. No shell or regex interpretation.
- **Freshness:** Reinspect every returned candidate from the authorized cap-std root, check each ancestor is an actual directory and not a symlink, verify kind, file size and modified timestamp. Stale or inaccessible candidates are suppressed and mark `index_incomplete`. These are *metadata identity heuristics*, not full content SHA or ACL proof. The actual Reader/edit tool must independently open and authorize any result.
- **Literal content:** Optional separate `content_query` examines at most **96 files / 4 MiB per query / 64 KiB per file**, with a bounded, sanitized excerpt. An explicit path continuation `next_content_after` can advance the search over subsequent batches. Per-turn contents may be cached in RAM up to 4 MiB but are **never written into the persistent metadata cache or user library** and expire at the end of the task. Long documents should be streamed by P04 Reader rather than treated as enormous filestrings.
- **Safety and observability:** No symlink/reparse escape grant, no model-chosen absolute root, no implicit write authority, no network transfer, no persistent whole-disk index, no unchecked source-content instructions. `index_incomplete`, `results_truncated`, `content_incomplete`, `index_generation` and `content_files_examined` tell the AI and host whether a search is exhaustive.
- **Desktop:** The existing Ask Workspace button and normal Send read-only scoped routing continue to use the same OS picker. They advertise the new discovery schema. The native model instruction recommends discovery before repeated folder traversal and tells Lu not to treat a hit as proof of file identity or permission.

## Example model calls (only within a current approved folder turn)

```json
{"query":"Vulkan","kind":"file","limit":20}
{"query":"","file_suffix":".pdf","modified_since":1735689600,"limit":30}
{"query":"","directory":"notes","content_query":"memory budget","limit":24}
{"query":"","directory":"notes","content_query":"memory budget","content_after":"notes/last-scanned.txt","limit":24}
{"query":"guide","refresh":true}
```

The model's view of a document is always a candidate until it invokes an authorized read tool and rechecks identity. Empty query requires some filter, a subtree, or a content query to avoid unbounded enumeration by accident. Content matches are literal untrusted data, not semantic understanding.

## Verification evidence and remaining boundaries

**Tests authored, not executed:** Rust native unit fixtures for scoped name discovery, reused generations, renamed/deleted entries, incremental refresh, bounded content search/continuation, path traversal, hidden entries and Unix symlink refusal. Desktop source-contract tests for native registration, no WebView root injection and bounded caches. Exact SHA Rust compile, Node suite, Tauri build, adversarial NTFS/junction/ACL races, installed Windows and model selection/response acceptance remain **OPEN**.

**P01 bounded source closeout is not a claim of universal search.** Further P01 capabilities that may be worthwhile later but are not prerequisites for the bounded initial implementation:

1. Persistent encrypted/consent-reviewed multi-root indexing and per-root revoke/delete UI; a RAM cache is deliberately cleared on restart. Cross-device permissions and OS identity require design, not implicit reuse.
2. OS-native watcher integration and durable change journals (currently a 15-second metadata refresh plus forced reconciliation; no live watcher).
3. Hybrid semantic/vector search and full-content indexing for very large document collections; Reader P04 and AI Operations P03 should provide shared document/chunk identity rather than inventing a second parser.
4. Performance tuning for million-file trees, network shares, Windows junctions/reparse points, case-collision policies, high-concurrency edits and source-handle identity hardening.

These are clearly marked future enhancements/security gates; P01's *bounded, authorized filename and metadata discovery with selective content search* is the deliverable of FS-DISCOVER-01. **Do not roll into P02 without a new owner instruction.**
