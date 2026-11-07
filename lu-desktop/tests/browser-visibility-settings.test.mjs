import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';

const ui=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
const host=readFileSync(new URL('../src-tauri/src/lib.rs',import.meta.url),'utf8');
const browserHost=readFileSync(new URL('../src-tauri/src/native_browser_host.rs',import.meta.url),'utf8');

test('browser visibility is loaded from native preferences, not model text',()=>{
  assert.ok(ui.includes("invoke('lu_browser_visibility')"));
  assert.ok(host.includes('browser_preferences: lu_core::browser_preferences::BrowserPreferences'));
  assert.ok(host.includes('fn lu_browser_visibility('));
  assert.ok(host.includes('state.browser_preferences.visibility()'));
});
test('browser modes require persisted native host approval without launching a session',()=>{
  assert.ok(ui.includes('aria-label="Browser visibility settings"'));
  assert.ok(ui.includes('id="browserVisibilityMode"'));
  assert.ok(ui.includes('value="automatic_hidden"'));
  assert.ok(ui.includes('value="always_visible"'));
  assert.ok(ui.includes("invoke('lu_set_browser_visibility',{mode})"));
  assert.ok(host.includes('"automatic_hidden"=>Mode::AutomaticHidden'));
  assert.ok(host.includes('"always_visible"=>Mode::AlwaysVisible'));
  assert.ok(!ui.includes("invoke('lu_start_browser'"));
});
test('browser settings UI accurately distinguishes mode choice from task admission',()=>{
  assert.ok(ui.includes('This preference does not launch or authorize browsing.'));
  assert.ok(ui.includes('Lu browser runtime unavailable') || ui.includes('Checking native browser runtime admission'));
  assert.ok(ui.includes('browserPreferenceError'));
});
test('native browser status and stop retain host authority and reviewed runtime pinning',()=>{
  assert.ok(host.includes('native_browser_host::DesktopBrowserHost::new('));
  assert.ok(host.includes('fn lu_browser_runtime_status('));
  assert.ok(host.includes('async fn lu_browser_stop('));
  assert.ok(ui.includes("invoke('lu_browser_runtime_status')"));
  assert.ok(ui.includes("browserOperation('lu_browser_stop')"));
  assert.ok(browserHost.includes('EMBEDDED_BROWSER_MANIFEST'));
  assert.ok(browserHost.includes('manifest.status!="reviewed"'));
  assert.ok(browserHost.includes('pin_matches(&dir.join(&file.path),&file.sha256,file.bytes)'));
  assert.ok(!host.includes('fn lu_browser_navigate('));
});
