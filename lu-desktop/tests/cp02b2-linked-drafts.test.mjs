import test from 'node:test';
import assert from 'node:assert/strict';
import {mergeAttachmentDraft,attachmentRefs,attachedRecoveryMatches,attachmentSubmissionText} from '../web/linked_attachment_drafts.mjs';
const id=(kind,n)=>kind+'-'+String(n).padStart(32,'0');
const doc=(n,extra={})=>({attachmentId:id('doc',n),name:'Report '+n+'.pdf',bytes:42,available:true,readerFormat:'Pdf',selector:'page:1',offsetChars:0,...extra});
const pic=(n,extra={})=>({attachmentId:id('img',n),name:'photo.png',bytes:32,mediaType:'image/png',available:true,...extra});
const draft={prompt:'  Examine these  ',images:[],files:[]};
test('mixed draft preserves native interleaving and exact human text, not document bytes',()=>{
  const legacy={imageRef:id('img',9),name:'capture',observed:true,available:true};
  const merged=mergeAttachmentDraft({images:[legacy],linked:[doc(1,{text:'must not persist',sourcePath:'/private'}),pic(2),doc(3)]},draft);
  assert.deepEqual(attachmentRefs(merged),{imageRefs:[id('img',9)],linkedRefs:[id('doc',1),id('img',2),id('doc',3)]});
  assert.equal(attachmentSubmissionText(merged),draft.prompt);
  assert.equal(merged.images[0].observed,true);
  assert.ok(!JSON.stringify(merged).includes('must not persist'));
  assert.ok(!JSON.stringify(merged).includes('/private'));
  assert.ok(attachedRecoveryMatches(merged,attachmentRefs(merged)));
});
test('reordered, removed, cross-chat and duplicate references cannot resume a nonce',()=>{
  const merged=mergeAttachmentDraft({images:[],linked:[doc(1),pic(2)]},draft);
  const snap=attachmentRefs(merged);
  for(const rows of [[pic(2),doc(1)],[doc(1)],[doc(4),pic(2)]]){
    assert.equal(attachedRecoveryMatches(mergeAttachmentDraft({images:[],linked:rows},draft),snap),false);
  }
  const duplicate=mergeAttachmentDraft({images:[],linked:[doc(1),doc(1)]},draft);
  assert.throws(()=>attachmentRefs(duplicate),/Attachments changed/);
});
test('missing original drops stale preview and refuses sending without deleting linkage',()=>{
  const previous=mergeAttachmentDraft({images:[],linked:[pic(2)]},draft);
  previous.images[0].previewUrl='data:image/png;base64,AA';
  const merged=mergeAttachmentDraft({images:[],linked:[pic(2,{available:false})]},previous);
  assert.equal(merged.images[0].imageRef,id('img',2));
  assert.equal(merged.images[0].previewUrl,undefined);
  assert.throws(()=>attachmentSubmissionText(merged),/File not available/);
});
test('pathless old document excerpt cannot be sent as a new original-file link',()=>{
  assert.throws(()=>attachmentSubmissionText({...draft,files:[{name:'old.txt',text:'old'}]}),/Attach the original/);
  assert.equal(attachmentSubmissionText(draft),'Examine these');
  assert.throws(()=>attachmentSubmissionText({...draft,prompt:'🦈'.repeat(4097)}),/16 KiB/);
});
test('document-only Send has a durable linked receipt identity',()=>{
  const linked=mergeAttachmentDraft({images:[],linked:[doc(1)]},draft);
  assert.deepEqual(attachmentRefs(linked),{imageRefs:[],linkedRefs:[id('doc',1)]});
  assert.ok(attachedRecoveryMatches(linked,{imageRefs:[],linkedRefs:[id('doc',1)]}));
  assert.equal(attachedRecoveryMatches(linked,{imageRefs:[]}),false);
});
