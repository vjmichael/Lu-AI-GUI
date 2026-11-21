import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {submissionNonce,snapshotDraft,sameSubmittedDraft,receiptDecision,clearSubmittedDraft,emptyDraft,removeOwnedJob} from '../web/submission_lifecycle.mjs';
const draft={prompt:'Inspect these',images:[{imageRef:'a'},{imageRef:'b'}],files:[{name:'notes.txt',size:4,text:'note'}]};
const attempt={generationId:'g-a',marker:'marker',snapshot:snapshotDraft(draft)};
test('correlation nonce is bounded and independently generated',()=>{
  assert.match(submissionNonce(),/^s-[0-9a-f]{32}$/);
  assert.notEqual(submissionNonce(),submissionNonce());
});
test('only exact confirmed original permits consuming a draft',()=>{
  assert.equal(receiptDecision(attempt,{generationId:'g-a',visibleSeq:17,rejected:false}),'saved');
  assert.equal(receiptDecision(attempt,{generationId:'g-other',visibleSeq:17,rejected:false}),'uncertain');
  assert.equal(receiptDecision(attempt,{generationId:'g-a',visibleSeq:17}),'uncertain');
  assert.equal(receiptDecision(attempt,{generationId:'g-a',visibleSeq:17,rejected:true}),'uncertain');
  assert.equal(receiptDecision(attempt,{generationId:'g-a',visibleSeq:null,rejected:false}),'uncertain');
  assert.equal(receiptDecision(attempt,{generationId:'g-a',visibleSeq:null,rejected:true}),'rejected');
});
test('rejected or unconfirmed admission retains raw prompt, ordered images and file draft',()=>{
  const snapshot=snapshotDraft(draft);
  assert.deepEqual(snapshot,draft);
  assert.notEqual(snapshot.images,draft.images);
  assert.notEqual(snapshot.files,draft.files);
  assert.equal(emptyDraft(snapshot),false);
  assert.equal(sameSubmittedDraft(snapshot,draft),true);
});
test('confirmed old send cannot clear an edited or reordered next draft',()=>{
  const edited={...draft,prompt:'Next question'};
  const reordered={...draft,images:[...draft.images].reverse()};
  const changedText={...draft,files:[{...draft.files[0],text:'else'}]};
  for(const current of [edited,reordered,changedText]){
    assert.equal(sameSubmittedDraft(current,attempt.snapshot),false);
    assert.equal(clearSubmittedDraft(current,attempt.snapshot),current);
  }
  assert.deepEqual(clearSubmittedDraft(draft,attempt.snapshot),{prompt:'',images:[],files:[]});
});
test('late completion cannot delete another chat or a newer generation',()=>{
  const jobs={A:{generationId:'g-a'},B:{generationId:'g-b'}};
  assert.deepEqual(removeOwnedJob(jobs,'A',attempt),{B:jobs.B});
  const newer={...jobs,A:{generationId:'g-next'}};
  assert.equal(removeOwnedJob(newer,'A',attempt),newer);
  assert.deepEqual(removeOwnedJob({A:{generationId:null,submissionMarker:'marker'},B:jobs.B},
    'A',{marker:'marker',generationId:null}),{B:jobs.B});
});
const read=p=>readFileSync(new URL(p,import.meta.url),'utf8');
test('native receipt checks, refusal fence and restore remain host scoped',()=>{
  const host=read('../src-tauri/src/lib.rs');
  const core=read('../../src-tauri/lu-core/src/ansf_image_attachments.rs');
  assert.match(host,/authenticated_chat_turn_visible_seq\(&chat_id,&gid\)/);
  assert.match(host,/book.reject_image_submission\(chat_id,&gid\)/);
  assert.match(core,/image_submission_rejected\(tx,chat,turn\)\?/);
  assert.match(host,/book.restore_linked_draft\(&chat_id,visible_seq,&restored\)/);
  assert.match(host,/stage_attachment_ui_submission\(&chat_id,&receipt.generation_id/);
  assert.match(core,/unresolved_ui_snapshot_is_exact_and_cannot_be_overwritten_or_cleared/);
  assert.match(host,/restored|RestoredImageDraft/);
});
test('rendered lifecycle has explicit recovery and no implicit release or replay',()=>{
  const ui=read('../web/main.jsx');
  const portion=ui.slice(ui.indexOf('async function admitSavedImageAttempt'),ui.indexOf('async function attachReaderDocument'));
  assert.match(portion,/invoke\('lu_submit_image_turn'/);
  assert.match(ui,/invoke\('lu_prepare_image_submission'/);
  assert.match(ui,/invoke\('lu_pending_image_submission'/);
  assert.match(ui,/invoke\('lu_clear_image_submission'/);
  assert.match(portion,/submissionId:attempt.nonce/);
  assert.match(portion,/invoke\('lu_stop_image_submission'/);
  assert.match(portion,/invoke\('lu_restore_image_turn'/);
  assert.doesNotMatch(portion,/invoke\('lu_release_image'/);
  assert.match(ui,/aria-label="Saved attachments"/);
  assert.match(ui,/visibleSeq:entry.seq/);
});

test('linked-document identity distinguishes same-name drafts',()=>{
  const a={prompt:'Read',images:[],files:[{attachmentId:'link-a',name:'same.pdf'}]};
  const b={...a,files:[{attachmentId:'link-b',name:'same.pdf'}]};
  assert.equal(sameSubmittedDraft(a,b),false);
  assert.equal(clearSubmittedDraft(b,a),b);
});

test('late receipt preserves an in-place edited original file draft',()=>{
  const draft={prompt:'Inspect',images:[],files:[{name:'notes.txt',size:3,text:'old'}]};
  const original=snapshotDraft(draft);
  draft.files[0].text='new';
  assert.equal(original.files[0].text,'old');
  assert.equal(sameSubmittedDraft(draft,original),false);
  assert.equal(clearSubmittedDraft(draft,original),draft);
});
test('late receipt preserves an in-place changed image or linked-file identity',()=>{
  const draft={prompt:'Inspect',images:[{imageRef:'img-a'}],
    files:[{attachmentId:'doc-a',name:'same.pdf'}]};
  const original=snapshotDraft(draft);
  draft.images[0].imageRef='img-b';
  draft.files[0].attachmentId='doc-b';
  assert.equal(original.images[0].imageRef,'img-a');
  assert.equal(original.files[0].attachmentId,'doc-a');
  assert.equal(sameSubmittedDraft(draft,original),false);
  assert.equal(clearSubmittedDraft(draft,original),draft);
});
test('unchanged attachment snapshot still clears following durable native admission',()=>{
  const draft={prompt:'Inspect',images:[{imageRef:'img-a'}],
    files:[{name:'notes',size:3,text:'old'}]};
  const original=snapshotDraft(draft);
  assert.equal(sameSubmittedDraft(draft,original),true);
  assert.deepEqual(clearSubmittedDraft(draft,original),{prompt:'',images:[],files:[]});
});
