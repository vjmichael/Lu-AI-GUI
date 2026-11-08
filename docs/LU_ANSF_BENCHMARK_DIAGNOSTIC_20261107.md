# ANSF three-arm baseline — first real GGUF diagnostics, not an accepted win

Observed November 7, 2026 (UTC; Lu day remains November 7 in Arizona). Original GitHub Actions runs:
- https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42415620395
- https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42415589606

These were **real** 84-generation headless experiments against pinned Qwen3-0.6B Q4_K_M / llama.cpp. The first run's metrics were:
| Arm | Strict 2026-11-07 original scoring | Prompt tokens/turn | Avg model latency | Lookup |
| --- | --- | --- | --- | --- |
| No memory | 0/28 | 100.1 | 466 ms | 0 |
| 9 recent messages | 0/28 | 254.1 | 756 ms | 0 |
| Recent messages + original ANSF source-ID lookup | 0/28 | 293.6 | 829 ms | 2.9 ms |

The original format instruction literally used `ANSWER: <short exact value>` and `ANSWER: <value>`. The small model frequently echoed those format placeholders, e.g. `<value> Linux</value>` and `<short exact value> 15;`. The original scorer compared the full wrapper literally, so even correct contents (including independent arithmetic) counted wrong. Full original raw responses were retained in the GitHub Action artifact and must not be overwritten.

## Exploratory post-hoc diagnostic — NOT preregistered
A conservative symmetric normalization, run over all 84 immutable original answers (not re-generated), accepts a single unambiguous fact inside a harmless `<value>`/`<short exact value>` wrapper, plus trailing terminal punctuation. It rejects placeholders without a fact, multiple/conflicting facts, unknown embeddings, and changes to the expected values.

| Arm | Diagnostic 28 original generations | Accuracy |
| --- | ---: | ---: |
| No memory | 2/28 | 7.1% |
| Recent bounded history | 6/28 | 21.4% |
| Recent history + canonical exact ANSF lookup | 18/28 | 64.3% |

This supports **testing** whether exact-source ANSF evidence helps a local model. It does not validate full ANSF semantic search, autonomous discovery, benchmark robustness or statistical advantage on real user tasks. The same single fixture appears twice and answers within an arm are correlated. The ANSF arm receives the exact original source ID by the harness. A newly written semantic retriever MUST be benchmarked separately without oracle IDs.

## Corrective engineering change
- The benchmark prompt no longer contains literal placeholder tags; requests plain `ANSWER: fact` or `ANSWER: UNKNOWN`.
- One conservative formatter normalization is now applied *symmetrically* across all arms, with false answers remaining false.
- Unit tests guard against both placeholder contamination and incorrectly crediting contradictory answers.
- Rerun prospectively with the same pinned model/seed/fixtures, preserve the old artifact and compare both strict-legacy and corrected protocol outcomes.
- Extend benchmark beyond known-ID lookup to automatic FTS, hybrid embeddings, revisions, source uncertainty, duplicate tool calls avoided and task continuation. An uncompleted or uninstalled workflow is **not** proof of a new result.

Failure categories: K model/output adherence; I original-source recall/routing; A authorization; F scoring/attribution; U user benefit. The original 0/28 scores are an **F** measurement defect, not a meaningful universal verdict about SQLite memory.
