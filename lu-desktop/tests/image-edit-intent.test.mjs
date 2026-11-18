import test from 'node:test';
import assert from 'node:assert/strict';
import {validateImageEditIntent, IMAGE_EDIT_OPERATIONS} from '../web/image_edit_intent.mjs';
const base={asset_id:'image-1',base_revision:'sha256:original',base_width:800,base_height:600};
test('image editing intent has declared operations',()=>assert.equal(IMAGE_EDIT_OPERATIONS.length,7));
test('valid crop is a bounded immutable recipe, not a path grant',()=>{
 const x=validateImageEditIntent({...base,operation:'crop',region:{x:10,y:10,width:400,height:300},path:'C:/outside.png'});
 assert.equal(x.operation,'crop');assert.equal(x.path,undefined);assert.ok(Object.isFrozen(x));assert.ok(Object.isFrozen(x.region));
});
test('rejects overflow, invalid pixels, geometry, and missing operation arguments',()=>{
 for (const a of [
  {...base,operation:'crop',region:{x:799,y:599,width:2,height:1}},
  {...base,operation:'resize',target_width:32768,target_height:32768},
  {...base,operation:'erase',region:{x:0,y:0,width:2,height:2}},
  {...base,operation:'edit_prompt',text:''},
  {...base,operation:'shell'},
  {...base,base_width:0,operation:'remove_background'},
  {...base,base_revision:'',operation:'remove_background'},
 ]) assert.throws(()=>validateImageEditIntent(a));
});
test('valid bounded erase requires a referenced mask',()=>{
 const x=validateImageEditIntent({...base,operation:'erase',mask_id:'mask-1',region:{x:5,y:6,width:10,height:20}});
 assert.equal(x.mask_id,'mask-1');
});

test('control characters are rejected in image identities and edit text',()=>{
 assert.throws(()=>validateImageEditIntent({...base,base_revision:'revision\nspoof',operation:'crop',region:{x:0,y:0,width:1,height:1}}));
 assert.throws(()=>validateImageEditIntent({...base,operation:'edit_prompt',text:'line\rcontrol'}));
});
