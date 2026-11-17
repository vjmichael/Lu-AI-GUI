import test from 'node:test';
import assert from 'node:assert/strict';
import { splitModelOutput, streamForChat } from '../web/response_view.mjs';
import {readFileSync} from 'node:fs';
const ui = readFileSync(new URL('../web/main.jsx', import.meta.url),'utf8');
const host = readFileSync(new URL('../src-tauri/src/lib.rs', import.meta.url),'utf8');
const config = JSON.parse(readFileSync(new URL('../src-tauri/tauri.conf.json', import.meta.url),'utf8'));

test('thinking spans produce an isolated, collapsible part',() => {
  assert.deepEqual(splitModelOutput('hello <think>reasoning words</think>world'),[
    {kind:'answer',text:'hello '},{kind:'think',text:'reasoning words'},
    {kind:'answer',text:'world'}]);
});
test('unfinished thinking output remains typed as reasoning',() => {
  assert.deepEqual(splitModelOutput('<think>unfinished'),[{kind:'think',text:'unfinished'}]);
});
test('normal response is not mistaken for hidden reasoning',()=>{
  assert.deepEqual(splitModelOutput('4'),[{kind:'answer',text:'4'}]);
});
test('background token stream never leaks into selected chat',()=>{
  assert.equal(streamForChat({chatId:'chat-A',text:'private response'},'chat-B'),'');
  assert.equal(streamForChat({chatId:'chat-A',text:'private response'},'chat-A'),'private response');
});
test('Windows HTML drag and drop is enabled in WebView',()=>{
  assert.equal(config.app.windows[0].dragDropEnabled,false);
});
test('submitting an image retains draft until canonical native admission',()=>{
  const start=ui.indexOf('async function send(e)');
  const section=ui.slice(start,ui.indexOf('async function openSavedAttachment(',start));
  assert.match(section,/snapshotDraft\(readComposerDraft\(origin\)\)/);
  assert.match(section,/await runImageAttempt\(activeId,attempt/);
  assert.match(ui,/receiptDecision\(attempt,receipt\)!=='saved'/);
  assert.match(ui,/writeComposerDraft\(target,clearSubmittedDraft\(old,attempt.snapshot\)\)/);
  assert.doesNotMatch(section,/setPrompt\(''\); setImages\(\[\]\)/);
});
test('cancelled saved turn offers explicit restore, not automatic duplicate submission',()=>{
  assert.match(ui,/Message stopped\. Your original images are saved/);
  assert.match(ui,/invoke\('lu_restore_image_turn'/);
  assert.match(ui,/Keep draft without sending/);
});
test('transcript is never forcibly switched on background completion',()=>{
  assert.ok(!ui.includes('switchChatWhenSafe'));
  assert.match(ui,/if\(currentChat.current===target\)/);
});
test('image content is admitted by the native host with strict size caps',()=>{
  assert.ok(host.includes('fn lu_ingest_image_data_url('));
  assert.ok(host.includes('admit_data_url(&data_url)'));
  assert.ok(host.includes('data_url.len() > 11 * 1024 * 1024'));
});
test('no unconditional model auto-load path',()=>{
  assert.ok(!ui.includes('lu_load_model\', {modelRef: models[0]'));
});
