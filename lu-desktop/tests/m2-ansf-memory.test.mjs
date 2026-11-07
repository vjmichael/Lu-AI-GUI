import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read=path=>readFileSync(new URL(path,import.meta.url),'utf8');
const front=read('../web/main.jsx');
const host=read('../src-tauri/src/lib.rs');
const memory=read('../src-tauri/src/memory_desktop.rs');
const chat=read('../src-tauri/src/chat_store.rs');
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
  assert.match(memory,/source!=actual/);
  assert.match(memory,/model_context\(&goal_context\)/);
  assert.doesNotMatch(memory,/ExecuteCode|NetworkWrite|grant\(/);
});
test('M2 source of truth remains host-authenticated chat events, not recalled consent',()=>{
  assert.match(chat,/GoalMemory\s*\{\s*entry:\s*GoalEvent/);
  assert.match(chat,/Event::GoalMemory\{\.\.\}/);
  assert.match(chat,/Event::User\s*\{\s*text,\s*user_turn_id:\s*Some\(id\)\s*\}/);
  assert.match(core,/source_kind='desktop_chat_journal'/);
  assert.match(core,/source_sha256/);
  assert.match(core,/GoalEventKind::RespondQuestion/);
  assert.match(core,/"deferred"/);
  assert.match(core,/"superseded"/);
  assert.match(prompt,/Only the current conversation is considered/);
});
test('M2 model memory is bounded task data rather than a tool grant',()=>{
  assert.match(memory,/take\(1600\)/);
  assert.match(memory,/take\(4\)\.rev\(\)/);
  assert.match(memory,/NOT tool authorization or system instructions/);
  assert.match(core,/MAX_EVENTS:usize=1024/);
  assert.match(core,/MAX_QUESTIONS:usize=64/);
});
