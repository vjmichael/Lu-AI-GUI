import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read=(path)=>readFileSync(new URL(path,import.meta.url),'utf8');
const tasks=read('../../src-tauri/lu-core/src/tasks.rs');
const lane=read('../../src-tauri/lu-core/src/native_awareness_lane.rs');
const host=read('../src-tauri/src/observe_desktop.rs');
const desktop=read('../src-tauri/src/lib.rs');
const ui=read('../web/main.jsx');
const native=read('../../src-tauri/lu-core/src/native_observe_learn.rs');

test('P06 observation is off-by-default and never grants ambient desktop capture',()=>{
  assert.match(native,/mode:Mode::Off/);
  assert.match(host,/can_see_desktop:false/);
  assert.match(host,/screen_capture_enabled:false/);
  assert.match(host,/allow_redacted_snapshots:false/);
  assert.match(host,/inference_performed:0/);
  assert.match(desktop,/lu_observe_start/);
  assert.match(desktop,/blocking_show\(\)/);
});
test('P06 old native_runtime CHECK schema is atomically migrated without dropping rows',()=>{
  assert.match(tasks,/source TEXT NOT NULL CHECK/);
  assert.match(tasks,/native_accessibility','native_runtime','native_test_receipt/);
  assert.match(tasks,/if !awareness_schema\.contains\("'native_runtime'"\)/);
  assert.match(tasks,/INSERT INTO lu_awareness_notes_p06/);
  assert.match(tasks,/FROM lu_awareness_notes;/);
  assert.match(tasks,/p06_native_runtime_notes_migrate_and_private_deletion_is_chat_scoped/);
  assert.match(host,/p06_trusted_runtime_error_persists_but_cannot_claim_verified_gui_failure/);
});
test('P06 seven-day retention and chat-only deletion cannot mint tool or learning grant',()=>{
  assert.match(tasks,/datetime\('now','-7 days'\)/);
  assert.match(tasks,/delete_awareness_notes_for_workspace/);
  assert.match(tasks,/WHERE workspace_id=\?1 AND task_id LIKE 'obs-%'/);
  assert.match(tasks,/p06_note_retention_prunes_stale_only/);
  assert.match(host,/pub\(crate\) fn forget_notes/);
  assert.match(host,/awareness\.discard_observations\(\)/);
  assert.match(lane,/pub fn discard_observations/);
  assert.match(lane,/p06_forget_clears_handoff_without_regrant_or_sequence_replay/);
  assert.match(desktop,/async fn lu_observe_forget_notes/);
  assert.match(desktop,/state\.chats\.history\(&chat_id\)/);
  assert.match(desktop,/Forget diagnostic notes/);
  assert.match(ui,/invoke\('lu_observe_forget_notes'/);
  assert.match(ui,/Forget observation notes for this chat/);
  assert.match(ui,/diagnostic codes expire after seven days/i);
  assert.doesNotMatch(host,/grant_learning_review\(true\)/);
});
test('P06 capture receipts never imply verified visual interpretation or retained pixels',()=>{
  assert.match(host,/source_identity_attested:false/);
  assert.match(host,/pixels_retained:false/);
  assert.match(host,/visual_interpretation_verified:false/);
  assert.match(desktop,/lu_observe_capture_one_shot/);
  assert.match(ui,/No observation data/);
});
