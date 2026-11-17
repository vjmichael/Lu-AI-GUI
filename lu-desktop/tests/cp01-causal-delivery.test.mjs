import test from 'node:test';
import assert from 'node:assert/strict';
import {deliveryCandidate} from '../web/causal_delivery.mjs';
const witness={chatId:'a',generationId:'g-a',assistantSeq:12};
const entry={seq:12,role:'assistant',text:'answer',interrupted:false,cancelled:false};
const baseline={witness,chatId:'a',currentChat:'a',activeTab:'chat',job:null,entries:[entry],hasCommittedNode:true};
test('exact final original plus committed React DOM node reconciles the stream',()=>{
  assert.equal(deliveryCandidate(baseline),true);
  assert.equal(deliveryCandidate({...baseline,job:{generationId:'g-next'}}),true);
});
test('same text, different row/conversation, in-flight original, and hidden tab do not establish delivery',()=>{
  for(const variant of [
    {entries:[{...entry,seq:13}]},{chatId:'b',currentChat:'b'},{currentChat:'b'},
    {job:{generationId:'g-a'}},{activeTab:'settings'},{hasCommittedNode:false},
    {entries:[{...entry,interrupted:true}]},{entries:[{...entry,cancelled:true}]},
    {entries:[{...entry,role:'user'}]},{entries:[{...entry,text:''}]}
  ])assert.equal(deliveryCandidate({...baseline,...variant}),false);
});
