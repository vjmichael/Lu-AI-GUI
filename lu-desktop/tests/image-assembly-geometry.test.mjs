import test from 'node:test';
import assert from 'node:assert/strict';
import {stagedImageDimensions,REGION_IMAGE_OPERATIONS,
  imagePointerPixel,imageDragRectangle}
  from '../web/image_assembly_geometry.mjs';

test('staged geometry tracks crop, resize, rotate and annotation space',()=>{
  const steps=[
    {operation:'crop',x:2,y:2,width:12,height:8},
    {operation:'rotate90'},
    {operation:'ellipse',x:0,y:0,width:8,height:12,stroke:2,color:0x223344},
    {operation:'line',x:1,y:2,width:6,height:8,stroke:3,color:0x112233},
  ];
  assert.deepEqual(stagedImageDimensions(20,16,steps),{width:8,height:12});
  assert.ok(REGION_IMAGE_OPERATIONS.includes('line'));
  assert.ok(REGION_IMAGE_OPERATIONS.includes('ellipse'));
});
test('new steps cannot use stale original coordinates after crop',()=>{
  assert.equal(stagedImageDimensions(20,16,[
    {operation:'crop',x:0,y:0,width:5,height:4},
    {operation:'redact',x:9,y:0,width:2,height:2},
  ]),null);
  assert.equal(stagedImageDimensions(20,16,[
    {operation:'line',x:0,y:0,width:10,height:10,stroke:33,color:0xffffff},
  ]),null);
  assert.equal(stagedImageDimensions(20,16,[
    {operation:'ellipse',x:0,y:0,width:10,height:10,stroke:1,color:0xffffff},
    {operation:'resize',width:9,height:9},
  ]).width,9);
  assert.equal(stagedImageDimensions(20,16,
    Array.from({length:13},()=>({operation:'flip_h'}))),null);
});

test('native filter parameters are bounded and never change dimensions',()=>{
  const valid=[
    {operation:'grayscale'},{operation:'invert'},
    {operation:'blur',sigma:2},
    {operation:'brightness',amount:-80},
    {operation:'contrast',amount:60},
    {operation:'sharpen',sigma:3,threshold:12}
  ];
  assert.deepEqual(stagedImageDimensions(64,48,valid),{width:64,height:48});
  assert.equal(stagedImageDimensions(64,48,[{operation:'blur',sigma:9}]),null);
  assert.equal(stagedImageDimensions(64,48,[{operation:'brightness',amount:129}]),null);
  assert.equal(stagedImageDimensions(64,48,[{operation:'contrast',amount:-101}]),null);
  assert.equal(stagedImageDimensions(64,48,[{operation:'sharpen',sigma:2,threshold:65}]),null);
});
test('pointer regions map rendered zoomed rectangles to real original pixels',()=>{
  const box={left:50,top:100,width:400,height:200};
  const first=imagePointerPixel(100,150,box,200,100);
  const last=imagePointerPixel(399,250,box,200,100);
  assert.deepEqual(first,{x:25,y:25});
  assert.deepEqual(last,{x:174,y:75});
  assert.deepEqual(imageDragRectangle(first,last,200,100),
    {x:25,y:25,width:150,height:51});
  assert.deepEqual(imagePointerPixel(-100,999,box,200,100),{x:0,y:99});
  assert.equal(imageDragRectangle({x:220,y:0},{x:1,y:1},200,100),null);
});
