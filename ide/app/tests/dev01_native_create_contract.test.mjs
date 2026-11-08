import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';

const read = file => readFileSync(new URL(file,import.meta.url),'utf8');
const ui=read('../web/main.jsx');
const tauri=read('../src-tauri/src/lib.rs');
const native=read('../../host/src/source_create.rs');
const browser=read('../../host/src/browser.rs');

test('visible Create file/folder always invokes native scoped exclusive operation',()=>{
  for(const text of [
    "setEntryCreateKind('file')", "setEntryCreateKind('directory')",
    "onSubmit={createNativeEntry}",
    "invoke('ide_create_source_entry'",
    "projectId, relativeDir: parentDir, name, kind",
    "created.projectId !== projectId",
    "created.relativePath !== path",
    "currentDirectoryRef.current !== parentDir",
    "setHint('Created empty '",
    "Native exclusive creation only. Existing files are never overwritten.",
  ]) assert.ok(ui.includes(text), text);
  for(const text of [
    '#[tauri::command]\nasync fn ide_create_source_entry',
    'registry\n            .create_source_entry(&project_id, &relative_dir, &name, &kind)',
    'ide_create_source_entry,'
  ]) assert.ok(tauri.includes(text), text);
});

test('native creation has no ambient path write or overwrite and shares Save lock',()=>{
  for(const text of [
    'source_capability(project_id)', 'descend(base, &parent_parts)',
    'self.save_lock.lock()', 'OpenOptions::new()', 'options.write(true).create_new(true)',
    'parent.create_dir(name)', 'validate_relative(&relative_path, false)',
    'portable_components(&parent_parts)', 'portable_components(&name_parts)',
    'BrowserError::AlreadyExists', 'MAX_SIBLING_SCAN'
  ]) assert.ok(native.includes(text), text);
  assert.ok(browser.includes('SOURCE_TARGET_ALREADY_EXISTS'));
  for(const bad of ['fs::write(', 'fs::rename(', 'remove_file(', 'remove_dir_all(',
                    'std::process', 'Command::new(', 'ambient_authority()', 'canonicalize(']){
    assert.ok(!native.includes(bad),'native create must not use '+bad);
  }
});

test('explicit create never bypasses editor unsaved state or parser',()=>{
  const start=ui.indexOf('async function createNativeEntry(event)');
  assert.ok(start>=0);
  const end=ui.indexOf('async function openDirectory(project, path)',start);
  assert.ok(end>start);
  const code=ui.slice(start,end);
  assert.ok(code.includes('setEntryCreateBusy(true)'));
  assert.ok(code.includes('setEntryCreateBusy(false)'));
  assert.ok(code.includes("entryCreateKind !== 'file' && entryCreateKind !== 'directory'"));
  for(const bad of ['setTabs(', 'editorStates.current.clear(', 'discardProjectTabs(',
    "invoke('ide_save_source_file'", 'window.__TAURI_INTERNALS__']) {
    assert.ok(!code.includes(bad),bad);
  }
});
