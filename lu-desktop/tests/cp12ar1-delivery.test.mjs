import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read=p=>readFileSync(new URL(p,import.meta.url),'utf8');
const host=read('../src-tauri/src/lib.rs'),perf=read('../src-tauri/src/perf01_telemetry.rs'),ui=read('../web/main.jsx');
test('CP12AR1 text receipt is minted by host, scoped to Chat and returned from exact invocation',()=>{
 const entry=host.slice(host.indexOf('async fn lu_generate('),host.indexOf('async fn lu_generate_impl('));
 assert.match(entry,/observe_delivery==Some\(true\).*caller.label\(\)!="main"/);
 assert.match(entry,/observed.then\(\|\|\(next_id\(""\),std::time::Instant::now\(\)\)\)/);
 assert.match(entry,/owner.perf01.text_delivery\(&owner.chats,&chat_id,&r\)/);
 assert.match(entry,/w.generation_id!=generation_id\|\|w.assistant_seq!=assistant_seq/);
 assert.match(entry,/diagnostic_completed_assistant_seq\(&chat_id,&generation_id,None,w.reader\)/);
 assert.match(perf,/delivered&&!trace.reply_seen\(\)/);
});
test('CP12AR1 native tool routes share the exact host correlation and no extra model turn',()=>{
 for(const route of ['ask_git_for_chat','ask_reader_for_chat','ask_workspace_for_chat','ask_utility_for_chat']){
  assert.match(host,new RegExp(route+'\\([\\s\\S]*?cp01_trace[\\s\\S]*?\\)\\.await'));
 }
 assert.match(ui,/observeDelivery:true/);
 assert.match(ui,/invoke\('lu_text_reply_observed',\{\.\.\.w,delivered:false\}\)/);
 assert.match(ui,/ok===true\?invoke\('lu_text_reply_observed',\{\.\.\.delivery,delivered:true\}\)/);
 assert.match(ui,/displayRevisions\[witness.assistantSeq\]\?\.currentVersion>0/);
});
