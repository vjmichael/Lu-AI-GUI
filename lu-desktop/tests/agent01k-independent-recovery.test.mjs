// AGENT-01K source contracts only; never real OS custody or loaded-model proof.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const core=readFileSync(new URL('../../src-tauri/lu-core/src/agent_recovery_decision.rs',import.meta.url),'utf8');
const desktop=readFileSync(new URL('../src-tauri/src/agent_recovery_desktop.rs',import.meta.url),'utf8');
const lib=readFileSync(new URL('../src-tauri/src/lib.rs',import.meta.url),'utf8');
const ui=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
const receipts=readFileSync(new URL('../../src-tauri/lu-core/src/agent_deliverable_receipts.rs',import.meta.url),'utf8');
const scheduler=readFileSync(new URL('../../src-tauri/lu-core/src/agent_host_scheduler.rs',import.meta.url),'utf8');
const ledger=readFileSync(new URL('../../src-tauri/lu-core/src/agent_plan_ledger.rs',import.meta.url),'utf8');
const revisions=readFileSync(new URL('../../src-tauri/lu-core/src/agent_plan_revisions.rs',import.meta.url),'utf8');

test('recovery uses real task/effect oracle and bounded chain, no model permission',()=>{
  assert.match(core,/receipts\.history\(/);
  assert.match(core,/reverify_child_before_execution\(/);
  assert.match(core,/inspect_continuation_lineage\(/);
  assert.match(core,/MAX_GENERATIONS:usize=32/);
  assert.match(core,/NOT_AUTHORIZED_FOR_EXECUTION/);
  assert.doesNotMatch(core,/ConversationAuthority::new|TaskAuthority\{|run_journaled_observed\(|Command::new/);
});
test('native host checks saved original human chat and obtains fixed host profile',()=>{
  assert.match(desktop,/history_window\(&chat_id,None,"older",1\)/);
  assert.match(desktop,/host\.catalog_path\.parent\(\)/);
  assert.match(desktop,/existing_plan_file\(profile\)/);
  assert.match(desktop,/original_human_turn_id/);
  assert.match(desktop,/NoIndependentCustodyProof/);
  assert.match(lib,/agent_recovery_desktop::lu_agent_reconcile_recovery/);
});
test('all original SQLite stores are opened existing READ ONLY for recovery',()=>{
  for(const s of [receipts,scheduler,ledger,revisions]){
    assert.match(s,/pub fn open_existing_readonly\(/);
    assert.match(s,/OpenFlags::SQLITE_OPEN_READ_ONLY/);
  }
  assert.match(desktop,/AgentPlanLedger::open_existing_readonly/);
  assert.match(desktop,/AgentDeliverableReceipts::open_existing_readonly/);
  assert.match(desktop,/AgentHostScheduler::open_existing_readonly/);
});
test('historical receipt includes exact execution turn, never inferred from origin',()=>{
  assert.match(receipts,/pub execution_turn_id:String/);
  assert.match(receipts,/execution_turn_id:actual_turn\.to_owned\(\)/);
  assert.match(core,/row\.execution_turn_id!=claim\.original_request_turn/);
  assert.match(core,/seen_calls\.insert\(row\.effect_call_id\.clone\(\)\)/);
});
test('stale receipts, active owner and cancelled jobs block continuation',()=>{
  for(const name of [
    'STOP_UNVERIFIED_WORKER_ORACLE','STOP_DESCENDANT_CUSTODY_UNPROVEN',
    'STOP_CANCEL_UNCERTAIN','STOP_ABANDONED_HOST_CUSTODY',
    'STOP_GOAL_OR_SOURCE_CHANGED','STOP_ANCESTOR_ORACLE_UNVERIFIED',
    'STOP_LINEAGE_RECONCILIATION'
  ])assert.ok(core.includes(name),name);
});
test('no provenance of descendant termination means deny, even with verified Git effect',()=>{
  assert.match(core,/pub struct NoIndependentCustodyProof/);
  assert.match(core,/bool\{false\}/);
  assert.match(desktop,/&NoIndependentCustodyProof/);
  assert.match(core,/REVIEW_NEW_STEP_WITH_FRESH_AUTHORIZATION_ONLY/);
  assert.match(core,/execution_authority:"NOT_AUTHORIZED_FOR_EXECUTION"/);
});
test('recovery inspection is explicit, user-visible, and never a resume button',()=>{
  assert.match(ui,/Review independent recovery evidence/);
  assert.match(ui,/invoke\('lu_agent_reconcile_recovery'/);
  assert.match(ui,/Historical, read-only|historical, read-only/);
  assert.match(ui,/descendantCustody/);
  assert.match(ui,/setAgentRecoveryByPlan\(old=>\(\{\.\.\.old,\[key\]:null\}\)\)/);
  assert.match(ui,/No execution authority was granted/);
});
test('actual SQLite integration test withholds oracle after original completion',()=>{
  assert.match(receipts,/fn agent01k_real_sqlite_reopen_reconciles_worker_oracle_without_claiming_os_custody\(/);
  assert.match(receipts,/lu-reviewed-effects-/);
  assert.match(receipts,/agent01k-withheld-independent-effect\.sqlite/);
  assert.match(receipts,/open_existing_readonly/);
  assert.match(receipts,/STOP_GOAL_OR_SOURCE_CHANGED/);
});
