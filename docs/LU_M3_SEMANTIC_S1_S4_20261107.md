# M3-S1 through M3-S4: persistent semantic retrieval

Date: November 7, 2026
Status: S1/S2 indexing and hybrid *interfaces* are source-implemented; no actual embedding engine or model benchmark yet. M3 release and installed-app gates remain open.

## Preserve the ANSF contract
Authoritative original messages, chapters, revisions and goal decisions remain in per-conversation ANSF SQLite books. Neither the Library nor derived embeddings become a second original transcript. Index rows can be deleted and recomputed. Stable source IDs, original message text and role/provenance survive index rebuild and model swap. Authenticated current-turn permissions are never acquired by reading historical messages. External media remains brokered out of ANSF.

## S1: rebuildable indexed passage vectors (source-implemented)
- Core module: `src-tauri/lu-core/src/ansf_semantic.rs`. Schema: `ansf_semantic_projection` inside each *original* ANSF book, storing entry ID, chunk ordinal, embedding model/version ID, dimensions, SHA-256 of original source, and packed little-endian float32 vectors. No source content is copied.
- Only finite nonzero vectors of dimension 2..2048, model IDs restricted to safe ASCII, chunks 0..127; rejects system/tool roles and missing original entries. Existing embedding model/version entries are immutable/idempotent: conflicting bytes fail closed, version changes must have a new model ID.
- Delete/rebuild only the derived index for a given model; source messages remain untouched.
- Exact cosine scanning of up to 8192 indexed passage vectors **per book**, retrieving full source content only for selected results. Beyond this limit the API fails rather than falsely promising a complete search. Approximate nearest-neighbor infrastructure is NOT bundled and not yet chosen. Native synthetic vectors test storage and ranking, **not** real semantic understanding.

## S2: original-source hybrid ranking (source-implemented interface)
- Reciprocal-rank fusion (RRF) combines ordered lexical FTS5 IDs and semantic candidate IDs. It accepts only canonical ANSF source IDs. Final candidates are reopened through the current desktop profile's original-book validator.
- `ChatStore::hybrid_recall_with_embedding` is an **internal host-only** seam: it combines lexical recall and trusted local-engine query embeddings over up to 24 source books, returns at most three provenance-checked original excerpts. It is intentionally not a WebView command or an untrusted model-selected raw file.
- Native tests verify synthetic paraphrase retrieval through exact vectors even where FTS terms differ, persistence after ChatStore reopening, model-version isolation and no authority propagation.

## S3: actual local embedding workers and model-facing recall (OPEN)
- Select/license/bundle a local **embedding** model independently of the main GGUF assistant. Pin model identity, digest, dimension, tokenizer and embedding normalization.
- Supervised worker accepts host-owned original passage bytes; output cannot contain paths/grants. Queue incremental index/reindex with durable checkpoints and bounded CPU/GPU/memory. Never silently offload private memory to an external API.
- Connect real worker output to S1 and actual human-origin GGUF Send via S2; include revision/conflict labels, confidence and source citations. No automatic model-supplied embeddings accepted.
- Require real installed Windows application acceptance after restart and legacy ANSF migration. Current default M3 remains lexical-only until this is genuinely implemented.

## S4: falsifiable real-model three-arm benchmark (OPEN)
Compare (A) stateless ordinary model, (B) conventional bounded chat history, and (C) ANSF full source-ID + FTS + local vector hybrid. Hold same GGUF quantization, prompt budget, tools and hardware. Measure answer accuracy, correct source IDs, missing facts, false memories, contradictory/superseded-decision handling, tool authorization violations, task completion, first-token latency, overall time, energy/VRAM, persistent storage and failure/restart recovery. Stratify exact-ID, paraphrase, long-gap, multi-book, revision, stale reference and malicious-source cases. Measure p50/p95 latency and confidence intervals over multiple seeds/tasks. Never call synthetic vector fixtures proof of better AI memory.

## Engineering judgment
Embedding vectors are an approximate association layer, not understanding, memory authority, or infallible truth. ANSF must beat baseline outcomes: less rediscovery, fewer repeated tool calls, fewer unsupported recollections, stable restart and acceptable resource cost. If hybrid recall does not deliver a measured advantage, retain the immutable original source model and improve retrieval before expanding architecture. HDC is not required; evaluate it separately only if benchmark evidence justifies complexity.
