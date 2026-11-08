# AGENT-01F — Real GGUF planning output to original ANSF draft plan

**Owner direction:** November 8, 2026. **Branch:** `lu/kiaf-execution-agent01f-20261108` PR #159 stacked on UX-CHAT02 #158. **State:** source implementation candidate only. Source lock and native CI require exact final-head evidence; no Windows installed or genuine GGUF generation run is claimed by documentation.

## K/I/A/F boundary

- **User and memory:** A user explicitly sets an original ANSF goal and resolves (not defers) material clarification questions. They then select **Draft plan with Lu** on one normal text-only Send. Lu's existing `memory_desktop::model_context` and bounded ANSF recall supply source-backed context. Untrusted retrieved content cannot approve anything.
- **K:** the same loaded GGUF backend used by normal `lu_generate` responds to a bounded exact JSON proposal `{"luPlanV1":{"steps":[{"kind":"COLLECT_SOURCE","dependsOn":null,"expected":"SOURCE_REFERENCES"}]}}`. There are at most eight ordered entries, from the existing six plan step kinds and six expected-deliverable types, with only backward dependencies. Unknown fields/types, duplicate step fields, trailing unrelated output and prose answers do not stage a plan. The model never submits command/path/binary/grant fields.
- **I:** only completed model responses, with the same persisted human turn, feed `agent_plan_admission::stage_completed_model_plan`. The host verifies the goal event has not changed during inference, and uses host-derived stable plan/revision/deliverable IDs to write an immutable AGENT-01A/B draft. It initializes **empty** AGENT-01C/01D SQLite structures for the read-only original-conversation history. This is not a worker, tool invocation or independent outcome.
- **A:** one-turn opt-in, original saved human turn and current goal are checked before and after inference. No new tool grants; plan approval still requires separate exact-revision native dialog and fresh human turn. Existing `agent_reviewed_broker_bridge::preflight_reviewed_broker` now restricts an approved scheduled step to current read-only reviewed-source/inspection/independent-witness kinds; it retains exact claim, live owner, source, fresh turn, cap and registry checks. It still does **not** call `run_journaled_observed`.
- **F:** unparseable model output is persisted as the original assistant answer but gets a visible **not admitted** message, never a plan/effect. A valid response gets an original-conversation plan record and review notice. The parser and step validation are structural; they do NOT validate the model's substantive strategy or completion. A model or OS-exit success cannot attest a verified deliverable; the real reviewed-app worker must produce APPLIED inner/outer journals and a separate independent outcome source before agent result claims.

## Tests and acceptance evidence

| Test group | What it actually covers | Status |
|---|---|---|
| Real portable Rust ChatStore + ANSF/SQLite | Valid draft, same-turn idempotency, restart, interrupted two-phase recovery, strict model response, hostile fields, wrong chat, stale goal/clarification, original plan history projection | Committed source, native CI pending |
| AGENT reviewed-broker guard | Refuse to dispatch a presentation/human-review step through reviewed Git/FFprobe; existing exact owner/authority tests remain | Committed source, native CI pending |
| Node source contracts | Actual `lu_generate` opt-in, complete-result staging, current goal binding, exclusion from tool proposal side channels | Source only, not runtime acceptance |
| React and Windows Tauri | Full desktop build, model UI compile and native host | Exact-head CI pending |
| Genuine GGUF E2E | Locally loaded GGUF, plan proposal token output, successful/invalid plan display, current ANSF memory, user review, restart and changed goal | NOT RUN |
| Reviewed step to independent deliverable | New human step consent + scope + reviewed installed Git/FFprobe, live OS custody, outer/inner APPLIED effect, independent VERIFIED witness + model/user feedback | NOT IMPLEMENTED / NOT ACCEPTED |

## Open implementation tasks

1. Finish durable *concurrent* plan-admission serialization and broader planner relevance/clarification evaluation. Avoid human-turn ID duplication or model-generated fake user IDs.
2. Bind an **approved immutable plan revision plus one fresh human step request** to an in-process host-owned AGENT-01D claim, existing reviewed application native picker and source evidence. Actual broker call must use `ReviewedApplicationTurnSession::preflight_scheduled_step` for the exact operation and per-step registry.
3. Only after native E1-P03/P06/P07/P08C/P10/P12/P13/P15 acceptance allows the promise, run the already existing `run_journaled_observed` with task journal + independent fact oracle; link its verified receipt to AGENT-01C expected deliverable. Do not invent unverified success, retry uncertain writes or extend capabilities outside the user's task.
4. Observe actual Windows/Linux CI results and repair specific errors; real GGUF benchmark and installed Windows NSIS acceptance near v1.0 are independent gates.

**This stage is model-to-reviewable-plan continuity, not a finished autonomous execution system.**
