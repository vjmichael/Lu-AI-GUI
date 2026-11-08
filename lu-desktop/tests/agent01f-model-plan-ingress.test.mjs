// Source-contract regression only. Real GGUF/Windows acceptance is separate.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const host=readFileSync(new URL('../src-tauri/src/lib.rs',import.meta.url),'utf8');
const admission=readFileSync(new URL('../src-tauri/src/agent_plan_admission.rs',import.meta.url),'utf8');
const web=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
test('explicit user opt-in reaches the existing loaded GGUF generation path',()=>{
  assert.match(host,/plan_only:\s*Option<bool>/);
  assert.match(host,/GenerationOrigin::Human,true/);
  assert.match(host,/agent_plan_admission::planning_prompt/);
  assert.match(host,/supervisor\.run_reserved_with_history/);
  assert.match(web,/planOnly:\s*usePlanDraft/);
  assert.match(web,/Draft plan with Lu/);
});
test('model plan bytes are parsed only after a complete original model turn',()=>{
  assert.match(host,/state\.chats\.complete\(&chat_id,\s*&gid\)\?/);
  assert.match(host,/stage_completed_model_plan/);
  assert.match(admission,/serde_json::from_str\(output\)/);
  assert.match(admission,/deny_unknown_fields/);
  assert.match(admission,/live|current\.revision_event_id/);
});
test('planning cannot become implicit native tool or application authority',()=>{
  assert.match(host,/if origin==GenerationOrigin::Human&&!plan_only/);
  assert.match(host,/if !plan_only/);
  assert.match(admission,/NOT_AUTHORIZED_FOR_EXECUTION/);
  assert.doesNotMatch(admission,/Command::new|\.spawn\(|run_journaled_observed\(/);
  assert.match(web,/no tools will run/);
});
test('model planner always uses host verified current ANSF goal and fails closed on contradictions',()=>{
  assert.match(admission,/project_goal_events\(&chats\.memory_events\(chat\)\?\)/);
  assert.match(admission,/latest_authenticated_user_turn_id/);
  assert.match(admission,/unrecognized|Unknown model plan step kind|Forward\/self-dependent/);
  assert.match(admission,/changed during inference/);
});

test('desktop owns complete empty receipt and scheduler schemas while portable draft parser stays independent',()=>{
  assert.match(host,/AgentHostScheduler::open\(&plan_db\)/);
  assert.match(host,/AgentDeliverableReceipts::open\(&plan_db\)/);
  assert.doesNotMatch(admission,/use lu_core::agent_host_scheduler::AgentHostScheduler/);
  assert.doesNotMatch(admission,/AgentDeliverableReceipts::open\(/);
});
