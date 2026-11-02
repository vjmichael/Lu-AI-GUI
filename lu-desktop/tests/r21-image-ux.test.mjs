import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {sniffSupportedImageMime,isPotentialImage,normalizedImageFile} from '../web/image_attachment_input.mjs';

const png=Uint8Array.from([137,80,78,71,13,10,26,10,0,0,0,13]);
const jpeg=Uint8Array.from([255,216,255,224,0,16,74,70,73,70]);
const ui=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
const css=readFileSync(new URL('../web/styles.css',import.meta.url),'utf8');
function fakeFile(header,name,type='image/jpeg'){
 const blob=new Blob([header],{type});
 return {name,type,size:blob.size,slice:blob.slice.bind(blob)};
}

test('recognizes PNG and JPEG signatures rather than trusting extension',()=>{
 assert.equal(sniffSupportedImageMime(png),'image/png');
 assert.equal(sniffSupportedImageMime(jpeg),'image/jpeg');
 assert.equal(sniffSupportedImageMime([0,0,0]),null);
 assert.equal(isPotentialImage({name:'test-doc-image.jpg',type:'image/jpeg'}),true);
});
test('a PNG misnamed .jpg gets a correctly typed image blob',async()=>{
 const item=await normalizedImageFile(fakeFile(png,'test-doc-image.jpg'));
 assert.equal(item.mimeType,'image/png');
 assert.equal(item.blob.type,'image/png');
 assert.equal(item.wasRelabeled,true);
});
test('a genuine JPEG remains JPEG',async()=>{
 const item=await normalizedImageFile(fakeFile(jpeg,'test_picture.jpg'));
 assert.equal(item.mimeType,'image/jpeg');
 assert.equal(item.wasRelabeled,false);
});
test('unsupported disguised image fails closed',async()=>{
 await assert.rejects(normalizedImageFile(fakeFile([60,104,116,109,108,62],'malicious.jpg')),/not a supported PNG\/JPEG/);
});
test('image attachment normalization happens before host admission',()=>{
 assert.match(ui,/await normalizedImageFile\(file\)/);
 assert.match(ui,/reader\.readAsDataURL\(normalized\.blob\)/);
 assert.match(ui,/invoke\('lu_ingest_image_data_url'/);
});
test('drop targets the whole Lu shell and not just the composer',()=>{
 assert.match(ui,/className=\{'shell' \+ \(dragActive/);
 assert.match(ui,/onDrop=\{handleDrop\}/);
 assert.match(ui,/appDropHint/);
 assert.match(css,/\.shell\.appDragActive::after/);
});
test('model loading is labeled without inventing percentage telemetry',()=>{
 assert.match(ui,/role="status" aria-live="polite"/);
 assert.match(ui,/modelLoadSeconds.*elapsed/);
 assert.match(ui,/setInterval\(\(\) => setModelLoadSeconds/);
 assert.match(css,/@keyframes spin/);
 assert.match(css,/prefers-reduced-motion:reduce/);
});
