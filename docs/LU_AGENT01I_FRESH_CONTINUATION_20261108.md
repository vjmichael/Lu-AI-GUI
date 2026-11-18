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


## 2026-11-08/09 source implementation extension — separately authorized child execution

**New production desktop path:** `lu_agent_prepare_next_step` is now a registered Tauri command and a manual **Prepare separate next-step plan** control on an original released plan with a verified predecessor. A native OS dialog confirms an *entirely new* preparation request, which is saved as a new authenticated human chat turn. AGENT-01I then re-verifies the original goal, source assumptions, original released owner, exact dependent step and AGENT-01C independent receipts, commits immutable continuation intent, and creates a distinct one-step child plan in **PENDING_HUMAN_REVIEW**. The child-plan ID is durably bound to its parent and exact human preparation turn; the previous host owner and tool authorization are never inherited.

The existing normal `lu_agent_review_plan` native SHA-specific dialog must separately approve the child plan using a **different saved human review turn**. Only after that is the existing `lu_agent_run_reviewed_step` flow eligible: the human must pick an actual reviewed Git/FFprobe source, approve the exact resource/tool in another OS-native dialog, and create yet another fresh saved execution turn. This latter path allocates a **new child-plan-only AGENT-01D owner**, fresh one-tool ConversationAuthority and separate TaskStore/effect journals, and uses the existing supervised model-selected tool, independent outcome oracle, AGENT-01C receipt and release/restart protections. No code resurrects a released scheduler owner, copies permission tokens between parent and child, or enables unattended continuation.

A critical fail-closed guard was added at both the native picker and immediately before the new child owner claim: the child-to-parent intent, original goal and revision, released parent fence, ordered historical claims and **currently independently re-verified original worker receipts** must all agree. A crash after registering an inert child draft but before writing its lineage link must block child execution as **unlinked requiring reconciliation**, rather than accidentally treating it as an independent plan.

**New real-model acceptance test** `agent01i_genuine_qwen_two_fresh_approvals_independent_git_and_restart` is separately invoked by the existing pinned Qwen3-0.6B GGUF E1-P07 workflow. Its two independent real-Qwen tool decisions must each execute native Git using distinct saved human consent, distinct plan/revision/owner/claim and task/effect/ModelTurnJournal identities, and independently verified direct Git source oracles. It must reopen the persisted journals, deny reclaims/replay of both owners and revoke both tool authorities. It deliberately withholds the original independent-effect journal and requires child source-lineage revalidation to fail until that exact evidence is restored. **Test submitted, not passed; no fabricated tokens or OS processes.**

**Windows installer requested by owner:** current-source AGENT-01G Windows CPU NSIS workflow is configured to build real `Lu` x64 with a pinned CPU llama.cpp and upload an unsigned test-only installer, checksum JSON and hosted install/uninstall evidence. The current PR branch triggers it. There is **no downloadable exact-source installer until the workflow completes successfully**. CPU evaluation installer is not AMD Vulkan acceptance and should be kept isolated from an existing Lu installation. No local GitHub runner, Windows compiler or full cloned repo is present in this tool environment.

**Open gates:** native Windows/Linux Rust + desktop compilation, eight AGENT-01I source-boundary tests, Qwen3 two-step actual token/OS/oracle job, standalone native OS process-custody evidence, hosted install/uninstall, and the user's actual physical Windows NSIS test. No automatic multi-step behavior until these pass. More-than-two-step parent/child ancestry reconciliation remains future work rather than permission reuse.
