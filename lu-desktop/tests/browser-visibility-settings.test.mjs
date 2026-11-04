import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';

const ui=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
const host=readFileSync(new URL('../src-tauri/src/lib.rs',import.meta.url),'utf8');
const css=readFileSync(new URL('../web/styles.css',import.meta.url),'utf8');
const browserHost=readFileSync(new URL('../src-tauri/src/native_browser_host.rs',import.meta.url),'utf8');

test('browser visibility setting is read from native SQLite and not invented in the UI',()=>{
  assert.ok(ui.includes("invoke('lu_browser_settings')"));
  assert.ok(host.includes('browser_preferences: lu_core::browser_preferences::BrowserPreferences'));
  assert.ok(host.includes('fn lu_browser_settings('));
  assert.ok(host.includes('browser-preferences.sqlite'));
  assert.ok(!ui.includes("useState('always-visible')"));
});
test('user can select automatic-hidden or always-visible without launching a browser',()=>{
  assert.ok(ui.includes('aria-label="Browser visibility settings"'));
  assert.ok(ui.includes('name="browser-visibility"'));
  assert.ok(ui.includes("saveBrowserVisibility('automatic')"));
  assert.ok(ui.includes("saveBrowserVisibility('always-visible')"));
  assert.ok(ui.includes("invoke('lu_set_browser_visibility', {mode})"));
  assert.ok(host.includes('"automatic"=>P::AutomaticHidden'));
  assert.ok(host.includes('"always-visible"=>P::AlwaysVisible'));
  assert.ok(!ui.includes("invoke('lu_start_browser'"));
});
test('setting is disclosed as next-session preference and requires native host acceptance',()=>{
  assert.ok(ui.includes('This preference applies to the next browser session.'));
  assert.ok(ui.includes('Native window visibility and human takeover are under development.'));
  assert.ok(css.includes('.browserVisibilityChoices'));
  assert.ok(ui.includes('disabled={savingBrowserMode || browserMode === null}'));
});

test('runtime status and stop are host-owned, and no unpinned Chromium launches',()=>{
  assert.ok(host.includes('native_browser_host::DesktopBrowserHost::new('));
  assert.ok(host.includes('fn lu_browser_runtime_status('));
  assert.ok(host.includes('fn lu_browser_stop('));
  assert.ok(ui.includes("invoke('lu_browser_runtime_status')"));
  assert.ok(ui.includes("invoke('lu_browser_stop')"));
  assert.ok(ui.includes('disabled={!browserRuntime?.running}'));
  assert.ok(browserHost.includes('EMBEDDED_BROWSER_MANIFEST'));
  assert.ok(browserHost.includes('manifest.status!="reviewed"'));
  assert.ok(browserHost.includes('pin_matches(&dir.join(&file.path),&file.sha256,file.bytes)'));
  assert.ok(browserHost.includes('Lu browser runtime unavailable: missing reviewed, pinned package'));
  assert.ok(!host.includes('fn lu_browser_navigate('));
});
