// Source contracts only. These MUST NOT be accepted as genuine GGUF/OS proof.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const native=readFileSync(new URL('../../src-tauri/lu-core/tests/e1_reviewed_model_tools.rs',import.meta.url),'utf8');
const workflow=readFileSync(new URL('../../.github/workflows/lu-e1-p07-genuine-gguf.yml',import.meta.url),'utf8');
const receipts=readFileSync(new URL('../../src-tauri/lu-core/src/agent_deliverable_receipts.rs',import.meta.url),'utf8');
const host=readFileSync(new URL('../src-tauri/src/reviewed_app_desktop.rs',import.meta.url),'utf8');

test('the pinned actual GGUF model is required, never scripted into success',()=>{
  assert.match(native,/async fn agent01g_genuine_gguf_requires_fresh_ansf_review_native_git_oracle_and_restart/);
  assert.match(native,/let mut model=GenuineGgufStep\{raw_replies:vec!\[\]\}/);
  assert.match(native,/session\.run_journaled_observed\(/);
  assert.match(native,/report\.host_routed_calls,0/);
  assert.match(native,/report\.tool_calls,1/);
  assert.match(workflow,/Download checksum-pinned genuine Qwen3-0\.6B GGUF/);
  assert.match(workflow,/agent01g_genuine_gguf_requires_fresh_ansf_review_native_git_oracle_and_restart/);
  assert.match(workflow,/-- --ignored --nocapture/);
});
test('real persisted human source and goal are distinct from approval and next step',()=>{
  assert.match(native,/AnsfStore::open/);
  assert.match(native,/book\.mirror_goal_event/);
  assert.match(native,/book\.goal_events/);
  assert.match(native,/project_goal_events/);
  assert.match(native,/book\.append_entry\(&chat,&original,EntryRole::User/);
  assert.match(native,/book\.append_entry\(&chat,&review_turn,EntryRole::User/);
  assert.match(native,/book\.append_entry\(&chat,&fresh_turn,EntryRole::User/);
  assert.match(native,/revisions\.record_review\(/);
  assert.match(native,/Err\(ScheduleError::NoFreshConsent\)/);
});
test('a live host claim and real per-turn authority precede the native worker',()=>{
  assert.match(native,/scheduler\.claim_host\(/);
  assert.match(native,/scheduler\.propose_step\(/);
  assert.match(native,/auth\.open_user_task\(TaskAuthority/);
  assert.match(native,/bind_reviewed_human_turn\(&chat,&fresh_turn\)/);
  assert.match(native,/ReviewedApplicationTurnSession::from_verified_host_task/);
  assert.match(native,/session\.preflight_scheduled_step\(/);
  assert.match(native,/ReviewedBrokerOperation::GitHead/);
  assert.ok(native.lastIndexOf('session.preflight_scheduled_step(')<
            native.lastIndexOf('session.run_journaled_observed('));
  assert.match(host,/Authorize one reviewed agent step/);
});
test('the completed effect requires independent native witness and restart',()=>{
  assert.match(native,/link_verified_scheduled_worker_outcome\(/);
  assert.match(native,/VERIFIED_AT_OBSERVATION/);
  assert.match(native,/scheduler\.release_without_dispatch\(/);
  assert.match(native,/AgentDeliverableReceipts::open\(&db\)/);
  assert.match(native,/AgentHostScheduler::open\(&db\)/);
  assert.match(native,/assert_eq!\(view\.state,"RELEASED"\)/);
  assert.match(receipts,/SELECT conversation_id,user_turn_id FROM lu_reviewed_turn_origin/);
  assert.match(receipts,/lu_independent_outcomes/);
});
test('approval never extends to a subsequent turn or a changed goal',()=>{
  assert.match(native,/permitted_tool_ids\(&fresh_origin\)\.is_err\(\)/);
  assert.match(native,/a restarted host cannot reuse an approved plan|restarted host cannot reuse an approved plan/);
  assert.match(native,/A different task supersedes the original local Git inspection/);
  assert.match(native,/NEEDS_RECONCILIATION/);
  assert.match(workflow,/Installed UI native picker remains separate/);
});
