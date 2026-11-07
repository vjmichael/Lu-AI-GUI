import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const ui=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
const core=readFileSync(new URL('../../src-tauri/lu-core/src/native_image_revision.rs',import.meta.url),'utf8');
const host=readFileSync(new URL('../src-tauri/src/lib.rs',import.meta.url),'utf8');
const bridge=readFileSync(new URL('../src-tauri/src/image_workspace_desktop.rs',import.meta.url),'utf8');
test('image annotations use the same native immutable preview/export pipeline',()=>{
  assert.match(ui,/stagedImageDimensions\(imageViewer\.width,imageViewer\.height,next\)/);
  assert.match(ui,/\<option value="line"\>/);
  assert.match(ui,/\<option value="ellipse"\>/);
  assert.match(bridge,/"rectangle"\|"line"\|"ellipse"/);
  assert.match(host,/"rectangle"\|"line"\|"ellipse"/);
  assert.match(core,/ImageRevisionOperation::Ellipse/);
  assert.match(core,/ImageRevisionOperation::Line/);
  assert.match(core,/transform_admitted_image_sequence/);
  assert.match(bridge,/preview_revision_plan/);
  assert.match(host,/write_new_revision_plan/);
});

test('bounded native filters and original image selection share immutable assembly',()=>{
  assert.match(ui,/stagedImageDimensions\(imageViewer\.width,imageViewer\.height,next\)/);
  assert.match(ui,/onPointerDown=\{selectImagePointerStart\}/);
  assert.match(ui,/onPointerUp=\{selectImagePointerEnd\}/);
  assert.match(ui,/imageDragRectangle\(/);
  assert.match(ui,/\<option value="brightness"\>/);
  assert.match(ui,/\<option value="blur"\>/);
  assert.match(ui,/\<option value="sharpen"\>/);
  assert.match(ui,/operation:queuedPlan\?'resize':imageEdit.mode/);
  assert.match(core,/ImageRevisionOperation::Grayscale/);
  assert.match(core,/ImageRevisionOperation::InvertRgb/);
  assert.match(core,/ImageRevisionOperation::Brightness/);
  assert.match(core,/ImageRevisionOperation::Contrast/);
  assert.match(core,/ImageRevisionOperation::Sharpen/);
  assert.match(core,/original_alpha\(&decoded/);
  assert.match(bridge,/"sharpen"=>Op::Sharpen/);
  assert.match(host,/"contrast"=>Operation::Contrast/);
});

test('new arrows and staged-preview drag selection are native-source bounded',()=>{
  assert.match(ui,/<option value="arrow">Arrow annotation<\/option>/);
  assert.match(ui,/onPointerDown=\{startStagedSelection\}/);
  assert.match(ui,/onPointerMove=\{moveStagedSelection\}/);
  assert.match(ui,/onPointerUp=\{endStagedSelection\}/);
  assert.match(ui,/stageSelection\?\.key===currentImagePreviewKey/);
  assert.match(ui,/setImageEdit\(v=>\(\{\.\.\.v,x:String\(rect\.x\)/);
  assert.match(core,/ImageRevisionOperation::Arrow/);
  assert.match(core,/paint_segment\(&mut canvas/);
  assert.match(bridge,/"arrow"=>Op::Arrow/);
  assert.match(host,/"arrow"=>Operation::Arrow/);
});
