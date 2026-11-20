import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {isModelFreeSendQuestion,isHostReminderRequest} from '../web/native_intent.mjs';
const read=p=>readFileSync(new URL(p,import.meta.url),'utf8');
const ui=read('../web/scheduled_reminders.jsx');
const app=read('../web/main.jsx');
const host=read('../src-tauri/src/lib.rs');
const core=read('../../src-tauri/lu-core/src/native_background_scheduler/reminders.rs');
test('explicit plain-language reminder requests enable model-free Send only for bounded complete forms',()=>{
  for(const prompt of ['Remind me in 1 minute to read','Remind me every day to review notes','Remind me every 2 hours to drink water','Remind me in 1 week to call home','REMIND ME IN 10 MINUTES TO 阅读'])assert.equal(isModelFreeSendQuestion(prompt),true,prompt);
  for(const prompt of ['How do I set reminders?','Remind me tomorrow to read','Remind me in 0 minutes to read','Remind me in 2 weeks to read','Remind me every 10081 minutes to read','Remind me in 1 minute to read without any tools','Remind me in 1 minute to read\n[Attached text file: x]','Remind me in 1 minute to a\u202e','Remind me in 1 minute to '+ '漢'.repeat(172)])assert.equal(isModelFreeSendQuestion(prompt),false,prompt);
  assert.equal(isHostReminderRequest(null),false);
});
test('task management reads the durable native journal; notification events are invalidation only',()=>{
  assert.match(ui,/invoke\('lu_scheduled_reminders'\)/);
  assert.match(ui,/listen\('lu-reminders-changed',\(\)=>void refresh\(\)\)/);
  assert.match(ui,/setInterval\(\(\)=>void refresh\(\),10000\)/);
  assert.match(app,/taskView==='reminders'\?<ScheduledReminders/);
  assert.match(app,/Reminders due \(\{reminders.unread\}\)/);
  assert.doesNotMatch(ui,/invoke\('lu_generate'|localStorage|eval\(/);
});
test('approval and dismissal preserve exact ownership and the original seen occurrence',()=>{
  assert.match(host,/fn review_and_schedule_reminder\(/);
  assert.match(host,/if !app\.dialog\(\)\.message\(message\)\.title\("Lu — Approve reminder"\)[\s\S]*?blocking_show\(\)\{return Ok\(None\)\}/);
  assert.match(host,/owner\.chats\.trusted_user_turn_text\(chat_id,turn\)\.as_deref\(\)!=Some\(text\)/);
  assert.match(host,/owner\.background_scheduler\.create_reminder\(&id,chat_id,/);
  assert.match(ui,/chatId:row\.workspaceId,reminderId:row\.id,action,seenCount:action==='acknowledge'\?row\.firedCount:null/);
  assert.match(core,/WHERE id=\?1 AND workspace_id=\?2 AND fired_count=\?3 AND unread_count>0/);
});
test('recurrence is inert and local; pause, resume, cancel and acknowledged history are visible',()=>{
  assert.match(host,/background_scheduler\.tick_reminders_at\(/);
  assert.match(core,/transaction_with_behavior\(rusqlite::TransactionBehavior::Immediate\)/);
  assert.match(core,/let count = \(now-due\)\/interval\+1/);
  assert.doesNotMatch(core,/std::process::Command|generate_internal|enqueue_host_plan/);
  for(const phrase of ['Pause reminder','Resume reminder','Confirm cancellation','Mark read','All reminders','Missed reminders appear'])assert.ok(ui.includes(phrase),phrase);
});
