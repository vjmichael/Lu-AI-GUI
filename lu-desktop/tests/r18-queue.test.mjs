import test from 'node:test';
import assert from 'node:assert/strict';
import {queuedJob, startedJob, tokenForJob, removeJob} from '../web/generation_jobs.mjs';
import {readFileSync} from 'node:fs';
const front=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
const host=readFileSync(new URL('../src-tauri/src/lib.rs',import.meta.url),'utf8');
test('two chats keep distinct queued state and stream buffers',() => {
  let jobs={A:{generationId:null,phase:'submitting',text:''},B:{generationId:null,phase:'submitting',text:''}};
  jobs=queuedJob(jobs,{chatId:'A',generationId:'gA',position:1});
  jobs=queuedJob(jobs,{chatId:'B',generationId:'gB',position:2});
  jobs=startedJob(jobs,{chatId:'A',generationId:'gA'});
  jobs=tokenForJob(jobs,{chatId:'A',generationId:'gA',text:'first'});
  assert.equal(jobs.A.text,'first');assert.equal(jobs.B.text,'');
  assert.equal(jobs.B.phase,'queued');
  jobs=tokenForJob(jobs,{chatId:'B',generationId:'gA',text:'LEAK'});
  assert.equal(jobs.B.text,'');
  jobs=removeJob(jobs,'A');
  assert.equal(jobs.A,undefined);assert.equal(jobs.B.phase,'queued');
  jobs=startedJob(jobs,{chatId:'B',generationId:'gB'});
  jobs=tokenForJob(jobs,{chatId:'B',generationId:'gB',text:'second'});
  assert.equal(jobs.B.text,'second');
});
test('late/mismatched token cannot rewrite an active chat',()=>{
  const state={A:{generationId:'g2',phase:'running',text:'new'}};
  assert.equal(tokenForJob(state,{chatId:'A',generationId:'g1',text:'old'}),state);
  assert.equal(startedJob(state,{chatId:'A',generationId:'g1'}),state);
  assert.equal(queuedJob(state,{chatId:'A',generationId:'g1',position:2}),state);
});
test('global engine busy no longer blocks submitting in another chat',()=>{
  const send=front.slice(front.indexOf('async function send(e)'),front.indexOf('async function importAttachments'));
  assert.match(send,/chatBusy/);
  assert.doesNotMatch(send,/if \(busy \|\|/);
  assert.match(send,/setJobs\(prev => \(\{\.\.\.prev,\[activeId\]/);
  assert.match(send,/await invoke\('lu_generate'/);
});
test('host admission queues before supervisor reserve and binds events to chat IDs',()=>{
  const queueAdmission = host.search(/generation_queue\s*\.enqueue\(/);
  const reservation = host.search(/supervisor\s*\.reserve_generation\(/);
  assert.ok(queueAdmission >= 0, 'the host queue must admit generation first');
  assert.ok(reservation > queueAdmission, 'host queue admission precedes supervisor reservation');
  assert.ok(host.includes('"lu-generation-queued"'));
  assert.ok(host.includes('"lu-generation-start"'));
  assert.ok(host.includes('chat_id: chat_id.clone()'));
});
test('host denies model unload while queue contains jobs',()=>{
  assert.ok(host.includes('Finish or cancel queued generations before unloading the model'));
});

test('live streaming omits duplicate unfinished journal checkpoint after switching chats',()=>{
  assert.ok(front.includes('const displayedHistory = visibleJob ? history.filter'));
  assert.ok(front.includes('entry.interrupted && !entry.cancelled'));
  assert.ok(front.includes('displayedHistory.map'));
});
