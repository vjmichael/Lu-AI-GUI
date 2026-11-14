import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const source = path => readFileSync(new URL(path, import.meta.url), 'utf8');

test('IDE-MERGE-02 restored source actions reach one Tauri host and explicit Explorer controls', () => {
  const desktop=source('../src-tauri/src/lib.rs');
  const ui=source('../web/main.jsx');
  const host=source('../../host/src/lib.rs');
  for(const command of [
    'ide_create_source_entry','ide_move_source_file','ide_trash_source_file',
    'ide_list_trashed_sources','ide_restore_source_file',
  ]) {
    assert.match(desktop,new RegExp('async fn '+command+'\\('));
    assert.match(desktop,new RegExp('\\s'+command+','));
    assert.match(ui,new RegExp("'"+command+"'"));
  }
  assert.match(host,/pub mod source_create;/);
  assert.match(host,/pub mod source_file_transactions;/);
  assert.match(ui,/sourceActionTicket/);
  assert.match(ui,/workspaceTransitionRef\.current/);
  assert.match(ui,/SOURCE_ACTION_CONTEXT_CHANGED/);
  assert.match(ui,/SOURCE_ACTION_RECEIPT_MISMATCH/);
  assert.match(ui,/Close this file tab first/);
  assert.match(ui,/sourceActionMode==='restore'/);
});
test('IDE-MERGE-02 does not replace existing editor, private drafts or Quick Open', () => {
  const desktop=source('../src-tauri/src/lib.rs');
  const ui=source('../web/main.jsx');
  for(const command of ['ide_save_source_file','ide_draft_checkpoint','ide_draft_latest',
    'ide_draft_load','ide_draft_discard','ide_quick_open_source_paths',
    'ide_list_source_directory_page']) {
    assert.match(desktop,new RegExp('async fn '+command+'\\('));
  }
  assert.match(ui,/async function searchQuickOpen\(/);
  assert.match(ui,/async function closeTab\(/);
  assert.match(ui,/sourceQuickOpenResults/);
  assert.match(ui,/draftRecoveryByTab/);
});
test('IDE-MERGE-03 restored SCM Job WFP source is opt-in, native-only and no LLDB grant', () => {
  const supervisor=source('../../supervisor/src/lib.rs');
  const cargo=source('../../supervisor/Cargo.toml');
  const desktop=source('../src-tauri/src/lib.rs');
  const desktopCargo=source('../src-tauri/Cargo.toml');
  const nativeMenu=source('../src-tauri/src/fin04w2_c4_desktop.rs');
  for(const name of ['fin04w2-protocol','fin04w2-scm','fin04w2-contained-probe','fin04w2-c2'])
    assert.match(cargo,new RegExp(name+' = '));
  assert.match(supervisor,/mod windows_fin04w2_kernel;/);
  assert.match(supervisor,/pub mod windows_fin04w2_service;/);
  assert.match(supervisor,/consume_grant_after_c2_service_admission/);
  assert.match(desktopCargo,/fin04w2-c4-desktop = \["native-build"/);
  assert.match(desktop,/compile_error!\("FIN-04W2-C4 native menu is Windows debug-only/);
  assert.match(desktop,/fin04w2_c4_desktop::install\(app\)/);
  assert.doesNotMatch(nativeMenu,/#\[tauri::command\]/);
  assert.match(nativeMenu,/FIN04C4_AUTOMATED_APPROVAL_FORBIDDEN/);
  assert.match(nativeMenu,/run_fixed_after_visible_native_consent/);
});
