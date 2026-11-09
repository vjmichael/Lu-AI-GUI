import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {explainMemoryProjectionRecovery} from '../web/reviewed_memory_recovery.mjs';
const root=new URL('../../',import.meta.url);
function source(name){return readFileSync(new URL(name,root),'utf8')}
const memory=source('lu-desktop/src-tauri/src/reviewed_memory_evidence.rs');
const rust=source('lu-desktop/src-tauri/src/lib.rs');
const ui=source('lu-desktop/web/main.jsx');
const core=source('src-tauri/lu-core/src/ansf_verified_procedures.rs');
test('evidence row and complete projection intent commit in same transaction',()=>{
  const staged=memory.indexOf('let plan=make_plan');
  const index=memory.indexOf('INSERT OR IGNORE INTO lu_reviewed_memory_evidence',staged);
  const intent=memory.indexOf('INSERT OR IGNORE INTO lu_reviewed_memory_projection_intent',index);
  const committed=memory.indexOf('tx.commit().map_err(err)?;',intent);
  const derived=memory.indexOf('project_derived_only(chats,continuity,&root,&row,&plan)',committed);
  assert.ok(staged>0&&index>staged&&intent>index&&committed>intent&&derived>committed);
  assert.match(memory,/let _=read_plan\(&conn,&row\)/);
});
test('exclusive startup replays projections but never old OS worker',()=>{
  const owner=rust.indexOf('DesktopOwner::acquire(');
  const recovery=rust.indexOf('reviewed_memory_evidence::reconcile_incomplete_exclusive(');
  assert.ok(owner>0&&recovery>owner);
  const body=memory.slice(memory.indexOf('fn reconcile_one_projection('),
    memory.indexOf('/// Exact authenticated CURRENT-human selector'));
  assert.doesNotMatch(body,/\.resume\(|run_journaled|open_user_task\(|spawn\(/);
  assert.match(body,/reviewed_memory_projection_has_human_correction/);
  assert.match(body,/latest_authenticated_chat_turn/);
  assert.match(body,/current_message/);
  assert.match(memory,/different WAL SQLite databases/);
});
test('operator-only recovery status is chat-scoped and nonauthorizing',()=>{
  assert.match(rust,/fn lu_reviewed_memory_recovery_status/);
  assert.match(rust,/lu_reviewed_memory_recovery_status,/);
  assert.match(ui,/invoke\('lu_reviewed_memory_recovery_status'/);
  assert.match(ui,/Reviewed memory projection recovery/);
  assert.match(ui,/entry\.executionAuthority/);
  assert.match(ui,/entry\.retryAllowed/);
  assert.match(core,/pub fn reviewed_memory_projection_has_human_correction/);
  assert.match(explainMemoryProjectionRecovery({status:'LEGACY_REVIEW_REQUIRED'}).title,/human review/);
});

test('reviewed Windows restart fixture releases authority listener-owned journals before exclusive reopen',()=>{
  const fixture=source('lu-desktop/src-tauri/src/memory_loop03c_reviewed_worker_tests.rs');
  const session=fixture.indexOf('drop(fresh_session);');
  const authority=fixture.indexOf('drop(authority);');
  const reopen=fixture.indexOf('let reopened=EffectJournal::open(&new_effect_path)');
  const cleanup=fixture.lastIndexOf('fs::remove_dir_all(root).unwrap();');
  assert.ok(session>=0 && authority>session && reopen>authority && cleanup>reopen);
  const broker=source('src-tauri/lu-core/src/native_app_typed_git.rs');
  assert.match(broker,/authority\.on_task_revoked\(Arc::new\(move \|task_id\|/);
  assert.match(broker,/let journal=Arc::clone\(&effects\)/);
});
