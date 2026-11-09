# AGENT-01G — One approved plan step through Lu's actual reviewed model/tool broker

**Status:** SOURCE IMPLEMENTATION CANDIDATE; queued CI, no claimed native/runtime/physical acceptance. **Branch:** `lu/kiaf-execution-agent01f-20261108`; draft PR #159 stacked on UX-CHAT02 #158. **User direction:** resume K/I/A/F execution-layer work instead of building more specialist tools.

## One-step host-governed execution boundary

1. **Current original task (K + memory):** A GGUF model's normal conversation turn has proposed a strict, bounded AGENT-01F plan using Lu's current original ANSF goal and memory. A separate native confirmation has approved exactly one immutable plan revision. This is not execution authority.
2. **Fresh A for one concrete step:** In the original conversation's Agent work plans UI, select an eligible `INDEPENDENT_CHECK / INDEPENDENT_WITNESS` step and an exact operation (reviewed Git HEAD, tracked status, branch, history or FFprobe media facts). The host rechecks the plan/revision/goal/clarifications and previous independent dependencies, opens a native Git folder/WAV/MP4 picker, validates the selected reviewed binary, and requires a SECOND native confirmation displaying the exact selected source, tool pair, plan, revision, step and program digest. Cancellation creates no permission. A successful confirmation creates a NEW saved original human `g-` turn.
3. **Host fenced I:** The host creates an AGENT-01D in-process owner and immutable single-step claim using `HostFreshStepConsent`. A true `ConversationAuthority` grants only the host-reviewed operation and its read pair for that fresh turn. `ReviewedApplicationTurnSession::preflight_scheduled_step` checks real owner, claim, revision, source, source-read status, exact original task authority and registry. No arbitrary model-selected executable/path/arguments or Lu-wide sandbox.
4. **Real worker run:** The loaded GGUF receives the current source-backed goal context plus the **single** selected reviewed tool definition. The actual existing `run_journaled_observed` drives model-chosen tool selection, protected Git/FFprobe native worker, cancellation and separate durable inner/outer journals. Both one-tool limit and current ANSF clarification status are rechecked before publishing a plan result.
5. **F / independent result:** The host selects one inner effect from the fresh turn's actual effect+independent-outcome journal, and AGENT-01C re-reads the task-completed, APPLIED/VERIFIED receipt, matching original chat+fresh execution turn E1-P15 origin singleton, source method and observed SHA. Only then AGENT-01D acknowledges the step as `OBSERVED_VERIFIED_HISTORICAL`. The assistant's final text is saved with an appended **host-authored historical source observation**. This does not verify the entire explanation, assure current source state forever, grant a next step or declare OS descendant termination.
6. **Failure/restart:** Cancelled, stale, duplicated, multi-call, missing, mismatched, interrupted or unsupported outcomes never become a verified step; journal history remains and no effects are automatically replayed. An old host owner cannot be claimed by a new process. UI now tells the user to investigate OWNED/possibly interrupted plans instead of blindly retrying.

## Implementation and acceptance separation

| Check | Status |
|---|---|
| `reviewed_app_desktop.rs` exact plan command, native picker, second consent, real per-turn host origin and review-only operation catalog | Implemented in GitHub source |
| Existing model/native supervised worker called after scheduler live preflight, one authorized read-only tool | Implemented in GitHub source, **not executed here** |
| AGENT-01C/01D verified effect join and original plan historical receipt, conversation-visible independent witness | Implemented in GitHub source, **not OS-accepted** |
| Unknown operation, wrong origin/step, unresolved dependency/clarification, stale plan, multi-call and missing oracle refusal | Source checks and submitted tests; real OS proofs pending |
| Source-only Node static integration contracts (10), actual Windows desktop native Rust guard tests (7), React/Tauri full build and exact-head REPO-01 lock | Submitted to native CI; not a pass until logs complete |
| Real GGUF proposes plan, user approves, separate selected Git/media operation, real worker and independent oracle | **Not run or accepted** |
| Full multiple-step planner/scheduler, handoffs for utility and specialist apps, stateful continuation and context-change handling | **Not implemented** |
| Linux cgroup-v2 escape worker and Windows Job Object process-tree custody, cancellation and restart E2E | **Not accepted** |
| Physical installed NSIS/AMD/Vulkan/Win11 release-level E2E | Deferred nearer v1.0 |

## Next actual work order

1. Obtain complete exact-head Windows/Linux CI and logs, fix specific compiler, type, tests, schema, UI source-contract failures without pretending queued results are passes.
2. Perform genuine selected GGUF, reviewed Git/FFprobe and native source-oracle acceptance with actual worker process evidence. Keep original user and fresh step IDs distinct.
3. Introduce a durable multi-step host continuation/session policy *after* verified evidence and user re-approval, including more deliverable types and non-duplicating interrupted-step reconciliation. Never let AGENT-01D's persistent owner row become automatic restart authority.
4. Benchmark K/I/A/F actual task outcomes. Physical installed-Windows NSIS acceptance remains near v1.0 and separate from native CI.

This is a **bounded, explicitly human-authorized one-step candidate**, not an autonomous self-approving plan executor.

## 01G-R1 — fail-closed host lifetime and executable first-step preparation (November 8)

**Source-level repair, native acceptance pending:** The exact-head AGENT-01G source initially had a gap: after `AgentHostScheduler::claim_host` succeeded, a failed plan proposal, model rejection, worker cancellation, missing independent oracle, or conversation persistence failure could drop the owner without recording the uncertainty. This is not evidence that the worker stopped.

- Added `PlannedStepLifecycle` owned immediately after the irreversible scheduler claim. On any early return its `Drop` calls the production `AgentHostScheduler::request_cancel`, durably marking `CANCEL_REQUESTED` and any proposed step `CANCEL_INTENT_UNCERTAIN`. This does **not** claim OS child termination, rollback, or automatic retry. A late cancellation after APPLIED stays historically inspectable.
- After one source operation receives an independently verified original AGENT-01C witness and the chat history `complete` call durably succeeds, the owner runs `release_without_dispatch`. Only that path marks the lease released and prevents the fail-closed Drop. Existing AGENT-01D one-host-per-plan semantics remain; release cannot become restart authority.
- Adjusted the bounded AGENT-01F planning example so a standalone eligible Git or WAV/MP4 independent check may be the **first** step with no prerequisite; the prior sample placed a source-collection prerequisite ahead of the only natively executable AGENT-01G step, making the example inoperable. The prompt explicitly forbids changing unrelated user goals to fit current tool coverage. Source-collection, explanation and generic app tasks are still plan data, **not newly executable tools**.
- Added a genuine source-level Rust/SQLite test `claimed_step_lifecycle_drop_records_uncertain_cancel_intent` that creates a real ChatStore original human turn and ANSF goal, stages and approves a real immutable plan, claims a production scheduler owner, drops the lifecycle without outcome, and asserts persisted `CANCEL_REQUESTED` on reopen.
- Extended the AGENT-01G source-contract matrix to 10 Node cases; the actual Windows reviewed-app native suite expects 7 Rust tests. These tests are now *requested from CI* but have not been observed passing.
- Still open: actual Windows/Linux compiler and runtime results; genuine local GGUF selecting and invoking Git/FFprobe; OS-level cancellation descendant custody and independent source method validity; one-plan multi-step scheduling and restart recovery; physical installed Windows validation near v1.0. Do not report these as accepted merely because source is committed.
