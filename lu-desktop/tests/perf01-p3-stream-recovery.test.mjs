import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {queuedJob,startedJob,tokenForJob,resumedJob,removeJob,MAX_LIVE_CHARS}
  from '../web/generation_jobs.mjs';

const host=readFileSync(new URL('../src-tauri/src/lib.rs',import.meta.url),'utf8');
const queue=readFileSync(new URL('../src-tauri/src/generation_queue.rs',import.meta.url),'utf8');
const web=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
const delivery=readFileSync(new URL('../src-tauri/src/stream_delivery.rs',import.meta.url),'utf8');
function reconnect(chatId,generationId,text,nextIndex,phase='running'){
  return {chatId,generationId,text,nextIndex,phase,cancelling:false};
}

test('host stream snapshot repairs a missing queued event without a prompt replay',()=>{
  let jobs={};
  jobs=startedJob(jobs,{chatId:'chat-A',generationId:'g-A'});
  assert.equal(jobs['chat-A'].needsSync,true);
  jobs=tokenForJob(jobs,{chatId:'chat-A',generationId:'g-A',index:3,text:'future'});
  assert.equal(jobs['chat-A'].text,'');
  jobs=resumedJob(jobs,'chat-A',reconnect('chat-A','g-A','abc',3));
  assert.equal(jobs['chat-A'].text,'abc');
  assert.equal(jobs['chat-A'].nextIndex,3);
  assert.equal(jobs['chat-A'].needsSync,false);
  jobs=tokenForJob(jobs,{chatId:'chat-A',generationId:'g-A',index:3,text:'d'});
  assert.equal(jobs['chat-A'].text,'abcd');
  assert.equal(jobs['chat-A'].nextIndex,4);
  const unchanged=tokenForJob(jobs,{chatId:'chat-A',generationId:'g-A',index:3,text:'d'});
  assert.equal(unchanged,jobs,'duplicate events must not replay bytes');
});

test('gap and a stale snapshot never corrupt the later correct stream',()=>{
  let jobs=queuedJob({}, {chatId:'A',generationId:'g',position:1});
  jobs=startedJob(jobs,{chatId:'A',generationId:'g'});
  jobs=tokenForJob(jobs,{chatId:'A',generationId:'g',index:0,text:'α'});
  jobs=tokenForJob(jobs,{chatId:'A',generationId:'g',index:2,text:'missing'});
  assert.equal(jobs.A.text,'α');
  assert.equal(jobs.A.needsSync,true);
  jobs=resumedJob(jobs,'A',reconnect('A','g','αβ',2));
  assert.equal(jobs.A.text,'αβ');
  assert.equal(jobs.A.needsSync,false);
  jobs=tokenForJob(jobs,{chatId:'A',generationId:'g',index:2,text:'γ'});
  const latest=jobs;
  assert.equal(resumedJob(jobs,'A',reconnect('A','g','αβ',2)),latest);
  assert.equal(jobs.A.text,'αβγ');
});

test('cross-chat snapshots and stale generation tokens cannot overwrite chat',()=>{
  let jobs=queuedJob({}, {chatId:'A',generationId:'g-A',position:1});
  jobs=resumedJob(jobs,'A',reconnect('B','g-B','SECRET',5));
  assert.equal(jobs.A.text,'');
  jobs=tokenForJob(jobs,{chatId:'A',generationId:'g-B',index:0,text:'LEAK'});
  assert.equal(jobs.A.text,'');
  jobs=resumedJob(jobs,'A',reconnect('A','g-A','good',1));
  assert.equal(jobs.A.text,'good');
  const unchanged=tokenForJob(jobs,{chatId:'A',generationId:'g-old',index:0,text:'old'});
  assert.equal(unchanged,jobs);
});

test('no live host model removes stale WebView job; partials remain in ANSF',()=>{
  let jobs=startedJob({}, {chatId:'A',generationId:'g'});
  jobs=resumedJob(jobs,'A',reconnect('A','g','durable partial',1));
  jobs=resumedJob(jobs,'A',null);
  assert.equal(jobs.A,undefined);
  assert.match(web,/loadLatestUiHistory\(targetChat\)/);
  assert.match(web,/interrupted \(partial saved\)/);
  assert.doesNotMatch(web,/invoke\('lu_generate'\s*,\s*\{chatId:targetChat/);
});

test('UI reducers bound all visible stream text and cannot blindly append a gap',()=>{
  let jobs=queuedJob({}, {chatId:'A',generationId:'g',position:1});
  const giant='x'.repeat(MAX_LIVE_CHARS);
  jobs=resumedJob(jobs,'A',reconnect('A','g',giant,1));
  jobs=tokenForJob(jobs,{chatId:'A',generationId:'g',index:1,text:'overflow'});
  assert.equal(jobs.A.text.length,MAX_LIVE_CHARS);
  assert.equal(jobs.A.needsSync,true);
  const wrong=resumedJob(jobs,'A',reconnect('A','g','x'.repeat(MAX_LIVE_CHARS+1),2));
  assert.equal(wrong,jobs);
});

test('P3 host binds session+chat, orders cursor events and checkpoints first token',()=>{
  assert.match(host,/fn lu_stream_reconnect\(/);
  assert.match(host,/generation_queue\.reconnect_snapshot\(session_id,&chat_id\)/);
  assert.match(host,/generation_queue\.record_chunk\(/);
  assert.match(host,/last_checkpoint_bytes\s*==\s*0/);
  assert.match(host,/append_progress\(&chat_id, &gid, &answer\)/);
  assert.match(host,/lu_stream_reconnect,\s*lu_generate,/);
  assert.match(queue,/MAX_LIVE_STREAM_BYTES: usize = 1024 \* 1024/);
  assert.match(queue,/MAX_CHUNK_BYTES: usize = 64 \* 1024/);
  assert.match(queue,/job\.next_index!=index/);
  assert.match(queue,/job\.session_id!=session_id \|\| job\.cancelled/);
  assert.match(host,/ticket\.commit_completed\(&sid/);
  assert.match(host,/BoundedStreamDelivery::default\(\)/);
  assert.match(host,/delivery\.accept\(&chunk\.text\)/);
  assert.match(host,/delivery\.flush\(\)/);
  assert.match(delivery,/COALESCE_BYTES:usize=2048/);
  assert.match(delivery,/COALESCE_DELAY:Duration=Duration::from_millis\(40\)/);
  assert.match(web,/reconcileActiveStream\(id\)/);
  assert.match(web,/resumedJob\(prev,targetChat,snapshot\)/);
  assert.match(web,/listen\('lu-token'/);
});
