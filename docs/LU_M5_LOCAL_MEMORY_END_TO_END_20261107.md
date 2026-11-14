# Lu M5 — end-to-end local memory and repeated-task acceptance

**Opened:** November 7, 2026. **Base:** open M4 draft PR #98 at `dcf43f23b357bcd7da16f2313745af0848825895`. New branch `lu/m5-local-memory-e2e-20261107` stacks on M4; M3/M4 remain unmerged and independently gated. This ledger does not convert implementation or source-based tests into installed application acceptance.

## M5 scope, sequenced work orders

- **M5-01 — Native same-task recovery/control baseline:** run the real production ChatStore/ANSF original books against the same repeated task without memory and with exact original-source memory after genuine SQLite closure/reopen; check cross-conversation original evidence, tracked checkpoints, conflicting reports, later user correction, changed app version and stale source/authority denial. Test interrupted EffectJournal recovery and never turn uncertainty into applied evidence. **Source implementation submitted; independent Windows/Linux execution pending.**
- **M5-02 — Real Qwen local-model paired comparison:** execute identical controlled questions in randomized `no_memory` and `original_ansf` arms after each source resolver invocation cold-opens original ANSF SQLite via the actual Rust adapter. Pin model SHA, llama.cpp engine commit, deterministic decoding seed, repetitions, raw generations, source IDs, latency and token accounting. Score known-answer improvement *and* unknown/authority safety; deny positive outcome if memory reduces safety or does not beat baseline. **Submitted as a CI candidate, not measured.**
- **M5-03 — Installed Windows memory/UI test:** execute user-visible Lu installed-app model replies, source search, same-task continuation across process restart and actual model replacement in a physical GPU-enabled Windows session. Assert no stale advice, duplicate effects, lost user revisions, access bleed or historical approval replay. **Not executed.**
- **M5-04 — Per-effect/procedural outcome and authorization verification:** exercised UI/editor approvals and independent OS/postcondition receipts for actual effect cases, including revoked permission, failed/interrupted workers and manual reconciliation, with strict previous/current task separation. **Open.**
- **M5-05 — Comparative pilot and release-gate decision:** measure real correctness, source precision, rediscovery calls, unnecessary clarification, human interruption effort, p50/p95 first-response latency and authorization negatives on representative user tasks. Compare with/without memory and at least one real model switch; evaluate independent Windows install/uninstall, package identity and idle/hot-scroll. **Open.**

## Current submitted evidence — M5-01

The new `m5_native_replay_acceptance` module in the **existing production native-chat Cargo harness** tests the actual desktop `ChatStore`, `AnsfStore`, `ContinuityStore` and `EffectJournal`; these are not mocked database tables.

- First original conversation contains two *reported*, mutually contradictory backend claims and a source-backed checkpoint with unresolved blockers; a different active conversation contains only its own current task question.
- After closing every store handle and opening the actual SQLite files, **memory-off** sees no original task facts, while **memory-on** reopens the exact original IDs and returns the conflicting record states and blocked checkpoint without granting tool authority.
- A newer source-authenticated human message retracts one claim; the earlier original cannot impersonate a later correction. A changed application version marks the remaining reported claim stale. A further SQLite restart verifies correction persistence and lack of historical tool grants.
- A separate actual native effect is left EXECUTING, closed, reopened, and reconciled to UNCERTAIN; the checkpoint cannot link the effect as applied and there are no proven receipts.
- These are **deterministic native prerequisite tests** and DO NOT establish that a local model understood the evidence, that a user-facing installed Lu successfully resumed, or that one arm saved tool calls. They exist to stop false-positive model acceptance.

## M5-02 genuine-model experimental contract

`benchmarks/memory/m5_repeated_task_real_model.py` uses the existing production Lu Core `ansf_memory_baseline` example and the real pinned Qwen3-0.6B Q4_K_M GGUF. Nine original-source tasks cover repeated project names, cross-chat context, explicit user correction, a stale application version, bad historical advice and nonexistent/current authorization facts. Each model question is identical across arms, randomized per paired trial; only historical source context differs. A real Rust child process cold-opens the ANSF book and verifies the original ID on each memory-on turn.

- The method is explicitly **source-identified**, not an experiment in autonomous source discovery/ranking (that M3 gate stays open). The benchmark's task fixture determines the specific original to re-open, but the expected answer is never placed in the model prompt.
- Raw generation text, model and engine SHA-256, provenance, lookup times, latency and tokens are stored in `results.json`. The output also reports p50/p95 model time, known-answer accuracy and unknown-safety accuracy. The verdict requires greater known-answer accuracy than the no-memory arm **and 100% unknown-safety accuracy** on this deliberately small test set.
- The model has no tool interface in this fixture, so *absence of unauthorized effects* must still be demonstrated by separate native UI/host effect tests, not inferred from harmless text answers.
- A passing headless model benchmark is **not** installed Windows, model-swap, physical Vulkan offload or release acceptance. These need independent witnesses and actual locally available GGUFs.

## Acceptance policy

1. New exact-head native tests must pass on actual GitHub-hosted Linux and Windows, plus the pinned original-source model-input schema.
2. No result counts, accuracy, saved discovery/tool calls or model first-token latency may be reported unless actual measured artifacts exist.
3. Installed Windows must use a reviewed real NSIS package, pinned actual llama-server binaries, physical GPU backend, two genuine GGUFs, WebView input/history UI and OS process-tree witnesses. The M4 physical acceptance harness can be reused as a prerequisite, not a surrogate.
4. Keep **M5 draft and M4 draft** until the distinct gates have passing evidence. Never merge downstream to hide M3/M4 acceptance debt.

**Next:** await genuine native M5-01 and Qwen M5-02 results; repair precise failures at exact head. Then connect real installed Windows memory UI to the M5 physical acceptance trial, followed by same-task multi-model comparison.

## M5-03–05 follow-on source checkpoint (November 7, 2026)

**M5-03 Windows installed app pilot:** New `scripts/m5-windows-installed-memory-pilot.ps1` explicitly requires real interactive Windows, disposable Lu test profile, the installed native EXE and exact packaged inference-engine SHA-256, and **two distinct** reviewed GGUF files. It exercises three real app process cycles (first model, cold restart on first model, second model), checks UIAutomation owner PID and OS worker process identity/reaping and records real window start time. It gathers user-typed source IDs, model outputs and reported functional scenarios (hot-scroll, changed versions, permissions, cancellation/CCM). Script data is clearly `OPERATOR_REPORTED` unless independently OS-observed. **Not executed on a physical PC; GPU-offload and WebView answer truth not independently verified.**

**M5-04 real authority and effect recovery:** New `src-tauri/lu-core/tests/m5_effect_authority_acceptance.rs` compiles production `ConversationAuthority`, `AnsfStore`, `EffectJournal` and `ContinuityStore`. It authenticates the original USER turn, rejects assistant-role privilege claims, wrong conversation, wrong task/user origin, wrong scope, host veto and privilege/effect ceiling; a bounded host-authorized effect performs a real disposable local filesystem write. Then revocation denies a second write, and a fresh registry after restart has no historical grant. A separate test persists EXECUTING and on restart reconciles to UNCERTAIN, forbidding journal receipt linking or reuse under newly admitted read-only intent. **Native OS matrix submitted; results outstanding.**

**M5-05 comparative scorer and gate:** New `benchmarks/memory/m5_comparative_acceptance.py` reads genuine M5-02 raw Qwen generations and exact paired arm cases (if available), validates the computed correct/miss flag against raw single-line model text, and reports known/unknown safety, p50/p95 latency, original lookup costs, tokens and net memory benefits. It can also ingest separate M5 physical-Windows and native CI evidence files as `SUBMITTED_NOT_INDEPENDENTLY_ATTESTED`. No input gives `NOT_SUBMITTED`, malformed input `INVALID_OR_INCOMPLETE`. It **never** declares a release accepted. User effort, model swap times, installed GPU and rediscovery tool-call metrics are intentionally `null` until measured.

**Regression discovered and repaired:** Initial M5-01 actual desktop native CI failed `m5_native_replay_acceptance::repeated_task_control_vs_memory_after_reopen_conflict_and_human_correction` with **`invalid authenticated human turn`** because the test used untrusted malformed `g-m5-*` turn IDs. The fixed fixture now supplies Lu-valid generated-shape `g-<32 hex>` IDs for all three authenticated human turns. The regression is a *test-fixture repair*, not an authorization bypass. Re-run the complete native Windows/Linux tests at the final new exact head.

**Outstanding acceptance:** dedicated M5 exact-head native and real GGUF jobs, real NSIS installed/uninstall and real physical Windows/GPU+second GGUF, independent model-output and OS effect verification, durable source/UI performance, real user pilot metrics; M3/M4 gates remain independently OPEN.
