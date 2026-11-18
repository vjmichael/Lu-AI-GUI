// AGENT-01H source-contract tests. These DO NOT prove OS custody or worker output.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';

const progress=readFileSync(new URL('../src-tauri/src/agent_plan_progress.rs',import.meta.url),'utf8');
const ui=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
const scheduler=readFileSync(new URL('../../src-tauri/lu-core/src/agent_host_scheduler.rs',import.meta.url),'utf8');

test('next-step preview is actually emitted by the production saved-chat projection',()=>{
  assert.match(progress,/pub struct AgentContinuationPreview/);
  assert.match(progress,/pub continuation_preview:AgentContinuationPreview/);
  assert.match(progress,/let preview=continuation_preview\(/);
  assert.match(progress,/continuation_preview:preview/);
  assert.match(ui,/plan\.continuationPreview\.requiredAction/);
});
test('next-step preview never creates an executable grant or marks old receipts current',()=>{
  assert.match(progress,/execution_authority:"NOT_AUTHORIZED_FOR_EXECUTION"/);
  assert.match(progress,/live_source_reverified:false/);
  const previewOnly=/fn continuation_preview[\s\S]*?fn valid\(/.exec(progress)?.[0]??'';
  assert.doesNotMatch(previewOnly,/Command::new|\.spawn\(|\.execute\(/);
  assert.match(ui,/A historic witness link is not current source correctness/);
  assert.match(ui,/No new host owner, worker, tool permission or automatic next turn/);
});
test('released or uncertain ownership is not reused for a later plan step',()=>{
  assert.match(progress,/owner_state=="RELEASED"/);
  assert.match(progress,/STOP_RELEASED_OWNER/);
  assert.match(progress,/STOP_UNCERTAIN_STEP/);
  assert.match(progress,/STOP_OWNER_CUSTODY_UNCERTAIN/);
  assert.match(progress,/STOP_OUTSTANDING_STEP/);
  assert.match(scheduler,/live_owners:Mutex<BTreeSet/);
});
test('dependencies require a fresh oracle and unsupported step types are refused',()=>{
  assert.match(progress,/PREVIEW_DEPENDENCY_RECHECK/);
  assert.match(progress,/STOP_DEPENDENCY_UNVERIFIED/);
  assert.match(progress,/STOP_MISSING_WITNESS/);
  assert.match(progress,/STOP_UNSUPPORTED_STEP_TYPE/);
  assert.match(progress,/previous\.schedule_state!="HISTORICAL_WITNESS_RECORDED"/);
});
test('manual reviewed action is only visible for exactly the one first candidate',()=>{
  assert.match(ui,/plan\.continuationPreview\?\.status==='MANUAL_ONE_STEP_REVIEW_ONLY'/);
  assert.match(ui,/plan\.continuationPreview\?\.candidateStepIndex===step\.stepIndex/);
  assert.match(ui,/Review and run one step/);
  assert.match(ui,/No automatic retry or/);
});
test('native SQLite tests cover a real original human chat and restart/blockers',()=>{
  for(const testCase of [
    'real_approved_first_step_is_advisory_not_an_execution_grant',
    'released_owner_never_becomes_a_reusable_step_grant_after_restart',
    'historical_witness_link_cannot_override_uncertain_scheduler_custody',
    'cached_dependency_or_unsupported_step_never_authorizes_next_worker'
  ])assert.ok(progress.includes('fn '+testCase+'()'));
  assert.match(progress,/ChatStore::open\(&root\)/);
  assert.match(progress,/read_chat_progress\(&root,&chat,&reopened\)/);
});
