# AGENT-01I — Fresh authorization and continuation transaction (draft)

Stack: AGENT-01H PR #166, deliberately separate from AGENT-01F/G PR #159 and its pending CPU NSIS Windows builder.

## Delivered in this source candidate

- Added `lu-core::agent_continuation_intent::stage_fresh_continuation`, a host-side-only operation. It persists an immutable, single-use, **non-executing** request to review the *next* step of the saved original plan. It does not launch a worker, call a model, mint any ConversationAuthority, adopt a prior scheduler owner, or activate auto-continuation.
- The caller must provide a **different saved authenticated human turn** and an exact plan / immutable revision / next-step / bounded-budget consent witness. The original ANSF goal event and reviewed assumption sources must still be current and accessible.
- The prior scheduler owner must be `RELEASED`, with all prior claims `OBSERVED_VERIFIED_HISTORICAL`. No uncertain, active, cancelled, or recovered owner may be inherited. Steps are sequential and require a direct previous dependency, not a skipped or substituted tool.
- Freshly reopens AGENT-01C receipt history for **each preceding step** and requires `VERIFIED_AT_OBSERVATION` for the same immutable revision; a cached front-end preview is never evidence. The same SQLite write transaction rechecks original plan identity, owner fence, current revision and historical claim count.
- A new `lu_agent_continuation_intents` record is keyed by (plan, revision, step), has a uniqueness constraint on the human step turn, and has no UPDATE/DELETE allowed. An exact retry is idempotent; a different user turn cannot replace the first staged request.
- The returned status is always `NOT_AUTHORIZED_FOR_EXECUTION`; successful staging cannot grant tools or approve execution. This is an explicit transaction-preparation step, not an implemented multi-step runner.

## Acceptance required before execution activation

1. Native Rust/SQLite tests must **actually** compile and pass under Windows 2025 and Ubuntu 24.04 on the exact PR head. The dedicated workflow tests strict IDs, immutable SQL rows and no recycled human turn; these are NOT full real-model or effect tests.
2. Existing AGENT-01G pinned Qwen3-0.6B GGUF must produce genuine model-selected reviewed Git operation, and its actual native worker/task/effect and independent source-oracle receipts must pass. A local Qwen diagnostic is insufficient.
3. Add and test a host-controlled **new execution session or child-plan transaction** for each next step. Do NOT modify AGENT-01D's permanent released-owner fence, reuse the preceding claim, or infer permissions from this intent record. A separate native selection, confirmation, authenticated human turn, scoped broker authority, fresh source oracle and cancellation custody must be verified before any tool execution.
4. Establish genuine Windows Job Object / Linux process custody and installed Windows NSIS UI acceptance. No automatic continuation until the above gates pass.

## Status

Draft source implementation. No local Rust compiler or network Git clone was available to this execution environment; no native compilation, full Qwen inference, application worker, model feedback, or installed NSIS proof was performed here. Workflow completion and artifacts must be inspected before accepting even the native immutable-record stage.
