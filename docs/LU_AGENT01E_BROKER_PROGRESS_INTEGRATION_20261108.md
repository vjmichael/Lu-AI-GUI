# AGENT-01E — Reviewed broker binding, original-chat plans and human recovery

**Date:** November 8, 2026. **Stack:** AGENT-01A/B/C/D draft PRs #152–155 on E1-P14; independently merges E1-P15 conversation-recovery draft PR #151.  
**Acceptance:** Draft source/native test candidate. Absolutely no claim of installed Windows execution, GGUF-generated planning, safe third-party process lifetime, or unprivileged Linux cgroup-v2 custody.

## What was implemented

### 1. Bridge AGENT-01D planning to the real reviewed broker's capability boundary

The new Lu Core `agent_reviewed_broker_bridge.rs` preflights a scheduled proposal using **actual production types**: `AgentHostScheduler`, `AgentPlanLedger`, `AgentPlanRevisionStore`, `ConversationAuthority`, `BoundCallOrigin`, and `ReviewedApplicationTurnSession`. The existing live broker session exposes `preflight_scheduled_step` and explicitly checks its registered capability via `verify_actual_broker_session`.

For a scheduled step to pass this preflight:

- The current revision remains `APPROVED_PLAN_ONLY`, matches the immutable plan and human chat, and still references a current ANSF goal with accessible assumptions.
- The step is still in `PROPOSED_NO_EXECUTION` state and belongs to the exact claim, revision, owner ID, immutable epoch, and **original live scheduler instance**. Merely reopening SQLite with the same owner ID does not recover this in-memory ownership fence.
- The separate **fresh saved human turn** for that step exactly matches the existing reviewed broker's `BoundCallOrigin` (`conversation_id`, `user_turn_id`, `task_id`); the initial planning turn and plan-review turn do not substitute for it.
- `ConversationAuthority.permitted_tool_ids` independently includes both the exact host-reviewed operation ID and its matching **resource-read** ID. Valid combinations are Git HEAD/status/branch/history + repository read or FFprobe media facts + media read. A Git grant cannot be repurposed to media.
- The actual `ReviewedApplicationTurnSession` registry must expose that specific model-visible reviewed tool as a registered native `execute_code` contract. Schemas and capability discovery are not permissions.

**The return value is only `WAITING_FOR_INDEPENDENT_NATIVE_OS_ACCEPTANCE`, never an execution permit.** This contract does NOT call the broker's `run_journaled_observed` execution method, launch a native process, grant resource selection, authorize an old effect ID, or assume E1 OS tests have passed.

The current reviewed desktop still uses the original independently authorized direct-turn broker; the new scheduling bridge **does not intercept or enable it automatically**. An active agent dispatch bridge, per-step runtime quota enforcement and installed-app OS custody remain separately gated.

### 2. Merge reviewed conversation history with AGENT plans

The integration branch safely combines independent changes from E1-P15's original-chat recovery and AGENT-01D's plan/revision/receipt/scheduler hierarchy. These changed disjoint production paths; their combined source manifest is regenerated at the final head. The existing reviewed-app audit and conversation recovery timeline remain available, including its clear distinction between `APPLIED` and independently verified outcomes.

The new `agent_plan_progress.rs` opens **only** the host's fixed `lu-agent-plans.sqlite` under the Lu profile. It is read-only, checks regular files/sidecars and database bounds, validates actual saved human USER origin through `ChatStore`, and returns a maximum 16 plans and 32 steps each. No WebView-provided filesystem path, SQL, worker call or grant is permitted. A missing database returns no plans; an incomplete/linked/corrupt database returns a visible error and is never silently rebuilt.

The registered `lu_agent_plan_progress` Tauri command accepts ONLY the selected chat ID and returns normalized states: original human turn, latest revision and exact digest, review status, assumption-source count, historical owner/cancellation/recovery state, typed step purpose/expected deliverable, dependency, previously recorded scheduler proposal and historical witness reference. A previous witness is NEVER claimed current without AGENT-01C independent source revalidation. No scheduler worker is spawned by this read.

The **Native tool activity** UI now presents:
- Existing genuine model/tool and reviewed-app evidence from the original conversation, including E1-P15 recovery details.
- AGENT plan review/progress, dependencies, historical witness links, cancellation intent, interrupted/uncertain proposals and next safe action within the same conversation. Both reads use independent `Promise.allSettled` so damaged AGENT history cannot hide native tool audit records.
- Explicit human review controls and cancellation intent with actual backend actions; no decorative Continue/Resume button or automatic worker retry.

### 3. Native-confirmed, exact-revision human approval/rejection

`lu_agent_review_plan` opens a **real native desktop dialog** listing the selected immutable revision's typed steps/outputs, SHA, and that approval does not run any tools. A WebView click alone is insufficient; cancellation of the native dialog returns no action. On confirmation, Lu writes a fresh actual saved USER turn in the original chat and calls the production `AgentPlanRevisionStore::record_review` with a host-only exact-decision verifier. The revision is rechecked against the current SQLite head, the original user turn, and current goal.

`OriginalChatGoalSources` verifies the original conversation's real stored ANSF goal-event sequence and current projected goal. Assumption-source references are deliberately **NOT** considered readable without a reviewed host source resolver: approvals involving them fail closed (rejection is allowed), and the UI shows a source-check count and disables approval.

A previously approved revision cannot be reapproved under a different digest, user turn, or decision. No `ConversationAuthority` tool task or native broker grant is opened during review.

### 4. Native-confirmed cancellation intent / interruption recovery

`lu_agent_cancel_plan` likewise requires a real native OS confirmation, authenticates the selected original chat's plan, then calls production `AgentPlanLedger::request_cancel` and writes a new human chat notice. This blocks new planning steps; **it is not an OS cancellation, signal, descendant cleanup, or reaping receipt.** The read-only UI displays `CANCEL_REQUESTED`, `RECOVERY_REQUIRED`, `CANCEL_INTENT_UNCERTAIN` or `ABANDONED_UNCERTAIN` as recorded. There is intentionally no unsafe automatic resume.

The original AGENT-01D scheduler has an additional `check_live_claim` method, which verifies a real process-local ownership fence before any broker preflight. This prevents a restarted process from converting old SQLite owner IDs into new broker admission.

## Native and UI acceptance

New exact-head `.github/workflows/lu-agent01e-broker-progress.yml`:
- Ubuntu 24.04 and Windows Server 2025 compile/test **actual** Lu Core bridge and actual Rust SQLite scheduler/parent plan modules. The bridge tests verify current reviewed Git read+execute grants, missing/revoked/wrong-tool authority, stale ANSF sources, wrong original turn, and a restarted scheduler's loss of in-memory authority.
- Portable real production ChatStore tests cover read-only progress, chat isolation, restart, human approval vs uncertain process state, corrupt partial SQLite, Linux symlink refusal, original ANSF goal supersession and exact native decision matching.
- Six Node/UI contract tests verify that real Tauri commands are wired, independent audit failures don't hide AGENT history, no runtime dispatch in bridge, and human review/cancel use native confirmation.
- Windows React/Tauri production compile with clearly marked **nonfunctional** CI-only engine placeholders; this is **NOT installed-app acceptance**.
- Exact source-tree verification and published head/OS job logs. No current-host native run can be claimed successful until the latest individual job logs pass.

**OS tool receipts in these tests are synthetic fixtures**, not actual native subprocess acceptance. All previous E1 gates remain independently open.

## Remaining tasks and sequencing

1. Review exact-head Windows/Linux compiler and test reports for AGENT-01A/B/C/D, E1-P15, and this integration. Fix concrete failures and re-freeze before merging any draft.
2. Prove E1-P03 genuine Windows Job Object worker, P06 actual Git/FFprobe, P07 loaded GGUF choosing a reviewed tool, P10/P12 semantic output, P13 revision custody, E1-P15 per-turn recovery, and separate Linux P08C delegated cgroup-v2 descendant custody.
3. Integrate a **host-created plan** from authenticated goal/context into `lu-agent-plans.sqlite`. The new UI currently displays existing plans but does not invent or auto-create plans from untrusted model responses. It cannot yet show progress for ordinary chat requests without a registered AGENT plan.
4. Only after step 2 passes, bind genuinely host-reviewed resource selection and per-new-turn authority at the **actual broker's OS admission/dispatch boundary**, with real kernel/process lifetime and runtime resource accounting. A preflight `WAITING_FOR_INDEPENDENT_NATIVE_OS_ACCEPTANCE` is not permission.
5. Expand production host ANSF source resolver to verify assumption references before enabling their approval. Add true live cancellation and verified process/descendant termination receipts; never infer them from SQLite.
6. Perform actual installed NSIS Windows + GGUF/Vulkan + chat switching/recovery + plan approval/cancellation/end-to-end acceptance near Lu v1.0, per the owner's release preference.

Do not claim active agent execution, production approval of assumption-bearing plans, OS process cleanup, or full installed UI success merely because the source and test suite exist.
