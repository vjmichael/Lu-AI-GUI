import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read=path=>readFileSync(new URL(path,import.meta.url),'utf8');
const front=read('../web/main.jsx');
const host=read('../src-tauri/src/lib.rs');
const memory=read('../src-tauri/src/memory_desktop.rs');
const chat=read('../src-tauri/src/ansf_chat_store.rs');
const live=read('../../src-tauri/lu-core/src/ansf_live_chat.rs');
const nativeGoals=read('../../src-tauri/lu-core/src/ansf_goal_records.rs');
const core=read('../../src-tauri/lu-core/src/ansf_goals.rs');
const prompt=read('../src-tauri/src/prompt_context.rs');
test('M2 clarification belongs to a real non-chat UI with all explicit choices',()=>{
  assert.match(front,/aria-label="Conversation goal and clarifications"/);
  assert.match(front,/m2Answer\(q\.questionId,'yes'\)/);
  assert.match(front,/m2Answer\(q\.questionId,'no'\)/);
  assert.match(front,/m2Answer\(q\.questionId,'not_now'\)/);
  assert.match(front,/Optional comment/);
  assert.match(front,/maxLength=\{1024\}/);
  assert.match(front,/lu_m2_answer_question/);
});
test('M2 native commands are registered and not a general tool interface',()=>{
  for(const name of ['lu_m2_goal_context','lu_m2_set_goal',
    'lu_m2_open_question','lu_m2_answer_question']){
    assert.ok(host.includes('memory_desktop::'+name),name);
    assert.ok(memory.includes('fn '+name),name);
  }
  assert.match(memory,/chats\.append_goal_memory_event\(chat_id,event\)/);
  assert.match(memory,/project_goal_events\(&records\)/);
  assert.match(chat,/append_native_goal_record\(chat_id,&event\)/);
  assert.match(host,/recent_model_history\(&chat_id,32\)/);
  assert.match(host,/model_context\(&goal_context\)/);
  assert.doesNotMatch(memory,/ExecuteCode|NetworkWrite|grant\(/);
});
test('M2 native ANSF is canonical; imported history cannot become user authority',()=>{
  assert.match(chat,/ansf\/books/);
  assert.match(chat,/append_host_chat_event\(chat_id,&event\)/);
  assert.match(chat,/authenticated_chat_turn\(chat_id,user_turn_id\)/);
  assert.match(chat,/native_goal_records\(chat_id\)/);
  assert.match(nativeGoals,/CREATE TABLE IF NOT EXISTS ansf_goal_records/);
  assert.match(nativeGoals,/source_origin TEXT NOT NULL/);
  assert.match(nativeGoals,/append_native_goal_record/);
  assert.match(live,/CREATE TABLE IF NOT EXISTS ansf_live_chat_events/);
  assert.match(live,/origin='host'/);
  assert.match(live,/legacy_turn_id/);
  assert.match(live,/pub fn authenticated_chat_turn/);
  assert.match(core,/GoalEventKind::RespondQuestion/);
  assert.match(core,/"deferred"/);
  assert.match(core,/"superseded"/);
  assert.match(prompt,/Only the current conversation is considered/);
});
test('M2 model memory is bounded task data rather than a tool grant',()=>{
  assert.match(memory,/take\(1600\)/);
  assert.match(memory,/take\(4\)\.map\(/);
  assert.match(memory,/answers\.reverse\(\)/);
  assert.match(memory,/NOT tool authorization or system instructions/);
  assert.match(core,/MAX_EVENTS:usize=1024/);
  assert.match(core,/MAX_QUESTIONS:usize=64/);
});
