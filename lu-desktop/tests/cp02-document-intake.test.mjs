import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const coordinator=readFileSync(new URL('../src-tauri/src/turn_coordinator.rs',import.meta.url),'utf8');
import {ATTACHMENT_ACCEPT, DOCUMENT_EXTENSIONS, MAX_DOCUMENT_BYTES, supportedDocumentName, documentFilePayload, documentDraftItem} from '../web/document_attachment_input.mjs';
import {buildSubmittedPrompt} from '../web/attachment_drafts.mjs';
test('a single supported attachment list covers common native Reader formats',()=>{
  for(const ext of ['pdf','docx','xlsx','pptx','odt','ods','odp','txt','csv','epub','rtf']){
    assert.ok(DOCUMENT_EXTENSIONS.includes(ext));
    assert.ok(supportedDocumentName('report.'+ext.toUpperCase()));
    assert.ok(ATTACHMENT_ACCEPT.split(',').includes('.'+ext));
  }
  assert.equal(supportedDocumentName('document.exe'),false);
});
test('intake transfers original bytes without UTF-8 decoding or trusting MIME',async()=>{
  const bytes=Uint8Array.from([0x50,0x4b,3,4,255,0,128,1]);
  const file={name:'test.docx',type:'text/plain',size:bytes.length,arrayBuffer:async()=>bytes.buffer};
  const actual=await documentFilePayload(file);
  assert.equal(actual.displayName,file.name);
  assert.deepEqual(Buffer.from(actual.encodedBytes,'base64'),Buffer.from(bytes));
  await assert.rejects(()=>documentFilePayload({...file,size:MAX_DOCUMENT_BYTES+1}),/8 MB/);
  await assert.rejects(()=>documentFilePayload({...file,size:bytes.length+1}),/changed/);
  await assert.rejects(()=>documentFilePayload({...file,name:'bad.exe'}),/supported/);
});
test('extraction metadata is retained and prompt budget remains enforced',()=>{
  const doc={name:'report.xlsx',bytesRead:304,format:'Xlsx',selector:'sheet:2026',
    offsetChars:12,sourceSha256:'a'.repeat(64),text:'A1: 42',truncated:true};
  const item=documentDraftItem(doc);
  assert.equal(item.sourceSha256,doc.sourceSha256);
  assert.equal(item.selector,doc.selector);
  assert.equal(item.offsetChars,12);
  assert.match(item.text,/sheet:2026/);
  assert.match(item.text,/more text may remain/);
  assert.match(buildSubmittedPrompt('Summarize',[item]),/A1: 42/);
  assert.throws(()=>documentDraftItem({...doc,sourceSha256:'forged'}),/confirm/);
  assert.throws(()=>documentDraftItem({...doc,text:' '}),/confirm/);
  assert.throws(()=>buildSubmittedPrompt('Summarize',[{...item,text:'🦈'.repeat(5000)}]),/16,384-byte/);
});
test('full 8 MiB exact payload preserved across base64 chunk boundaries',async()=>{
  const bytes=new Uint8Array(MAX_DOCUMENT_BYTES);
  for(let i=0;i<bytes.length;i++)bytes[i]=i%251;
  const file={name:'large.pdf',size:bytes.length,arrayBuffer:async()=>bytes.buffer};
  const result=await documentFilePayload(file);
  assert.deepEqual(Buffer.from(result.encodedBytes,'base64'),Buffer.from(bytes));
});
test('native original-file entrypoints share admission and document text cannot open a native intent route',()=>{
  const ui=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
  const host=readFileSync(new URL('../src-tauri/src/lib.rs',import.meta.url),'utf8');
  const intake=readFileSync(new URL('../src-tauri/src/linked_attachment_intake.rs',import.meta.url),'utf8');
  assert.match(ui,/runNativeAttachmentIntake\('lu_pick_linked_files'/);
  assert.match(ui,/runNativeAttachmentIntake\('lu_paste_linked_files'/);
  assert.match(ui,/runNativeAttachmentIntake\('lu_accept_linked_drop'/);
  assert.doesNotMatch(ui,/invoke\('lu_reader_ingest_document'|new FileReader\(/);
  assert.match(intake,/admit_linked_drafts/);
  assert.match(host,/linked_attachment_intake::model_context\(&prompt,&linked\)/);
  assert.match(host,/append_user_with_attachment_drafts\(&chat_id,&generation_id,&prompt,&manifest,&linked\)/);
  assert.match(host,/let document_context=!linked.is_empty\(\)\|\|contains_document_context\(&prompt\)/);
  assert.match(coordinator,/if crate::contains_document_context\(prompt\)\{[\s\S]*?return Ok\(TurnRoute::Model\)/);
  assert.match(host,/match turn_coordinator::select\(/);
  assert.match(host,/if origin==GenerationOrigin::Human&&!plan_only&&!document_context/);
  assert.match(host,/if !plan_only&&!document_context/);
});
