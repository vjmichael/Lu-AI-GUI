import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const read = path => readFileSync(new URL(path, import.meta.url), 'utf8');

test('G04D developer-only variable inspection has no arbitrary DAP or OS authority', () => {
  const commands = read('../src-tauri/src/lib.rs');
  const cargo = read('../src-tauri/Cargo.toml');
  const backend = read('../src-tauri/src/debug_backend.rs');
  assert.match(commands, /async fn ide_debug_inspect_project_variables\(\s*app: AppHandle,\s*registry: State<'_, Arc<IdeWorkspaceRegistry>>,\s*project_id: String,/);
  assert.match(commands, /ReviewedNativeAction::InspectVariables/);
  assert.match(commands, /IDE_G04D_VARIABLE_INSPECTION_NOT_INSTALLED/);
  assert.match(commands, /feature = "dev046d-native-dialog"/);
  assert.match(commands, /compile_error!\("DEV-04\.6D native dialog must not ship in release/);
  assert.match(cargo, /dev046d-native-dialog = \["native-project-debug"/);
  assert.doesNotMatch(cargo, /default = \[[^\]]*dev046d-native-dialog/);
  assert.match(backend, /inspect_one_protected_native_variables\(/);
  assert.match(backend, /native_dialog_variables_redacted_uncertain/);
  assert.match(backend, /read\.variables\.iter\(\)\.any\(\|v\| !v\.value_redacted\)/);
});

test('G04D one-use OS owner and human challenge are checked on every read', () => {
  const native = read('../../supervisor/src/windows_lsp_worker.rs');
  const gateway = read('../../supervisor/src/dap_transport.rs');
  const protocol = read('../../host/src/dap_session.rs');
  assert.match(native, /pub fn inspect_one_protected_native_variables\(/);
  assert.match(native, /NativeConsentMarker::for_trusted_modal/);
  assert.match(native, /verify_lldb_kernel_event_owner\(/);
  assert.match(native, /action\s*\.consume_for_receipt_test\(/);
  assert.match(native, /slot\s*\.take_variables\(\)/);
  assert.match(native, /ci_send_scopes\(dispatched\.frame_id\(\)\)/);
  assert.match(native, /ci_send_variables\(\)/);
  assert.match(native, /action\s*\.record_uncertain_result\(/);
  assert.match(gateway, /pub\(crate\) fn ci_send_scopes/);
  assert.match(gateway, /pub\(crate\) fn ci_send_variables/);
  assert.match(protocol, /pub fn ci_scope_ready/);
  assert.match(protocol, /pub fn ci_take_redacted_variables/);
  assert.doesNotMatch(native, /pub fn (evaluate|setVariable|readMemory)/);
});

test('G04D restart denies reconnect and variable values remain redacted', () => {
  const receipts = read('../../supervisor/src/debug_action_receipts.rs');
  const protocol = read('../../host/src/dap_session.rs');
  const backend = read('../src-tauri/src/debug_backend.rs');
  assert.match(receipts, /inspect_action_receipts_after_restart/);
  assert.match(receipts, /"Prepared" \| "Approved" \| "Consumed"/);
  assert.match(receipts, /"ResultUncertain"/);
  assert.match(protocol, /value_redacted: true/);
  assert.match(protocol, /entries\.len\(\) > 16/);
  assert.match(protocol, /self\.ci_variable_probe = None/);
  assert.match(backend, /DEBUG_KERNEL_RECOVERY_REQUIRED_BEFORE_ANY_NEW_RUN/);
  assert.match(backend, /DEV046D_INTERRUPTED_ACTION_OS_RECONCILIATION_REQUIRED/);
});
