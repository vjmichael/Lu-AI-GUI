// Source-structure regressions for AGENT-01G. Exact-head native OS and GGUF
// acceptance is REQUIRED separately; this file is not a runtime receipt.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const host=readFileSync(new URL('../src-tauri/src/reviewed_app_desktop.rs',import.meta.url),'utf8');
const main=readFileSync(new URL('../src-tauri/src/lib.rs',import.meta.url),'utf8');
const ui=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
const witness=readFileSync(new URL('../../src-tauri/lu-core/src/agent_deliverable_receipts.rs',import.meta.url),'utf8');
const bridge=readFileSync(new URL('../../src-tauri/lu-core/src/agent_reviewed_broker_bridge.rs',import.meta.url),'utf8');

test('a distinct native-host command exposes one reviewed plan step, not a model tool',()=>{
  assert.match(main,/lu_agent_run_reviewed_step,/);
  assert.match(host,/pub\(crate\) async fn lu_agent_run_reviewed_step\(/);
  assert.match(host,/fn reviewed_step_operation\(/);
  for(const op of ['git_head','git_status','git_branch','git_history','media_facts']){
    assert.ok(host.includes('"'+op+'"'));
  }
  assert.match(ui,/invoke\('lu_agent_run_reviewed_step'/);
  assert.match(ui,/Review and run one step/);
});
test('host validates approved original ANSF plan and independent witness before picker',()=>{
  assert.match(host,/original_step_for_review\(/);
  assert.match(host,/revision_review!="APPROVED_PLAN_ONLY"/);
  assert.match(host,/supervisor_state!="NOT_CLAIMED"/);
  assert.match(host,/PlanStepKind::IndependentlyCheck/);
  assert.match(host,/ExpectedDeliverable::IndependentWitness/);
  assert.ok(host.indexOf('original_step_for_review(')<host.indexOf('.blocking_pick_folder()'));
});
test('OS picker and second native confirmation precede fresh journal and claim',()=>{
  assert.match(host,/\.blocking_pick_folder\(\)/);
  assert.match(host,/\.blocking_pick_file\(\)/);
  assert.match(host,/Authorize one reviewed agent step/);
  assert.ok(host.indexOf('if !confirmed{return Ok(None)}')<
    host.indexOf('host.chats.append_user_with_turn_id'));
  assert.match(host,/HostFreshStepConsent for NativeScheduledStepConsent/);
  assert.match(host,/scheduler\.propose_step\(/);
});
test('schedule checks current goal, exact claim and per-turn authority before worker runs',()=>{
  assert.match(host,/AgentHostScheduler::open/);
  assert.match(host,/OriginalChatGoalSources/);
  assert.match(host,/scheduler\.claim_host\(/);
  assert.match(host,/reviewed\.preflight_scheduled_step\(/);
  assert.match(host,/ReviewedApplicationTurnSession::from_verified_host_task/);
  assert.ok(host.indexOf('reviewed.preflight_scheduled_step(')<
    host.indexOf('reviewed.run_journaled_observed('));
  assert.match(host,/let budget=schedule_budget\(\)/);
});
test('planned reviewed capability is only one exact Git or media operation',()=>{
  assert.match(host,/if op!=expected\{continue\}/);
  assert.match(host,/let expected=match target.operation/);
  assert.match(host,/if answer.tool_calls!=1/);
  assert.match(host,/matched|unique_independent_reviewed_call/);
});
test('a completed model answer alone cannot create an independently verified result',()=>{
  assert.match(host,/unique_independent_reviewed_call/);
  assert.match(host,/link_verified_scheduled_worker_outcome/);
  assert.match(bridge,/record_verified_execution_turn/);
  assert.match(bridge,/acknowledge_historical_receipt/);
  assert.match(witness,/lu_reviewed_turn_origin/);
  assert.match(witness,/lu_independent_outcomes/);
  assert.match(witness,/VERIFIED_AT_OBSERVATION/);
  assert.match(host,/NOT_AUTHORIZED_FOR_NEXT_STEP/);
});
test('ordinary reviewed-app mode remains a separate explicit native host flow',()=>{
  assert.match(host,/pub\(crate\) async fn lu_reviewed_app_ask\(/);
  assert.match(host,/session_id,chat_id,prompt,mode,None/);
  assert.match(ui,/plan\.revisionReview==='APPROVED_PLAN_ONLY'/);
  assert.match(ui,/plan\.supervisorState==='NOT_CLAIMED'/);
});

test('failure after schedule admission is never silently treated as owner-free',()=>{
  assert.match(host,/struct PlannedStepLifecycle/);
  assert.match(host,/impl Drop for PlannedStepLifecycle/);
  assert.match(host,/self\.scheduler\.request_cancel\(/);
  assert.match(host,/if !self\.released/);
  assert.match(host,/guard\.scheduler\.propose_step\(/);
  assert.match(host,/let mut schedule=if let Some\(target\)=scheduled\.as_ref\(\)/);
  assert.match(host,/guard\.release_after_durable_chat\(\)\?/);
});
test('a verified step can release authority only after durable conversation completion',()=>{
  const persist=host.indexOf('host.chats.complete_reviewed_if_not_revoked(');
  assert.match(host,/&chat_id,&gid_run,revocation_watermark\)\?/);
  const release=host.indexOf('guard.release_after_durable_chat()?');
  assert.ok(persist>=0&&release>persist);
  assert.match(host,/scheduler\.release_without_dispatch\(/);
  assert.match(host,/self\.released=true/);
  assert.match(host,/NOT_AUTHORIZED_FOR_NEXT_STEP/);
});

test('planner offers eligible first independent check without manufacturing general tool authority',()=>{
  const planner=readFileSync(new URL('../src-tauri/src/agent_plan_admission.rs',import.meta.url),'utf8');
  const example=planner.slice(planner.indexOf('JSON object, without Markdown or comments:'));
  assert.ok(example.indexOf('INDEPENDENT_CHECK')>=0);
  assert.ok(example.indexOf('INDEPENDENT_CHECK')<example.indexOf('PRESENT'));
  assert.match(example,/dependsOn/);
  assert.match(planner,/Do NOT change unrelated goals to fit currently reviewed tools/);
  assert.match(planner,/Other step kinds are plan data only/);
});

test('C3 planned execution demands verified independent native Protobuf outcome',()=>{
  const loop=readFileSync(new URL('../../src-tauri/lu-core/src/agent_tool_loop.rs',import.meta.url),'utf8');
  const session=readFileSync(new URL('../../src-tauri/lu-core/src/native_reviewed_app_tool.rs',import.meta.url),'utf8');
  assert.match(loop,/requires_verified_independent_outcome/);
  assert.match(loop,/fn verified_reviewed_oracle\(/);
  assert.match(loop,/GIT_HEAD_DIRECT_V1/);
  assert.match(loop,/MP4_BOXES_DIRECT_V1/);
  assert.match(loop,/ObservationRequiredBeforeFinal/);
  assert.match(loop,/ensure_continuation_authorized\(authority\)/);
  assert.match(session,/verify_actual_broker_session\(/);
  assert.match(session,/self\.turn\.require_verified_independent_outcome\(\)/);
  assert.ok(session.indexOf('verify_actual_broker_session(')<
    session.indexOf('self.turn.require_verified_independent_outcome()'));
});

test('C3 native task-board reconciliation reopens exact original worker and source without auto-completion',()=>{
  const criteria=readFileSync(new URL('../src-tauri/src/project_task_verified_evidence.rs',import.meta.url),'utf8');
  const board=readFileSync(new URL('../web/task_board.jsx',import.meta.url),'utf8');
  assert.match(main,/project_task_verified_evidence::lu_project_task_reconcile_completion/);
  assert.match(board,/invoke\('lu_project_task_reconcile_completion'/);
  assert.match(board,/Reconcile native requirement and original completion evidence/);
  assert.match(criteria,/project_task_agent_handoff::source_anchor\(/);
  assert.match(criteria,/project_task_agent_handoff::independently_verified_task_witness\(/);
  assert.match(criteria,/source_rescanned_this_review:true/);
  assert.match(criteria,/task_completed:false,execution_authorized:false/);
  assert.match(criteria,/STOP_REVIEW_HISTORY_RECONCILIATION_REQUIRED/);
});
