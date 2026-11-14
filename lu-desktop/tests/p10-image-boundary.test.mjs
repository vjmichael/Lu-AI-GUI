import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read=path=>readFileSync(new URL(path,import.meta.url),'utf8');
const core=read('../../src-tauri/lu-core/src/native_image_revision.rs');
const host=read('../src-tauri/src/image_workspace_desktop.rs');
const desktop=read('../src-tauri/src/lib.rs');
const ledger=read('../../src-tauri/lu-core/src/native_image_revision_history.rs');
const principles=read('../../docs/LU_AI_TOOL_DESIGN_PRINCIPLES_20261107.md');

test('P10 remains a bounded image I/O capability, not an image application',()=>{
  assert.match(principles,/No feature cloning/);
  assert.match(principles,/P10 Image/);
  assert.match(core,/pub enum ImageRevisionOperation/);
  for(const op of ['Crop','Resize','Rotate90','FlipHorizontal','Redact','Pixelate','Rectangle','Line','Ellipse','Arrow','Grayscale','Blur','Sharpen'])
    assert.match(core,new RegExp('    '+op+'\\b'));
  assert.doesNotMatch(core,/pub enum (Layer|BrushEngine|GenerativePipeline)/);
});
test('P10 review preserves original bytes and distinguishes redaction from mosaic',()=>{
  assert.match(core,/fn solid_redaction_is_opaque_and_does_not_reveal_original_pixels/);
  assert.match(core,/fn image_plan_rejects_empty_or_excessive_stages_without_mutation/);
  assert.match(core,/mosaic, not a security redaction/);
  assert.match(core,/source\.bytes\.clone\(\)/);
  assert.match(host,/persisted:false/);
  assert.match(desktop,/image_history\.reconcile_interrupted\(\)/);
  assert.match(ledger,/Never auto-replay an/);
});
test('P10 user-reviewed output remains a new source-pinned file and uncertain work stays uncertain',()=>{
  assert.match(host,/write_new_revision_plan/);
  assert.match(host,/overwritten_existing_file:bool/);
  assert.match(desktop,/journal\.prepare\(&chat_id,&intended\)/);
  assert.match(desktop,/journal\.uncertain\(&chat_id,&revision_id\)/);
});
