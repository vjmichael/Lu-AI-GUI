import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';

const ui=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
const host=readFileSync(new URL('../src-tauri/src/lib.rs',import.meta.url),'utf8');
const browserHost=readFileSync(new URL('../src-tauri/src/native_browser_host.rs',import.meta.url),'utf8');

test('visibility is a native persisted preference, never an implicit browser grant',()=>{
  assert.ok(ui.includes("invoke('lu_browser_visibility')"));
  assert.ok(host.includes('fn lu_browser_visibility('));
  assert.ok(host.includes('fn lu_set_browser_visibility('));
  assert.ok(ui.includes("invoke('lu_set_browser_visibility',{mode})"));
  assert.ok(ui.includes('value="automatic_hidden"'));
  assert.ok(ui.includes('value="always_visible"'));
  assert.ok(ui.includes('This preference does not launch or authorize browsing.'));
  assert.ok(!ui.includes("invoke('lu_start_browser'"));
});
test('browser runtime is host-owned and launch requires reviewed pinned package',()=>{
  assert.ok(host.includes('fn lu_browser_runtime_status('));
  assert.ok(host.includes('fn lu_browser_stop('));
  assert.ok(ui.includes("browserOperation('lu_browser_runtime_status')"));
  assert.ok(ui.includes("browserOperation('lu_browser_start',{chatId,origins:browserOrigins})"));
  assert.ok(ui.includes('Authorize read-only Browser'));
  assert.ok(ui.includes('disabled={browserBusy||!chatId||!browserRuntime?.launchReady}'));
  assert.ok(browserHost.includes('EMBEDDED_BROWSER_MANIFEST'));
  assert.ok(browserHost.includes('manifest.status!="reviewed"'));
  assert.ok(browserHost.includes('pin_matches(&dir.join(&file.path),&file.sha256,file.bytes)'));
  assert.ok(!host.includes('fn lu_browser_navigate('));
});
