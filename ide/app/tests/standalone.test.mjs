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

test('WebView has only project lifecycle and read-only source-browse commands', () => {
  const web = read('../web/main.jsx');
  const calls = [...web.matchAll(/(?:invoke|operation)\('(ide_[a-z_]+)'/g)].map(v => v[1]);
  assert.ok(calls.includes('ide_list_projects'));
  assert.ok(calls.includes('ide_choose_project'));
  assert.ok(calls.includes('ide_close_project'));
  assert.ok(calls.includes('ide_open_project'));
  assert.ok(calls.includes('ide_list_source_directory'));
  assert.ok(calls.includes('ide_preview_source_file'));

  assert.deepEqual([...new Set(calls)].sort(), [
    'ide_choose_project', 'ide_close_project', 'ide_list_projects',
    'ide_list_source_directory', 'ide_open_project', 'ide_preview_source_file'
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
  assert.match(web, /<pre className="sourceCode"><code>\{preview\.text\}<\/code><\/pre>/);
  assert.match(web, /entry\.kind === 'directory'/);
  assert.match(web, /entry\.kind === 'file'/);
  assert.doesNotMatch(web, /dangerouslySetInnerHTML|innerHTML|eval\(|new Function\(/);
  assert.match(host, /cap_std/);
  assert.match(host, /MAX_TEXT_BYTES/);
  assert.match(host, /symlink_metadata/);
  assert.doesNotMatch(host, /Command::new|std::process::Command/);
});
