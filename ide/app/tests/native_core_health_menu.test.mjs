import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';

const native = readFileSync(new URL('../src-tauri/src/native_core_health_menu.rs', import.meta.url), 'utf8');
const app = readFileSync(new URL('../src-tauri/src/lib.rs', import.meta.url), 'utf8');
const lifecycle = readFileSync(new URL('../../host/src/installed_core_health.rs', import.meta.url), 'utf8');

test('native Health menu is opt-in Linux only, admitted after verified startup', () => {
  assert.match(app, /cfg\(all\(feature = "native-core-health", target_os = "linux"\)\)/);
  assert.match(app, /InstalledCoreHealthRuntime::from_installed_linux\(\)/);
  assert.match(app, /native_core_health_menu::install\(app\)\?/);
  assert.match(app, /native_core_health_menu::on_menu_event\(app, &event\)/);
});
test('native OS confirmation separates menu event from one-shot issuer', () => {
  assert.match(native, /reserve_native_action\(\)/);
  assert.match(native, /MessageDialogButtons::OkCancelCustom/);
  assert.match(native, /let disposition = permit.finish\(approved\)/);
  assert.match(lifecycle, /native_action_claimed\s*\.compare_exchange\(/);
  assert.match(lifecycle, /if !human_confirmed \{\s*self\.runtime\.session\.cancel\(\)/);
});
test('Core Health does not appear in frontend invoke surface', () => {
  const invocation = app.split('.invoke_handler(tauri::generate_handler![')[1];
  assert.ok(invocation);
  assert.doesNotMatch(invocation, /native_core_health_menu|core_ipc_|installed_core_health|core_health_request/);
  assert.doesNotMatch(native, /#\[tauri::command\]|emit\(|eval\(/);
});
