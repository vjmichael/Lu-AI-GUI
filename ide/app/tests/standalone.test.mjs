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

test('WebView uses only four native project lifecycle commands', () => {
  const web = read('../web/main.jsx');
  const calls = [...web.matchAll(/(?:invoke|operation)\('(ide_[a-z_]+)'/g)].map(v => v[1]);
  assert.ok(calls.includes('ide_list_projects'));
  assert.ok(calls.includes('ide_choose_project'));
  assert.ok(calls.includes('ide_close_project'));
  assert.ok(calls.includes('ide_open_project'));
  assert.deepEqual([...new Set(calls)].sort(), [
    'ide_choose_project', 'ide_close_project', 'ide_list_projects', 'ide_open_project'
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
