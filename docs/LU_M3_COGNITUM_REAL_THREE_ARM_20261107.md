# M3 real Cognitum-backed three-arm baseline

Source implementation only; native and real-model GitHub Actions are pending. The public Cognitum commit `3c7614634ed210580644708be304a14702ec5325` and exact original Git blob SHAs are pinned; no private user Library files are published.

A real local MIT-licensed BGE-small-en-v1.5 Q8_0 GGUF (SHA-256 `f046db1dc724cf4f6f0a0c5917e922823b73eb1d27b8f9a9c2797f7866974804`) produces true original-source and query embeddings, with Lu Core S1 SQLite projections, FTS5+semantic cosine fusion, and validated original ANSF source IDs. The identical pinned Qwen3-0.6B Q4_K_M chat model runs paired three-arm tasks: no memory, conventional recent history, and recent history plus **unassisted** semantic ANSF discovery. Gold answers and IDs are never passed to the third arm. Capture raw answers, retrieval proof, model hashes, accuracy, latency and all failures. A negative advantage is a valid result.

Prior 2/28, 6/28, 18/28 post-hoc known-ID figures are NOT results for this experiment. Passing headless CI does not close M3: actual installed Windows WebView scroll/stream/cancellation and local embedding plus NSIS recovery acceptance remain necessary.
## Baseline fairness correction (new experiment only)

The recent-chat baseline now includes **three genuine verbatim excerpts** from the exact pinned Cognitum original documents, selected at benchmark-fixture construction time. Both conventional recent chat and ANSF hybrid receive those same recent source facts; the ANSF arm may also retrieve independently from its source index. This prevents treating an intentionally fact-free recent context as a universal conventional-memory baseline. The script stratifies recent-fact, long-gap and unknown questions, explicitly flags ANSF regressions on recent or unknown questions, and preserves raw model responses. These are paired CPU-only real-model trials; no accuracy result may be claimed before executing the workflow.
