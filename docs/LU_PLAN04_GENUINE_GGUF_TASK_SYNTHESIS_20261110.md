# PLAN-04 — Genuine local-GGUF project task synthesis

**Scope:** Lu Core + normal installed desktop Settings in existing `vjmichael/Lu-AI-GUI`; stacked on PLAN-03 draft PR #209. No IDE work, no new repo, no PLAN-05 task register, no execution or GitHub issue mutation.

## Host pipeline

1. **Human approval:** the operator opens Settings → *Analyze project with loaded GGUF…* and approves a native OS message dialog. The operator then selects the source directory through the native folder picker. The loaded model must be an actual ready source-pinned GGUF session with its original `sessionId` and `modelRef`. A cancelled picker/consent makes no proposal.
2. **CPU-first evidence:** PLAN-02 performs the bounded selected project scan (requirements, files, native branch refs and declared run reports); PLAN-03 audits a SHA-bound deterministic status classification. This stage invokes **no model** and performs no filesystem mutation in the project.
3. **Bounded model input:** an audited `lu-project-plan-evidence-v1` capsule with original requirement IDs, source/line, status, confidence, evidence freshness, referenced evidence node IDs, and required next checks. It includes at most 24 requirements and reports partial coverage. Project source/document text is explicitly untrusted data, never a user or system instruction.
4. **Real inference:** the SAME `TrustedInferenceSupervisor<PersistentLlamaBackend>`, `GenerationQueue`, existing llama.cpp GGUF session, and original Protobuf `TextChunk` decoder as normal chat; native Rust host makes an actual generation and records the original model-generated text SHA. No mock inference, new worker, browser, shell, network, new native tool grants, synthetic human chat, or fallback data.
5. **Host validation:** strict model JSON `lu-project-model-tasks-v1` (unknown fields rejected), exact requirement ID and at least one same-requirement graph node ID, status equality with PLAN-03, bounded fields, priority, stable host-derived `lu-task-` ID, backward-only task dependency indices (acyclic), acceptance criteria and blocked-by conditions. A model cannot declare tasks complete, accepted or authorized; these fields are host-issued `false`.
6. **Post-inference source review:** re-scan operator-selected source and branch after generation. Compare typed original graph nodes/edges and critical PLAN-03 findings without confusing the timestamp with source identity. A source, branch, status or session change rejects publication. Inference failure/timeout/invalid JSON produces explicit incomplete/error rather than a fabricated task list.
7. **Durable ANSF:** rederive the entire accepted task proposal from the original raw model reply and audited graph/status, append original reply bytes and SHA-256, prompt SHA-256, original model/session/generation, and immutable proposal digest into an additional table in existing ANSF `ansf/library.db`. Reopen original bytes and run host audit before transaction commits. Historical task proposals are never overwritten. Drafts are **not** a user-approved task register and may not schedule or execute operations.
8. **Presentation:** Settings shows grouped draft tasks with proposed priority/status/dependencies, source/evidence IDs, acceptance criteria, blockers and an **updated engineering priorities** section LAST. Operator review is required. PLAN-05 handles persistent human revisions/corrections; PLAN-06 later adds complete task-register filtering/editing.

## Limits and exclusions

Up to 24 ranked unresolved requirement leads (BROKEN → BLOCKED → NOT_INTEGRATED → PARTIAL → MISSING → UNVERIFIED → UNKNOWN, then original document line) and at most 16 task suggestions can enter one model proposal. Input also has a strict 10 KiB requirement-content budget; counts of omitted and already-accepted requirements are included in the source capsule. A larger project remains explicitly incomplete; no silent whole-project completeness is claimed. A model may produce fewer suggestions than requirements; their omission is disclosed, not silently marked done. A correct typed proposal is NOT proof that its tasks are correct. Only native criteria with independently observed results can be marked accepted by PLAN-03; no model can alter that.

## Required acceptance

- Unit tests: schema, invented requirement/citation/status, missing citation, duplicate task identity, forward/cyclic dependency, model instructions attempting tool authorization, stale graph and tampered proposal.
- Native Windows/Linux: source lock; frontend/Settings picker authority; real Rust Core validation; Tauri compilation; ANSF immutable append and raw reply readback.
- Genuine Qwen3: original native generated evidence fixture → pinned actual llama.cpp CPU engine (`2bc46baa9207d098d03741ac394e7faaf1a955b6`) → SHA-256 pinned Qwen3-0.6B Q4_K_M GGUF (`7af3fdf842f87b24672f8a7f1dd50404043f0bfb71093ff91c31d2b49df4631d`) → unmodified `/completion` → original Lu Core validator. Invalid responses fail, no model reply rewriting or prefilled task JSON. This separate calibration is not physical Radeon R9700/Qwen 27B acceptance.
- Installed OS approval, cancellation/recovery, physical local GGUF task generation, the full large-repo comparison rubric and operator task correction/editing remain open unless proven by separate native evidence.

## Next (separate task group)

**PLAN-05:** durable versioned project task register, human corrections and source-change reconciliation, without reusing model/worker authority. PLAN-06 desktop task-list editor and PLAN-07 incremental evidence refresh follow.

### Actual first-model evidence and protocol repair

The first pinned real Qwen3-0.6B execution reached a healthy model and generated a 573-byte response (SHA-256 `c3ad0a704c7f48135bb86821ccfd9cddac740ea18f4850bb8129a5977ab14bb2`) but included Markdown fences and copied exemplar fields; the original Lu Core JSON parser **rejected it**. This is an actual model-quality/serialization failure, not a model pass. A subsequent reviewed test-only protocol uses the pinned llama.cpp **generic JSON GBNF** (after a literal decoder preflight) and the exact host-created requirement/status hints. The grammar constrains syntax only, never fills task ID, status, source evidence, or acceptance outcome. The same strict host validator remains authoritative. A passing source or mock test does not close the real GGUF gate.
