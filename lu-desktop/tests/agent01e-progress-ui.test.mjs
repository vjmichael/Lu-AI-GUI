import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';

const ui=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
const host=readFileSync(new URL('../src-tauri/src/lib.rs',import.meta.url),'utf8');
const progress=readFileSync(new URL('../src-tauri/src/agent_plan_progress.rs',import.meta.url),'utf8');
const bridge=readFileSync(new URL('../../src-tauri/lu-core/src/agent_reviewed_broker_bridge.rs',import.meta.url),'utf8');
const broker=readFileSync(new URL('../../src-tauri/lu-core/src/native_reviewed_app_tool.rs',import.meta.url),'utf8');
const scheduler=readFileSync(new URL('../../src-tauri/lu-core/src/agent_host_scheduler.rs',import.meta.url),'utf8');

test('E: host reads original authenticated chat and never accepts a model path',()=>{
  assert.match(host,/fn lu_agent_plan_progress\(/);
  assert.match(host,/agent_plan_progress::read_chat_progress\(root,&chat_id,&state\.chats\)/);
  assert.match(host,/lu_agent_plan_progress,/);
  assert.match(progress,/SQLITE_OPEN_READ_ONLY/);
  assert.match(progress,/lu-agent-plans\.sqlite/);
  assert.match(progress,/trusted_user_turn_text\(chat,&turn\)/);
  assert.match(progress,/symlink_metadata/);
  assert.doesNotMatch(progress,/pub fn (?:execute|resume|retry|grant|start_worker)/);
});

test('E: original conversation UI shows review and recovery without fake worker success',()=>{
  assert.match(ui,/aria-label="Durable agent planning and recovery"/);
  assert.match(ui,/plan\.revisionReview/);
  assert.match(ui,/plan\.supervisorState/);
  assert.match(ui,/plan\.steps\.map/);
  assert.match(ui,/step\.historicalWitnessLinked/);
  assert.match(ui,/no automatic retry or/i);
  assert.match(ui,/lu_agent_plan_progress/);
});

test('E: independent audit failure cannot erase remaining plan progress',()=>{
  assert.match(ui,/Promise\.allSettled\(\[/);
  assert.match(ui,/setAgentProgressByChat/);
  assert.match(ui,/results\[0\]\.status==='fulfilled'/);
  assert.match(ui,/results\[1\]\.status==='fulfilled'/);
});

test('D/E: actual reviewed broker checks one exact live host and original human authority',()=>{
  assert.match(bridge,/scheduler\.check_live_claim\(/);
  assert.match(bridge,/authority\.permitted_tool_ids\(broker_origin\)/);
  assert.match(bridge,/broker_origin\.user_turn_id!=broker_origin\.task_id/);
  assert.match(bridge,/WAITING_FOR_INDEPENDENT_NATIVE_OS_ACCEPTANCE/);
  assert.match(bridge,/verify_actual_broker_session/);
  assert.match(broker,/pub fn preflight_scheduled_step/);
  assert.match(broker,/verify_actual_broker_session\(/);
  assert.match(scheduler,/fn require_live_owner/);
  assert.match(scheduler,/pub fn check_live_claim/);
  assert.doesNotMatch(bridge,/Command::new|\.spawn\(|run_journaled_observed\(/);
});

test('E: SQLite records are display facts only and tests explicitly cover uncertainty',()=>{
  assert.match(progress,/execution_authority:"NOT_AUTHORIZED_FOR_EXECUTION"/);
  assert.match(progress,/INTERRUPTED_NEEDS_RECONCILIATION/);
  assert.match(progress,/CANCELLED_NEEDS_RECONCILIATION/);
  assert.match(progress,/HISTORICAL_WITNESS_RECORDED/);
  assert.match(progress,/recorded_human_approval_and_interruption_never_mark_worker_proven/);
  assert.match(progress,/inconsistent_schema_is_error_not_fabricated_empty_history/);
  assert.match(bridge,/actual_conversation_authority_not_planner_approval_controls_broker_capabilities/);
});
