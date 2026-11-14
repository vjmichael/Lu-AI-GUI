import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read = path => readFileSync(new URL(path, import.meta.url), 'utf8');

test('D6D release remains closed and dev feature is explicit', () => {
  const backend = read('../src-tauri/src/debug_backend.rs');
  const commands = read('../src-tauri/src/lib.rs');
  assert.match(backend, /operation_control_supported:\s*cfg!\(all\(/);
  assert.match(backend, /feature = "dev046d-native-dialog"/);
  assert.match(backend, /DEV046D_NATIVE_HUMAN_ACTION_APPROVAL_NOT_ACCEPTED/);
  assert.match(commands, /"operationControlSupported":false/);
});
test('D6C CI-only debugger cannot enter installed release build', () => {
  const cargo = read('../src-tauri/Cargo.toml');
  assert.doesNotMatch(cargo, /dev046c-ci-control|dev045d5-ci-event-owner/);
  assert.match(cargo, /dev046d-native-dialog = \["native-project-debug"/);
  const control = read('../../supervisor/src/debug_control_dispatch.rs');
  assert.match(control, /#!\[cfg\(any\(test,\s*all\(windows,\s*debug_assertions,\s*feature = "dev046c-ci-control"\)\)\)\]/);
});
test('D6D interrupted cleanup cannot resume prior debugger approval', () => {
  const backend = read('../src-tauri/src/debug_backend.rs');
  assert.match(backend, /No debugger or project executable will be restarted/);
  assert.match(backend, /DEBUG_OS_TREE_RECONCILED_NO_REEXECUTION/);
  const receipts = read('../../supervisor/src/debug_action_receipts.rs');
  assert.match(receipts, /inspect_action_receipts_after_restart/);
  assert.match(receipts, /"Prepared" \| "Approved" \| "Consumed"/);
});
