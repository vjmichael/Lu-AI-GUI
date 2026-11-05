import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';

const ui=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
const host=readFileSync(new URL('../src-tauri/src/lib.rs',import.meta.url),'utf8');

test('native Reader file selection is host-mediated and file path never supplied by WebView',()=>{
  assert.ok(host.includes('async fn lu_reader_pick_document(app:AppHandle)'));
  assert.ok(host.includes('blocking_pick_file()'));
  assert.ok(host.includes('lu_core::native_reader::read_text_bytes'));
  assert.ok(host.includes('MAX_PICK_BYTES:u64=8*1024*1024'));
  assert.ok(host.includes('meta.file_type().is_file()'));
  assert.ok(host.includes('read_to_end(&mut bytes)'));
  assert.ok(host.includes('lu_reader_pick_document,'));
  assert.ok(ui.includes("invoke('lu_reader_pick_document')"));
  assert.ok(!ui.includes("invoke('lu_reader_pick_document',{path:"));
});
test('Reader attachments remain in originating chat and respect the 16k prompt cap',()=>{
  assert.ok(ui.includes("const targetChat=currentChat.current||'new';"));
  assert.ok(ui.includes('pendingDrafts.current.set(targetChat,{...old,files:next})'));
  assert.ok(ui.includes('buildSubmittedPrompt(prior?.prompt'));
  assert.ok(ui.includes('MAX_TEXT_FILES'));
  assert.ok(ui.includes('document.truncated'));
  assert.ok(ui.includes('Read document'));
});
