# Lu ANSF three-arm real-model baseline — November 7, 2026

**Owner request:** establish real same-model baselines for Lu without persistent memory, Lu with bounded conventional chat history, and Lu with persistent ANSF source-ID recall. Negative or null results are valuable and should block unsupported superiority claims.

## Trial setup (source implementation)

GitHub workflow: `.github/workflows/lu-ansf-three-arm-benchmark.yml`.

Experimental implementation: `benchmarks/memory/three_arm_real_model.py` plus production Lu Core `src-tauri/lu-core/examples/ansf_memory_baseline.rs` for real `AnsfStore`, `AnsfLibrary`, `AnsfSourceId`, and `resolve_known_source` storage/lookup.

**Crucial scope:** these are three **isolated headless Lu Core memory configurations** queried by one **real** local GGUF backend. They are *not* three separately installed Lu desktop apps. The third configuration tests the **current known-source-ID implementation**, **not** the complete future ANSF M3/M4 hierarchy, semantic/object memory, procedural learning or autonomous clarification loop. Installed-app evaluation must follow separately.

### Model and engine provenance

- Fixed small-model test: `QuantFactory/Qwen3-0.6B-GGUF`, `Qwen3-0.6B.Q4_K_M.gguf` (~484 MB). SHA-256: `7af3fdf842f87b24672f8a7f1dd50404043f0bfb71093ff91c31d2b49df4631d`, verified before inference. Model source: https://huggingface.co/QuantFactory/Qwen3-0.6B-GGUF/blob/main/Qwen3-0.6B.Q4_K_M.gguf (underlying Qwen model license: Apache-2.0; confirm bundled attribution prior to distribution, CI downloads temporarily).
- `llama.cpp` official tag `b12143`, source commit prefix `2bc46ba`; GitHub CI builds real Linux CPU llama-server and records its entire source SHA and executable SHA. Never substitute fake engine placeholders.
- Same GGUF, CPU backend, context size 4096, threads 2, identical task, generation temperature 0.0, output limit 96, seed tied to trial, and a randomized per-trial condition order. The inference API accepts JSON as a *model boundary*; Lu Core uses SQLite for ANSF.
- 14 synthetic deterministic tasks × 3 arms × 2 paired repeats = **84 real model generations**. No actual user history or sensitive files are uploaded or benchmarked.

### Three arms

| Arm | Supplied task context |
| --- | --- |
| **None** | Current question and identical system instruction. No persistent project facts. |
| **Recent chat** | The exact same question and instruction plus up to nine most recent messages from the **same** active conversation, capped at 2,600 characters. |
| **ANSF direct ID** | Everything supplied to **Recent chat**, plus a bounded, source-verified original entry loaded by **real Lu Core** SQLite indexed lookup using its canonical `ansf:v1:` ID. The host checks exact bytes against the seeded immutable original, not the ground-truth answer value. Missing IDs are honestly reported. |

All three arms see the same opaque source ID locator when a question has one; the ID itself does not encode its answer. Retrieval uses real book/library SQLite files and a bounded, fixture-scoped authorized opener. The **scorer's answers are kept separately** and are never supplied to the model as a task prompt or retrieval hint.

### Task groups and fairness controls

- **Old:** four old facts outside recent-chat truncation — tests persistent memory gain.
- **Recent:** three facts present inside conventional history — tests whether ANSF adds unnecessary overhead or hurts an already-good baseline.
- **Cross-chat:** four source facts in different conversation books — tests host-authorized direct source lookup.
- **Missing:** an absent source ID — tests whether the model admits no verified source.
- **Stale:** an obsolete original decision plus a later correction in recent history — penalizes overconfident, stale source promotion.
- **General:** a memory-independent arithmetic problem — verifies overhead/regression on an ordinary task.

The fixed synthetic suite is a CI **smoke experiment**, not a statistically powered claim about model intelligence or generalizing to real multi-week software development. It is intentionally favorable to direct-ID retrieval on old facts, so success on these tasks alone does **not** demonstrate comprehensive ANSF superiority. Future evaluation must add unknown-source discovery, source selection, sparse misleading metadata, contradictory revisions, large-scale datasets, independent benchmarks, model switching, failures and live installed app tasks.

### Evidence and verdict

Both exact source and native contract tests must pass before real inference. Then the real-model job records:
- Raw answers, independently graded short exact-match accuracy, missing-source behavior.
- Per-case/per-arm outcomes and success rates by bucket.
- Actual request prompt/completion token counts (when provider reports them).
- Wall-clock model latency and separate ANSF source lookup time.
- Model SHA-256, engine executable SHA-256, engine tag/commit, Git SHA, seeds, runtime settings.
- Machine-readable `results.json`, readable `summary.md` and partial case-by-case receipts; uploaded on both success and failure.

**Baseline rule:** If ANSF does not improve **aggregate** accuracy relative to conventional recent chat, report `NO_ANSF_ADVANTAGE` and exit nonzero. If it regresses against recent chat in recent/stale/general/missing controls, report `ANSF_REGRESSION_...` and exit nonzero. The workflow upload still preserves the raw evidence. A missing or unusable model/backend is an **invalid experiment**, not a model loss or fictitious zero score.

**Interpretation:** A red experiment means don't prematurely promote the ANSF design based on this suite; inspect result breakdown and hypotheses. One small deterministic CI suite by itself is not sufficient grounds to delete the memory system or claim a universal theorem. Independent matched baselines and harder real tasks follow before making expensive architecture decisions.

### K/I/A/F failure attribution

- **K:** model fails even when exact verified context is supplied, or cannot follow output instructions.
- **I:** retrieval fails, wrong source identity or omitted context, incorrect library mapping, malformed prompt adaptation.
- **A:** an unauthorized book is opened, foreign data leaks, source ID acts as a grant. Such failures are fatal independent of answer score.
- **F:** benchmark misgrades a response, unverified source is described as verified, or test outcome is fabricated/uncertain.
- **U (human goal):** whether better recall meaningfully improves real task success and decreases repeated explanation; not yet measured in this synthetic smoke.

## Implementation and release gates

- **Implemented in branch:** benchmark adapter, deterministic dataset and scorer, three matched inference policies, regression controls, reproducible CI workflow, artifact publication, document and source manifest registration.
- **Must be established by GitHub:** exact-head tests, actual model download/checksum, real llama.cpp build, 84 completed model generations, valid comparisons and artifacts.
- **Not claimed:** positive results, full three-desktop installed acceptance, completion of M2 JSONL→ANSF source migration, automatic cross-chat discovery, autonomous model tool invocation, reduction in real user effort.

The owner wants clear evidence, including when conventional history is superior. We do not rewrite the result to protect the experiment.
