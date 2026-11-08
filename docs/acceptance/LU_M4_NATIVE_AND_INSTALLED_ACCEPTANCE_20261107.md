# Lu M4 — Native model, restart, and installed Windows acceptance

**Current disposition:** M4-P02/P03 semantic claims and conflict-safe belief revision are source implemented. M4-P04 local GGUF model-context injection and exact source/correction UI are source implemented. M4-P05 native ANSF/SQLite restart tests are submitted, but actual installed Windows and different-GGUF model-swap acceptance remains **OPEN** until true receipts exist. M3 release gates remain separately open.

## Native source-to-model pipeline

The existing human-origin local-model Send path recognizes only the exact explicit syntax:

`Recall belief project <project> task <task> subject <subject> predicate <predicate> version <current-version>`

The desktop host authenticates the active ANSF book, queries one exact subject and predicate, revalidates source hashes and current revision, source-linked native receipts, expiry and application version, and injects at most 4 KiB of historical-data JSON to the local GGUF prompt. Labels identify **reported**, **contradicted**, **superseded**, **retracted** and **stale**. An actual native effect needs new per-turn authorization. Since the GUI currently lacks an independent OS postcondition witness for claim truth, the model-facing path does NOT silently label journal-APPLIED evidence independently verified; that state requires separate trusted verification integration.

The independent History Library pane can associate only **current-book original human text** with a newly reported claim. It provides read-only conflict inspection and human-sourced retractions. The model never receives direct SQLite write or retraction authority.

## Genuine local model evidence candidate

`src-tauri/lu-core/examples/m4_belief_model_proof.rs` writes real ANSF user entries and source-bound claim rows to a new disposable SQLite original book, then constructs contradictory, expired-version, retracted, and reopened-book cases. `benchmarks/memory/m4_real_local_model.py` executes a pinned SHA-256 Qwen3-0.6B Q4_K_M using CPU-only local `llama-server`, emits real response text, source IDs, latency and raw results, and returns a negative CI result if any expected classification fails. This validates a narrow provenance/trust-label interpretation task, **not** general semantic truth, real BGE search, or a human-facing installed Lu.

The `Lu M4 Verified Procedural Memory Native Acceptance` workflow runs actual Windows/Linux Core tests and portable chat-journal restart tests, plus a separate genuine GGUF job. Synthetic answers, Windows resource placeholders and source-only greps do not meet model-acceptance criteria.

## Physical Windows installed app acceptance

Run `scripts/m4-windows-installed-memory-acceptance.ps1` with independently reviewed installed Lu and bundled llama-server hashes, distinct primary and replacement GGUFs, and explicit `-PhysicalInteractiveWindowsConfirmed -ManualMemoryTrialConfirmed`. The harness independently checks real Windows UIA process identity and hash-reviewed model-worker process and cleanup, and asks the human operator to witness historical source preservation, contradiction resolution, changed app versions, model swaps, interrupted worker recovery, CCM, and denial of historical tool authority.

The harness fails closed when model workers are unobserved, OS reap is uncertain, or any requested human scenario is not confirmed. Human answers are labeled **OPERATOR_REPORTED_PASS** and do not substitute for independent WebView checks, real model answers or the final release decision. Do not run against an existing personal installation without a reviewed operator plan.

## Open gating requirements

1. Latest exact-head native Rust/portable chat and frontend compilation on both OSes; repair actual errors and rerun the final commit.
2. Genuine M4 local Qwen test results and raw artifact, including any negative findings.
3. Source revisions, cross-chat denial, provenance/conflict/freshness, permission-negative, restart, missing-file, and interrupted-worker tests.
4. Actual installed Windows user-driven model-swap, NSIS package and WebView acceptance, with OS process witnesses.
5. Model-supplied instructions cannot promote a belief, execute a tool, reauthorize an old effect or silently resolve contradictory evidence.

**M4 is not considered closed until these gates have actual passing evidence.**
