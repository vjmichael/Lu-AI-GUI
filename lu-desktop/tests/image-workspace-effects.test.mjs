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
