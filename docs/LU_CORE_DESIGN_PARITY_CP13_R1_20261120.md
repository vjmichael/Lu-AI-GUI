# CP-13-R1 — Current Lu design versus working implementation, November 20, 2026

**Review type:** documented source/evidence reconciliation; **no runtime change, new model download, installer, CI dispatch, or R7 branch modification.**

**Exact implementation snapshot:** `2e0ed022d03bacae73a669e362c41f72a5c1d145` on `lu/cp12b-r7-windows-stage-eval`, original verified 1,262 non-lock source entries. This is the pre-existing published Windows R7-R2 candidate, **not** a declaration that its GitHub Windows CI or physical acceptance passed. The authoring worktree for this document is separate, `lu/cp13-r1-design-parity`.

**Existing governing audit:** [`LU_CORE_DESIGN_TRACE_CP13_20261116.md`](LU_CORE_DESIGN_TRACE_CP13_20261116.md) and [`LU_CORE_DESIGN_PARITY_CP13_20261116.md`](LU_CORE_DESIGN_PARITY_CP13_20261116.md). **Complete R1 delta:** [`acceptance/LU_CP13_R1_REQUIREMENT_DELTA_20261120.md`](acceptance/LU_CP13_R1_REQUIREMENT_DELTA_20261120.md), retaining all **98** original register IDs and Nov 16 dispositions. This is an update to existing CP-13—not a new authoritative design, not 98 new success claims and not a substitute for original requirements.

## 1. Governing design and conflicts

1. **Precedence** is the central design §0 and prior CP-13 source trace: latest *explicit owner* decision, original preserved Jan tool intent where unsuperseded, compatible subsequent approved Lu design, then central synthesis. Implementation artifacts, assistant proposals and CI are **evidence**, not superior design authority. Original Jan runtime, global mandatory sandbox, self-modifying developer agent and DLC scope remain superseded/excluded; broad native OS access is distinct from authenticated per-task authority.
2. **Current Core** is a local-first Tauri/React/Rust desktop with GGUF execution, ANSF/SQLite original records and memory, typed native tools, authorization/revocation, durable effects and model/worker supervision. Actual value is `C_practical = f(K,I,A,F)` (knowledge, interfaces, authority, verified feedback/recovery), not merely compiled features or a tool-count benchmark. Use Foundational / Utility / Specialist categories only.
3. **Nov 16 parity** was scoped *complete as a recovered design register*, not a product release or proof of every lost historical artifact. R1 preserves its 98 entries and previously unresolved general code execution/scratchpad, deep P04/P05 expansion, memory retention thresholds, repair authority and exact old DX-IPC-05/AX artifact questions. New work is evidence disposition and task dependency mapping, not silently treating those questions as resolved.
4. **Version scope:** later November 19–20 records in this checkout have their own test source SHAs and qualification boundaries. This current tree includes subsequent changes, but a prior result does **not** automatically certify this R7 source snapshot. No new installed model or physical Windows tests were performed in R1.

## 2. Actual entrypoint and evidence chain

The ordinary text **Send** path in `lu-desktop/src-tauri/src/lib.rs` (`generate_internal`, especially around lines 5980–6230) now attempts bounded model-led negotiation before ordinary text generation: native web/background handlers, selected reviewed file effects and `capability_negotiation::try_read_turn`. It reuses one authenticated foreground ticket, and can feed bounded ANSF goal, original source, reviewed prior outcome and background task notices back to the same local model. This meaningfully improves the Nov 16 disconnected/partial entries.

However, `lib.rs` has **several distinct routing branches**, and there is no evidence that every one of the 42 registered codecs is universally available or successfully invoked by arbitrary plain-language prompts. `LU_CP06A_TOOL_REACHABILITY_20261119.md` explicitly divided them into ordinarily offerable, reviewed Git, host-confirmed, latent and withheld categories. CP-06D / CP-06E added multiple genuine effect families with original-turn review, exact native effect and verified output. An independently completed user goal requires **user request → offered model schema → verified authority → native handler/worker → real observed result → model response → persisted UI result**. A type check, host-only receipt or fixture-only model selection does not finish this chain.

## 3. Delta findings by product capability

| Binding area | Source and later qualification | Current R1 disposition | Still missing for release-level parity |
|---|---|---|---|
| **Model → tool → answer** (V1-04/06/07, ELA, P01–P12) | `lib.rs` native negotiation/feedback; `LU_CP06A_TOOL_REACHABILITY_20261119.md`; `LU_CP06D_MODEL_CONFIRMED_TRANSFERS_20261119.md`; CP-06E latent-family disposition | **Integrated partially / genuine scoped GGUF-native pass.** Includes real file copy/move and specific file/Office paths; abstentions, misleading environment assertion and latent codecs explicitly retained | Full ordinary Send coverage/negative corpus for every intended category; real source and effect/oracle for each; installed and physical Windows |
| **ANSF SQLite + memory in natural chat** (V1-03/08, ANSF-01–14, MEM) | `lu-desktop/src-tauri/src/lib.rs` memory context and `docs/LU_CP06M_ORDINARY_MEMORY_20261120.md`; prior original/SQLite revisions, retrieval, belief and continuity | **Previously disconnected conversational replacement now source-connected and scoped local GGUF-fixture qualified.** Superseded historical facts and old grants are bounded as data | Full installed Windows/history-source accuracy, long history, backup/export/migration, user correction UI, CP-09M three-arm value study |
| **User-scheduled work and opt-in testing** (P02, AUTO-01/02/05) | `LU_CP11A_R2_ACCEPTANCE_20261120.md`, `LU_CP11B_SCHEDULED_REMINDERS_20261120.md`, `LU_CP11B_R3_CHANGING_FILE_20261120.md` | **Scoped installed Linux pass** for off-by-default model checks, reminder scheduling, recurring selected text-file reads, explicit reapproval/revocation and restart; model answers may fail despite correct custody | Installed Windows + physical lifecycle, broader resources and precise date/time semantics, nontrivial benchmark validity, no privileged automatic worker/permission resurrection |
| **DX diagnostics + collaboration** (DX-00–15G, DX-IPC, AUTO-03) | `LU_CP12B_R5_STAGE_REVIEW_20261120.md`, `LU_CP12B_R5_R1_MODEL_QUALIFICATION_20261120.md`, `LU_CP12B_R6_STAGE_LIFECYCLE_20261120.md`, `LU_CP12B_R7_WINDOWS_STAGE_EVAL_20261120.md` | **Stage evidence/replay/cancel/crash and Linux installed two-model pass for confirmed-delivery; Windows native named-pipe and SID isolation pass.** Production Windows feature default-off | Real installed Windows two GGUF instances, full GUI consent, different-account process and physical Windows; positive actual fault localization (0/6 unique held-out stages for each small-model baseline); map original DX negatives individually |
| **Multimodal/document/model lifecycle** (V1-02, ANSF-12, P04/P10) | `LU_CP01_MULTIMODAL_DELIVERY_ACCEPTANCE.md`, `lu-desktop/tests/cp01-mmproj-delivery.test.mjs`, model picker and `lu-desktop/web/native_windows.jsx` | **Present source/contracts; scoped Linux model reload and inference proven.** Do not infer visual competence merely from MMProj availability | Installed full PDF/DOCX/1/4-image request→model perception→answer→saved source and restart; physical Radeon R9700 Vulkan/CPU and memory/worker recovery |
| **Simple user-facing interface** (V1-09, UX-CHAT01/02, UX-NOV16) | `lu-desktop/web/main.jsx`, `lu-desktop/web/native_windows.jsx`, `lu-desktop/web/unified_tabs.jsx` | **Partially simplified.** Chat/history/tasks/settings/models and separate-window capability exist, but static UI strings still expose technical implementation concepts | Basic user-only vocabulary, advanced diagnostics separated, accessible installed screenshot/walkthrough and preserved original revision/security behavior |
| **Windows installer and end-to-end** (V1-01/10, REL-01, AX) | `LU_CP12B_R7_R2_HOSTED_PUBLICATION_20261120.md`, NSIS sources, real Micro11 native acceptance | **Native Windows transport passed; source SHA published.** CI was in progress at R1 snapshot, not terminal evidence | Exact SHA hosted Windows CI reconciliation, fresh installed NSIS with real two-model GUI and negative account, physical Win10/11 and AMD GPU, upgrade/restart/uninstall preservation, clean dependency provenance |

### The user-facing engineering-language regression is specific

Source-visible labels include `ANSF Library` (`main.jsx` around 430), `Original ANSF text` and SHA-256 (`main.jsx` around 4804–4845), `Original ANSF source provenance · P0-TOOLS-03` (around 5341), and `Local GGUF inference profile` plus detailed CPU/tokens/options (`native_windows.jsx` around 459–500). `native_windows.jsx` also contains technical project/tool administration (`capability ROUTES`, PLAN-03/04, source graphs), although some panels are conditional on whether the window is a dedicated Settings window or a tab. **This is a reproducible code inspection, not a claim that every label is visible simultaneously in a particular build.** Preserve provenance on demand under understandable language; hide implementation machinery behind Advanced rather than deleting safety or data.

### Model result reliability must be measured, not hidden

The R5-R1 held-out stage test shows **0 of 6** correct specific checkpoint localizations for each tested small GGUF baseline, while uncertainty recognition was **2/3** and **3/3** respectively. Both incorrect findings were rejected by the deterministic verifier. The same prior R5 installed test passed a *confirmed-delivery* two-model exchange, which is **not evidence** of positive fault localization. Model accuracy and native security are separate gates. No larger-model download is appropriate solely to decorate CP-13; retain measured baseline and a future optional model gate.

## 4. Evidence hierarchy for every future closure

1. **Design authority:** exact approved owner requirement and current scope/deferred supersession.
2. **Tracked source:** implementation, registration and callable route, named evidence path/commit.
3. **Native contract:** selected user intent, denial, auth scope, durable effect, worker/cancellation/recovery, independent witness.
4. **Genuine GGUF operation:** chosen local model/engine, raw bounded model proposal, native result, checked final answer and persisted receipt; distinct from synthetic stub.
5. **Installed GUI:** real Tauri/NSIS app, visible user consent and no engineering bypass, crash/restart and no automatic privilege restoration.
6. **Physical Windows:** actual Windows host/hardware GPU and all negative/lifecycle scenarios, not a VM or hosted CI surrogate.

A **PASS at a lower tier never automatically advances a higher tier**. Preserve fails/ignored/blocked checks. This R1 review locally reran **42/42 focused Node source-contract tests** spanning tool-turn, memory, automation, DX, Windows evaluation and multimodal; these are not fresh native Rust/model/installed tests. See `CP13_R1_FOCUSED_CONTRACTS.log` in handoff evidence.

## 5. Prioritized existing work (not a competing backlog)

| Priority | Real existing owner/task mapping | Reconciled gap and measurable release condition | Dependency or independence |
|---|---|---|---|
| P0 | **CP-12B-R7-R2 / CP-10** | Collect **one unchanged-SHA** terminal Windows hosted run; reconcile actual job results; then real installed two-model Windows GUI with user approvals, stop/revoke/replay/crash and a distinct user-account process. Physical Windows remains separate | Hosted CI already running: do not queue more CI or change R7 SHA |
| P0 | **CP-05/06E/06F + CP-09** | Ordinary conversational 42-codec coverage register: for every intended user task record correctly offered/abstained tool, grant/refusal, actual native verified effect and model final. Existing deferred/withheld codecs remain scoped, not blanket release blockers | Isolated source/fixture planning can proceed, merged candidate only after prior source lock |
| P0 | **CP-07/08** | Redesign user-facing settings/memory/provenance language. No mandatory `ANSF`, `Protobuf`, source hashes or worker internals in Basic; preserve Advanced audit, consent and accessibility | Independent UX branch; no R7 edits/CI |
| P0 | **CP-01/02 + CP-10** | Genuine installed document and MMProj acceptance from user intake to model answer, source retention and restart, including missing-projector and corrupted-reference negatives | Physical AMD acceptance needed later; no new model downloads now |
| P1 | **CP-06M + CP-09M** | Matched *memory* three-arm benchmark: stateless / recent / ANSF using identical real tasks and model, original/revised-source correctness and long-context truncation, not KIAF execution three-arm | Independent corpus and source fixtures now |
| P1 | **CP-11A/11B** | Installed Windows opt-in checks and scheduled user work; exact report outcomes, future approval, pause/revoke/stop and restart; no network or uncontrolled repair | Independent of current R7 native CI; no stacked CI |
| P1 | **CP-12A/B + CP-09** | Positive fault-localization test on independent native failure/incomplete/multiple evidence; reject invalid model answers and maintain no unsolicited repair. Original DX-15 historical negative assertion checklist | Model capacity gate open; existing model suite for regression only |
| P1 | **CP-10/AX + V1-03** | Exact multi-version installer upgrade/rollback, import/export/backup/old attachments, licensed runtime provenance and clean PATH tests | Requires final exact installer SHA |
| P2 / owner decision | **ELA-06, P04/P05, AUTO-04, AUTO-06** | General-purpose code runner/scratchpad, giant document/transfer expansion and auto-repair require an explicit scope decision; do **not** revive Jan runtime, autonomous modification or DLC by inference | Preserve unresolved/deferred, no implementation in CP-13 |

### Scope notes and go/no-go

- **Do not announce overall v1.0 completion** until V1-10 and physical Windows data/worker/permission gates genuinely pass.
- **Do not equate smaller-model abstention with useful diagnostics**, or complete native tests with live source truth.
- **Do not weaken host authority, cancellation or model-answer verification** to improve apparent pass rates.
- **Do not run new CI while R7's first source-locked Windows run is active**, and do not merge this design-only branch into R7 as an incidental change.
- **Do not revive the original Jan runtime, unrestricted arbitrary execution, desktop engineering screens or post-v1 DLCs.**

## 6. CP-13-R1 disposition

The **98 original design IDs remain fully mapped** to their previous authoritative source/requirements and an updated evidence tier/remaining gate. R1 does **not** claim that every subrequirement or original historical document has been recovered, nor does it mark 98 tests passed. The updated task ledger is the existing `docs/LU_DEVELOPMENT_LEDGER.md`; there is no second task tracker. CP-13 design comparison is complete *as a scoped documentation/source reconciliation* on this independent branch; the measurable runtime and installed closure belongs to the pre-existing CP-01/02, CP-05/06, CP-07/08, CP-09/09M, CP-10/AX, CP-11 and CP-12 requirements above.
