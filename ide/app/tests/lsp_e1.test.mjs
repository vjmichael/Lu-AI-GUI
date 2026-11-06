import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const read = path => readFileSync(new URL(path, import.meta.url), 'utf8');

test('E1 language-server discovery remains non-executable and independent of Lu', () => {
  const host = read('../../host/src/lsp.rs');
  const desktop = read('../src-tauri/src/lib.rs');
  const moduleList = read('../../host/src/lib.rs');
  assert.match(moduleList, /pub mod lsp;/);
  assert.match(desktop, /async fn ide_list_language_servers\(/);
  assert.match(desktop, /ide_list_language_servers,/);
  assert.match(host, /executable_from_desktop: false/g);
  assert.match(host, /lifecycle: "catalog_only"/);
  assert.match(host, /pub fn available_servers\(/);
  assert.match(host, /pub fn encode_frame\(/);
  assert.match(host, /pub struct FrameParser/);
  for (const name of ['rust-analyzer', 'clangd', 'pyright', 'ruff',
    'typescript-language-server', 'vscode-html-language-server',
    'vscode-css-language-server', 'vscode-json-language-server']) {
    assert.ok(host.includes('id: "' + name + '"'), name);
  }
  assert.doesNotMatch(host, /Command::new|\.spawn\(|std::process::Command|install_package|execute_windows_worker/);
  assert.doesNotMatch(desktop, /ide_run_job|ide_launch_worker|ide_execute_job|ide_start_language_server/);
});

test('E1 frames are length-bounded and diagnostics require current source identity', () => {
  const host = read('../../host/src/lsp.rs');
  assert.match(host, /MAX_MESSAGE: usize = 512 \* 1024/);
  assert.match(host, /MAX_HEADER: usize = 256/);
  assert.match(host, /serde_json::from_slice/);
  assert.match(host, /InvalidLength/);
  assert.match(host, /OversizedHeader/);
  assert.match(host, /pub fn same_revision/);
  assert.match(host, /buffer_revision/);
  assert.match(host, /disk_sha256/);
});
