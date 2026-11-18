import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {isModelFreeSendQuestion} from '../web/native_intent.mjs';
const read=rel=>readFileSync(new URL(rel,import.meta.url),'utf8');
const parity=JSON.parse(read('./cp05-routing-parity.json'));
const ui=read('../web/main.jsx');
const host=read('../src-tauri/src/lib.rs');
const coordinator=read('../src-tauri/src/turn_coordinator.rs');

test('CP05D4 shared native parity matrix: exact CPU affordance only, never renderer tool routing',()=>{
  assert.equal(parity.length,37);
  for(const {prompt,route} of parity){
    assert.equal(isModelFreeSendQuestion(prompt),route.startsWith('cpu-'),prompt);
    assert.equal(isModelFreeSendQuestion(`${prompt}\n[Attached text file: original]`),false,prompt);
  }
  for(const veto of ['no tools','without tools','without any tools','without using tools','do not use tools',"don't use tools",'do not call tools',"don't call tools",'without specialists','without any specialists','do not use specialists','never call a tool']){
    assert.equal(isModelFreeSendQuestion(`Search this chat for: abc ${veto}`),false,veto);
    assert.equal(isModelFreeSendQuestion(`Search this folder for: abc ${veto}`),false,veto);
  }
});
test('CP05D4 ordinary composer has exactly one textual RPC, attachment transaction separate',()=>{
  const send=ui.slice(ui.indexOf('async function send(e)'),ui.indexOf('async function openSavedAttachment('));
  assert.equal([...send.matchAll(/invoke\('lu_generate'/g)].length,1);
  assert.doesNotMatch(ui,/workspaceToolsNextSend|planDraftNextSend|reviewedAppNextSend|browserResearchNextSend|browserOrigins|readOnlyWorkspace:|planOnly:|lu_reviewed_app_ask|lu_generate_browser/);
  assert.match(send,/if\(attempt\.nonce\)\{\s*await runImageAttempt/);
  assert.match(send,/invoke\('lu_generate',\{sessionId:runtime\.sessionId\?\?'',chatId:activeId,\s*prompt:submitted,imageRefs:\[\],maxNewTokens:512\}\)/);
  assert.match(send,/clearSubmittedDraft\(readComposerDraft\(activeId\),snapshot\)/);
  assert.match(host,/SendMode::from_legacy\(read_only_workspace,plan_only\)\?/);
  assert.match(host,/match turn_coordinator::select\(&prompt,&image_refs,max_new_tokens,mode\)/);
  assert.match(coordinator,/\(true,true\)=>Err/);
  assert.match(host,/capability_negotiation::try_read_turn\(/);
});
