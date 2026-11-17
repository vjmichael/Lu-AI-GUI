import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
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
test('all File-object entrypoints use same native intake and document text cannot open a native intent route',()=>{
  const ui=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
  const host=readFileSync(new URL('../src-tauri/src/lib.rs',import.meta.url),'utf8');
  assert.match(ui,/accept=\{ATTACHMENT_ACCEPT\}/);
  assert.match(ui,/invoke\('lu_reader_ingest_document',\{chatId:targetChat,\.\.\.payload\}\)/);
  assert.match(ui,/void importAttachments\(fileItems\)/);
  assert.match(ui,/void importAttachments\(event.dataTransfer\?\.files\)/);
  const importer=ui.slice(ui.indexOf('async function importAttachments'),ui.indexOf('function handlePaste'));
  assert.doesNotMatch(importer,/file.text\(\)|PDF\/DOCX are not yet supported/);
  assert.match(host,/reader_attachment_view\(name,bytes,selector,offset,expected\)/);
  assert.match(host,/reader_attachment_view\(display_name,bytes,String::new\(\),0,String::new\(\)\)/);
  assert.match(host,/caller.label\(\)!="main"/);
  assert.match(host,/if !document_context \{[\s\S]*?native intent routing from attached document text/);
  assert.match(host,/if origin==GenerationOrigin::Human&&!plan_only&&!document_context/);
  assert.match(host,/if !plan_only&&!document_context/);
});
