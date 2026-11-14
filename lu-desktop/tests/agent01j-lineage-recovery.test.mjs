// AGENT-01J source contracts: do not claim model, native worker or installed OS proof.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const lineage=readFileSync(new URL('../../src-tauri/lu-core/src/agent_lineage_recovery.rs',import.meta.url),'utf8');
const core=readFileSync(new URL('../../src-tauri/lu-core/src/lib.rs',import.meta.url),'utf8');
const progress=readFileSync(new URL('../src-tauri/src/agent_plan_progress.rs',import.meta.url),'utf8');
const ui=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');

test('read-only ancestry reader is compiled into production Lu Core',()=>{
  assert.match(core,/pub mod agent_lineage_recovery;/);
  assert.match(lineage,/pub fn inspect_continuation_lineage\(/);
  assert.match(lineage,/SQLITE_OPEN_READ_ONLY/);
  assert.match(lineage,/MAX_DEPTH:usize=32/);
});
test('integrity report does not manufacture execution authority',()=>{
  assert.match(lineage,/execution_authority:"NOT_AUTHORIZED_FOR_EXECUTION"/);
  assert.match(lineage,/LINEAGE_RECORDED_REVIEW_ONLY/);
  assert.doesNotMatch(lineage,/ConversationAuthority::new|\.spawn\(|run_journaled_observed\(/);
});
test('crashed unlinked child and corrupt origin are visibly blocked',()=>{
  for(const id of ['STOP_ORPHAN_CONTINUATION','STOP_CROSS_CHAT_ORIGIN',
    'STOP_MISSING_PLAN_ORIGIN','STOP_DAMAGED_LINEAGE','STOP_MISSING_CONTINUATION_INTENT']){
    assert.ok(lineage.includes(id),id);
  }
  assert.match(lineage,/fresh_human_turn_id=\?4/);
  assert.match(lineage,/seen\.insert\(current\.clone\(\)\)/);
});
test('conversation progress uses actual ancestry result, not a model assertion',()=>{
  assert.match(progress,/inspect_continuation_lineage\(/);
  assert.match(progress,/pub lineage_status:String/);
  assert.match(progress,/pub lineage_generations:u8/);
  assert.match(progress,/STOP_LINEAGE_RECONCILIATION/);
  assert.match(progress,/candidate_step_index:None/);
});
test('UI shows ancestry but not an automatic run control',()=>{
  assert.match(ui,/Continuation ancestry:/);
  assert.match(ui,/Lineage requires manual reconciliation/);
  assert.match(ui,/Prepare separate next-step plan/);
  assert.doesNotMatch(lineage,/HostFreshStepConsent|TaskAuthority\{/);
});
test('native sqlite regressions include crash/reopen, cycle and cross-chat',()=>{
  for(const n of [
    'root_and_two_generation_lineage_remain_review_only',
    'orphan_and_cross_chat_child_are_blocked',
    'cycle_and_corrupt_revision_do_not_become_recovery',
    'real_sqlite_reopen_keeps_orphan_blocked_and_never_mints_authority'
  ]) assert.ok(lineage.includes('fn '+n+'()'),n);
});
