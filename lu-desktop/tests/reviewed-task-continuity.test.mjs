import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {explainReviewedContinuation} from '../web/reviewed_tool_feedback.mjs';

const tasks=readFileSync(new URL('../../src-tauri/lu-core/src/tasks.rs',import.meta.url),'utf8');
const model=readFileSync(new URL('../../src-tauri/lu-core/src/native_reviewed_app_tool.rs',import.meta.url),'utf8');
const desktop=readFileSync(new URL('../src-tauri/src/reviewed_app_desktop.rs',import.meta.url),'utf8');
const host=readFileSync(new URL('../src-tauri/src/lib.rs',import.meta.url),'utf8');
const projection=readFileSync(new URL('../src-tauri/src/reviewed_task_continuity.rs',import.meta.url),'utf8');
const ui=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');

test('P15-A: original human turn has a durable, immutable, host-only task binding',()=>{
  assert.match(tasks,/CREATE TABLE IF NOT EXISTS lu_reviewed_turn_origin/);
  assert.match(tasks,/pub fn bind_reviewed_human_turn/);
  assert.match(tasks,/INSERT OR IGNORE INTO lu_reviewed_turn_origin/);
  assert.match(tasks,/existing\.0!=conversation_id\|\|existing\.1!=user_turn_id/);
  assert.match(desktop,/PersistedReviewedHumanTurn/);
  assert.match(desktop,/bind_reviewed_human_turn\(&chat_id,&gid_run\)/);
  assert.match(desktop,/trusted_user_turn_text\(chat,turn\)/);
});

test('P15-B: recovery joins task/checkpoint, actual reviewed worker and second independent outcome',()=>{
  assert.match(model,/adapter_id\(&invocation\.call_id,operation\)/);
  assert.match(projection,/lu-reviewed-task-\{turn\}\.sqlite/);
  assert.match(projection,/lu-reviewed-effects-\{turn\}\.sqlite/);
  assert.match(projection,/lu_reviewed_turn_origin/);
  assert.match(projection,/lu_task_events/);
  assert.match(projection,/lu_independent_outcomes/);
  assert.match(projection,/Sha256::digest\(receipt\)/);
  assert.match(projection,/outer_ok&&task_complete&&receipt_ok/);
  assert.match(projection,/VERIFIED_AT_OBSERVATION/);
  assert.doesNotMatch(projection,/pub.*fn.*(?:launch|execute|replay|resume)/);
  assert.doesNotMatch(projection,/Command::new|\.spawn\(|TaskStore::resume/);
});

test('P15-C: continuation text never grants replay or confuses historical proof with current custody',()=>{
  const safe=explainReviewedContinuation('FRESH_REQUEST_ONLY');
  assert.match(safe.explanation,/not proof/i);
  assert.match(safe.action,/fresh authorization/i);
  const blocked=explainReviewedContinuation('RECONCILIATION_REQUIRED');
  assert.match(blocked.explanation,/Do not replay/);
  for(const status of ['__proto__','constructor','FRESH_REQUEST_ONLYX',null,undefined,'']) {
    assert.equal(explainReviewedContinuation(status).title,'Recovery decision unavailable');
  }
  assert.match(projection,/RECONCILIATION_REQUIRED/);
  assert.match(projection,/FRESH_REQUEST_ONLY/);
  assert.match(projection,/origin\.as_ref\(\)\.is_none_or/);
});

test('P15-D: original indexed conversation owns a read-only recovery timeline',()=>{
  assert.match(host,/mod reviewed_task_continuity;/);
  assert.match(host,/reviewed_continuity:Option<reviewed_task_continuity::ReviewedTurnContinuity>/);
  assert.match(host,/reviewed_task_continuity::reviewed_turn_continuity\(/);
  assert.match(host,/trusted_user_turn_text\(/);
  assert.match(ui,/Reviewed task recovery timeline/);
  assert.match(ui,/item\.reviewedContinuity\.operations\.map/);
  assert.match(ui,/last recorded progress:/);
  assert.match(ui,/This view never resumes a task or grants execution/);
  assert.doesNotMatch(ui,/invoke\(['"]lu_(?:reviewed_)?(?:task_resume|effect_replay)/);
});

test('P15-E: native acceptance must be tested before claiming installed recovery',()=>{
  assert.match(projection,/fn origin_bound_verified_past_effect_never_authorizes_resumption/);
  assert.match(projection,/fn missing_and_uncertain_journals_never_offer_resume/);
  assert.match(projection,/fn unsupported_or_ambiguous_os_effect_requires_reconciliation/);
  assert.match(tasks,/reviewed_origin_is_durable_idempotent_and_cannot_be_reassigned/);
  assert.match(projection,/SQLITE_OPEN_READ_ONLY/);
  assert.match(projection,/file_type\(\)\.is_symlink\(\)/);
});
