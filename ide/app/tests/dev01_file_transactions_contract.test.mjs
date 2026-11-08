import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read = path => readFileSync(new URL(path, import.meta.url), 'utf8');
const native = read('../../host/src/source_file_transactions.rs');
const ui = read('../web/main.jsx');
const tauri = read('../src-tauri/src/lib.rs');

test('native file-only transfer cannot clobber an existing target', () => {
  for (const marker of [
    'source.hard_link(source_name, dest, dest_name)',
    'source.remove_file(source_name)?',
    'reject_case_alias(dest, dest_name)?',
    'read_verified(source, source_name, expected)?',
    'read_verified(dest, dest_name, expected)',
    'self.save_lock.lock()', 'record.version != 1',
    'file.sync_all()?', 'MAX_TRASH_ITEMS',
  ]) assert.ok(native.includes(marker), marker);
  for (const forbidden of [
    '.rename(', 'remove_dir_all(', 'std::fs::rename(',
    'Command::new(', 'ambient_authority()',
  ]) assert.ok(!native.includes(forbidden), forbidden);
});

test('real UI refuses mutations to open editor tabs and verifies native receipts', () => {
  const start=ui.indexOf('// DEV-01.3 file-only mutations');
  const end=ui.indexOf('async function createNativeEntry',start);
  assert.ok(start>0 && end>start);
  const block=ui.slice(start,end);
  for (const marker of [
    'tab.relativePath === path',
    "invoke('ide_preview_source_file'",
    "invoke('ide_move_source_file'",
    "invoke('ide_trash_source_file'",
    "invoke('ide_list_trashed_sources'",
    "invoke('ide_restore_source_file'",
    'expectedRevision: fresh.revision',
    'window.confirm(',
    'result.currentRelativePath !== item.originalRelativePath',
  ]) assert.ok(block.includes(marker), marker);
  for (const forbidden of ['setTabs(', 'editorStates.current.delete(', 'discardProjectTabs(']) {
    assert.ok(!block.includes(forbidden), forbidden);
  }
});

test('Tauri commands expose project IDs and relative paths only', () => {
  for (const marker of [
    'async fn ide_move_source_file(',
    'async fn ide_trash_source_file(',
    'async fn ide_restore_source_file(',
    'async fn ide_list_trashed_sources(',
    'registry.move_source_file(',
    'registry.trash_source_file(',
    'registry.restore_source_file(',
    'registry.list_trashed_source_files(',
  ]) assert.ok(tauri.includes(marker), marker);
});
