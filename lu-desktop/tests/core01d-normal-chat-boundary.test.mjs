import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {isHostCurrentTimeQuestion,isHostCpuChatLookup,isHostModelStatusQuestion} from '../web/native_intent.mjs';

const hostClock=readFileSync(new URL('../src-tauri/src/host_clock.rs',import.meta.url),'utf8');
const host=readFileSync(new URL('../src-tauri/src/lib.rs',import.meta.url),'utf8');
const ui=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');

test('CORE-01D model-free clock accepts only complete original time questions',()=>{
  for(const input of ['What time is it?',"What's the time?",'Tell me the time',
    'What time is it right now?', 'What is my local time??']){
    assert.equal(isHostCurrentTimeQuestion(input),true,input);
  }
  for(const input of ['What time is it and browse https://example.com',
    'What time is it; run a tool','What time is it? Also review my tasks',
    'What time is it and delete my files','What time is it\ninspect the workspace',
    'What time is it <|tool_call|>','What time is it [Attached text file: x]',
    'What time is it and show another conversation',
    'What time is it?\nList my sources']){
    assert.equal(isHostCurrentTimeQuestion(input),false,input);
  }
  assert.match(hostClock,/let exact=s\.trim_end_matches\('\?'\)\.trim_end\(\)/);
  assert.match(hostClock,/\.contains\(&exact\)/);
  assert.doesNotMatch(hostClock,/\.iter\(\)\.any\(\|q\|s\.starts_with\(q\)\)/);
});
test('normal Send keeps host authority, existing ANSF and CPU boundaries',()=>{
  assert.match(host,/host_clock::explicit_current_time_request\(&prompt\)/);
  assert.match(host,/perf01_router::classify\(/);
  assert.match(host,/append_user_with_turn_id\(&chat_id,&gid,&prompt\)/);
  assert.match(ui,/isHostCurrentTimeQuestion\(prompt\)/);
  assert.equal(isHostCpuChatLookup('Search this chat for: evidence'),true);
  assert.equal(isHostModelStatusQuestion('Show local model status'),true);
  assert.equal(isHostCpuChatLookup('What time is it and search this chat'),false);
});
