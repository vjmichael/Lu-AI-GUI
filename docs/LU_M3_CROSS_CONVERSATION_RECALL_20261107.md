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
