import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';

const desktop=readFileSync(new URL('../src-tauri/src/lib.rs',import.meta.url),'utf8');
const core=readFileSync(new URL('../../src-tauri/lu-core/src/native_image_revision_history.rs',import.meta.url),'utf8');
const screen=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');

test('native image export records intent before original-safe immutable creation',()=>{
  assert.match(desktop,/journal\.prepare\(&chat_id,&intended\)/);
  assert.match(desktop,/write_new_revision\(&source,edit,&destination\)/);
  assert.match(desktop,/journal\.applied\(&chat_id,&revision_id,/);
  assert.match(desktop,/journal\.uncertain\(&chat_id,&revision_id\)/);
  assert.match(desktop,/image_history\.reconcile_interrupted\(\)/);
  assert.match(core,/SET state='UNCERTAIN' WHERE state='PREPARED'/);
  assert.match(core,/WHERE id=\?1 AND chat_id=\?2 AND revision_sha256=\?3 AND state='PREPARED'/);
});
test('staged native image preview is volatile, origin-pinned and never an implicit save',()=>{
  const preview=readFileSync(new URL('../src-tauri/src/image_workspace_desktop.rs',import.meta.url),'utf8');
  assert.match(desktop,/async fn lu_image_workspace_preview\(/);
  assert.match(desktop,/lu_image_workspace_preview,/);
  assert.match(desktop,/preview_revision_plan\(&source,&steps\)/);
  assert.match(preview,/transform_admitted_image_sequence\(source,&operations\)/);
  assert.match(preview,/data_url:format!\("data:image\/png;base64/);
  assert.match(preview,/persisted:false/);
  assert.match(screen,/invoke\('lu_image_workspace_preview'/);
  assert.match(screen,/imagePreviewKeyRef\.current===key/);
  assert.match(screen,/No file saved\. Source image remains unchanged/);
  assert.match(screen,/const queuedPlan=imagePlan\.chatId===chatId/);
  assert.match(screen,/operation:queuedPlan\?'resize':imageEdit\.mode/);
  assert.match(screen,/steps:queuedPlan/);
  assert.match(screen,/blockSize:'12',stroke:'3',color:'#ff5500'/);
});
test('history is chat scoped and displays receipts, not paths or automatic undo rights',()=>{
  assert.match(desktop,/fn lu_image_workspace_history\(/);
  assert.match(desktop,/ledger\.history\(&chat_id,32\)/);
  assert.match(desktop,/lu_image_workspace_history,/);
  assert.match(core,/WHERE chat_id=\?1/);
  assert.match(core,/parent_revision_id/);
  assert.match(core,/prev\.revision_sha256=cur\.base_sha256/);
  assert.match(core,/prev\.chat_id=cur\.chat_id/);
  assert.match(screen,/invoke\('lu_image_workspace_history',\{chatId:sourceChat\}\)/);
  assert.match(screen,/imageHistoryView\.chatId===chatId/);
  assert.match(screen,/Historical exports only/);
  assert.doesNotMatch(desktop,/fn lu_image_workspace_auto_restore\(/);
});
