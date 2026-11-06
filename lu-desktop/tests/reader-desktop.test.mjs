import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {buildSubmittedPrompt} from '../web/attachment_drafts.mjs';

const ui=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
const host=readFileSync(new URL('../src-tauri/src/lib.rs',import.meta.url),'utf8');
const readerHost=readFileSync(new URL('../src-tauri/src/reader_desktop.rs',import.meta.url),'utf8');

test('native Reader file selection is host-mediated and file path never supplied by WebView',()=>{
  assert.ok(host.includes('async fn lu_reader_pick_document(app:AppHandle)'));
  assert.ok(readerHost.includes('blocking_pick_file()'));
  assert.ok(host.includes('lu_core::native_reader::read_text_bytes'));
  assert.ok(host.includes('pick_reader_bytes(&app,8*1024*1024)'));
  assert.ok(readerHost.includes('meta.file_type().is_file()'));
  assert.ok(readerHost.includes('read_to_end(&mut bytes)'));
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
  assert.ok(ui.includes('Read file'));
});

test('Reader attachment context uses the model UTF-8 byte budget, not JS code-unit length',()=>{
  const unicode='🦈'.repeat(4400);
  assert.throws(()=>buildSubmittedPrompt('Read this',[{name:'marine.docx',text:unicode}]),
    /16,384-byte/);
  assert.match(buildSubmittedPrompt('Summarize',[{name:'notes.docx',text:'hello'}]),
    /notes.docx/);
});

test('Ask Reader routes exactly one native-picker document through the supervised read-only model turn',()=>{
  assert.ok(host.includes('lu_reader_ask_document,'));
  assert.ok(readerHost.includes('fn lu_reader_ask_document('));
  assert.ok(readerHost.includes('ReaderTurnSession::new('));
  assert.ok(readerHost.includes('reader_user_turn_matches('));
  assert.ok(readerHost.includes('append_reader_user(&chat_id,&generation_id,&prompt)'));
  assert.ok(!readerHost.includes('messages.last().cloned()'));

  assert.ok(readerHost.includes('ModelTurnJournal::open('));
  assert.ok(readerHost.includes('run_journaled_observed('));
  assert.ok(readerHost.includes('if result.tool_calls==0'));
  assert.ok(readerHost.includes('ticket.wait_for_slot()?'));
  assert.ok(host.includes('reader.cancellation.cancel()'));
  assert.ok(host.includes('reader.current_step.lock()'));
  assert.ok(ui.includes("invoke('lu_reader_ask_document',{"));
  assert.ok(ui.includes('Ask Reader</button>'));
  assert.ok(!ui.includes("invoke('lu_reader_ask_document',{path:"));
  assert.ok(!ui.includes("invoke('lu_reader_ask_document',{filePath:"));
});

test('attached-image OCR shares the pinned worker and remains user-reviewed',()=>{
  assert.match(host,/async fn lu_ocr_read_attached_image\(/);
  assert.match(host,/lu_ocr_read_attached_image,/);
  assert.match(host,/offline_ocr_view\(&source\.bytes/);
  assert.match(host,/Image detached or changed during OCR approval/);
  assert.match(host,/source_verified:false,human_review_required:true/);
  assert.match(ui,/invoke\('lu_ocr_read_attached_image'/);
  assert.match(ui,/ocrImageScopeRef\.current===key/);
  assert.match(ui,/Recognize text in this attached image/);
});

test('native-picked OCR pins bytes before consent and never reopens after approval',()=>{
  const start=host.indexOf('async fn lu_ocr_read_selected_image(');
  const end=host.indexOf('async fn lu_ocr_read_attached_image(',start);
  const code=host.slice(start,end);
  const read=code.indexOf('file.take(12*1024*1024+1).read_to_end(&mut bytes)');
  const consent=code.indexOf('let approved=tauri::async_runtime::spawn_blocking');
  const run=code.indexOf('offline_ocr_view(&bytes');
  assert.ok(read>0&&consent>read&&run>consent);
  assert.doesNotMatch(code.slice(consent),/std::fs::read\(&path\)/);
  assert.match(code,/Source SHA-256/);
});
test('reviewed OCR can be corrected but not overfill the chat draft',()=>{
  assert.match(ui,/const \[ocrReviewText,setOcrReviewText\]=useState/);
  assert.match(ui,/maxLength=\{12_000\}/);
  assert.match(ui,/candidate\.length>16\*1024/);
  assert.match(ui,/User-edited OCR transcription/);
  assert.match(ui,/The worker text SHA-256 identifies the uncorrected transcript/);
});
