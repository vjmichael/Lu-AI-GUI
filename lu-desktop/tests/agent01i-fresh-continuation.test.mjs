// AGENT-01I: source wiring guards, NOT native process or real-GGUF proof.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const core=readFileSync(new URL('../../src-tauri/lu-core/src/agent_continuation_intent.rs',import.meta.url),'utf8');
const host=readFileSync(new URL('../src-tauri/src/agent_plan_continuation_desktop.rs',import.meta.url),'utf8');
const app=readFileSync(new URL('../src-tauri/src/reviewed_app_desktop.rs',import.meta.url),'utf8');
const lib=readFileSync(new URL('../src-tauri/src/lib.rs',import.meta.url),'utf8');
const ui=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
const preview=readFileSync(new URL('../src-tauri/src/agent_plan_progress.rs',import.meta.url),'utf8');
const model=readFileSync(new URL('../../src-tauri/lu-core/tests/e1_reviewed_model_tools.rs',import.meta.url),'utf8');
const ggufCI=readFileSync(new URL('../../.github/workflows/lu-e1-p07-genuine-gguf.yml',import.meta.url),'utf8');

test('fresh continuation is immutable review intent and never a worker grant',()=>{
  assert.match(core,/pub fn stage_fresh_continuation\(/);
  assert.match(core,/AWAITING_NEW_HOST_REVIEW/);
  assert.match(core,/NOT_AUTHORIZED_FOR_EXECUTION/);
  assert.match(core,/lu_agent_continuation_no_update/);
  assert.match(core,/lu_agent_continuation_no_delete/);
});
test('native OS confirmation saves a fresh HUMAN turn and new child draft',()=>{
  assert.match(host,/blocking_show\(\)/);
  assert.match(host,/append_user_with_turn_id\(/);
  assert.match(host,/stage_fresh_continuation\(/);
  assert.match(host,/stage_initial_draft\(/);
  assert.match(host,/bind_child_plan\(/);
  assert.match(host,/PENDING_HUMAN_REVIEW/);
});
test('the original released owner cannot be resurrected for a new child',()=>{
  assert.match(host,/progress\.supervisor_state!="RELEASED"/);
  assert.match(core,/owner\.state!="RELEASED"/);
  assert.match(core,/child_plan_id TEXT NOT NULL UNIQUE/);
  assert.match(core,/parent==child/);
});
test('current original ANSF and each independent predecessor source are verified',()=>{
  assert.match(core,/goal_event_is_current/);
  assert.match(core,/assumption_source_is_readable/);
  assert.match(core,/receipts\.history\(/);
  assert.match(core,/VERIFIED_AT_OBSERVATION/);
  assert.match(core,/original_row/);
  assert.match(core,/owner_row/);
});
test('orphan child after crash is refused, not executed as an ordinary plan',()=>{
  assert.match(core,/Unlinked continuation draft requires reconciliation/g);
  assert.match(core,/lu_agent_continuation_intents/);
  assert.match(core,/pub fn reverify_child_before_execution\(/);
});
test('lineage rechecked both before OS picker and before new broker claim',()=>{
  assert.equal((app.match(/reverify_child_before_execution\(/g)||[]).length,2);
  assert.match(app,/scheduler\.claim_host\(/);
  assert.match(app,/preflight_scheduled_step\(/);
});
test('UI only prepares a child plan, then demands separate review and authorization',()=>{
  assert.match(lib,/agent_plan_continuation_desktop::lu_agent_prepare_next_step/);
  assert.match(ui,/Prepare separate next-step plan/);
  assert.match(ui,/invoke\('lu_agent_prepare_next_step'/);
  assert.match(ui,/Review and run one step/);
  assert.match(preview,/PREPARE_NEW_CHILD_PLAN_ONLY/);
});
test('CI requires two raw Qwen model choices, native worker receipts and restart proof',()=>{
  assert.match(model,/async fn agent01i_genuine_qwen_two_fresh_approvals_independent_git_and_restart/);
  assert.match(model,/let mut model=GenuineGgufStep/);
  assert.match(model,/let mut child_model=GenuineGgufStep/);
  assert.match(model,/link_verified_scheduled_worker_outcome/g);
  assert.match(model,/AGENT01I_TWO_STEP_REAL_QWEN_AND_NATIVE_ORACLE_PASS/);
  assert.match(model,/forbidden_child\.permitted_tool_ids\(&child_origin\)\.is_err\(\)/);
  assert.match(model,/child_auth\.permitted_tool_ids\(&child_origin\)\.is_ok_and\(\|names\|names\.is_empty\(\)\)/);
  assert.match(ggufCI,/agent01i-two-step-real-qwen-native\.log/);
  assert.match(ggufCI,/test result: ok\. 1 passed; 0 failed;/);
});
