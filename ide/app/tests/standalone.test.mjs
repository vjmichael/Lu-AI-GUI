import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, statSync } from 'node:fs';
const read = path => readFileSync(new URL(path, import.meta.url), 'utf8');

test('independent desktop package has no Lu desktop or Lu Core runtime dependency', () => {
  const cargo = read('../src-tauri/Cargo.toml');
  assert.match(cargo, /name = "lu-ide-desktop"/);
  assert.match(cargo, /lu-ide-workspace/);
  assert.doesNotMatch(cargo, /lu-core|lu-desktop/);
  assert.match(read('../src-tauri/tauri.conf.json'), /org\.lu\.ide/);
});

test('WebView exposes only project lifecycle, source browse and explicit save commands', () => {
  const web = read('../web/main.jsx');
  const calls = [...web.matchAll(/(?:invoke|operation)\('(ide_[a-z_]+)'/g)].map(v => v[1]);
  assert.ok(calls.includes('ide_list_projects'));
  assert.ok(calls.includes('ide_choose_project'));
  assert.ok(calls.includes('ide_close_project'));
  assert.ok(calls.includes('ide_open_project'));
  assert.ok(calls.includes('ide_list_source_directory'));
  assert.ok(calls.includes('ide_preview_source_file'));
  assert.ok(calls.includes('ide_save_source_file'));

  assert.deepEqual([...new Set(calls)].sort(), [
    'ide_choose_project', 'ide_close_project', 'ide_list_projects',
    'ide_list_source_directory', 'ide_open_project', 'ide_preview_source_file',
    'ide_save_source_file'
  ]);
  for(const term of ['lu_generate','lu_load_model','shell_exec','exec_process','package_install']) {
    assert.ok(!web.includes(term), term);
  }
});

test('host uses native chooser and does not accept WebView project paths', () => {
  const host = read('../src-tauri/src/lib.rs');
  assert.match(host, /blocking_pick_folder\(\)/);
  assert.match(host, /IdeWorkspaceRegistry::open/);
  assert.match(host, /project_id: String/);
  assert.doesNotMatch(host, /project_path: String|root_path: String|\.command\(/);
});

test('standalone UI build exists and uses no external services', () => {
  const html = read('../dist/index.html');
  assert.match(html, /assets\/app\.js/);
  const js = read('../dist/assets/app.js');
  assert.ok(statSync(new URL('../dist/assets/app.js', import.meta.url)).size > 50000);
  assert.ok(js.includes('ide_choose_project'));
  assert.ok(!html.includes('https://'));
});

test('source explorer displays escaped text only and never executes a project file', () => {
  const web = read('../web/main.jsx');
  const host = read('../../host/src/browser.rs');
  assert.match(web, /<CodeEditor/);
  assert.match(web, /editorState=\{editorStates\.current\.get\(activeTab\.key\)\}/);
  assert.match(web, /entry\.kind === 'directory'/);
  assert.match(web, /entry\.kind === 'file'/);
  assert.doesNotMatch(web, /dangerouslySetInnerHTML|innerHTML|eval\(|new Function\(/);
  assert.match(host, /cap_std/);
  assert.match(host, /MAX_TEXT_BYTES/);
  assert.match(host, /symlink_metadata/);
  assert.doesNotMatch(host, /Command::new|std::process::Command/);
});

test('R3 editor has explicit revision-checked save, dirty buffers and conflict handling', () => {
  const web = read('../web/main.jsx');
  const native = read('../src-tauri/src/lib.rs');
  const editor = read('../web/code_editor.jsx');
  assert.match(web, /expectedRevision: base/);
  assert.match(web, /text: content/);
  assert.match(web, /FILE_CHANGED_ON_DISK/);
  assert.match(web, /Your unsaved edits are preserved/);
  assert.match(web, /window\.confirm/);
  assert.match(web, /key === 's'/);
  assert.match(web, /saveTab\(activeTab\)/);
  assert.match(web, /beforeunload/);
  assert.match(native, /fn ide_save_source_file/);
  assert.match(native, /\.save_source_file\(&project_id, &relative_path, &expected_revision, &text\)/);
  assert.match(editor, /EditorView\.updateListener\.of/);
  assert.match(editor, /basicSetup/);
  assert.match(editor, /rust\(\)/);
  assert.match(editor, /python\(\)/);
  assert.match(editor, /javascript\(/);
  assert.doesNotMatch(editor, /eval\(|Function\(/);
});

test('saved preview includes revision and uses capability-relative atomic replace', () => {
  const browser = read('../../host/src/browser.rs');
  const editor = read('../../host/src/editor.rs');
  assert.match(browser, /pub revision: String/);
  assert.match(browser, /Sha256::digest/);
  assert.match(editor, /create_new\(true\)/);
  assert.match(editor, /file\.sync_all\(\)/);
  assert.match(editor, /parent\.rename\(&temp, &parent, &leaf\)/);
  assert.match(editor, /BrowserError::Conflict/);
  assert.match(editor, /self\.source_capability\(project_id\)/);
  assert.doesNotMatch(editor, /Command::new|std::process::Command/);
});
