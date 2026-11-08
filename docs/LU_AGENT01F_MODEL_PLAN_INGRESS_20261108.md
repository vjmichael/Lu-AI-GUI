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

## Stage 3 — fresh reviewed worker turn to original goal deliverable (source only)

**Correctness repair:** The original authenticated human turn that created the plan is an immutable *plan origin*, not the newly authenticated user turn required to execute one approved step. Earlier AGENT-01C `record_observed` looked only at original-turn SQLite worker journals, so it could not safely associate actual later step effects with the original plan.

The source now provides:
1. A `record_verified_execution_turn` API that binds **plan origin**, **exact fresh human step turn**, **live AGENT-01D scheduler claim**, current immutable revision, and original chat; a raw model result or old plan approval cannot supply this identity.
2. A check against the actual E1-P15 per-worker `lu_reviewed_turn_origin` singleton (conversation ID + execution turn ID), the exact worker TaskStore journal and inner/outer APPLIED effects, and a separate `VERIFIED` outcome with matching receipt digest/method/source witness.
3. An additive `execution_turn_id` SQLite receipt reference: historical rows with NULL retain old original-turn scope; genuine fresh-turn rows reopen the actual fresh journal after Lu restart. Missing/modified proof is explicitly `NEEDS_RECONCILIATION`.
4. A broker `link_verified_scheduled_worker_outcome` helper that accepts only matching reviewed Git/FFprobe effects and, after independent evidence re-validation, acknowledges the claim as `OBSERVED_VERIFIED_HISTORICAL`. It does NOT run a worker and does NOT automatically authorize another step.
5. Native tests for old/new SQLite schemas, wrong/missing fresh authorization and E1-P15 user-turn singleton, independent effect association and restart. CI is queued pending exact new head and **not** a pass.

**Next missing runtime boundary:** create the user's fresh step confirmation and broker session in the desktop, with the host-reviewed selected repository/media and exact authority, call the existing supervised `run_journaled_observed` only after the relevant native OS E1 gates are independently accepted, then call the new proof join for the exact worker call and show verified/unsupported/conflict with recovery. Existing source integration alone has not invoked a GGUF worker or OS child.
